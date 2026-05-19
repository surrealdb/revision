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

/// If `ty` is one of the FIXED_SUPPORTED_INTS, return its name; else `None`.
///
/// The match is syntactic (compares the type's token stream against the
/// known names) and so does not see through type aliases. That mirrors the
/// existing constraints elsewhere in the macro.
pub fn fixed_int_name(ty: &Type) -> Option<&'static str> {
	let s = ty.to_token_stream().to_string();
	// `to_token_stream` for a primitive type produces just the bare name.
	let trimmed = s.trim();
	FIXED_SUPPORTED_INTS.iter().find(|name| **name == trimmed).copied()
}

/// Emit `serialize_<int>_fixed_le(*value, writer)` for a `#[revision(fixed)]`
/// field. Returns an error if `ty` is not a supported primitive integer.
pub fn emit_serialize_fixed_le(
	ty: &Type,
	value_expr: &TokenStream,
	writer_expr: &TokenStream,
) -> syn::Result<TokenStream> {
	let kind = fixed_int_name(ty).ok_or_else(|| {
		Error::new_spanned(
			ty,
			"`#[revision(fixed)]` is only valid on `u32`, `i32`, `u64`, `i64`, `u128`, `i128` fields",
		)
	})?;
	let fn_name = format_ident!("encode_{}_fixed_le", kind);
	Ok(quote! {
		::revision::implementations::primitives::#fn_name(*#value_expr, #writer_expr)?;
	})
}

/// Emit `decode_<int>_fixed_le(reader)` for a `#[revision(fixed)]` field.
pub fn emit_deserialize_fixed_le(
	ty: &Type,
	reader_expr: &TokenStream,
) -> syn::Result<TokenStream> {
	let kind = fixed_int_name(ty).ok_or_else(|| {
		Error::new_spanned(
			ty,
			"`#[revision(fixed)]` is only valid on `u32`, `i32`, `u64`, `i64`, `u128`, `i128` fields",
		)
	})?;
	let fn_name = format_ident!("decode_{}_fixed_le", kind);
	Ok(quote! {
		::revision::implementations::primitives::#fn_name(#reader_expr)?
	})
}

/// Emit `skip_<int>_fixed_le(reader)` for a `#[revision(fixed)]` field.
pub fn emit_skip_fixed_le(ty: &Type, reader_expr: &TokenStream) -> syn::Result<TokenStream> {
	let kind = fixed_int_name(ty).ok_or_else(|| {
		Error::new_spanned(
			ty,
			"`#[revision(fixed)]` is only valid on `u32`, `i32`, `u64`, `i64`, `u128`, `i128` fields",
		)
	})?;
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
