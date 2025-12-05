use crate::DeserializeRevisioned;
use crate::Error;
use crate::Revisioned;
use crate::SerializeRevisioned;
use std::any::TypeId;
use std::io::Write;

pub(crate) fn serialize_bytes<W>(v: &[u8], writer: &mut W) -> Result<(), Error>
where
	W: Write,
{
	v.len().serialize_revisioned(writer)?;
	writer.write_all(v).map_err(Error::Io)
}

impl<T> SerializeRevisioned for Vec<T>
where
	T: SerializeRevisioned + 'static,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Try specialized implementations based on TypeId (when feature enabled)
		#[cfg(feature = "specialised")]
		{
			macro_rules! try_specialized {
				($ty:ty) => {
					if TypeId::of::<T>() == TypeId::of::<$ty>() {
						use crate::implementations::specialised::SerializeRevisionedSpecialised;
						let specialized = unsafe { &*(self as *const Vec<T> as *const Vec<$ty>) };
						return SerializeRevisionedSpecialised::serialize_revisioned_specialised(
							specialized,
							writer,
						);
					}
				};
			}

			try_specialized!(u8);
			try_specialized!(i8);
			try_specialized!(u16);
			try_specialized!(i16);
			try_specialized!(u32);
			try_specialized!(i32);
			try_specialized!(u64);
			try_specialized!(i64);
			try_specialized!(u128);
			try_specialized!(i128);
			try_specialized!(f32);
			try_specialized!(f64);
			#[cfg(feature = "rust_decimal")]
			try_specialized!(rust_decimal::Decimal);
			#[cfg(feature = "uuid")]
			try_specialized!(uuid::Uuid);
		}

		// Generic fallback

		// Get the length once
		let len = self.len();
		// Write the length first
		len.serialize_revisioned(writer)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(());
		}
		// Slow path: per-element serialization
		for v in self {
			// Serialize the value
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<T> DeserializeRevisioned for Vec<T>
where
	T: DeserializeRevisioned + 'static,
{
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		// Try specialized implementations based on TypeId (when feature enabled)
		#[cfg(feature = "specialised")]
		{
			macro_rules! try_specialized {
				($ty:ty) => {
					if TypeId::of::<T>() == TypeId::of::<$ty>() {
						use crate::implementations::specialised::DeserializeRevisionedSpecialised;
						return Vec::<$ty>::deserialize_revisioned_specialised(reader)
							.map(|v| unsafe { std::mem::transmute(v) });
					}
				};
			}

			try_specialized!(u8);
			try_specialized!(i8);
			try_specialized!(u16);
			try_specialized!(i16);
			try_specialized!(u32);
			try_specialized!(i32);
			try_specialized!(u64);
			try_specialized!(i64);
			try_specialized!(u128);
			try_specialized!(i128);
			try_specialized!(f32);
			try_specialized!(f64);
			#[cfg(feature = "rust_decimal")]
			try_specialized!(rust_decimal::Decimal);
			#[cfg(feature = "uuid")]
			try_specialized!(uuid::Uuid);
		}

		// Generic fallback

		// Read the length first
		let len = usize::deserialize_revisioned(reader)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(Self::new());
		}
		// Create a vector with the necessary capacity
		let mut vec = Self::with_capacity(len);
		// Slow path: per-element deserialization
		for _ in 0..len {
			// Deserialize the value
			let v = T::deserialize_revisioned(reader)?;
			// Allow the compiler to optimize away bounds checks
			unsafe { std::hint::assert_unchecked(vec.len() < vec.capacity()) };
			// Push the value to the vector
			vec.push(v);
		}
		Ok(vec)
	}
}

impl<T> Revisioned for Vec<T> {
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
		// Test empty vectors for generic types
		let empty_bool: Vec<bool> = vec![];
		let empty_string: Vec<String> = vec![];

		// Test bool
		let mut mem: Vec<u8> = vec![];
		empty_bool.serialize_revisioned(&mut mem).unwrap();
		let out_bool =
			<Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(empty_bool, out_bool);

		// Test String
		mem.clear();
		empty_string.serialize_revisioned(&mut mem).unwrap();
		let out_string =
			<Vec<String> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(empty_string, out_string);
	}

	#[test]
	fn test_vec_large() {
		// Test large bool vector
		let large_bool: Vec<bool> = (0..100).map(|i| i % 2 == 0).collect();
		let mut mem: Vec<u8> = vec![];
		large_bool.serialize_revisioned(&mut mem).unwrap();
		let out_bool =
			<Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(large_bool, out_bool);
	}

	#[test]
	fn test_vec_edge_cases() {
		// Test bool edge cases (all true, all false)
		let all_true = vec![true; 50];
		let mut mem: Vec<u8> = vec![];
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
	}
}
