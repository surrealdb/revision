//! [`WalkRevisioned`] implementations for primitives, collections, wrappers,
//! and feature-gated types. Mirror layout of [`super::skip`].

#[cfg(feature = "specialised-vectors")]
use std::any::TypeId;
use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet};
use std::hash::{BuildHasher, Hash};
use std::num::Wrapping;
use std::ops::{Bound, Range};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use crate::slice_reader::BorrowedReader;
use crate::walk::{LeafWalker, MapWalker, OptionWalker, ResultWalker, SeqWalker};
use crate::{Error, Revisioned, WalkRevisioned};

/// Whether `Vec<T>` uses the compact bulk layout from `try_specialized!` in
/// `implementations/vecs.rs` for the given `T`. Only `Vec<T>` selects that
/// path; sets and heaps always use per-element framing, so this check is
/// scoped to [`Vec<T>::walk_revisioned`] and not the generic [`SeqWalker`].
#[cfg(feature = "specialised-vectors")]
#[inline]
fn vec_uses_bulk_encoding<T: 'static>() -> bool {
	let id = TypeId::of::<T>();
	macro_rules! bulk_primitive {
		($($ty:ty),* $(,)?) => {{
			$(if id == TypeId::of::<$ty>() {
				return true;
			})*
		}};
	}
	bulk_primitive!(bool, u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, f32, f64);
	#[cfg(feature = "uuid")]
	if id == TypeId::of::<uuid::Uuid>() {
		return true;
	}
	#[cfg(feature = "rust_decimal")]
	if id == TypeId::of::<rust_decimal::Decimal>() {
		return true;
	}
	false
}

#[cfg(not(feature = "specialised-vectors"))]
#[inline]
fn vec_uses_bulk_encoding<T: 'static>() -> bool {
	let _ = std::marker::PhantomData::<T>;
	false
}

// -----------------------------------------------------------------------------
// Primitive leaves
// -----------------------------------------------------------------------------

macro_rules! leaf_walk {
	($($t:ty),* $(,)?) => {$(
		impl WalkRevisioned for $t {
			type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, $t, R>;

			#[inline]
			fn walk_revisioned<'r, R: BorrowedReader>(
				reader: &'r mut R,
			) -> Result<Self::Walker<'r, R>, Error> {
				Ok(LeafWalker::new(reader))
			}
		}
	)*};
}

leaf_walk!(
	bool,
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
	usize,
	isize,
	f32,
	f64,
	char,
	String,
	std::time::Duration,
	PathBuf,
);

impl WalkRevisioned for SystemTime {
	type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, SystemTime, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		Ok(LeafWalker::new(reader))
	}
}

// -----------------------------------------------------------------------------
// Range<T>
// -----------------------------------------------------------------------------

impl<T: WalkRevisioned> WalkRevisioned for Range<T> {
	type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, Range<T>, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		Ok(LeafWalker::new(reader))
	}
}

// -----------------------------------------------------------------------------
// Tuples and arrays — treated as opaque leaves; deserialise/skip is the
// natural granularity. Future extensions could expose per-element walkers.
// -----------------------------------------------------------------------------

macro_rules! tuple_walk_impl {
	($($n:ident),* $(,)?) => {
		impl<$($n,)*> WalkRevisioned for ($($n,)*)
		where $($n: Revisioned,)*
		{
			type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, Self, R>;

			#[inline]
			fn walk_revisioned<'r, R: BorrowedReader>(
				reader: &'r mut R,
			) -> Result<Self::Walker<'r, R>, Error> {
				Ok(LeafWalker::new(reader))
			}
		}
	};
}

tuple_walk_impl! { A }
tuple_walk_impl! { A, B }
tuple_walk_impl! { A, B, C }
tuple_walk_impl! { A, B, C, D }
tuple_walk_impl! { A, B, C, D, E }
tuple_walk_impl! { A, B, C, D, E, F }

