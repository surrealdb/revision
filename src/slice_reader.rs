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
