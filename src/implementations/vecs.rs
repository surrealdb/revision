use crate::DeserializeRevisioned;
use crate::Error;
use crate::Revisioned;
use crate::SerializeRevisioned;

pub(crate) fn serialize_bytes<W>(v: &[u8], writer: &mut W) -> Result<(), Error>
where
	W: std::io::Write,
{
	v.len().serialize_revisioned(writer)?;
	writer.write_all(v).map_err(Error::Io)
}

impl<T> SerializeRevisioned for Vec<T>
where
	T: SerializeRevisioned,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.len().serialize_revisioned(writer)?;
		for v in self {
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<T> DeserializeRevisioned for Vec<T>
where
	T: DeserializeRevisioned,
{
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let len = usize::deserialize_revisioned(reader)?;
		let mut vec = Vec::with_capacity(len);
		for _ in 0..len {
			let v: T = T::deserialize_revisioned(reader)?;
			// Hint telling the compiler that the push is within capacity.
			if vec.len() >= vec.capacity() {
				unsafe { std::hint::unreachable_unchecked() }
			}
			vec.push(v);
		}
		Ok(vec)
	}
}

impl<T> Revisioned for Vec<T>
where
	T: Revisioned,
{
	#[inline]
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn test_vec() {
		let val: Vec<String> =
			vec![String::from("this"), String::from("is"), String::from("a"), String::from("test")];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out =
			<Vec<String> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_bool() {
		let val = vec![true, false, true, true, false];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_i8() {
		let val = vec![i8::MIN, -1, 0, 1, i8::MAX];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<i8> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_u8() {
		let val = vec![0, 1, 127, 255];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<u8> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_i16() {
		let val = vec![i16::MIN, -1000, 0, 1000, i16::MAX];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<i16> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_u16() {
		let val = vec![0, 1000, 32767, 65535];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<u16> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_i32() {
		let val = vec![i32::MIN, -100000, 0, 100000, i32::MAX];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<i32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_u32() {
		let val = vec![0, 100000, 2147483647, 4294967295];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<u32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_i64() {
		let val = vec![i64::MIN, -1000000000, 0, 1000000000, i64::MAX];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<i64> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_u64() {
		let val = vec![0, 1000000000, 9223372036854775807, 18446744073709551615];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<u64> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_i128() {
		let val = vec![i128::MIN, -1000000000000000000, 0, 1000000000000000000, i128::MAX];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<i128> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_u128() {
		let val = vec![0, 1000000000000000000, u128::MAX / 2, u128::MAX];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<u128> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_f32() {
		let val = vec![f32::MIN, -std::f32::consts::PI, 0.0, std::f32::consts::PI, f32::MAX];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<f32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_f64() {
		let val = vec![f64::MIN, -std::f64::consts::PI, 0.0, std::f64::consts::PI, f64::MAX];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<f64> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_string() {
		let val = vec![
			String::from("hello"),
			String::from("world"),
			String::from(""),
			String::from("longer string with spaces and symbols!@#$%"),
			String::from("unicode: 🚀🔥✨"),
		];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Vec<String> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_empty() {
		// Test empty vectors for all specialized types
		let empty_bool: Vec<bool> = vec![];
		let empty_i8: Vec<i8> = vec![];
		let empty_u8: Vec<u8> = vec![];
		let empty_i32: Vec<i32> = vec![];
		let empty_f64: Vec<f64> = vec![];

		// Test bool
		let mut mem: Vec<u8> = vec![];
		empty_bool.serialize_revisioned(&mut mem).unwrap();
		let out_bool =
			<Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(empty_bool, out_bool);

		// Test i8
		mem.clear();
		empty_i8.serialize_revisioned(&mut mem).unwrap();
		let out_i8 =
			<Vec<i8> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(empty_i8, out_i8);

		// Test u8
		mem.clear();
		empty_u8.serialize_revisioned(&mut mem).unwrap();
		let out_u8 =
			<Vec<u8> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(empty_u8, out_u8);

		// Test i32
		mem.clear();
		empty_i32.serialize_revisioned(&mut mem).unwrap();
		let out_i32 =
			<Vec<i32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(empty_i32, out_i32);

		// Test f64
		mem.clear();
		empty_f64.serialize_revisioned(&mut mem).unwrap();
		let out_f64 =
			<Vec<f64> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(empty_f64, out_f64);
	}

	#[test]
	fn test_vec_large() {
		// Test larger vectors to ensure bulk operations work correctly
		let large_u8: Vec<u8> = (0..=255).collect();
		let mut mem: Vec<u8> = vec![];
		large_u8.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<u8> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(large_u8, out);

		// Test large i32 vector
		let large_i32: Vec<i32> = (0..1000).map(|i| i * 2 - 500).collect();
		mem.clear();
		large_i32.serialize_revisioned(&mut mem).unwrap();
		let out_i32 =
			<Vec<i32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(large_i32, out_i32);

		// Test large bool vector
		let large_bool: Vec<bool> = (0..100).map(|i| i % 2 == 0).collect();
		mem.clear();
		large_bool.serialize_revisioned(&mut mem).unwrap();
		let out_bool =
			<Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(large_bool, out_bool);
	}

	#[test]
	fn test_vec_edge_cases() {
		// Test single element vectors
		let single_u8 = vec![42u8];
		let mut mem: Vec<u8> = vec![];
		single_u8.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<u8> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(single_u8, out);

		// Test bool edge cases (all true, all false)
		let all_true = vec![true; 50];
		mem.clear();
		all_true.serialize_revisioned(&mut mem).unwrap();
		let out_true =
			<Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(all_true, out_true);

		let all_false = vec![false; 50];
		mem.clear();
		all_false.serialize_revisioned(&mut mem).unwrap();
		let out_false =
			<Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(all_false, out_false);

		// Test floating point special values
		let float_specials =
			vec![f64::NEG_INFINITY, f64::MIN, -0.0, 0.0, f64::MAX, f64::INFINITY, f64::NAN];
		mem.clear();
		float_specials.serialize_revisioned(&mut mem).unwrap();
		let out_floats =
			<Vec<f64> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		// Note: NaN != NaN, so we check each element individually
		assert_eq!(out_floats.len(), float_specials.len());
		for (i, (&expected, &actual)) in float_specials.iter().zip(out_floats.iter()).enumerate() {
			if expected.is_nan() {
				assert!(actual.is_nan(), "Element {} should be NaN", i);
			} else {
				assert_eq!(expected, actual, "Element {} mismatch", i);
			}
		}
	}
}
