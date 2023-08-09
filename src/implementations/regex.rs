use super::super::Error;
use super::super::Revisioned;
use bincode::Options;
use regex::Regex;
use std::borrow::Cow;

impl Revisioned for Regex {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.reject_trailing_bytes()
			.serialize_into(writer, self.as_str())
			.map_err(|ref err| Error::Serialize(format!("{:?}", err)))
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let s: Cow<str> = bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.reject_trailing_bytes()
			.deserialize_from(reader)
			.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))?;
		s.parse().map_err(|_| Error::Deserialize("invalid regex".to_string()))
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::Regex;
	use super::Revisioned;

	#[test]
	fn test_regex() {
		let val = Regex::new("/this ([a-z]+) a tes?/").unwrap();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 23);
		let out = <Regex as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val.as_str(), out.as_str());
	}
}
