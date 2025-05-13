use crate::DeserializeRevisioned;
use crate::SerializeRevisioned;

use super::super::Error;
use super::super::Revisioned;

pub(crate) fn serialize_slice<T, W>(v: &[T], writer: &mut W) -> Result<(), Error>
where
	W: std::io::Write,
	T: SerializeRevisioned,
{
	v.len().serialize_revisioned(writer)?;
	for v in v {
		v.serialize_revisioned(writer)?;
	}
	Ok(())
}

impl<T> SerializeRevisioned for Vec<T>
where
	T: SerializeRevisioned,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		serialize_slice(self.as_slice(), writer)
	}
}

impl<T> DeserializeRevisioned for Vec<T>
where
	T: DeserializeRevisioned,
{
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let len = usize::deserialize_revisioned(reader)?;
		let mut vec = Vec::with_capacity(len);
		for _ in 0..len {
			let v: T = T::deserialize_revisioned(reader)?;
			vec.push(v);
		}
		Ok(vec)
	}
}

impl<T> Revisioned for Vec<T>
where
	T: Revisioned,
{
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn test_vec() {
		let val: Vec<String> =
			vec![String::from("this"), String::from("is"), String::from("a"), String::from("test")];
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out =
			<Vec<String> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}
}
