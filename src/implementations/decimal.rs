#![cfg(feature = "rust_decimal")]

use super::super::Error;
use super::super::{DeserializeRevisioned, Revisioned, SerializeRevisioned};
use rust_decimal::Decimal;

const DECIMAL_SIZE: usize = 16;

impl SerializeRevisioned for Decimal {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		writer.write_all(self.serialize().as_slice()).map_err(Error::Io)
	}
}

impl DeserializeRevisioned for Decimal {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let mut b = [0u8; DECIMAL_SIZE];
		reader.read_exact(&mut b).map_err(Error::Io)?;
		Ok(Decimal::deserialize(b))
	}
}

impl Revisioned for Decimal {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// --------------------------------------------------
// Optimized implementation for Vec<Decimal>
// --------------------------------------------------

impl SerializeRevisioned for Vec<Decimal> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Write the length first (number of Decimal elements)
		self.len().serialize_revisioned(writer)?;
		// For zero-length vectors, return early
		if self.is_empty() {
			return Ok(());
		}
		// Pre-allocate buffer for all decimals to reduce syscalls
		let total = self.len().checked_mul(DECIMAL_SIZE).ok_or(Error::IntegerOverflow)?;
		let mut buffer = Vec::with_capacity(total);
		// Write all decimals to the buffer
		for v in self {
			buffer.extend_from_slice(v.serialize().as_slice());
		}
		writer.write_all(&buffer).map_err(Error::Io)
	}
}

impl DeserializeRevisioned for Vec<Decimal> {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first (number of Decimal elements)
		let len = usize::deserialize_revisioned(reader)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(Vec::new());
		}
		// Get the total number of bytes to read (with overflow check)
		let total = len.checked_mul(DECIMAL_SIZE).ok_or(Error::IntegerOverflow)?;
		// Create the vector
		let mut bytes = Vec::with_capacity(total);
		// SAFETY: We immediately read into the uninitialized buffer, and if read_exact fails,
		// we never use the vec. All u8 bit patterns are valid.
		unsafe {
			bytes.set_len(total);
		}
		reader.read_exact(&mut bytes).map_err(Error::Io)?;
		// Convert to Decimals
		let mut vec = Vec::with_capacity(len);
		for chunk in bytes.chunks_exact(DECIMAL_SIZE) {
			let arr: [u8; DECIMAL_SIZE] = chunk.try_into().unwrap();
			vec.push(Decimal::deserialize(arr));
		}
		Ok(vec)
	}
}

impl Revisioned for Vec<Decimal> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::*;
	use std::str::FromStr;

	#[test]
	fn test_decimal_min() {
		let val = Decimal::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out = <Decimal as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_decimal_max() {
		let val = Decimal::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out = <Decimal as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_decimal() {
		let val = vec![
			Decimal::MIN,
			Decimal::from_str("-1.5").unwrap(),
			Decimal::ZERO,
			Decimal::from_str("3.14159").unwrap(),
			Decimal::MAX,
		];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		// 1 byte length + 5 * 16 bytes
		assert_eq!(mem.len(), 1 + 5 * 16);
		let out =
			<Vec<Decimal> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_decimal_empty() {
		let val: Vec<Decimal> = vec![];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Vec<Decimal> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_decimal_large() {
		let val: Vec<Decimal> = (0..100).map(|i| Decimal::from(i)).collect();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Vec<Decimal> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}
}
