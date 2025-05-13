#![cfg(feature = "rust_decimal")]

use super::super::Error;
use super::super::{Revisioned, DeserializeRevisioned, SerializeRevisioned};
use rust_decimal::Decimal;

impl SerializeRevisioned for Decimal {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		writer.write_all(self.serialize().as_slice()).map_err(Error::Io)
	}
}

impl DeserializeRevisioned for Decimal {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let mut b = [0u8; 16];
		reader.read_exact(&mut b).map_err(Error::Io)?;
		Ok(Decimal::deserialize(b))
	}
}

impl Revisioned for Decimal {
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn test_decimal_min() {
		let val = Decimal::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out = <Decimal as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_decimal_max() {
		let val = Decimal::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out = <Decimal as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}
}
