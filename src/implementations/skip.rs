//! [`SkipRevisioned`] / [`SkipCheckRevisioned`] implementations (feature `skip`).

use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet};
use std::hash::{BuildHasher, Hash};
use std::io::Read;
use std::num::Wrapping;
use std::ops::{Bound, Range};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use crate::slice_reader::{SliceReader, advance_read};
use crate::{DeserializeRevisioned, Error, Revisioned};
use crate::{SkipCheckRevisioned, SkipRevisioned};

macro_rules! skip_mirror_both {
	($($t:ty),* $(,)?) => {$(
		impl SkipRevisioned for $t {
			#[inline]
			fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
				let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
				Ok(())
			}
		}
		impl SkipCheckRevisioned for $t {
			#[inline]
			fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
				let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
				Ok(())
			}
		}
	)*};
}

skip_mirror_both!(
	bool,
	usize,
	isize,
	u8,
	i8,
	u16,
	i16,
	u32,
	i32,
	u64,
	i64,
	u128,
	i128,
	f32,
	f64,
	char,
	std::time::Duration,
	PathBuf,
);

impl SkipRevisioned for String {
	#[inline]
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		advance_read(reader, len)?;
		Ok(())
	}
	#[inline]
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		reader.consume(len)?;
		Ok(())
	}
}

impl SkipCheckRevisioned for String {
	#[inline]
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<T> SkipRevisioned for Range<T>
where
	T: SkipRevisioned + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		T::skip_revisioned(reader)?;
		T::skip_revisioned(reader)?;
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		T::skip_revisioned_slice(reader)?;
		T::skip_revisioned_slice(reader)?;
		Ok(())
	}
}

impl<T> SkipCheckRevisioned for Range<T>
where
	T: SkipCheckRevisioned + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		T::skip_check_revisioned(reader)?;
		T::skip_check_revisioned(reader)?;
		Ok(())
	}
}

impl SkipRevisioned for SystemTime {
	#[inline]
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		<u64 as SkipRevisioned>::skip_revisioned(reader)?;
		<u32 as SkipRevisioned>::skip_revisioned(reader)?;
		Ok(())
	}
	#[inline]
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		<u64 as SkipRevisioned>::skip_revisioned_slice(reader)?;
		<u32 as SkipRevisioned>::skip_revisioned_slice(reader)?;
		Ok(())
	}
}

impl SkipCheckRevisioned for SystemTime {
	#[inline]
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		<u64 as SkipCheckRevisioned>::skip_check_revisioned(reader)?;
		<u32 as SkipCheckRevisioned>::skip_check_revisioned(reader)?;
		Ok(())
	}
}

macro_rules! skip_array_sizes {
    ($($N:literal)+) => {
        $(
        impl<T> SkipRevisioned for [T; $N]
        where T: Revisioned + SkipRevisioned + Copy + Default {
            #[inline]
            fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
                for _ in 0..$N {
                    T::skip_revisioned(reader)?;
                }
                Ok(())
            }
            #[inline]
            fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
                for _ in 0..$N {
                    T::skip_revisioned_slice(reader)?;
                }
                Ok(())
            }
        }
        impl<T> SkipCheckRevisioned for [T; $N]
        where T: Revisioned + SkipCheckRevisioned + Copy + Default {
            #[inline]
            fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
                for _ in 0..$N {
                    T::skip_check_revisioned(reader)?;
                }
                Ok(())
            }
        }
        )+
    };
}

skip_array_sizes! {
	 1  2  3  4  5  6  7  8  9 10
	11 12 13 14 15 16 17 18 19 20
	21 22 23 24 25 26 27 28 29 30
	31 32
}

macro_rules! tuple_skip_impl {
    ($($n:ident),*) => {
        impl<$($n),*> SkipRevisioned for ($($n,)*)
        where $($n: SkipRevisioned + Revisioned,)*
        {
            #[inline]
            fn skip_revisioned<R: Read>(_reader: &mut R) -> Result<(), Error> {
                $($n::skip_revisioned(_reader)?;)*
                Ok(())
            }
            #[inline]
            fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
                $($n::skip_revisioned_slice(reader)?;)*
                Ok(())
            }
        }
        impl<$($n),*> SkipCheckRevisioned for ($($n,)*)
        where $($n: SkipCheckRevisioned + Revisioned,)*
        {
            #[inline]
            fn skip_check_revisioned<R: Read>(_reader: &mut R) -> Result<(), Error> {
                $($n::skip_check_revisioned(_reader)?;)*
                Ok(())
            }
        }
    };
}

