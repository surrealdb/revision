use crate::DeserializeRevisioned;
use crate::SerializeRevisioned;

use super::super::Error;
use super::super::Revisioned;
use std::ops::Range;

impl<T: SerializeRevisioned> SerializeRevisioned for Range<T> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.start.serialize_revisioned(writer)?;
		self.end.serialize_revisioned(writer)
	}
}

impl<T: DeserializeRevisioned> DeserializeRevisioned for Range<T> {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let start = T::deserialize_revisioned(reader)?;
		let end = T::deserialize_revisioned(reader)?;
		Ok(Range {
			start,
			end,
		})
	}
}

impl<T: Revisioned> Revisioned for Range<T> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_range_u32() {
		let val = 10u32..100u32;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<u32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_string() {
		let val = String::from("aaa")..String::from("zzz");
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<String> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_empty() {
		let val = 5u32..5u32;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<u32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
		assert!(val.is_empty());
	}

	#[test]
	#[allow(clippy::reversed_empty_ranges)]
	fn test_range_inverted() {
		let val = 100u32..10u32;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<u32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_i32() {
		let val = -100i32..100i32;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<i32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_i32_negative() {
		let val = -200i32..-100i32;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<i32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_f64() {
		let val = 1.5f64..9.9f64;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<f64> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_zero() {
		let val = 0u32..0u32;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<u32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_max_u64() {
		let val = 0u64..u64::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<u64> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_nested_option() {
		let val = Some(10u32)..Some(100u32);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Range<Option<u32>> as DeserializeRevisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_nested_option_with_none() {
		let val: Range<Option<u32>> = None..Some(100u32);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Range<Option<u32>> as DeserializeRevisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_revision() {
		assert_eq!(<Range<u32> as Revisioned>::revision(), 1);
		assert_eq!(<Range<String> as Revisioned>::revision(), 1);
		assert_eq!(<Range<i64> as Revisioned>::revision(), 1);
	}

	#[test]
	fn test_range_serialized_bytes() {
		let val = 10u32..100u32;
		let mut range_bytes: Vec<u8> = vec![];
		val.serialize_revisioned(&mut range_bytes).unwrap();

		let mut expected_bytes: Vec<u8> = vec![];
		val.start.serialize_revisioned(&mut expected_bytes).unwrap();
		val.end.serialize_revisioned(&mut expected_bytes).unwrap();

		assert_eq!(range_bytes, expected_bytes);
	}

	#[test]
	fn test_range_truncated_data() {
		let val = 10u32..100u32;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();

		let mut truncated = &mem[..1];
		let result = <Range<u32> as DeserializeRevisioned>::deserialize_revisioned(&mut truncated);
		assert!(result.is_err());
	}

	#[test]
	fn test_range_empty_reader() {
		let mut empty: &[u8] = &[];
		let result = <Range<u32> as DeserializeRevisioned>::deserialize_revisioned(&mut empty);
		assert!(result.is_err());
	}

	#[test]
	fn test_range_i64_boundaries() {
		let val = i64::MIN..i64::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<i64> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_f64_special_values() {
		let val = f64::NEG_INFINITY..f64::INFINITY;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<f64> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_u8() {
		let val = 0u8..255u8;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Range<u8> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_range_bool() {
		let val = false..true;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Range<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}
}
