use std::collections::{HashMap, HashSet};

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Error, Ident, Type};

use crate::ast::{self, Visit};

/// Scans the AST for any `convert_fn` annotation on a field or a variant.
///
/// `convert_fn` participates in cross-revision deserialisation by mutating
/// the under-construction `Self` based on a wire-only field; it cannot be
/// expressed as field-by-field walking. Types that contain such annotations
/// require the walker's materialised fallback path when the wire revision
/// differs from the schema revision.
#[derive(Default)]
pub struct HasConvertFn {
	pub found: bool,
}

impl HasConvertFn {
	/// Returns `true` if any field or variant in `item` has a `convert_fn`
	/// annotation at any revision in its presence range.
	pub fn check(item: &ast::Item) -> syn::Result<bool> {
		let mut visitor = HasConvertFn::default();
		visitor.visit_item(item)?;
		Ok(visitor.found)
	}
}

impl<'ast> Visit<'ast> for HasConvertFn {
	fn visit_variant(&mut self, i: &'ast ast::Variant) -> syn::Result<()> {
		if i.attrs.options.convert.is_some() {
			self.found = true;
			return Ok(());
		}
		ast::visit_variant(self, i)
	}

	fn visit_field(&mut self, i: &'ast ast::Field) -> syn::Result<()> {
		if i.attrs.options.convert.is_some() {
			self.found = true;
		}
		Ok(())
	}
}

/// A pass which calculates discriminats for enum variants.
pub struct CalcDiscriminant<'a> {
	revision: usize,
	discriminants: &'a mut HashMap<Ident, u32>,
	used: HashSet<u32>,
	next: u32,
}

impl<'a> CalcDiscriminant<'a> {
	pub fn new(revision: usize, discriminants: &'a mut HashMap<Ident, u32>) -> Self {
		Self {
			revision,
			discriminants,
			used: HashSet::new(),
			next: 0,
		}
	}
}

impl<'ast> Visit<'ast> for CalcDiscriminant<'_> {
	fn visit_enum(&mut self, i: &'ast crate::ast::Enum) -> syn::Result<()> {
		GatherOverrides {
			revision: self.revision,
			discriminants: self.discriminants,
			used: &mut self.used,
		}
		.visit_enum(i)?;

		ast::visit_enum(self, i)
	}

	fn visit_variant(&mut self, i: &'ast ast::Variant) -> syn::Result<()> {
		if !i.attrs.options.exists_at(self.revision) {
			return Ok(());
		}

		if self.discriminants.contains_key(&i.ident) {
			return Ok(());
		}

		while self.used.contains(&self.next) {
			self.next += 1;
		}

		self.used.insert(self.next);
		self.discriminants.insert(i.ident.clone(), self.next);
		Ok(())
	}
}

pub struct GatherOverrides<'a> {
	revision: usize,
	discriminants: &'a mut HashMap<Ident, u32>,
	used: &'a mut HashSet<u32>,
}

impl<'ast> Visit<'ast> for GatherOverrides<'_> {
	fn visit_variant(&mut self, i: &'ast crate::ast::Variant) -> syn::Result<()> {
		if !i.attrs.options.exists_at(self.revision) {
			return Ok(());
		}

		let Some(x) = i.attrs.options.overrides.get(&self.revision) else {
			return Ok(());
		};

		let Some(ref descr) = x.discriminant else {
			return Ok(());
		};

		if !self.used.insert(descr.value) {
			return Err(Error::new(descr.span, "discriminant used twice for different variants"));
		}

		self.discriminants.insert(i.ident.clone(), descr.value);
		Ok(())
	}
}

// -----------------------------------------------------------------------------
// Per-field encoding-override dispatch helpers
// -----------------------------------------------------------------------------

/// Names of primitive integer types that `#[revision(fixed)]` supports.
///
/// The set matches the integers whose `SerializeRevisioned`/`DeserializeRevisioned`
/// dispatch on the `fixed-width-encoding` cargo feature — these are the
/// types where varint vs fixed actually differ on the wire.
const FIXED_SUPPORTED_INTS: &[&str] = &["u32", "i32", "u64", "i64", "u128", "i128"];

/// If `ty` is one of the FIXED_SUPPORTED_INTS spelled as a bare name,
/// return its name; else `None`.
///
/// The match is purely syntactic — it does **not** see through type aliases
/// (`type MyId = u32;` ... `x: MyId` is rejected) or qualified paths
/// (`::std::primitive::u32`, `core::primitive::u32` are rejected). The
/// macro can't resolve aliases at parse time; rejecting them here forces
/// the caller to spell the bare name so the wire-format encoding the field
/// gets is visible at the declaration site rather than buried in a
/// distant `type` alias.
pub fn fixed_int_name(ty: &Type) -> Option<&'static str> {
	let s = ty.to_token_stream().to_string();
	// `to_token_stream` for a bare primitive type produces just its name
	// (e.g. `u32`); paths produce `:: std :: primitive :: u32` with
	// whitespace around `::`. Compare to the canonical bare-name list to
	// reject both aliases and paths.
	let trimmed = s.trim();
	FIXED_SUPPORTED_INTS.iter().find(|name| **name == trimmed).copied()
}

