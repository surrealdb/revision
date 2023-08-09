use super::super::Error;
use super::super::Revisioned;

impl<T> Revisioned for Option<T>
where
	T: Revisioned,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		match self {
			Some(value) => {
				1u8.serialize_revisioned(writer)?;
				value.serialize_revisioned(writer)
			}
			None => 0u8.serialize_revisioned(writer),
		}
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let option = u8::deserialize_revisioned(reader)?;
		match option {
			0u8 => Ok(None),
			1u8 => Ok(Some(T::deserialize_revisioned(reader)?)),
			value => Err(Error::Deserialize(format!("Invalid option value {}", value))),
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
	fn test_option_none() {
		let val: Option<String> = None;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 1);
		let out =
			<Option<String> as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_option_some() {
		let val: Option<String> = Some(String::from("this is a test"));
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out =
			<Option<String> as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}
}
