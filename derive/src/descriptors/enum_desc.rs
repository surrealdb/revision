use crate::common::{Descriptor, GenericDescriptor};
use crate::fields::enum_inner::*;
use crate::fields::enum_struct::*;
use crate::fields::enum_tuple::*;
use crate::helpers::compute_revision;
use quote::quote;

pub(crate) type EnumDescriptor = GenericDescriptor<EnumInner>;

impl EnumDescriptor {
	pub fn new(input: &syn::DataEnum, ident: syn::Ident) -> Self {
		// Create the new descriptor
		let mut descriptor = EnumDescriptor {
			ident,
			revision: 1,
			fields: vec![],
		};
		// Parse the enum variants
		descriptor.parse_enum_variants(&input.variants);
		// Compute the enum revision
		descriptor.revision = compute_revision(&descriptor.fields);
		// Check field attributes
		for field in &descriptor.fields {
			field.check_attributes(descriptor.revision);
		}
		// Return the descriptor
		descriptor
	}

	fn parse_enum_variants(
		&mut self,
		variants: &syn::punctuated::Punctuated<syn::Variant, syn::token::Comma>,
	) {
		for (i, variant) in variants.iter().enumerate() {
			match variant.fields {
				syn::Fields::Unnamed(_) => self.fields.push(EnumInner::EnumTuple(EnumTuple::new(
					self.revision,
					variant,
					i as u32,
				))),
				syn::Fields::Named(_) => self.fields.push(EnumInner::EnumStruct(EnumStruct::new(
					self.revision,
					variant,
					i as u32,
				))),
				syn::Fields::Unit => self.fields.push(EnumInner::EnumTuple(EnumTuple::new(
					self.revision,
					variant,
					i as u32,
				))),
			}
		}
	}
}

impl Descriptor for EnumDescriptor {
	// Generate the serializer for this type
	fn generate_serializer(&self) -> proc_macro2::TokenStream {
		// Get the current revision
		let revision = self.revision;
		// Create a new token stream
		let mut serializer = proc_macro2::TokenStream::new();
		// Extend the token stream for each field
		for field in &self.fields {
			serializer.extend(field.generate_serializer(self.revision));
		}
		// Output the token stream
		quote! {
			revision::Revisioned::serialize_revisioned(&#revision, writer)?;
			match self {
				#serializer
			}
			Ok(())
		}
	}
	// Generate the deserializer for this type
	fn generate_deserializer(&self) -> proc_macro2::TokenStream {
		// Create a new token stream
		let mut deserializer = proc_macro2::TokenStream::new();
		// Extend the token stream for each revision
		for i in 1..=self.revision {
			// Create a new token stream for the struct revision `i`.
			let mut variant = proc_macro2::TokenStream::new();
			// Generate field and semantic deserializers for all fields.
			for field in &self.fields {
				variant.extend(field.generate_deserializer(self.revision, i));
			}
			// Generate the deserializer match arm for revision `i`.
			deserializer.extend(quote! {
				#i => match variant {
					#variant
					v => return Err(revision::Error::Deserialize({
						let res = format!(
							"Unknown {:?} variant {}.",
							<Self as revision::Revisioned>::type_id(),
							variant
						);
						res
					})),
				},
			});
		}
		// Output the token stream
		quote! {
			// Deserialize the data revision
			let revision = <u16 as revision::Revisioned>::deserialize_revisioned(&mut reader)?;
			// Deserialize the enum variant
			let variant = <u32 as revision::Revisioned>::deserialize_revisioned(&mut reader)?;
			// Output logic for this revision
			match revision {
				#deserializer
				v => return Err(revision::Error::Deserialize({
					let res = format!(
						"Unknown {:?} revision {}.",
						<Self as revision::Revisioned>::type_id(),
						revision
					);
					res
				})),
			}
		}
	}

	fn revision(&self) -> u16 {
		self.revision
	}
}
