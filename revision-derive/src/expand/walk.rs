//! Derive emitter for [`WalkRevisioned`].
//!
//! For every `#[revisioned(...)]` type whose `walk` derive flag is enabled
//! (defaults to the `deserialize` flag) the emitter generates:
//!
//! - A per-type walker struct `<Type>Walker<'r, R>` whose internal
//!   representation is one of:
//!   - **Wire**: holds `&'r mut R`, the wire revision, and a position counter.
//!     Used for the current-rev fast path and for older revs of types that do
//!     not use `convert_fn` (purely additive evolution).
//!   - **Materialised**: holds an owned `Vec<u8>` of bytes serialised at the
//!     current schema revision, plus a cursor. Used for older revs of types
//!     that use `convert_fn`; the conversion has already been honoured by
//!     `DeserializeRevisioned` and the bytes are then re-encoded so the
//!     walker can read sequentially without rev branching.
//! - The `WalkRevisioned` impl whose `walk_revisioned` reads the `u16` header
//!   and chooses the appropriate mode.
//! - Named per-field methods on structs (`decode_<name>` / `skip_<name>` /
//!   `walk_<name>`) that handle wire-mode rev branching internally.
//! - Per-variant accessors on enums (`into_<variant>` for single-field tuple
//!   variants, `is_<variant>` boolean check), `discriminant()`, and a
//!   `walk_revisioned_variant_name(wire_rev, disc)` lookup table.
//! - `walk_revisioned_field_names(wire_rev)` lookup for structs.
//!
//! The Wire and Materialised arms share an identical surface API; only the
//! byte source differs. For purely additive types the materialised arm is
//! effectively unreachable but is still emitted (to keep the walker shape
//! uniform); compilers eliminate the unused arm.

use proc_macro2::TokenStream;
use quote::{TokenStreamExt, format_ident, quote};
use syn::Ident;

use std::collections::HashMap;

use crate::ast::attributes::VariantSize;
use crate::ast::history::{HistoryEntry, StructEncoding};
use crate::ast::{Enum, Field, FieldName, Fields, Item, ItemKind, Struct, Variant, Visit};

use super::common::CalcDiscriminant;

