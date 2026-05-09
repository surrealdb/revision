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
	has_convert_fn: bool,
	serialize_enabled: bool,
	deserialize_enabled: bool,
) -> syn::Result<TokenStream> {
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
			type Walker<'r, R: ::std::io::Read + 'r> = #walker_name<'r, R>;

			fn walk_revisioned<'r, R: ::std::io::Read>(
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
				#post_header_read
				::std::result::Result::Ok(#walker_name { repr: #wire_constructor })
			}
		}

		impl<'r, R: ::std::io::Read + 'r> #walker_name<'r, R> {
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
		pub struct #walker_name<'r, R: ::std::io::Read + 'r> {
			#[doc(hidden)]
			pub repr: #walker_repr_name<'r, R>,
		}

		#[doc(hidden)]
		pub enum #walker_repr_name<'r, R: ::std::io::Read + 'r> {
			Wire {
				reader: &'r mut R,
				wire_rev: u16,
				pos: u32,
			},
			Materialised {
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
		pub struct #walker_name<'r, R: ::std::io::Read + 'r> {
			#[doc(hidden)]
			pub repr: #walker_repr_name<'r, R>,
		}

		#[doc(hidden)]
		pub enum #walker_repr_name<'r, R: ::std::io::Read + 'r> {
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
		let decode_wire_body = if always_present {
			quote! {
				let __v = <#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?;
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
					<#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?
				} else {
					#default_expr
				};
			}
		};

		let skip_wire_body = if always_present {
			quote! {
				<#ty as ::revision::SkipRevisioned>::skip_revisioned(reader)?;
			}
		} else {
			quote! {
				if *wire_rev >= #start_val {
					<#ty as ::revision::SkipRevisioned>::skip_revisioned(reader)?;
				}
			}
		};

		let walk_wire_body = if always_present {
			quote! {
				<#ty as ::revision::WalkRevisioned>::walk_revisioned(reader)
			}
		} else {
			let walk_err_msg = format!(
				"walk_{} not available at wire revision {{}}: field added at revision {}",
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

		out.append_all(quote! {
			/// Decode this field, advancing the walker.
			#[inline]
			pub fn #decode_name(&mut self) -> ::std::result::Result<#ty, ::revision::Error> {
				match &mut self.repr {
					#walker_repr_name::Wire { reader, wire_rev, pos } => {
						#decode_wire_body
						*pos = #pos_lit + 1;
						::std::result::Result::Ok(__v)
					}
					#walker_repr_name::Materialised { bytes, cursor, pos, .. } => {
						let mut __slice: &[u8] = &bytes[*cursor..];
						let __v = <#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __slice)?;
						*cursor = bytes.len() - __slice.len();
						*pos = #pos_lit + 1;
						::std::result::Result::Ok(__v)
					}
				}
			}

			/// Skip this field, advancing the walker.
			#[inline]
			pub fn #skip_name(&mut self) -> ::std::result::Result<(), ::revision::Error> {
				match &mut self.repr {
					#walker_repr_name::Wire { reader, wire_rev, pos } => {
						#skip_wire_body
						*pos = #pos_lit + 1;
						::std::result::Result::Ok(())
					}
					#walker_repr_name::Materialised { bytes, cursor, pos, .. } => {
						let mut __slice: &[u8] = &bytes[*cursor..];
						<#ty as ::revision::SkipRevisioned>::skip_revisioned(&mut __slice)?;
						*cursor = bytes.len() - __slice.len();
						*pos = #pos_lit + 1;
						::std::result::Result::Ok(())
					}
				}
			}

			/// Walk into this field, **consuming** the parent walker and
			/// returning a sub-walker that owns the underlying reader for
			/// the original `'r` lifetime. The parent walker cannot be used
			/// further; subsequent fields cannot be visited.
			///
			/// Only supported in wire mode; materialised walkers (older-rev
			/// `convert_fn` types) return [`revision::Error::Conversion`].
			/// Callers needing to walk inside a materialised value should
			/// `decode_*` it instead and use the resulting Rust value
			/// directly.
			#[inline]
			pub fn #walk_name(
				self,
			) -> ::std::result::Result<<#ty as ::revision::WalkRevisioned>::Walker<'r, R>, ::revision::Error> {
				match self.repr {
					#walker_repr_name::Wire { reader, wire_rev, pos: _ } => {
						#walk_wire_body
					}
					#walker_repr_name::Materialised { .. } => {
						::std::result::Result::Err(::revision::Error::Conversion(
							"walk_<field> is not supported on materialised walkers; use decode_<field>".into(),
						))
					}
				}
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
					/// `decode` from a fresh walker in that case.
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
									"into_<variant> is not supported on materialised walkers".into(),
								))
							}
						}
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
