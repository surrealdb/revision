//! Tagged-value envelope encoders and decoders.
//!
//! These helpers are the runtime side of `optimised` codegen. The
//! macro emits calls into them; user code reaches them only indirectly.

use std::io::{Read, Write};

use crate::Error;
use crate::optimised::tag::{SizeClass, Tag, read_tag, write_tag};
use crate::slice_reader::{BorrowedReader, advance_read};

/// Encode an inline value: just the tag byte, no payload.
#[doc(hidden)]
#[inline]
pub fn encode_inline<W: Write>(w: &mut W, variant_id: u8) -> Result<(), Error> {
	write_tag(w, Tag::new(variant_id, SizeClass::Inline))
}

/// Encode a fixed-size value: tag byte plus the writer-supplied static-size payload.
///
/// The macro emits a `debug_assert_eq!(__bytes_written, expected_size)` after the
/// payload write so encoding mismatches surface in debug builds.
#[doc(hidden)]
#[inline]
pub fn encode_fixed<W: Write, F>(w: &mut W, variant_id: u8, body: F) -> Result<(), Error>
where
	F: FnOnce(&mut W) -> Result<(), Error>,
{
	write_tag(w, Tag::new(variant_id, SizeClass::Fixed))?;
	body(w)
}

/// Encode a varlen value: tag byte, then `u32_le byte_length`, then payload.
///
/// Implementation note: the body is written into a scratch `Vec<u8>` first so the
/// length is known before the prefix is emitted. This trades one allocation per
/// varlen value for a simple, single-pass macro expansion (no measure-then-write
/// dance on the writer side, no in-place patching of the length prefix).
#[doc(hidden)]
pub fn encode_varlen<W: Write, F>(w: &mut W, variant_id: u8, body: F) -> Result<(), Error>
where
	F: FnOnce(&mut Vec<u8>) -> Result<(), Error>,
{
	write_tag(w, Tag::new(variant_id, SizeClass::Varlen))?;
	let mut scratch: Vec<u8> = Vec::new();
	body(&mut scratch)?;
	let len: u32 = scratch
		.len()
		.try_into()
		.map_err(|_| Error::Serialize("optimised varlen payload exceeds u32::MAX bytes".into()))?;
	w.write_all(&len.to_le_bytes()).map_err(Error::Io)?;
	w.write_all(&scratch).map_err(Error::Io)
}

/// Read just the tag of an optimised value, validating the size class.
#[doc(hidden)]
#[inline]
pub fn read_optimised_tag<R: Read>(r: &mut R) -> Result<(Tag, SizeClass), Error> {
	let tag = read_tag(r)?;
	let sc = tag.size_class()?;
	Ok((tag, sc))
}

/// Read the `u32_le` byte length of a varlen value. The caller is expected to have
/// already consumed the tag byte.
#[doc(hidden)]
#[inline]
pub fn read_varlen_len<R: Read>(r: &mut R) -> Result<u32, Error> {
	let mut buf = [0u8; 4];
	r.read_exact(&mut buf).map_err(Error::Io)?;
	Ok(u32::from_le_bytes(buf))
}

/// Read a varlen value's payload as a borrowed slice.
///
/// Tag must have already been consumed. Returns the payload bytes and advances the
/// reader past them. The returned slice's lifetime is tied to the reader's input.
#[doc(hidden)]
#[inline]
pub fn read_varlen_slice<R: BorrowedReader>(r: &mut R) -> Result<&[u8], Error> {
	let len = read_varlen_len(r)? as usize;
	crate::slice_reader::read_borrowed_bytes(r, len)
}

/// Skip past a varlen value's payload (tag already consumed). Streaming-reader friendly.
#[doc(hidden)]
#[inline]
pub fn skip_varlen<R: Read>(r: &mut R) -> Result<(), Error> {
	let len = read_varlen_len(r)? as usize;
	advance_read(r, len)
}