/// Emit the [`WalkRevisioned`] impl and supporting types for a
/// `#[revisioned(...)]` item.
///
/// `has_convert_fn` is the AST-derived flag from `HasConvertFn`; when the
/// type does not use `convert_fn` anywhere, the materialised path is omitted
/// at construction (the Wire arm handles all wire revisions).
///
/// `serialize_enabled` and `deserialize_enabled` come from the
/// `#[revisioned(serialize = .., deserialize = ..)]` attribute; both must be
/// true for the materialised path to be emitted (it needs to deserialize
/// then serialize).
pub fn emit_walk_impl(
	name: &Ident,
	revision: usize,
	item: &Item,
	history: &[HistoryEntry],
	has_convert_fn: bool,
	serialize_enabled: bool,
	deserialize_enabled: bool,
) -> syn::Result<TokenStream> {
	// Reject `walk` on a `convert_fn`-bearing type when either side of the
	// materialised round-trip is disabled. Without both `serialize` and
	// `deserialize` the wire-only walker can't apply the converter or skip
	// removed (`end = ..`) fields, which would silently miscompare bytes
	// across revisions.
	if has_convert_fn && !(serialize_enabled && deserialize_enabled) {
		return Err(syn::Error::new(
			name.span(),
			"`walk` on a type using `convert_fn` requires both `serialize = true` and \
			 `deserialize = true`: the walker's cross-revision materialised path needs \
			 to deserialize at the wire revision and re-serialize at the current \
			 revision. Either enable both, or set `walk = false`.",
		));
	}

	let revision_lit = revision as u16;
	let revision_error = format!("Invalid revision `{{}}` for type `{}`", name);
	let walker_name = format_ident!("{}Walker", name);
	let walker_repr_name = format_ident!("{}WalkerRepr", name);

	// Materialised path is only emitted when the type actually has at least
	// one `convert_fn` annotation AND can both serialize and deserialize.
	let materialise_supported = has_convert_fn && serialize_enabled && deserialize_enabled;

	let walker_struct = match &item.kind {
		ItemKind::Struct(_) => emit_struct_walker_struct(&walker_name, &walker_repr_name),
		ItemKind::Enum(_) => emit_enum_walker_struct(&walker_name, &walker_repr_name),
	};

	// Construction: read the u16 header, optionally materialise.
	let materialise_branch = if !materialise_supported {
		quote! {}
	} else {
		match &item.kind {
			ItemKind::Struct(_) => quote! {
				if __wire_rev != #revision_lit {
					let __value = Self::__deserialize_after_header(reader, __wire_rev)?;
					let mut __buf = ::std::vec::Vec::new();
					<Self as ::revision::SerializeRevisioned>::serialize_revisioned(&__value, &mut __buf)?;
					let mut __slice: &[u8] = __buf.as_slice();
					let _ = <u16 as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __slice)?;
					let __cursor = __buf.len() - __slice.len();
					return ::std::result::Result::Ok(#walker_name {
						repr: #walker_repr_name::Materialised {
							bytes: __buf,
							cursor: __cursor,
							offsets: ::std::option::Option::None,
							pos: 0,
							_marker: ::std::marker::PhantomData,
						},
					});
				}
			},
			ItemKind::Enum(_) => quote! {
				if __wire_rev != #revision_lit {
					let __value = Self::__deserialize_after_header(reader, __wire_rev)?;
					let mut __buf = ::std::vec::Vec::new();
					<Self as ::revision::SerializeRevisioned>::serialize_revisioned(&__value, &mut __buf)?;
					let mut __slice: &[u8] = __buf.as_slice();
					let _ = <u16 as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __slice)?;
					let mut __cursor = __buf.len() - __slice.len();
					let __mat_disc = {
						let mut __ms: &[u8] = &__buf[__cursor..];
						let __d = <u32 as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __ms)?;
						__cursor = __buf.len() - __ms.len();
						__d
					};
					return ::std::result::Result::Ok(#walker_name {
						repr: #walker_repr_name::Materialised {
							bytes: __buf,
							cursor: __cursor,
							discriminant: __mat_disc,
							pos: 0,
							_marker: ::std::marker::PhantomData,
						},
					});
				}
			},
		}
	};

	// For each optimised history entry, emit an arm that prepares the walker
	// for the optimised wire format:
	//
	// - For structs: advance past the `u32_le payload_length` (and any
	//   `[u32_le; field_count]` prologue for `struct = "indexed"`) so the
	//   subsequent Wire walker reads field bytes directly.
	// - For enums: read the 1-byte tag, slurp the payload per the variant's
	//   declared size class, and return a Materialised walker with
	//   `discriminant = variant_id` (the existing per-variant decode code on
	//   the Materialised arm reads the slurped payload directly).
	let optimised_struct_field_count = match &item.kind {
		ItemKind::Struct(s) => Some(alive_field_count(s, revision) as u32),
		ItemKind::Enum(_) => None,
	};
	let optimised_skip_arms: Vec<TokenStream> = history
		.iter()
		.filter(|h| h.is_optimised())
		.map(|h| -> syn::Result<TokenStream> {
			let rev_lit = h.revision.value as u16;
			match &item.kind {
				ItemKind::Struct(_) => {
					if matches!(h.struct_kind, StructEncoding::Indexed) {
						// Indexed struct: slurp the whole payload, parse
						// the offset table once, and return a Materialised
						// walker holding both. Per-field methods can then
						// jump directly to a field's byte range in O(1).
						let field_count = optimised_struct_field_count.unwrap_or(0) as usize;
						let field_count_u16 = field_count as u16;
						Ok(quote! {
							#rev_lit => {
								let mut __len_buf = [0u8; 4];
								::std::io::Read::read_exact(reader, &mut __len_buf)
									.map_err(::revision::Error::Io)?;
								let __payload_len = u32::from_le_bytes(__len_buf) as usize;
								let mut __payload: ::std::vec::Vec<u8> =
									::std::vec![0u8; __payload_len];
								::std::io::Read::read_exact(reader, &mut __payload)
									.map_err(::revision::Error::Io)?;
								// Tag the walker with the field count so
								// per-field decode knows it's the indexed
								// case; offsets are read lazily from
								// `bytes[i*4..i*4+4]` per access — no
								// Vec<u32> alloc.
								return ::std::result::Result::Ok(#walker_name {
									repr: #walker_repr_name::Materialised {
										bytes: __payload,
										cursor: 0,
										offsets: ::std::option::Option::Some(#field_count_u16),
										pos: 0,
										_marker: ::std::marker::PhantomData,
									},
								});
							}
						})
					} else {
						// Optimised sequential (no prologue): advance past the
						// u32_le length and continue with the Wire walker.
						Ok(quote! {
							#rev_lit => {
								let mut __len_buf = [0u8; 4];
								::std::io::Read::read_exact(reader, &mut __len_buf)
									.map_err(::revision::Error::Io)?;
								let _ = u32::from_le_bytes(__len_buf);
							}
						})
					}
				}
				ItemKind::Enum(e) => {
					emit_optimised_enum_walker_arm(e, h, rev_lit, &walker_name, &walker_repr_name)
				}
			}
		})
		.collect::<syn::Result<Vec<_>>>()?;

	let optimised_skip_dispatch = if optimised_skip_arms.is_empty() {
		quote! {}
	} else {
		quote! {
			match __wire_rev {
				#(#optimised_skip_arms)*
				_ => {}
			}
		}
	};

	let post_header_read = match &item.kind {
		ItemKind::Enum(_) => quote! {
			let __discriminant =
				<u32 as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?;
		},
		ItemKind::Struct(_) => quote! {},
	};

	let wire_constructor = match &item.kind {
		ItemKind::Enum(_) => quote! {
			#walker_repr_name::Wire {
				reader,
				wire_rev: __wire_rev,
				discriminant: __discriminant,
				pos: 0,
			}
		},
		ItemKind::Struct(_) => quote! {
			#walker_repr_name::Wire {
				reader,
				wire_rev: __wire_rev,
				pos: 0,
			}
		},
	};

	// Per-type method body
	let methods = match &item.kind {
		ItemKind::Struct(s) => emit_struct_methods(name, &walker_repr_name, revision, s)?,
		ItemKind::Enum(e) => emit_enum_methods(name, &walker_repr_name, revision, e)?,
	};

	// Introspection tables
	let variant_table = match &item.kind {
		ItemKind::Enum(e) => emit_variant_tables(name, revision, e)?,
		ItemKind::Struct(_) => quote! {},
	};
	let field_table = match &item.kind {
		ItemKind::Struct(s) => emit_field_tables(name, revision, s),
		ItemKind::Enum(_) => quote! {},
	};

	// Common revision accessor + raw repr access (for hand-written walkers).
	let revision_method = match &item.kind {
		ItemKind::Enum(_) => quote! {
			/// Wire revision of the encoded value being walked.
			#[inline]
			pub fn revision(&self) -> u16 {
				match &self.repr {
					#walker_repr_name::Wire { wire_rev, .. } => *wire_rev,
					#walker_repr_name::Materialised { .. } => #revision_lit,
				}
			}

			/// Variant discriminant on the wire (for `Wire`) or on the
			/// re-encoded current-rev bytes (for `Materialised`). Use
			/// [`walk_revisioned_variant_name`](Self::walk_revisioned_variant_name)
			/// to map this to a variant identifier.
			#[inline]
			pub fn discriminant(&self) -> u32 {
				match &self.repr {
					#walker_repr_name::Wire { discriminant, .. } => *discriminant,
					#walker_repr_name::Materialised { discriminant, .. } => *discriminant,
				}
			}
		},
		ItemKind::Struct(_) => quote! {
			/// Wire revision of the encoded value being walked.
			#[inline]
			pub fn revision(&self) -> u16 {
				match &self.repr {
					#walker_repr_name::Wire { wire_rev, .. } => *wire_rev,
					#walker_repr_name::Materialised { .. } => #revision_lit,
				}
			}
		},
	};

	let walk_impl = quote! {
		impl ::revision::WalkRevisioned for #name {
			type Walker<'r, R: ::revision::BorrowedReader + 'r> = #walker_name<'r, R>;

			fn walk_revisioned<'r, R: ::revision::BorrowedReader>(
				reader: &'r mut R,
			) -> ::std::result::Result<Self::Walker<'r, R>, ::revision::Error> {
				let __wire_rev =
					<u16 as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?;
				if __wire_rev == 0 || __wire_rev > #revision_lit {
					return ::std::result::Result::Err(::revision::Error::Deserialize(
						::std::format!(#revision_error, __wire_rev),
					));
				}
				#materialise_branch
				#optimised_skip_dispatch
				#post_header_read
				::std::result::Result::Ok(#walker_name { repr: #wire_constructor })
			}
		}

		impl<'r, R: ::revision::BorrowedReader + 'r> #walker_name<'r, R> {
			#revision_method
			#methods
		}
	};

	Ok(quote! {
		#walker_struct
		#walk_impl
		#variant_table
		#field_table
	})
}

