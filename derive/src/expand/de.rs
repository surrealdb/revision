use std::collections::HashMap;

use proc_macro2::{Span, TokenStream};
use quote::{quote, TokenStreamExt};
use syn::{Ident, Index};

use crate::ast::{Enum, Fields, Struct, Variant, Visit};

use super::common::CalcDiscriminant;

/// Visitor which creates structs for fields in a an enum variant.
pub struct EnumStructsVisitor<'a> {
	pub revision: usize,
	pub stream: &'a mut TokenStream,
}

impl<'a> EnumStructsVisitor<'a> {
	pub fn new(revision: usize, stream: &'a mut TokenStream) -> Self {
		Self {
			revision,
			stream,
		}
	}
}

impl<'ast> Visit<'ast> for EnumStructsVisitor<'_> {
	fn visit_enum(&mut self, i: &'ast Enum) -> syn::Result<()> {
		for v in i.variants.iter() {
			let name = v.fields_name(&i.name.to_string());

			let new_struct = match v.fields {
				Fields::Named {
					ref fields,
					..
				} => {
					let fields = fields
						.iter()
						.filter(|x| x.attrs.options.exists_at(self.revision))
						.map(|x| {
							let name = &x.name;
							let ty = &x.ty;
							quote! {
								#name: #ty
							}
						});
					quote! {
						struct #name{ #(#fields),* }
					}
				}
				Fields::Unnamed {
					ref fields,
					..
				} => {
					let fields = fields
						.iter()
						.filter(|x| x.attrs.options.exists_at(self.revision))
						.map(|x| &x.ty);
					quote! {
						struct #name( #(#fields),* );
					}
				}
				Fields::Unit => {
					quote! {
						#[allow(dead_code)]
						struct #name;
					}
				}
			};
			self.stream.append_all(new_struct);
		}
		Ok(())
	}
}

pub struct DeserializeVisitor<'a> {
	pub target: usize,
	pub current: usize,
	pub stream: &'a mut TokenStream,
}

