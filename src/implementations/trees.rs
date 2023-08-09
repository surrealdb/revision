use super::super::Error;
use super::super::Revisioned;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::hash::Hash;

impl<K: Revisioned + Eq + Hash, V: Revisioned> Revisioned for HashMap<K, V> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.len().serialize_revisioned(writer)?;
		for (k, v) in self.iter() {
			k.serialize_revisioned(writer)?;
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let len = usize::deserialize_revisioned(reader)?;
		let mut map = HashMap::with_capacity(len);
		for _ in 0..len {
			let k = K::deserialize_revisioned(reader)?;
			let v = V::deserialize_revisioned(reader)?;
			map.insert(k, v);
		}
		Ok(map)
	}

	fn revision() -> u16 {
		1
	}
}

impl<K: Revisioned + Ord, V: Revisioned> Revisioned for BTreeMap<K, V> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.len().serialize_revisioned(writer)?;
		for (k, v) in self.iter() {
			k.serialize_revisioned(writer)?;
			v.serialize_revisioned(writer)?;
		}
		Ok(())
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let len = usize::deserialize_revisioned(reader)?;
		let mut map = BTreeMap::new();
		for _ in 0..len {
			let k = K::deserialize_revisioned(reader)?;
			let v = V::deserialize_revisioned(reader)?;
			map.insert(k, v);
		}
		Ok(map)
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::BTreeMap;
	use super::HashMap;
	use super::Revisioned;

	#[test]
	fn test_hashmap() {
		let mut val: HashMap<String, Vec<f64>> = HashMap::new();
		val.insert("some".into(), vec![1.449, -5365.3849, 97194619.117391]);
		val.insert("test".into(), vec![-3917.195, 19461.3849, -365.195759]);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 61);
		let out =
			<HashMap<String, Vec<f64>> as Revisioned>::deserialize_revisioned(&mut mem.as_slice())
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
		let out =
			<BTreeMap<String, Vec<f64>> as Revisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}
}
