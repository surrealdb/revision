//! Helpers for advancing through revisioned bytes without building values.
//!
//! Used by [`crate::SkipRevisioned`] implementations and [`crate::skip_slice`].

use std::io::{Read, Result as IoResult};

use crate::Error;

/// Discards `len` bytes from `reader` using a fixed stack buffer (no large allocations).
#[inline]
pub fn advance_read<R: Read + ?Sized>(reader: &mut R, mut len: usize) -> Result<(), Error> {
	let mut buf = [0u8; 4096];
	while len > 0 {
		let chunk = len.min(buf.len());
		reader.read_exact(&mut buf[..chunk]).map_err(Error::Io)?;
		len -= chunk;
	}
	Ok(())
}

/// `Read` adapter over a byte slice, tracking how many bytes were consumed.
///
/// This is optional; [`crate::skip_slice`] uses [`&[u8]`](std::slice) as a [`Read`]
/// implementor instead. `SliceReader` is useful when you need the consumed length
/// after partially reading with other APIs.
#[derive(Clone, Copy, Debug)]
pub struct SliceReader<'a> {
	inner: &'a [u8],
	original_len: usize,
}

impl<'a> SliceReader<'a> {
	#[inline]
	pub fn new(slice: &'a [u8]) -> Self {
		Self {
			original_len: slice.len(),
			inner: slice,
		}
	}

	/// Remaining unconsumed bytes.
	#[inline]
	pub fn remaining(&self) -> &[u8] {
		self.inner
	}

	/// Number of bytes consumed since construction.
	#[inline]
	pub fn consumed_len(&self) -> usize {
		self.original_len - self.inner.len()
	}

	/// Advance by `n` bytes without copying.
	#[inline]
	pub fn consume(&mut self, n: usize) -> Result<(), Error> {
		if n > self.inner.len() {
			return Err(Error::Io(std::io::Error::new(
				std::io::ErrorKind::UnexpectedEof,
				"unexpected EOF while skipping",
			)));
		}
		self.inner = &self.inner[n..];
		Ok(())
	}

	/// Construct a new `SliceReader` over a sub-range of the original slice.
	///
	/// `offset` is measured from the start of the slice this reader was originally
	/// constructed with — *not* the current cursor position. The cursor in the
	/// returned reader starts at the beginning of the sub-range.
	///
	/// Used by optimised walkers to hand a child walker exactly one field's bytes
	/// without mutating the parent cursor.
	#[inline]
	pub fn sub(&self, offset: usize, len: usize) -> Result<SliceReader<'a>, Error> {
		// `offset` is into the original slice; convert to an `inner`-relative offset.
		let inner_offset = offset.checked_sub(self.consumed_len()).ok_or_else(|| {
			Error::Io(std::io::Error::new(
				std::io::ErrorKind::InvalidInput,
				"SliceReader::sub: offset precedes current cursor",
			))
		})?;
		let end = inner_offset.checked_add(len).ok_or_else(|| {
			Error::Io(std::io::Error::new(
				std::io::ErrorKind::InvalidInput,
				"SliceReader::sub: offset + len overflow",
			))
		})?;
		if end > self.inner.len() {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		Ok(SliceReader::new(&self.inner[inner_offset..end]))
	}
}

impl Read for SliceReader<'_> {
	#[inline]
	fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
		let n = buf.len().min(self.inner.len());
		buf[..n].copy_from_slice(&self.inner[..n]);
		self.inner = &self.inner[n..];
		Ok(n)
	}
}

/// A [`Read`] that can hand out borrowed slices of upcoming bytes.
///
/// Implemented by `&[u8]` and [`SliceReader`]; used by walker methods
/// (`LeafWalker::with_bytes`, `MapEntry::with_key_bytes`, etc.) to peek at
/// length-prefixed payloads without materialising them into owned values.
///
/// `Read` itself cannot be peeked — it might be a streaming source like
/// `File` or a network socket whose bytes don't sit in an addressable
/// buffer. `BorrowedReader` is the explicit "I am slice-backed" contract;
/// methods that want zero-copy access opt into it via a trait bound.
pub trait BorrowedReader: Read {
	/// Borrow the next `n` bytes without advancing the cursor.
	///
	/// Returns the slice on success. The slice is valid until the next
	/// call that mutably borrows the reader (typically [`advance`]
	/// (Self::advance) or any `Read`-trait method).
	fn peek_bytes(&self, n: usize) -> Result<&[u8], Error>;

	/// Advance the cursor past `n` bytes without copying them.
	///
	/// Equivalent to reading `n` bytes and discarding them, but cheaper
	/// (the bytes are never touched). Returns an error if fewer than
	/// `n` bytes remain.
	fn advance(&mut self, n: usize) -> Result<(), Error>;

	/// Bytes consumed since the reader was constructed.
	///
	/// Used by optimised walkers and the encode side to compute offsets
	/// relative to the start of an optimised compound's payload.
	fn position(&self) -> usize;
}

