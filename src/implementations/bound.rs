use super::super::Error;
use super::super::Revisioned;
use std::ops::Bound;

impl<T: Revisioned> Revisioned for Bound<T> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		match *self {
			Bound::Unbounded => 0u32.serialize_revisioned(writer),
			Bound::Included(ref value) => {
				1u32.serialize_revisioned(writer)?;
				value.serialize_revisioned(writer)
			}
			Bound::Excluded(ref value) => {
				2u32.serialize_revisioned(writer)?;
				value.serialize_revisioned(writer)
			}
		}
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let variant = u32::deserialize_revisioned(reader)?;
		match variant {
			0 => Ok(Bound::Unbounded),
			1 => Ok(Bound::Included(
				T::deserialize_revisioned(reader)
					.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))?,
			)),
			2 => Ok(Bound::Excluded(
				T::deserialize_revisioned(reader)
					.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))?,
			)),
			_ => Err(Error::Deserialize("Unknown variant index".to_string())),
		}
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::Bound;
	use super::Revisioned;

	#[test]
	fn test_bound_unbounded() {
		let val: Bound<String> = Bound::Unbounded;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 1);
		let out =
			<Bound<String> as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_bound_excluded() {
		let val: Bound<String> = Bound::Excluded(String::from("this is a test"));
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out =
			<Bound<String> as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_bound_included() {
		let val: Bound<String> = Bound::Included(String::from("this is a test"));
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out =
			<Bound<String> as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}
}
