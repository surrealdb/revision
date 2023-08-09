use super::struct_field::*;
use super::struct_index::*;
use crate::common::Exists;
use proc_macro2::TokenStream;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) enum StructInner {
    StructField(StructField),
    StructIndex(StructIndex),
}

impl Exists for StructInner {
    fn start_revision(&self) -> u16 {
        match self {
            Self::StructField(v) => v.start_revision(),
            Self::StructIndex(v) => v.start_revision(),
        }
    }
    fn end_revision(&self) -> u16 {
        match self {
            Self::StructField(v) => v.end_revision(),
            Self::StructIndex(v) => v.end_revision(),
        }
    }
    fn sub_revision(&self) -> u16 {
        match self {
            Self::StructField(v) => v.sub_revision(),
            Self::StructIndex(v) => v.sub_revision(),
        }
    }
}

impl StructInner {
    pub fn check_attributes(&self, current: u16) {
        match self {
            Self::StructField(v) => v.check_attributes(current),
            Self::StructIndex(v) => v.check_attributes(current),
        }
    }
    pub fn generate_serializer(&self, current: u16) -> TokenStream {
        match self {
            Self::StructField(v) => v.generate_serializer(current),
            Self::StructIndex(v) => v.generate_serializer(current),
        }
    }
    pub fn generate_deserializer(
        &self,
        current: u16,
        revision: u16,
    ) -> (TokenStream, TokenStream, TokenStream) {
        match self {
            Self::StructField(v) => v.generate_deserializer(current, revision),
            Self::StructIndex(v) => v.generate_deserializer(current, revision),
        }
    }
}