impl BorrowedReader for &[u8] {
	#[inline]
	fn peek_bytes(&self, n: usize) -> Result<&[u8], Error> {
		self.get(..n).ok_or_else(|| {
			Error::Io(std::io::Error::new(
				std::io::ErrorKind::UnexpectedEof,
				"unexpected EOF while peeking borrowed bytes",
			))
		})
	}

	#[inline]
	fn advance(&mut self, n: usize) -> Result<(), Error> {
		if n > self.len() {
			return Err(Error::Io(std::io::Error::new(
				std::io::ErrorKind::UnexpectedEof,
				"unexpected EOF while advancing slice reader",
			)));
		}
		*self = &self[n..];
		Ok(())
	}

	#[inline]
	fn position(&self) -> usize {
		// `&[u8]` has no original-length tracking, so we cannot report a meaningful
		// absolute position. Callers that need positions should use `SliceReader`.
		0
	}
}

impl<'a> BorrowedReader for SliceReader<'a> {
	#[inline]
	fn peek_bytes(&self, n: usize) -> Result<&[u8], Error> {
		self.inner.get(..n).ok_or_else(|| {
			Error::Io(std::io::Error::new(
				std::io::ErrorKind::UnexpectedEof,
				"unexpected EOF while peeking borrowed bytes",
			))
		})
	}

	#[inline]
	fn advance(&mut self, n: usize) -> Result<(), Error> {
		self.consume(n)
	}

	#[inline]
	fn position(&self) -> usize {
		self.consumed_len()
	}
}

/// Borrow `n` bytes and advance past them in one step.
///
/// `BorrowedReader::take_bytes` would be the natural place for this, but expressing
/// "return a borrow whose lifetime survives the mutating `advance` call" as a trait
/// default fights the borrow checker. Per-impl free functions sidestep the issue.
#[inline]
pub fn take_bytes_slice<'a>(reader: &mut &'a [u8], n: usize) -> Result<&'a [u8], Error> {
	if n > reader.len() {
		return Err(Error::Io(std::io::Error::new(
			std::io::ErrorKind::UnexpectedEof,
			"unexpected EOF while taking borrowed bytes",
		)));
	}
	let (head, tail) = reader.split_at(n);
	*reader = tail;
	Ok(head)
}

/// `take_bytes` for `SliceReader`. See [`take_bytes_slice`] for rationale.
#[inline]
pub fn take_bytes_reader<'r, 'a: 'r>(
	reader: &'r mut SliceReader<'a>,
	n: usize,
) -> Result<&'a [u8], Error> {
	if n > reader.inner.len() {
		return Err(Error::Io(std::io::Error::new(
			std::io::ErrorKind::UnexpectedEof,
			"unexpected EOF while taking borrowed bytes",
		)));
	}
	let (head, tail) = reader.inner.split_at(n);
	reader.inner = tail;
	Ok(head)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn slice_reader_position_tracks_consumed() {
		let data = [0u8, 1, 2, 3, 4];
		let mut r = SliceReader::new(&data);
		assert_eq!(r.position(), 0);
		r.consume(2).unwrap();
		assert_eq!(r.position(), 2);
		r.consume(1).unwrap();
		assert_eq!(r.position(), 3);
	}

	#[test]
	fn slice_reader_sub_carves_subrange() {
		let data = [0u8, 1, 2, 3, 4, 5];
		let r = SliceReader::new(&data);
		let sub = r.sub(2, 3).unwrap();
		assert_eq!(sub.remaining(), &[2, 3, 4]);
	}

	#[test]
	fn slice_reader_sub_rejects_overflow() {
		let data = [0u8, 1, 2, 3];
		let r = SliceReader::new(&data);
		assert!(matches!(r.sub(2, 3), Err(Error::OptimisedSubReaderOverrun)));
	}

	#[test]
	fn slice_reader_sub_after_consume() {
		let data = [0u8, 1, 2, 3, 4, 5];
		let mut r = SliceReader::new(&data);
		r.consume(2).unwrap();
		// `offset` is absolute against the original slice.
		let sub = r.sub(3, 2).unwrap();
		assert_eq!(sub.remaining(), &[3, 4]);
	}

	#[test]
	fn slice_reader_sub_rejects_offset_before_cursor() {
		let data = [0u8, 1, 2, 3];
		let mut r = SliceReader::new(&data);
		r.consume(2).unwrap();
		assert!(r.sub(1, 1).is_err());
	}

	#[test]
	fn take_bytes_slice_advances_and_returns_borrow() {
		let data: &[u8] = &[1, 2, 3, 4];
		let mut cursor = data;
		let taken = take_bytes_slice(&mut cursor, 2).unwrap();
		assert_eq!(taken, &[1, 2]);
		assert_eq!(cursor, &[3, 4]);
	}

	#[test]
	fn take_bytes_reader_advances_and_returns_borrow() {
		let data = [1u8, 2, 3, 4];
		let mut r = SliceReader::new(&data);
		let taken = take_bytes_reader(&mut r, 3).unwrap();
		assert_eq!(taken, &[1, 2, 3]);
		assert_eq!(r.remaining(), &[4]);
	}
}
