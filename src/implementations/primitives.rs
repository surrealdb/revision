use std::io;

use super::super::Revisioned;
use crate::{DeserializeRevisioned, Error, SerializeRevisioned};

#[inline]
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

// Variable-length encoding (default)
#[cfg(not(feature = "fixed-width-encoding"))]
fn encode_u64<W>(writer: &mut W, i: u64) -> Result<(), Error>
where
	W: io::Write,
{
	if i < 251 {
		writer.write_all(&[i as u8]).map_err(Error::Io)
	} else if i < (1 << 16) {
		let bytes = (i as u16).to_le_bytes();
		writer.write_all(&[251, bytes[0], bytes[1]]).map_err(Error::Io)
	} else if i < (1 << 32) {
		let bytes = (i as u32).to_le_bytes();
		writer.write_all(&[252, bytes[0], bytes[1], bytes[2], bytes[3]]).map_err(Error::Io)
	} else {
		let bytes = i.to_le_bytes();
		writer
			.write_all(&[
				253, bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
			])
			.map_err(Error::Io)
	}
}

#[cfg(not(feature = "fixed-width-encoding"))]
fn encode_u128<W>(writer: &mut W, i: u128) -> Result<(), Error>
where
	W: io::Write,
{
	if i < 251 {
		writer.write_all(&[i as u8]).map_err(Error::Io)
	} else if i < (1 << 16) {
		let bytes = (i as u16).to_le_bytes();
		writer.write_all(&[251, bytes[0], bytes[1]]).map_err(Error::Io)
	} else if i < (1 << 32) {
		let bytes = (i as u32).to_le_bytes();
		writer.write_all(&[252, bytes[0], bytes[1], bytes[2], bytes[3]]).map_err(Error::Io)
	} else if i < (1 << 64) {
		let bytes = (i as u64).to_le_bytes();
		writer
			.write_all(&[
				253, bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
			])
			.map_err(Error::Io)
	} else {
		let bytes = i.to_le_bytes();
		let bytes = [
			254, bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
			bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
		];
		writer.write_all(&bytes).map_err(Error::Io)
	}
}

#[cfg(not(feature = "fixed-width-encoding"))]
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

#[cfg(not(feature = "fixed-width-encoding"))]
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

impl SerializeRevisioned for bool {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, w: &mut W) -> Result<(), Error> {
		let v = *self as u8;
		w.write(&[v]).map_err(Error::Io)?;
		Ok(())
	}
}

impl DeserializeRevisioned for bool {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(r: &mut R) -> Result<Self, Error> {
		let buffer = read_buffer::<1, _>(r)?;
		match buffer[0] {
			0 => Ok(false),
			1 => Ok(true),
			x => Err(Error::InvalidBoolValue(x)),
		}
	}
}

impl Revisioned for bool {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for usize {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, w: &mut W) -> Result<(), Error> {
		((*self) as u64).serialize_revisioned(w)
	}
}

impl DeserializeRevisioned for usize {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		u64::deserialize_revisioned(r).map(|x| x as usize)
	}
}

impl Revisioned for usize {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for isize {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, w: &mut W) -> Result<(), Error> {
		((*self) as i64).serialize_revisioned(w)
	}
}

impl DeserializeRevisioned for isize {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(r: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		i64::deserialize_revisioned(r).map(|x| x as isize)
	}
}

impl Revisioned for isize {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for u8 {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		writer.write_all(&[*self]).map_err(Error::Io)
	}
}

impl DeserializeRevisioned for u8 {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		Ok(read_buffer::<1, _>(reader)?[0])
	}
}

impl Revisioned for u8 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for i8 {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		writer.write_all(&[*self as u8]).map_err(Error::Io)
	}
}

impl DeserializeRevisioned for i8 {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		Ok(read_buffer::<1, _>(reader)?[0] as i8)
	}
}

