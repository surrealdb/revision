#![cfg(feature = "roaring")]

use super::super::Error;
use super::super::Revisioned;
use roaring::{RoaringBitmap, RoaringTreemap};

impl Revisioned for RoaringTreemap {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.serialize_into(writer).map_err(|ref err| Error::Serialize(format!("{:?}", err)))
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Self::deserialize_from(reader).map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
	}

	fn revision() -> u16 {
		1
	}
}

impl Revisioned for RoaringBitmap {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.serialize_into(writer).map_err(|ref err| Error::Serialize(format!("{:?}", err)))
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Self::deserialize_from(reader).map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {
	use super::Revisioned;
	use bincode::Options;
	use rand::random;
	use roaring::{RoaringBitmap, RoaringTreemap};
	use std::time::SystemTime;

	#[test]
	fn test_roaring_treemap() {
		let val = RoaringTreemap::new();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 8);
		let out =
			<RoaringTreemap as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_roaring_bitmap() {
		let val = RoaringBitmap::new();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 8);
		let out =
			<RoaringBitmap as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_roaring_serialization_benchmark() {
		let mut val = RoaringTreemap::new();
		for i in 0..10000 {
			if random() {
				val.insert(i);
			}
		}
		// COLLECTING ELAPSED TIME AND SIZE

		//Bincode with default options is: Slower and bigger than direct serialization
		let bincode_elapsed;
		let bincode_size;
		{
			let mut mem: Vec<u8> = vec![];
			let t = SystemTime::now();
			bincode::serialize_into(&mut mem, &val).unwrap();
			bincode_elapsed = t.elapsed().unwrap();
			bincode_size = mem.len();
		}
		//Bincode with options is: As fast, but still bigger than direct serialization
		let bincode_options_elapsed;
		let bincode_options_size;
		{
			let mut mem: Vec<u8> = vec![];
			let t = SystemTime::now();
			bincode::options()
				.with_no_limit()
				.with_little_endian()
				.with_varint_encoding()
				.reject_trailing_bytes()
				.serialize_into(&mut mem, &val)
				.unwrap();
			bincode_options_elapsed = t.elapsed().unwrap();
			bincode_options_size = mem.len();
		}
		//Direct serialization  is : Faster and smaller
		let direct_elapsed;
		let direct_size;
		{
			let mut mem: Vec<u8> = vec![];
			let t = SystemTime::now();
			val.serialize_into(&mut mem).unwrap();
			direct_elapsed = t.elapsed().unwrap();
			direct_size = mem.len();
		}

		// ASSERTIONS

		println!("Bincode::default, Bincode::options, Direct, Ratio direct/bincode::options");
		// Direct is faster
		println!(
			"Elapsed - {} > {} > {} - {}",
			bincode_elapsed.as_micros(),
			bincode_options_elapsed.as_micros(),
			direct_elapsed.as_micros(),
			direct_elapsed.as_micros() as f32 / bincode_options_elapsed.as_micros() as f32
		);
		assert!(direct_elapsed < bincode_elapsed);
		assert!(
			(direct_elapsed.as_micros() as f32 / bincode_options_elapsed.as_micros() as f32) < 1.1
		);
		// Direct is smaller
		println!(
			"Size: {} > {} > {}  - {}",
			bincode_size,
			bincode_options_size,
			direct_size,
			direct_size as f32 / bincode_options_size as f32
		);
		assert!(direct_size < bincode_size);
		assert!(direct_size < bincode_options_size);
	}
}