macro_rules! array_walk_sizes {
	($($N:literal)+) => {$(
		impl<T> WalkRevisioned for [T; $N]
		where
			T: Revisioned + Copy + Default,
		{
			type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, Self, R>;

			#[inline]
			fn walk_revisioned<'r, R: BorrowedReader>(
				reader: &'r mut R,
			) -> Result<Self::Walker<'r, R>, Error> {
				Ok(LeafWalker::new(reader))
			}
		}
	)+};
}

array_walk_sizes! {
	 1  2  3  4  5  6  7  8  9 10
	11 12 13 14 15 16 17 18 19 20
	21 22 23 24 25 26 27 28 29 30
	31 32
}

// -----------------------------------------------------------------------------
// Option<T>
// -----------------------------------------------------------------------------

impl<T> WalkRevisioned for Option<T>
where
	T: Revisioned,
{
	type Walker<'r, R: BorrowedReader + 'r> = OptionWalker<'r, T, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		OptionWalker::new(reader)
	}
}

// -----------------------------------------------------------------------------
// Result<T, E>
// -----------------------------------------------------------------------------

impl<T, E> WalkRevisioned for Result<T, E>
where
	T: Revisioned,
	E: Revisioned,
{
	type Walker<'r, R: BorrowedReader + 'r> = ResultWalker<'r, T, E, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		ResultWalker::new(reader)
	}
}

// -----------------------------------------------------------------------------
// Bound<T> — opaque (tag + optional payload). Surface as a leaf for v1; users
// who need to inspect the variant can decode/skip and then post-process.
// -----------------------------------------------------------------------------

impl<T> WalkRevisioned for Bound<T>
where
	T: Revisioned,
{
	type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, Bound<T>, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		Ok(LeafWalker::new(reader))
	}
}

// -----------------------------------------------------------------------------
// Vec<T> — sequence with length prefix
// -----------------------------------------------------------------------------

impl<T> WalkRevisioned for Vec<T>
where
	T: Revisioned + 'static,
{
	type Walker<'r, R: BorrowedReader + 'r> = SeqWalker<'r, T, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		if vec_uses_bulk_encoding::<T>() {
			return Err(Error::Deserialize(
				"Vec<T>: cannot walk Vec whose element type uses specialised bulk encoding \
				 when the `specialised-vectors` feature is enabled; use DeserializeRevisioned \
				 or SkipRevisioned instead."
					.into(),
			));
		}
		SeqWalker::new(reader)
	}
}

// -----------------------------------------------------------------------------
// HashSet<T>, BTreeSet<T>, BinaryHeap<T>
// -----------------------------------------------------------------------------

impl<T, S> WalkRevisioned for HashSet<T, S>
where
	T: Revisioned + Eq + Hash,
	S: BuildHasher + Default,
{
	type Walker<'r, R: BorrowedReader + 'r> = SeqWalker<'r, T, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		SeqWalker::new(reader)
	}
}

impl<T> WalkRevisioned for BTreeSet<T>
where
	T: Revisioned + Ord,
{
	type Walker<'r, R: BorrowedReader + 'r> = SeqWalker<'r, T, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		SeqWalker::new(reader)
	}
}

impl<T> WalkRevisioned for BinaryHeap<T>
where
	T: Revisioned + Ord,
{
	type Walker<'r, R: BorrowedReader + 'r> = SeqWalker<'r, T, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		SeqWalker::new(reader)
	}
}

// -----------------------------------------------------------------------------
// HashMap<K, V>, BTreeMap<K, V>
// -----------------------------------------------------------------------------

impl<K, V, S> WalkRevisioned for HashMap<K, V, S>
where
	K: Revisioned + Eq + Hash,
	V: Revisioned,
	S: BuildHasher + Default,
{
	type Walker<'r, R: BorrowedReader + 'r> = MapWalker<'r, K, V, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		MapWalker::new(reader)
	}
}