/// Emit walker types for a revisioned struct.
fn emit_struct_walker_struct(walker_name: &Ident, walker_repr_name: &Ident) -> TokenStream {
	quote! {
		#[doc = "Walker for a revisioned struct. Generated by `#[revisioned(...)]`."]
		pub struct #walker_name<'r, R: ::revision::BorrowedReader + 'r> {
			#[doc(hidden)]
			pub repr: #walker_repr_name<'r, R>,
		}

		#[doc(hidden)]
		pub enum #walker_repr_name<'r, R: ::revision::BorrowedReader + 'r> {
			Wire {
				reader: &'r mut R,
				wire_rev: u16,
				pos: u32,
			},
			/// Materialised mode covers two cases:
			///
			/// 1. cross-revision `convert_fn` round-trip: bytes are the
			///    re-encoded current-rev value; `offsets` is `None` and the
			///    walker reads fields sequentially through `cursor`.
			/// 2. optimised + `struct = "indexed"` payload: bytes are the
			///    indexed-struct body (offset table parsed off the wire
			///    into `offsets`); per-field decode jumps via
			///    `offsets[i]` for O(1) random access.
			Materialised {
				bytes: ::std::vec::Vec<u8>,
				cursor: usize,
				/// `Some(field_count)` when the payload was an indexed struct.
				/// The offset table sits at the start of `bytes`; offsets are
				/// read on demand from `bytes[i*4..i*4+4]` per field access,
				/// avoiding a Vec<u32> alloc at walker construction.
				/// `None` for the sequential `convert_fn` round-trip case.
				offsets: ::std::option::Option<u16>,
				pos: u32,
				_marker: ::std::marker::PhantomData<&'r mut R>,
			},
		}
	}
}

/// Emit walker types for a revisioned enum.
fn emit_enum_walker_struct(walker_name: &Ident, walker_repr_name: &Ident) -> TokenStream {
	quote! {
		#[doc = "Walker for a revisioned enum. Generated by `#[revisioned(...)]`."]
		pub struct #walker_name<'r, R: ::revision::BorrowedReader + 'r> {
			#[doc(hidden)]
			pub repr: #walker_repr_name<'r, R>,
		}

		#[doc(hidden)]
		pub enum #walker_repr_name<'r, R: ::revision::BorrowedReader + 'r> {
			Wire {
				reader: &'r mut R,
				wire_rev: u16,
				/// Variant discriminant on the wire.
				discriminant: u32,
				pos: u32,
			},
			Materialised {
				bytes: ::std::vec::Vec<u8>,
				cursor: usize,
				/// Variant discriminant on the re-encoded current-rev bytes.
				discriminant: u32,
				pos: u32,
				_marker: ::std::marker::PhantomData<&'r mut R>,
			},
		}
	}
}

// -----------------------------------------------------------------------------
// Struct method emission
// -----------------------------------------------------------------------------

