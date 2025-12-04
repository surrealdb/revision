#![cfg(feature = "geo")]

use super::super::Error;
use super::super::{DeserializeRevisioned, Revisioned, SerializeRevisioned};
use geo::{Coord, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon};
use std::io::{Read, Write};

impl SerializeRevisioned for Coord {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.x.serialize_revisioned(writer)?;
		self.y.serialize_revisioned(writer)
	}
}

impl DeserializeRevisioned for Coord {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let x = f64::deserialize_revisioned(reader)?;
		let y = f64::deserialize_revisioned(reader)?;
		Ok(Self {
			x,
			y,
		})
	}
}

impl Revisioned for Coord {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for Point {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.0.serialize_revisioned(writer)
	}
}

impl DeserializeRevisioned for Point {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok(Self(DeserializeRevisioned::deserialize_revisioned(reader)?))
	}
}

impl Revisioned for Point {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for LineString {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.0.serialize_revisioned(writer)
	}
}

impl DeserializeRevisioned for LineString {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok(Self(DeserializeRevisioned::deserialize_revisioned(reader)?))
	}
}

impl Revisioned for LineString {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for Polygon {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.exterior().serialize_revisioned(writer)?;
		self.interiors().len().serialize_revisioned(writer)?;
		for interior in self.interiors() {
			interior.serialize_revisioned(writer)?;
		}
		Ok(())
	}
}

impl DeserializeRevisioned for Polygon {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok(Self::new(
			DeserializeRevisioned::deserialize_revisioned(reader)?,
			DeserializeRevisioned::deserialize_revisioned(reader)?,
		))
	}
}

impl Revisioned for Polygon {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for MultiPoint {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.0.serialize_revisioned(writer)
	}
}

impl DeserializeRevisioned for MultiPoint {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok(Self(DeserializeRevisioned::deserialize_revisioned(reader)?))
	}
}

impl Revisioned for MultiPoint {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for MultiLineString {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.0.serialize_revisioned(writer)
	}
}

impl DeserializeRevisioned for MultiLineString {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok(Self(DeserializeRevisioned::deserialize_revisioned(reader)?))
	}
}

impl Revisioned for MultiLineString {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for MultiPolygon {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.0.serialize_revisioned(writer)
	}
}

impl DeserializeRevisioned for MultiPolygon {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok(Self(DeserializeRevisioned::deserialize_revisioned(reader)?))
	}
}

impl Revisioned for MultiPolygon {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// --------------------------------------------------
// Optimized implementation for Vec<Coord>
// --------------------------------------------------

impl SerializeRevisioned for Vec<Coord> {
	#[inline]
	fn serialize_revisioned<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.len().serialize_revisioned(writer)?;
		if self.is_empty() {
			return Ok(());
		}
		#[cfg(target_endian = "little")]
		{
			// SAFETY: Coord contains two f64 fields. On little-endian platforms, the memory
			// layout matches the wire format. We cast *const Coord to *const u8, which is
			// always safe as u8 has no alignment requirement. We only read from the slice.
			let bytes = unsafe {
				std::slice::from_raw_parts(
					self.as_ptr() as *const u8,
					self.len() * std::mem::size_of::<Coord>(),
				)
			};
			writer.write_all(bytes).map_err(Error::Io)
		}
		#[cfg(target_endian = "big")]
		{
			for v in self {
				writer.write_all(&v.x.to_le_bytes()).map_err(Error::Io)?;
				writer.write_all(&v.y.to_le_bytes()).map_err(Error::Io)?;
			}
			Ok(())
		}
	}
}

impl DeserializeRevisioned for Vec<Coord> {
	#[inline]
	fn deserialize_revisioned<R: Read>(reader: &mut R) -> Result<Self, Error> {
		let len = usize::deserialize_revisioned(reader)?;
		if len == 0 {
			return Ok(Vec::new());
		}
		#[cfg(target_endian = "little")]
		{
			let byte_len =
				len.checked_mul(std::mem::size_of::<Coord>()).ok_or(Error::IntegerOverflow)?;
			// Allocate Vec<Coord> first to ensure proper alignment (Coord requires 8-byte alignment).
			// Then cast down to *mut u8 for reading (u8 has no alignment requirement).
			let mut vec: Vec<Coord> = vec![Coord::default(); len];
			// SAFETY: We cast *mut Coord to *mut u8, which is safe as u8 has no alignment
			// requirement. The slice length matches the allocated capacity. All f64 bit
			// patterns are valid, and on little-endian the wire format matches memory layout.
			unsafe {
				let byte_slice =
					std::slice::from_raw_parts_mut(vec.as_mut_ptr().cast::<u8>(), byte_len);
				reader.read_exact(byte_slice).map_err(Error::Io)?;
			}
			Ok(vec)
		}
		#[cfg(target_endian = "big")]
		{
			let mut vec = Vec::with_capacity(len);
			for _ in 0..len {
				let x = f64::deserialize_revisioned(reader)?;
				let y = f64::deserialize_revisioned(reader)?;
				vec.push(Coord {
					x,
					y,
				});
			}
			Ok(vec)
		}
	}
}

