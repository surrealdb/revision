use super::super::Error;
use super::super::Revisioned;

impl<A, B> Revisioned for (A, B)
where
	A: Revisioned,
	B: Revisioned,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.0.serialize_revisioned(writer)?;
		self.1.serialize_revisioned(writer)?;
		Ok(())
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok((A::deserialize_revisioned(reader)?, B::deserialize_revisioned(reader)?))
	}

	fn revision() -> u16 {
		1
	}
}

impl<A, B, C> Revisioned for (A, B, C)
where
	A: Revisioned,
	B: Revisioned,
	C: Revisioned,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.0.serialize_revisioned(writer)?;
		self.1.serialize_revisioned(writer)?;
		self.2.serialize_revisioned(writer)?;
		Ok(())
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok((
			A::deserialize_revisioned(reader)?,
			B::deserialize_revisioned(reader)?,
			C::deserialize_revisioned(reader)?,
		))
	}

	fn revision() -> u16 {
		1
	}
}

impl<A, B, C, D> Revisioned for (A, B, C, D)
where
	A: Revisioned,
	B: Revisioned,
	C: Revisioned,
	D: Revisioned,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.0.serialize_revisioned(writer)?;
		self.1.serialize_revisioned(writer)?;
		self.2.serialize_revisioned(writer)?;
		self.3.serialize_revisioned(writer)?;
		Ok(())
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok((
			A::deserialize_revisioned(reader)?,
			B::deserialize_revisioned(reader)?,
			C::deserialize_revisioned(reader)?,
			D::deserialize_revisioned(reader)?,
		))
	}

	fn revision() -> u16 {
		1
	}
}

impl<A, B, C, D, E> Revisioned for (A, B, C, D, E)
where
	A: Revisioned,
	B: Revisioned,
	C: Revisioned,
	D: Revisioned,
	E: Revisioned,
{
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.0.serialize_revisioned(writer)?;
		self.1.serialize_revisioned(writer)?;
		self.2.serialize_revisioned(writer)?;
		self.3.serialize_revisioned(writer)?;
		self.4.serialize_revisioned(writer)?;
		Ok(())
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		Ok((
			A::deserialize_revisioned(reader)?,
			B::deserialize_revisioned(reader)?,
			C::deserialize_revisioned(reader)?,
			D::deserialize_revisioned(reader)?,
			E::deserialize_revisioned(reader)?,
		))
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {

	use super::Revisioned;

	#[test]
	fn test_tuple_2() {
		let val = (String::from("test"), true);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 6);
		let out =
			<(String, bool) as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_tuple_3() {
		let val = (String::from("test"), true, 1419247293847192847.13947134978139487);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 14);
		let out = <(String, bool, f64) as Revisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_tuple_4() {
		let val = (String::from("test"), true, 1419247293847192847.13947134978139487, Some('t'));
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out = <(String, bool, f64, Option<char>) as Revisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_tuple_5() {
		let val = (
			String::from("test"),
			true,
			1419247293847192847.13947134978139487,
			Some('t'),
			vec![4u8, 19u8, 133u8],
		);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 20);
		let out =
			<(String, bool, f64, Option<char>, Vec<u8>) as Revisioned>::deserialize_revisioned(
				&mut mem.as_slice(),
			)
			.unwrap();
		assert_eq!(val, out);
	}
}