fn emit_struct_methods(
	owner_name: &Ident,
	walker_repr_name: &Ident,
	revision: usize,
	s: &Struct,
) -> syn::Result<TokenStream> {
	let mut out = TokenStream::new();
	let fields_iter: Vec<&Field> = match &s.fields {
		Fields::Named {
			fields,
			..
		}
		| Fields::Unnamed {
			fields,
			..
		} => fields.iter().collect(),
		Fields::Unit => Vec::new(),
	};

	// Latest-schema fields are those existing at `revision`. We emit one set
	// of methods per latest-schema field, in declaration order.
	for (latest_idx, f) in
		fields_iter.iter().filter(|f| f.attrs.options.exists_at(revision)).enumerate()
	{
		let method_base = field_method_base(&f.name, latest_idx);
		let decode_name = format_ident!("decode_{}", method_base);
		let skip_name = format_ident!("skip_{}", method_base);
		let walk_name = format_ident!("walk_{}", method_base);
		let into_walk_name = format_ident!("into_walk_{}", method_base);
		let ty = &f.ty;
		let pos_lit = latest_idx as u32;

		// At wire_rev `r`, this field exists iff `f.exists_at(r)`. For purely
		// additive evolution (no `convert_fn`), `start = N` is the only
		// annotation; below `N` the field is absent and we synthesise a
		// default. `end = N` requires `convert_fn`, which forces the
		// materialised path — so the wire arm here only ever sees fields
		// that exist at `wire_rev` or are added later.
		let start_val = f.attrs.options.start.as_ref().map(|s| s.value).unwrap_or(0) as u16;
		let always_present = start_val == 0;

		// Default synthesis: prefer the user-supplied `default_fn` (defined
		// as an inherent method on the source type, not on the walker) if
		// the field is added later (rev `< start`); otherwise
		// `Default::default`. Only emitted into the codegen when the field
		// could actually be absent (`start > 0`); otherwise `Default` would
		// be required even for types that never need it.
		// Pick the decode / skip call for this field. Fields with
		// `#[revision(indexed_map)]` / `#[revision(indexed_seq)]` route through
		// the runtime indexed helpers; everything else uses the type's own
		// `DeserializeRevisioned` / `SkipRevisioned` impl.
		let field_decode_call = if f.attrs.options.indexed_map {
			quote! {
				<#ty as ::revision::optimised::indexed::IndexedMapEncoded>::deserialize_indexed_map(reader)?
			}
		} else if f.attrs.options.indexed_seq {
			quote! {
				<#ty as ::revision::optimised::indexed::IndexedSeqEncoded>::deserialize_indexed_seq(reader)?
			}
		} else if f.attrs.options.indexed_set {
			quote! {
				<#ty as ::revision::optimised::indexed::IndexedSetEncoded>::deserialize_indexed_set(reader)?
			}
		} else {
			quote! {
				<#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?
			}
		};
		let field_skip_call = if f.attrs.options.indexed_map {
			quote! {
				<#ty as ::revision::optimised::indexed::IndexedMapEncoded>::skip_indexed_map(reader)?;
			}
		} else if f.attrs.options.indexed_seq {
			quote! {
				<#ty as ::revision::optimised::indexed::IndexedSeqEncoded>::skip_indexed_seq(reader)?;
			}
		} else if f.attrs.options.indexed_set {
			quote! {
				<#ty as ::revision::optimised::indexed::IndexedSetEncoded>::skip_indexed_set(reader)?;
			}
		} else {
			quote! {
				<#ty as ::revision::SkipRevisioned>::skip_revisioned(reader)?;
			}
		};

		let decode_wire_body = if always_present {
			quote! {
				let __v = #field_decode_call;
			}
		} else {
			let default_expr = if let Some(default) = f.attrs.options.default.as_ref() {
				let default = Ident::new(&default.value(), default.span());
				quote! { #owner_name::#default(*wire_rev)? }
			} else {
				quote! { <#ty as ::std::default::Default>::default() }
			};
			quote! {
				let __v = if *wire_rev >= #start_val {
					#field_decode_call
				} else {
					#default_expr
				};
			}
		};

		let skip_wire_body = if always_present {
			field_skip_call.clone()
		} else {
			quote! {
				if *wire_rev >= #start_val {
					#field_skip_call
				}
			}
		};

		// Consuming variant body — `reader` is `&'r mut R` (moved from `self.repr`),
		// `wire_rev` is `u16` (moved).
		let into_walk_wire_body = if always_present {
			quote! {
				<#ty as ::revision::WalkRevisioned>::walk_revisioned(reader)
			}
		} else {
			let walk_err_msg = format!(
				"into_walk_{} not available at wire revision {{}}: field added at revision {}",
				method_base, start_val,
			);
			quote! {
				if wire_rev < #start_val {
					return ::std::result::Result::Err(::revision::Error::Conversion(
						::std::format!(#walk_err_msg, wire_rev),
					));
				}
				<#ty as ::revision::WalkRevisioned>::walk_revisioned(reader)
			}
		};

		// Borrowing variant body — `reader` is `&mut &'r mut R` (from
		// `match &mut self.repr`); reborrow as `&mut R` via `&mut **reader`
		// so the inner walker carries a shorter lifetime tied to `&mut self`.
		// `wire_rev` is `&mut u16`, so dereference for comparison.
		let walk_wire_body_borrow = if always_present {
			quote! {
				let __w = <#ty as ::revision::WalkRevisioned>::walk_revisioned(&mut **reader)?;
				*pos = #pos_lit + 1;
				::std::result::Result::Ok(__w)
			}
		} else {
			let walk_err_msg = format!(
				"walk_{} not available at wire revision {{}}: field added at revision {}",
				method_base, start_val,
			);
			quote! {
				if *wire_rev < #start_val {
					return ::std::result::Result::Err(::revision::Error::Conversion(
						::std::format!(#walk_err_msg, *wire_rev),
					));
				}
				let __w = <#ty as ::revision::WalkRevisioned>::walk_revisioned(&mut **reader)?;
				*pos = #pos_lit + 1;
				::std::result::Result::Ok(__w)
			}
		};

		// For fields with `#[revision(indexed_map)]` / `#[revision(indexed_seq)]`,
		// the inner walker is `IndexedMapWalker` / `IndexedSeqWalker` rather
		// than the field type's own `WalkRevisioned::Walker`. These walkers
		// borrow from a payload slice, so we return an owned-bytes wrapper
		// (`OwnedIndexedMapView` / `OwnedIndexedSeqView`) that the caller
		// keeps alive while borrowing the walker from it.
		//
		// Implementation strategy: decode the field via the indexed
		// deserializer, then re-serialize into a Vec<u8> via the indexed
		// serializer. The wire format is canonical (sorted by key bytes), so
		// re-serializing produces identical bytes a second walker can read.
		// This is one extra alloc + walk per field — acceptable for the
		// "I want walker access" use case; callers who only need the
		// materialised value should use `decode_<field>` instead.
		let walk_return_ty;
		let walk_body;
		let into_walk_return_ty;
		let into_walk_body;
		if f.attrs.options.indexed_map {
			walk_return_ty = quote! {
				::revision::optimised::indexed::OwnedIndexedMapView<
					<#ty as ::revision::optimised::indexed::IndexedMapEncoded>::Key,
					<#ty as ::revision::optimised::indexed::IndexedMapEncoded>::Value,
				>
			};
			walk_body = quote! {
				let __v: #ty = self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedMapEncoded>::serialize_indexed_map(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::OwnedIndexedMapView::new(__bytes),
				)
			};
			into_walk_return_ty = walk_return_ty.clone();
			into_walk_body = quote! {
				let mut __self = self;
				let __v: #ty = __self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedMapEncoded>::serialize_indexed_map(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::OwnedIndexedMapView::new(__bytes),
				)
			};
		} else if f.attrs.options.indexed_seq {
			walk_return_ty = quote! {
				::revision::optimised::indexed::OwnedIndexedSeqView<
					<#ty as ::revision::optimised::indexed::IndexedSeqEncoded>::Item,
				>
			};
			walk_body = quote! {
				let __v: #ty = self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedSeqEncoded>::serialize_indexed_seq(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::OwnedIndexedSeqView::new(__bytes),
				)
			};
			into_walk_return_ty = walk_return_ty.clone();
			into_walk_body = quote! {
				let mut __self = self;
				let __v: #ty = __self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedSeqEncoded>::serialize_indexed_seq(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::OwnedIndexedSeqView::new(__bytes),
				)
			};
		} else if f.attrs.options.indexed_set {
			walk_return_ty = quote! {
				::revision::optimised::indexed::OwnedIndexedSetView<
					<#ty as ::revision::optimised::indexed::IndexedSetEncoded>::Item,
				>
			};
			walk_body = quote! {
				let __v: #ty = self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedSetEncoded>::serialize_indexed_set(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::OwnedIndexedSetView::new(__bytes),
				)
			};
			into_walk_return_ty = walk_return_ty.clone();
			into_walk_body = quote! {
				let mut __self = self;
				let __v: #ty = __self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedSetEncoded>::serialize_indexed_set(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::OwnedIndexedSetView::new(__bytes),
				)
			};
		} else {
			walk_return_ty = quote! { <#ty as ::revision::WalkRevisioned>::Walker<'_, R> };
			walk_body = quote! {
				match &mut self.repr {
					#walker_repr_name::Wire { reader, wire_rev, pos } => {
						#walk_wire_body_borrow
					}
					#walker_repr_name::Materialised { .. } => {
						::std::result::Result::Err(::revision::Error::Conversion(
							"walk_<field> is not supported on materialised walkers; use decode_<field>".into(),
						))
					}
				}
			};
			into_walk_return_ty = quote! { <#ty as ::revision::WalkRevisioned>::Walker<'r, R> };
			into_walk_body = quote! {
				match self.repr {
					#walker_repr_name::Wire { reader, wire_rev, pos: _ } => {
						#into_walk_wire_body
					}
					#walker_repr_name::Materialised { .. } => {
						::std::result::Result::Err(::revision::Error::Conversion(
							"into_walk_<field> is not supported on materialised walkers; use decode_<field>".into(),
						))
					}
				}
			};
		}

		out.append_all(quote! {
			/// Decode this field, advancing the walker.
			///
			/// For materialised walkers with an indexed-struct payload
			/// (optimised + `struct = "indexed"`), the offset table is
			/// consulted to jump directly to this field's bytes in O(1) —
			/// reading any field is independent of how many fields precede
			/// it on the wire.
			#[inline]
			pub fn #decode_name(&mut self) -> ::std::result::Result<#ty, ::revision::Error> {
				match &mut self.repr {
					#walker_repr_name::Wire { reader, wire_rev, pos } => {
						#decode_wire_body
						*pos = #pos_lit + 1;
						::std::result::Result::Ok(__v)
					}
					#walker_repr_name::Materialised { bytes, cursor, offsets, pos, .. } => {
						if let ::std::option::Option::Some(__field_count) = *offsets {
							// Indexed struct: parse this field's offset from
							// the table at the start of `bytes`. O(1) — two
							// u32 reads, no Vec<u32> alloc.
							let __off_base = (#pos_lit as usize) * 4;
							let __start = u32::from_le_bytes(
								bytes[__off_base..__off_base + 4]
									.try_into()
									.expect("4-byte slice"),
							) as usize;
							let __end = if (#pos_lit as usize) + 1 < (__field_count as usize) {
								let __next_base = __off_base + 4;
								u32::from_le_bytes(
									bytes[__next_base..__next_base + 4]
										.try_into()
										.expect("4-byte slice"),
								) as usize
							} else {
								bytes.len()
							};
							let mut __slice: &[u8] = &bytes[__start..__end];
							let __v = <#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __slice)?;
							*pos = #pos_lit + 1;
							::std::result::Result::Ok(__v)
						} else {
							// Sequential materialised path (convert_fn round-trip).
							let mut __slice: &[u8] = &bytes[*cursor..];
							let __v = <#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __slice)?;
							*cursor = bytes.len() - __slice.len();
							*pos = #pos_lit + 1;
							::std::result::Result::Ok(__v)
						}
					}
				}
			}

			/// Skip this field, advancing the walker.
			///
			/// Free under indexed-struct mode (the offset table already
			/// lets `decode_<field>` jump directly to any field; "skip"
			/// just bumps the position counter).
			#[inline]
			pub fn #skip_name(&mut self) -> ::std::result::Result<(), ::revision::Error> {
				match &mut self.repr {
					#walker_repr_name::Wire { reader, wire_rev, pos } => {
						#skip_wire_body
						*pos = #pos_lit + 1;
						::std::result::Result::Ok(())
					}
					#walker_repr_name::Materialised { bytes, cursor, offsets, pos, .. } => {
						if offsets.is_some() {
							// Indexed: skipping is free (any field reachable in O(1)).
							*pos = #pos_lit + 1;
							::std::result::Result::Ok(())
						} else {
							let mut __slice: &[u8] = &bytes[*cursor..];
							<#ty as ::revision::SkipRevisioned>::skip_revisioned(&mut __slice)?;
							*cursor = bytes.len() - __slice.len();
							*pos = #pos_lit + 1;
							::std::result::Result::Ok(())
						}
					}
				}
			}

			/// Walk into this field. For fields tagged
			/// `#[revision(indexed_map)]` / `#[revision(indexed_seq)]`,
			/// returns an [`OwnedIndexedMapView`] / [`OwnedIndexedSeqView`]
			/// the caller can borrow an [`IndexedMapWalker`] /
			/// [`IndexedSeqWalker`] from. For all other fields, returns the
			/// inner type's `WalkRevisioned::Walker` as usual (borrowing
			/// the parent walker for the duration of the sub-walk).
			///
			/// Materialised walkers (older-rev `convert_fn` types) return
			/// [`revision::Error::Conversion`] for the legacy path; the
			/// indexed branches re-serialise from the materialised value.
			///
			/// [`OwnedIndexedMapView`]: revision::optimised::indexed::OwnedIndexedMapView
			/// [`OwnedIndexedSeqView`]: revision::optimised::indexed::OwnedIndexedSeqView
			/// [`IndexedMapWalker`]: revision::optimised::IndexedMapWalker
			/// [`IndexedSeqWalker`]: revision::optimised::IndexedSeqWalker
			#[inline]
			pub fn #walk_name(
				&mut self,
			) -> ::std::result::Result<#walk_return_ty, ::revision::Error> {
				#walk_body
			}

			/// Walk into this field, consuming the parent walker. See
			/// [`Self::#walk_name`] for shape details.
			#[inline]
			pub fn #into_walk_name(
				self,
			) -> ::std::result::Result<#into_walk_return_ty, ::revision::Error> {
				#into_walk_body
			}
		});
	}

	Ok(out)
}

