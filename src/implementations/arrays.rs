use crate::DeserializeRevisioned;
use crate::Error;
use crate::Revisioned;
use crate::SerializeRevisioned;

macro_rules! impl_revisioned_array_with_size {
	($ty:literal) => {
		impl<T> SerializeRevisioned for [T; $ty]
		where
			T: Copy + Default + SerializeRevisioned,
		{
			#[inline]
			fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
				for element in self {
					element.serialize_revisioned(writer)?;
				}
				Ok(())
			}
		}

		impl<T> DeserializeRevisioned for [T; $ty]
		where
			T: Copy + Default + DeserializeRevisioned,
		{
			#[inline]
			fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
				let mut array = [T::default(); $ty];
				for i in 0..$ty {
					array[i] = T::deserialize_revisioned(reader)?;
				}
				Ok(array)
			}
		}

		impl<T> Revisioned for [T; $ty]
		where
			T: Copy + Default + Revisioned,
		{
			#[inline]
			fn revision() -> u16 {
				1
			}
		}
	};
}

macro_rules! impl_revisioned_arrays {
    ($($N:literal)+) => {
        $(
            impl_revisioned_array_with_size!($N);
        )+
    }
}

impl_revisioned_arrays! {
	1  2  3  4  5  6  7  8  9 10
   11 12 13 14 15 16 17 18 19 20
   21 22 23 24 25 26 27 28 29 30
   31 32
}
