//! Indexed-sequence walker.
//!
//! Layout of an indexed-seq payload (after the outer envelope has been opened):
//!
//! ```text
//! u8 flags                          // bit 0: indexed
//! varint len                        // element count
//! if flags.0:
//!     [u32_le elem_off; len]        // offset table
//!     elem_0 || elem_1 || ... || elem_{len-1}
//! else:
//!     elem_0 || elem_1 || ...       // legacy-shape body
//! ```
//!
//! Offsets are measured from the start of the indexed body (i.e. just past the
//! offset table). Walker construction validates the prologue once.

use std::marker::PhantomData;

use crate::Error;
use crate::optimised::validation::validate_seq_prologue;

#[doc(hidden)]
pub const FLAG_INDEXED: u8 = 0b0000_0001;

/// Walker over an indexed-seq body.
///
/// `T` is recorded only for type-driven decode helpers; the walker itself stores raw bytes.
#[derive(Debug)]
pub struct IndexedSeqWalker<'p, T> {
	body: &'p [u8],
	offsets: Option<Vec<u32>>,
	len: usize,
	_marker: PhantomData<fn() -> T>,
}

impl<'p, T> IndexedSeqWalker<'p, T> {
	/// Construct a walker from a flag-prefixed seq payload.
	///
	/// `payload` is the bytes after the outer optimised-envelope tag+length:
	/// `flags || varint(len) || body`.
	pub fn from_payload(payload: &'p [u8]) -> Result<Self, Error> {
		Self::from_payload_inner(payload, true)
	}

	/// Open a walker **without** validating the prologue (monotonic offsets).
	///
	/// Skips the O(len) offset-table check that [`from_payload`] runs. Use
	/// only when the bytes are trusted (e.g. freshly written by the same
	/// process). On untrusted input a malformed prologue produces silent
	/// wrong-element bytes rather than a clean
	/// [`Error::OptimisedOffsetsNonMonotonic`].
	///
	/// [`from_payload`]: Self::from_payload
	/// [`Error::OptimisedOffsetsNonMonotonic`]: crate::Error::OptimisedOffsetsNonMonotonic
	pub fn from_payload_unvalidated(payload: &'p [u8]) -> Result<Self, Error> {
		Self::from_payload_inner(payload, false)
	}

	fn from_payload_inner(payload: &'p [u8], validate: bool) -> Result<Self, Error> {
		if payload.is_empty() {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		let flags = payload[0];
		let mut cursor = 1usize;
		let (len, varint_len) = read_varint(&payload[cursor..])?;
		cursor += varint_len;

		let indexed = (flags & FLAG_INDEXED) != 0;
		if !indexed {
			return Ok(Self {
				body: &payload[cursor..],
				offsets: None,
				len,
				_marker: PhantomData,
			});
		}

		let table_bytes = len
			.checked_mul(4)
			.ok_or_else(|| Error::Deserialize("indexed-seq offset table size overflow".into()))?;
		if payload.len() < cursor + table_bytes {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		let offsets = parse_offsets(&payload[cursor..cursor + table_bytes]);
		cursor += table_bytes;
		let body = &payload[cursor..];
		if validate {
			validate_seq_prologue(&offsets, body.len() as u32)?;
		}
		Ok(Self {
			body,
			offsets: Some(offsets),
			len,
			_marker: PhantomData,
		})
	}

	#[inline]
	pub fn len(&self) -> usize {
		self.len
	}

	#[inline]
	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	#[inline]
	pub fn is_indexed(&self) -> bool {
		self.offsets.is_some()
	}

	/// Borrow the bytes for element `index`. O(1) on the indexed path; falls
	/// through to an error on the legacy path because we have no offsets.
	pub fn element_bytes(&self, index: usize) -> Result<&'p [u8], Error> {
		let offsets = self
			.offsets
			.as_ref()
			.ok_or_else(|| Error::Deserialize("element_bytes called on non-indexed seq".into()))?;
		if index >= self.len {
			return Err(Error::Deserialize(format!("index {index} out of range ({})", self.len)));
		}
		let start = offsets[index] as usize;
		let end = if index + 1 < self.len {
			offsets[index + 1] as usize
		} else {
			self.body.len()
		};
		Ok(&self.body[start..end])
	}

	/// Raw bytes for the body (post-prologue). Used by legacy-fallback iteration.
	#[inline]
	pub fn body(&self) -> &'p [u8] {
		self.body
	}
}

#[inline]
fn parse_offsets(bytes: &[u8]) -> Vec<u32> {
	bytes.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect()
}

