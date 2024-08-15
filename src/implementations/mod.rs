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
