//! Defines a generic trait for version tolerant serialization and deserialization
//! and implements it for primitive data types using the `bincode` format.
//!
//! The `Revisioned` trait is automatically implemented for the following primitives:
//! u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char,
//! String, Vec<T>, Arrays up to 32 elements, Option<T>, Box<T>, Bound<T>, Wrapping<T>,
//! (A, B), (A, B, C), (A, B, C, D), (A, B, C, D, E), Duration, HashMap<K, V>,
//! BTreeMap<K, V>, Decimal, regex::Regex, uuid::Uuid, chrono::DateTime<Utc>,
//! geo::Point, geo::LineString geo::Polygon, geo::MultiPoint, geo::MultiLineString,
//! and geo::MultiPolygon.

pub mod error;
pub mod implementations;

pub use crate::error::Error;
pub use derive::revisioned;
pub use derive::Revisioned;

use std::any::TypeId;
use std::io::{Read, Write};

/// Trait that provides an interface for version aware serialization and deserialization.
///
/// Example implementation
/// ```
/// use revision::Error;
/// use revision::Revisioned;
///
/// struct MyType<T>(T);
///
/// impl<T> Revisioned for MyType<T>
/// where
///     T: Revisioned,
/// {
///     #[inline]
///     fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
///         self.0.serialize_revisioned(writer)
///     }
///
///     #[inline]
///     fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
///         Ok(MyType(T::deserialize_revisioned(reader)?))
///     }
///
///     fn revision() -> u16 {
///         1
///     }
/// }
/// ```
pub trait Revisioned {
	/// Returns the current revision of this type.
	fn revision() -> u16;
	/// Serializes the struct using the specficifed `writer`.
	fn serialize_revisioned<W: Write>(&self, w: &mut W) -> Result<(), Error>;
	/// Deserializes a new instance of the struct from the specficifed `reader`.
	fn deserialize_revisioned<R: Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized;
	/// Returns the type id of this type.
	fn type_id() -> std::any::TypeId
	where
		Self: 'static,
	{
		TypeId::of::<Self>()
	}
}