/// Helper: return the suffix for `decode_<x>` / `skip_<x>` / `walk_<x>`.
/// For named fields it's the field identifier; for tuple fields it's
/// `field_<idx>` (matching the existing `FieldName::to_binding` convention).
fn field_method_base(name: &FieldName, fallback_idx: usize) -> String {
	match name {
		FieldName::Ident(i) => i.to_string(),
		FieldName::Index(_) => format!("field_{}", fallback_idx),
	}
}

// -----------------------------------------------------------------------------
// Enum method emission
// -----------------------------------------------------------------------------

/// For one enum variant: each wire revision and its discriminant when present.
type VariantDiscriminantsByWireRev = Vec<(u16, Option<u32>)>;
/// Pre-computed variant, current-rev discriminant, and per-wire-rev table.
type EnumVariantDiscTable<'a> = Vec<(&'a Variant, u32, VariantDiscriminantsByWireRev)>;

fn emit_enum_methods(
	enum_name: &Ident,
	walker_repr_name: &Ident,
	revision: usize,
	e: &Enum,
) -> syn::Result<TokenStream> {
	let mut out = TokenStream::new();

	// Per-revision discriminant tables. We need:
	// - For `into_<variant>` at runtime we look up "what discriminant does
	//   <variant> have at wire_rev?" — derived from per-rev `CalcDiscriminant`.
	// - For `is_<variant>` and `into_<variant>` at materialised mode we use
	//   the current-rev discriminant directly.
	let mut current_discs = std::collections::HashMap::new();
	CalcDiscriminant::new(revision, &mut current_discs).visit_enum(e)?;

	// Pre-compute, for each variant existing at the latest rev, the
	// discriminant at every wire revision in `1..=revision`.
	let per_variant: EnumVariantDiscTable<'_> = e
		.variants
		.iter()
		.filter(|v| v.attrs.options.exists_at(revision))
		.map(|v| {
			let curr = *current_discs
				.get(&v.ident)
				.expect("variant exists at current rev should have a discriminant");
			let mut per_rev = Vec::new();
			for r in 1..=revision {
				if v.attrs.options.exists_at(r) {
					let mut discs = std::collections::HashMap::new();
					CalcDiscriminant::new(r, &mut discs).visit_enum(e)?;
					per_rev.push((r as u16, discs.get(&v.ident).copied()));
				} else {
					per_rev.push((r as u16, None));
				}
			}
			Ok::<_, syn::Error>((v, curr, per_rev))
		})
		.collect::<syn::Result<Vec<_>>>()?;

	for (v, current_disc, per_rev) in &per_variant {
		let variant_ident = &v.ident;
		let snake = snake_case(&variant_ident.to_string());
		let into_name = format_ident!("into_{}", snake);
		let is_name = format_ident!("is_{}", snake);
		let decode_name = format_ident!("decode_{}", snake);
		let view_name = format_ident!("{}_view", snake);

		// Build the wire-rev → expected-discriminant arms.
		let wire_disc_arms: Vec<TokenStream> = per_rev
			.iter()
			.map(|(rev, opt_disc)| match opt_disc {
				Some(d) => quote! { #rev => ::std::option::Option::Some(#d), },
				None => quote! { #rev => ::std::option::Option::None, },
			})
			.collect();

		out.append_all(quote! {
			/// Returns `true` if the wire's discriminant identifies this variant
			/// at the wire revision (or, for materialised walkers, on the
			/// re-encoded current-rev bytes).
			#[inline]
			pub fn #is_name(&self) -> bool {
				match &self.repr {
					#walker_repr_name::Wire { wire_rev, discriminant, .. } => {
						let __expected: ::std::option::Option<u32> = match *wire_rev {
							#(#wire_disc_arms)*
							_ => ::std::option::Option::None,
						};
						__expected == ::std::option::Option::Some(*discriminant)
					}
					#walker_repr_name::Materialised { discriminant, .. } => {
						*discriminant == #current_disc
					}
				}
			}
		});

		// `into_<variant>` only emitted for unit variants and single-field
		// tuple variants. Multi-field / struct variants would require
		// generating a sub-walker for the variant's fields struct, which
		// the existing derive emits as a plain struct without
		// `WalkRevisioned`. Skipping these in v1.
		match &v.fields {
			Fields::Unit => {
				out.append_all(quote! {
					/// Verify the wire encoding identifies the unit variant
					/// `#variant_ident` and return `()`.
					#[inline]
					pub fn #into_name(self) -> ::std::result::Result<(), ::revision::Error> {
						if !self.#is_name() {
							return ::std::result::Result::Err(::revision::Error::Deserialize(
								::std::format!(
									"walker variant mismatch: expected `{}` (rev {}), got discriminant {}",
									stringify!(#variant_ident),
									self.revision(),
									self.discriminant(),
								),
							));
						}
						::std::result::Result::Ok(())
					}

					/// Decode the unit variant — same as `into_<variant>` for
					/// unit variants, kept for API symmetry.
					#[inline]
					pub fn #decode_name(self) -> ::std::result::Result<(), ::revision::Error> {
						self.#into_name()
					}
				});
			}
			Fields::Unnamed {
				fields,
				..
			} if fields.iter().filter(|f| f.attrs.options.exists_at(revision)).count() == 1 => {
				// Single-field tuple variant: descend into the inner type's
				// walker.
				let inner_field = fields
					.iter()
					.find(|f| f.attrs.options.exists_at(revision))
					.expect("variant has a field at current rev");
				let inner_ty = &inner_field.ty;
				out.append_all(quote! {
					/// Walk into the payload of the `#variant_ident` variant.
					///
					/// Errors with [`revision::Error::Deserialize`] if the
					/// wire encoding does not identify this variant. Errors
					/// with [`revision::Error::Conversion`] in materialised
					/// mode (older-rev `convert_fn` types); callers should
					/// use [`decode_<variant>`](Self::#decode_name) in that case.
					#[inline]
					pub fn #into_name(
						self,
					) -> ::std::result::Result<<#inner_ty as ::revision::WalkRevisioned>::Walker<'r, R>, ::revision::Error> {
						if !self.#is_name() {
							return ::std::result::Result::Err(::revision::Error::Deserialize(
								::std::format!(
									"walker variant mismatch: expected `{}` (rev {}), got discriminant {}",
									stringify!(#variant_ident),
									self.revision(),
									self.discriminant(),
								),
							));
						}
						match self.repr {
							#walker_repr_name::Wire { reader, .. } => {
								<#inner_ty as ::revision::WalkRevisioned>::walk_revisioned(reader)
							}
							#walker_repr_name::Materialised { .. } => {
								::std::result::Result::Err(::revision::Error::Conversion(
									"into_<variant> is not supported on materialised walkers (including optimised enums); use `decode_<variant>` instead".into(),
								))
							}
						}
					}

					/// Decode and return the inner value of the `#variant_ident`
					/// variant directly. Unlike `into_<variant>`, this works for
					/// both Wire and Materialised (including optimised) walkers
					/// because it deserialises the inner type by value rather
					/// than handing back a sub-walker.
					#[inline]
					pub fn #decode_name(
						self,
					) -> ::std::result::Result<#inner_ty, ::revision::Error> {
						if !self.#is_name() {
							return ::std::result::Result::Err(::revision::Error::Deserialize(
								::std::format!(
									"walker variant mismatch: expected `{}` (rev {}), got discriminant {}",
									stringify!(#variant_ident),
									self.revision(),
									self.discriminant(),
								),
							));
						}
						match self.repr {
							#walker_repr_name::Wire { reader, .. } => {
								<#inner_ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)
							}
							#walker_repr_name::Materialised { ref bytes, cursor, .. } => {
								let mut __slice: &[u8] = &bytes[cursor..];
								<#inner_ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __slice)
							}
						}
					}

					/// Return the variant payload as an [`OwnedVariantView`].
					///
					/// Works on both Wire and Materialised walkers (including
					/// optimised enums). For Wire mode the variant body is read
					/// into an owned `Vec<u8>`; for Materialised mode the body
					/// bytes are sliced out of the existing buffer. Either way
					/// the caller owns a self-contained view they can call
					/// `T::walk_revisioned` against, retain across function
					/// boundaries, or feed into another walker.
					///
					/// Use [`decode_<variant>`](Self::#decode_name) when you
					/// want the inner type by value directly.
					///
					/// [`OwnedVariantView`]: revision::optimised::indexed::OwnedVariantView
					#[inline]
					pub fn #view_name(
						self,
					) -> ::std::result::Result<
						::revision::optimised::indexed::OwnedVariantView<#inner_ty>,
						::revision::Error,
					> {
						if !self.#is_name() {
							return ::std::result::Result::Err(::revision::Error::Deserialize(
								::std::format!(
									"walker variant mismatch: expected `{}` (rev {}), got discriminant {}",
									stringify!(#variant_ident),
									self.revision(),
									self.discriminant(),
								),
							));
						}
						let __bytes: ::std::vec::Vec<u8> = match self.repr {
							#walker_repr_name::Wire { reader, .. } => {
								// Read the inner value then re-emit its bytes.
								// One alloc + one re-serialise; matches the
								// indexed-field view shape.
								let __v: #inner_ty =
									<#inner_ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?;
								let mut __buf = ::std::vec::Vec::new();
								<#inner_ty as ::revision::SerializeRevisioned>::serialize_revisioned(&__v, &mut __buf)?;
								__buf
							}
							#walker_repr_name::Materialised { bytes, cursor, .. } => {
								bytes[cursor..].to_vec()
							}
						};
						::std::result::Result::Ok(
							::revision::optimised::indexed::OwnedVariantView::new(__bytes),
						)
					}
				});
			}
			_ => {
				// Skip emission for multi-field variants in v1.
			}
		}
	}

	// Suppress an unused-variable warning when the enum has zero variants
	// at the current revision.
	let _ = enum_name;
	Ok(out)
}