/// Skip past a varlen value's payload using a [`BorrowedReader`]; cheaper than `skip_varlen`.
#[doc(hidden)]
#[inline]
pub fn skip_varlen_borrowed<R: BorrowedReader>(r: &mut R) -> Result<(), Error> {
	let len = read_varlen_len(r)? as usize;
	r.advance(len)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::slice_reader::SliceReader;

	#[test]
	fn inline_round_trip() {
		let mut buf = Vec::new();
		encode_inline(&mut buf, 3).unwrap();
		assert_eq!(buf.len(), 1);
		let mut r: &[u8] = &buf;
		let (tag, sc) = read_optimised_tag(&mut r).unwrap();
		assert_eq!(tag.variant_id(), 3);
		assert_eq!(sc, SizeClass::Inline);
		assert_eq!(r.len(), 0);
	}

	#[test]
	fn fixed_round_trip() {
		let mut buf = Vec::new();
		encode_fixed(&mut buf, 7, |w| {
			w.write_all(&42i64.to_le_bytes()).map_err(Error::Io)?;
			Ok(())
		})
		.unwrap();
		assert_eq!(buf.len(), 1 + 8);
		let mut r: &[u8] = &buf;
		let (tag, sc) = read_optimised_tag(&mut r).unwrap();
		assert_eq!(tag.variant_id(), 7);
		assert_eq!(sc, SizeClass::Fixed);
		let mut payload = [0u8; 8];
		r.read_exact(&mut payload).unwrap();
		assert_eq!(i64::from_le_bytes(payload), 42);
	}

	#[test]
	fn varlen_round_trip() {
		let mut buf = Vec::new();
		encode_varlen(&mut buf, 12, |scratch| {
			scratch.extend_from_slice(b"hello world");
			Ok(())
		})
		.unwrap();
		assert_eq!(buf.len(), 1 + 4 + 11);
		let mut r = SliceReader::new(&buf);
		let (tag, sc) = read_optimised_tag(&mut r).unwrap();
		assert_eq!(tag.variant_id(), 12);
		assert_eq!(sc, SizeClass::Varlen);
		let payload = read_varlen_slice(&mut r).unwrap();
		assert_eq!(payload, b"hello world");
	}

	#[test]
	fn skip_varlen_advances_full_length() {
		let mut buf = Vec::new();
		encode_varlen(&mut buf, 1, |s| {
			s.extend_from_slice(&[0xAA; 100]);
			Ok(())
		})
		.unwrap();
		let mut r: &[u8] = &buf;
		let (_, sc) = read_optimised_tag(&mut r).unwrap();
		assert_eq!(sc, SizeClass::Varlen);
		skip_varlen(&mut r).unwrap();
		assert_eq!(r.len(), 0);
	}

	#[test]
	fn skip_varlen_borrowed_matches_skip_varlen() {
		let mut buf = Vec::new();
		encode_varlen(&mut buf, 1, |s| {
			s.extend_from_slice(&[0xAA; 100]);
			Ok(())
		})
		.unwrap();
		let mut r = SliceReader::new(&buf);
		let (_, _) = read_optimised_tag(&mut r).unwrap();
		skip_varlen_borrowed(&mut r).unwrap();
		assert!(r.remaining().is_empty());
	}

	#[test]
	fn read_optimised_tag_errors_on_reserved_size_class() {
		// Hand-craft a tag with size_class = 0b11.
		let bad = 0b0110_0000;
		let mut r: &[u8] = &[bad];
		let err = read_optimised_tag(&mut r).unwrap_err();
		assert!(matches!(err, Error::InvalidOptimisedTag(b) if b == bad));
	}

	#[test]
	fn read_varlen_slice_overrun_errors() {
		// Tag + length = 100 but only 4 bytes of payload follow.
		let mut buf = Vec::new();
		buf.push(Tag::new(0, SizeClass::Varlen).0);
		buf.extend_from_slice(&100u32.to_le_bytes());
		buf.extend_from_slice(&[0u8; 4]);
		let mut r = SliceReader::new(&buf);
		let _ = read_optimised_tag(&mut r).unwrap();
		assert!(read_varlen_slice(&mut r).is_err());
	}
}
