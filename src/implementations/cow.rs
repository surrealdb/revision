use std::borrow::Cow;

use crate::{DeserializeRevisioned, Revisioned, SerializeRevisioned};

impl<T> SerializeRevisioned for Cow<'_, T>
where
	T: Sized + ToOwned + SerializeRevisioned,
	T::Owned: SerializeRevisioned,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, w: &mut W) -> Result<(), crate::Error> {
		match self {
			Cow::Borrowed(b) => b.serialize_revisioned(w),
			Cow::Owned(o) => o.serialize_revisioned(w),
		}
	}
}

impl<T> DeserializeRevisioned for Cow<'_, T>
where
	T: Sized + ToOwned + DeserializeRevisioned,
	T::Owned: DeserializeRevisioned,
{
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(r: &mut R) -> Result<Self, crate::Error> {
		T::Owned::deserialize_revisioned(r).map(Cow::Owned)
	}
}

impl<T> Revisioned for Cow<'_, T>
where
	T: Sized + ToOwned + Revisioned,
	T::Owned: Revisioned,
{
	#[inline]
	fn revision() -> u16 {
		T::revision()
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn cow_borrow() {
		let number = 20u8;

		let cow = Cow::Borrowed(&number);
		let mut mem = Vec::new();
		cow.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 1);
		let out = Cow::<u8>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert!(matches!(out, Cow::Owned(_)));
		assert_eq!(*out, number)
	}

	#[test]
	fn cow_owned() {
		let number = 20u8;

		let cow: Cow<u8> = Cow::Owned(number);
		let mut mem = Vec::new();
		cow.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 1);
		let out = Cow::<u8>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert!(matches!(out, Cow::Owned(_)));
		assert_eq!(*out, number)
	}
}
