use super::ParsedField;
use crate::common::Exists;
use darling::FromField;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::spanned::Spanned;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) struct StructIndex {
	index: u32,
	revision: u16,
	parsed: ParsedField,
}

impl Exists for StructIndex {
	fn start_revision(&self) -> u16 {
		self.parsed.start.unwrap_or(self.revision)
	}
	fn end_revision(&self) -> Option<u16> {
		self.parsed.end
	}
	fn sub_revision(&self) -> u16 {
		0
	}
}

impl StructIndex {
	pub fn new(revision: u16, field: &syn::Field, index: u32) -> Self {
		// Parse the field macro attributes
		let parsed = match ParsedField::from_field(field) {
			Ok(x) => x,
			Err(e) => {
				abort!(e.span(), "{e}")
			}
		};

		assert!(parsed.ident.is_none(), "tried to parse a named field as a tuple field");

		// Create the struct field holder
		StructIndex {
			index,
			revision,
			parsed,
		}
	}

	pub fn reexpand(&self) -> TokenStream {
		let vis = &self.parsed.vis;
		let ty = &self.parsed.ty;
		let attrs = &self.parsed.attrs;
		quote!(
			#(#attrs)* #vis #ty
		)
	}

	pub fn check_attributes(&self, current: u16) {
		if !self.exists_at(current) && self.parsed.convert_fn.is_none() {
			abort!(
				self.parsed.ty.span(),
				"Expected a 'convert_fn' to be specified for field {}",
				self.index
			);
		}
	}

	pub fn generate_serializer(&self, current: u16) -> TokenStream {
		// Get the field identifier.
		let field = syn::Index::from(self.index as usize);
		// Check if this field exists for this revision.
		if !self.exists_at(current) {
			return proc_macro2::TokenStream::new();
		}
		// Match the type of the field.
		match &self.parsed.ty {
			syn::Type::Array(_) => quote! {
				for element in self.#field.iter() {
					revision::Revisioned::serialize_revisioned(element, writer)?;
				}
			},
			syn::Type::Path(_) => quote! {
				revision::Revisioned::serialize_revisioned(&self.#field, writer)?;
			},
			syn::Type::Reference(_) => quote! {
				self.#field.serialize_revisioned(writer)?;
			},
			v => abort!(v.span(), "Unsupported field type"),
		}
	}

	pub fn generate_deserializer(
		&self,
		current: u16,
		revision: u16,
	) -> (TokenStream, TokenStream, TokenStream) {
		// Get the field type.
		let kind = &self.parsed.ty;
		// Get the field index.
		let index = syn::Index::from(self.index as usize);
		// Get the field identifier.
		let field = format_ident!("v{}", index);
		// If the field didn't exist, use default annotation or Default trait.
		if !self.exists_at(revision) {
			return self.generate_deserializer_newfield();
		}
		// If the field did exist, but no longer does, use convert annotation if specified.
		if self.exists_at(revision) && !self.exists_at(current) {
			return self.generate_deserializer_oldfield();
		}
		// Output the token streams
		(
			// Deserialize the field from the reader
			quote! {
				let #field = <#kind as revision::Revisioned>::deserialize_revisioned(reader)?;
			},
			// Insert the field value into the struct
			quote! {
				#index: #field,
			},
			// No need for any field post-processing
			quote! {},
		)
	}

	fn generate_deserializer_newfield(&self) -> (TokenStream, TokenStream, TokenStream) {
		let index = syn::Index::from(self.index as usize);
		// Output the token streams
		(
			// Field did not exist, so don't deserialize it
			quote! {},
			// Set the field default value on the struct
			match &self.parsed.default_fn {
				Some(default_fn) => {
					let default_fn = syn::Ident::new(default_fn, Span::call_site());
					quote! {
						#index: Self::#default_fn(revision),
					}
				}
				None => quote! {
					#index: Default::default(),
				},
			},
			// No need for any field post-processing
			quote! {},
		)
	}

	fn generate_deserializer_oldfield(&self) -> (TokenStream, TokenStream, TokenStream) {
		// Get the field type.
		let kind = &self.parsed.ty;
		// Get the field index.
		let index = syn::Index::from(self.index as usize);
		// Get the field identifier.
		let field = format_ident!("v{}", index);
		// Output the token streams
		(
			// Deserialize the field which no longer exists
			quote! {
				let #field = <#kind as revision::Revisioned>::deserialize_revisioned(reader)?;
			},
			// Don't insert the field into the current struct
			quote! {
				// TODO: remove this field entirely using proc macro
				Default::default(),
			},
			// Post process the field data with the struct
			match &self.parsed.convert_fn {
				Some(convert_fn) => {
					let convert_fn = syn::Ident::new(convert_fn, Span::call_site());
					quote! {
						object.#convert_fn(revision, #field)?;
					}
				}
				None => quote! {},
			},
		)
	}
}
