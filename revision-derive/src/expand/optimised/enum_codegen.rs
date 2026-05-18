//! Optimised codegen for revisioned enums.
//!
//! Wire layout under `encoding = "optimised"`:
//!
//! ```text
//! u16 revision                     (written by the outer impl)
//! u8 tag                            (variant_id in bits 0..=4, size class in 5..=6)
//! [payload per size class]
//! ```
//!
//! The macro emits one branch per (variant_id, size_class) pair. Each variant
//! must declare its size class via `#[revision(size = "...")]`. Validation of
//! that requirement and the `variant_id < 32` constraint lives in the
//! `ValidateOptimised` pass.

use std::collections::HashMap;

use proc_macro2::{Span, TokenStream};
use quote::{TokenStreamExt, quote};
use syn::{Error, Ident};

use crate::ast::attributes::{VariantOptions, VariantSize};
use crate::ast::{Enum, Field, Fields, Variant, Visit};

use super::super::common::CalcDiscriminant;
use super::super::context::EncodingContext;

/// Resolve the size class declared by `#[revision(size = "...")]` on a variant.
/// Returns a typed error if missing or out-of-range under `optimised`.
fn variant_size(v: &Variant) -> syn::Result<&VariantSize> {
	let Some(s) = v.attrs.options.size.as_ref() else {
		return Err(Error::new(
			v.ident.span(),
			"variant requires `#[revision(size = \"inline\" | \"fixed(N)\" | \"varlen\")]` under `encoding = \"optimised\"`",
		));
	};
	Ok(&s.size)
}

fn size_class_path(size: &VariantSize) -> TokenStream {
	match size {
		VariantSize::Inline => {
			quote! { ::revision::optimised::tag::SizeClass::Inline }
		}
		VariantSize::Fixed(_) => {
			quote! { ::revision::optimised::tag::SizeClass::Fixed }
		}
		VariantSize::Varlen => {
			quote! { ::revision::optimised::tag::SizeClass::Varlen }
		}
	}
}

fn validate_discriminants(discriminants: &HashMap<Ident, u32>) -> syn::Result<()> {
	for (name, d) in discriminants {
		if *d >= 32 {
			return Err(Error::new(
				name.span(),
				format!(
					"variant `{name}` has discriminant {d} which exceeds the 5-bit limit (max 31) under `encoding = \"optimised\"`",
				),
			));
		}
	}
	Ok(())
}

fn alive_variants(e: &Enum, revision: usize) -> Vec<&Variant> {
	e.variants.iter().filter(|v| v.attrs.options.exists_at(revision)).collect()
}

