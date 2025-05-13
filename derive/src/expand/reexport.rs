use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use syn::{punctuated::Pair, token};

use crate::ast::{self, Fields, Visit};

/// Visitor which reexports the item, recreating it with only the given fields.
pub struct Reexport<'a> {
	pub revision: usize,
	pub stream: &'a mut TokenStream,
}
impl<'ast> Visit<'ast> for Reexport<'_> {
	fn visit_item(&mut self, i: &'ast ast::Item) -> syn::Result<()> {
		for attr in i.attrs.other.iter() {
			attr.to_tokens(self.stream)
		}
		i.vis.to_tokens(self.stream);
		ast::visit_item(self, i)
	}

	fn visit_item_kind(&mut self, i: &'ast ast::ItemKind) -> syn::Result<()> {
		ast::visit_item_kind(self, i)
	}

	fn visit_enum(&mut self, i: &'ast ast::Enum) -> syn::Result<()> {
		i.enum_.to_tokens(self.stream);
		i.name.to_tokens(self.stream);
		i.generics.to_tokens(self.stream);
		i.braces.surround(self.stream, |stream| {
			let mut this = Reexport {
				revision: self.revision,
				stream,
			};
			for pairs in i.variants.pairs() {
				match pairs {
					Pair::Punctuated(v, p) => {
						if v.attrs.options.exists_at(self.revision) {
							this.visit_variant(v).unwrap();
							p.to_tokens(this.stream);
						}
					}
					Pair::End(v) => {
						if v.attrs.options.exists_at(self.revision) {
							this.visit_variant(v).unwrap();
						}
					}
				}
			}
		});
		Ok(())
	}

	fn visit_struct(&mut self, i: &'ast ast::Struct) -> syn::Result<()> {
		i.struct_.to_tokens(self.stream);
		i.name.to_tokens(self.stream);
		i.generics.to_tokens(self.stream);
		ast::visit_struct(self, i)?;
		if matches!(i.fields, Fields::Unnamed { .. } | Fields::Unit) {
			token::Semi(Span::call_site()).to_tokens(self.stream);
		}

		Ok(())
	}

	fn visit_variant(&mut self, i: &'ast ast::Variant) -> syn::Result<()> {
		if !i.attrs.options.exists_at(self.revision) {
			return Ok(());
		}

		i.attrs.other.iter().for_each(|x| x.to_tokens(self.stream));
		i.ident.to_tokens(self.stream);
		ast::visit_variant(self, i)?;

		if let Some((eq, expr)) = i.discriminant.as_ref() {
			eq.to_tokens(self.stream);
			expr.to_tokens(self.stream);
		}

		Ok(())
	}

	fn visit_fields(&mut self, i: &'ast ast::Fields) -> syn::Result<()> {
		match i {
			ast::Fields::Named {
				brace,
				fields,
			} => {
				brace.surround(self.stream, |stream| {
					let mut this = Reexport {
						revision: self.revision,
						stream,
					};
					for pair in fields.pairs() {
						match pair {
							Pair::Punctuated(f, c) => {
								if f.attrs.options.exists_at(self.revision) {
									this.visit_field(f).unwrap();
									c.to_tokens(this.stream)
								}
							}
							Pair::End(f) => {
								if f.attrs.options.exists_at(self.revision) {
									this.visit_field(f).unwrap();
								}
							}
						}
					}
				});
				Ok(())
			}
			ast::Fields::Unnamed {
				paren,
				fields,
			} => {
				paren.surround(self.stream, |stream| {
					let mut this = Reexport {
						revision: self.revision,
						stream,
					};
					for pair in fields.pairs() {
						match pair {
							Pair::Punctuated(f, c) => {
								if f.attrs.options.exists_at(self.revision) {
									this.visit_field(f).unwrap();
									c.to_tokens(this.stream)
								}
							}
							Pair::End(f) => {
								if f.attrs.options.exists_at(self.revision) {
									this.visit_field(f).unwrap();
								}
							}
						}
					}
				});
				Ok(())
			}
			ast::Fields::Unit => Ok(()),
		}
	}

	fn visit_field(&mut self, i: &'ast ast::Field) -> syn::Result<()> {
		i.attrs.other.iter().for_each(|x| x.to_tokens(self.stream));
		i.vis.to_tokens(self.stream);
		match i.name {
			ast::FieldName::Ident(ref x) => x.to_tokens(self.stream),
			ast::FieldName::Index(_) => {}
		}
		if let Some(x) = i.colon_token {
			x.to_tokens(self.stream);
		}
		i.ty.to_tokens(self.stream);
		Ok(())
	}
}
