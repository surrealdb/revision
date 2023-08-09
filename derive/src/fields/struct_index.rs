use super::super::{CONVERT_FN, DEFAULT_FN};
use crate::common::Exists;
use crate::helpers::{
    get_end_revision, get_ident_attr, get_start_revision, parse_field_attributes,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::hash_map::HashMap;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) struct StructIndex {
    index: u32,
    ty: syn::Type,
    start_revision: u16,
    end_revision: u16,
    attrs: HashMap<String, syn::Lit>,
}

impl Exists for StructIndex {
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

impl StructIndex {
    pub fn new(revision: u16, field: &syn::Field, index: u32) -> Self {
        // Parse the field macro attributes
        let attrs = parse_field_attributes(&field.attrs);
        // Create the struct field holder
        StructIndex {
            index,
            ty: field.ty.clone(),
            start_revision: get_start_revision(&attrs).unwrap_or(revision),
            end_revision: get_end_revision(&attrs).unwrap_or_default(),
            attrs,
        }
    }

    pub fn check_attributes(&self, current: u16) {
        if !self.exists_at(current) {
            if get_ident_attr(&self.attrs, CONVERT_FN).is_none() {
                panic!(
                    "Expected a 'convert_fn' to be specified for field {}",
                    self.index
                );
            }
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
        match &self.ty {
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
            v => panic!("Unsupported field type {v:?}"),
        }
    }

    pub fn generate_deserializer(
        &self,
        current: u16,
        revision: u16,
    ) -> (TokenStream, TokenStream, TokenStream) {
        // Get the field type.
        let kind = &self.ty;
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
                let #field = <#kind as revision::Revisioned>::deserialize_revisioned(&mut reader)?;
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
        // Output the token streams
        (
            // Field did not exist, so don't deserialize it
            quote! {},
            // Set the field default value on the struct
            match get_ident_attr(&self.attrs, DEFAULT_FN) {
                Some(default_fn) => quote! {
                    Self::#default_fn(revision),
                },
                None => quote! {
                    Default::default(),
                },
            },
            // No need for any field post-processing
            quote! {},
        )
    }

    fn generate_deserializer_oldfield(&self) -> (TokenStream, TokenStream, TokenStream) {
        // Get the field type.
        let kind = &self.ty;
        // Get the field index.
        let index = syn::Index::from(self.index as usize);
        // Get the field identifier.
        let field = format_ident!("v{}", index);
        // Output the token streams
        (
            // Deserialize the field which no longer exists
            quote! {
                let #field = <#kind as revision::Revisioned>::deserialize_revisioned(&mut reader)?;
            },
            // Don't insert the field into the current struct
            quote! {
                // TODO: remove this field entirely using proc macro
                Default::default(),
            },
            // Post process the field data with the struct
            match get_ident_attr(&self.attrs, CONVERT_FN) {
                Some(convert_fn) => quote! {
                    object.#convert_fn(revision, #field)?;
                },
                None => quote! {},
            },
        )
    }
}