tuple_skip_impl! { A }
tuple_skip_impl! { A, B }
tuple_skip_impl! { A, B, C }
tuple_skip_impl! { A, B, C, D }
tuple_skip_impl! { A, B, C, D, E }
tuple_skip_impl! { A, B, C, D, E, F }

impl<T> SkipRevisioned for Vec<T>
where
	T: SkipRevisioned + Revisioned + 'static,
{
	#[inline]
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		#[cfg(feature = "specialised-vectors")]
		{
			use std::any::TypeId;
			macro_rules! specialised_bulk {
				($ty:ty) => {
					if TypeId::of::<T>() == TypeId::of::<$ty>() {
						let len = usize::deserialize_revisioned(reader)?;
						let byte_len = len
							.checked_mul(std::mem::size_of::<$ty>())
							.ok_or(Error::IntegerOverflow)?;
						advance_read(reader, byte_len)?;
						return Ok(());
					}
				};
			}

			if TypeId::of::<T>() == TypeId::of::<bool>() {
				let len = usize::deserialize_revisioned(reader)?;
				let packed = len.div_ceil(8);
				advance_read(reader, packed)?;
				return Ok(());
			}
			if TypeId::of::<T>() == TypeId::of::<u8>() || TypeId::of::<T>() == TypeId::of::<i8>() {
				let len = usize::deserialize_revisioned(reader)?;
				advance_read(reader, len)?;
				return Ok(());
			}
			specialised_bulk!(u16);
			specialised_bulk!(i16);
			specialised_bulk!(u32);
			specialised_bulk!(i32);
			specialised_bulk!(u64);
			specialised_bulk!(i64);
			specialised_bulk!(u128);
			specialised_bulk!(i128);
			specialised_bulk!(f32);
			specialised_bulk!(f64);
			#[cfg(feature = "rust_decimal")]
			{
				if TypeId::of::<T>() == TypeId::of::<rust_decimal::Decimal>() {
					let len = usize::deserialize_revisioned(reader)?;
					let byte_len = len.checked_mul(16).ok_or(Error::IntegerOverflow)?;
					advance_read(reader, byte_len)?;
					return Ok(());
				}
			}
			#[cfg(feature = "uuid")]
			{
				if TypeId::of::<T>() == TypeId::of::<uuid::Uuid>() {
					let len = usize::deserialize_revisioned(reader)?;
					let byte_len = len.checked_mul(16).ok_or(Error::IntegerOverflow)?;
					advance_read(reader, byte_len)?;
					return Ok(());
				}
			}
		}

		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned(reader)?;
		}
		Ok(())
	}

	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		#[cfg(feature = "specialised-vectors")]
		{
			use std::any::TypeId;
			macro_rules! specialised_bulk_slice {
				($ty:ty) => {
					if TypeId::of::<T>() == TypeId::of::<$ty>() {
						let len = usize::deserialize_revisioned(reader)?;
						let byte_len = len
							.checked_mul(std::mem::size_of::<$ty>())
							.ok_or(Error::IntegerOverflow)?;
						reader.consume(byte_len)?;
						return Ok(());
					}
				};
			}

			if TypeId::of::<T>() == TypeId::of::<bool>() {
				let len = usize::deserialize_revisioned(reader)?;
				let packed = len.div_ceil(8);
				reader.consume(packed)?;
				return Ok(());
			}
			if TypeId::of::<T>() == TypeId::of::<u8>() || TypeId::of::<T>() == TypeId::of::<i8>() {
				let len = usize::deserialize_revisioned(reader)?;
				reader.consume(len)?;
				return Ok(());
			}
			specialised_bulk_slice!(u16);
			specialised_bulk_slice!(i16);
			specialised_bulk_slice!(u32);
			specialised_bulk_slice!(i32);
			specialised_bulk_slice!(u64);
			specialised_bulk_slice!(i64);
			specialised_bulk_slice!(u128);
			specialised_bulk_slice!(i128);
			specialised_bulk_slice!(f32);
			specialised_bulk_slice!(f64);
			#[cfg(feature = "rust_decimal")]
			{
				if TypeId::of::<T>() == TypeId::of::<rust_decimal::Decimal>() {
					let len = usize::deserialize_revisioned(reader)?;
					let byte_len = len.checked_mul(16).ok_or(Error::IntegerOverflow)?;
					reader.consume(byte_len)?;
					return Ok(());
				}
			}
			#[cfg(feature = "uuid")]
			{
				if TypeId::of::<T>() == TypeId::of::<uuid::Uuid>() {
					let len = usize::deserialize_revisioned(reader)?;
					let byte_len = len.checked_mul(16).ok_or(Error::IntegerOverflow)?;
					reader.consume(byte_len)?;
					return Ok(());
				}
			}
		}

		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned_slice(reader)?;
		}
		Ok(())
	}
}

