use crate::common::Exists;
use crate::fields::enum_struct_field::*;
use darling::FromVariant;
use proc_macro2::TokenStream;
use quote::quote;
use std::cmp::max;

use super::ParsedEnumVariant;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) struct EnumStruct {
	revision: u16,
	index: u32,
	parsed: ParsedEnumVariant,
	// fields: Vec<(syn::Ident, syn::Type)>,
	fields: Vec<StructField>,
}

impl Exists for EnumStruct {
	fn start_revision(&self) -> u16 {
		self.parsed.start.unwrap_or(self.revision)
	}
	fn end_revision(&self) -> u16 {
		self.parsed.end.unwrap_or_default()
	}
	fn sub_revision(&self) -> u16 {
		let mut revision = 1;
		for field in self.fields.iter() {
			revision = max(revision, max(field.start_revision(), field.end_revision()));
		}
		revision
	}
}

impl EnumStruct {
	pub fn new(revision: u16, variant: &syn::Variant, index: u32) -> Self {
		// Parse the field macro attributes
		let parsed = match ParsedEnumVariant::from_variant(variant) {
			Ok(x) => x,
			Err(e) => {
				abort!(variant.ident.span(), "{}", e);
			}
		};
		// Process the enum variant fields
		let fields = match &variant.fields {
			// syn::Fields::Named(fields) => fields
			//     .named
			//     .iter()
			//     .map(|field| (field.ident.clone().unwrap(), field.ty.clone()))
			//     .collect(),
			syn::Fields::Named(fields) => fields
				.named
				.iter()
				.enumerate()
				.map(|(i, field)| StructField::new(revision, field, i as u32))
				.collect(),
			_ => Vec::new(),
		};
		// Create the enum variant holder
		EnumStruct {
			revision,
			index,
			fields,
			parsed,
		}
	}

	pub fn reexpand(&self) -> TokenStream {
		let ident = &self.parsed.ident;
		let attrs = &self.parsed.attrs;
		let fields = self.fields.iter().map(|x| x.reexpand());
		quote!(
			#(#attrs)*
			#ident{ #(#fields,)* }
		)
	}

	pub fn check_attributes(&self, current: u16) {
		if !self.exists_at(current) && self.parsed.convert_fn.is_none() {
			abort!(
				self.parsed.ident.span(),
				"Expected a 'convert_fn' to be specified for enum variant {}",
				self.parsed.ident
			);
		}
		// Check field attributes
		for field in &self.fields {
			field.check_attributes(current);
		}
	}

	pub fn generate_serializer(&self, revision: u16) -> TokenStream {
		// Get the variant identifier
		let field_ident = &self.parsed.ident;
		// Get the variant index
		let index = self.index;
		// Create a token stream for the serializer
		let mut serializer = TokenStream::new();
		// Create a token stream for the variant fields
		let mut inner = TokenStream::new();
		// Loop over each of the enum variant fields
		for field in &self.fields {
			// Get the field identifier
			let name = field.name();
			// Extend the enum constructor
			inner.extend(quote!(#name,));
			// Extend the serializer
			serializer.extend(field.generate_serializer(revision));
		}
		// Output the token stream
		quote! {
			Self::#field_ident{#inner} => {
				let index: u32 = #index;
				revision::Revisioned::serialize_revisioned(&index, writer)?;
				#serializer
			},
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
		// Create a new token stream for the struct revision `i`.
		let mut outer = proc_macro2::TokenStream::new();
		let mut inner = proc_macro2::TokenStream::new();
		let mut after = proc_macro2::TokenStream::new();
		// Loop over each of the enum variant fields
		for field in &self.fields {
			let (o, i, a) = field.generate_deserializer(current, revision);
			outer.extend(o);
			inner.extend(i);
			after.extend(a);
		}
		// Output the token stream
		quote! {
			#index => {
				#outer
				let mut object = Self::#ident{#inner};
				#after
				Ok(object)
			},
		}
	}
}
