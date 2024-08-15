use std::io;

use crate::Error;

pub mod arrays;
pub mod bound;
pub mod boxes;
pub mod chrono;
pub mod cow;
pub mod decimal;
pub mod duration;
pub mod geo;
pub mod notnan;
pub mod option;
pub mod path;
pub mod primitives;
pub mod regex;
pub mod result;
pub mod reverse;
pub mod roaring;
pub mod string;
pub mod trees;
pub mod tuple;
pub mod uuid;
pub mod vecs;
pub mod wrapping;

pub fn unexpected_eof() -> Error {
	Error::Io(io::Error::new(io::ErrorKind::UnexpectedEof, ""))
}

pub fn read_buffer<const COUNT: usize, R: io::Read>(reader: &mut R) -> Result<[u8; COUNT], Error> {
	let mut buffer = [0u8; COUNT];
	let count = reader.read(&mut buffer).map_err(Error::Io)?;
	if count != COUNT {
		return Err(unexpected_eof());
	}
	Ok(buffer)
}

#[cfg(test)]
#[track_caller]
pub fn assert_bincode_compat<T>(v: &T)
where
	T: serde::Serialize + crate::Revisioned,
{
	use bincode::Options;

	let bincode = bincode::options()
		.with_no_limit()
		.with_little_endian()
		.with_varint_encoding()
		.reject_trailing_bytes()
		.serialize(&v)
		.unwrap();

	let mut revision = Vec::new();
	v.serialize_revisioned(&mut revision).unwrap();

	assert_eq!(revision, bincode)
}
