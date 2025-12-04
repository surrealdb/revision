#![cfg(feature = "imbl")]

use super::super::Error;
use super::super::{DeserializeRevisioned, Revisioned, SerializeRevisioned};
use imbl::{HashMap, HashSet, OrdMap, OrdSet, Vector};
use std::hash::Hash;

// --------------------------------------------------
// Vector<T>
// --------------------------------------------------

impl<T: SerializeRevisioned + Clone> SerializeRevisioned for Vector<T> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.len().serialize_revisioned(writer)?;
		for v in self.iter() {
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<T: DeserializeRevisioned + Clone> DeserializeRevisioned for Vector<T> {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let len = usize::deserialize_revisioned(reader)?;
		let mut vec = Self::new();
		for _ in 0..len {
			let v = T::deserialize_revisioned(reader)?;
			vec.push_back(v);
		}
		Ok(vec)
	}
}

impl<T: Revisioned + Clone> Revisioned for Vector<T> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// --------------------------------------------------
// OrdMap<K, V>
// --------------------------------------------------

impl<K: SerializeRevisioned + Ord + Clone, V: SerializeRevisioned + Clone> SerializeRevisioned
	for OrdMap<K, V>
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

impl<K: DeserializeRevisioned + Ord + Clone, V: DeserializeRevisioned + Clone> DeserializeRevisioned
	for OrdMap<K, V>
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

impl<K: Revisioned + Ord + Clone, V: Revisioned + Clone> Revisioned for OrdMap<K, V> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// --------------------------------------------------
// OrdSet<T>
// --------------------------------------------------

impl<T: SerializeRevisioned + Ord + Clone> SerializeRevisioned for OrdSet<T> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.len().serialize_revisioned(writer)?;
		for v in self.iter() {
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<T: DeserializeRevisioned + Ord + Clone> DeserializeRevisioned for OrdSet<T> {
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

impl<T: Revisioned + Ord + Clone> Revisioned for OrdSet<T> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// --------------------------------------------------
// HashMap<K, V>
// --------------------------------------------------

impl<K: SerializeRevisioned + Hash + Eq + Clone, V: SerializeRevisioned + Clone>
	SerializeRevisioned for HashMap<K, V>
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

impl<K: DeserializeRevisioned + Hash + Eq + Clone, V: DeserializeRevisioned + Clone>
	DeserializeRevisioned for HashMap<K, V>
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

impl<K: Revisioned + Hash + Eq + Clone, V: Revisioned + Clone> Revisioned for HashMap<K, V> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// --------------------------------------------------
// HashSet<T>
// --------------------------------------------------

impl<T: SerializeRevisioned + Hash + Eq + Clone> SerializeRevisioned for HashSet<T> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.len().serialize_revisioned(writer)?;
		for v in self.iter() {
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl<T: DeserializeRevisioned + Hash + Eq + Clone> DeserializeRevisioned for HashSet<T> {
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

impl<T: Revisioned + Hash + Eq + Clone> Revisioned for HashSet<T> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// --------------------------------------------------
// Tests
// --------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_vector() {
		let mut val: Vector<String> = Vector::new();
		val.push_back("this".into());
		val.push_back("is".into());
		val.push_back("a".into());
		val.push_back("test".into());
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Vector<String> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vector_empty() {
		let val: Vector<i32> = Vector::new();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Vector<i32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_vector_i32() {
		let val: Vector<i32> = vec![1, 2, 3, 4, 5].into_iter().collect();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<Vector<i32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_ordmap() {
		let mut val: OrdMap<String, Vec<f64>> = OrdMap::new();
		val.insert("some".into(), vec![1.449, -5365.3849, 97194619.117391]);
		val.insert("test".into(), vec![-3917.195, 19461.3849, -365.195759]);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <OrdMap<String, Vec<f64>> as DeserializeRevisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_ordmap_empty() {
		let val: OrdMap<String, i32> = OrdMap::new();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<OrdMap<String, i32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_ordset() {
		let mut val: OrdSet<String> = OrdSet::new();
		val.insert("one".into());
		val.insert("two".into());
		val.insert("three".into());
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<OrdSet<String> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_ordset_empty() {
		let val: OrdSet<i32> = OrdSet::new();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<OrdSet<i32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_hashmap() {
		let mut val: HashMap<String, Vec<f64>> = HashMap::new();
		val.insert("some".into(), vec![1.449, -5365.3849, 97194619.117391]);
		val.insert("test".into(), vec![-3917.195, 19461.3849, -365.195759]);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <HashMap<String, Vec<f64>> as DeserializeRevisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_hashmap_empty() {
		let val: HashMap<String, i32> = HashMap::new();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out = <HashMap<String, i32> as DeserializeRevisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_hashset() {
		let mut val: HashSet<String> = HashSet::new();
		val.insert("one".into());
		val.insert("two".into());
		val.insert("three".into());
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<HashSet<String> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_hashset_empty() {
		let val: HashSet<i32> = HashSet::new();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<HashSet<i32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}
}

