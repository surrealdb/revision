use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{TokenStreamExt, quote};
use syn::Ident;

use crate::ast::{Enum, Fields, Struct, Variant, Visit};

use super::common::CalcDiscriminant;

pub struct SkipVisitor<'a> {
	pub target: usize,
	pub current: usize,
	pub stream: &'a mut TokenStream,
}

impl<'ast> Visit<'ast> for SkipVisitor<'_> {
	fn visit_enum(&mut self, i: &'ast Enum) -> syn::Result<()> {
		let mut discriminants = HashMap::new();
		CalcDiscriminant::new(self.current, &mut discriminants).visit_enum(i)?;

		let mut variants = TokenStream::new();
		SkipVariant {
			target: self.target,
			current: self.current,
			stream: &mut variants,
			discriminants,
		}
		.visit_enum(i)?;

		let error_string =
			format!("Invalid discriminant `{{x}}` for enum `{}` revision `{{__revision}}`", i.name);

		self.stream.append_all(quote! {
			let __discriminant =
				<u32 as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?;
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
		match i.fields {
			Fields::Unit => {}
			Fields::Named {
				..
			}
			| Fields::Unnamed {
				..
			} => {
				let mut skips = TokenStream::new();
				SkipFields {
					target: self.target,
					current: self.current,
					stream: &mut skips,
				}
				.visit_fields(&i.fields)?;

				self.stream.append_all(skips);
			}
		}

		self.stream.append_all(quote! { Ok(()) });
		Ok(())
	}
}

pub struct SkipVariant<'a> {
	pub target: usize,
	pub current: usize,
	pub stream: &'a mut TokenStream,
	pub discriminants: HashMap<Ident, u32>,
}

impl<'ast> Visit<'ast> for SkipVariant<'_> {
	fn visit_variant(&mut self, i: &'ast Variant) -> syn::Result<()> {
		let exists_current = i.attrs.options.exists_at(self.current);
		let exists_target = i.attrs.options.exists_at(self.target);

		if !exists_current {
			return Ok(());
		}

		let mut fields = TokenStream::new();
		SkipFields {
			target: self.target,
			current: self.current,
			stream: &mut fields,
		}
		.visit_variant(i)?;

		if exists_target && exists_current || !exists_target && exists_current {
			let discr = self
				.discriminants
				.get(&i.ident)
				.expect("missed variant during discriminant calculation");

			self.stream.append_all(quote! {
				#discr => {
					#fields
					Ok(())
				},
			});
		}

		Ok(())
	}
}

pub struct SkipFields<'a> {
	pub target: usize,
	pub current: usize,
	pub stream: &'a mut TokenStream,
}

impl<'ast> Visit<'ast> for SkipFields<'_> {
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
					let exists_current = f.attrs.options.exists_at(self.current);
					let exists_target = f.attrs.options.exists_at(self.target);

					if exists_target && exists_current {
						let ty = &f.ty;
						self.stream.append_all(quote! {
							<#ty as ::revision::SkipRevisioned>::skip_revisioned(reader)?;
						})
					} else if exists_target && !exists_current {
						// Field absent on wire at this revision.
					} else if !exists_target && exists_current {
						let ty = &f.ty;
						self.stream.append_all(quote! {
							<#ty as ::revision::SkipRevisioned>::skip_revisioned(reader)?;
						})
					}
				}
			}
			Fields::Unit => {}
		}
		Ok(())
	}
}
