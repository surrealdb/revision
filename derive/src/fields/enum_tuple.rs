use super::ParsedEnumVariant;
use crate::common::Exists;
use darling::FromVariant;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::abort;
use quote::{format_ident, quote};

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) struct EnumTuple {
	revision: u16,
	index: u32,
	fields: Vec<syn::Type>,
	is_unit: bool,
	parsed: ParsedEnumVariant,
}

impl Exists for EnumTuple {
	fn start_revision(&self) -> u16 {
		self.parsed.start.unwrap_or(self.revision)
	}
	fn end_revision(&self) -> u16 {
		self.parsed.end.unwrap_or_default()
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

		let mut is_unit = false;

		// Process the enum variant fields
		let fields = match &variant.fields {
			syn::Fields::Unnamed(fields) => {
				fields.unnamed.iter().map(|field| field.ty.clone()).collect()
			}
			syn::Fields::Unit => {
				is_unit = true;
				Vec::new()
			}
			_ => Vec::new(),
		};
		// Create the enum variant holder
		EnumTuple {
			revision,
			index,
			fields,
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
			let fields = &self.fields;
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
		// Get the name of the variant
		let name = self.parsed.ident.to_string();
		// Get the variant index
		let index = self.index;
		// Get the variant identifier
		let ident = &self.parsed.ident;
		// Create a token stream for the serializer
		let mut serializer = TokenStream::new();
		// Create a token stream for the variant fields
		let mut inner = TokenStream::new();
		// Loop over each of the enum variant fields
		for (index, _) in self.fields.iter().enumerate() {
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
		if self.fields.is_empty() {
			if !self.exists_at(current) {
				quote! {
					Self::#ident => {
						// TODO: remove this variant entirely using proc macro
						panic!("The {} enum variant has been deprecated", #name);
					},
				}
			} else {
				quote! {
					Self::#ident => {
						let index: u32 = #index;
						revision::Revisioned::serialize_revisioned(&index, writer)?;
					},
				}
			}
		} else if !self.exists_at(current) {
			quote! {
				Self::#ident(#inner) => {
					// TODO: remove this variant entirely using proc macro
					panic!("The {} enum variant has been deprecated", #name);
				},
			}
		} else {
			quote! {
				Self::#ident(#inner) => {
					let index: u32 = #index;
					revision::Revisioned::serialize_revisioned(&index, writer)?;
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
		for (index, kind) in self.fields.iter().enumerate() {
			// Get the field identifier
			let field = format_ident!("v{}", index);
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
			if self.fields.is_empty() {
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