impl Revisioned for i8 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// u16 implementations
impl SerializeRevisioned for u16 {
	#[inline]
	fn serialize_revisioned<W: io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			encode_u64(writer, (*self) as u64)
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let bytes = self.to_le_bytes();
			writer.write_all(&bytes).map_err(Error::Io)
		}
	}
}

impl DeserializeRevisioned for u16 {
	#[inline]
	fn deserialize_revisioned<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			decode_u64(reader).and_then(|x| x.try_into().map_err(|_| Error::IntegerOverflow))
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let b = read_buffer::<2, _>(reader)?;
			Ok(u16::from_le_bytes(b))
		}
	}
}

impl Revisioned for u16 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// u32 implementations
impl SerializeRevisioned for u32 {
	#[inline]
	fn serialize_revisioned<W: io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			encode_u64(writer, (*self) as u64)
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let bytes = self.to_le_bytes();
			writer.write_all(&bytes).map_err(Error::Io)
		}
	}
}

impl DeserializeRevisioned for u32 {
	#[inline]
	fn deserialize_revisioned<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			decode_u64(reader).and_then(|x| x.try_into().map_err(|_| Error::IntegerOverflow))
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let b = read_buffer::<4, _>(reader)?;
			Ok(u32::from_le_bytes(b))
		}
	}
}

impl Revisioned for u32 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// u64 implementations
impl SerializeRevisioned for u64 {
	#[inline]
	fn serialize_revisioned<W: io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			encode_u64(writer, *self)
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let bytes = self.to_le_bytes();
			writer.write_all(&bytes).map_err(Error::Io)
		}
	}
}

impl DeserializeRevisioned for u64 {
	#[inline]
	fn deserialize_revisioned<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			decode_u64(reader)
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let b = read_buffer::<8, _>(reader)?;
			Ok(u64::from_le_bytes(b))
		}
	}
}

impl Revisioned for u64 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// i16 implementations
impl SerializeRevisioned for i16 {
	#[inline]
	fn serialize_revisioned<W: io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			encode_u64(writer, zigzag_64((*self) as i64))
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let bytes = (zigzag_64(*self as i64) as u16).to_le_bytes();
			writer.write_all(&bytes).map_err(Error::Io)
		}
	}
}

impl DeserializeRevisioned for i16 {
	#[inline]
	fn deserialize_revisioned<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			decode_u64(reader)
				.and_then(|x| gazgiz_64(x).try_into().map_err(|_| Error::IntegerOverflow))
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let b = read_buffer::<2, _>(reader)?;
			Ok(gazgiz_64(u16::from_le_bytes(b) as u64) as i16)
		}
	}
}

impl Revisioned for i16 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// i32 implementations
impl SerializeRevisioned for i32 {
	#[inline]
	fn serialize_revisioned<W: io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			encode_u64(writer, zigzag_64((*self) as i64))
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let bytes = (zigzag_64(*self as i64) as u32).to_le_bytes();
			writer.write_all(&bytes).map_err(Error::Io)
		}
	}
}

impl DeserializeRevisioned for i32 {
	#[inline]
	fn deserialize_revisioned<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			decode_u64(reader)
				.and_then(|x| gazgiz_64(x).try_into().map_err(|_| Error::IntegerOverflow))
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let b = read_buffer::<4, _>(reader)?;
			Ok(gazgiz_64(u32::from_le_bytes(b) as u64) as i32)
		}
	}
}

impl Revisioned for i32 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// i64 implementations
impl SerializeRevisioned for i64 {
	#[inline]
	fn serialize_revisioned<W: io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			encode_u64(writer, zigzag_64(*self))
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let bytes = zigzag_64(*self).to_le_bytes();
			writer.write_all(&bytes).map_err(Error::Io)
		}
	}
}

impl DeserializeRevisioned for i64 {
	#[inline]
	fn deserialize_revisioned<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			decode_u64(reader).map(gazgiz_64)
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let b = read_buffer::<8, _>(reader)?;
			Ok(gazgiz_64(u64::from_le_bytes(b)))
		}
	}
}

