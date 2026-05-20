//! Indexed-struct walker.
//!
//! Layout of an indexed-struct payload (after the outer revision tag and the
//! `u32_le byte_length` envelope have been consumed):
//!
//! ```text
//! payload:
//!   [u32_le field_off; field_count]      // prologue, field_count * 4 bytes
//!   field_0 || field_1 || ... || field_{field_count - 1}
//! ```
//!
//! Each offset is an absolute byte index into `payload`. The first offset is
//! always `field_count * 4` (just past the prologue). Walker construction
//! validates monotonicity once; per-field access is then O(1).

use crate::DeserializeRevisioned;
use crate::Error;
use crate::optimised::validation::validate_struct_prologue;

/// Walker over an indexed-struct payload borrowed from `&'p [u8]`.
///
/// Constructed by the derive macro when a type opts into
/// `#[revisioned(revision(N, optimised, indexed_struct))]`.
/// Hand-constructed use is supported for testing and surrealdb-style
/// pre-decode filters.
///
/// ```
/// use revision::optimised::IndexedStructWalker;
/// use revision::prelude::*;
///
/// #[revisioned(revision(1, optimised, indexed_struct))]
/// #[derive(PartialEq, Debug)]
/// struct Doc {
///     id: u32,
///     name: String,
///     value: u64,
/// }
///
/// let doc = Doc { id: 42, name: "answer".into(), value: 9001 };
/// let bytes = revision::to_vec(&doc).unwrap();
///
/// // Strip the outer envelope: u16 revision + u32_le payload length.
/// // u16 width depends on the `fixed-width-encoding` cargo feature, so probe at runtime.
/// let rev_len = {
///     let mut buf = Vec::new();
///     <u16 as SerializeRevisioned>::serialize_revisioned(&1u16, &mut buf).unwrap();
///     buf.len()
/// };
/// let payload = &bytes[rev_len + 4..];
///
/// let w = IndexedStructWalker::from_payload(payload, 1, 3).unwrap();
/// assert_eq!(w.decode_field::<u32>(0).unwrap(), 42);
/// assert_eq!(w.decode_field::<String>(1).unwrap(), "answer");
/// ```
#[derive(Debug)]
pub struct IndexedStructWalker<'p> {
	payload: &'p [u8],
	field_count: u16,
	revision: u16,
}

