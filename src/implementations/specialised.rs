#![cfg(feature = "specialised")]

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
				// Write the length first (number of elements)
				self.len().serialize_revisioned(writer)?;
				// For zero-length vectors, return early
				if self.is_empty() {
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
							self.len() * std::mem::size_of::<$ty>(),
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
				// Read the length first (number of elements)
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
					let mut vec = Vec::with_capacity(len);
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
		// Write the length first (number of i8 elements)
		self.len().serialize_revisioned(writer)?;
		// For zero-length vectors, return early
		if self.is_empty() {
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
		// Read the length first (number of i8 elements)
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