impl Revisioned for Vec<Coord> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// --------------------------------------------------
// Optimized implementation for Vec<Point>
// --------------------------------------------------

impl SerializeRevisioned for Vec<Point> {
	#[inline]
	fn serialize_revisioned<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.len().serialize_revisioned(writer)?;
		if self.is_empty() {
			return Ok(());
		}
		#[cfg(target_endian = "little")]
		{
			// SAFETY: Point wraps Coord which contains two f64 fields. On little-endian
			// platforms, the memory layout matches the wire format. We cast *const Point
			// to *const u8, which is always safe as u8 has no alignment requirement.
			let bytes = unsafe {
				std::slice::from_raw_parts(
					self.as_ptr() as *const u8,
					self.len() * std::mem::size_of::<Point>(),
				)
			};
			writer.write_all(bytes).map_err(Error::Io)
		}
		#[cfg(target_endian = "big")]
		{
			for v in self {
				writer.write_all(&v.0.x.to_le_bytes()).map_err(Error::Io)?;
				writer.write_all(&v.0.y.to_le_bytes()).map_err(Error::Io)?;
			}
			Ok(())
		}
	}
}

impl DeserializeRevisioned for Vec<Point> {
	#[inline]
	fn deserialize_revisioned<R: Read>(reader: &mut R) -> Result<Self, Error> {
		let len = usize::deserialize_revisioned(reader)?;
		if len == 0 {
			return Ok(Vec::new());
		}
		#[cfg(target_endian = "little")]
		{
			let byte_len =
				len.checked_mul(std::mem::size_of::<Point>()).ok_or(Error::IntegerOverflow)?;
			// Allocate Vec<Point> first to ensure proper alignment (Point requires 8-byte alignment).
			// Then cast down to *mut u8 for reading (u8 has no alignment requirement).
			let mut vec: Vec<Point> = vec![Point::default(); len];
			// SAFETY: We cast *mut Point to *mut u8, which is safe as u8 has no alignment
			// requirement. The slice length matches the allocated capacity. All f64 bit
			// patterns are valid, and on little-endian the wire format matches memory layout.
			unsafe {
				let byte_slice =
					std::slice::from_raw_parts_mut(vec.as_mut_ptr().cast::<u8>(), byte_len);
				reader.read_exact(byte_slice).map_err(Error::Io)?;
			}
			Ok(vec)
		}
		#[cfg(target_endian = "big")]
		{
			let mut vec = Vec::with_capacity(len);
			for _ in 0..len {
				let x = f64::deserialize_revisioned(reader)?;
				let y = f64::deserialize_revisioned(reader)?;
				vec.push(Point::new(x, y));
			}
			Ok(vec)
		}
	}
}

impl Revisioned for Vec<Point> {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

crate::impl_revisioned_vec!(LineString);
crate::impl_revisioned_vec!(Polygon);
crate::impl_revisioned_vec!(MultiPoint);
crate::impl_revisioned_vec!(MultiLineString);
crate::impl_revisioned_vec!(MultiPolygon);

#[cfg(test)]
mod test {
	use std::cell::Cell;

	use super::*;

	use crate::implementations::assert_bincode_compat;

	pub struct Rng(pub Cell<u64>);

	impl Rng {
		pub fn next(&self) -> u64 {
			let mut x = self.0.get();
			x ^= x << 13;
			x ^= x >> 7;
			x ^= x << 17;
			self.0.set(x);
			x
		}

		pub fn next_f64(&self) -> f64 {
			f64::from_bits(self.next())
		}

		pub fn next_point(&self) -> Point {
			Point::new(self.next_f64(), self.next_f64())
		}

		pub fn next_points(&self, len: usize) -> Vec<Point> {
			(0..len).map(|_| self.next_point()).collect()
		}

		pub fn next_coords(&self, len: usize) -> Vec<Coord> {
			(0..len).map(|_| self.next_point().0).collect()
		}
	}

	#[test]
	fn compat() {
		let rng = Rng(Cell::new(0x1fb931de31));

		let point_a = rng.next_point();
		let point_b = rng.next_point();
		assert_bincode_compat(&point_a);
		assert_bincode_compat(&point_b);

		let line_string = LineString(rng.next_coords(10));
		assert_bincode_compat(&line_string);

		let create_multi_line =
			|| (0..10).map(|_| LineString(rng.next_coords(10))).collect::<Vec<_>>();

		let create_polygon = || Polygon::new(LineString(rng.next_coords(10)), create_multi_line());

		let polygon = create_polygon();
		assert_bincode_compat(&polygon);

		let multi_point = MultiPoint(rng.next_points(10));
		assert_bincode_compat(&multi_point);

		let multi_line = MultiLineString(create_multi_line());
		assert_bincode_compat(&multi_line);

		let multi_polygon = MultiPolygon((0..10).map(|_| create_polygon()).collect());
		assert_bincode_compat(&multi_polygon);
	}
}