impl<'p> IndexedStructWalker<'p> {
	/// Open an indexed-struct walker over an already-extracted payload slice.
	///
	/// `field_count` comes from the type definition (the macro emits the literal).
	/// Performs eager prologue validation — monotonic offsets, last offset
	/// within payload, first offset past the prologue. Use
	/// [`from_payload_unvalidated`](Self::from_payload_unvalidated) when the
	/// payload is trusted (e.g. freshly written by the same process) and the
	/// O(field_count) validation cost is measurable on the hot path.
	pub fn from_payload(payload: &'p [u8], revision: u16, field_count: u16) -> Result<Self, Error> {
		let prologue_bytes = (field_count as usize) * 4;
		if payload.len() < prologue_bytes {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		let offsets = parse_offsets(&payload[..prologue_bytes]);
		validate_struct_prologue(&offsets, payload.len() as u32)?;
		// First offset must point at start of body (`prologue_bytes`).
		if let Some(&first) = offsets.first()
			&& (first as usize) < prologue_bytes
		{
			return Err(Error::OptimisedOffsetOutOfRange {
				offset: first,
				payload_len: prologue_bytes as u32,
			});
		}
		Ok(Self {
			payload,
			field_count,
			revision,
		})
	}

	/// Open an indexed-struct walker **without** validating the prologue.
	///
	/// Skips the O(field_count) offset-table check that [`from_payload`] runs.
	/// Use only when the bytes are trusted — typically when they were
	/// produced by `to_vec` or another in-process serialiser in the same
	/// run.
	///
	/// # Panics on malformed input
	///
	/// On untrusted input this trades a clean
	/// [`Error::OptimisedOffsetsNonMonotonic`] /
	/// [`Error::OptimisedOffsetOutOfRange`] at construction for a **panic
	/// on field access** if the offset table is corrupt. Specifically:
	///
	/// - The offset *table itself* (the `field_count * 4` byte prologue)
	///   is bounds-checked at construction — the up-front
	///   `payload.len() < prologue_bytes` check returns
	///   `Error::OptimisedSubReaderOverrun` if the payload is too short
	///   to hold the table at all, so reading the four bytes for any
	///   given offset is safe.
	/// - The offset *values* read from that table are not checked.
	///   [`field_bytes`](Self::field_bytes) and
	///   [`decode_field`](Self::decode_field) slice the payload by those
	///   values: an offset that exceeds `payload.len()` or whose
	///   neighbour is smaller (non-monotonic) triggers a slice-out-of-
	///   bounds panic via standard `Index`/`Range` bounds checking.
	///
	/// This is the intended behaviour: the caller asserted trust by
	/// choosing this constructor, and a panic on corrupted "trusted"
	/// bytes signals that the trust assumption was wrong. Callers who
	/// cannot make that assertion should use [`from_payload`] instead,
	/// which fails cleanly with a typed error.
	///
	/// Returns `Error::OptimisedSubReaderOverrun` only when the payload
	/// is too short to hold the offset table itself — that check is
	/// kept because constructing the walker over a payload shorter than
	/// `field_count * 4` would have no defensible interpretation, and
	/// keeps the in-bounds guarantee for offset-table reads above.
	///
	/// [`from_payload`]: Self::from_payload
	/// [`Error::OptimisedOffsetsNonMonotonic`]: crate::Error::OptimisedOffsetsNonMonotonic
	/// [`Error::OptimisedOffsetOutOfRange`]: crate::Error::OptimisedOffsetOutOfRange
	pub fn from_payload_unvalidated(
		payload: &'p [u8],
		revision: u16,
		field_count: u16,
	) -> Result<Self, Error> {
		let prologue_bytes = (field_count as usize) * 4;
		if payload.len() < prologue_bytes {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		Ok(Self {
			payload,
			field_count,
			revision,
		})
	}

	/// Wire revision recorded in the outer envelope (passed through by caller).
	#[inline]
	pub fn revision(&self) -> u16 {
		self.revision
	}

	/// Number of fields recorded in the prologue.
	#[inline]
	pub fn field_count(&self) -> u16 {
		self.field_count
	}

	/// Borrow the bytes for field `index`. O(1).
	pub fn field_bytes(&self, index: u16) -> Result<&'p [u8], Error> {
		let i = index as usize;
		if i >= self.field_count as usize {
			return Err(Error::Deserialize(format!(
				"field index {i} out of range ({})",
				self.field_count
			)));
		}
		let start = self.offset(i) as usize;
		let end = if i + 1 < self.field_count as usize {
			self.offset(i + 1) as usize
		} else {
			self.payload.len()
		};
		Ok(&self.payload[start..end])
	}

	/// Decode field `index` as `T`.
	pub fn decode_field<T: DeserializeRevisioned>(&self, index: u16) -> Result<T, Error> {
		let mut bytes = self.field_bytes(index)?;
		T::deserialize_revisioned(&mut bytes)
	}

	/// Skip field `index`. Free under indexed encoding — the offset table already
	/// makes seeking past the field a constant-time arithmetic operation.
	#[inline]
	pub fn skip_field(&self, _index: u16) -> Result<(), Error> {
		Ok(())
	}

	#[inline]
	fn offset(&self, index: usize) -> u32 {
		let start = index * 4;
		let bytes = &self.payload[start..start + 4];
		u32::from_le_bytes(bytes.try_into().expect("4-byte slice"))
	}
}

// `walk_field` is intentionally omitted: returning a walker that borrows from a
// SliceReader constructed inside the method would dangle. Callers construct the
// SliceReader themselves and pass it to `T::walk_revisioned`:
//
// ```ignore
// let bytes = walker.field_bytes(idx)?;
// let mut sub = SliceReader::new(bytes);
// let child = T::walk_revisioned(&mut sub)?;
// // ... use `child` here; both `sub` and `child` must die before `walker.payload`.
// ```
//
// The macro emits this pattern directly per field.

#[inline]
fn parse_offsets(bytes: &[u8]) -> Vec<u32> {
	bytes.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn build_struct_payload(field_data: &[&[u8]]) -> Vec<u8> {
		let field_count = field_data.len();
		let prologue_bytes = field_count * 4;
		let mut offsets = Vec::with_capacity(field_count);
		let mut running = prologue_bytes as u32;
		for f in field_data {
			offsets.push(running);
			running += f.len() as u32;
		}
		let mut out = Vec::with_capacity(running as usize);
		for o in &offsets {
			out.extend_from_slice(&o.to_le_bytes());
		}
		for f in field_data {
			out.extend_from_slice(f);
		}
		out
	}

	#[test]
	fn opens_and_reads_field_bytes_in_order() {
		let payload = build_struct_payload(&[b"alpha", b"beta", b"gamma"]);
		let w = IndexedStructWalker::from_payload(&payload, 2, 3).unwrap();
		assert_eq!(w.field_count(), 3);
		assert_eq!(w.revision(), 2);
		assert_eq!(w.field_bytes(0).unwrap(), b"alpha");
		assert_eq!(w.field_bytes(1).unwrap(), b"beta");
		assert_eq!(w.field_bytes(2).unwrap(), b"gamma");
	}

	#[test]
	fn rejects_out_of_range_field() {
		let payload = build_struct_payload(&[b"a", b"b"]);
		let w = IndexedStructWalker::from_payload(&payload, 1, 2).unwrap();
		assert!(w.field_bytes(2).is_err());
	}

	#[test]
	fn rejects_truncated_prologue() {
		let payload = [0u8, 0, 0]; // 3 bytes but field_count = 2 needs 8.
		let err = IndexedStructWalker::from_payload(&payload, 1, 2).unwrap_err();
		assert!(matches!(err, Error::OptimisedSubReaderOverrun));
	}

	#[test]
	fn rejects_offset_out_of_range() {
		// field_count = 1 → 4 bytes of prologue, but offset says 100.
		let mut payload = vec![0u8; 8];
		payload[0..4].copy_from_slice(&100u32.to_le_bytes());
		let err = IndexedStructWalker::from_payload(&payload, 1, 1).unwrap_err();
		assert!(matches!(err, Error::OptimisedOffsetOutOfRange { .. }));
	}

	#[test]
	fn rejects_non_monotonic_offsets() {
		// field_count = 2, prologue = 8 bytes, then 16 bytes of data.
		let mut payload = vec![0u8; 8 + 16];
		// Offsets: [16, 8] — non-monotonic.
		payload[0..4].copy_from_slice(&16u32.to_le_bytes());
		payload[4..8].copy_from_slice(&8u32.to_le_bytes());
		let err = IndexedStructWalker::from_payload(&payload, 1, 2).unwrap_err();
		assert!(matches!(err, Error::OptimisedOffsetsNonMonotonic));
	}

	#[test]
	fn rejects_first_offset_before_prologue_end() {
		// field_count = 1, prologue = 4 bytes, but first offset says 2.
		let mut payload = vec![0u8; 8];
		payload[0..4].copy_from_slice(&2u32.to_le_bytes());
		let err = IndexedStructWalker::from_payload(&payload, 1, 1).unwrap_err();
		assert!(matches!(err, Error::OptimisedOffsetOutOfRange { .. }));
	}
}