/// Convert `CamelCase` to `snake_case`.
fn snake_case(s: &str) -> String {
	let mut out = String::with_capacity(s.len() + 4);
	for (i, ch) in s.chars().enumerate() {
		if ch.is_uppercase() && i > 0 {
			out.push('_');
		}
		out.extend(ch.to_lowercase());
	}
	out
}

// -----------------------------------------------------------------------------
// Per-revision introspection tables
// -----------------------------------------------------------------------------

fn emit_variant_tables(name: &Ident, revision: usize, e: &Enum) -> syn::Result<TokenStream> {
	// For each rev `r in 1..=revision`, build a list of (variant_name, disc).
	let mut per_rev_arms = Vec::new();
	let mut per_rev_table_arms = Vec::new();
	for r in 1..=revision {
		let mut discs = std::collections::HashMap::new();
		CalcDiscriminant::new(r, &mut discs).visit_enum(e)?;
		let mut name_arms = Vec::new();
		let mut entries = Vec::new();
		for v in e.variants.iter().filter(|v| v.attrs.options.exists_at(r)) {
			if let Some(d) = discs.get(&v.ident) {
				let name_str = v.ident.to_string();
				name_arms.push(quote! { #d => ::std::option::Option::Some(#name_str), });
				entries.push(quote! { (#name_str, #d) });
			}
		}
		let r_lit = r as u16;
		per_rev_arms.push(quote! {
			#r_lit => match discriminant {
				#(#name_arms)*
				_ => ::std::option::Option::None,
			},
		});
		per_rev_table_arms.push(quote! {
			#r_lit => &[#(#entries),*],
		});
	}

	Ok(quote! {
		impl #name {
			/// Resolve a wire discriminant at `wire_revision` to the variant
			/// identifier at that revision. Returns `None` if no such variant
			/// exists at that revision.
			///
			/// Generated by the `revisioned` derive.
			#[inline]
			pub fn walk_revisioned_variant_name(
				wire_revision: u16,
				discriminant: u32,
			) -> ::std::option::Option<&'static str> {
				match wire_revision {
					#(#per_rev_arms)*
					_ => ::std::option::Option::None,
				}
			}

			/// Variant name + discriminant pairs at `wire_revision`. Returns
			/// an empty slice if the revision is unknown. Generated by the
			/// `revisioned` derive.
			#[inline]
			pub fn walk_revisioned_variant_table(
				wire_revision: u16,
			) -> &'static [(&'static str, u32)] {
				match wire_revision {
					#(#per_rev_table_arms)*
					_ => &[],
				}
			}
		}
	})
}

