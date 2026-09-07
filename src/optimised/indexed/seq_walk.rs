//! Indexed-sequence walker.
//!
//! Layout of an indexed-seq payload (after the outer envelope has been opened):
//!
//! ```text
//! u8 flags                          // bit 0: indexed, bit 1: strided
//! varint len                        // element count
//! if flags.0 && flags.1:
//!     varint stride                 // byte width shared by every element
//!     elem_0 || elem_1 || ... || elem_{len-1}
//! else if flags.0:
//!     [u32_le elem_off; len]        // offset table
//!     elem_0 || elem_1 || ... || elem_{len-1}
//! else:
//!     elem_0 || elem_1 || ...       // legacy-shape body
//! ```
//!
//! Offsets are measured from the start of the indexed body (i.e. just past the
//! offset table). Walker construction validates the prologue once.
//!
//! The strided shape carries the same O(1) random access as the offset table
//! for the case where every element serialises to the same width — element `i`
//! starts at `i * stride` — at a prologue cost of one varint instead of
//! `len * 4` bytes. Readers accept both shapes unconditionally; whether the
//! encoder *emits* the strided shape is gated on the `strided-seq` feature,
//! because a reader predating this flag misreads the stride varint as the
//! head of an offset table.

use std::marker::PhantomData;

use crate::Error;
use crate::optimised::validation::{validate_seq_prologue, validate_strided_seq_prologue};

#[doc(hidden)]
pub const FLAG_INDEXED: u8 = 0b0000_0001;

/// Set alongside [`FLAG_INDEXED`] when the prologue carries a single stride
/// instead of a per-element offset table. Meaningless on its own: a payload
/// with this bit but not [`FLAG_INDEXED`] is read as a legacy body.
#[doc(hidden)]
pub const FLAG_STRIDED: u8 = 0b0000_0010;

/// How element start positions are recovered from an indexed-seq body.
#[derive(Clone, Copy, Debug)]
enum ElementIndex<'p> {
	/// Legacy body: element positions are not recorded, so there is no random
	/// access — callers decode elements in order off [`body`](IndexedSeqWalker::body).
	Linear,
	/// A `len * 4` byte slice of `u32_le` element start offsets, borrowed from
	/// the payload.
	Offsets(&'p [u8]),
	/// Every element occupies exactly this many bytes, so element `i` starts at
	/// `i * stride`. Always non-zero — [`validate_strided_seq_prologue`] rejects
	/// a zero stride.
	Stride(usize),
}

