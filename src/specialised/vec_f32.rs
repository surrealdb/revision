//! Specialized implementations for vector data structures.

use crate::{DeserializeRevisioned, Error, Revisioned, SerializeRevisioned};
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};

/// A specialized wrapper for Vec<f32> that provides optimized serialization.
/// Uses direct byte serialization instead of element-by-element serialization.
#[derive(Debug, Clone, PartialEq)]
pub struct RevisionSpecialisedVecF32 {
	inner: Vec<f32>,
}

impl RevisionSpecialisedVecF32 {
	/// Create a new empty RevisionSpecialisedVecF32
	#[inline]
	pub fn new() -> Self {
		Self {
			inner: Vec::new(),
		}
	}

	/// Create a RevisionSpecialisedVecF32 with the given capacity
	#[inline]
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			inner: Vec::with_capacity(capacity),
		}
	}

	/// Create a RevisionSpecialisedVecF32 from an existing Vec<f32>
	#[inline]
	pub fn from_vec(vec: Vec<f32>) -> Self {
		Self {
			inner: vec,
		}
	}

	/// Extract the inner Vec<f32>
	#[inline]
	pub fn into_inner(self) -> Vec<f32> {
		self.inner
	}

	/// Get a reference to the inner Vec<f32>
	#[inline]
	pub fn as_inner(&self) -> &Vec<f32> {
		&self.inner
	}

	/// Get a mutable reference to the inner Vec<f32>
	#[inline]
	pub fn as_inner_mut(&mut self) -> &mut Vec<f32> {
		&mut self.inner
	}

	/// Get the length of the vector
	#[inline]
	pub fn len(&self) -> usize {
		self.inner.len()
	}

	/// Check if the vector is empty
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	/// Get the capacity of the vector
	#[inline]
	pub fn capacity(&self) -> usize {
		self.inner.capacity()
	}

	/// Push an element to the vector
	#[inline]
	pub fn push(&mut self, value: f32) {
		self.inner.push(value);
	}

	/// Pop an element from the vector
	#[inline]
	pub fn pop(&mut self) -> Option<f32> {
		self.inner.pop()
	}

	/// Clear the vector
	#[inline]
	pub fn clear(&mut self) {
		self.inner.clear();
	}

	/// Reserve capacity for at least `additional` more elements
	#[inline]
	pub fn reserve(&mut self, additional: usize) {
		self.inner.reserve(additional);
	}

	/// Shrink the vector to fit its contents
	#[inline]
	pub fn shrink_to_fit(&mut self) {
		self.inner.shrink_to_fit();
	}

	/// Extend the vector with the contents of an iterator
	#[inline]
	pub fn extend<I: IntoIterator<Item = f32>>(&mut self, iter: I) {
		self.inner.extend(iter);
	}

	/// Get a slice of the vector's contents
	#[inline]
	pub fn as_slice(&self) -> &[f32] {
		&self.inner
	}

	/// Get a mutable slice of the vector's contents
	#[inline]
	pub fn as_mut_slice(&mut self) -> &mut [f32] {
		&mut self.inner
	}
}

impl Default for RevisionSpecialisedVecF32 {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}

impl Deref for RevisionSpecialisedVecF32 {
	type Target = Vec<f32>;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl DerefMut for RevisionSpecialisedVecF32 {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

impl From<Vec<f32>> for RevisionSpecialisedVecF32 {
	#[inline]
	fn from(vec: Vec<f32>) -> Self {
		Self::from_vec(vec)
	}
}

impl From<RevisionSpecialisedVecF32> for Vec<f32> {
	#[inline]
	fn from(wrapper: RevisionSpecialisedVecF32) -> Self {
		wrapper.into_inner()
	}
}

impl FromIterator<f32> for RevisionSpecialisedVecF32 {
	#[inline]
	fn from_iter<T: IntoIterator<Item = f32>>(iter: T) -> Self {
		Self {
			inner: Vec::from_iter(iter),
		}
	}
}

impl Extend<f32> for RevisionSpecialisedVecF32 {
	#[inline]
	fn extend<T: IntoIterator<Item = f32>>(&mut self, iter: T) {
		self.inner.extend(iter);
	}
}

impl AsRef<[f32]> for RevisionSpecialisedVecF32 {
	#[inline]
	fn as_ref(&self) -> &[f32] {
		&self.inner
	}
}

impl AsMut<[f32]> for RevisionSpecialisedVecF32 {
	#[inline]
	fn as_mut(&mut self) -> &mut [f32] {
		&mut self.inner
	}
}

impl std::ops::Index<usize> for RevisionSpecialisedVecF32 {
	type Output = f32;

	#[inline]
	fn index(&self, index: usize) -> &Self::Output {
		&self.inner[index]
	}
}

impl std::ops::IndexMut<usize> for RevisionSpecialisedVecF32 {
	#[inline]
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		&mut self.inner[index]
	}
}

impl Revisioned for RevisionSpecialisedVecF32 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for RevisionSpecialisedVecF32 {
	#[inline]
	fn serialize_revisioned<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Write the length first (number of f32 elements)
		self.inner.len().serialize_revisioned(writer)?;
		// For zero-length vectors, return early
		if self.inner.is_empty() {
			return Ok(());
		}
		// On little-endian platforms, f32 values are already in the correct
		// byte order, whilst on big-endian platforms, we need to convert them
		if cfg!(target_endian = "little") {
			// Fast path for little-endian platforms: direct byte copy
			// This is safe because:
			// 1. f32 has a well-defined byte representation
			// 2. On little-endian platforms, memory representation matches wire format
			// 3. We're only reading from the slice, not modifying it
			unsafe {
				let byte_slice = std::slice::from_raw_parts(
					self.inner.as_ptr() as *const u8,
					self.inner.len() * std::mem::size_of::<f32>(),
				);
				writer.write_all(byte_slice).map_err(Error::Io)
			}
		} else {
			// Slower path for big-endian platforms: convert each f32 to little-endian bytes
			for &value in &self.inner {
				let bytes = value.to_le_bytes();
				writer.write_all(&bytes).map_err(Error::Io)?;
			}
			Ok(())
		}
	}
}

