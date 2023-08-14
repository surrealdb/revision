use std::borrow::Cow;

use crate::Revisioned;

impl<T> Revisioned for Cow<'_, T>
where
	T: ?Sized + ToOwned + Revisioned,
	T::Owned: Revisioned,
{
	fn revision() -> u16 {
		T::revision()
	}

	fn serialize_revisioned<W: std::io::Write>(&self, w: &mut W) -> Result<(), crate::Error> {
		(**self).serialize_revisioned(w)
	}

	fn deserialize_revisioned<R: std::io::Read>(r: &mut R) -> Result<Self, crate::Error>
	where
		Self: Sized,
	{
		T::Owned::deserialize_revisioned(r).map(Cow::Owned)
	}
}