fn emit_field_tables(name: &Ident, revision: usize, s: &Struct) -> TokenStream {
	let fields_iter: Vec<&Field> = match &s.fields {
		Fields::Named {
			fields,
			..
		}
		| Fields::Unnamed {
			fields,
			..
		} => fields.iter().collect(),
		Fields::Unit => Vec::new(),
	};

	let mut per_rev_arms = Vec::new();
	for r in 1..=revision {
		let entries: Vec<TokenStream> = fields_iter
			.iter()
			.filter(|f| f.attrs.options.exists_at(r))
			.map(|f| {
				let lit = match &f.name {
					FieldName::Ident(i) => i.to_string(),
					FieldName::Index(i) => i.index.to_string(),
				};
				quote! { #lit }
			})
			.collect();
		let r_lit = r as u16;
		per_rev_arms.push(quote! {
			#r_lit => &[#(#entries),*],
		});
	}

	quote! {
		impl #name {
			/// Field names at the given wire revision in declaration order.
			/// Returns an empty slice if the revision is unknown. Generated
			/// by the `revisioned` derive.
			#[inline]
			pub fn walk_revisioned_field_names(
				wire_revision: u16,
			) -> &'static [&'static str] {
				match wire_revision {
					#(#per_rev_arms)*
					_ => &[],
				}
			}
		}
	}
}

