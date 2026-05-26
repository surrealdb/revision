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
///
/// # Safety
///
/// This trait is `unsafe` to implement because the crate's unsafe code
/// (in particular [`crate::optimised::envelope::read_varlen_slice`], the
/// `walk_<field>` Wire-fast-path in macro-generated walkers, and the
/// optimised walker constructors more broadly) relies on a stronger
/// contract than could be expressed in safe Rust:
///
/// 1. **`peek_bytes(n)` and [`remaining`](Self::remaining) must return
///    slices that point into stable, addressable memory** — not a
///    transient buffer that could be moved, reallocated, or overwritten
///    by a subsequent call.
///
/// 2. **`advance(n)` must only move the cursor; it must not invalidate,
///    move, or mutate the bytes already returned by `peek_bytes` or
///    `remaining`.** In particular, an impl that holds bytes in an
///    internal `Vec<u8>` and refills the `Vec` on `advance` (e.g. a
///    buffered file reader) does **not** satisfy this contract, because
///    previously-returned slice pointers would dangle after the refill.
///
/// 3. **Bytes returned by `peek_bytes` or `remaining` must remain valid
///    for the reader's lifetime** (i.e. for `'r` where the reader is
///    borrowed as `&'r mut R`), regardless of how many `advance` calls
///    happen in between, so the unsafe code extending peek lifetimes via
///    [`read_borrowed_bytes`] and the macro-emitted `walk_<field>` Wire
///    fast path do not create dangling pointers.
///
/// 4. **`remaining().len()` is monotonically non-increasing under
///    `advance`** — for any successful `advance` call,
///    `remaining_after.len() <= remaining_before.len()` must hold.
///    The macro-emitted Wire fast path computes the consumed byte
///    count as `remaining_before.len() - remaining_after.len()` and
///    relies on the result being non-negative; an impl that ever
///    grows `remaining` across an `advance` would underflow this
///    subtraction. (The macro guards against underflow with a
///    `checked_sub` that returns a corrupt-impl error rather than
///    triggering UB, but in-crate impls and any reasonable downstream
///    impl must honour this invariant.) The bullet deliberately does
///    *not* require that `advance(n)` reduces `remaining` by exactly
///    `n` — an impl that coalesces, buffers, or otherwise chooses a
///    larger reduction is still sound for the unsafe code that
///    depends on this contract.
///
///    *Semantic caveat:* the UB-safety promise above is **only** about
///    memory safety. The macro-emitted `walk_<field>` Wire fast path
///    additionally interprets `remaining_before.len() -
///    remaining_after.len()` as "the number of bytes the
///    `skip_indexed_*` call consumed", and hands those bytes to
///    `IndexedMapView` / `IndexedSeqView` / `IndexedSetView` for
///    parsing. An impl that reduces `remaining` by *more* than what
///    `skip_*` actually visited (e.g. by coalescing reads or
///    pre-fetching the next field) would still be UB-safe, but the
///    view would see trailing bytes the skip didn't consume and
///    misparse the field. The two in-crate impls (`&[u8]`,
///    `SliceReader<'a>`) advance by exactly `n`, so this is dormant
///    in practice — but a downstream impl that wants to be both
///    UB-safe **and** semantically correct against the macro should
///    advance by exactly `n` as well.
///
/// The two impls in this crate — `&[u8]` and [`SliceReader`] — both
/// trivially satisfy this contract: `peek_bytes` and `remaining` return
/// slices into a caller-owned buffer that the reader never mutates, and
/// `advance` is a pure cursor bump that only ever reduces `remaining`.
///
/// Violating any of these is **undefined behaviour**, not just a logic
/// bug; the crate's unsafe code is correct only under this contract.
pub unsafe trait BorrowedReader: Read {
	/// Borrow the next `n` bytes without advancing the cursor.
	///
	/// Returns the slice on success. The slice must point into stable
	/// memory that survives subsequent `advance` calls — see the trait's
	/// safety contract.
	fn peek_bytes(&self, n: usize) -> Result<&[u8], Error>;

	/// Advance the cursor past `n` bytes without copying them.
	///
	/// Equivalent to reading `n` bytes and discarding them, but cheaper
	/// (the bytes are never touched). Returns an error if fewer than
	/// `n` bytes remain.
	///
	/// **Safety**: must not invalidate or move bytes returned by previous
	/// `peek_bytes` calls — see the trait's safety contract.
	fn advance(&mut self, n: usize) -> Result<(), Error>;

	/// Bytes consumed since the reader was constructed.
	///
	/// Used by optimised walkers and the encode side to compute offsets
	/// relative to the start of an optimised compound's payload.
	fn position(&self) -> usize;

