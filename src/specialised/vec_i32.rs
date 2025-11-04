//! Specialized implementations for vector data structures (i32).

use crate::{DeserializeRevisioned, Error, Revisioned, SerializeRevisioned};
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};

/// A specialized wrapper for Vec<i32> that provides optimized serialization.
/// Uses fixed-width 4-byte little-endian packing for maximum speed and predictable size.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RevisionSpecialisedVecI32 {
	inner: Vec<i32>,
}

impl RevisionSpecialisedVecI32 {
	/// Create a new empty RevisionSpecialisedVecI32
	#[inline]
	pub fn new() -> Self {
		Self {
			inner: Vec::new(),
		}
	}

	/// Create a RevisionSpecialisedVecI32 with the given capacity
	#[inline]
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			inner: Vec::with_capacity(capacity),
		}
	}

	/// Create a RevisionSpecialisedVecI32 from an existing Vec<i32>
	#[inline]
	pub fn from_vec(vec: Vec<i32>) -> Self {
		Self {
			inner: vec,
		}
	}

	/// Extract the inner Vec<i32>
	#[inline]
	pub fn into_inner(self) -> Vec<i32> {
		self.inner
	}

	/// Get a reference to the inner Vec<i32>
	#[inline]
	pub fn as_inner(&self) -> &Vec<i32> {
		&self.inner
	}

	/// Get a mutable reference to the inner Vec<i32>
	#[inline]
	pub fn as_inner_mut(&mut self) -> &mut Vec<i32> {
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
	pub fn push(&mut self, value: i32) {
		self.inner.push(value);
	}

	/// Pop an element from the vector
	#[inline]
	pub fn pop(&mut self) -> Option<i32> {
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
	pub fn extend<I: IntoIterator<Item = i32>>(&mut self, iter: I) {
		self.inner.extend(iter);
	}

	/// Get a slice of the vector's contents
	#[inline]
	pub fn as_slice(&self) -> &[i32] {
		&self.inner
	}

	/// Get a mutable slice of the vector's contents
	#[inline]
	pub fn as_mut_slice(&mut self) -> &mut [i32] {
		&mut self.inner
	}
}

impl Default for RevisionSpecialisedVecI32 {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}

impl Deref for RevisionSpecialisedVecI32 {
	type Target = Vec<i32>;
	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl DerefMut for RevisionSpecialisedVecI32 {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

impl From<Vec<i32>> for RevisionSpecialisedVecI32 {
	#[inline]
	fn from(vec: Vec<i32>) -> Self {
		Self::from_vec(vec)
	}
}

impl From<RevisionSpecialisedVecI32> for Vec<i32> {
	#[inline]
	fn from(wrapper: RevisionSpecialisedVecI32) -> Self {
		wrapper.into_inner()
	}
}

impl FromIterator<i32> for RevisionSpecialisedVecI32 {
	#[inline]
	fn from_iter<T: IntoIterator<Item = i32>>(iter: T) -> Self {
		Self {
			inner: Vec::from_iter(iter),
		}
	}
}

impl Extend<i32> for RevisionSpecialisedVecI32 {
	#[inline]
	fn extend<T: IntoIterator<Item = i32>>(&mut self, iter: T) {
		self.inner.extend(iter);
	}
}

impl AsRef<[i32]> for RevisionSpecialisedVecI32 {
	#[inline]
	fn as_ref(&self) -> &[i32] {
		&self.inner
	}
}

impl AsMut<[i32]> for RevisionSpecialisedVecI32 {
	#[inline]
	fn as_mut(&mut self) -> &mut [i32] {
		&mut self.inner
	}
}

impl std::ops::Index<usize> for RevisionSpecialisedVecI32 {
	type Output = i32;
	#[inline]
	fn index(&self, index: usize) -> &Self::Output {
		&self.inner[index]
	}
}

impl std::ops::IndexMut<usize> for RevisionSpecialisedVecI32 {
	#[inline]
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		&mut self.inner[index]
	}
}

impl Revisioned for RevisionSpecialisedVecI32 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for RevisionSpecialisedVecI32 {
	#[inline]
	fn serialize_revisioned<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Write the length first (number of i32 elements)
		self.inner.len().serialize_revisioned(writer)?;
		// For zero-length vectors, return early
		if self.inner.is_empty() {
			return Ok(());
		}
		// On little-endian platforms we can do a direct bulk copy of bytes.
		if cfg!(target_endian = "little") {
			unsafe {
				let byte_slice = std::slice::from_raw_parts(
					self.inner.as_ptr().cast::<u8>(),
					self.inner.len() * std::mem::size_of::<i32>(),
				);
				writer.write_all(byte_slice).map_err(Error::Io)
			}
		} else {
			for &value in &self.inner {
				let bytes = value.to_le_bytes();
				writer.write_all(&bytes).map_err(Error::Io)?;
			}
			Ok(())
		}
	}
}

impl DeserializeRevisioned for RevisionSpecialisedVecI32 {
	#[inline]
	fn deserialize_revisioned<R: Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first (number of i32 elements)
		let len = usize::deserialize_revisioned(reader)?;
		if len == 0 {
			return Ok(Self::new());
		}
		if cfg!(target_endian = "little") {
			let byte_len =
				len.checked_mul(std::mem::size_of::<i32>()).ok_or(Error::IntegerOverflow)?;
			// Allocate initialized i32 buffer to ensure proper alignment and safety
			let mut vec_i32 = vec![0i32; len];
			unsafe {
				let byte_slice =
					std::slice::from_raw_parts_mut(vec_i32.as_mut_ptr().cast::<u8>(), byte_len);
				reader.read_exact(byte_slice).map_err(Error::Io)?;
			}
			Ok(Self {
				inner: vec_i32,
			})
		} else {
			let mut vec = Vec::with_capacity(len);
			for _ in 0..len {
				let mut b = [0u8; 4];
				reader.read_exact(&mut b).map_err(Error::Io)?;
				let v = i32::from_le_bytes(b);
				// Hint telling the compiler that the push is within capacity.
				if vec.len() >= vec.capacity() {
					unsafe { std::hint::unreachable_unchecked() }
				}
				vec.push(v);
			}
			Ok(Self {
				inner: vec,
			})
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{from_slice, to_vec};

	#[test]
	fn test_revision_specialised_vec_i32_new() {
		let vec = RevisionSpecialisedVecI32::new();
		assert!(vec.is_empty());
		assert_eq!(vec.len(), 0);
	}

	#[test]
	fn test_revision_specialised_vec_i32_with_capacity() {
		let vec = RevisionSpecialisedVecI32::with_capacity(10);
		assert!(vec.is_empty());
		assert_eq!(vec.len(), 0);
		assert!(vec.capacity() >= 10);
	}

	#[test]
	fn test_revision_specialised_vec_i32_from_vec() {
		let original = vec![1i32, 2, 3, 4, 5];
		let wrapper = RevisionSpecialisedVecI32::from_vec(original.clone());
		assert_eq!(wrapper.as_slice(), &original);
		assert_eq!(wrapper.len(), 5);
	}

	#[test]
	fn test_revision_specialised_vec_i32_into_inner() {
		let original = vec![1i32, 2, 3, 4, 5];
		let wrapper = RevisionSpecialisedVecI32::from_vec(original.clone());
		let extracted = wrapper.into_inner();
		assert_eq!(extracted, original);
	}

	#[test]
	fn test_revision_specialised_vec_i32_deref() {
		let mut wrapper = RevisionSpecialisedVecI32::from_vec(vec![1i32, 2, 3]);
		assert_eq!(wrapper[0], 1);
		assert_eq!(wrapper[1], 2);
		assert_eq!(wrapper[2], 3);
		wrapper[0] = 10;
		assert_eq!(wrapper[0], 10);
	}

	#[test]
	fn test_revision_specialised_vec_i32_push_pop() {
		let mut wrapper = RevisionSpecialisedVecI32::new();
		wrapper.push(42);
		wrapper.push(100);
		assert_eq!(wrapper.len(), 2);
		assert_eq!(wrapper.pop(), Some(100));
		assert_eq!(wrapper.pop(), Some(42));
		assert_eq!(wrapper.pop(), None);
		assert!(wrapper.is_empty());
	}

	#[test]
	fn test_revision_specialised_vec_i32_extend() {
		let mut wrapper = RevisionSpecialisedVecI32::from_vec(vec![1, 2]);
		wrapper.extend(vec![3, 4]);
		assert_eq!(wrapper.as_slice(), &[1, 2, 3, 4]);
	}

	#[test]
	fn test_revision_specialised_vec_i32_from_iterator() {
		let wrapper: RevisionSpecialisedVecI32 = vec![1i32, 2, 3, 4].into_iter().collect();
		assert_eq!(wrapper.as_slice(), &[1, 2, 3, 4]);
	}

	#[test]
	fn test_revision_specialised_vec_i32_serialization_empty() {
		let wrapper = RevisionSpecialisedVecI32::new();
		let bytes = to_vec(&wrapper).unwrap();
		let out: RevisionSpecialisedVecI32 = from_slice(&bytes).unwrap();
		assert_eq!(out.as_slice(), &[]);
	}

	#[test]
	fn test_revision_specialised_vec_i32_serialization_single() {
		let wrapper = RevisionSpecialisedVecI32::from_vec(vec![123456789]);
		let bytes = to_vec(&wrapper).unwrap();
		let out: RevisionSpecialisedVecI32 = from_slice(&bytes).unwrap();
		assert_eq!(out.as_slice(), &[123456789]);
	}

	#[test]
	fn test_revision_specialised_vec_i32_serialization_multiple() {
		let wrapper = RevisionSpecialisedVecI32::from_vec(vec![-1, 0, 1, i32::MIN, i32::MAX]);
		let bytes = to_vec(&wrapper).unwrap();
		let out: RevisionSpecialisedVecI32 = from_slice(&bytes).unwrap();
		assert_eq!(out.as_slice(), &[-1, 0, 1, i32::MIN, i32::MAX]);
	}

	#[test]
	fn test_revision_specialised_vec_i32_serialization_large() {
		let data: Vec<i32> = (0..20_000).map(|i| (i as i32).wrapping_mul(13)).collect();
		let wrapper = RevisionSpecialisedVecI32::from_vec(data.clone());
		let bytes = to_vec(&wrapper).unwrap();
		let out: RevisionSpecialisedVecI32 = from_slice(&bytes).unwrap();
		assert_eq!(out.as_slice(), data.as_slice());
	}

	#[test]
	fn test_revision_specialised_vec_i32_conversion() {
		let original = vec![1i32, 2, 3];
		let wrapper: RevisionSpecialisedVecI32 = original.clone().into();
		let back: Vec<i32> = wrapper.into();
		assert_eq!(back, original);
	}

	#[test]
	fn test_revision_specialised_vec_i32_as_ref() {
		let wrapper = RevisionSpecialisedVecI32::from_vec(vec![1i32, 2, 3]);
		let slice: &[i32] = wrapper.as_ref();
		assert_eq!(slice, &[1, 2, 3]);
	}

	#[test]
	fn test_revision_specialised_vec_i32_clear_and_reserve() {
		let mut wrapper = RevisionSpecialisedVecI32::from_vec(vec![1i32, 2, 3]);
		wrapper.clear();
		assert!(wrapper.is_empty());
		wrapper.reserve(100);
		assert!(wrapper.capacity() >= 100);
	}

	#[test]
	fn test_revision_specialised_vec_i32_extreme_and_negative_values() {
		let vals = vec![i32::MIN, -10_000_000, -1, 0, 1, 10_000_000, i32::MAX];
		let wrapper = RevisionSpecialisedVecI32::from_vec(vals.clone());
		let bytes = to_vec(&wrapper).unwrap();
		let out: RevisionSpecialisedVecI32 = from_slice(&bytes).unwrap();
		assert_eq!(out.as_slice(), vals.as_slice());
	}

	#[test]
	fn test_consistency_with_regular_vec_i32() {
		// Ensure that a Vec<i32> serialized via this specialized type round-trips correctly
		let values: Vec<i32> = (-50_000..50_000).map(|x| x as i32).collect();
		let wrapper = RevisionSpecialisedVecI32::from_vec(values.clone());
		let bytes = to_vec(&wrapper).unwrap();
		let out: RevisionSpecialisedVecI32 = from_slice(&bytes).unwrap();
		assert_eq!(out.as_slice(), values.as_slice());
	}
}
