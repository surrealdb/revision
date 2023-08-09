use super::super::Error;
use super::super::Revisioned;
use bincode::Options;

macro_rules! impl_revisioned {
	($ty:ident) => {
		impl Revisioned for $ty {
			#[inline]
			fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
				bincode::options()
					.with_no_limit()
					.with_little_endian()
					.with_varint_encoding()
					.reject_trailing_bytes()
					.serialize_into(writer, self)
					.map_err(|ref err| Error::Serialize(format!("{:?}", err)))
			}

			#[inline]
			fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error>
			where
				Self: Sized,
			{
				bincode::options()
					.with_no_limit()
					.with_little_endian()
					.with_varint_encoding()
					.reject_trailing_bytes()
					.deserialize_from(reader)
					.map_err(|ref err| Error::Deserialize(format!("{:?}", err)))
			}

			fn revision() -> u16 {
				1
			}
		}
	};
}

impl_revisioned!(bool);
impl_revisioned!(isize);
impl_revisioned!(i8);
impl_revisioned!(i16);
impl_revisioned!(i32);
impl_revisioned!(i64);
impl_revisioned!(i128);
impl_revisioned!(usize);
impl_revisioned!(u8);
impl_revisioned!(u16);
impl_revisioned!(u32);
impl_revisioned!(u64);
impl_revisioned!(u128);
impl_revisioned!(f32);
impl_revisioned!(f64);
impl_revisioned!(char);

#[cfg(test)]
mod tests {

	use super::Revisioned;

	#[test]
	fn test_bool() {
		let val = true;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 1);
		let out = <bool as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_isize() {
		let val = isize::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 9);
		let out = <isize as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_i8() {
		let val = i8::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 1);
		let out = <i8 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_i16() {
		let val = i16::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 3);
		let out = <i16 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_i32() {
		let val = i32::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 5);
		let out = <i32 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_i64() {
		let val = i64::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 9);
		let out = <i64 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_i128() {
		let val = i128::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 17);
		let out = <i128 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_usize() {
		let val = usize::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 9);
		let out = <usize as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_u8() {
		let val = u8::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 1);
		let out = <u8 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_u16() {
		let val = u16::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 3);
		let out = <u16 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_u32() {
		let val = u32::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 5);
		let out = <u32 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_u64() {
		let val = u64::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 9);
		let out = <u64 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_u128() {
		let val = u128::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 17);
		let out = <u128 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_f32() {
		let val = f32::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 4);
		let out = <f32 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_f64() {
		let val = f64::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 8);
		let out = <f64 as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_char() {
		let val = char::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 4);
		let out = <char as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}
}