/// Build the rejection error for a field whose type isn't one of the bare
/// supported integer names. The message calls out both the "wrong type"
/// case (e.g. `String`) and the "right type, wrong spelling" case (paths,
/// aliases) because the macro can't disambiguate them syntactically.
fn fixed_attr_error(ty: &Type) -> syn::Error {
	Error::new_spanned(
		ty,
		"`#[revision(fixed)]` requires the field type to be one of `u32`, `i32`, \
		 `u64`, `i64`, `u128`, `i128` spelled as the bare primitive name. \
		 Qualified paths (`::std::primitive::u32`, `core::primitive::u32`) and \
		 type aliases (`type MyId = u32; field: MyId`) are not seen through by \
		 the macro and so cannot carry this attribute — spell the bare name on \
		 the field or remove the attribute.",
	)
}

/// Emit `serialize_<int>_fixed_le(*value, writer)` for a `#[revision(fixed)]`
/// field. Returns an error if `ty` is not a supported primitive integer.
pub fn emit_serialize_fixed_le(
	ty: &Type,
	value_expr: &TokenStream,
	writer_expr: &TokenStream,
) -> syn::Result<TokenStream> {
	let kind = fixed_int_name(ty).ok_or_else(|| fixed_attr_error(ty))?;
	let fn_name = format_ident!("encode_{}_fixed_le", kind);
	Ok(quote! {
		::revision::implementations::primitives::#fn_name(*#value_expr, #writer_expr)?;
	})
}

/// Emit `decode_<int>_fixed_le(reader)` for a `#[revision(fixed)]` field.
pub fn emit_deserialize_fixed_le(ty: &Type, reader_expr: &TokenStream) -> syn::Result<TokenStream> {
	let kind = fixed_int_name(ty).ok_or_else(|| fixed_attr_error(ty))?;
	let fn_name = format_ident!("decode_{}_fixed_le", kind);
	Ok(quote! {
		::revision::implementations::primitives::#fn_name(#reader_expr)?
	})
}

/// Emit `skip_<int>_fixed_le(reader)` for a `#[revision(fixed)]` field.
pub fn emit_skip_fixed_le(ty: &Type, reader_expr: &TokenStream) -> syn::Result<TokenStream> {
	let kind = fixed_int_name(ty).ok_or_else(|| fixed_attr_error(ty))?;
	let fn_name = format_ident!("skip_{}_fixed_le", kind);
	Ok(quote! {
		::revision::implementations::primitives::#fn_name(#reader_expr)?;
	})
}

/// Emit a bulk-encoded `Vec<T>` serialize call for a `#[revision(specialised)]`
/// field. The `SerializeRevisionedSpecialised` trait is only implemented for
/// `Vec<primitive>`, so any other type fires a trait-bound error at compile
/// time pointing at the field.
pub fn emit_serialize_specialised(
	ty: &Type,
	value_expr: &TokenStream,
	writer_expr: &TokenStream,
) -> TokenStream {
	quote! {
		<#ty as ::revision::implementations::specialised::SerializeRevisionedSpecialised>::serialize_revisioned_specialised(
			#value_expr,
			#writer_expr,
		)?;
	}
}

/// Emit a bulk-encoded `Vec<T>` deserialize call for a
/// `#[revision(specialised)]` field. Same trait-bound contract as
/// [`emit_serialize_specialised`].
pub fn emit_deserialize_specialised(ty: &Type, reader_expr: &TokenStream) -> TokenStream {
	quote! {
		<#ty as ::revision::implementations::specialised::DeserializeRevisionedSpecialised>::deserialize_revisioned_specialised(
			#reader_expr,
		)?
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use syn::parse_str;

	fn matches(input: &str) -> Option<&'static str> {
		let ty: Type = parse_str(input).expect("test input parses as a Type");
		fixed_int_name(&ty)
	}

	#[test]
	fn bare_primitives_match() {
		assert_eq!(matches("u32"), Some("u32"));
		assert_eq!(matches("i32"), Some("i32"));
		assert_eq!(matches("u64"), Some("u64"));
		assert_eq!(matches("i64"), Some("i64"));
		assert_eq!(matches("u128"), Some("u128"));
		assert_eq!(matches("i128"), Some("i128"));
	}

	#[test]
	fn qualified_paths_are_rejected() {
		// The macro can't see through paths — these must NOT silently
		// match and fall through to the default varint encoding.
		assert_eq!(matches("::std::primitive::u32"), None);
		assert_eq!(matches("std::primitive::u32"), None);
		assert_eq!(matches("core::primitive::u32"), None);
		assert_eq!(matches("::core::primitive::i64"), None);
	}

	#[test]
	fn aliases_and_unsupported_types_are_rejected() {
		// Aliases — the macro can't resolve them.
		assert_eq!(matches("MyId"), None);
		assert_eq!(matches("Self::Id"), None);
		// Types not in the supported set.
		assert_eq!(matches("u8"), None);
		assert_eq!(matches("u16"), None);
		assert_eq!(matches("usize"), None);
		assert_eq!(matches("isize"), None);
		assert_eq!(matches("f32"), None);
		assert_eq!(matches("f64"), None);
		assert_eq!(matches("String"), None);
		assert_eq!(matches("Vec<u32>"), None);
		// Wrapping / reference forms.
		assert_eq!(matches("&u32"), None);
		assert_eq!(matches("(u32)"), None);
		assert_eq!(matches("Wrapping<u32>"), None);
	}
}
