use super::ParsedEnumVariant;
use crate::common::Exists;
use darling::FromVariant;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::spanned::Spanned;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) struct EnumTuple {
	revision: u16,
	index: u32,
	is_unit: bool,
	parsed: ParsedEnumVariant,
}

impl Exists for EnumTuple {
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

impl EnumTuple {
	pub fn new(revision: u16, variant: &syn::Variant, index: u32) -> Self {
		// Parse the variant macro attributes
		let parsed = match ParsedEnumVariant::from_variant(variant) {
			Ok(x) => x,
			Err(e) => {
				abort!(variant.ident.span(), "{}", e);
			}
		};

		for f in parsed.fields.iter() {
			if f.end.is_some()
				|| f.start.is_some()
				|| f.default_fn.is_some()
				|| f.convert_fn.is_some()
			{
				abort!(
					f.ty.span(),
					"Revision attributes are not yet supported on enum variant fields"
				)
			}
		}

		let is_unit = matches!(variant.fields, syn::Fields::Unit);

		// Create the enum variant holder
		EnumTuple {
			revision,
			index,
			parsed,
			is_unit,
		}
	}

	pub fn reexpand(&self) -> TokenStream {
		let ident = &self.parsed.ident;
		let attrs = &self.parsed.attrs;
		if self.is_unit {
			quote!(
				#(#attrs)*
				#ident
			)
		} else {
			let fields = self.parsed.fields.iter().map(|x| {
				let attr = &x.attrs;
				let ty = &x.ty;
				quote!(
					#(#attr)* #ty
				)
			});
			quote!(
				#(#attrs)*
				#ident( #(#fields,)* )
			)
		}
	}

	pub fn check_attributes(&self, current: u16) {
		if !self.exists_at(current) && self.parsed.convert_fn.is_none() {
			abort!(
				self.parsed.ident.span(),
				"Expected a 'convert_fn' to be specified for enum variant {}",
				self.parsed.ident
			);
		}
	}

	pub fn generate_serializer(&self, current: u16) -> TokenStream {
		// Get the variant index
		let index = self.index;
		// Get the variant identifier
		let ident = &self.parsed.ident;
		// Create a token stream for the serializer
		let mut serializer = TokenStream::new();
		// Create a token stream for the variant fields
		let mut inner = TokenStream::new();
		// Loop over each of the enum variant fields
		for (index, _) in self.parsed.fields.iter().enumerate() {
			// Get the field identifier
			let field = format_ident!("v{}", index);
			// Extend the enum constructor
			inner.extend(quote!(#field,));
			// Extend the serializer
			serializer.extend(quote! {
				revision::Revisioned::serialize_revisioned(#field, writer)?;
			});
		}
		// Output the token stream
		if self.parsed.fields.is_empty() {
			if !self.exists_at(current) {
				panic!("tried to generate a serializer a field which was deleted.");
			} else {
				quote! {
					Self::#ident => {
						revision::Revisioned::serialize_revisioned(&#index, writer)?;
					},
				}
			}
		} else if !self.exists_at(current) {
			panic!("tried to generate a serializer a field which was deleted.");
		} else {
			quote! {
				Self::#ident(#inner) => {
					revision::Revisioned::serialize_revisioned(&#index, writer)?;
					#serializer
				},
			}
		}
	}

	pub fn generate_deserializer(&self, current: u16, revision: u16) -> TokenStream {
		// Get the variant index
		let index = self.index;
		// Get the variant identifier
		let ident = &self.parsed.ident;
		// Check if the variant is new.
		if !self.exists_at(revision) {
			return quote!();
		}
		// Create a token stream for the field deserialisation
		let mut deserializer = TokenStream::new();
		// Create a token stream for the fields
		let mut inner = TokenStream::new();
		// Loop over the enum variant fields
		for (index, f) in self.parsed.fields.iter().enumerate() {
			// Get the field identifier
			let field = format_ident!("v{}", index);
			let kind = &f.ty;
			// Extend the enum constructor
			inner.extend(quote!(#field,));
			// Extend the deserializer
			deserializer.extend(quote! {
				let #field = <#kind as revision::Revisioned>::deserialize_revisioned(reader)?;
			});
		}
		// Check if the variant no longer exists
		if !self.exists_at(current) {
			// Get the conversion function
			let convert_fn =
				syn::Ident::new(self.parsed.convert_fn.as_ref().unwrap(), Span::call_site());
			// Output the
			quote! {
				#index => {
					#deserializer
					return Self::#convert_fn(revision, (#inner));
				},
			}
		} else {
			// Check if this is a simple enum
			if self.parsed.fields.is_empty() {
				quote! {
					#index => {
						return Ok(Self::#ident);
					},
				}
			} else {
				quote! {
					#index => {
						#deserializer
						return Ok(Self::#ident(#inner));
					},
				}
			}
		}
	}
}