fn alive_field_count(s: &Struct, revision: usize) -> usize {
	match &s.fields {
		Fields::Named {
			fields,
			..
		}
		| Fields::Unnamed {
			fields,
			..
		} => fields.iter().filter(|f| f.attrs.options.exists_at(revision)).count(),
		Fields::Unit => 0,
	}
}

/// Emit the walker-construction arm for an optimised enum revision.
///
/// Reads the 1-byte tag, slurps the variant payload per the declared size
/// class, and returns a `Materialised` walker whose `discriminant` field holds
/// the variant id from the tag. The existing per-variant `decode_<v>` /
/// `is_<v>` / `into_<v>` methods on the `Materialised` arm then read fields
/// directly from the slurped payload.
fn emit_optimised_enum_walker_arm(
	e: &Enum,
	entry: &HistoryEntry,
	rev_lit: u16,
	walker_name: &Ident,
	walker_repr_name: &Ident,
) -> syn::Result<TokenStream> {
	// Compute the variant_id -> declared SizeClass map for this entry.
	let mut discriminants = HashMap::new();
	CalcDiscriminant::new(entry.revision.value, &mut discriminants).visit_enum(e)?;
	let mut sizes: HashMap<u32, VariantSize> = HashMap::new();
	for v in e.variants.iter().filter(|v| v.attrs.options.exists_at(entry.revision.value)) {
		let id = *discriminants.get(&v.ident).expect("alive variant has discriminant");
		let Some(spanned_size) = v.attrs.options.size.as_ref() else {
			return Err(syn::Error::new(
				v.ident.span(),
				"variant requires `#[revision(size = \"inline\" | \"fixed(N)\" | \"varlen\")]` under `encoding = \"optimised\"`",
			));
		};
		sizes.insert(id, spanned_size.size);
	}

	// Build the (variant_id, size_class) → slurp body match arms.
	let mut arms = TokenStream::new();
	for (id, size) in sizes.iter() {
		let id_lit = *id as u8;
		let arm = match size {
			VariantSize::Inline => quote! {
				(#id_lit, ::revision::optimised::tag::SizeClass::Inline) => {
					::std::vec::Vec::new()
				}
			},
			VariantSize::Fixed(n) => {
				let n_lit = *n as usize;
				quote! {
					(#id_lit, ::revision::optimised::tag::SizeClass::Fixed) => {
						let mut __buf = ::std::vec![0u8; #n_lit];
						::std::io::Read::read_exact(reader, &mut __buf)
							.map_err(::revision::Error::Io)?;
						__buf
					}
				}
			}
			VariantSize::Varlen => quote! {
				(#id_lit, ::revision::optimised::tag::SizeClass::Varlen) => {
					let mut __len_buf = [0u8; 4];
					::std::io::Read::read_exact(reader, &mut __len_buf)
						.map_err(::revision::Error::Io)?;
					let __len = u32::from_le_bytes(__len_buf) as usize;
					let mut __buf = ::std::vec![0u8; __len];
					::std::io::Read::read_exact(reader, &mut __buf)
						.map_err(::revision::Error::Io)?;
					__buf
				}
			},
		};
		arms.append_all(arm);
	}

	let bad_arm_msg = format!(
		"unknown variant tag for optimised enum at revision {rev_lit}: variant_id={{}} size_class={{:?}}",
	);

	Ok(quote! {
		#rev_lit => {
			let __tag = ::revision::optimised::tag::read_tag(reader)?;
			let __sc = __tag.size_class()?;
			let __variant_id = __tag.variant_id();
			let __payload: ::std::vec::Vec<u8> = match (__variant_id, __sc) {
				#arms
				_ => {
					return ::std::result::Result::Err(::revision::Error::Deserialize(
						::std::format!(#bad_arm_msg, __variant_id, __sc),
					));
				}
			};
			return ::std::result::Result::Ok(#walker_name {
				repr: #walker_repr_name::Materialised {
					bytes: __payload,
					cursor: 0,
					discriminant: __variant_id as u32,
					pos: 0,
					_marker: ::std::marker::PhantomData,
				},
			});
		}
	})
}