	/// The unconsumed tail as a borrowed slice.
	///
	/// Returns a view of the bytes the reader has yet to read, tied to the
	/// reader's borrow. Unlike [`peek_bytes`](Self::peek_bytes), the caller
	/// does not have to know the length in advance — and unlike `position`,
	/// the returned slice is concrete bytes, not a count.
	///
	/// **Use case**: capture before/after slices around a `skip`-style call
	/// to recover the field's exact wire bytes without decoding them:
	///
	/// ```ignore
	/// let before = reader.remaining();        // &[u8] of all unread bytes
	/// some_skip_routine(&mut reader)?;        // advances past one logical value
	/// let after = reader.remaining();
	/// let consumed_bytes = &before[..before.len() - after.len()];
	/// // `consumed_bytes` is the just-skipped value's wire bytes, zero-copy.
	/// ```
	///
	/// Any impl that is used as the source for an optimised-walker's
	/// `walk_<field>` accessor **must** override this. The default
	/// implementation `debug_assert!`s on call (panicking in debug builds
	/// to surface the missing override) and returns `&[]` in release
	/// (which would produce silently-empty field views — equally bad, but
	/// at least not crashing). Both in-crate impls (`&[u8]` and
	/// `SliceReader<'a>`) override; any new downstream `BorrowedReader`
	/// impl should too, even if it only uses the legacy walker paths
	/// (which don't consult `remaining`) — the override is cheap and the
	/// foot-gun cost is much higher than the cost of writing it.
	#[inline]
	fn remaining(&self) -> &[u8] {
		debug_assert!(
			false,
			"BorrowedReader::remaining() default impl invoked — your impl must \
			 override it (returns &[]; the macro-emitted walk_<field> Wire fast \
			 path will produce silently empty field views otherwise)"
		);
		&[]
	}
}

// SAFETY: `&[u8]::peek_bytes` returns a slice into the caller-owned buffer the
// reference points at; `advance` only updates the slice reference (cursor),
// never touching the underlying bytes. Both invariants hold for any caller-
// provided buffer.
unsafe impl BorrowedReader for &[u8] {
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

	#[inline]
	fn remaining(&self) -> &[u8] {
		// `*self` is `&[u8]` — the entire unread tail by definition (advancing
		// replaces the slice with its suffix).
		self
	}
}

// SAFETY: Forwarding impl. Every method delegates to the underlying `R`,
// which is itself `BorrowedReader` and therefore obeys the trait's safety
// contract (stable backing buffer, non-mutating `peek_bytes` / `remaining`,
// monotonic non-increasing `remaining().len()` under `advance`). The
// forwarding adds no state and cannot violate any invariant the inner impl
// upholds.
//
// This impl is what lets the macro-emitted `walk_<field>` / `skip_<field>`
// paths call `skip_indexed_*` with a `reader: &mut &'r mut R` binding
// (extracted from the `Wire` repr variant) without an explicit reborrow.
unsafe impl<R: BorrowedReader + ?Sized> BorrowedReader for &mut R {
	#[inline]
	fn peek_bytes(&self, n: usize) -> Result<&[u8], Error> {
		(**self).peek_bytes(n)
	}

	#[inline]
	fn advance(&mut self, n: usize) -> Result<(), Error> {
		(**self).advance(n)
	}

	#[inline]
	fn position(&self) -> usize {
		(**self).position()
	}

	#[inline]
	fn remaining(&self) -> &[u8] {
		(**self).remaining()
	}
}

// SAFETY: `SliceReader<'a>` borrows an external `&'a [u8]` it never modifies.
// `peek_bytes` returns slices into that external buffer; `advance` only updates
// the internal cursor (`inner`), never the buffer. Both invariants hold.
unsafe impl<'a> BorrowedReader for SliceReader<'a> {
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

	#[inline]
	fn remaining(&self) -> &[u8] {
		self.inner
	}
}

/// Peek `n` bytes from `reader`, advance past them, and return the peeked
/// slice with the reader's full `'r` lifetime.
///
/// This is the canonical "borrow body bytes from a slice-backed reader" helper
/// used by the optimised wire format's runtime and macro-emitted walkers. It
/// replaces 4 copies of the same `peek_bytes + advance + slice::from_raw_parts`
/// dance and is the single audit point for the unsafe lifetime extension.
///
/// The unsafe block is sound because [`BorrowedReader`] is itself an `unsafe
/// trait`: every conforming impl guarantees that the bytes returned by
/// `peek_bytes` remain valid for the reader's lifetime regardless of how many
/// `advance` calls happen in between. See the [`BorrowedReader`] safety
/// contract for the full requirements.
#[inline]
pub fn read_borrowed_bytes<'r, R: BorrowedReader + ?Sized>(
	reader: &'r mut R,
	n: usize,
) -> Result<&'r [u8], Error> {
	let peeked = reader.peek_bytes(n)?;
	let ptr = peeked.as_ptr();
	reader.advance(n)?;
	// SAFETY: `peek_bytes(n)` returned a slice of length `n` from `reader`'s
	// underlying buffer. By the `unsafe trait BorrowedReader` contract,
	// `advance(n)` only moves the cursor and must not invalidate or move the
	// peeked bytes. Therefore the slice [ptr, ptr+n) remains valid for the
	// reader's lifetime `'r`. Reconstructing the slice with the extended
	// lifetime is sound because nothing between here and `'r`'s end can move
	// or free the underlying buffer.
	let slice: &'r [u8] = unsafe { std::slice::from_raw_parts(ptr, n) };
	Ok(slice)
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
