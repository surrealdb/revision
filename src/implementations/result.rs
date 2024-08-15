use super::super::Error;
use super::super::Revisioned;

impl<E: Revisioned, T: Revisioned> Revisioned for Result<T, E> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		match self {
			Ok(v) => {
				0u32.serialize_revisioned(writer)?;
				v.serialize_revisioned(writer)
			}
			Err(e) => {
				1u32.serialize_revisioned(writer)?;
				e.serialize_revisioned(writer)
			}
		}
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let variant = u32::deserialize_revisioned(reader)?;
		match variant {
			0 => Ok(Ok(T::deserialize_revisioned(reader)
				.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))?)),
			1 => Ok(Err(E::deserialize_revisioned(reader)
				.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))?)),
			_ => Err(Error::Deserialize("Unknown variant index".to_string())),
		}
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::Revisioned;

	#[test]
	fn test_result_ok() {
		let val: Result<bool, String> = Ok(true);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 2);
		let out = <Result<bool, String> as Revisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_result_err() {
		let val: Result<bool, String> = Err("some error".into());
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 12);
		let out = <Result<bool, String> as Revisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}
}
