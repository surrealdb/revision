use super::enum_struct::*;
use super::enum_tuple::*;
use crate::common::Exists;
use proc_macro2::TokenStream;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) enum EnumInner {
	EnumTuple(EnumTuple),
	EnumStruct(EnumStruct),
}

impl Exists for EnumInner {
	fn start_revision(&self) -> u16 {
		match self {
			Self::EnumTuple(v) => v.start_revision(),
			Self::EnumStruct(v) => v.start_revision(),
		}
	}
	fn end_revision(&self) -> Option<u16> {
		match self {
			Self::EnumTuple(v) => v.end_revision(),
			Self::EnumStruct(v) => v.end_revision(),
		}
	}
	fn sub_revision(&self) -> u16 {
		match self {
			Self::EnumTuple(v) => v.sub_revision(),
			Self::EnumStruct(v) => v.sub_revision(),
		}
	}
}

impl EnumInner {
	pub fn check_attributes(&self, current: u16) {
		match self {
			Self::EnumTuple(v) => v.check_attributes(current),
			Self::EnumStruct(v) => v.check_attributes(current),
		}
	}
	pub fn generate_serializer(&self, current: u16) -> TokenStream {
		match self {
			Self::EnumTuple(v) => v.generate_serializer(current),
			Self::EnumStruct(v) => v.generate_serializer(current),
		}
	}
	pub fn generate_deserializer(&self, current: u16, revision: u16) -> TokenStream {
		match self {
			Self::EnumTuple(v) => v.generate_deserializer(current, revision),
			Self::EnumStruct(v) => v.generate_deserializer(current, revision),
		}
	}

	pub fn reexpand(&self) -> TokenStream {
		match self {
			Self::EnumTuple(v) => v.reexpand(),
			Self::EnumStruct(v) => v.reexpand(),
		}
	}
}
