use std::io;

use super::super::Revisioned;
use crate::Error;

pub fn read_buffer<const COUNT: usize, R: io::Read>(reader: &mut R) -> Result<[u8; COUNT], Error> {
	let mut buffer = [0u8; COUNT];
	reader.read_exact(&mut buffer).map_err(Error::Io)?;
	Ok(buffer)
}

/// zigzag encode a 64bit integer
fn zigzag_64(v: i64) -> u64 {
	(v >> (i64::BITS - 1)) as u64 ^ ((v as u64) << 1)
}

/// undo zigzag encoding
fn gazgiz_64(v: u64) -> i64 {
	(v >> 1) as i64 ^ -((v & 1) as i64)
}

/// zigzag encode a 128bit integer
fn zigzag_128(v: i128) -> u128 {
	(v >> (i128::BITS - 1)) as u128 ^ ((v as u128) << 1)
}

/// undo zigzag encoding
fn gazgiz_128(v: u128) -> i128 {
	(v >> 1) as i128 ^ -((v & 1) as i128)
}

fn encode_u64<W>(writer: &mut W, i: u64) -> Result<(), Error>
where
	W: io::Write,
{
	if i < 251 {
		writer.write_all(&[i as u8]).map_err(Error::Io)?;
	} else if i < (1 << 16) {
		let bytes = (i as u16).to_le_bytes();
		writer.write_all(&[251, bytes[0], bytes[1]]).map_err(Error::Io)?;
	} else if i < (1 << 32) {
		let bytes = (i as u32).to_le_bytes();
		writer.write_all(&[252, bytes[0], bytes[1], bytes[2], bytes[3]]).map_err(Error::Io)?;
	} else {
		let bytes = i.to_le_bytes();
		writer
			.write_all(&[
				253, bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
			])
			.map_err(Error::Io)?;
	}

	Ok(())
}

fn encode_u128<W>(writer: &mut W, i: u128) -> Result<(), Error>
where
	W: io::Write,
{
	if i < 251 {
		writer.write_all(&[i as u8]).map_err(Error::Io)?;
	} else if i < (1 << 16) {
		let bytes = (i as u16).to_le_bytes();
		writer.write_all(&[251, bytes[0], bytes[1]]).map_err(Error::Io)?;
	} else if i < (1 << 32) {
		let bytes = (i as u32).to_le_bytes();
		writer.write_all(&[252, bytes[0], bytes[1], bytes[2], bytes[3]]).map_err(Error::Io)?;
	} else if i < (1 << 64) {
		let bytes = (i as u64).to_le_bytes();
		writer
			.write_all(&[
				253, bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
			])
			.map_err(Error::Io)?;
	} else {
		let bytes = i.to_le_bytes();
		let bytes = [
			254, bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
			bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
		];
		writer.write_all(&bytes).map_err(Error::Io)?;
	}

	Ok(())
}
fn decode_u64<R>(reader: &mut R) -> Result<u64, Error>
where
	R: io::Read,
{
	let b = read_buffer::<1, _>(reader)?;
	let v = match b[0] {
		251 => {
			let b = read_buffer::<2, _>(reader)?;
			u16::from_le_bytes(b) as u64
		}
		252 => {
			let b = read_buffer::<4, _>(reader)?;
			u32::from_le_bytes(b) as u64
		}
		253 => {
			let b = read_buffer::<8, _>(reader)?;
			u64::from_le_bytes(b)
		}
		254 => return Err(Error::IntegerOverflow),
		255 => return Err(Error::InvalidIntegerEncoding),
		x => x as u64,
	};
	Ok(v)
}

