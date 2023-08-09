use super::super::Error;
use super::super::Revisioned;
use bincode::Options;

impl Revisioned for String {
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
			.reject_trailing_bytes()
			.deserialize_from(reader)
			.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::Revisioned;

	#[test]
	fn test_string() {
		let val = String::from("this is a test");
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 15);
		let out = <String as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}
}
