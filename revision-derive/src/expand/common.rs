use std::collections::{HashMap, HashSet};

use syn::{Error, Ident};

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
