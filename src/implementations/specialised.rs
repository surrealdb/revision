#![cfg(feature = "specialised-vectors")]

use crate::DeserializeRevisioned;
use crate::Error;
use crate::Revisioned;
use crate::SerializeRevisioned;
use std::io::ErrorKind::UnexpectedEof;
use std::io::{Read, Write};

pub trait SerializeRevisionedSpecialised: Revisioned + SerializeRevisioned {
	/// Serializes the struct using the specficifed `writer`, using specialised serialization.
	fn serialize_revisioned_specialised<W: Write>(&self, w: &mut W) -> Result<(), Error>;
}

pub trait DeserializeRevisionedSpecialised: Revisioned + DeserializeRevisioned {
	/// Deserializes a new instance of the struct from the specified `reader`, using specialised deserialization.
	fn deserialize_revisioned_specialised<R: Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized;
}

// --------------------------------------------------
// Macro for generating optimized Vec<T> implementations for numeric types
// --------------------------------------------------

/// Macro to generate optimized `SerializeRevisioned`, `DeserializeRevisioned`, and `Revisioned`
/// implementations for `Vec<T>` where `T` is a primitive numeric type with a well-defined
/// little-endian byte representation.
///
/// On little-endian platforms, this uses direct memory copy for maximum performance.
/// On big-endian platforms, it falls back to per-element conversion.
macro_rules! impl_revisioned_specialised_vec {
	($ty:ty) => {
		impl SerializeRevisionedSpecialised for Vec<$ty> {
			#[inline]
			fn serialize_revisioned_specialised<W: Write>(
				&self,
				writer: &mut W,
			) -> Result<(), Error> {
				// Get the length once
				let len = self.len();
				// Write the length first
				len.serialize_revisioned(writer)?;
				// For zero-length vectors, return early
				if len == 0 {
					return Ok(());
				}
				// On little-endian platforms, numbers are already in the correct byte
				// order, whilst on big-endian platforms, we need to convert them.
				if cfg!(target_endian = "little") {
					// This is safe because:
					// 1. This type has a well-defined byte representation
					// 2. On little-endian platforms, memory representation matches wire format
					// 3. We're only reading from the slice, not modifying it
					unsafe {
						let byte_slice = std::slice::from_raw_parts(
							self.as_ptr().cast::<u8>(),
							len * std::mem::size_of::<$ty>(),
						);
						writer.write_all(byte_slice).map_err(Error::Io)
					}
				} else {
					// Slow path: per-element little-endian conversion
					for value in self.iter() {
						writer.write_all(&value.to_le_bytes()).map_err(Error::Io)?;
					}
					Ok(())
				}
			}
		}

		impl DeserializeRevisionedSpecialised for Vec<$ty> {
			#[inline]
			fn deserialize_revisioned_specialised<R: Read>(reader: &mut R) -> Result<Self, Error> {
				// Read the length first
				let len = usize::deserialize_revisioned(reader)?;
				// For zero-length vectors, return early
				if len == 0 {
					return Ok(Self::new());
				}
				// On little-endian platforms, numbers are already in the correct byte
				// order, whilst on big-endian platforms, we need to convert them.
				if cfg!(target_endian = "little") {
					// Fast path: bulk read directly into Vec
					let byte_len = len
						.checked_mul(std::mem::size_of::<$ty>())
						.ok_or(Error::IntegerOverflow)?;
					// Allocate initialized buffer to ensure proper alignment and safety
					let mut vec = vec![<$ty>::default(); len];
					// Read the bytes into the vector
					unsafe {
						let byte_slice =
							std::slice::from_raw_parts_mut(vec.as_mut_ptr().cast::<u8>(), byte_len);
						reader.read_exact(byte_slice).map_err(Error::Io)?;
					}
					// Return the vector
					Ok(vec)
				} else {
					// Create a vector with the necessary capacity
					let mut vec = Self::with_capacity(len);
					// Slow path: per-element little-endian conversion
					for _ in 0..len {
						// Read the bytes into a temporary buffer
						let mut b = [0u8; std::mem::size_of::<$ty>()];
						reader.read_exact(&mut b).map_err(Error::Io)?;
						// Convert the bytes to the target type
						let v = <$ty>::from_le_bytes(b);
						// Allow the compiler to optimize away bounds checks
						unsafe { std::hint::assert_unchecked(vec.len() < vec.capacity()) };
						// Push the value to the vector
						vec.push(v);
					}
					Ok(vec)
				}
			}
		}
	};
}

// --------------------------------------------------
// Optimized implementation for Vec<u8>
// --------------------------------------------------

impl SerializeRevisionedSpecialised for Vec<u8> {
	#[inline]
	fn serialize_revisioned_specialised<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Use the optimized serialize_bytes function for Vec<u8>
		super::vecs::serialize_bytes(self, writer)
	}
}