impl<T> SkipCheckRevisioned for Vec<T>
where
	T: DeserializeRevisioned + Revisioned + 'static,
{
	#[inline]
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Vec<T> as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<T> SkipRevisioned for Option<T>
where
	T: SkipRevisioned + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		match u8::deserialize_revisioned(reader)? {
			0u8 => Ok(()),
			1u8 => T::skip_revisioned(reader),
			v => Err(Error::Deserialize(format!("Invalid option value {v}"))),
		}
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		match u8::deserialize_revisioned(reader)? {
			0u8 => Ok(()),
			1u8 => T::skip_revisioned_slice(reader),
			v => Err(Error::Deserialize(format!("Invalid option value {v}"))),
		}
	}
}

impl<T> SkipCheckRevisioned for Option<T>
where
	T: DeserializeRevisioned + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<E, T> SkipRevisioned for Result<T, E>
where
	T: SkipRevisioned + Revisioned,
	E: SkipRevisioned + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		match u32::deserialize_revisioned(reader)? {
			0 => T::skip_revisioned(reader),
			1 => E::skip_revisioned(reader),
			_ => Err(Error::Deserialize("Unknown variant index".to_string())),
		}
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		match u32::deserialize_revisioned(reader)? {
			0 => T::skip_revisioned_slice(reader),
			1 => E::skip_revisioned_slice(reader),
			_ => Err(Error::Deserialize("Unknown variant index".to_string())),
		}
	}
}

impl<E, T> SkipCheckRevisioned for Result<T, E>
where
	T: DeserializeRevisioned + Revisioned,
	E: DeserializeRevisioned + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<T> SkipRevisioned for Box<T>
where
	T: SkipRevisioned + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		T::skip_revisioned(reader)
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		T::skip_revisioned_slice(reader)
	}
}

impl<T> SkipCheckRevisioned for Box<T>
where
	T: DeserializeRevisioned + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<T> SkipRevisioned for Arc<T>
where
	T: SkipRevisioned + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		T::skip_revisioned(reader)
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		T::skip_revisioned_slice(reader)
	}
}

impl<T> SkipCheckRevisioned for Arc<T>
where
	T: DeserializeRevisioned + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl SkipRevisioned for Arc<str> {
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		String::skip_revisioned(reader)
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		String::skip_revisioned_slice(reader)
	}
}

impl SkipCheckRevisioned for Arc<str> {
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<T> SkipRevisioned for Wrapping<T>
where
	T: SkipRevisioned + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		T::skip_revisioned(reader)
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		T::skip_revisioned_slice(reader)
	}
}

impl<T> SkipCheckRevisioned for Wrapping<T>
where
	T: DeserializeRevisioned + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<T> SkipRevisioned for Reverse<T>
where
	T: SkipRevisioned + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		T::skip_revisioned(reader)
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		T::skip_revisioned_slice(reader)
	}
}

impl<T> SkipCheckRevisioned for Reverse<T>
where
	T: DeserializeRevisioned + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<T> SkipRevisioned for Bound<T>
where
	T: SkipRevisioned + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		match u32::deserialize_revisioned(reader)? {
			0 => Ok(()),
			1 => T::skip_revisioned(reader),
			2 => T::skip_revisioned(reader),
			_ => Err(Error::Deserialize("Unknown variant index".to_string())),
		}
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		match u32::deserialize_revisioned(reader)? {
			0 => Ok(()),
			1 => T::skip_revisioned_slice(reader),
			2 => T::skip_revisioned_slice(reader),
			_ => Err(Error::Deserialize("Unknown variant index".to_string())),
		}
	}
}

impl<T> SkipCheckRevisioned for Bound<T>
where
	T: DeserializeRevisioned + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<T> SkipRevisioned for Cow<'_, T>
where
	T: Sized + ToOwned + Revisioned,
	T::Owned: SkipRevisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		T::Owned::skip_revisioned(reader)
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		T::Owned::skip_revisioned_slice(reader)
	}
}

