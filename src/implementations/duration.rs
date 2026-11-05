use crate::DeserializeRevisioned;
use crate::SerializeRevisioned;

use super::super::Error;
use super::super::Revisioned;
use std::time::Duration;

impl SerializeRevisioned for Duration {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.as_secs().serialize_revisioned(writer)?;
		self.subsec_nanos().serialize_revisioned(writer)
	}
}

impl DeserializeRevisioned for Duration {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let secs = u64::deserialize_revisioned(reader)?;
		let nanos = u32::deserialize_revisioned(reader)?;
		// Duration::new carries whole seconds out of nanos and panics if that
		// pushes secs past u64::MAX, so do the carry here and report it instead.
		let secs = secs
			.checked_add((nanos / 1_000_000_000) as u64)
			.ok_or_else(|| Error::Deserialize("invalid duration".to_string()))?;
		Ok(Duration::new(secs, nanos % 1_000_000_000))
	}
}

impl Revisioned for Duration {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {
	use crate::implementations::assert_bincode_compat;

	use super::*;

	#[test]
	fn test_string() {
		let val = Duration::from_secs(604800);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 6);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 12);
		let out = <Duration as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn deserialize_rejects_overflowing_duration() {
		let mut mem: Vec<u8> = vec![];
		u64::MAX.serialize_revisioned(&mut mem).unwrap();
		1_000_000_000u32.serialize_revisioned(&mut mem).unwrap();
		let out = <Duration as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice());
		assert!(out.is_err());
	}

	#[test]
	fn deserialize_accepts_maximum_duration() {
		let val = Duration::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Duration as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn bincode_compat() {
		assert_bincode_compat(&Duration::ZERO);
		assert_bincode_compat(&Duration::MAX);
		assert_bincode_compat(&Duration::new(u64::MAX, 0));
		assert_bincode_compat(&Duration::new(0, 999_999_999));
	}
}
