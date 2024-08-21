use crate::ast::{self, Visit};

pub struct ValidateRevision(pub usize);
impl<'ast> Visit<'ast> for ValidateRevision {
	fn visit_field(&mut self, i: &'ast ast::Field) -> syn::Result<()> {
		if let Some(s) = i.attrs.options.start.as_ref() {
			if s.value > self.0 {
				return Err(syn::Error::new(s.span, "used revision exceededs current revision"));
			}
		}
		if let Some(s) = i.attrs.options.end.as_ref() {
			if s.value > self.0 {
				return Err(syn::Error::new(s.span, "used revision exceededs current revision"));
			}
		}
		Ok(())
	}

	fn visit_variant(&mut self, i: &'ast ast::Variant) -> syn::Result<()> {
		if let Some(s) = i.attrs.options.start.as_ref() {
			if s.value > self.0 {
				return Err(syn::Error::new(s.span, "used revision exceededs current revision"));
			}
		}
		if let Some(s) = i.attrs.options.end.as_ref() {
			if s.value > self.0 {
				return Err(syn::Error::new(s.span, "used revision exceededs current revision"));
			}
		}
		ast::visit_variant(self, i)
	}
}
