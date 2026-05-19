use std::collections::HashMap;

use crate::ast::history::HistoryEntry;
use crate::ast::{self, Visit};

use super::common::CalcDiscriminant;

pub struct ValidateRevision(pub usize);
impl<'ast> Visit<'ast> for ValidateRevision {
	fn visit_field(&mut self, i: &'ast ast::Field) -> syn::Result<()> {
		if let Some(s) = i.attrs.options.start.as_ref()
			&& s.value > self.0
		{
			return Err(syn::Error::new(s.span, "used revision exceededs current revision"));
		}
		if let Some(s) = i.attrs.options.end.as_ref()
			&& s.value > self.0
		{
			return Err(syn::Error::new(s.span, "used revision exceededs current revision"));
		}
		Ok(())
	}

	fn visit_variant(&mut self, i: &'ast ast::Variant) -> syn::Result<()> {
		if let Some(s) = i.attrs.options.start.as_ref()
			&& s.value > self.0
		{
			return Err(syn::Error::new(s.span, "used revision exceededs current revision"));
		}
		if let Some(s) = i.attrs.options.end.as_ref()
			&& s.value > self.0
		{
			return Err(syn::Error::new(s.span, "used revision exceededs current revision"));
		}
		ast::visit_variant(self, i)
	}
}

/// Validate optimised-encoding-specific invariants up front so the codegen
/// modules can assume they hold:
///
/// - Every variant alive at an optimised revision declares a `size = "..."`
///   class.
/// - At most 32 alive variants per optimised revision (5-bit variant id).
/// - Variant discriminants under optimised fit in 5 bits (max 31).
///
/// Runs once per item, against every optimised entry in the history. Errors
/// here surface at the macro invocation site with a precise span.
pub struct ValidateOptimised<'a>(pub &'a [HistoryEntry]);

impl<'a> ValidateOptimised<'a> {
	pub fn check(&self, item: &ast::Item) -> syn::Result<()> {
		for entry in self.0.iter().filter(|e| e.is_optimised()) {
			self.check_entry(item, entry)?;
		}
		Ok(())
	}

	fn check_entry(&self, item: &ast::Item, entry: &HistoryEntry) -> syn::Result<()> {
		let ast::ItemKind::Enum(e) = &item.kind else {
			return Ok(());
		};
		let rev = entry.revision.value;
		let alive: Vec<&ast::Variant> =
			e.variants.iter().filter(|v| v.attrs.options.exists_at(rev)).collect();

		if alive.len() > 32 {
			return Err(syn::Error::new(
				e.name.span(),
				format!(
					"enum has {} variants alive at revision {} but `encoding = \"optimised\"` allows at most 32",
					alive.len(),
					rev,
				),
			));
		}

		for v in &alive {
			if v.attrs.options.size.is_none() {
				return Err(syn::Error::new(
					v.ident.span(),
					"variant requires `#[revision(size = \"inline\" | \"fixed(N)\" | \"varlen\")]` under `encoding = \"optimised\"`",
				));
			}
		}

		let mut discs = HashMap::new();
		CalcDiscriminant::new(rev, &mut discs).visit_enum(e)?;
		for (name, d) in &discs {
			if *d >= 32 {
				return Err(syn::Error::new(
					name.span(),
					format!(
						"variant `{name}` has discriminant {d} which exceeds the 5-bit limit (max 31) under `encoding = \"optimised\"`",
					),
				));
			}
		}

		Ok(())
	}
}
