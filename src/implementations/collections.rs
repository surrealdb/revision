use crate::DeserializeRevisioned;
use crate::Error;
use crate::Revisioned;
use crate::SerializeRevisioned;

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::BuildHasher;
use std::hash::Hash;

impl<K: SerializeRevisioned + Eq + Hash, V: SerializeRevisioned, S: BuildHasher + Default>
	SerializeRevisioned for HashMap<K, V, S>
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Get the length once
		let len = self.len();
		// Write the length first
		len.serialize_revisioned(writer)?;
		// For zero-length maps, return early
		if len == 0 {
			return Ok(());
		}
		// Iterate and serialize each item
		for (k, v) in self.iter() {
			k.serialize_revisioned(writer)?;
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<K: DeserializeRevisioned + Eq + Hash, V: DeserializeRevisioned, S: BuildHasher + Default>
	DeserializeRevisioned for HashMap<K, V, S>
{
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first
		let len = usize::deserialize_revisioned(reader)?;
		// Create a hash map with the necessary capacity
		let mut map = Self::with_capacity_and_hasher(len, S::default());
		// Iterate and deserialize each item
		for _ in 0..len {
			let k = K::deserialize_revisioned(reader)?;
			let v = V::deserialize_revisioned(reader)?;
			map.insert(k, v);
		}
		Ok(map)
	}
}

