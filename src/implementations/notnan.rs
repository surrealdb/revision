#![cfg(feature = "ordered-float")]

use super::super::Error;
use super::super::{DeserializeRevisioned, Revisioned, SerializeRevisioned};
use ordered_float::{FloatCore, NotNan};

impl<T> SerializeRevisioned for NotNan<T>
where
	T: SerializeRevisioned + FloatCore,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.as_ref().serialize_revisioned(writer)
	}
}

impl<T> DeserializeRevisioned for NotNan<T>
where
	T: DeserializeRevisioned + FloatCore,
{
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		NotNan::new(T::deserialize_revisioned(reader)?)
			.map_err(|e| Error::Deserialize(format!("{:?}", e)))
	}
}

impl<T> Revisioned for NotNan<T>
where
	T: Revisioned + FloatCore,
{
	#[inline]
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn test_wrapping() {
		let val: NotNan<f32> = NotNan::new(f32::MAX).unwrap();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 4);
		let out =
			<NotNan<f32> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}
}
