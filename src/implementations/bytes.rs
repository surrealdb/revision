#![cfg(feature = "bytes")]

use crate::implementations::vecs::serialize_bytes;
use crate::Error;
use crate::{DeserializeRevisioned, Revisioned, SerializeRevisioned};
use ::bytes::Bytes;
use std::io::ErrorKind::UnexpectedEof;
use std::io::Read;

impl SerializeRevisioned for Bytes {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		serialize_bytes(self.as_ref(), writer)
	}
}

impl DeserializeRevisioned for Bytes {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let len = usize::deserialize_revisioned(reader)?;
		if len == 0 {
			return Ok(Bytes::new());
		}

		let mut bytes = Vec::with_capacity(len);
		let mut take = reader.take(len as u64);
		if len != take.read_to_end(&mut bytes).map_err(Error::Io)? {
			return Err(Error::Io(UnexpectedEof.into()));
		}
		Ok(Bytes::from(bytes))
	}
}

impl Revisioned for Bytes {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{from_slice, to_vec};

	#[test]
	fn test_revision_specialised_vec_u8_serialization_single() {
		let original = Bytes::from(vec![42]);
		let serialized = to_vec(&original).unwrap();
		let deserialized: Bytes = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.len(), 1);
		assert_eq!(deserialized[0], 42);
	}

	#[test]
	fn test_revision_specialised_vec_u8_serialization_multiple() {
		let data = vec![0, 1, 127, 128, 255];
		let original = Bytes::from(data.clone());
		let serialized = to_vec(&original).unwrap();
		let deserialized: Bytes = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.as_ref(), &data);
	}

	#[test]
	fn test_revision_specialised_vec_u8_serialization_large() {
		let data: Vec<u8> = (0..=255).collect();
		let original = Bytes::from(data.clone());
		let serialized = to_vec(&original).unwrap();
		let deserialized: Bytes = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.len(), 256);
		assert_eq!(deserialized.as_ref(), &data);
	}

	#[test]
	fn test_revision_specialised_vec_u8_conversion() {
		let original_vec = vec![1, 2, 3, 4, 5];
		let wrapper: Bytes = original_vec.clone().into();
		assert_eq!(wrapper.as_ref(), &original_vec);

		let extracted_vec: Vec<u8> = wrapper.into();
		assert_eq!(extracted_vec, original_vec);
	}

	#[test]
	fn test_revision_specialised_vec_u8_as_ref() {
		let wrapper = Bytes::from(vec![1, 2, 3]);
		let slice: &[u8] = wrapper.as_ref();
		assert_eq!(slice, &[1, 2, 3]);
	}
}
