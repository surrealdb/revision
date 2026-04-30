//! Defines a generic trait for version tolerant serialization and deserialization
//! and implements it for primitive data types using the `bincode` format.
//!
//! The `Revisioned` trait is automatically implemented for the following primitives:
//! u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char,
//! str, String, Vec<T>, Arrays up to 32 elements, Option<T>, Box<T>, Bound<T>, Range<T>, Wrapping<T>,
//! (A, B), (A, B, C), (A, B, C, D), (A, B, C, D, E), Duration, SystemTime, HashMap<K, V>,
//! BTreeMap<K, V>, Result<T, E>, Cow<'_, T>, Decimal, regex::Regex, uuid::Uuid, chrono::Duration,
//! chrono::DateTime<Utc>, geo::Point, geo::LineString geo::Polygon, geo::MultiPoint,
//! geo::MultiLineString, and geo::MultiPolygon.

pub mod error;
pub mod implementations;

#[cfg(feature = "skip")]
pub mod slice_reader;

pub use crate::error::Error;
pub use revision_derive::revisioned;

use std::any::TypeId;
use std::io::{Read, Write};

#[cfg(feature = "skip")]
pub use slice_reader::{SliceReader, advance_read};

#[cfg(feature = "skip")]
pub trait SkipRevisioned: Revisioned {
	fn skip_revisioned<R: Read>(r: &mut R) -> Result<(), Error>;
}

#[cfg(feature = "skip")]
pub trait SkipCheckRevisioned: Revisioned {
	fn skip_check_revisioned<R: Read>(r: &mut R) -> Result<(), Error>;
}

#[cfg(feature = "skip")]
#[inline]
pub fn skip_revisioned<T: SkipRevisioned, R: Read>(r: &mut R) -> Result<(), Error> {
	T::skip_revisioned(r)
}

#[cfg(feature = "skip")]
#[inline]
pub fn skip_check_revisioned<T: SkipCheckRevisioned, R: Read>(r: &mut R) -> Result<(), Error> {
	T::skip_check_revisioned(r)
}

#[cfg(feature = "skip")]
#[inline]
pub fn skip_slice<T: SkipRevisioned>(bytes: &[u8]) -> Result<usize, Error> {
	let mut cursor = bytes;
	let start = cursor.len();
	<T as SkipRevisioned>::skip_revisioned(&mut cursor)?;
	Ok(start - cursor.len())
}

#[cfg(feature = "skip")]
#[inline]
pub fn skip_check_slice<T: SkipCheckRevisioned>(bytes: &[u8]) -> Result<usize, Error> {
	let mut cursor = bytes;
	let start = cursor.len();
	<T as SkipCheckRevisioned>::skip_check_revisioned(&mut cursor)?;
	Ok(start - cursor.len())
}

pub mod prelude {
	pub use crate::{DeserializeRevisioned, Revisioned, SerializeRevisioned, revisioned};
	#[cfg(feature = "skip")]
	pub use crate::{
		SkipCheckRevisioned, SkipRevisioned, skip_check_revisioned, skip_check_slice,
		skip_revisioned, skip_slice,
	};
}

/// Trait that provides an interface for version aware serialization and deserialization.
///
/// Example implementation
/// ```
/// use revision::Error;
/// use revision::prelude::*;
///
/// struct MyType<T>(T);
///
/// impl<T> SerializeRevisioned for MyType<T>
/// where
///    T: SerializeRevisioned,
/// {
///    #[inline]
///   fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
///       self.0.serialize_revisioned(writer)
///   }
/// }
///
/// impl<T> DeserializeRevisioned for MyType<T>
/// where
///    T: DeserializeRevisioned,
/// {
///   #[inline]
///   fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
///       Ok(MyType(T::deserialize_revisioned(reader)?))
///   }
/// }
///
/// impl<T> Revisioned for MyType<T>
/// where
///     T: Revisioned,
/// {
///     fn revision() -> u16 {
///         1
///     }
/// }
/// ```
pub trait Revisioned {
	/// Returns the current revision of this type.
	fn revision() -> u16;
	/// Returns the type id of this type.
	#[inline]
	fn type_id() -> std::any::TypeId
	where
		Self: 'static,
	{
		TypeId::of::<Self>()
	}
}

pub trait SerializeRevisioned: Revisioned {
	/// Serializes the struct using the specified `writer`.
	fn serialize_revisioned<W: Write>(&self, w: &mut W) -> Result<(), Error>;
}

pub trait DeserializeRevisioned: Revisioned {
	/// Deserializes a new instance of the struct from the specified `reader`.
	fn deserialize_revisioned<R: Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized;
}

/// Deserialize a revisioned type from a reader
#[inline]
pub fn from_reader<R, T>(rdr: &mut R) -> Result<T, Error>
where
	R: Read,
	T: DeserializeRevisioned,
{
	DeserializeRevisioned::deserialize_revisioned(rdr)
}

/// Deserialize a revisioned type from a slice of bytes
#[inline]
pub fn from_slice<T>(mut bytes: &[u8]) -> Result<T, Error>
where
	T: DeserializeRevisioned,
{
	DeserializeRevisioned::deserialize_revisioned(&mut bytes)
}

/// Serialize a revisioned type into a vec of bytes
#[inline]
pub fn to_writer<W, T>(writer: &mut W, t: &T) -> Result<(), Error>
where
	W: Write,
	T: SerializeRevisioned,
{
	SerializeRevisioned::serialize_revisioned(t, writer)
}

/// Serialize a revisioned type into a vec of bytes
#[inline]
pub fn to_vec<T>(t: &T) -> Result<Vec<u8>, Error>
where
	T: SerializeRevisioned,
{
	let mut res = Vec::new();
	SerializeRevisioned::serialize_revisioned(t, &mut res)?;
	Ok(res)
}