impl Revisioned for i64 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// i128 implementations
impl SerializeRevisioned for i128 {
	#[inline]
	fn serialize_revisioned<W: io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			encode_u128(writer, zigzag_128(*self))
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let bytes = zigzag_128(*self).to_le_bytes();
			writer.write_all(&bytes).map_err(Error::Io)
		}
	}
}

impl DeserializeRevisioned for i128 {
	#[inline]
	fn deserialize_revisioned<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			decode_u128(reader).map(gazgiz_128)
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let b = read_buffer::<16, _>(reader)?;
			Ok(gazgiz_128(u128::from_le_bytes(b)))
		}
	}
}

impl Revisioned for i128 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

// u128 implementations
impl SerializeRevisioned for u128 {
	#[inline]
	fn serialize_revisioned<W: io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			encode_u128(writer, *self)
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let bytes = self.to_le_bytes();
			writer.write_all(&bytes).map_err(Error::Io)
		}
	}
}

impl DeserializeRevisioned for u128 {
	#[inline]
	fn deserialize_revisioned<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		#[cfg(not(feature = "fixed-width-encoding"))]
		{
			decode_u128(reader)
		}
		#[cfg(feature = "fixed-width-encoding")]
		{
			let b = read_buffer::<16, _>(reader)?;
			Ok(u128::from_le_bytes(b))
		}
	}
}

impl Revisioned for u128 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for f32 {
	#[inline]
	fn serialize_revisioned<W: io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		let bytes = self.to_le_bytes();
		writer.write_all(&bytes).map_err(Error::Io)
	}
}

impl DeserializeRevisioned for f32 {
	#[inline]
	fn deserialize_revisioned<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		let b = read_buffer::<4, _>(reader)?;
		Ok(f32::from_le_bytes(b))
	}
}

impl Revisioned for f32 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for f64 {
	#[inline]
	fn serialize_revisioned<W: io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		let bytes = self.to_le_bytes();
		writer.write_all(&bytes).map_err(Error::Io)
	}
}

impl DeserializeRevisioned for f64 {
	#[inline]
	fn deserialize_revisioned<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Self: Sized,
	{
		let b = read_buffer::<8, _>(reader)?;
		Ok(f64::from_le_bytes(b))
	}
}

impl Revisioned for f64 {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {
	use crate::implementations::{
		assert_bincode_compat,
		primitives::{gazgiz_64, zigzag_64},
	};

	use super::*;

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
		let out =
			<bool as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_isize() {
		let val = isize::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 9);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 8);
		let out =
			<isize as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_i8() {
		let val = i8::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 1);
		let out =
			<i8 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_i16() {
		let val = i16::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 3);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 2);
		let out =
			<i16 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_i32() {
		let val = i32::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 5);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 4);
		let out =
			<i32 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_i64() {
		let val = i64::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 9);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 8);
		let out =
			<i64 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_i128() {
		let val = i128::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 17);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 16);
		let out =
			<i128 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_usize() {
		let val = usize::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 9);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 8);
		let out =
			<usize as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_u8() {
		let val = u8::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 1);
		let out =
			<u8 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_u16() {
		let val = u16::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 3);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 2);
		let out =
			<u16 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_u32() {
		let val = u32::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 5);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 4);
		let out =
			<u32 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_u64() {
		let val = u64::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 9);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 8);
		let out =
			<u64 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_u128() {
		let val = u128::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		#[cfg(not(feature = "fixed-width-encoding"))]
		assert_eq!(mem.len(), 17);
		#[cfg(feature = "fixed-width-encoding")]
		assert_eq!(mem.len(), 16);
		let out =
			<u128 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_f32() {
		let val = f32::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 4);
		let out =
			<f32 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_f64() {
		let val = f64::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 8);
		let out =
			<f64 as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_char() {
		let val = char::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 4);
		let out =
			<char as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	macro_rules! test_integer_compat {
		($n:ident,$ty:ident) => {
			#[test]
			#[cfg(not(feature = "fixed-width-encoding"))]
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
