use super::super::Error;
use super::super::Revisioned;
use ordered_float::{Float, NotNan};

impl<T> Revisioned for NotNan<T>
where
	T: Revisioned + Float,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.as_ref().serialize_revisioned(writer)
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok(NotNan::new(
            T::deserialize_revisioned(reader)?).map_err(|e| 
            Error::Deserialize(format!("{:?}", e))
        )?)
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::Revisioned;
    use ordered_float::NotNan;

	#[test]
	fn test_wrapping() {
		let val: NotNan<f32> = NotNan::new(f32::MAX).unwrap();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 4);
		let out =
			<NotNan<f32> as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}
}