fn decode_u128<R>(reader: &mut R) -> Result<u128, Error>
where
	R: io::Read,
{
	let b = read_buffer::<1, _>(reader)?;
	let v = match b[0] {
		251 => {
			let b = read_buffer::<2, _>(reader)?;
			u16::from_le_bytes(b) as u128
		}
		252 => {
			let b = read_buffer::<4, _>(reader)?;
			u32::from_le_bytes(b) as u128
		}
		253 => {
			let b = read_buffer::<8, _>(reader)?;
			u64::from_le_bytes(b) as u128
		}
		254 => {
			let b = read_buffer::<16, _>(reader)?;
			u128::from_le_bytes(b)
		}
		255 => return Err(Error::InvalidIntegerEncoding),
		x => x as u128,
	};
	Ok(v)
}

impl Revisioned for bool {
	fn revision() -> u16 {
		1
	}

	fn serialize_revisioned<W: std::io::Write>(&self, w: &mut W) -> Result<(), Error> {
		let v = *self as u8;
		w.write(&[v]).map_err(Error::Io)?;
		Ok(())
	}

	fn deserialize_revisioned<R: std::io::Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		let buffer = read_buffer::<1, _>(r)?;
		match buffer[0] {
			0 => Ok(false),
			1 => Ok(true),
			x => Err(Error::InvalidBoolValue(x)),
		}
	}
}

impl Revisioned for usize {
	fn revision() -> u16 {
		1
	}

	fn serialize_revisioned<W: std::io::Write>(&self, w: &mut W) -> Result<(), Error> {
		((*self) as u64).serialize_revisioned(w)
	}

	fn deserialize_revisioned<R: std::io::Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		u64::deserialize_revisioned(r).map(|x| x as usize)
	}
}

impl Revisioned for isize {
	fn revision() -> u16 {
		1
	}

	fn serialize_revisioned<W: std::io::Write>(&self, w: &mut W) -> Result<(), Error> {
		((*self) as i64).serialize_revisioned(w)
	}

	fn deserialize_revisioned<R: std::io::Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		i64::deserialize_revisioned(r).map(|x| x as isize)
	}
}

impl Revisioned for u8 {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		writer.write_all(&[*self]).map_err(Error::Io)
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		Ok(read_buffer::<1, _>(reader)?[0])
	}

	fn revision() -> u16 {
		1
	}
}

impl Revisioned for i8 {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		writer.write_all(&[*self as u8]).map_err(Error::Io)
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		Ok(read_buffer::<1, _>(reader)?[0] as i8)
	}

	fn revision() -> u16 {
		1
	}
}

macro_rules! impl_revisioned_int {
	($ty:ident) => {
		impl Revisioned for $ty {
			#[inline]
			fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
				encode_u64(writer, (*self) as u64)
			}

			#[inline]
			fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error>
			where
				Self: Sized,
			{
				decode_u64(reader).and_then(|x| x.try_into().map_err(|_| Error::IntegerOverflow))
			}

			fn revision() -> u16 {
				1
			}
		}
	};
}

macro_rules! impl_revisioned_signed_int {
	($ty:ident) => {
		impl Revisioned for $ty {
			#[inline]
			fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
				encode_u64(writer, zigzag_64((*self) as i64))
			}

			#[inline]
			fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error>
			where
				Self: Sized,
			{
				decode_u64(reader)
					.and_then(|x| gazgiz_64(x).try_into().map_err(|_| Error::IntegerOverflow))
			}

			fn revision() -> u16 {
				1
			}
		}
	};
}

impl_revisioned_int!(u16);
impl_revisioned_int!(u32);
impl_revisioned_int!(u64);

impl_revisioned_signed_int!(i16);
impl_revisioned_signed_int!(i32);
impl_revisioned_signed_int!(i64);

impl Revisioned for i128 {
	fn revision() -> u16 {
		1
	}

	fn serialize_revisioned<W: io::Write>(&self, w: &mut W) -> Result<(), Error> {
		encode_u128(w, zigzag_128(*self))
	}

	fn deserialize_revisioned<R: io::Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		decode_u128(r).map(gazgiz_128)
	}
}