impl<T> SkipCheckRevisioned for Cow<'_, T>
where
	T: Sized + ToOwned + DeserializeRevisioned + Revisioned,
	T::Owned: DeserializeRevisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl SkipRevisioned for Cow<'_, str> {
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		String::skip_revisioned(reader)
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		String::skip_revisioned_slice(reader)
	}
}

impl SkipCheckRevisioned for Cow<'_, str> {
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<K, V, S> SkipRevisioned for HashMap<K, V, S>
where
	K: Eq + Hash + SkipRevisioned + Revisioned,
	V: SkipRevisioned + Revisioned,
	S: BuildHasher + Default,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			K::skip_revisioned(reader)?;
			V::skip_revisioned(reader)?;
		}
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			K::skip_revisioned_slice(reader)?;
			V::skip_revisioned_slice(reader)?;
		}
		Ok(())
	}
}

impl<K, V, S> SkipCheckRevisioned for HashMap<K, V, S>
where
	K: Eq + Hash + DeserializeRevisioned + Revisioned,
	V: DeserializeRevisioned + Revisioned,
	S: BuildHasher + Default,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<K, V> SkipRevisioned for BTreeMap<K, V>
where
	K: Ord + SkipRevisioned + Revisioned,
	V: SkipRevisioned + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			K::skip_revisioned(reader)?;
			V::skip_revisioned(reader)?;
		}
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			K::skip_revisioned_slice(reader)?;
			V::skip_revisioned_slice(reader)?;
		}
		Ok(())
	}
}

impl<K, V> SkipCheckRevisioned for BTreeMap<K, V>
where
	K: Ord + DeserializeRevisioned + Revisioned,
	V: DeserializeRevisioned + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<T, S> SkipRevisioned for HashSet<T, S>
where
	T: Eq + Hash + SkipRevisioned + Revisioned,
	S: BuildHasher + Default,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned(reader)?;
		}
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned_slice(reader)?;
		}
		Ok(())
	}
}

impl<T, S> SkipCheckRevisioned for HashSet<T, S>
where
	T: Eq + Hash + DeserializeRevisioned + Revisioned,
	S: BuildHasher + Default,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<T> SkipRevisioned for BTreeSet<T>
where
	T: Ord + SkipRevisioned + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned(reader)?;
		}
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned_slice(reader)?;
		}
		Ok(())
	}
}

impl<T> SkipCheckRevisioned for BTreeSet<T>
where
	T: Ord + DeserializeRevisioned + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

impl<T> SkipRevisioned for BinaryHeap<T>
where
	T: Ord + SkipRevisioned + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned(reader)?;
		}
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned_slice(reader)?;
		}
		Ok(())
	}
}

impl<T> SkipCheckRevisioned for BinaryHeap<T>
where
	T: Ord + DeserializeRevisioned + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

#[cfg(feature = "ordered-float")]
use ordered_float::{FloatCore, NotNan};

#[cfg(feature = "ordered-float")]
impl<T> SkipRevisioned for NotNan<T>
where
	T: SkipRevisioned + FloatCore + Revisioned,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		T::skip_revisioned(reader)
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		T::skip_revisioned_slice(reader)
	}
}

#[cfg(feature = "ordered-float")]
impl<T> SkipCheckRevisioned for NotNan<T>
where
	T: DeserializeRevisioned + FloatCore + Revisioned,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

#[cfg(feature = "rust_decimal")]
use rust_decimal::Decimal;

#[cfg(feature = "rust_decimal")]
impl SkipRevisioned for Decimal {
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		advance_read(reader, 16)
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		reader.consume(16)
	}
}

#[cfg(feature = "rust_decimal")]
impl SkipCheckRevisioned for Decimal {
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

#[cfg(feature = "uuid")]
use uuid::Uuid;

#[cfg(feature = "uuid")]
impl SkipRevisioned for Uuid {
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		advance_read(reader, 16)
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		reader.consume(16)
	}
}

#[cfg(feature = "uuid")]
impl SkipCheckRevisioned for Uuid {
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

#[cfg(feature = "regex")]
use regex::Regex;

#[cfg(feature = "regex")]
impl SkipRevisioned for Regex {
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		String::skip_revisioned(reader)
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		String::skip_revisioned_slice(reader)
	}
}