/// Walker over an indexed-seq body.
///
/// `T` is recorded only for type-driven decode helpers; the walker itself stores raw bytes.
///
/// On the offset-table path the table is borrowed directly from the payload as
/// `&'p [u8]` (a contiguous `len * 4` slice of `u32_le` entries). Individual
/// offsets are decoded on demand via [`offset_at`] — a single
/// `u32::from_le_bytes` on a 4-byte window, essentially one aligned `mov` on
/// common targets. Borrowing the table instead of materialising it into a
/// `Vec<u32>` removes one allocation + `O(len)` copy loop from every walker
/// construction; on scan-heavy workloads that fires per row per descent level.
///
/// On the strided path there is no table to borrow at all: the start offset is
/// a multiply, and construction validates the geometry in O(1) rather than
/// walking the table for monotonicity.
///
/// [`offset_at`]: Self::offset_at
#[derive(Debug)]
pub struct IndexedSeqWalker<'p, T> {
	body: &'p [u8],
	index: ElementIndex<'p>,
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
	/// process).
	///
	/// # Panics on malformed input
	///
	/// On untrusted input this trades a clean
	/// [`Error::OptimisedOffsetsNonMonotonic`] at construction for
	/// failures on access. Specifically:
	///
	/// - The offset *table* itself is bounds-checked at construction —
	///   `OptimisedSubReaderOverrun` is returned if the payload is too
	///   short to hold `len * 4` bytes of offsets. Reading any offset
	///   from the table is therefore safe.
	/// - The offset *values* read from that table are not checked.
	///   [`element_bytes`](Self::element_bytes) slices the body by
	///   those values; an offset past the body's length or a
	///   non-monotonic adjacent entry triggers a slice-out-of-bounds
	///   panic.
	///
	/// This is intended behaviour: the caller asserted trust. Callers
	/// who cannot make that assertion should use [`from_payload`].
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
				index: ElementIndex::Linear,
				len,
				_marker: PhantomData,
			});
		}

		if (flags & FLAG_STRIDED) != 0 {
			let (stride, stride_len) = read_varint(&payload[cursor..])?;
			cursor += stride_len;
			let body = &payload[cursor..];
			// Unlike the offset table, the strided geometry is cheap enough to
			// check that `from_payload_unvalidated` checks it too: it is one
			// multiply, not an O(len) walk, and it is what makes the
			// multiply-and-slice in `element_bytes` total.
			validate_strided_seq_prologue(stride, len, body.len())?;
			return Ok(Self {
				body,
				index: ElementIndex::Stride(stride),
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
		let offsets = &payload[cursor..cursor + table_bytes];
		cursor += table_bytes;
		let body = &payload[cursor..];
		if validate {
			validate_seq_prologue(offsets, len, body.len() as u32)?;
		}
		Ok(Self {
			body,
			index: ElementIndex::Offsets(offsets),
			len,
			_marker: PhantomData,
		})
	}

	/// Decode the `index`-th offset from the borrowed table.
	///
	/// `offsets` is the table-bytes slice produced at construction
	/// (`len * 4` bytes); the caller must have already validated `index < len`.
	#[inline]
	fn offset_at(offsets: &[u8], index: usize) -> u32 {
		crate::optimised::validation::decode_u32_le_at(offsets, index * 4)
	}

	#[inline]
	pub fn len(&self) -> usize {
		self.len
	}

	#[inline]
	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	/// Whether this body supports random access — true for both the
	/// offset-table and the strided shape.
	#[inline]
	pub fn is_indexed(&self) -> bool {
		!matches!(self.index, ElementIndex::Linear)
	}

	/// The byte width shared by every element, when the body is strided.
	/// `None` on the offset-table and legacy shapes.
	#[inline]
	pub fn stride(&self) -> Option<usize> {
		match self.index {
			ElementIndex::Stride(stride) => Some(stride),
			_ => None,
		}
	}

	/// Borrow the bytes for element `index`. O(1) on both indexed shapes; falls
	/// through to an error on the legacy path because element positions were
	/// never recorded.
	pub fn element_bytes(&self, index: usize) -> Result<&'p [u8], Error> {
		if index >= self.len {
			return Err(Error::Deserialize(format!("index {index} out of range ({})", self.len)));
		}
		match self.index {
			ElementIndex::Linear => {
				Err(Error::Deserialize("element_bytes called on non-indexed seq".into()))
			}
			// `validate_strided_seq_prologue` established `len * stride ==
			// body.len()` at construction, so this slice is always in range —
			// but it is expressed as a fallible lookup rather than an indexing
			// panic so that the unvalidated constructor cannot be made to
			// panic through this path.
			ElementIndex::Stride(stride) => {
				let start = index * stride;
				self.body
					.get(start..start + stride)
					.ok_or_else(|| Error::Deserialize("strided element out of range".into()))
			}
			ElementIndex::Offsets(offsets) => {
				let start = Self::offset_at(offsets, index) as usize;
				let end = if index + 1 < self.len {
					Self::offset_at(offsets, index + 1) as usize
				} else {
					self.body.len()
				};
				Ok(&self.body[start..end])
			}
		}
	}

	/// Raw bytes for the body (post-prologue). Used by legacy-fallback iteration.
	#[inline]
	pub fn body(&self) -> &'p [u8] {
		self.body
	}
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

	fn build_strided_seq(elements: &[&[u8]], stride: usize) -> Vec<u8> {
		let mut out = vec![FLAG_INDEXED | FLAG_STRIDED];
		out.extend_from_slice(&varint(elements.len()));
		out.extend_from_slice(&varint(stride));
		for e in elements {
			out.extend_from_slice(e);
		}
		out
	}

	#[test]
	fn opens_strided_seq_and_reads_elements() {
		let payload = build_strided_seq(&[b"foo", b"bar", b"baz"], 3);
		let w: IndexedSeqWalker<()> = IndexedSeqWalker::from_payload(&payload).unwrap();
		assert!(w.is_indexed());
		assert_eq!(w.stride(), Some(3));
		assert_eq!(w.len(), 3);
		assert_eq!(w.element_bytes(0).unwrap(), b"foo");
		assert_eq!(w.element_bytes(1).unwrap(), b"bar");
		assert_eq!(w.element_bytes(2).unwrap(), b"baz");
		assert!(w.element_bytes(3).is_err(), "index past the end must not slice the body");
	}

	#[test]
	fn strided_seq_survives_a_multi_byte_stride_varint() {
		// A 768-dim f64 vector's elements are 16 bytes each; a stride that
		// crosses the one-byte varint boundary must round-trip too.
		let element = vec![7u8; 300];
		let refs: Vec<&[u8]> = (0..8).map(|_| element.as_slice()).collect();
		let payload = build_strided_seq(&refs, 300);
		let w: IndexedSeqWalker<()> = IndexedSeqWalker::from_payload(&payload).unwrap();
		assert_eq!(w.stride(), Some(300));
		assert_eq!(w.len(), 8);
		assert_eq!(w.element_bytes(7).unwrap(), element.as_slice());
	}

	#[test]
	fn rejects_strided_geometry_mismatch() {
		// Declares 4 elements of 3 bytes but carries only 9 bytes of body.
		let mut payload = vec![FLAG_INDEXED | FLAG_STRIDED, 4, 3];
		payload.extend_from_slice(b"aaabbbccc");
		let err = IndexedSeqWalker::<()>::from_payload(&payload).unwrap_err();
		assert!(matches!(err, Error::OptimisedStrideMismatch { .. }));
	}

	#[test]
	fn rejects_zero_stride() {
		let payload = vec![FLAG_INDEXED | FLAG_STRIDED, 4, 0];
		let err = IndexedSeqWalker::<()>::from_payload(&payload).unwrap_err();
		assert!(matches!(err, Error::OptimisedStrideMismatch { .. }));
	}

	#[test]
	fn strided_geometry_is_checked_even_when_unvalidated() {
		// `from_payload_unvalidated` skips the O(len) offset-table walk, but
		// the strided geometry check is a single multiply and is what makes
		// `element_bytes` total, so it is never skipped.
		let mut payload = vec![FLAG_INDEXED | FLAG_STRIDED, 4, 3];
		payload.extend_from_slice(b"aaabbbccc");
		let err = IndexedSeqWalker::<()>::from_payload_unvalidated(&payload).unwrap_err();
		assert!(matches!(err, Error::OptimisedStrideMismatch { .. }));
	}

	#[test]
	fn strided_flag_without_indexed_flag_reads_as_legacy_body() {
		// `FLAG_STRIDED` is only meaningful next to `FLAG_INDEXED`; on its own
		// the prologue is a legacy body and carries no stride varint.
		let payload = [FLAG_STRIDED, 2, 1, 2];
		let w: IndexedSeqWalker<()> = IndexedSeqWalker::from_payload(&payload).unwrap();
		assert!(!w.is_indexed());
		assert_eq!(w.stride(), None);
		assert_eq!(w.body(), &[1u8, 2]);
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