/// Emit the optimised serialize body for an enum.
pub fn emit_enum_serialize(e: &Enum, ctx: EncodingContext) -> syn::Result<TokenStream> {
	let revision = ctx.revision as usize;
	let mut discriminants = HashMap::new();
	CalcDiscriminant::new(revision, &mut discriminants).visit_enum(e)?;
	validate_discriminants(&discriminants)?;
	if alive_variants(e, revision).len() > 32 {
		return Err(Error::new(
			e.name.span(),
			"enum has more than 32 variants alive at this revision; `encoding = \"optimised\"` allows at most 32",
		));
	}

	let mut arms = TokenStream::new();
	for v in alive_variants(e, revision) {
		let name = &v.ident;
		let id = *discriminants.get(name).expect("alive variant has discriminant");
		let size = variant_size(v)?;
		let sc_path = size_class_path(size);
		let id_lit = id as u8;
		let alive_field_count = match &v.fields {
			Fields::Named {
				fields,
				..
			}
			| Fields::Unnamed {
				fields,
				..
			} => fields.iter().filter(|f| f.attrs.options.exists_at(revision)).count(),
			Fields::Unit => 0,
		};

		let payload = match (size, &v.fields) {
			(VariantSize::Inline, Fields::Unit) => quote! {},
			(VariantSize::Inline, _) if alive_field_count == 0 => quote! {},
			(VariantSize::Inline, _) => {
				return Err(Error::new(
					name.span(),
					"variant marked `size = \"inline\"` must have no fields alive at this revision",
				));
			}
			(VariantSize::Fixed(expected), _) => {
				let mut body = TokenStream::new();
				body.append_all(quote! {
					let mut __scratch: ::std::vec::Vec<u8> = ::std::vec::Vec::new();
				});
				for f in alive_fields(v, revision) {
					let binding = f.name.to_binding();
					body.append_all(quote! {
						::revision::SerializeRevisioned::serialize_revisioned(#binding, &mut __scratch)?;
					});
				}
				let expected_lit = *expected as usize;
				body.append_all(quote! {
					debug_assert_eq!(
						__scratch.len(),
						#expected_lit,
						"optimised fixed-size variant produced {} bytes; declared `size = \"fixed({})\"` requires exactly that many",
						__scratch.len(),
						#expected_lit,
					);
					::std::io::Write::write_all(writer, &__scratch)
						.map_err(::revision::Error::Io)?;
				});
				body
			}
			(VariantSize::Varlen, _) => {
				let mut body = TokenStream::new();
				body.append_all(quote! {
					let mut __scratch: ::std::vec::Vec<u8> = ::std::vec::Vec::new();
				});
				for f in alive_fields(v, revision) {
					let binding = f.name.to_binding();
					body.append_all(quote! {
						::revision::SerializeRevisioned::serialize_revisioned(#binding, &mut __scratch)?;
					});
				}
				body.append_all(quote! {
					let __len: u32 = __scratch.len().try_into().map_err(|_| {
						::revision::Error::Serialize(
							"optimised varlen variant payload exceeds u32::MAX bytes".into()
						)
					})?;
					::std::io::Write::write_all(writer, &__len.to_le_bytes())
						.map_err(::revision::Error::Io)?;
					::std::io::Write::write_all(writer, &__scratch)
						.map_err(::revision::Error::Io)?;
				});
				body
			}
		};

		let pattern = variant_pattern(name, v, revision);
		arms.append_all(quote! {
			#pattern => {
				let __tag = ::revision::optimised::tag::Tag::new(#id_lit, #sc_path);
				::revision::optimised::tag::write_tag(writer, __tag)?;
				#payload
				Ok(())
			}
		});
	}

	Ok(quote! {
		match *self {
			#arms
		}
	})
}

/// Emit the optimised deserialize body for an enum.
pub fn emit_enum_deserialize(
	e: &Enum,
	ctx: EncodingContext,
	target: usize,
) -> syn::Result<TokenStream> {
	let current = ctx.revision as usize;
	let mut discriminants = HashMap::new();
	CalcDiscriminant::new(current, &mut discriminants).visit_enum(e)?;
	validate_discriminants(&discriminants)?;

	let mut arms = TokenStream::new();
	for v in alive_variants(e, current) {
		let name = &v.ident;
		let id = *discriminants.get(name).expect("alive variant has discriminant");
		let size = variant_size(v)?;
		let id_lit = id as u8;
		let exists_at_target = v.attrs.options.exists_at(target);
		let body = decode_variant_body(name, v, size, current, target, exists_at_target, &e.name)?;
		let sc_match = match size {
			VariantSize::Inline => quote! { ::revision::optimised::tag::SizeClass::Inline },
			VariantSize::Fixed(_) => quote! { ::revision::optimised::tag::SizeClass::Fixed },
			VariantSize::Varlen => quote! { ::revision::optimised::tag::SizeClass::Varlen },
		};
		arms.append_all(quote! {
			(#id_lit, #sc_match) => {
				#body
			}
		});
	}

	let error_string = format!("Invalid tag for enum `{}` revision `{{}}`", e.name);
	let rev_lit = current as u16;

	Ok(quote! {
		let __tag = ::revision::optimised::tag::read_tag(reader)?;
		let __sc = __tag.size_class()?;
		match (__tag.variant_id(), __sc) {
			#arms
			_ => {
				return Err(::revision::Error::Deserialize(
					format!(#error_string, #rev_lit)
				));
			}
		}
	})
}

/// Emit the optimised skip body for an enum.
///
/// For Inline variants nothing to advance; for Fixed we need the static size
/// keyed by variant id; for Varlen we read the u32_le length and advance.
pub fn emit_enum_skip(
	e: &Enum,
	ctx: EncodingContext,
	slice_mode: bool,
) -> syn::Result<TokenStream> {
	let revision = ctx.revision as usize;
	let mut discriminants = HashMap::new();
	CalcDiscriminant::new(revision, &mut discriminants).visit_enum(e)?;
	validate_discriminants(&discriminants)?;

	// Build a (variant_id -> static_size) table for Fixed variants. Inline and
	// Varlen variants don't need an entry; the size_class tells us what to do.
	let mut fixed_table: Vec<(u32, u8)> = Vec::new();
	for v in alive_variants(e, revision) {
		let id = *discriminants.get(&v.ident).expect("alive variant");
		if let VariantSize::Fixed(n) = variant_size(v)? {
			fixed_table.push((id, *n));
		}
	}
	// `id` < 32, so a 32-entry array is plenty.
	let mut size_arr: [u8; 32] = [0u8; 32];
	for (id, n) in &fixed_table {
		size_arr[*id as usize] = *n;
	}
	let size_arr_lits: Vec<u8> = size_arr.to_vec();

	let advance_fixed = if slice_mode {
		quote! { reader.consume(__size as usize)?; }
	} else {
		quote! { ::revision::slice_reader::advance_read(reader, __size as usize)?; }
	};

	let advance_varlen = if slice_mode {
		quote! { reader.consume(__len as usize)?; }
	} else {
		quote! { ::revision::slice_reader::advance_read(reader, __len as usize)?; }
	};

	Ok(quote! {
		static __SIZE_TABLE: [u8; 32] = [#(#size_arr_lits),*];
		let __tag = ::revision::optimised::tag::read_tag(reader)?;
		let __sc = __tag.size_class()?;
		match __sc {
			::revision::optimised::tag::SizeClass::Inline => {
				Ok(())
			}
			::revision::optimised::tag::SizeClass::Fixed => {
				let __size = __SIZE_TABLE[__tag.variant_id() as usize];
				#advance_fixed
				Ok(())
			}
			::revision::optimised::tag::SizeClass::Varlen => {
				let mut __len_buf = [0u8; 4];
				::std::io::Read::read_exact(reader, &mut __len_buf)
					.map_err(::revision::Error::Io)?;
				let __len = u32::from_le_bytes(__len_buf);
				#advance_varlen
				Ok(())
			}
		}
	})
}

fn variant_pattern(name: &Ident, v: &Variant, revision: usize) -> TokenStream {
	match &v.fields {
		Fields::Named {
			fields,
			..
		} => {
			let bindings =
				fields.iter().filter(|f| f.attrs.options.exists_at(revision)).map(|f| &f.name);
			quote! { Self::#name { #(ref #bindings),* } }
		}
		Fields::Unnamed {
			fields,
			..
		} => {
			let bindings = fields
				.iter()
				.filter(|f| f.attrs.options.exists_at(revision))
				.map(|f| f.name.to_binding());
			quote! { Self::#name ( #(ref #bindings),* ) }
		}
		Fields::Unit => quote! { Self::#name },
	}
}

fn decode_variant_body(
	name: &Ident,
	v: &Variant,
	size: &VariantSize,
	current: usize,
	target: usize,
	exists_at_target: bool,
	enum_name: &Ident,
) -> syn::Result<TokenStream> {
	let _ = target; // explicitly unused once we've decided which path to emit
	let body_reader = match size {
		VariantSize::Inline => quote! { let mut __body: &[u8] = &[]; let _ = &mut __body; },
		VariantSize::Fixed(n) => {
			let n_lit = *n as usize;
			quote! {
				let mut __body_buf = ::std::vec![0u8; #n_lit];
				::std::io::Read::read_exact(reader, &mut __body_buf)
					.map_err(::revision::Error::Io)?;
				let mut __body: &[u8] = &__body_buf;
			}
		}
		VariantSize::Varlen => quote! {
			let mut __len_buf = [0u8; 4];
			::std::io::Read::read_exact(reader, &mut __len_buf)
				.map_err(::revision::Error::Io)?;
			let __len = u32::from_le_bytes(__len_buf) as usize;
			let mut __body_buf = ::std::vec![0u8; __len];
			::std::io::Read::read_exact(reader, &mut __body_buf)
				.map_err(::revision::Error::Io)?;
			let mut __body: &[u8] = &__body_buf;
		},
	};

	// Decode each alive field from __body.
	let mut decode_fields = TokenStream::new();
	let alive: Vec<&Field> = alive_fields(v, current);
	for f in &alive {
		let binding = f.name.to_binding();
		let ty = &f.ty;
		decode_fields.append_all(quote! {
			let #binding = <#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __body)?;
		});
	}

	if !exists_at_target {
		// Variant present on wire but removed at target — convert_fn required.
		// Decode fields into the auto-generated `<Enum><Variant>Fields` struct
		// (emitted by `EnumStructsVisitor` for the current revision), then
		// hand it to the user-supplied convert_fn which produces a `Self`.
		let Some(convert) = v.attrs.options.convert.as_ref() else {
			return Err(Error::new(
				name.span(),
				"removing a variant across revisions requires `#[revision(end = ..., convert_fn = \"...\")]`",
			));
		};
		let convert_ident = syn::Ident::new(&convert.value(), convert.span());
		let fields_struct_ident = v.fields_name(&enum_name.to_string());
		let wire_rev_lit = current as u16;

		let construction = match &v.fields {
			Fields::Named {
				fields,
				..
			} => {
				let names = fields
					.iter()
					.filter(|f| f.attrs.options.exists_at(current))
					.map(|f| f.name.to_binding());
				quote! { #fields_struct_ident { #(#names),* } }
			}
			Fields::Unnamed {
				fields,
				..
			} => {
				let bindings = fields
					.iter()
					.filter(|f| f.attrs.options.exists_at(current))
					.map(|f| f.name.to_binding());
				quote! { #fields_struct_ident ( #(#bindings),* ) }
			}
			Fields::Unit => quote! { #fields_struct_ident },
		};

		return Ok(quote! {
			#body_reader
			#decode_fields
			let __removed = #construction;
			return Self::#convert_ident(__removed, #wire_rev_lit);
		});
	}

	let construction = match &v.fields {
		Fields::Named {
			fields,
			..
		} => {
			let bindings = fields
				.iter()
				.filter(|f| f.attrs.options.exists_at(current))
				.map(|f| f.name.to_binding());
			quote! { Self::#name { #(#bindings),* } }
		}
		Fields::Unnamed {
			fields,
			..
		} => {
			let bindings = fields
				.iter()
				.filter(|f| f.attrs.options.exists_at(current))
				.map(|f| f.name.to_binding());
			quote! { Self::#name ( #(#bindings),* ) }
		}
		Fields::Unit => quote! { Self::#name },
	};

	Ok(quote! {
		#body_reader
		#decode_fields
		Ok(#construction)
	})
}

fn alive_fields(v: &Variant, revision: usize) -> Vec<&Field> {
	match &v.fields {
		Fields::Named {
			fields,
			..
		}
		| Fields::Unnamed {
			fields,
			..
		} => fields.iter().filter(|f| f.attrs.options.exists_at(revision)).collect(),
		Fields::Unit => Vec::new(),
	}
}

// Silence unused-import warnings; future iterations will use these.
#[allow(dead_code)]
const _: Option<&VariantOptions> = None;
const _SPAN: Option<Span> = None;
