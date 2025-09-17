//! Specialized implementations for vector data structures.

use crate::implementations::vecs::serialize_bytes;
use crate::{DeserializeRevisioned, Error, Revisioned, SerializeRevisioned};
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};

/// A specialized wrapper for Vec<u8> that provides optimized serialization.
/// Uses direct byte serialization instead of element-by-element serialization.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RevisionSpecialisedVecU8 {
	inner: Vec<u8>,
}

impl RevisionSpecialisedVecU8 {
	/// Create a new empty RevisionSpecialisedVecU8
	#[inline]
	pub fn new() -> Self {
		Self {
			inner: Vec::new(),
		}
	}

	/// Create a RevisionSpecialisedVecU8 with the given capacity
	#[inline]
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			inner: Vec::with_capacity(capacity),
		}
	}

	/// Create a RevisionSpecialisedVecU8 from an existing Vec<u8>
	#[inline]
	pub fn from_vec(vec: Vec<u8>) -> Self {
		Self {
			inner: vec,
		}
	}

	/// Extract the inner Vec<u8>
	#[inline]
	pub fn into_inner(self) -> Vec<u8> {
		self.inner
	}

	/// Get a reference to the inner Vec<u8>
	#[inline]
	pub fn as_inner(&self) -> &Vec<u8> {
		&self.inner
	}

	/// Get a mutable reference to the inner Vec<u8>
	#[inline]
	pub fn as_inner_mut(&mut self) -> &mut Vec<u8> {
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
	pub fn push(&mut self, value: u8) {
		self.inner.push(value);
	}

	/// Pop an element from the vector
	#[inline]
	pub fn pop(&mut self) -> Option<u8> {
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
	pub fn extend<I: IntoIterator<Item = u8>>(&mut self, iter: I) {
		self.inner.extend(iter);
	}

	/// Get a slice of the vector's contents
	#[inline]
	pub fn as_slice(&self) -> &[u8] {
		&self.inner
	}

	/// Get a mutable slice of the vector's contents
	#[inline]
	pub fn as_mut_slice(&mut self) -> &mut [u8] {
		&mut self.inner
	}
}

impl Default for RevisionSpecialisedVecU8 {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}

impl Deref for RevisionSpecialisedVecU8 {
	type Target = Vec<u8>;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl DerefMut for RevisionSpecialisedVecU8 {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

impl From<Vec<u8>> for RevisionSpecialisedVecU8 {
	#[inline]
	fn from(vec: Vec<u8>) -> Self {
		Self::from_vec(vec)
	}
}

impl From<RevisionSpecialisedVecU8> for Vec<u8> {
	#[inline]
	fn from(wrapper: RevisionSpecialisedVecU8) -> Self {
		wrapper.into_inner()
	}
}

impl FromIterator<u8> for RevisionSpecialisedVecU8 {
	#[inline]
	fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> Self {
		Self {
			inner: Vec::from_iter(iter),
		}
	}
}

impl Extend<u8> for RevisionSpecialisedVecU8 {
	#[inline]
	fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
		self.inner.extend(iter);
	}
}

impl AsRef<[u8]> for RevisionSpecialisedVecU8 {
	#[inline]
	fn as_ref(&self) -> &[u8] {
		&self.inner
	}
}

impl AsMut<[u8]> for RevisionSpecialisedVecU8 {
	#[inline]
	fn as_mut(&mut self) -> &mut [u8] {
		&mut self.inner
	}
}

impl std::ops::Index<usize> for RevisionSpecialisedVecU8 {
	type Output = u8;

	#[inline]
	fn index(&self, index: usize) -> &Self::Output {
		&self.inner[index]
	}
}

impl std::ops::IndexMut<usize> for RevisionSpecialisedVecU8 {
	#[inline]
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		&mut self.inner[index]
	}
}

impl Revisioned for RevisionSpecialisedVecU8 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for RevisionSpecialisedVecU8 {
	#[inline]
	fn serialize_revisioned<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Use the optimized serialize_bytes function for Vec<u8>
		serialize_bytes(&self.inner, writer)
	}
}

impl DeserializeRevisioned for RevisionSpecialisedVecU8 {
	#[inline]
	fn deserialize_revisioned<R: Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first
		let len = usize::deserialize_revisioned(reader)?;
		// For zero-length vectors, return early
		if len == 0 {
			return Ok(Self::new());
		}
		// Create a vector with the necessary capacity
		let mut vec = Vec::with_capacity(len);
		// Get safe access to uninitialized memory using spare_capacity_mut()
		let spare = vec.spare_capacity_mut();
		// Safety: Convert MaybeUninit<u8> slice to u8 slice. This is safe because:
		// 1. spare_capacity_mut() provides access to allocated but uninitialized memory
		// 2. MaybeUninit<u8> has the same layout as u8
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
	fn test_revision_specialised_vec_u8_new() {
		let vec = RevisionSpecialisedVecU8::new();
		assert!(vec.is_empty());
		assert_eq!(vec.len(), 0);
	}