#[cfg(feature = "regex")]
impl SkipCheckRevisioned for Regex {
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

#[cfg(feature = "bytes")]
use bytes::Bytes;

#[cfg(feature = "bytes")]
impl SkipRevisioned for Bytes {
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		advance_read(reader, len)?;
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		reader.consume(len)?;
		Ok(())
	}
}

#[cfg(feature = "bytes")]
impl SkipCheckRevisioned for Bytes {
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

#[cfg(feature = "roaring")]
use roaring::{RoaringBitmap, RoaringTreemap};

#[cfg(feature = "roaring")]
skip_mirror_both!(RoaringBitmap, RoaringTreemap);

#[cfg(feature = "chrono")]
use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, NaiveTime, Utc};

#[cfg(feature = "chrono")]
skip_mirror_both!(DateTime<Utc>, NaiveDate, NaiveTime, ChronoDuration);

#[cfg(feature = "geo")]
use geo::{Coord, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon};

#[cfg(feature = "geo")]
skip_mirror_both!(Coord, Point, LineString, Polygon, MultiPoint, MultiLineString, MultiPolygon);

#[cfg(feature = "imbl")]
use imbl::{HashMap as ImblHashMap, HashSet as ImblHashSet, OrdMap, OrdSet, Vector};

#[cfg(feature = "imbl")]
impl<T> SkipRevisioned for Vector<T>
where
	T: SkipRevisioned + Revisioned + Clone,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned(reader)?;
		}
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned_slice(reader)?;
		}
		Ok(())
	}
}

#[cfg(feature = "imbl")]
impl<T> SkipCheckRevisioned for Vector<T>
where
	T: DeserializeRevisioned + Revisioned + Clone,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

#[cfg(feature = "imbl")]
impl<K, V> SkipRevisioned for OrdMap<K, V>
where
	K: Ord + SkipRevisioned + Revisioned + Clone,
	V: SkipRevisioned + Revisioned + Clone,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			K::skip_revisioned(reader)?;
			V::skip_revisioned(reader)?;
		}
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			K::skip_revisioned_slice(reader)?;
			V::skip_revisioned_slice(reader)?;
		}
		Ok(())
	}
}

#[cfg(feature = "imbl")]
impl<K, V> SkipCheckRevisioned for OrdMap<K, V>
where
	K: Ord + DeserializeRevisioned + Revisioned + Clone,
	V: DeserializeRevisioned + Revisioned + Clone,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

#[cfg(feature = "imbl")]
impl<T> SkipRevisioned for OrdSet<T>
where
	T: Ord + SkipRevisioned + Revisioned + Clone,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned(reader)?;
		}
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned_slice(reader)?;
		}
		Ok(())
	}
}

#[cfg(feature = "imbl")]
impl<T> SkipCheckRevisioned for OrdSet<T>
where
	T: Ord + DeserializeRevisioned + Revisioned + Clone,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

#[cfg(feature = "imbl")]
impl<K, V> SkipRevisioned for ImblHashMap<K, V>
where
	K: Eq + Hash + SkipRevisioned + Revisioned + Clone,
	V: SkipRevisioned + Revisioned + Clone,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			K::skip_revisioned(reader)?;
			V::skip_revisioned(reader)?;
		}
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			K::skip_revisioned_slice(reader)?;
			V::skip_revisioned_slice(reader)?;
		}
		Ok(())
	}
}

#[cfg(feature = "imbl")]
impl<K, V> SkipCheckRevisioned for ImblHashMap<K, V>
where
	K: Eq + Hash + DeserializeRevisioned + Revisioned + Clone,
	V: DeserializeRevisioned + Revisioned + Clone,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}

#[cfg(feature = "imbl")]
impl<T> SkipRevisioned for ImblHashSet<T>
where
	T: Eq + Hash + SkipRevisioned + Revisioned + Clone,
{
	fn skip_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned(reader)?;
		}
		Ok(())
	}
	fn skip_revisioned_slice(reader: &mut SliceReader<'_>) -> Result<(), Error> {
		let len = usize::deserialize_revisioned(reader)?;
		for _ in 0..len {
			T::skip_revisioned_slice(reader)?;
		}
		Ok(())
	}
}

#[cfg(feature = "imbl")]
impl<T> SkipCheckRevisioned for ImblHashSet<T>
where
	T: Eq + Hash + DeserializeRevisioned + Revisioned + Clone,
{
	fn skip_check_revisioned<R: Read>(reader: &mut R) -> Result<(), Error> {
		let _ = <Self as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Ok(())
	}
}
