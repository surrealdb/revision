#![cfg(feature = "geo")]

use super::super::Error;
use super::super::Revisioned;
use bincode::Options;

impl Revisioned for geo::Point {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.reject_trailing_bytes()
			.serialize_into(writer, self)
			.map_err(|ref err| Error::Serialize(format!("{:?}", err)))
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.allow_trailing_bytes()
			.deserialize_from(reader)
			.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
	}

	fn revision() -> u16 {
		1
	}
}

impl Revisioned for geo::LineString {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.allow_trailing_bytes()
			.serialize_into(writer, self)
			.map_err(|ref err| Error::Serialize(format!("{:?}", err)))
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.allow_trailing_bytes()
			.deserialize_from(reader)
			.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
	}

	fn revision() -> u16 {
		1
	}
}

impl Revisioned for geo::Polygon {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.allow_trailing_bytes()
			.serialize_into(writer, self)
			.map_err(|ref err| Error::Serialize(format!("{:?}", err)))
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.allow_trailing_bytes()
			.deserialize_from(reader)
			.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
	}

	fn revision() -> u16 {
		1
	}
}

impl Revisioned for geo::MultiPoint {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.allow_trailing_bytes()
			.serialize_into(writer, self)
			.map_err(|ref err| Error::Serialize(format!("{:?}", err)))
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.allow_trailing_bytes()
			.deserialize_from(reader)
			.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
	}

	fn revision() -> u16 {
		1
	}
}

impl Revisioned for geo::MultiLineString {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.allow_trailing_bytes()
			.serialize_into(writer, self)
			.map_err(|ref err| Error::Serialize(format!("{:?}", err)))
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.allow_trailing_bytes()
			.deserialize_from(reader)
			.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
	}

	fn revision() -> u16 {
		1
	}
}

impl Revisioned for geo::MultiPolygon {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.allow_trailing_bytes()
			.serialize_into(writer, self)
			.map_err(|ref err| Error::Serialize(format!("{:?}", err)))
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.allow_trailing_bytes()
			.deserialize_from(reader)
			.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
	}

	fn revision() -> u16 {
		1
	}
}