/// Parse a `usize` varint matching the on-wire shape used by `Vec`/map lengths.
///
/// Mirrors `revision::implementations::primitives` — tag byte then 0/2/4/8 trailing
/// bytes for the value width. Returns `(value, bytes_consumed)`.
fn read_varint(bytes: &[u8]) -> Result<(usize, usize), Error> {
	if bytes.is_empty() {
		return Err(Error::OptimisedSubReaderOverrun);
	}
	let tag = bytes[0];
	match tag {
		0..=250 => Ok((tag as usize, 1)),
		251 => {
			if bytes.len() < 3 {
				return Err(Error::OptimisedSubReaderOverrun);
			}
			Ok((u16::from_le_bytes([bytes[1], bytes[2]]) as usize, 3))
		}
		252 => {
			if bytes.len() < 5 {
				return Err(Error::OptimisedSubReaderOverrun);
			}
			Ok((u32::from_le_bytes(bytes[1..5].try_into().unwrap()) as usize, 5))
		}
		253 => {
			if bytes.len() < 9 {
				return Err(Error::OptimisedSubReaderOverrun);
			}
			let v = u64::from_le_bytes(bytes[1..9].try_into().unwrap());
			let v: usize = v.try_into().map_err(|_| Error::IntegerOverflow)?;
			Ok((v, 9))
		}
		_ => Err(Error::InvalidIntegerEncoding),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn varint(v: usize) -> Vec<u8> {
		match v {
			0..=250 => vec![v as u8],
			251..=65535 => {
				let mut out = vec![251u8];
				out.extend_from_slice(&(v as u16).to_le_bytes());
				out
			}
			_ => {
				let mut out = vec![252u8];
				out.extend_from_slice(&(v as u32).to_le_bytes());
				out
			}
		}
	}

	fn build_indexed_seq(elements: &[&[u8]]) -> Vec<u8> {
		let len = elements.len();
		let mut out = Vec::new();
		out.push(FLAG_INDEXED);
		out.extend_from_slice(&varint(len));
		let mut running = 0u32;
		let mut offsets = Vec::with_capacity(len);
		for e in elements {
			offsets.push(running);
			running += e.len() as u32;
		}
		for o in &offsets {
			out.extend_from_slice(&o.to_le_bytes());
		}
		for e in elements {
			out.extend_from_slice(e);
		}
		out
	}

	#[test]
	fn opens_indexed_seq_and_reads_elements() {
		let payload = build_indexed_seq(&[b"foo", b"barbar", b"baz"]);
		let w: IndexedSeqWalker<()> = IndexedSeqWalker::from_payload(&payload).unwrap();
		assert!(w.is_indexed());
		assert_eq!(w.len(), 3);
		assert_eq!(w.element_bytes(0).unwrap(), b"foo");
		assert_eq!(w.element_bytes(1).unwrap(), b"barbar");
		assert_eq!(w.element_bytes(2).unwrap(), b"baz");
	}

	#[test]
	fn opens_legacy_seq_passes_through() {
		// Legacy: flags=0, varint(2), then two zero-length payloads (not very useful but legal)
		let payload = [0u8, 2, 1, 2];
		let w: IndexedSeqWalker<()> = IndexedSeqWalker::from_payload(&payload).unwrap();
		assert!(!w.is_indexed());
		assert_eq!(w.len(), 2);
		assert_eq!(w.body(), &[1u8, 2]);
		// element_bytes errors on the legacy path
		assert!(w.element_bytes(0).is_err());
	}

	#[test]
	fn rejects_truncated_payload() {
		// flags + half a varint
		let payload = [FLAG_INDEXED, 251, 0];
		let err: Error = IndexedSeqWalker::<()>::from_payload(&payload).unwrap_err();
		assert!(matches!(err, Error::OptimisedSubReaderOverrun));
	}

	#[test]
	fn rejects_truncated_offset_table() {
		// flags + len=3 + only one offset
		let mut payload = vec![FLAG_INDEXED, 3];
		payload.extend_from_slice(&0u32.to_le_bytes());
		let err: Error = IndexedSeqWalker::<()>::from_payload(&payload).unwrap_err();
		assert!(matches!(err, Error::OptimisedSubReaderOverrun));
	}

	#[test]
	fn rejects_non_monotonic_offsets() {
		// indexed flags, len=2, offsets [10, 0], body 16 bytes
		let mut payload = vec![FLAG_INDEXED, 2];
		payload.extend_from_slice(&10u32.to_le_bytes());
		payload.extend_from_slice(&0u32.to_le_bytes());
		payload.extend_from_slice(&[0u8; 16]);
		let err: Error = IndexedSeqWalker::<()>::from_payload(&payload).unwrap_err();
		assert!(matches!(err, Error::OptimisedOffsetsNonMonotonic));
	}
}
