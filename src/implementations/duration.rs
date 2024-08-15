use super::super::Error;
use super::super::Revisioned;
use std::time::Duration;

impl Revisioned for Duration {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.as_secs().serialize_revisioned(writer)?;
		self.subsec_nanos().serialize_revisioned(writer)
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let secs = u64::deserialize_revisioned(reader)?;
		let nanos = u32::deserialize_revisioned(reader)?;
		Ok(Duration::new(secs, nanos))
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {
	use super::Duration;
	use super::Revisioned;
	use bincode::Options as _;

	#[test]
	fn test_string() {
		let val = Duration::from_secs(604800);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 6);
		let out = <Duration as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn bincode_compat() {
		fn assert_compat(d: Duration) {
			let bincode = bincode::options()
				.with_no_limit()
				.with_little_endian()
				.with_varint_encoding()
				.reject_trailing_bytes()
				.serialize(&d)
				.unwrap();

			let mut revision = Vec::new();
			d.serialize_revisioned(&mut revision).unwrap();

			assert_eq!(revision, bincode)
		}

		assert_compat(Duration::ZERO);
		assert_compat(Duration::MAX);
		assert_compat(Duration::new(u64::MAX, 0));
		assert_compat(Duration::new(0, 999_999_999));
	}
}
