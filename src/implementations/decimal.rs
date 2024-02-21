#![cfg(feature = "rust_decimal")]

use super::super::Error;
use super::super::Revisioned;
use rust_decimal::Decimal;

impl Revisioned for Decimal {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		writer
			.write_all(self.serialize().as_slice())
			.map_err(|e| Error::Io(e.raw_os_error().unwrap_or(0)))
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let mut v = vec![0u8; 16];
		reader
			.read_exact(v.as_mut_slice())
			.map_err(|e| Error::Io(e.raw_os_error().unwrap_or(0)))?;
		Ok(Decimal::deserialize([
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
			v.remove(0),
		]))
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::Decimal;
	use super::Revisioned;

	#[test]
	fn test_decimal_min() {
		let val = Decimal::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out = <Decimal as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_decimal_max() {
		let val = Decimal::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out = <Decimal as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}
}