impl<K, V> WalkRevisioned for BTreeMap<K, V>
where
	K: Revisioned + Ord,
	V: Revisioned,
{
	type Walker<'r, R: BorrowedReader + 'r> = MapWalker<'r, K, V, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		MapWalker::new(reader)
	}
}

// -----------------------------------------------------------------------------
// Wrapping pointers/wrappers — transparent, defer to the inner walker.
// -----------------------------------------------------------------------------

impl<T> WalkRevisioned for Box<T>
where
	T: WalkRevisioned,
{
	type Walker<'r, R: BorrowedReader + 'r> = T::Walker<'r, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		T::walk_revisioned(reader)
	}
}

impl<T> WalkRevisioned for Arc<T>
where
	T: WalkRevisioned,
{
	type Walker<'r, R: BorrowedReader + 'r> = T::Walker<'r, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		T::walk_revisioned(reader)
	}
}

impl WalkRevisioned for Arc<str> {
	type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, Arc<str>, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		Ok(LeafWalker::new(reader))
	}
}

impl WalkRevisioned for Box<str> {
	type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, Box<str>, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		Ok(LeafWalker::new(reader))
	}
}

impl<T> WalkRevisioned for Wrapping<T>
where
	T: WalkRevisioned,
{
	type Walker<'r, R: BorrowedReader + 'r> = T::Walker<'r, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		T::walk_revisioned(reader)
	}
}

impl<T> WalkRevisioned for Reverse<T>
where
	T: WalkRevisioned,
{
	type Walker<'r, R: BorrowedReader + 'r> = T::Walker<'r, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		T::walk_revisioned(reader)
	}
}

/// `Cow<'_, T>` is treated as opaque by the walker. The walker is a
/// [`LeafWalker`] over `T::Owned` rather than a sub-walker into `T::Owned`'s
/// shape, so `decode()` returns `T::Owned` (e.g. `String` for
/// `Cow<'_, str>`), not a `Cow`. Use [`DeserializeRevisioned`] if you need
/// a `Cow` back, or call `T::Owned::walk_revisioned` directly to descend
/// into the inner shape.
///
/// [`DeserializeRevisioned`]: crate::DeserializeRevisioned
impl<T> WalkRevisioned for Cow<'_, T>
where
	T: ToOwned + Revisioned,
	T::Owned: WalkRevisioned,
{
	type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, T::Owned, R>;

	#[inline]
	fn walk_revisioned<'r, R: BorrowedReader>(
		reader: &'r mut R,
	) -> Result<Self::Walker<'r, R>, Error> {
		Ok(LeafWalker::new(reader))
	}
}

// -----------------------------------------------------------------------------
// Feature-gated leaves
// -----------------------------------------------------------------------------

#[cfg(feature = "ordered-float")]
mod notnan_walk {
	use super::*;
	use ordered_float::{FloatCore, NotNan};

	impl<T> WalkRevisioned for NotNan<T>
	where
		T: WalkRevisioned + FloatCore,
	{
		type Walker<'r, R: BorrowedReader + 'r> = T::Walker<'r, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			T::walk_revisioned(reader)
		}
	}
}

#[cfg(feature = "rust_decimal")]
mod decimal_walk {
	use super::*;
	use rust_decimal::Decimal;

	impl WalkRevisioned for Decimal {
		type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, Decimal, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			Ok(LeafWalker::new(reader))
		}
	}
}

#[cfg(feature = "uuid")]
mod uuid_walk {
	use super::*;
	use uuid::Uuid;

	impl WalkRevisioned for Uuid {
		type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, Uuid, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			Ok(LeafWalker::new(reader))
		}
	}
}

#[cfg(feature = "regex")]
mod regex_walk {
	use super::*;
	use regex::Regex;

	impl WalkRevisioned for Regex {
		type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, Regex, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			Ok(LeafWalker::new(reader))
		}
	}
}

#[cfg(feature = "bytes")]
mod bytes_walk {
	use super::*;
	use bytes::Bytes;

