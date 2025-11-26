mod arc;
pub mod arrays;
pub mod bound;
pub mod boxes;
pub mod bytes;
pub mod chrono;
pub mod collections;
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
pub mod tuple;
pub mod uuid;
pub mod vecs;
pub mod wrapping;

#[cfg(test)]
#[track_caller]
pub fn assert_bincode_compat<T>(v: &T)
where
	T: serde::Serialize + crate::SerializeRevisioned,
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