impl Revisioned for u128 {
	fn revision() -> u16 {
		1
	}

	fn serialize_revisioned<W: io::Write>(&self, w: &mut W) -> Result<(), Error> {
		encode_u128(w, *self)
	}

	fn deserialize_revisioned<R: io::Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		decode_u128(r)
	}
}

impl Revisioned for f32 {
	fn revision() -> u16 {
		1
	}

	fn serialize_revisioned<W: io::Write>(&self, w: &mut W) -> Result<(), Error> {
		let bytes = self.to_le_bytes();
		w.write_all(&bytes).map_err(Error::Io)
	}

	fn deserialize_revisioned<R: io::Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		let b = read_buffer::<4, _>(r)?;
		Ok(f32::from_le_bytes(b))
	}
}

impl Revisioned for f64 {
	fn revision() -> u16 {
		1
	}

	fn serialize_revisioned<W: io::Write>(&self, w: &mut W) -> Result<(), Error> {
		let bytes = self.to_le_bytes();
		w.write_all(&bytes).map_err(Error::Io)
	}

	fn deserialize_revisioned<R: io::Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		let b = read_buffer::<8, _>(r)?;
		Ok(f64::from_le_bytes(b))
	}
}

#[cfg(test)]
mod tests {
	use crate::implementations::{
		assert_bincode_compat,
		primitives::{gazgiz_64, zigzag_64},
	};

	use super::Revisioned;

	#[test]
	fn test_zigzag() {
		assert_eq!(zigzag_64(0), 0);
		assert_eq!(zigzag_64(1), 2);
		assert_eq!(zigzag_64(-1), 1);

		assert_eq!(zigzag_64(i64::MIN), u64::MAX);
		assert_eq!(zigzag_64(i64::MAX), u64::MAX - 1);
	}

	#[test]
	fn test_gazgiz() {
		assert_eq!(gazgiz_64(0), 0);
		assert_eq!(gazgiz_64(1), -1);
		assert_eq!(gazgiz_64(2), 1);

		assert_eq!(gazgiz_64(u64::MAX), i64::MIN);
		assert_eq!(gazgiz_64(u64::MAX - 1), i64::MAX);
	}

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

	macro_rules! test_integer_compat {
		($n:ident,$ty:ident) => {
			#[test]
			fn $n() {
				let zero: $ty = 0;
				assert_bincode_compat(&zero);
				assert_bincode_compat(&$ty::MAX);
				assert_bincode_compat(&$ty::MIN);
			}
		};
	}

	test_integer_compat!(compat_i8, i8);
	test_integer_compat!(compat_u8, u8);
	test_integer_compat!(compat_i16, i16);
	test_integer_compat!(compat_u16, u16);
	test_integer_compat!(compat_i32, i32);
	test_integer_compat!(compat_u32, u32);
	test_integer_compat!(compat_i64, i64);
	test_integer_compat!(compat_u64, u64);
	test_integer_compat!(compat_i128, i128);
	test_integer_compat!(compat_u128, u128);

	#[test]
	fn compat_f64() {
		assert_bincode_compat(&0f64);
		assert_bincode_compat(&f64::MAX);
		assert_bincode_compat(&f64::MIN);
		assert_bincode_compat(&f64::EPSILON);
		assert_bincode_compat(&f64::INFINITY);
		assert_bincode_compat(&f64::NEG_INFINITY);
		assert_bincode_compat(&f64::NAN);
	}

	#[test]
	fn compat_f32() {
		assert_bincode_compat(&0f32);
		assert_bincode_compat(&f32::MAX);
		assert_bincode_compat(&f32::MIN);
		assert_bincode_compat(&f32::EPSILON);
		assert_bincode_compat(&f32::INFINITY);
		assert_bincode_compat(&f32::NEG_INFINITY);
		assert_bincode_compat(&f32::MIN_POSITIVE);
		assert_bincode_compat(&f32::NAN);
	}
}