impl DeserializeRevisionedSpecialised for Vec<u8> {
	#[inline]
	fn deserialize_revisioned_specialised<R: Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first
		let len = usize::deserialize_revisioned(reader)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(Self::new());
		}
		// Create the vector
		let mut vec: Vec<u8> = Vec::with_capacity(len);
		// Take the required bytes from the reader
		let mut bytes = reader.take(len as u64);
		// Read the bytes into the vector
		if len != bytes.read_to_end(&mut vec).map_err(Error::Io)? {
			return Err(Error::Io(UnexpectedEof.into()));
		}
		// Return the vector
		Ok(vec)
	}
}

// --------------------------------------------------
// Optimized bulk implementation for Vec<i8>
// --------------------------------------------------

impl SerializeRevisionedSpecialised for Vec<i8> {
	#[inline]
	fn serialize_revisioned_specialised<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Get the length once
		let len = self.len();
		// Write the length first
		len.serialize_revisioned(writer)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(());
		}
		// Since i8 serializes as a single byte (cast to u8), we can do bulk writes
		// Safety: i8 and u8 have the same size and alignment, and we're only reading
		unsafe {
			let byte_slice = std::slice::from_raw_parts(self.as_ptr().cast::<u8>(), self.len());
			writer.write_all(byte_slice).map_err(Error::Io)
		}
	}
}

impl DeserializeRevisionedSpecialised for Vec<i8> {
	#[inline]
	fn deserialize_revisioned_specialised<R: Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first
		let len = usize::deserialize_revisioned(reader)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(Self::new());
		}
		// Create the vector
		let mut vec: Vec<u8> = Vec::with_capacity(len);
		// Take the required bytes from the reader
		let mut bytes = reader.take(len as u64);
		// Read the bytes into the vector
		if len != bytes.read_to_end(&mut vec).map_err(Error::Io)? {
			return Err(Error::Io(UnexpectedEof.into()));
		}
		// Get the Vec<u8> raw parts
		let (ptr, len, cap) = (vec.as_mut_ptr(), vec.len(), vec.capacity());
		// Prevent drop of the Vec<u8>
		std::mem::forget(vec);
		// Convert the Vec<u8> to Vec<i8>
		let vec = unsafe { Vec::from_raw_parts(ptr.cast::<i8>(), len, cap) };
		// Return the vector
		Ok(vec)
	}
}

// --------------------------------------------------
// Bit-packed implementation for Vec<bool>
// --------------------------------------------------

impl SerializeRevisionedSpecialised for Vec<bool> {
	#[inline]
	fn serialize_revisioned_specialised<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Get the length once
		let len = self.len();
		// Write the length first
		len.serialize_revisioned(writer)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(());
		}
		// Pack 8 bools per byte
		let num_bytes = len.div_ceil(8);
		let mut buffer = Vec::with_capacity(num_bytes);
		// Pack the bools into bytes
		for chunk in self.chunks(8) {
			let mut byte = 0u8;
			for (i, &b) in chunk.iter().enumerate() {
				if b {
					byte |= 1 << i;
				}
			}
			buffer.push(byte);
		}
		// Write the buffer to the writer
		writer.write_all(&buffer).map_err(Error::Io)
	}
}

impl DeserializeRevisionedSpecialised for Vec<bool> {
	#[inline]
	fn deserialize_revisioned_specialised<R: Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first
		let len = usize::deserialize_revisioned(reader)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(Self::new());
		}
		// Calculate number of bytes
		let num_bytes = len.div_ceil(8);
		// Read all packed bytes
		let mut buffer = vec![0u8; num_bytes];
		reader.read_exact(&mut buffer).map_err(Error::Io)?;
		// Unpack bits into bools
		let mut vec = Vec::with_capacity(len);
		for (i, &byte) in buffer.iter().enumerate() {
			let bits_in_this_byte = std::cmp::min(8, len - i * 8);
			for bit in 0..bits_in_this_byte {
				vec.push((byte >> bit) & 1 == 1);
			}
		}
		// Return the vector
		Ok(vec)
	}
}

// --------------------------------------------------
// Optimized implementations for Vec<u16>, Vec<u32>, Vec<u64>, Vec<u128>
// --------------------------------------------------

impl_revisioned_specialised_vec!(u16);
impl_revisioned_specialised_vec!(u32);
impl_revisioned_specialised_vec!(u64);
impl_revisioned_specialised_vec!(u128);

// --------------------------------------------------
// Optimized implementations for Vec<i16>, Vec<i32>, Vec<i64>, Vec<i128>
// --------------------------------------------------

impl_revisioned_specialised_vec!(i16);
impl_revisioned_specialised_vec!(i32);
impl_revisioned_specialised_vec!(i64);
impl_revisioned_specialised_vec!(i128);