impl DeserializeRevisioned for RevisionSpecialisedVecF32 {
	#[inline]
	fn deserialize_revisioned<R: Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first (number of f32 elements)
		let len = usize::deserialize_revisioned(reader)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(Self::new());
		}
		// Create a vector with the necessary capacity
		let mut vec = Vec::with_capacity(len);
		// On little-endian platforms, f32 values are already in the correct
		// byte order, whilst on big-endian platforms, we need to convert them
		if cfg!(target_endian = "little") {
			// Fast path for little-endian platforms: direct byte read
			let byte_len = len * std::mem::size_of::<f32>();
			// Safety: We read directly into uninitialized memory and only set the length
			// after the read is successful. If read_exact fails, the vector length remains 0.
			unsafe {
				let byte_slice =
					std::slice::from_raw_parts_mut(vec.as_mut_ptr() as *mut u8, byte_len);
				// Read the data directly into uninitialized memory
				reader.read_exact(byte_slice).map_err(Error::Io)?;
				// Only set the length after successful read
				vec.set_len(len);
			}
		} else {
			// Slower path for big-endian platforms: read and convert each f32
			for _ in 0..len {
				let mut bytes = [0u8; 4];
				reader.read_exact(&mut bytes).map_err(Error::Io)?;
				let value = f32::from_le_bytes(bytes);
				vec.push(value);
			}
		}
		// Return the specialized vector
		Ok(Self {
			inner: vec,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{from_slice, to_vec};

	#[test]
	fn test_revision_specialised_vec_f32_new() {
		let vec = RevisionSpecialisedVecF32::new();
		assert!(vec.is_empty());
		assert_eq!(vec.len(), 0);
	}

	#[test]
	fn test_revision_specialised_vec_f32_with_capacity() {
		let vec = RevisionSpecialisedVecF32::with_capacity(10);
		assert!(vec.is_empty());
		assert_eq!(vec.len(), 0);
		assert!(vec.capacity() >= 10);
	}

	#[test]
	fn test_revision_specialised_vec_f32_from_vec() {
		let original = vec![1.0, 2.0, 3.0, 4.0, 5.0];
		let wrapper = RevisionSpecialisedVecF32::from_vec(original.clone());
		assert_eq!(wrapper.as_slice(), &original);
		assert_eq!(wrapper.len(), 5);
	}

	#[test]
	fn test_revision_specialised_vec_f32_into_inner() {
		let original = vec![1.0, 2.0, 3.0, 4.0, 5.0];
		let wrapper = RevisionSpecialisedVecF32::from_vec(original.clone());
		let extracted = wrapper.into_inner();
		assert_eq!(extracted, original);
	}

	#[test]
	fn test_revision_specialised_vec_f32_deref() {
		let mut wrapper = RevisionSpecialisedVecF32::from_vec(vec![1.0, 2.0, 3.0]);
		assert_eq!(wrapper[0], 1.0);
		assert_eq!(wrapper[1], 2.0);
		assert_eq!(wrapper[2], 3.0);

		wrapper[0] = 10.0;
		assert_eq!(wrapper[0], 10.0);
	}

	#[test]
	fn test_revision_specialised_vec_f32_push_pop() {
		let mut wrapper = RevisionSpecialisedVecF32::new();
		wrapper.push(42.0);
		wrapper.push(100.0);

		assert_eq!(wrapper.len(), 2);
		assert_eq!(wrapper.pop(), Some(100.0));
		assert_eq!(wrapper.pop(), Some(42.0));
		assert_eq!(wrapper.pop(), None);
		assert!(wrapper.is_empty());
	}

	#[test]
	fn test_revision_specialised_vec_f32_extend() {
		let mut wrapper = RevisionSpecialisedVecF32::from_vec(vec![1.0, 2.0]);
		wrapper.extend(vec![3.0, 4.0, 5.0]);
		assert_eq!(wrapper.as_slice(), &[1.0, 2.0, 3.0, 4.0, 5.0]);
	}

	#[test]
	fn test_revision_specialised_vec_f32_from_iterator() {
		let wrapper: RevisionSpecialisedVecF32 = (0..5).map(|i| i as f32).collect();
		assert_eq!(wrapper.as_slice(), &[0.0, 1.0, 2.0, 3.0, 4.0]);
	}

	#[test]
	fn test_revision_specialised_vec_f32_serialization_empty() {
		let original = RevisionSpecialisedVecF32::new();
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecF32 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert!(deserialized.is_empty());
	}

	#[test]
	fn test_revision_specialised_vec_f32_serialization_single() {
		let original = RevisionSpecialisedVecF32::from_vec(vec![42.0]);
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecF32 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.len(), 1);
		assert_eq!(deserialized[0], 42.0);
	}

	#[test]
	fn test_revision_specialised_vec_f32_serialization_multiple() {
		let data = vec![0.0, 1.0, 127.0, 128.0, 255.0];
		let original = RevisionSpecialisedVecF32::from_vec(data.clone());
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecF32 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.as_slice(), &data);
	}

	#[test]
	fn test_revision_specialised_vec_f32_serialization_large() {
		let data: Vec<f32> = (0..=255).map(|i| i as f32).collect();
		let original = RevisionSpecialisedVecF32::from_vec(data.clone());
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecF32 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.len(), 256);
		assert_eq!(deserialized.as_slice(), &data);
	}

	#[test]
	fn test_revision_specialised_vec_f32_conversion() {
		let original_vec = vec![1.0, 2.0, 3.0, 4.0, 5.0];

		// Vec<f32> -> RevisionSpecialisedVecF32
		let wrapper: RevisionSpecialisedVecF32 = original_vec.clone().into();
		assert_eq!(wrapper.as_slice(), &original_vec);

		// RevisionSpecialisedVecF32 -> Vec<f32>
		let extracted_vec: Vec<f32> = wrapper.into();
		assert_eq!(extracted_vec, original_vec);
	}

	#[test]
	fn test_revision_specialised_vec_f32_as_ref() {
		let wrapper = RevisionSpecialisedVecF32::from_vec(vec![1.0, 2.0, 3.0]);
		let slice: &[f32] = wrapper.as_ref();
		assert_eq!(slice, &[1.0, 2.0, 3.0]);
	}

	#[test]
	fn test_revision_specialised_vec_f32_clear_and_reserve() {
		let mut wrapper = RevisionSpecialisedVecF32::from_vec(vec![1.0, 2.0, 3.0]);
		assert_eq!(wrapper.len(), 3);

		wrapper.clear();
		assert!(wrapper.is_empty());
		assert_eq!(wrapper.len(), 0);

		wrapper.reserve(100);
		assert!(wrapper.capacity() >= 100);
	}

	#[test]
	fn test_revision_specialised_vec_f32_special_values() {
		// Test that special f32 values serialize/deserialize correctly with byte-level optimization
		let special_values = vec![
			f32::NEG_INFINITY,
			f32::MIN,
			-0.0,
			0.0,
			f32::MIN_POSITIVE,
			f32::MAX,
			f32::INFINITY,
			f32::NAN,
		];
		let original = RevisionSpecialisedVecF32::from_vec(special_values.clone());
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecF32 = from_slice(&serialized).unwrap();

		assert_eq!(deserialized.len(), special_values.len());
		// Check each value individually since NaN != NaN
		for (i, (&expected, &actual)) in
			special_values.iter().zip(deserialized.as_slice().iter()).enumerate()
		{
			if expected.is_nan() {
				assert!(actual.is_nan(), "Element {} should be NaN", i);
			} else {
				assert_eq!(expected, actual, "Element {} mismatch", i);
				// Also check that the bit patterns are identical for special values
				assert_eq!(
					expected.to_bits(),
					actual.to_bits(),
					"Bit pattern mismatch for element {}",
					i
				);
			}
		}
	}
}
