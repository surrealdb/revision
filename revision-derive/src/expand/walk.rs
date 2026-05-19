//! Derive emitter for [`WalkRevisioned`].
//!
//! For every `#[revisioned(...)]` type whose `walk` derive flag is enabled
//! (defaults to the `deserialize` flag) the emitter generates:
//!
//! - A per-type walker struct `<Type>Walker<'r, R>` whose internal
//!   representation is one of:
//!   - **Wire**: holds `&'r mut R`, the wire revision, and a position counter.
//!     Used for the current-rev fast path and for older revs of types that do
//!     not use `convert_fn` (purely additive evolution). Optimised
//!     sequential structs also land here after advancing past their u32_le
//!     length prefix.
//!   - **IndexedBorrowed** (struct walker only): holds `&'r [u8]` pointing
//!     at an optimised + `struct = "indexed"` payload borrowed from the
//!     parent reader. Per-field methods jump via the offset table in O(1).
//!   - **OptimisedBorrowed** (enum walker only): holds `&'r [u8]` pointing
//!     at an optimised enum's variant body, borrowed from the parent
//!     reader. The discriminant is cached alongside.
//!   - **ConvertedOwned**: holds an owned `Vec<u8>` of bytes serialised at
//!     the current schema revision, plus a cursor. Used for older revs of
//!     types that use `convert_fn`; the conversion has already been honoured
//!     by `DeserializeRevisioned` and the bytes are then re-encoded so the
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
//! All repr arms share an identical surface API; only the
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

	// ConvertedOwned path is only emitted when the type actually has at least
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
						repr: #walker_repr_name::ConvertedOwned {
							bytes: __buf,
							cursor: __cursor,
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
						repr: #walker_repr_name::ConvertedOwned {
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
	// - For enums: read the 1-byte tag, borrow the payload per the variant's
	//   declared size class, and return an OptimisedBorrowed walker with
	//   `discriminant = variant_id` (the existing per-variant decode code on
	//   the OptimisedBorrowed arm reads the borrowed payload directly).
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
						// Indexed struct: borrow the payload directly from the
						// reader's buffer (no allocation) and return an
						// IndexedBorrowed walker. Per-field methods jump via
						// offsets in O(1).
						let field_count = optimised_struct_field_count.unwrap_or(0) as usize;
						let field_count_u16 = field_count as u16;
						Ok(quote! {
							#rev_lit => {
								let mut __len_buf = [0u8; 4];
								::std::io::Read::read_exact(reader, &mut __len_buf)
									.map_err(::revision::Error::Io)?;
								let __payload_len = u32::from_le_bytes(__len_buf) as usize;
								// Borrow the payload from the reader's buffer
								// via the canonical safe wrapper around the
								// unsafe lifetime-extension dance.
								let __payload: &'r [u8] =
									::revision::read_borrowed_bytes(reader, __payload_len)?;
								return ::std::result::Result::Ok(#walker_name {
									repr: #walker_repr_name::IndexedBorrowed {
										bytes: __payload,
										field_count: #field_count_u16,
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
					#walker_repr_name::OptimisedBorrowed { .. } => #revision_lit,
					#walker_repr_name::ConvertedOwned { .. } => #revision_lit,
				}
			}

			/// Variant discriminant on the wire (for `Wire`) or on the
			/// re-encoded current-rev bytes (for the borrowed / owned arms).
			/// Use
			/// [`walk_revisioned_variant_name`](Self::walk_revisioned_variant_name)
			/// to map this to a variant identifier.
			#[inline]
			pub fn discriminant(&self) -> u32 {
				match &self.repr {
					#walker_repr_name::Wire { discriminant, .. } => *discriminant,
					#walker_repr_name::OptimisedBorrowed { discriminant, .. } => *discriminant,
					#walker_repr_name::ConvertedOwned { discriminant, .. } => *discriminant,
				}
			}
		},
		ItemKind::Struct(_) => quote! {
			/// Wire revision of the encoded value being walked.
			#[inline]
			pub fn revision(&self) -> u16 {
				match &self.repr {
					#walker_repr_name::Wire { wire_rev, .. } => *wire_rev,
					#walker_repr_name::IndexedBorrowed { .. } => #revision_lit,
					#walker_repr_name::ConvertedOwned { .. } => #revision_lit,
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
			repr: #walker_repr_name<'r, R>,
		}

		/// Internal repr; one variant per source-of-bytes. Made `pub` for
		/// the macro-emitted accessors that match on it; not part of the
		/// public API and may change without notice.
		#[doc(hidden)]
		pub enum #walker_repr_name<'r, R: ::revision::BorrowedReader + 'r> {
			/// Streaming over a live reader. Used at the current revision
			/// for purely additive types and for any optimised
			/// sequential-struct revision (the new() arm advances past the
			/// `u32_le payload_length` and hands the reader through).
			Wire {
				reader: &'r mut R,
				wire_rev: u16,
				pos: u32,
			},
			/// Optimised + `struct = "indexed"` payload borrowed directly
			/// from the parent reader's buffer (no allocation). The offset
			/// table sits at the start of `bytes`; offsets are read on
			/// demand from `bytes[i*4..i*4+4]` for O(1) random access.
			IndexedBorrowed {
				bytes: &'r [u8],
				field_count: u16,
				pos: u32,
				_marker: ::std::marker::PhantomData<&'r mut R>,
			},
			/// Cross-revision `convert_fn` round-trip: bytes are the
			/// re-encoded current-rev value, owned because they don't
			/// exist anywhere else. The walker reads fields sequentially
			/// through `cursor`.
			ConvertedOwned {
				bytes: ::std::vec::Vec<u8>,
				cursor: usize,
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
			repr: #walker_repr_name<'r, R>,
		}

		/// Internal repr; one variant per source-of-bytes. Made `pub` for
		/// the macro-emitted accessors that match on it; not part of the
		/// public API and may change without notice.
		#[doc(hidden)]
		pub enum #walker_repr_name<'r, R: ::revision::BorrowedReader + 'r> {
			/// Streaming over a live reader.
			Wire {
				reader: &'r mut R,
				wire_rev: u16,
				/// Variant discriminant on the wire.
				discriminant: u32,
				pos: u32,
			},
			/// Optimised enum body borrowed directly from the parent
			/// reader's buffer. `bytes` covers the variant payload only
			/// (everything after the 1-byte tag, and for varlen variants
			/// after the `u32_le` length prefix).
			OptimisedBorrowed {
				bytes: &'r [u8],
				discriminant: u32,
				pos: u32,
				_marker: ::std::marker::PhantomData<&'r mut R>,
			},
			/// Cross-revision `convert_fn` round-trip; owned bytes the
			/// walker reads through `cursor`.
			ConvertedOwned {
				bytes: ::std::vec::Vec<u8>,
				cursor: usize,
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
		// borrow from a payload slice, so we return a wire-bytes wrapper
		// (`IndexedMapView` / `IndexedSeqView`) holding `Cow<'r, [u8]>`.
		//
		// Three paths, in order of preference (most-borrowed → most-allocating):
		//
		// 1. `IndexedBorrowed` parent (optimised + `struct = "indexed"`): the
		//    field's canonical bytes already live at `parent.bytes[offsets[i]
		//    ..offsets[i+1]]`. Extract directly via `Cow::Borrowed` with the
		//    parent's `'r` lifetime. Zero allocation, O(1) lookup.
		// 2. `Wire` parent (sequential optimised or current-rev legacy): the
		//    field's bytes are next in the reader, but their length isn't
		//    known up front. Capture `reader.remaining()` before and after a
		//    `skip_indexed_*` call to derive the field's exact slice, then
		//    extend the borrow to `'r` via the same unsafe pattern used by
		//    `read_borrowed_bytes`. Zero allocation, O(field bytes) skip.
		// 3. `ConvertedOwned` parent (cross-rev `convert_fn` round-trip):
		//    the parent's bytes are an owned `Vec<u8>` that dies with `self`
		//    on a `&self`/`self` accessor, so we can't borrow from it. Decode
		//    the field into the runtime type and re-encode into `Cow::Owned`.
		//    One allocation per call; rare path.
		// `self_expr` is the identifier used for the walker instance — `self`
		// for `walk_<field>(&mut self)` and `__self` for
		// `into_walk_<field>(self)` after the `let mut __self = self;` rebind.
		let fast_path_indexed_borrowed =
			|view_ty: &TokenStream, self_expr: &TokenStream| -> TokenStream {
				quote! {
					if let #walker_repr_name::IndexedBorrowed { bytes, field_count, .. } = &#self_expr.repr {
						let __bytes_borrow: &'r [u8] = *bytes;
						let __fc = *field_count as usize;
						let __off_base = (#pos_lit as usize) * 4;
						let __start = u32::from_le_bytes(
							__bytes_borrow[__off_base..__off_base + 4]
								.try_into()
								.expect("4-byte slice"),
						) as usize;
						let __end = if (#pos_lit as usize) + 1 < __fc {
							u32::from_le_bytes(
								__bytes_borrow[__off_base + 4..__off_base + 8]
									.try_into()
									.expect("4-byte slice"),
							) as usize
						} else {
							__bytes_borrow.len()
						};
						return ::std::result::Result::Ok(
							#view_ty::new(::std::borrow::Cow::Borrowed(&__bytes_borrow[__start..__end])),
						);
					}
				}
			};
		// Wire-parent fast path: skip the field, derive its bytes from the
		// before/after `remaining()` snapshots, lifetime-extend via the
		// canonical unsafe pattern (peeked bytes stay valid for `'r` per the
		// `BorrowedReader` contract).
		let fast_path_wire = |view_ty: &TokenStream,
		                      skip_call: &TokenStream,
		                      self_expr: &TokenStream|
		 -> TokenStream {
			quote! {
				if let #walker_repr_name::Wire { reader, .. } = &mut #self_expr.repr {
					let __before = ::revision::BorrowedReader::remaining(*reader);
					let __before_ptr = __before.as_ptr();
					let __before_len = __before.len();
					#skip_call
					let __after_len = ::revision::BorrowedReader::remaining(*reader).len();
					// `BorrowedReader`'s safety contract bullet (4) requires
					// `remaining().len()` to be monotonic non-increasing
					// under `advance`. A downstream impl that violates this
					// would underflow naive subtraction and trigger UB in
					// the `from_raw_parts` below — `checked_sub` turns the
					// contract violation into a clean deserialize error
					// instead.
					let __consumed_len = __before_len
						.checked_sub(__after_len)
						.ok_or_else(|| ::revision::Error::Deserialize(
							::std::format!(
								"BorrowedReader::remaining() grew across an advance call \
								 (before={}, after={}); the impl violates the trait's safety \
								 contract — this is a bug in the reader implementation, not the \
								 wire data",
								__before_len,
								__after_len,
							)
						))?;
					// SAFETY: `BorrowedReader::remaining` returns a slice
					// into the reader's stable buffer; the trait's safety
					// contract guarantees `advance`/`skip` only moves the
					// cursor and never invalidates previously-returned
					// bytes (bullets 2 and 3), so
					// `__before_ptr[..__consumed_len]` is valid for the
					// reader's lifetime `'r`. The `checked_sub` above also
					// ensures `__consumed_len <= __before_len`, so the
					// slice can never extend past the bytes the trait
					// promised were stable. Same pattern as
					// `read_borrowed_bytes` for `peek_bytes + advance`.
					let __field_bytes: &'r [u8] = unsafe {
						::std::slice::from_raw_parts(__before_ptr, __consumed_len)
					};
					return ::std::result::Result::Ok(
						#view_ty::new(::std::borrow::Cow::Borrowed(__field_bytes)),
					);
				}
			}
		};
		let self_walk = quote! { self };
		let self_into = quote! { __self };
		let walk_return_ty;
		let walk_body;
		let into_walk_return_ty;
		let into_walk_body;
		if f.attrs.options.indexed_map {
			walk_return_ty = quote! {
				::revision::optimised::indexed::IndexedMapView<
					'r,
					<#ty as ::revision::optimised::indexed::IndexedMapEncoded>::Key,
					<#ty as ::revision::optimised::indexed::IndexedMapEncoded>::Value,
				>
			};
			let view_ctor = quote! { ::revision::optimised::indexed::IndexedMapView };
			let skip_call = quote! {
				<#ty as ::revision::optimised::indexed::IndexedMapEncoded>::skip_indexed_map(*reader)?;
			};
			let fast_ib_w = fast_path_indexed_borrowed(&view_ctor, &self_walk);
			let fast_w_w = fast_path_wire(&view_ctor, &skip_call, &self_walk);
			let fast_ib_i = fast_path_indexed_borrowed(&view_ctor, &self_into);
			let fast_w_i = fast_path_wire(&view_ctor, &skip_call, &self_into);
			walk_body = quote! {
				#fast_ib_w
				#fast_w_w
				let __v: #ty = self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedMapEncoded>::serialize_indexed_map(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::IndexedMapView::new(
					::std::borrow::Cow::Owned(__bytes),
				),
				)
			};
			into_walk_return_ty = walk_return_ty.clone();
			into_walk_body = quote! {
				let mut __self = self;
				#fast_ib_i
				#fast_w_i
				let __v: #ty = __self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedMapEncoded>::serialize_indexed_map(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::IndexedMapView::new(
					::std::borrow::Cow::Owned(__bytes),
				),
				)
			};
		} else if f.attrs.options.indexed_seq {
			walk_return_ty = quote! {
				::revision::optimised::indexed::IndexedSeqView<
					'r,
					<#ty as ::revision::optimised::indexed::IndexedSeqEncoded>::Item,
				>
			};
			let view_ctor = quote! { ::revision::optimised::indexed::IndexedSeqView };
			let skip_call = quote! {
				<#ty as ::revision::optimised::indexed::IndexedSeqEncoded>::skip_indexed_seq(*reader)?;
			};
			let fast_ib_w = fast_path_indexed_borrowed(&view_ctor, &self_walk);
			let fast_w_w = fast_path_wire(&view_ctor, &skip_call, &self_walk);
			let fast_ib_i = fast_path_indexed_borrowed(&view_ctor, &self_into);
			let fast_w_i = fast_path_wire(&view_ctor, &skip_call, &self_into);
			walk_body = quote! {
				#fast_ib_w
				#fast_w_w
				let __v: #ty = self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedSeqEncoded>::serialize_indexed_seq(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::IndexedSeqView::new(
					::std::borrow::Cow::Owned(__bytes),
				),
				)
			};
			into_walk_return_ty = walk_return_ty.clone();
			into_walk_body = quote! {
				let mut __self = self;
				#fast_ib_i
				#fast_w_i
				let __v: #ty = __self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedSeqEncoded>::serialize_indexed_seq(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::IndexedSeqView::new(
					::std::borrow::Cow::Owned(__bytes),
				),
				)
			};
		} else if f.attrs.options.indexed_set {
			walk_return_ty = quote! {
				::revision::optimised::indexed::IndexedSetView<
					'r,
					<#ty as ::revision::optimised::indexed::IndexedSetEncoded>::Item,
				>
			};
			let view_ctor = quote! { ::revision::optimised::indexed::IndexedSetView };
			let skip_call = quote! {
				<#ty as ::revision::optimised::indexed::IndexedSetEncoded>::skip_indexed_set(*reader)?;
			};
			let fast_ib_w = fast_path_indexed_borrowed(&view_ctor, &self_walk);
			let fast_w_w = fast_path_wire(&view_ctor, &skip_call, &self_walk);
			let fast_ib_i = fast_path_indexed_borrowed(&view_ctor, &self_into);
			let fast_w_i = fast_path_wire(&view_ctor, &skip_call, &self_into);
			walk_body = quote! {
				#fast_ib_w
				#fast_w_w
				let __v: #ty = self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedSetEncoded>::serialize_indexed_set(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::IndexedSetView::new(
					::std::borrow::Cow::Owned(__bytes),
				),
				)
			};
			into_walk_return_ty = walk_return_ty.clone();
			into_walk_body = quote! {
				let mut __self = self;
				#fast_ib_i
				#fast_w_i
				let __v: #ty = __self.#decode_name()?;
				let mut __bytes = ::std::vec::Vec::new();
				<#ty as ::revision::optimised::indexed::IndexedSetEncoded>::serialize_indexed_set(
					&__v, &mut __bytes,
				)?;
				::std::result::Result::Ok(
					::revision::optimised::indexed::IndexedSetView::new(
					::std::borrow::Cow::Owned(__bytes),
				),
				)
			};
		} else {
			walk_return_ty = quote! { <#ty as ::revision::WalkRevisioned>::Walker<'_, R> };
			walk_body = quote! {
				match &mut self.repr {
					#walker_repr_name::Wire { reader, wire_rev, pos } => {
						#walk_wire_body_borrow
					}
					#walker_repr_name::IndexedBorrowed { .. } => {
						::std::result::Result::Err(::revision::Error::Conversion(
							"walk_<field> is not supported on borrowed-bytes walkers; use decode_<field>".into(),
						))
					}
					#walker_repr_name::ConvertedOwned { .. } => {
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
					#walker_repr_name::IndexedBorrowed { .. } => {
						::std::result::Result::Err(::revision::Error::Conversion(
							"into_walk_<field> is not supported on borrowed-bytes walkers; use decode_<field>".into(),
						))
					}
					#walker_repr_name::ConvertedOwned { .. } => {
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
			/// For `IndexedBorrowed` walkers (optimised + `struct = "indexed"`),
			/// the offset table is consulted to jump directly to this field's
			/// bytes in O(1) — reading any field is independent of how many
			/// fields precede it on the wire.
			#[inline]
			pub fn #decode_name(&mut self) -> ::std::result::Result<#ty, ::revision::Error> {
				match &mut self.repr {
					#walker_repr_name::Wire { reader, wire_rev, pos } => {
						#decode_wire_body
						*pos = #pos_lit + 1;
						::std::result::Result::Ok(__v)
					}
					#walker_repr_name::IndexedBorrowed { bytes, field_count, pos, .. } => {
						// Indexed struct: parse this field's offset from the
						// table at the start of `bytes`. O(1) — two u32 reads,
						// no Vec<u32> alloc.
						let __off_base = (#pos_lit as usize) * 4;
						let __start = u32::from_le_bytes(
							bytes[__off_base..__off_base + 4]
								.try_into()
								.expect("4-byte slice"),
						) as usize;
						let __end = if (#pos_lit as usize) + 1 < (*field_count as usize) {
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
					}
					#walker_repr_name::ConvertedOwned { bytes, cursor, pos, .. } => {
						// Sequential materialised path (convert_fn round-trip).
						let mut __slice: &[u8] = &bytes[*cursor..];
						let __v = <#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __slice)?;
						*cursor = bytes.len() - __slice.len();
						*pos = #pos_lit + 1;
						::std::result::Result::Ok(__v)
					}
				}
			}

			/// Skip this field, advancing the walker.
			///
			/// Free under `IndexedBorrowed` mode (the offset table already
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
					#walker_repr_name::IndexedBorrowed { pos, .. } => {
						// Indexed: skipping is free (any field reachable in O(1)).
						*pos = #pos_lit + 1;
						::std::result::Result::Ok(())
					}
					#walker_repr_name::ConvertedOwned { bytes, cursor, pos, .. } => {
						let mut __slice: &[u8] = &bytes[*cursor..];
						<#ty as ::revision::SkipRevisioned>::skip_revisioned(&mut __slice)?;
						*cursor = bytes.len() - __slice.len();
						*pos = #pos_lit + 1;
						::std::result::Result::Ok(())
					}
				}
			}

			/// Walk into this field. For fields tagged
			/// `#[revision(indexed_map)]` / `#[revision(indexed_seq)]`,
			/// returns an [`IndexedMapView`] / [`IndexedSeqView`]
			/// the caller can borrow an [`IndexedMapWalker`] /
			/// [`IndexedSeqWalker`] from. For all other fields, returns the
			/// inner type's `WalkRevisioned::Walker` as usual (borrowing
			/// the parent walker for the duration of the sub-walk).
			///
			/// `ConvertedOwned` walkers (older-rev `convert_fn` types) return
			/// [`revision::Error::Conversion`] for the legacy path; the
			/// indexed branches re-serialise from the owned value.
			///
			/// [`IndexedMapView`]: revision::optimised::indexed::IndexedMapView
			/// [`IndexedSeqView`]: revision::optimised::indexed::IndexedSeqView
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
			/// at the wire revision (or, for borrowed / owned walkers, on the
			/// current-rev bytes).
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
					#walker_repr_name::OptimisedBorrowed { discriminant, .. } => {
						*discriminant == #current_disc
					}
					#walker_repr_name::ConvertedOwned { discriminant, .. } => {
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
							#walker_repr_name::OptimisedBorrowed { .. } => {
								::std::result::Result::Err(::revision::Error::Conversion(
									"into_<variant> is not supported on borrowed-bytes walkers (optimised enums); use `decode_<variant>` or `<variant>_view` instead".into(),
								))
							}
							#walker_repr_name::ConvertedOwned { .. } => {
								::std::result::Result::Err(::revision::Error::Conversion(
									"into_<variant> is not supported on owned-bytes walkers (cross-rev convert_fn); use `decode_<variant>` instead".into(),
								))
							}
						}
					}

					/// Decode and return the inner value of the `#variant_ident`
					/// variant directly. Unlike `into_<variant>`, this works on
					/// every repr (Wire, OptimisedBorrowed, ConvertedOwned)
					/// because it deserialises by value rather than handing back
					/// a sub-walker.
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
							#walker_repr_name::OptimisedBorrowed { bytes, .. } => {
								let mut __slice: &[u8] = bytes;
								<#inner_ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __slice)
							}
							#walker_repr_name::ConvertedOwned { ref bytes, cursor, .. } => {
								let mut __slice: &[u8] = &bytes[cursor..];
								<#inner_ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __slice)
							}
						}
					}

					/// Return the variant payload as a [`VariantView`].
					///
					/// Works on every repr. `OptimisedBorrowed` (the common
					/// optimised-enum case) hands back a borrowed slice with
					/// the source's `'r` lifetime — no allocation. `Wire`
					/// walkers re-encode (one allocation); `ConvertedOwned`
					/// (cross-revision `convert_fn`) walkers move the owned
					/// bytes.
					///
					/// Use [`decode_<variant>`](Self::#decode_name) when you
					/// want the inner type by value directly.
					///
					/// [`VariantView`]: revision::optimised::indexed::VariantView
					#[inline]
					pub fn #view_name(
						self,
					) -> ::std::result::Result<
						::revision::optimised::indexed::VariantView<'r, #inner_ty>,
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
						let __bytes: ::std::borrow::Cow<'r, [u8]> = match self.repr {
							#walker_repr_name::Wire { reader, .. } => {
								// Read the inner value then re-emit its bytes.
								// One alloc + one re-serialise; matches the
								// indexed-field view shape.
								let __v: #inner_ty =
									<#inner_ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?;
								let mut __buf = ::std::vec::Vec::new();
								<#inner_ty as ::revision::SerializeRevisioned>::serialize_revisioned(&__v, &mut __buf)?;
								::std::borrow::Cow::Owned(__buf)
							}
							#walker_repr_name::OptimisedBorrowed { bytes, .. } => {
								// bytes: &'r [u8] — preserves the source's
								// lifetime so the view's bytes outlive `self`.
								::std::borrow::Cow::Borrowed(bytes)
							}
							#walker_repr_name::ConvertedOwned { bytes, cursor, .. } => {
								// cross-revision convert_fn re-encode — owned.
								let mut v = bytes;
								v.drain(..cursor);
								::std::borrow::Cow::Owned(v)
							}
						};
						::std::result::Result::Ok(
							::revision::optimised::indexed::VariantView::new(__bytes),
						)
					}
				});
			}
			_ => {
				// Multi-field tuple variants and struct variants: emit
				// `<variant>_view` returning a typed-only-by-phantom
				// [`VariantView<'r, ()>`] that hands the caller the variant
				// body bytes. Multi-field variants don't have a single inner
				// type to descend into via `into_<variant>` (their body is a
				// sequence of fields, not a single value), so the caller
				// decodes the bytes themselves.
				out.append_all(quote! {
					/// Return the variant payload bytes as a
					/// [`VariantView`].
					///
					/// Multi-field tuple and struct variants have no single
					/// inner type, so `into_<variant>` is not available;
					/// this `_view` hands you the body bytes and you can
					/// decode the variant's fields sequentially (or feed
					/// the bytes into another walker).
					///
					/// [`VariantView`]: revision::optimised::indexed::VariantView
					#[inline]
					pub fn #view_name(
						self,
					) -> ::std::result::Result<
						::revision::optimised::indexed::VariantView<'r, ()>,
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
						let __bytes: ::std::borrow::Cow<'r, [u8]> = match self.repr {
							#walker_repr_name::Wire { .. } => {
								// Wire mode for multi-field variants on legacy
								// enums: the body is a sequence of fields with
								// no length prefix, so we can't slurp without
								// reading the synthetic Fields struct. Errors
								// here; the optimised wire format gives every
								// variant a length-prefixed body so this works
								// for optimised enums.
								return ::std::result::Result::Err(::revision::Error::Conversion(
									"<variant>_view on a Wire-mode multi-field variant is not supported; encode the type under `encoding = \"optimised\"` to enable variant-body extraction".into(),
								));
							}
							#walker_repr_name::OptimisedBorrowed { bytes, .. } => {
								::std::borrow::Cow::Borrowed(bytes)
							}
							#walker_repr_name::ConvertedOwned { bytes, cursor, .. } => {
								let mut v = bytes;
								v.drain(..cursor);
								::std::borrow::Cow::Owned(v)
							}
						};
						::std::result::Result::Ok(
							::revision::optimised::indexed::VariantView::new(__bytes),
						)
					}
				});
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
/// Reads the 1-byte tag and borrows the variant payload from the reader's
/// buffer per the declared size class, returning an `OptimisedBorrowed`
/// walker. Dispatch is via two static `[u8; 32]` tables (size class code +
/// fixed size) keyed by variant id; the body-read match has three arms
/// regardless of how many variants the enum declares.
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

	// Build two parallel [u8; 32] tables:
	// - size_class_table: 0=Inline, 1=Fixed, 2=Varlen, 0xFF=unknown (no such variant id)
	// - fixed_size_table: byte length for Fixed variants, 0 otherwise
	let mut size_class_table: [u8; 32] = [0xFFu8; 32];
	let mut fixed_size_table: [u8; 32] = [0u8; 32];
	for v in e.variants.iter().filter(|v| v.attrs.options.exists_at(entry.revision.value)) {
		let id = *discriminants.get(&v.ident).expect("alive variant has discriminant");
		let Some(spanned_size) = v.attrs.options.size.as_ref() else {
			return Err(syn::Error::new(
				v.ident.span(),
				"variant requires `#[revision(size = \"inline\" | \"fixed(N)\" | \"varlen\")]` under `encoding = \"optimised\"`",
			));
		};
		let id_idx = id as usize;
		match spanned_size.size {
			VariantSize::Inline => size_class_table[id_idx] = 0,
			VariantSize::Fixed(n) => {
				size_class_table[id_idx] = 1;
				fixed_size_table[id_idx] = n;
			}
			VariantSize::Varlen => size_class_table[id_idx] = 2,
		}
	}

	let sc_lits: Vec<u8> = size_class_table.to_vec();
	let fx_lits: Vec<u8> = fixed_size_table.to_vec();

	let bad_arm_msg = format!(
		"unknown variant tag for optimised enum at revision {rev_lit}: variant_id={{}} size_class={{:?}}",
	);

	Ok(quote! {
		#rev_lit => {
			// Per-enum static tables — laid out once, indexed by variant id.
			// `0xFF` in the size_class table marks variant ids the enum
			// doesn't declare at this revision.
			static __SIZE_CLASS_TABLE: [u8; 32] = [#(#sc_lits),*];
			static __FIXED_SIZE_TABLE: [u8; 32] = [#(#fx_lits),*];

			let __tag = ::revision::optimised::tag::read_tag(reader)?;
			let __sc = __tag.size_class()?;
			let __variant_id = __tag.variant_id();
			let __expected_code = __SIZE_CLASS_TABLE[__variant_id as usize];
			let __actual_code: u8 = match __sc {
				::revision::optimised::tag::SizeClass::Inline => 0,
				::revision::optimised::tag::SizeClass::Fixed => 1,
				::revision::optimised::tag::SizeClass::Varlen => 2,
			};
			if __expected_code == 0xFF || __expected_code != __actual_code {
				return ::std::result::Result::Err(::revision::Error::Deserialize(
					::std::format!(#bad_arm_msg, __variant_id, __sc),
				));
			}
			// 3-arm match regardless of variant count — the static tables
			// have already validated the (id, sc) pair.
			let __payload: &'r [u8] = match __sc {
				::revision::optimised::tag::SizeClass::Inline => &[][..],
				::revision::optimised::tag::SizeClass::Fixed => {
					let __n = __FIXED_SIZE_TABLE[__variant_id as usize] as usize;
					::revision::read_borrowed_bytes(reader, __n)?
				}
				::revision::optimised::tag::SizeClass::Varlen => {
					let mut __len_buf = [0u8; 4];
					::std::io::Read::read_exact(reader, &mut __len_buf)
						.map_err(::revision::Error::Io)?;
					let __len = u32::from_le_bytes(__len_buf) as usize;
					::revision::read_borrowed_bytes(reader, __len)?
				}
			};
			return ::std::result::Result::Ok(#walker_name {
				repr: #walker_repr_name::OptimisedBorrowed {
					bytes: __payload,
					discriminant: __variant_id as u32,
					pos: 0,
					_marker: ::std::marker::PhantomData,
				},
			});
		}
	})
}
