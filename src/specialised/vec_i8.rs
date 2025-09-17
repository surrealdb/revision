//! Specialized implementations for vector data structures.

use crate::{DeserializeRevisioned, Error, Revisioned, SerializeRevisioned};
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};

/// A specialized wrapper for Vec<i8> that provides optimized serialization.
/// Uses direct byte serialization instead of element-by-element serialization.
/// Since i8 serializes to exactly 1 byte per element, we can use bulk operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RevisionSpecialisedVecI8 {
	inner: Vec<i8>,
}

impl RevisionSpecialisedVecI8 {
	/// Create a new empty RevisionSpecialisedVecI8
	#[inline]
	pub fn new() -> Self {
		Self {
			inner: Vec::new(),
		}
	}

	/// Create a RevisionSpecialisedVecI8 with the given capacity
	#[inline]
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			inner: Vec::with_capacity(capacity),
		}
	}

	/// Create a RevisionSpecialisedVecI8 from an existing Vec<i8>
	#[inline]
	pub fn from_vec(vec: Vec<i8>) -> Self {
		Self {
			inner: vec,
		}
	}

	/// Extract the inner Vec<i8>
	#[inline]
	pub fn into_inner(self) -> Vec<i8> {
		self.inner
	}

	/// Get a reference to the inner Vec<i8>
	#[inline]
	pub fn as_inner(&self) -> &Vec<i8> {
		&self.inner
	}

	/// Get a mutable reference to the inner Vec<i8>
	#[inline]
	pub fn as_inner_mut(&mut self) -> &mut Vec<i8> {
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
	pub fn push(&mut self, value: i8) {
		self.inner.push(value);
	}

	/// Pop an element from the vector
	#[inline]
	pub fn pop(&mut self) -> Option<i8> {
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
	pub fn extend<I: IntoIterator<Item = i8>>(&mut self, iter: I) {
		self.inner.extend(iter);
	}

	/// Get a slice of the vector's contents
	#[inline]
	pub fn as_slice(&self) -> &[i8] {
		&self.inner
	}

	/// Get a mutable slice of the vector's contents
	#[inline]
	pub fn as_mut_slice(&mut self) -> &mut [i8] {
		&mut self.inner
	}
}

impl Default for RevisionSpecialisedVecI8 {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}

impl Deref for RevisionSpecialisedVecI8 {
	type Target = Vec<i8>;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl DerefMut for RevisionSpecialisedVecI8 {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

impl From<Vec<i8>> for RevisionSpecialisedVecI8 {
	#[inline]
	fn from(vec: Vec<i8>) -> Self {
		Self::from_vec(vec)
	}
}

impl From<RevisionSpecialisedVecI8> for Vec<i8> {
	#[inline]
	fn from(wrapper: RevisionSpecialisedVecI8) -> Self {
		wrapper.into_inner()
	}
}

impl FromIterator<i8> for RevisionSpecialisedVecI8 {
	#[inline]
	fn from_iter<T: IntoIterator<Item = i8>>(iter: T) -> Self {
		Self {
			inner: Vec::from_iter(iter),
		}
	}
}

impl Extend<i8> for RevisionSpecialisedVecI8 {
	#[inline]
	fn extend<T: IntoIterator<Item = i8>>(&mut self, iter: T) {
		self.inner.extend(iter);
	}
}

impl AsRef<[i8]> for RevisionSpecialisedVecI8 {
	#[inline]
	fn as_ref(&self) -> &[i8] {
		&self.inner
	}
}

impl AsMut<[i8]> for RevisionSpecialisedVecI8 {
	#[inline]
	fn as_mut(&mut self) -> &mut [i8] {
		&mut self.inner
	}
}

impl std::ops::Index<usize> for RevisionSpecialisedVecI8 {
	type Output = i8;

	#[inline]
	fn index(&self, index: usize) -> &Self::Output {
		&self.inner[index]
	}
}

impl std::ops::IndexMut<usize> for RevisionSpecialisedVecI8 {
	#[inline]
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		&mut self.inner[index]
	}
}

impl Revisioned for RevisionSpecialisedVecI8 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for RevisionSpecialisedVecI8 {
	#[inline]
	fn serialize_revisioned<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Write the length first (number of i8 elements)
		self.inner.len().serialize_revisioned(writer)?;
		// For zero-length vectors, return early
		if self.inner.is_empty() {
			return Ok(());
		}
		// Since i8 serializes as a single byte (cast to u8), we can do bulk writes
		// Safety: i8 and u8 have the same size and alignment, and we're only reading
		unsafe {
			let byte_slice =
				std::slice::from_raw_parts(self.inner.as_ptr().cast::<u8>(), self.inner.len());
			writer.write_all(byte_slice).map_err(Error::Io)
		}
	}
}

impl DeserializeRevisioned for RevisionSpecialisedVecI8 {
	#[inline]
	fn deserialize_revisioned<R: Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first (number of i8 elements)
		let len = usize::deserialize_revisioned(reader)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(Self::new());
		}
		// Create a vector with the necessary capacity
		let mut vec = Vec::with_capacity(len);
		// Get safe access to uninitialized memory using spare_capacity_mut()
		let spare = vec.spare_capacity_mut();
		// Safety: Convert MaybeUninit<i8> slice to u8 slice. This is safe because:
		// 1. spare_capacity_mut() provides access to allocated but uninitialized memory
		// 2. MaybeUninit<i8> has the same layout as i8, and i8 has same representation as u8
		// 3. We only set the length after successful read
		let uninit_slice =
			unsafe { std::slice::from_raw_parts_mut(spare.as_mut_ptr().cast::<u8>(), len) };
		// Read the data - this is now safe because spare_capacity_mut() prevents UB
		reader.read_exact(uninit_slice).map_err(Error::Io)?;
		// Only set the length after successful read
		unsafe {
			vec.set_len(len);
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
	fn test_revision_specialised_vec_i8_new() {
		let vec = RevisionSpecialisedVecI8::new();
		assert!(vec.is_empty());
		assert_eq!(vec.len(), 0);
	}

	#[test]
	fn test_revision_specialised_vec_i8_with_capacity() {
		let vec = RevisionSpecialisedVecI8::with_capacity(10);
		assert!(vec.is_empty());
		assert_eq!(vec.len(), 0);
		assert!(vec.capacity() >= 10);
	}

	#[test]
	fn test_revision_specialised_vec_i8_from_vec() {
		let original = vec![1, 2, 3, 4, 5];
		let wrapper = RevisionSpecialisedVecI8::from_vec(original.clone());
		assert_eq!(wrapper.as_slice(), &original);
		assert_eq!(wrapper.len(), 5);
	}

	#[test]
	fn test_revision_specialised_vec_i8_into_inner() {
		let original = vec![1, 2, 3, 4, 5];
		let wrapper = RevisionSpecialisedVecI8::from_vec(original.clone());
		let extracted = wrapper.into_inner();
		assert_eq!(extracted, original);
	}

	#[test]
	fn test_revision_specialised_vec_i8_deref() {
		let mut wrapper = RevisionSpecialisedVecI8::from_vec(vec![1, 2, 3]);
		assert_eq!(wrapper[0], 1);
		assert_eq!(wrapper[1], 2);
		assert_eq!(wrapper[2], 3);

		wrapper[0] = 10;
		assert_eq!(wrapper[0], 10);
	}

	#[test]
	fn test_revision_specialised_vec_i8_push_pop() {
		let mut wrapper = RevisionSpecialisedVecI8::new();
		wrapper.push(42);
		wrapper.push(100);

		assert_eq!(wrapper.len(), 2);
		assert_eq!(wrapper.pop(), Some(100));
		assert_eq!(wrapper.pop(), Some(42));
		assert_eq!(wrapper.pop(), None);
		assert!(wrapper.is_empty());
	}

	#[test]
	fn test_revision_specialised_vec_i8_extend() {
		let mut wrapper = RevisionSpecialisedVecI8::from_vec(vec![1, 2]);
		wrapper.extend(vec![3, 4, 5]);
		assert_eq!(wrapper.as_slice(), &[1, 2, 3, 4, 5]);
	}

	#[test]
	fn test_revision_specialised_vec_i8_from_iterator() {
		let wrapper: RevisionSpecialisedVecI8 = (0..5).collect();
		assert_eq!(wrapper.as_slice(), &[0, 1, 2, 3, 4]);
	}

	#[test]
	fn test_revision_specialised_vec_i8_serialization_empty() {
		let original = RevisionSpecialisedVecI8::new();
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecI8 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert!(deserialized.is_empty());
	}

	#[test]
	fn test_revision_specialised_vec_i8_serialization_single() {
		let original = RevisionSpecialisedVecI8::from_vec(vec![42]);
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecI8 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.len(), 1);
		assert_eq!(deserialized[0], 42);
	}

	#[test]
	fn test_revision_specialised_vec_i8_serialization_multiple() {
		let data = vec![0, 1, 127, -128, -1];
		let original = RevisionSpecialisedVecI8::from_vec(data.clone());
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecI8 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.as_slice(), &data);
	}

	#[test]
	fn test_revision_specialised_vec_i8_serialization_large() {
		let data: Vec<i8> = (-128..=127).collect();
		let original = RevisionSpecialisedVecI8::from_vec(data.clone());
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecI8 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.len(), 256);
		assert_eq!(deserialized.as_slice(), &data);
	}

	#[test]
	fn test_revision_specialised_vec_i8_conversion() {
		let original_vec = vec![1, 2, 3, 4, 5];

		// Vec<i8> -> RevisionSpecialisedVecI8
		let wrapper: RevisionSpecialisedVecI8 = original_vec.clone().into();
		assert_eq!(wrapper.as_slice(), &original_vec);

		// RevisionSpecialisedVecI8 -> Vec<i8>
		let extracted_vec: Vec<i8> = wrapper.into();
		assert_eq!(extracted_vec, original_vec);
	}

	#[test]
	fn test_revision_specialised_vec_i8_as_ref() {
		let wrapper = RevisionSpecialisedVecI8::from_vec(vec![1, 2, 3]);
		let slice: &[i8] = wrapper.as_ref();
		assert_eq!(slice, &[1, 2, 3]);
	}

	#[test]
	fn test_revision_specialised_vec_i8_clear_and_reserve() {
		let mut wrapper = RevisionSpecialisedVecI8::from_vec(vec![1, 2, 3]);
		assert_eq!(wrapper.len(), 3);

		wrapper.clear();
		assert!(wrapper.is_empty());
		assert_eq!(wrapper.len(), 0);

		wrapper.reserve(100);
		assert!(wrapper.capacity() >= 100);
	}

	#[test]
	fn test_revision_specialised_vec_i8_extreme_values() {
		// Test extreme i8 values to ensure serialization/deserialization works correctly
		let extreme_values = vec![i8::MIN, -1, 0, 1, i8::MAX];
		let original = RevisionSpecialisedVecI8::from_vec(extreme_values.clone());
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecI8 = from_slice(&serialized).unwrap();

		assert_eq!(deserialized.len(), extreme_values.len());
		assert_eq!(deserialized.as_slice(), &extreme_values);
	}

	#[test]
	fn test_revision_specialised_vec_i8_negative_values() {
		// Test a range of negative values
		let negative_values: Vec<i8> = (-128..0).collect();
		let original = RevisionSpecialisedVecI8::from_vec(negative_values.clone());
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecI8 = from_slice(&serialized).unwrap();

		assert_eq!(deserialized.len(), negative_values.len());
		assert_eq!(deserialized.as_slice(), &negative_values);
	}

	#[test]
	fn test_consistency_with_regular_vec_i8() {
		// Test that serialized data is compatible between regular Vec<i8> and specialized version
		let test_data = vec![0, 1, -1, 100, -100, i8::MAX, i8::MIN, 42, -42];

		// Serialize with regular Vec<i8>
		let regular_serialized = to_vec(&test_data).unwrap();

		// Serialize with specialized Vec<i8>
		let specialized_data = RevisionSpecialisedVecI8::from_vec(test_data.clone());
		let specialized_serialized = to_vec(&specialized_data).unwrap();

		// They should be identical
		assert_eq!(
			regular_serialized, specialized_serialized,
			"Serialized data should be identical"
		);

		// Cross-compatibility test - deserialize each with the other's format
		let cross_regular: Vec<i8> = from_slice(&specialized_serialized).unwrap();
		let cross_specialized: RevisionSpecialisedVecI8 = from_slice(&regular_serialized).unwrap();

		assert_eq!(cross_regular, test_data);
		assert_eq!(cross_specialized.as_slice(), &test_data);
	}
}
