use crate::common::{Descriptor, GenericDescriptor, Kind};
use crate::fields::struct_field::*;
use crate::fields::struct_index::*;
use crate::fields::struct_inner::*;
use crate::helpers::compute_revision;
use quote::{format_ident, quote};

pub(crate) type StructDescriptor = GenericDescriptor<StructInner>;

impl StructDescriptor {
	pub fn new(input: &syn::ItemStruct) -> Self {
		// Create the new descriptor
		let mut descriptor = StructDescriptor {
			ident: input.ident.clone(),
			vis: input.vis.clone(),
			generics: input.generics.clone(),
			attrs: input.attrs.clone(),
			revision: 1,
			fields: vec![],
			kind: Kind::Struct,
		};
		// Parse the struct fields
		descriptor.parse_struct_fields(&input.fields);
		// Compute the struct revision
		descriptor.revision = compute_revision(&descriptor.fields);
		// Check field attributes
		for field in &descriptor.fields {
			field.check_attributes(descriptor.revision);
		}
		// Return the descriptor
		descriptor
	}

	fn parse_struct_fields(&mut self, fields: &syn::Fields) {
		match fields {
			syn::Fields::Named(fields) => {
				self.kind = Kind::Struct;
				let pairs = fields.named.pairs();
				for (i, field) in pairs.enumerate() {
					let field = field.value();
					self.fields.push(StructInner::StructField(StructField::new(
						self.revision,
						field,
						i as u32,
					)));
				}
			}
			syn::Fields::Unnamed(fields) => {
				self.kind = Kind::Tuple;
				let pairs = fields.unnamed.pairs();
				for (i, field) in pairs.enumerate() {
					let field = field.value();
					self.fields.push(StructInner::StructIndex(StructIndex::new(
						self.revision,
						field,
						i as u32,
					)));
				}
			}
			syn::Fields::Unit => {
				self.kind = Kind::Unit;
			}
		}
	}
}

impl Descriptor for StructDescriptor {
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
			#serializer
			Ok(())
		}
	}
	// Generate the deserializer for this type
	fn generate_deserializer(&self) -> proc_macro2::TokenStream {
		// Format the name of the struct
		let ident = format_ident!("{}", self.ident);
		// Create a new token stream
		let mut deserializer = proc_macro2::TokenStream::new();
		// Extend the token stream for each revision
		for i in 1..=self.revision {
			// Create a new token stream for the struct revision `i`.
			let mut outer = proc_macro2::TokenStream::new();
			let mut inner = proc_macro2::TokenStream::new();
			let mut after = proc_macro2::TokenStream::new();
			// Generate field and semantic deserializers for all fields.
			for field in &self.fields {
				let (o, i, a) = field.generate_deserializer(self.revision, i);
				outer.extend(o);
				inner.extend(i);
				after.extend(a);
			}
			// Generate the deserializer match arm for revision `i`.
			deserializer.extend(quote! {
				#i => {
					#outer
					let mut object = #ident {
						#inner
					};
					#after
					Ok(object)
				}
			});
		}
		let name = &self.ident;
		// Output the token stream
		quote! {
			// Deserialize the data revision
			let revision = <u16 as revision::Revisioned>::deserialize_revisioned(reader)?;
			// Output logic for this revision
			match revision {
				#deserializer
				v => return Err(revision::Error::Deserialize({
					let res = format!(
						concat!("Unknown '", stringify!(#name) ,"' variant {}."),
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

	fn reexpand(&self) -> proc_macro2::TokenStream {
		let vis = &self.vis;
		let ident = &self.ident;
		let attrs = &self.attrs;
		let generics = &self.generics;
		let fields = self.fields.iter().map(|e| e.reexpand());

		match self.kind {
			Kind::Unit => quote! {
				#(#attrs)*
				#vis struct #ident #generics;
			},
			Kind::Tuple => quote! {
				#(#attrs)*
				#vis struct #ident #generics(#(#fields,)*);
			},
			Kind::Struct => quote! {
				#(#attrs)*
				#vis struct #ident #generics {
					#(#fields,)*
				}
			},
			_ => unreachable!(),
		}
	}
}