impl<'ast> Visit<'ast> for DeserializeVisitor<'_> {
	fn visit_enum(&mut self, i: &'ast Enum) -> syn::Result<()> {
		let mut discriminants = HashMap::new();
		CalcDiscriminant::new(self.current, &mut discriminants).visit_enum(i)?;

		let mut variants = TokenStream::new();
		DeserializeVariant {
			name: i.name.clone(),
			target: self.target,
			current: self.current,
			stream: &mut variants,
			discriminants,
		}
		.visit_enum(i)
		.unwrap();

		let error_string =
			format!("Invalid discriminant `{{x}}` for enum `{}` revision `{{__revision}}`", i.name);

		self.stream.append_all(quote! {
			let __discriminant = <u32 as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?;
			match __discriminant {
				#variants
				x => {
					return Err(::revision::Error::Deserialize(
						format!(#error_string)
					))
				}
			}
		});
		Ok(())
	}

	fn visit_struct(&mut self, i: &'ast Struct) -> syn::Result<()> {
		let mut fields_binding = TokenStream::new();
		DeserializeFields {
			target: self.target,
			current: self.current,
			stream: &mut fields_binding,
		}
		.visit_struct(i)
		.unwrap();

		match i.fields {
			Fields::Named {
				ref fields,
				..
			} => {
				self.stream.append_all(fields_binding);

				let bindings = fields
					.iter()
					.filter(|x| x.attrs.options.exists_at(self.target))
					.map(|x| x.name.to_binding());
				self.stream.append_all(quote! {
					let mut __this = Self{ #(#bindings),* };
				});
			}
			Fields::Unnamed {
				ref fields,
				..
			} => {
				self.stream.append_all(fields_binding);

				let bindings = fields
					.iter()
					.filter(|x| x.attrs.options.exists_at(self.target))
					.map(|x| x.name.to_binding());
				self.stream.append_all(quote! {
					let mut __this = Self( #(#bindings),* );
				});
			}
			Fields::Unit => {
				self.stream.append_all(quote! {
					Ok(Self)
				});
				return Ok(());
			}
		}

		let (Fields::Named {
			ref fields,
			..
		}
		| Fields::Unnamed {
			ref fields,
			..
		}) = i.fields
		else {
			unreachable!();
		};

		for f in fields.iter().filter(|f| {
			f.attrs.options.exists_at(self.current) && !f.attrs.options.exists_at(self.target)
		}) {
			let binding = f.name.to_binding();
			let convert = f.attrs.options.convert.as_ref().unwrap();
			let convert = Ident::new(&convert.value(), convert.span());
			let revision = self.current as u16;
			self.stream.append_all(quote! {
				Self::#convert(&mut __this,#revision,#binding)?;
			})
		}

		self.stream.append_all(quote! { Ok(__this) });
		Ok(())
	}
}

pub struct DeserializeVariant<'a> {
	pub target: usize,
	pub current: usize,
	pub name: Ident,
	pub stream: &'a mut TokenStream,
	pub discriminants: HashMap<Ident, u32>,
}

impl<'ast> Visit<'ast> for DeserializeVariant<'_> {
	fn visit_variant(&mut self, i: &'ast Variant) -> syn::Result<()> {
		let exists_current = i.attrs.options.exists_at(self.current);
		let exists_target = i.attrs.options.exists_at(self.target);

		if !exists_current {
			return Ok(());
		}

		let mut fields = TokenStream::new();
		DeserializeFields {
			target: self.target,
			current: self.current,
			stream: &mut fields,
		}
		.visit_variant(i)
		.unwrap();

		let fields_struct_name = i.fields_name(&self.name.to_string());

		let (bindings, create) = match i.fields {
			Fields::Named {
				ref fields,
				..
			} => {
				let mut bindings = TokenStream::new();
				let mut create = TokenStream::new();
				let field_names = fields
					.iter()
					.filter(|x| x.attrs.options.exists_at(self.target))
					.map(|x| x.name.to_binding());

				let field_names_c = field_names.clone();
				bindings.append_all(quote! {
					let mut __fields = #fields_struct_name{ #(#field_names_c),* };
				});

				let variant_name = &i.ident;
				create.append_all(quote! {
					Ok(Self::#variant_name{
						#(#field_names: __fields.#field_names,)*
					})
				});

				for f in fields.iter().filter(|x| {
					x.attrs.options.exists_at(self.current)
						&& !x.attrs.options.exists_at(self.target)
				}) {
					let binding = f.name.to_binding();
					let convert = f.attrs.options.convert.as_ref().unwrap();
					let convert = Ident::new(&convert.value(), convert.span());
					let revision = self.current as u16;
					bindings.append_all(quote! {
						Self::#convert(&mut __fields,#revision,#binding)?;
					})
				}
				(bindings, create)
			}
			Fields::Unnamed {
				ref fields,
				..
			} => {
				let mut bindings = TokenStream::new();
				let mut create = TokenStream::new();
				let field_names = fields
					.iter()
					.filter(|x| x.attrs.options.exists_at(self.target))
					.map(|x| x.name.to_binding());

				bindings.append_all(quote! {
					let mut __fields = #fields_struct_name( #(#field_names),* );
				});

				let field_names = fields
					.iter()
					.filter(|x| x.attrs.options.exists_at(self.target))
					.enumerate()
					.map(|(idx, _)| Index {
						index: idx as u32,
						span: Span::call_site(),
					});
				let variant_name = &i.ident;
				create.append_all(quote! {
					Ok(Self::#variant_name( #(__fields.#field_names,)*))
				});

				for f in fields.iter().filter(|x| {
					x.attrs.options.exists_at(self.current)
						&& !x.attrs.options.exists_at(self.target)
				}) {
					let binding = f.name.to_binding();
					let convert = f.attrs.options.convert.as_ref().unwrap();
					let convert = Ident::new(&convert.value(), convert.span());
					let revision = self.current as u16;
					bindings.append_all(quote! {
						Self::#convert(&mut __fields,#revision,#binding)?;
					})
				}
				(bindings, create)
			}
			Fields::Unit => {
				let name = &i.ident;
				(
					quote! {
						let __fields = #fields_struct_name;
					},
					quote! {
						Ok(Self::#name)
					},
				)
			}
		};

		if exists_target && exists_current {
			let discr = self
				.discriminants
				.get(&i.ident)
				.expect("missed variant during discriminant calculation");

			self.stream.append_all(quote! {
				#discr => {
					#fields
					#bindings
					#create
				}
			});
		} else if !exists_target && exists_current {
			let discr = self
				.discriminants
				.get(&i.ident)
				.expect("missed variant during discriminant calculation");
			let convert = i.attrs.options.convert.as_ref().unwrap();
			let convert = Ident::new(&convert.value(), convert.span());
			let revision = self.current as u16;

			self.stream.append_all(quote! {
				#discr => {
					#fields
					#bindings

					let __conv_fn: fn(#fields_struct_name, u16) -> ::std::result::Result<Self,::revision::Error> = Self::#convert;
					Self::#convert(__fields,#revision)
				}
			})
		}

		Ok(())
	}
}

pub struct DeserializeFields<'a> {
	pub target: usize,
	pub current: usize,
	pub stream: &'a mut TokenStream,
}

impl<'ast> Visit<'ast> for DeserializeFields<'_> {
	fn visit_fields(&mut self, i: &'ast Fields) -> syn::Result<()> {
		match *i {
			Fields::Named {
				ref fields,
				..
			}
			| Fields::Unnamed {
				ref fields,
				..
			} => {
				for f in fields.iter() {
					let binding = f.name.to_binding();

					let exists_current = f.attrs.options.exists_at(self.current);
					let exists_target = f.attrs.options.exists_at(self.target);

					if exists_target && exists_current {
						let ty = &f.ty;
						self.stream.append_all(quote! {
							let #binding = <#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?;
						})
					} else if exists_target && !exists_current {
						if let Some(default) = f.attrs.options.default.as_ref() {
							let default = Ident::new(&default.value(), default.span());
							let revision = self.current as u16;
							self.stream.append_all(quote! {
								let #binding = Self::#default(#revision)?;
							})
						} else {
							self.stream.append_all(quote! {
								let #binding = Default::default();
							})
						}
					} else if !exists_target && exists_current {
						let ty = &f.ty;
						self.stream.append_all(quote! {
							let #binding = <#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?;
						})
					}
				}
			}
			Fields::Unit => {}
		}
		Ok(())
	}
}
