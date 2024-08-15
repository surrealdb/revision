use std::path::PathBuf;

use super::super::Error;
use super::super::Revisioned;

impl Revisioned for PathBuf {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		match self.to_str() {
			Some(s) => {
				(s.len() as u64).serialize_revisioned(writer)?;
				writer.write_all(s.as_bytes()).map_err(Error::Io)?;
				Ok(())
			}
			None => Err(Error::InvalidPath),
		}
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let s = String::deserialize_revisioned(reader)?;
		Ok(PathBuf::from(s))
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use std::path::PathBuf;

	use super::Revisioned;

	#[test]
	fn test_pathbuf() {
		let val = PathBuf::from("/test/path/to/file.txt");
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 23);
		let out = <PathBuf as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}
}