	#[test]
	fn test_revision_specialised_vec_u8_with_capacity() {
		let vec = RevisionSpecialisedVecU8::with_capacity(10);
		assert!(vec.is_empty());
		assert_eq!(vec.len(), 0);
		assert!(vec.capacity() >= 10);
	}

	#[test]
	fn test_revision_specialised_vec_u8_from_vec() {
		let original = vec![1, 2, 3, 4, 5];
		let wrapper = RevisionSpecialisedVecU8::from_vec(original.clone());
		assert_eq!(wrapper.as_slice(), &original);
		assert_eq!(wrapper.len(), 5);
	}

	#[test]
	fn test_revision_specialised_vec_u8_into_inner() {
		let original = vec![1, 2, 3, 4, 5];
		let wrapper = RevisionSpecialisedVecU8::from_vec(original.clone());
		let extracted = wrapper.into_inner();
		assert_eq!(extracted, original);
	}

	#[test]
	fn test_revision_specialised_vec_u8_deref() {
		let mut wrapper = RevisionSpecialisedVecU8::from_vec(vec![1, 2, 3]);
		assert_eq!(wrapper[0], 1);
		assert_eq!(wrapper[1], 2);
		assert_eq!(wrapper[2], 3);

		wrapper[0] = 10;
		assert_eq!(wrapper[0], 10);
	}

	#[test]
	fn test_revision_specialised_vec_u8_push_pop() {
		let mut wrapper = RevisionSpecialisedVecU8::new();
		wrapper.push(42);
		wrapper.push(100);

		assert_eq!(wrapper.len(), 2);
		assert_eq!(wrapper.pop(), Some(100));
		assert_eq!(wrapper.pop(), Some(42));
		assert_eq!(wrapper.pop(), None);
		assert!(wrapper.is_empty());
	}

	#[test]
	fn test_revision_specialised_vec_u8_extend() {
		let mut wrapper = RevisionSpecialisedVecU8::from_vec(vec![1, 2]);
		wrapper.extend(vec![3, 4, 5]);
		assert_eq!(wrapper.as_slice(), &[1, 2, 3, 4, 5]);
	}

	#[test]
	fn test_revision_specialised_vec_u8_from_iterator() {
		let wrapper: RevisionSpecialisedVecU8 = (0..5).collect();
		assert_eq!(wrapper.as_slice(), &[0, 1, 2, 3, 4]);
	}

	#[test]
	fn test_revision_specialised_vec_u8_serialization_empty() {
		let original = RevisionSpecialisedVecU8::new();
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecU8 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert!(deserialized.is_empty());
	}

	#[test]
	fn test_revision_specialised_vec_u8_serialization_single() {
		let original = RevisionSpecialisedVecU8::from_vec(vec![42]);
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecU8 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.len(), 1);
		assert_eq!(deserialized[0], 42);
	}

	#[test]
	fn test_revision_specialised_vec_u8_serialization_multiple() {
		let data = vec![0, 1, 127, 128, 255];
		let original = RevisionSpecialisedVecU8::from_vec(data.clone());
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecU8 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.as_slice(), &data);
	}

	#[test]
	fn test_revision_specialised_vec_u8_serialization_large() {
		let data: Vec<u8> = (0..=255).collect();
		let original = RevisionSpecialisedVecU8::from_vec(data.clone());
		let serialized = to_vec(&original).unwrap();
		let deserialized: RevisionSpecialisedVecU8 = from_slice(&serialized).unwrap();
		assert_eq!(original, deserialized);
		assert_eq!(deserialized.len(), 256);
		assert_eq!(deserialized.as_slice(), &data);
	}

	#[test]
	fn test_revision_specialised_vec_u8_conversion() {
		let original_vec = vec![1, 2, 3, 4, 5];

		// Vec<u8> -> RevisionSpecialisedVecU8
		let wrapper: RevisionSpecialisedVecU8 = original_vec.clone().into();
		assert_eq!(wrapper.as_slice(), &original_vec);

		// RevisionSpecialisedVecU8 -> Vec<u8>
		let extracted_vec: Vec<u8> = wrapper.into();
		assert_eq!(extracted_vec, original_vec);
	}

	#[test]
	fn test_revision_specialised_vec_u8_as_ref() {
		let wrapper = RevisionSpecialisedVecU8::from_vec(vec![1, 2, 3]);
		let slice: &[u8] = wrapper.as_ref();
		assert_eq!(slice, &[1, 2, 3]);
	}

	#[test]
	fn test_revision_specialised_vec_u8_clear_and_reserve() {
		let mut wrapper = RevisionSpecialisedVecU8::from_vec(vec![1, 2, 3]);
		assert_eq!(wrapper.len(), 3);

		wrapper.clear();
		assert!(wrapper.is_empty());
		assert_eq!(wrapper.len(), 0);

		wrapper.reserve(100);
		assert!(wrapper.capacity() >= 100);
	}
}
