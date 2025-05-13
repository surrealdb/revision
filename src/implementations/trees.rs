use super::super::Error;
use super::super::{DeserializeRevisioned, Revisioned, SerializeRevisioned};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::BuildHasher;
use std::hash::Hash;

impl<K: SerializeRevisioned + Eq + Hash, V: SerializeRevisioned> SerializeRevisioned
	for HashMap<K, V>
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.len().serialize_revisioned(writer)?;
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
		let len = usize::deserialize_revisioned(reader)?;
		let mut map = Self::with_capacity_and_hasher(len, S::default());
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
		self.len().serialize_revisioned(writer)?;
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
		let len = usize::deserialize_revisioned(reader)?;
		let mut map = Self::new();
		for _ in 0..len {
			let k = K::deserialize_revisioned(reader)?;
			let v = V::deserialize_revisioned(reader)?;
			map.insert(k, v);
		}
		Ok(map)
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
		self.len().serialize_revisioned(writer)?;
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
		let len = usize::deserialize_revisioned(reader)?;
		let mut set = Self::with_capacity_and_hasher(len, S::default());
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
		self.len().serialize_revisioned(writer)?;
		for v in self.iter() {
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<T: DeserializeRevisioned + Ord> DeserializeRevisioned for BTreeSet<T> {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let len = usize::deserialize_revisioned(reader)?;
		let mut set = Self::new();
		for _ in 0..len {
			let v = T::deserialize_revisioned(reader)?;
			set.insert(v);
		}
		Ok(set)
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
		self.len().serialize_revisioned(writer)?;
		for v in self.iter() {
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<T: DeserializeRevisioned + Ord> DeserializeRevisioned for BinaryHeap<T> {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let len = usize::deserialize_revisioned(reader)?;
		let mut heap = Self::with_capacity(len);
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
		assert_eq!(mem.len(), 61);
		let out = <HashMap<String, Vec<f64>> as DeserializeRevisioned>::deserialize_revisioned(
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
		assert_eq!(mem.len(), 61);
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
		assert_eq!(mem.len(), 11);
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
		assert_eq!(mem.len(), 11);
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
		assert_eq!(mem.len(), 11);
		let out = <BinaryHeap<String> as DeserializeRevisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val.into_sorted_vec(), out.into_sorted_vec());
	}
}
