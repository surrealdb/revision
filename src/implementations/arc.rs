use crate::DeserializeRevisioned;
use crate::SerializeRevisioned;

use super::super::Error;
use super::super::Revisioned;
use std::sync::Arc;

impl<T> SerializeRevisioned for Arc<T>
where
	T: SerializeRevisioned,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.as_ref().serialize_revisioned(writer)
	}
}

impl<T> DeserializeRevisioned for Arc<T>
where
	T: DeserializeRevisioned,
{
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok(Arc::new(T::deserialize_revisioned(reader)?))
	}
}

impl<T> Revisioned for Arc<T>
where
	T: Revisioned,
{
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// Specialized implementations for Arc<str>
impl SerializeRevisioned for Arc<str> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.as_ref().serialize_revisioned(writer)
	}
}

impl DeserializeRevisioned for Arc<str> {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		String::deserialize_revisioned(reader).map(Arc::from)
	}
}

impl Revisioned for Arc<str> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn test_arc() {
		let val = Arc::new(u32::MAX);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 5);
		let out = DeserializeRevisioned::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_arc_str() {
		let val: Arc<str> = Arc::from("hello world");
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 12); // 11 chars + 1 byte for length encoding
		let out: Arc<str> =
			<Arc<str> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}
}
