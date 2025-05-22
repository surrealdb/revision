#![cfg(feature = "regex")]

use super::super::Error;
use super::super::{DeserializeRevisioned, Revisioned, SerializeRevisioned};
use super::vecs::serialize_slice;
use regex::Regex;

impl SerializeRevisioned for Regex {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		serialize_slice(self.as_str().as_bytes(), writer)
	}
}

impl DeserializeRevisioned for Regex {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let s = String::deserialize_revisioned(reader)?;
		s.parse().map_err(|_| Error::Deserialize("invalid regex".to_string()))
	}
}

impl Revisioned for Regex {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_regex() {
		let val = Regex::new("/this ([a-z]+) a tes?/").unwrap();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 23);
		let out =
			<Regex as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val.as_str(), out.as_str());
	}
}
