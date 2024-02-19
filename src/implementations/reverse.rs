use super::super::Error;
use super::super::Revisioned;
use std::cmp::Reverse;

impl<T> Revisioned for Reverse<T>
where
	T: Revisioned,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.0.serialize_revisioned(writer)
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok(Reverse(T::deserialize_revisioned(reader)?))
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::Revisioned;
	use std::cmp::Reverse;

	#[test]
	fn test_reverse() {
		let val: Reverse<u32> = Reverse(u32::MAX);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 5);
		let out =
			<Reverse<u32> as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}
}
