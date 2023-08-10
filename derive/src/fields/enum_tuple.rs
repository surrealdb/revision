use super::super::CONVERT_FN;
use crate::common::Exists;
use crate::helpers::{
	get_end_revision, get_ident_attr, get_start_revision, parse_field_attributes,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::hash_map::HashMap;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) struct EnumTuple {
	ident: syn::Ident,
	index: u32,
	fields: Vec<syn::Type>,
	start_revision: u16,
	end_revision: u16,
	attrs: HashMap<String, syn::Lit>,
}

impl Exists for EnumTuple {
	fn start_revision(&self) -> u16 {
		self.start_revision
	}
	fn end_revision(&self) -> u16 {
		self.end_revision
	}
	fn sub_revision(&self) -> u16 {
		0
	}
}

impl EnumTuple {
	pub fn new(revision: u16, variant: &syn::Variant, index: u32) -> Self {
		// Parse the field macro attributes
		let attrs = parse_field_attributes(&variant.attrs);
		// Process the enum variant fields
		let fields = match &variant.fields {
			syn::Fields::Unnamed(fields) => {
				fields.unnamed.iter().map(|field| field.ty.clone()).collect()
			}
			_ => Vec::new(),
		};
		// Create the enum variant holder
		EnumTuple {
			ident: variant.ident.clone(),
			index,
			fields,
			start_revision: get_start_revision(&attrs).unwrap_or(revision),
			end_revision: get_end_revision(&attrs).unwrap_or_default(),
			attrs,
		}
	}

	pub fn check_attributes(&self, current: u16) {
		if !self.exists_at(current) {
			if get_ident_attr(&self.attrs, CONVERT_FN).is_none() {
				panic!("Expected a 'convert_fn' to be specified for enum variant {}", self.ident);
			}
		}
	}

	pub fn generate_serializer(&self, current: u16) -> TokenStream {
		// Get the name of the variant
		let name = self.ident.to_string();
		// Get the variant index
		let index = self.index;
		// Get the variant identifier
		let ident = &self.ident;
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
		} else {
			if !self.exists_at(current) {
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
	}

	pub fn generate_deserializer(&self, current: u16, revision: u16) -> TokenStream {
		// Get the variant index
		let index = self.index;
		// Get the variant identifier
		let ident = &self.ident;
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
			let convert_fn = get_ident_attr(&self.attrs, CONVERT_FN).unwrap();
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