	impl WalkRevisioned for Bytes {
		type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, Bytes, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			Ok(LeafWalker::new(reader))
		}
	}
}

#[cfg(feature = "roaring")]
mod roaring_walk {
	use super::*;
	use roaring::{RoaringBitmap, RoaringTreemap};

	impl WalkRevisioned for RoaringBitmap {
		type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, RoaringBitmap, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			Ok(LeafWalker::new(reader))
		}
	}

	impl WalkRevisioned for RoaringTreemap {
		type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, RoaringTreemap, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			Ok(LeafWalker::new(reader))
		}
	}
}

#[cfg(feature = "chrono")]
mod chrono_walk {
	use super::*;
	use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, NaiveTime, Utc};

	impl WalkRevisioned for DateTime<Utc> {
		type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, DateTime<Utc>, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			Ok(LeafWalker::new(reader))
		}
	}

	impl WalkRevisioned for NaiveDate {
		type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, NaiveDate, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			Ok(LeafWalker::new(reader))
		}
	}

	impl WalkRevisioned for NaiveTime {
		type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, NaiveTime, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			Ok(LeafWalker::new(reader))
		}
	}

	impl WalkRevisioned for ChronoDuration {
		type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, ChronoDuration, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			Ok(LeafWalker::new(reader))
		}
	}
}

#[cfg(feature = "geo")]
mod geo_walk {
	use super::*;
	use geo::{Coord, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon};

	macro_rules! geo_leaf {
		($($t:ty),* $(,)?) => {$(
			impl WalkRevisioned for $t {
				type Walker<'r, R: BorrowedReader + 'r> = LeafWalker<'r, $t, R>;

				#[inline]
				fn walk_revisioned<'r, R: BorrowedReader>(
					reader: &'r mut R,
				) -> Result<Self::Walker<'r, R>, Error> {
					Ok(LeafWalker::new(reader))
				}
			}
		)*};
	}

	geo_leaf!(Coord, Point, LineString, Polygon, MultiPoint, MultiLineString, MultiPolygon);
}

#[cfg(feature = "imbl")]
mod imbl_walk {
	use super::*;
	use imbl::{HashMap as ImblHashMap, HashSet as ImblHashSet, OrdMap, OrdSet, Vector};

	impl<T> WalkRevisioned for Vector<T>
	where
		T: Revisioned + Clone,
	{
		type Walker<'r, R: BorrowedReader + 'r> = SeqWalker<'r, T, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			SeqWalker::new(reader)
		}
	}

	impl<K, V> WalkRevisioned for OrdMap<K, V>
	where
		K: Revisioned + Clone + Ord,
		V: Revisioned + Clone,
	{
		type Walker<'r, R: BorrowedReader + 'r> = MapWalker<'r, K, V, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			MapWalker::new(reader)
		}
	}

	impl<T> WalkRevisioned for OrdSet<T>
	where
		T: Revisioned + Clone + Ord,
	{
		type Walker<'r, R: BorrowedReader + 'r> = SeqWalker<'r, T, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			SeqWalker::new(reader)
		}
	}

	impl<K, V> WalkRevisioned for ImblHashMap<K, V>
	where
		K: Revisioned + Clone + Eq + Hash,
		V: Revisioned + Clone,
	{
		type Walker<'r, R: BorrowedReader + 'r> = MapWalker<'r, K, V, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			MapWalker::new(reader)
		}
	}

	impl<T> WalkRevisioned for ImblHashSet<T>
	where
		T: Revisioned + Clone + Eq + Hash,
	{
		type Walker<'r, R: BorrowedReader + 'r> = SeqWalker<'r, T, R>;

		#[inline]
		fn walk_revisioned<'r, R: BorrowedReader>(
			reader: &'r mut R,
		) -> Result<Self::Walker<'r, R>, Error> {
			SeqWalker::new(reader)
		}
	}
}
