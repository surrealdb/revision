use proc_macro2::TokenStream;
use quote::{quote, TokenStreamExt};

use crate::ast::{Enum, Field, Fields, Struct, Variant, Visit};

use super::extend_from_filter;

pub struct SerializeVisitor<'a> {
	pub revision: usize,
	pub stream: &'a mut TokenStream,
}

impl<'a, 'ast> Visit<'ast> for SerializeVisitor<'a> {
	fn visit_struct(&mut self, i: &'ast Struct) -> syn::Result<()> {
		let mut ser_fields = TokenStream::new();
		SerializeFields {
			revision: self.revision,
			stream: &mut ser_fields,
		}
		.visit_struct(i)
		.unwrap();

		match i.fields {
			Fields::Named {
				ref fields,
				..
			} => {
				extend_from_filter(fields, self.stream, |field| {
					if !field.attrs.options.exists_at(self.revision) {
						return None;
					}
					let name = &field.name;
					Some(quote! { let #name = &self.#name })
				});

				self.stream.append_all(ser_fields);
				self.stream.append_all(quote! { Ok(()) });
				Ok(())
			}
			Fields::Unnamed {
				ref fields,
				..
			} => {
				extend_from_filter(fields, self.stream, |field| {
					if !field.attrs.options.exists_at(self.revision) {
						return None;
					}
					let name = &field.name;
					let binding = name.to_binding();
					Some(quote! { let #binding = &self.#name })
				});
				self.stream.append_all(ser_fields);
				self.stream.append_all(quote! { Ok(()) });
				Ok(())
			}
			Fields::Unit => Ok(()),
		}
	}

	fn visit_enum(&mut self, i: &'ast Enum) -> syn::Result<()> {
		let mut ser_variants = TokenStream::new();
		SerializeVariant {
			revision: self.revision,
			discriminant: 0,
			stream: &mut ser_variants,
		}
		.visit_enum(i)
		.unwrap();

		self.stream.append_all(quote! {
			match *self{
				#ser_variants
			}
		});

		Ok(())
	}

	fn visit_field(&mut self, i: &'ast Field) -> syn::Result<()> {
		let name = &i.name;

		self.stream.append_all(quote! {
			::revision::Revisioned::serialize_revisioned(#name,writer)?;
		});

		Ok(())
	}
}

pub struct SerializeFields<'a> {
	pub revision: usize,
	pub stream: &'a mut TokenStream,
}

impl<'a, 'ast> Visit<'ast> for SerializeFields<'a> {
	fn visit_field(&mut self, i: &'ast Field) -> syn::Result<()> {
		if !i.attrs.options.exists_at(self.revision) {
			return Ok(());
		}

		let name = i.name.to_binding();
		self.stream.append_all(quote! {
			::revision::Revisioned::serialize_revisioned(#name,writer)?;
		});

		Ok(())
	}
}

pub struct SerializeVariant<'a> {
	pub revision: usize,
	pub discriminant: u32,
	pub stream: &'a mut TokenStream,
}

impl<'a, 'ast> Visit<'ast> for SerializeVariant<'a> {
	fn visit_variant(&mut self, i: &'ast Variant) -> syn::Result<()> {
		if !i.attrs.options.exists_at(self.revision) {
			return Ok(());
		}

		let name = &i.ident;

		self.stream.append_all(quote! {Self::#name});

		let discr = self.discriminant;
		self.discriminant += 1;

		match i.fields {
			Fields::Named {
				ref fields,
				..
			} => {
				let bindings = fields
					.iter()
					.filter(|x| x.attrs.options.exists_at(self.revision))
					.map(|x| &x.name);
				self.stream.append_all(quote! {
					{ #(ref #bindings),* }
				});

				let mut fields_ser = TokenStream::new();

				SerializeFields {
					revision: self.revision,
					stream: &mut fields_ser,
				}
				.visit_variant(i)
				.unwrap();

				self.stream.append_all(quote! {
					=> {
						::revision::Revisioned::serialize_revisioned(&#discr,writer)?;
						#fields_ser
						Ok(())
					},
				})
			}
			Fields::Unnamed {
				ref fields,
				..
			} => {
				let bindings = fields
					.iter()
					.filter(|x| x.attrs.options.exists_at(self.revision))
					.map(|x| x.name.to_binding());
				self.stream.append_all(quote! {
					( #(ref #bindings),* )
				});

				let mut fields_ser = TokenStream::new();

				SerializeFields {
					revision: self.revision,
					stream: &mut fields_ser,
				}
				.visit_variant(i)
				.unwrap();

				self.stream.append_all(quote! {
					=> {
						::revision::Revisioned::serialize_revisioned(&#discr,writer)?;
						#fields_ser
						Ok(())
					}
				})
			}
			Fields::Unit => {
				self.stream.append_all(quote! { => {
					::revision::Revisioned::serialize_revisioned(&#discr,writer)?;
					Ok(())
				}});
			}
		}

		Ok(())
	}
}
