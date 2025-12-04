#![cfg(feature = "uuid")]

use super::super::Error;
use super::super::{DeserializeRevisioned, Revisioned, SerializeRevisioned};
use uuid::Uuid;

const UUID_SIZE: usize = 16;

impl SerializeRevisioned for Uuid {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		writer.write_all(self.as_bytes()).map_err(Error::Io)
	}
}

impl DeserializeRevisioned for Uuid {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let mut v = [0u8; UUID_SIZE];
		reader.read_exact(&mut v).map_err(Error::Io)?;
		Uuid::from_slice(&v).map_err(|_| Error::Deserialize("invalid uuid".to_string()))
	}
}

impl Revisioned for Uuid {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// --------------------------------------------------
// Optimized implementation for Vec<Uuid>
// --------------------------------------------------

#[cfg(feature = "specialised")]
impl super::specialised::SerializeRevisionedSpecialised for Vec<Uuid> {
	#[inline]
	fn serialize_revisioned_specialised<W: std::io::Write>(
		&self,
		writer: &mut W,
	) -> Result<(), Error> {
		// Write the length first (number of UUID elements)
		self.len().serialize_revisioned(writer)?;
		// For zero-length vectors, return early
		if self.is_empty() {
			return Ok(());
		}
		// Calculate byte length with overflow check
		let byte_len = self.len().checked_mul(UUID_SIZE).ok_or(Error::IntegerOverflow)?;
		// Direct byte copy - Uuid is #[repr(transparent)] over [u8; 16],
		// so Vec<Uuid> is contiguous 16-byte blocks we can write directly.
		// SAFETY:
		// 1. Uuid is #[repr(transparent)] over [u8; 16], guaranteeing layout
		// 2. Vec<Uuid> stores elements contiguously
		// 3. We're only reading from the slice, not modifying it
		// 4. UUID bytes are platform-independent (no endianness conversion needed)
		unsafe {
			let byte_slice = std::slice::from_raw_parts(self.as_ptr().cast::<u8>(), byte_len);
			writer.write_all(byte_slice).map_err(Error::Io)
		}
	}
}

#[cfg(feature = "specialised")]
impl super::specialised::DeserializeRevisionedSpecialised for Vec<Uuid> {
	#[inline]
	fn deserialize_revisioned_specialised<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first (number of UUID elements)
		let len = usize::deserialize_revisioned(reader)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(Vec::new());
		}
		// Calculate byte length with overflow check
		let byte_len = len.checked_mul(UUID_SIZE).ok_or(Error::IntegerOverflow)?;
		// Allocate initialized buffer to ensure safety on drop if read_exact fails
		let mut vec: Vec<Uuid> = vec![Uuid::nil(); len];
		// Direct byte read - Uuid is #[repr(transparent)] over [u8; 16],
		// so we can read directly into Vec<Uuid> memory.
		// SAFETY:
		// 1. Uuid is #[repr(transparent)] over [u8; 16], guaranteeing layout
		// 2. All byte patterns are valid UUIDs (it's just 16 raw bytes)
		// 3. Vec is already initialized, so drop is safe even if read_exact fails
		unsafe {
			let byte_slice =
				std::slice::from_raw_parts_mut(vec.as_mut_ptr().cast::<u8>(), byte_len);
			reader.read_exact(byte_slice).map_err(Error::Io)?;
		}
		Ok(vec)
	}
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn test_uuid() {
		#[rustfmt::skip]
        let val = Uuid::from_bytes([
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
        ]);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out =
			<Uuid as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_uuid() {
		let val = vec![
			Uuid::from_bytes([
				0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
				0x0f, 0x10,
			]),
			Uuid::from_bytes([
				0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
				0x1f, 0x20,
			]),
			Uuid::from_bytes([
				0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e,
				0x2f, 0x30,
			]),
		];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		// 1 byte length + 3 * 16 bytes
		assert_eq!(mem.len(), 1 + 3 * 16);
		let out = <Vec<Uuid> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}
}
