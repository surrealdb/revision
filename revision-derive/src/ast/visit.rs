use super::{Enum, Field, Fields, Item, ItemKind, Struct, Variant};

pub trait Visit<'ast>: Sized {
	fn visit_item(&mut self, i: &'ast Item) -> syn::Result<()> {
		visit_item(self, i)
	}

	fn visit_item_kind(&mut self, i: &'ast ItemKind) -> syn::Result<()> {
		visit_item_kind(self, i)
	}

	fn visit_enum(&mut self, i: &'ast Enum) -> syn::Result<()> {
		visit_enum(self, i)
	}

	fn visit_struct(&mut self, i: &'ast Struct) -> syn::Result<()> {
		visit_struct(self, i)
	}

	fn visit_variant(&mut self, i: &'ast Variant) -> syn::Result<()> {
		visit_variant(self, i)
	}

	fn visit_fields(&mut self, i: &'ast Fields) -> syn::Result<()> {
		visit_fields(self, i)
	}

	fn visit_field(&mut self, i: &'ast Field) -> syn::Result<()> {
		let _ = i;
		Ok(())
	}
}

pub fn visit_item<'ast, T>(this: &mut T, item: &'ast Item) -> syn::Result<()>
where
	T: Visit<'ast>,
{
	this.visit_item_kind(&item.kind)
}

pub fn visit_item_kind<'ast, T>(this: &mut T, kind: &'ast ItemKind) -> syn::Result<()>
where
	T: Visit<'ast>,
{
	match kind {
		ItemKind::Enum(e) => this.visit_enum(e)?,
		ItemKind::Struct(s) => this.visit_struct(s)?,
	}
	Ok(())
}

pub fn visit_enum<'ast, T>(this: &mut T, e: &'ast Enum) -> syn::Result<()>
where
	T: Visit<'ast>,
{
	for variant in e.variants.iter() {
		this.visit_variant(variant)?
	}
	Ok(())
}

pub fn visit_struct<'ast, T>(this: &mut T, s: &'ast Struct) -> syn::Result<()>
where
	T: Visit<'ast>,
{
	this.visit_fields(&s.fields)
}

pub fn visit_variant<'ast, T>(this: &mut T, s: &'ast Variant) -> syn::Result<()>
where
	T: Visit<'ast>,
{
	this.visit_fields(&s.fields)
}

pub fn visit_fields<'ast, T>(this: &mut T, f: &'ast Fields) -> syn::Result<()>
where
	T: Visit<'ast>,
{
	match f {
		Fields::Named {
			fields,
			..
		}
		| Fields::Unnamed {
			fields,
			..
		} => {
			for f in fields {
				this.visit_field(f)?
			}
			Ok(())
		}
		Fields::Unit => Ok(()),
	}
}