// --------------------------------------------------
// Optimized implementations for Vec<f32>, Vec<f64>
// --------------------------------------------------

impl_revisioned_specialised_vec!(f32);
impl_revisioned_specialised_vec!(f64);

#[cfg(test)]
mod tests {
	use crate::{DeserializeRevisioned, SerializeRevisioned};

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
	fn test_vec_empty() {
		// Test empty vectors for specialized numeric types
		let empty_i8: Vec<i8> = vec![];
		let empty_u8: Vec<u8> = vec![];
		let empty_i32: Vec<i32> = vec![];
		let empty_f64: Vec<f64> = vec![];

		// Test i8
		let mut mem: Vec<u8> = vec![];
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

	#[test]
	fn test_vec_f32_special_values() {
		// Test f32 special values to ensure optimized path handles them correctly
		let float_specials = vec![
			f32::NEG_INFINITY,
			f32::MIN,
			-0.0f32,
			0.0f32,
			f32::MIN_POSITIVE,
			f32::MAX,
			f32::INFINITY,
			f32::NAN,
		];
		let mut mem: Vec<u8> = vec![];
		float_specials.serialize_revisioned(&mut mem).unwrap();
		let out_floats =
			<Vec<f32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(out_floats.len(), float_specials.len());
		for (i, (&expected, &actual)) in float_specials.iter().zip(out_floats.iter()).enumerate() {
			if expected.is_nan() {
				assert!(actual.is_nan(), "Element {} should be NaN", i);
			} else {
				assert_eq!(expected, actual, "Element {} mismatch", i);
			}
		}
	}

	#[test]
	fn test_vec_i8_bulk() {
		// Test i8 bulk operations
		let val: Vec<i8> = (-128..=127).collect();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		// Length encoding (3 bytes for 256) + 256 bytes of data
		assert_eq!(mem.len(), 3 + 256);
		let out = <Vec<i8> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_bool_bitpacked() {
		// Test basic bit-packing
		let val = vec![true, false, true, true, false, false, true, false];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();

		// Length (1 byte for len=8) + 1 byte of packed data = 2 bytes total
		// Without bit-packing would be 1 + 8 = 9 bytes
		assert_eq!(mem.len(), 2, "Bit-packing should use 2 bytes for 8 bools");

		let out = <Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vec_bool_bitpacked_patterns() {
		// Test all false
		let all_false = vec![false; 100];
		let mut mem: Vec<u8> = vec![];
		all_false.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(all_false, out);

		// Test all true
		let all_true = vec![true; 100];
		mem.clear();
		all_true.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(all_true, out);

		// Test alternating pattern
		let alternating: Vec<bool> = (0..100).map(|i| i % 2 == 0).collect();
		mem.clear();
		alternating.serialize_revisioned(&mut mem).unwrap();
		let out = <Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(alternating, out);
	}

	#[test]
	fn test_vec_bool_bitpacked_sizes() {
		// Test various sizes to ensure partial byte handling works
		for size in [0, 1, 7, 8, 9, 15, 16, 17, 63, 64, 65, 100, 255, 256, 1000] {
			let val: Vec<bool> = (0..size).map(|i| (i * 7) % 3 == 0).collect();
			let mut mem: Vec<u8> = vec![];
			val.serialize_revisioned(&mut mem).unwrap();

			// Verify space savings
			if size > 0 {
				let expected_data_bytes = (size + 7) / 8;
				let len_bytes = if size < 251 {
					1
				} else if size < 65536 {
					3
				} else {
					5
				};
				assert_eq!(
					mem.len(),
					len_bytes + expected_data_bytes,
					"Size mismatch for {} bools",
					size
				);
			}

			let out =
				<Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
					.unwrap();
			assert_eq!(val, out, "Mismatch for size {}", size);
		}
	}

	#[test]
	fn test_vec_bool_bitpacked_empty() {
		let empty: Vec<bool> = vec![];
		let mut mem: Vec<u8> = vec![];
		empty.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 1, "Empty vec should only have length byte");
		let out = <Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(empty, out);
	}

	#[test]
	fn test_vec_bool_bitpacked_space_efficiency() {
		// Demonstrate space savings
		let large_bool_vec = vec![true; 10000];
		let mut mem: Vec<u8> = vec![];
		large_bool_vec.serialize_revisioned(&mut mem).unwrap();

		// With bit-packing: ~1250 bytes (10000/8)
		// Without: 10000 bytes
		// Savings: ~87.5%
		assert!(
			mem.len() < 1300,
			"Bit-packed 10000 bools should be under 1300 bytes, got {}",
			mem.len()
		);

		let out = <Vec<bool> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(large_bool_vec, out);
	}
}
