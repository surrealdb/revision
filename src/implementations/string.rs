use core::str;

use crate::{DeserializeRevisioned, Error, Revisioned, SerializeRevisioned};

use super::vecs::serialize_bytes;

impl SerializeRevisioned for String {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		serialize_bytes(self.as_bytes(), writer)
	}
}

impl DeserializeRevisioned for String {
	/// Reads the length-prefixed byte payload in a single bulk `read_exact`
	/// and validates it as UTF-8 in place, avoiding both the per-byte fallback
	/// when `specialised-vectors` is disabled and the `Take::read_to_end`
	/// overhead of the `Vec<u8>` specialised path.
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let len = usize::deserialize_revisioned(reader)?;
		if len == 0 {
			return Ok(String::new());
		}
		let mut buf: Vec<u8> = Vec::with_capacity(len);
		// SAFETY: `Vec::with_capacity(len)` guarantees capacity `>= len`, so
		// `from_raw_parts_mut(ptr, len)` yields a valid exclusive slice of
		// `len` (currently uninitialised) bytes. `read_exact` either fully
		// initialises the slice and returns `Ok`, in which case we commit
		// the length via `set_len`, or returns `Err`, in which case `?`
		// returns before `set_len` and `buf` is dropped with `len = 0`,
		// so no uninitialised memory is ever observed. `String::from_utf8`
		// then enforces UTF-8 validity before producing a `String`.
		unsafe {
			let slice = std::slice::from_raw_parts_mut(buf.as_mut_ptr(), len);
			reader.read_exact(slice).map_err(Error::Io)?;
			buf.set_len(len);
		}
		String::from_utf8(buf).map_err(|x| Error::Utf8Error(x.utf8_error()))
	}
}

impl Revisioned for String {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for str {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		serialize_bytes(self.as_bytes(), writer)
	}
}

impl Revisioned for str {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for char {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, w: &mut W) -> Result<(), Error> {
		let buffer = &mut [0u8; 4];
		w.write_all(self.encode_utf8(buffer).as_bytes()).map_err(Error::Io)
	}
}

impl DeserializeRevisioned for char {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(r: &mut R) -> Result<Self, Error> {
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

impl Revisioned for char {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

static CHAR_LENGTH: [u8; 256] = const {
	let mut r = [0u8; 256];
	let mut i = 0;
	while i < 256 {
		if i & 0b1000_0000 == 0 {
			r[i] = 1;
		} else if i & 0b1110_0000 == 0b1100_0000 {
			r[i] = 2;
		} else if i & 0b1111_0000 == 0b1110_0000 {
			r[i] = 3;
		} else if i & 0b1111_1000 == 0b1111_0000 {
			r[i] = 4;
		}

		i += 1;
	}

	r
};

#[cfg(test)]
mod tests {

	use super::*;

	use crate::implementations::assert_bincode_compat;

	#[test]
	fn test_string() {
		let val = String::from("this is a test");
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 15);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 22);
		let out =
			<String as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_char() {
		let char = '𐃌';
		let mut mem = Vec::new();
		char.serialize_revisioned(&mut mem).unwrap();
		let out = DeserializeRevisioned::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(char, out);
	}

	#[test]
	fn bincode_compat_char() {
		assert_bincode_compat(&char::MAX);
		assert_bincode_compat(&'\0');
		assert_bincode_compat(&'z');
		assert_bincode_compat(&'0');
		// in the 0x7F - 0x07FF range
		assert_bincode_compat(&'ʘ');
		// in the 0x7FF - 0xFFFF range
		assert_bincode_compat(&'ꚸ');
		// in the 0xFFFF - 0x10FFFF range
		assert_bincode_compat(&'𐃌');
	}

	#[test]
	fn bincode_compat_string() {
		assert_bincode_compat(&char::MAX.to_string());
		assert_bincode_compat(&'\0'.to_string());
		assert_bincode_compat(&'z'.to_string());
		assert_bincode_compat(&'0'.to_string());
		// in the 0x7F - 0x07FF range
		assert_bincode_compat(&'ʘ'.to_string());
		// in the 0x7FF - 0xFFFF range
		assert_bincode_compat(&'ꚸ'.to_string());
		// in the 0xFFFF - 0x10FFFF range
		assert_bincode_compat(&'𐃌'.to_string());
	}
}
