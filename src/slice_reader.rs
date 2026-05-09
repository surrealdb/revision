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
}
