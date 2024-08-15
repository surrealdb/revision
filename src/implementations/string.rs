use core::str;

use crate::{Error, Revisioned};

impl Revisioned for String {
	fn revision() -> u16 {
		1
	}

	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		(self.len() as u64).serialize_revisioned(writer)?;
		writer.write_all(self.as_bytes()).map_err(Error::Io)
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let len: usize =
			u64::deserialize_revisioned(reader)?.try_into().map_err(|_| Error::IntegerOverflow)?;
		let slice = vec![0u8; len];

		String::from_utf8(slice).map_err(|x| Error::Utf8Error(x.utf8_error()))
	}
}

impl Revisioned for char {
	fn revision() -> u16 {
		1
	}

	fn serialize_revisioned<W: std::io::Write>(&self, w: &mut W) -> Result<(), Error> {
		let buffer = &mut [0u8; 4];
		w.write_all(self.encode_utf8(buffer).as_bytes()).map_err(Error::Io)
	}

	fn deserialize_revisioned<R: std::io::Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		let mut buffer = [0u8; 4];
		r.read_exact(&mut buffer[..1]).map_err(Error::Io)?;

		let len = CHAR_LENGTH[buffer[0] as usize];
		if len == 0 {
			return Err(Error::InvalidCharEncoding);
		}

		r.read_exact(&mut buffer[1..(len as usize)]).map_err(Error::Io)?;

		str::from_utf8(&buffer[..(len as usize)])
			.map_err(|_| Error::InvalidCharEncoding)
			.map(|x| x.chars().next().unwrap())
	}
}

static CHAR_LENGTH: [u8; 256] = const {
	let mut r = [0u8; 256];
	let mut i = 0;
	while i < 256 {
		if i & 0b1000_0000 == 0 {
			r[i] = 1;
		} else if i & 0b1110_000 == 0b1100_0000 {
			r[i] = 2;
		} else if i & 0b1111_000 == 0b1110_0000 {
			r[i] = 3;
		} else if i & 0b1111_100 == 0b1111_0000 {
			r[i] = 4;
		}

		i += 1;
	}

	r
};

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