impl<K: Revisioned + Eq + Hash, V: Revisioned, S: BuildHasher + Default> Revisioned
	for HashMap<K, V, S>
{
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl<K: SerializeRevisioned + Ord, V: SerializeRevisioned> SerializeRevisioned for BTreeMap<K, V> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Get the length once
		let len = self.len();
		// Write the length first
		len.serialize_revisioned(writer)?;
		// For zero-length maps, return early
		if len == 0 {
			return Ok(());
		}
		// Iterate and serialize each item
		for (k, v) in self.iter() {
			k.serialize_revisioned(writer)?;
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<K: DeserializeRevisioned + Ord, V: DeserializeRevisioned> DeserializeRevisioned
	for BTreeMap<K, V>
{
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first
		let len = usize::deserialize_revisioned(reader)?;
		// Pre-allocate a Vec to collect all items with better cache locality
		let mut items = Vec::with_capacity(len);
		// Iterate and deserialize each item
		for _ in 0..len {
			// Deserialize the value
			let k = K::deserialize_revisioned(reader)?;
			let v = V::deserialize_revisioned(reader)?;
			// Hint to compiler that push is within capacity
			unsafe { std::hint::assert_unchecked(items.len() < items.capacity()) };
			// Push the item to the vector
			items.push((k, v));
		}
		// Use FromIterator for bulk construction
		Ok(items.into_iter().collect())
	}
}

impl<K: Revisioned + Ord, V: Revisioned> Revisioned for BTreeMap<K, V> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl<T: SerializeRevisioned + Eq + Hash, S: BuildHasher + Default> SerializeRevisioned
	for HashSet<T, S>
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Get the length once
		let len = self.len();
		// Write the length first
		len.serialize_revisioned(writer)?;
		// For zero-length sets, return early
		if len == 0 {
			return Ok(());
		}
		// Iterate and serialize each item
		for v in self.iter() {
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<T: DeserializeRevisioned + Eq + Hash, S: BuildHasher + Default> DeserializeRevisioned
	for HashSet<T, S>
{
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first
		let len = usize::deserialize_revisioned(reader)?;
		// Create a hash set with the necessary capacity
		let mut set = Self::with_capacity_and_hasher(len, S::default());
		// Iterate and deserialize each item
		for _ in 0..len {
			let v = T::deserialize_revisioned(reader)?;
			set.insert(v);
		}
		Ok(set)
	}
}

impl<T: Revisioned + Eq + Hash, S: BuildHasher + Default> Revisioned for HashSet<T, S> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl<T: SerializeRevisioned + Ord> SerializeRevisioned for BTreeSet<T> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Get the length once
		let len = self.len();
		// Write the length first
		len.serialize_revisioned(writer)?;
		// For zero-length sets, return early
		if len == 0 {
			return Ok(());
		}
		// Iterate and serialize each item
		for v in self.iter() {
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<T: DeserializeRevisioned + Ord> DeserializeRevisioned for BTreeSet<T> {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first
		let len = usize::deserialize_revisioned(reader)?;
		// Pre-allocate a Vec to collect all items with better cache locality
		let mut items = Vec::with_capacity(len);
		// Iterate and deserialize each item
		for _ in 0..len {
			// Deserialize the value
			let v = T::deserialize_revisioned(reader)?;
			// Hint to compiler that push is within capacity
			unsafe { std::hint::assert_unchecked(items.len() < items.capacity()) };
			// Push the item to the vector
			items.push(v);
		}
		// Use FromIterator for bulk construction
		Ok(items.into_iter().collect())
	}
}

impl<T: Revisioned + Eq + Ord> Revisioned for BTreeSet<T> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl<T: SerializeRevisioned + Ord> SerializeRevisioned for BinaryHeap<T> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		// Get the length once
		let len = self.len();
		// Write the length first
		len.serialize_revisioned(writer)?;
		// For zero-length heaps, return early
		if len == 0 {
			return Ok(());
		}
		// Iterate and serialize each item
		for v in self.iter() {
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<T: DeserializeRevisioned + Ord> DeserializeRevisioned for BinaryHeap<T> {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		// Read the length first
		let len = usize::deserialize_revisioned(reader)?;
		// Create a binary heap with the necessary capacity
		let mut heap = Self::with_capacity(len);
		// Iterate and deserialize each item
		for _ in 0..len {
			let v = T::deserialize_revisioned(reader)?;
			heap.push(v);
		}
		Ok(heap)
	}
}

impl<T: Revisioned + Ord> Revisioned for BinaryHeap<T> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn test_hashmap() {
		let mut val: HashMap<String, Vec<f64>> = HashMap::new();
		val.insert("some".into(), vec![1.449, -5365.3849, 97194619.117391]);
		val.insert("test".into(), vec![-3917.195, 19461.3849, -365.195759]);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 61);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 96);
		let out = <HashMap<String, Vec<f64>> as DeserializeRevisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_hashmap_nondefault_hasher() {
		#[derive(Default)]
		struct TestHasher(std::hash::RandomState);
		impl BuildHasher for TestHasher {
			type Hasher = <std::hash::RandomState as std::hash::BuildHasher>::Hasher;
			fn build_hasher(&self) -> Self::Hasher {
				self.0.build_hasher()
			}
		}

		let mut val: HashMap<String, Vec<f64>, TestHasher> =
			HashMap::with_hasher(Default::default());
		val.insert("some".into(), vec![1.449, -5365.3849, 97194619.117391]);
		val.insert("test".into(), vec![-3917.195, 19461.3849, -365.195759]);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 61);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 96);
		let out = <HashMap<String, Vec<f64>, TestHasher> as DeserializeRevisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_btreemap() {
		let mut val: BTreeMap<String, Vec<f64>> = BTreeMap::new();
		val.insert("some".into(), vec![1.449, -5365.3849, 97194619.117391]);
		val.insert("test".into(), vec![-3917.195, 19461.3849, -365.195759]);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 61);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 96);
		let out = <BTreeMap<String, Vec<f64>> as DeserializeRevisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_hashset() {
		let mut val: HashSet<String> = HashSet::new();
		val.insert("some".into());
		val.insert("test".into());
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 11);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 32);
		let out =
			<HashSet<String> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_btreeset() {
		let mut val: BTreeSet<String> = BTreeSet::new();
		val.insert("some".into());
		val.insert("test".into());
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 11);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 32);
		let out = <BTreeSet<String> as DeserializeRevisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_binheap() {
		let mut val: BinaryHeap<String> = BinaryHeap::new();
		val.push("some".into());
		val.push("test".into());
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 11);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 32);
		let out = <BinaryHeap<String> as DeserializeRevisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val.into_sorted_vec(), out.into_sorted_vec());
	}

	#[test]
	fn test_hashset_string_empty() {
		let set: HashSet<String> = HashSet::new();
		let mut mem: Vec<u8> = vec![];
		set.serialize_revisioned(&mut mem).unwrap();

		let out: HashSet<String> = HashSet::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(set, out);
	}

	#[test]
	fn test_btreeset_string_empty() {
		let set: BTreeSet<String> = BTreeSet::new();
		let mut mem: Vec<u8> = vec![];
		set.serialize_revisioned(&mut mem).unwrap();

		let out: BTreeSet<String> = BTreeSet::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(set, out);
	}

	#[test]
	fn test_hashmap_string_empty() {
		let map: HashMap<String, i32> = HashMap::new();
		let mut mem: Vec<u8> = vec![];
		map.serialize_revisioned(&mut mem).unwrap();

		let out: HashMap<String, i32> =
			HashMap::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(map, out);
	}

	#[test]
	fn test_btreemap_string_empty() {
		let map: BTreeMap<String, i32> = BTreeMap::new();
		let mut mem: Vec<u8> = vec![];
		map.serialize_revisioned(&mut mem).unwrap();

		let out: BTreeMap<String, i32> =
			BTreeMap::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(map, out);
	}

	#[test]
	fn test_hashset_string_specialization() {
		let mut set = HashSet::new();
		set.insert("item1".to_string());
		set.insert("item2".to_string());
		set.insert("".to_string());
		set.insert("longer_item_with_underscores".to_string());
		set.insert("unicode_🚀🔥✨".to_string());

		let mut mem: Vec<u8> = vec![];
		set.serialize_revisioned(&mut mem).unwrap();

		let out: HashSet<String> = HashSet::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(set, out);
	}

	#[test]
	fn test_btreeset_string_specialization() {
		let mut set = BTreeSet::new();
		set.insert("item1".to_string());
		set.insert("item2".to_string());
		set.insert("".to_string());
		set.insert("longer_item_with_underscores".to_string());
		set.insert("unicode_🚀🔥✨".to_string());

		let mut mem: Vec<u8> = vec![];
		set.serialize_revisioned(&mut mem).unwrap();

		let out: BTreeSet<String> = BTreeSet::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(set, out);
	}

	#[test]
	fn test_hashset_string_large() {
		// Test larger HashSet to verify bulk operations and deterministic serialization
		let mut set = HashSet::new();
		for i in 0..50 {
			set.insert(format!("item_{}", i));
		}

		let mut mem: Vec<u8> = vec![];
		set.serialize_revisioned(&mut mem).unwrap();

		let out: HashSet<String> = HashSet::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(set, out);
	}

	#[test]
	fn test_btreeset_string_large() {
		// Test larger BTreeSet to verify bulk operations and ordering
		let mut set = BTreeSet::new();
		for i in 0..50 {
			set.insert(format!("item_{:03}", i)); // Zero-padded for consistent ordering
		}

		let mut mem: Vec<u8> = vec![];
		set.serialize_revisioned(&mut mem).unwrap();

		let out: BTreeSet<String> = BTreeSet::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(set, out);
	}

	// Tests specifically for the String-keyed specializations
	#[test]
	fn test_hashmap_string_specialization() {
		let mut map = HashMap::new();
		map.insert("key1".to_string(), 42i32);
		map.insert("key2".to_string(), -100i32);
		map.insert("".to_string(), 0i32);
		map.insert("longer_key_with_underscores".to_string(), 999i32);
		map.insert("unicode_🚀🔥✨".to_string(), -42i32);

		let mut mem: Vec<u8> = vec![];
		map.serialize_revisioned(&mut mem).unwrap();

		let out: HashMap<String, i32> =
			HashMap::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(map, out);
	}

	#[test]
	fn test_btreemap_string_specialization() {
		let mut map = BTreeMap::new();
		map.insert("key1".to_string(), 42i32);
		map.insert("key2".to_string(), -100i32);
		map.insert("".to_string(), 0i32);
		map.insert("longer_key_with_underscores".to_string(), 999i32);
		map.insert("unicode_🚀🔥✨".to_string(), -42i32);

		let mut mem: Vec<u8> = vec![];
		map.serialize_revisioned(&mut mem).unwrap();

		let out: BTreeMap<String, i32> =
			BTreeMap::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(map, out);
	}
}
