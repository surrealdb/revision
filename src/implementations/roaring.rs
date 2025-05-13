#![cfg(feature = "roaring")]

use super::super::Error;
use super::super::{DeserializeRevisioned, Revisioned, SerializeRevisioned};
use roaring::{RoaringBitmap, RoaringTreemap};

impl SerializeRevisioned for RoaringTreemap {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.serialize_into(writer).map_err(|ref err| Error::Serialize(format!("{:?}", err)))
	}
}

impl DeserializeRevisioned for RoaringTreemap {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Self::deserialize_from(reader).map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
	}
}

impl Revisioned for RoaringTreemap {
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for RoaringBitmap {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.serialize_into(writer).map_err(|ref err| Error::Serialize(format!("{:?}", err)))
	}
}

impl DeserializeRevisioned for RoaringBitmap {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Self::deserialize_from(reader).map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
	}
}

impl Revisioned for RoaringBitmap {
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_roaring_treemap() {
		let val = RoaringTreemap::new();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 8);
		let out =
			<RoaringTreemap as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_roaring_bitmap() {
		let val = RoaringBitmap::new();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 8);
		let out =
			<RoaringBitmap as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}
}
