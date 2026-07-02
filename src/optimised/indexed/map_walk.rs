//! Indexed-map walker.
//!
//! Layout of an indexed-map payload (after the outer envelope has been opened):
//!
//! ```text
//! u8 flags                                  // bit 0: indexed
//! varint len                                // entry count
//! if flags.0:
//!     [(u32_le key_off, u32_le val_off); len]
//!     K_0 || K_1 || ... || K_{len-1}        // dense keys, ascending
//!     V_0 || V_1 || ... || V_{len-1}        // dense values, matching order
//! else:
//!     (K, V)*                               // legacy-shape body for small maps
//! ```
//!
//! Each offset is into its dense region (keys for `key_offsets`, values for
//! `val_offsets`). Walker construction validates the prologue and the key
//! region's ascending invariant.

use std::cmp::Ordering;
use std::marker::PhantomData;

use crate::Error;
use crate::optimised::indexed::seq_walk::FLAG_INDEXED;
use crate::optimised::validation::{validate_key_region_ascending, validate_map_prologue};

/// Walker over an indexed-map body.
///
/// Like [`IndexedSeqWalker`](super::IndexedSeqWalker), the offset table is
/// borrowed directly from the payload. The wire format stores key and value
/// offsets interleaved (`(u32_le key_off, u32_le val_off); count`), so we
/// keep them as a single `&'p [u8]` of `count * 8` bytes; the key column
/// lives at stride-8 offsets `0, 8, 16, …` and the value column at
/// `4, 12, 20, …`. Decoding entries on demand via [`key_off`] / [`val_off`]
/// trades one `u32::from_le_bytes` per access (essentially free) for
/// eliminating the two `Vec<u32>` allocations that the previous shape paid
/// on every walker construction — a measurable per-row cost on scan
/// workloads.
///
/// [`key_off`]: MapPrologue::key_off
/// [`val_off`]: MapPrologue::val_off
#[derive(Debug)]
pub struct IndexedMapWalker<'p, K, V> {
	/// Bytes past the flags+len header. Either the (offsets + keys + values) layout
	/// in indexed mode, or the legacy `(K, V)*` body.
	body: &'p [u8],
	/// `None` on the legacy path.
	prologue: Option<MapPrologue<'p>>,
	len: usize,
	_marker: PhantomData<fn() -> (K, V)>,
}

/// Result of [`IndexedMapWalker::find_value_bytes_with_hint`].
#[derive(Debug, Clone, Copy)]
pub struct HintedLookup<'p> {
	/// The matching entry's value bytes, or `None` if the key is absent
	/// from this map.
	pub value: Option<&'p [u8]>,
	/// The slot to pass as `hint` on the next call against a same-shape
	/// map. `None` only when no call has ever found the key (nothing
	/// learned yet); once a hint exists, a later miss carries the old one
	/// forward rather than discarding it.
	pub hint: Option<u32>,
}

#[derive(Debug)]
struct MapPrologue<'p> {
	/// `count * 8` bytes of interleaved `(u32_le key_off, u32_le val_off)`
	/// pairs, borrowed from the payload.
	offset_table: &'p [u8],
	keys_region: &'p [u8],
	vals_region: &'p [u8],
}

impl<'p> MapPrologue<'p> {
	/// Decode the `index`-th key offset (stride 8, column 0).
	#[inline]
	fn key_off(&self, index: usize) -> u32 {
		crate::optimised::validation::decode_u32_le_at(self.offset_table, index * 8)
	}

	/// Decode the `index`-th value offset (stride 8, column 1).
	#[inline]
	fn val_off(&self, index: usize) -> u32 {
		crate::optimised::validation::decode_u32_le_at(self.offset_table, index * 8 + 4)
	}

	/// Byte ranges (into [`keys_region`](Self::keys_region) /
	/// [`vals_region`](Self::vals_region)) of entry `index`, or `None` if
	/// `index >= n`. Shared by the binary search and the hinted-probe path
	/// in [`IndexedMapWalker::find_value_bytes_with_hint`] so both derive
	/// entry boundaries the same way.
	#[inline]
	fn entry_ranges(
		&self,
		n: usize,
		index: usize,
	) -> Option<(std::ops::Range<usize>, std::ops::Range<usize>)> {
		if index >= n {
			return None;
		}
		let k_start = self.key_off(index) as usize;
		let v_start = self.val_off(index) as usize;
		let (k_end, v_end) = if index + 1 < n {
			(self.key_off(index + 1) as usize, self.val_off(index + 1) as usize)
		} else {
			(self.keys_region.len(), self.vals_region.len())
		};
		Some((k_start..k_end, v_start..v_end))
	}
}

impl<'p, K, V> IndexedMapWalker<'p, K, V> {
	pub fn from_payload(payload: &'p [u8]) -> Result<Self, Error> {
		Self::from_payload_inner(payload, true)
	}

	/// Open a walker **without** validating the prologue (monotonic offsets,
	/// ascending key region).
	///
	/// Skips the O(len) validation that [`from_payload`] runs. Construction
	/// only bounds-checks the offset *tables* and region-length headers
	/// (returning [`Error::OptimisedSubReaderOverrun`] on a truncated
	/// payload); the offset *values* in those tables are trusted.
	///
	/// # Behaviour on malformed input
	///
	/// The per-entry accessors never panic on a bad offset — they are
	/// bounds-checked at the point of use, so this constructor is safe to
	/// call on untrusted bytes (important under `panic = 'abort'`, where a
	/// slice-OOB panic would abort the whole process). It trades the clean
	/// [`Error::OptimisedOffsetsNonMonotonic`] /
	/// [`Error::OptimisedKeyRegionNotAscending`] that [`from_payload`]
	/// reports *at construction* for the following weaker guarantees *on
	/// access*:
	///
	/// - [`find_value_bytes`](Self::find_value_bytes) returns
	///   [`Error::OptimisedOffsetOutOfRange`] instead of slicing out of
	///   bounds when an offset is past its region or non-monotonic.
	/// - [`entries`](Self::entries) clamps an out-of-range or inverted
	///   `(start, end)` to an empty slice for that entry rather than
	///   panicking.
	/// - Binary-search lookups still assume the keys region is byte-ascending.
	///   On a non-ascending (corrupt) region the search does not panic and
	///   does not read out of bounds, but may report a present key as absent
	///   (it silently converges on the wrong slot).
	///
	/// Callers that cannot tolerate the wrong-result-on-corruption case
	/// should use [`from_payload`]; callers protected by an upstream
	/// integrity check (e.g. storage-engine block checksums) can take this
	/// fast path and recover from the access-time error by falling back to a
	/// full decode.
	///
	/// [`from_payload`]: Self::from_payload
	/// [`Error::OptimisedOffsetsNonMonotonic`]: crate::Error::OptimisedOffsetsNonMonotonic
	/// [`Error::OptimisedKeyRegionNotAscending`]: crate::Error::OptimisedKeyRegionNotAscending
	pub fn from_payload_unvalidated(payload: &'p [u8]) -> Result<Self, Error> {
		Self::from_payload_inner(payload, false)
	}

	fn from_payload_inner(payload: &'p [u8], validate: bool) -> Result<Self, Error> {
		if payload.is_empty() {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		let flags = payload[0];
		let mut cursor = 1usize;
		let (len, varint_bytes) = read_varint(&payload[cursor..])?;
		cursor += varint_bytes;
		let indexed = (flags & FLAG_INDEXED) != 0;

		if !indexed {
			return Ok(Self {
				body: &payload[cursor..],
				prologue: None,
				len,
				_marker: PhantomData,
			});
		}

		let table_bytes = len
			.checked_mul(8)
			.ok_or_else(|| Error::Deserialize("indexed-map offset table size overflow".into()))?;
		if payload.len() < cursor + table_bytes {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		// Borrow the interleaved `(key_off, val_off)` table directly from the
		// payload; per-entry decode happens on access via `MapPrologue::key_off`
		// / `val_off`. No `Vec<u32>` allocation per walker construction.
		let offset_table = &payload[cursor..cursor + table_bytes];
		cursor += table_bytes;

		// `u32_le keys_region_len` and `u32_le vals_region_len` follow the
		// interleaved offset table; together they tell us where the keys
		// region ends and the vals region begins.
		if payload.len() < cursor + 8 {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		let keys_region_len =
			u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap()) as usize;
		cursor += 4;
		let vals_region_len =
			u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap()) as usize;
		cursor += 4;

		if payload.len() < cursor + keys_region_len + vals_region_len {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		let keys_region = &payload[cursor..cursor + keys_region_len];
		cursor += keys_region_len;
		let vals_region = &payload[cursor..cursor + vals_region_len];

		if validate {
			validate_map_prologue(
				offset_table,
				len,
				keys_region_len as u32,
				vals_region_len as u32,
			)?;
			validate_key_region_ascending(keys_region, offset_table, len)?;
		}

		Ok(Self {
			body: &payload[1 + varint_bytes..],
			prologue: Some(MapPrologue {
				offset_table,
				keys_region,
				vals_region,
			}),
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
		self.prologue.is_some()
	}

	/// Iterate key/value byte ranges in original insertion order. Indexed path only.
	///
	/// Forward-only by design: we carry `(k_start, v_start)` across iterations
	/// and decode only the *next* entry's offsets each step, reusing them as
	/// the current entry's end. This halves the per-entry decode count
	/// relative to the naive `decode(i), decode(i+1)` pattern. The returned
	/// iterator is `impl Iterator` (no `DoubleEndedIterator`), so callers
	/// can't step backwards and observe stale state.
	pub fn entries(&self) -> Option<impl Iterator<Item = (&'p [u8], &'p [u8])> + '_> {
		let p = self.prologue.as_ref()?;
		let keys = p.keys_region;
		let vals = p.vals_region;
		let n = self.len;
		let (mut k_start, mut v_start) = if n > 0 {
			(p.key_off(0) as usize, p.val_off(0) as usize)
		} else {
			(0, 0)
		};
		Some((0..n).map(move |i| {
			let (k_end, v_end) = if i + 1 < n {
				(p.key_off(i + 1) as usize, p.val_off(i + 1) as usize)
			} else {
				(keys.len(), vals.len())
			};
			// Checked slicing: a walker opened via `from_payload` has a
			// validated prologue so these ranges are always in-range and
			// monotonic. On `from_payload_unvalidated` a corrupt offset
			// (`k_start > k_end`, or past the region end) would otherwise
			// panic — aborting the process under `panic = 'abort'`. Clamp
			// to an empty slice so the caller observes a non-matching entry
			// and recovers (e.g. falls back to a full decode) instead.
			let entry = (
				keys.get(k_start..k_end).unwrap_or_default(),
				vals.get(v_start..v_end).unwrap_or_default(),
			);
			k_start = k_end;
			v_start = v_end;
			entry
		}))
	}

	/// Borrow the value bytes for the entry whose key bytes compare `Equal`
	/// under `predicate`. Uses binary search when indexed; linear scan when not.
	pub fn find_value_bytes<F>(&self, predicate: F) -> Result<Option<&'p [u8]>, Error>
	where
		F: FnMut(&[u8]) -> Ordering,
	{
		let Some(p) = &self.prologue else {
			return Err(Error::Deserialize("find_value_bytes called on non-indexed map".into()));
		};
		Ok(Self::binary_search_entry(p, self.len, predicate)?.map(|(value, _index)| value))
	}

	/// Like [`find_value_bytes`](Self::find_value_bytes), but probes a
	/// remembered `hint` slot before falling back to the binary search.
	///
	/// Intended for scan loops that call this repeatedly against a fixed
	/// `predicate` (the same needle key) over a sequence of maps with the
	/// same *shape* — e.g. successive rows of a table scan whose schema
	/// happens to be uniform. Same-shape maps sort their keys into the
	/// same slot every time, so probing the slot learned from the previous
	/// call turns the usual O(log n) search into one verified O(1) probe.
	///
	/// Returns the value bytes (`None` if the key is absent from this map)
	/// alongside the slot to pass as `hint` on the next call:
	/// - **Hint hits** (predicate reports `Equal` at the hinted slot):
	///   returned unchanged — the search rarely needs to run at all.
	/// - **Hint misses, but the binary search finds the key elsewhere**:
	///   the matched slot, so the caller's hint self-corrects toward
	///   whatever slot this differently-shaped map actually uses.
	/// - **Key not found at all**: the *original* `hint` is carried
	///   forward unchanged. A single row missing the field (common on
	///   schemaless tables) must not erase a hint that is still correct
	///   for every other row.
	///
	/// A `hint` that is out of range for this map (`>= len`, e.g. carried
	/// over from a smaller map of a different shape) is not itself
	/// evidence of payload corruption — it is a harmless miss that falls
	/// straight through to the binary search, never an [`Error`].
	pub fn find_value_bytes_with_hint<F>(
		&self,
		hint: Option<u32>,
		mut predicate: F,
	) -> Result<HintedLookup<'p>, Error>
	where
		F: FnMut(&[u8]) -> Ordering,
	{
		let Some(p) = &self.prologue else {
			return Err(Error::Deserialize(
				"find_value_bytes_with_hint called on non-indexed map".into(),
			));
		};
		let n = self.len;

		// A hint that fails to resolve to the needle — absent, out of
		// range, pointing at the wrong key, or (on an unvalidated walker)
		// at a corrupt key/value range — is a harmless miss, not proof of
		// corruption on its own; every failing branch here falls through
		// to the identical binary search below.
		if let Some(h) = hint
			&& let Some((k_range, v_range)) = p.entry_ranges(n, h as usize)
			&& let Some(key_bytes) = p.keys_region.get(k_range)
			&& predicate(key_bytes) == Ordering::Equal
			&& let Some(value_bytes) = p.vals_region.get(v_range)
		{
			return Ok(HintedLookup {
				value: Some(value_bytes),
				hint: Some(h),
			});
		}

		Ok(match Self::binary_search_entry(p, n, predicate)? {
			Some((value, index)) => HintedLookup {
				value: Some(value),
				hint: Some(index),
			},
			None => HintedLookup {
				value: None,
				hint,
			},
		})
	}

	/// Shared binary-search core for [`find_value_bytes`](Self::find_value_bytes)
	/// and [`find_value_bytes_with_hint`](Self::find_value_bytes_with_hint).
	/// Returns the matching entry's value bytes and its slot index (for
	/// hint-learning), or `None` if no entry compares `Equal`.
	fn binary_search_entry<F>(
		p: &MapPrologue<'p>,
		n: usize,
		mut predicate: F,
	) -> Result<Option<(&'p [u8], u32)>, Error>
	where
		F: FnMut(&[u8]) -> Ordering,
	{
		let mut lo = 0usize;
		let mut hi = n;
		while lo < hi {
			let mid = lo + (hi - lo) / 2;
			let (k_range, v_range) =
				p.entry_ranges(n, mid).expect("mid is within [0, n) by loop invariant");
			// Checked slicing: validated walkers (`from_payload`) always hit
			// the `Some` arm. On `from_payload_unvalidated` a corrupt offset
			// would otherwise panic and abort under `panic = 'abort'`; return
			// a recoverable error so the caller can fall back to a full decode.
			// `let .. else` rather than `ok_or`: this runs once per
			// binary-search probe on scan hot paths, and an eager `ok_or`
			// argument constructs and drops the error on every successful
			// probe — `Error`'s size and drop glue make that measurable.
			let Some(key_bytes) = p.keys_region.get(k_range.clone()) else {
				return Err(Error::OptimisedOffsetOutOfRange {
					offset: k_range.end as u32,
					payload_len: p.keys_region.len() as u32,
				});
			};
			match predicate(key_bytes) {
				Ordering::Equal => {
					let Some(value_bytes) = p.vals_region.get(v_range.clone()) else {
						return Err(Error::OptimisedOffsetOutOfRange {
							offset: v_range.end as u32,
							payload_len: p.vals_region.len() as u32,
						});
					};
					return Ok(Some((value_bytes, mid as u32)));
				}
				Ordering::Less => lo = mid + 1,
				Ordering::Greater => hi = mid,
			}
		}
		Ok(None)
	}

	/// Bytes for the legacy `(K, V)*` body. `None` on the indexed path.
	#[inline]
	pub fn legacy_body(&self) -> Option<&'p [u8]> {
		if self.prologue.is_some() {
			None
		} else {
			Some(self.body)
		}
	}
}

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

	fn build_indexed_map(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
		let mut sorted: Vec<(&[u8], &[u8])> = entries.to_vec();
		sorted.sort_by(|a, b| a.0.cmp(b.0));
		let n = sorted.len();
		let mut out = Vec::new();
		out.push(FLAG_INDEXED);
		out.extend_from_slice(&varint(n));
		// Offset tables
		let mut k_off = 0u32;
		let mut v_off = 0u32;
		let mut k_offsets = Vec::with_capacity(n);
		let mut v_offsets = Vec::with_capacity(n);
		for (k, v) in &sorted {
			k_offsets.push(k_off);
			v_offsets.push(v_off);
			k_off += k.len() as u32;
			v_off += v.len() as u32;
		}
		for i in 0..n {
			out.extend_from_slice(&k_offsets[i].to_le_bytes());
			out.extend_from_slice(&v_offsets[i].to_le_bytes());
		}
		// Region lengths (u32_le pair)
		out.extend_from_slice(&k_off.to_le_bytes());
		out.extend_from_slice(&v_off.to_le_bytes());
		// Dense keys
		for (k, _) in &sorted {
			out.extend_from_slice(k);
		}
		// Dense values
		for (_, v) in &sorted {
			out.extend_from_slice(v);
		}
		out
	}

	#[test]
	fn opens_indexed_map_and_finds_keys() {
		let payload = build_indexed_map(&[(b"foo", b"42"), (b"bar", b"7"), (b"baz", b"99")]);
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		assert_eq!(w.len(), 3);
		assert!(w.is_indexed());
		let found = w.find_value_bytes(|k| k.cmp(b"baz".as_slice())).unwrap().unwrap();
		assert_eq!(found, b"99");
		let found = w.find_value_bytes(|k| k.cmp(b"foo".as_slice())).unwrap().unwrap();
		assert_eq!(found, b"42");
		assert!(w.find_value_bytes(|k| k.cmp(b"missing".as_slice())).unwrap().is_none());
	}

	#[test]
	fn iter_returns_pairs_in_sorted_order() {
		let payload = build_indexed_map(&[(b"x", b"3"), (b"a", b"1"), (b"m", b"2")]);
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		let collected: Vec<(&[u8], &[u8])> = w.entries().unwrap().collect();
		assert_eq!(
			collected,
			vec![
				(b"a".as_slice(), b"1".as_slice()),
				(b"m".as_slice(), b"2".as_slice()),
				(b"x".as_slice(), b"3".as_slice())
			]
		);
	}

	#[test]
	fn legacy_map_passes_through() {
		// flags = 0, varint(2), then some legacy body bytes
		let payload = [0u8, 2, 0xAA, 0xBB];
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		assert!(!w.is_indexed());
		assert_eq!(w.len(), 2);
		assert_eq!(w.legacy_body().unwrap(), &[0xAA, 0xBB]);
		assert!(
			w.find_value_bytes(|_| Ordering::Equal).is_err(),
			"find_value_bytes errors on legacy path"
		);
	}

	/// Overwrite entry `i`'s `key_off` (stride-8 column 0) in a payload built
	/// by [`build_indexed_map`]: `FLAG_INDEXED` byte + 1-byte varint (n <= 250)
	/// puts the offset table at byte 2.
	fn corrupt_key_off(payload: &mut [u8], entry: usize, value: u32) {
		let at = 2 + entry * 8;
		payload[at..at + 4].copy_from_slice(&value.to_le_bytes());
	}

	#[test]
	fn validated_rejects_out_of_range_offset_at_construction() {
		let mut payload = build_indexed_map(&[(b"bar", b"7"), (b"baz", b"99"), (b"foo", b"42")]);
		corrupt_key_off(&mut payload, 1, u32::MAX);
		assert!(
			IndexedMapWalker::<(), ()>::from_payload(&payload).is_err(),
			"validating constructor must reject a key offset past the region"
		);
	}

	#[test]
	fn unvalidated_find_value_bytes_errors_instead_of_panicking() {
		let mut payload = build_indexed_map(&[(b"bar", b"7"), (b"baz", b"99"), (b"foo", b"42")]);
		// Corrupt the middle entry — binary search over n=3 probes mid=1 first.
		corrupt_key_off(&mut payload, 1, u32::MAX);
		let w: IndexedMapWalker<(), ()> =
			IndexedMapWalker::from_payload_unvalidated(&payload).unwrap();
		// Must return a recoverable error, not slice-OOB panic / abort.
		assert!(matches!(
			w.find_value_bytes(|k| k.cmp(b"baz".as_slice())),
			Err(Error::OptimisedOffsetOutOfRange { .. })
		));
	}

	#[test]
	fn unvalidated_entries_clamp_corrupt_offset_without_panicking() {
		let mut payload = build_indexed_map(&[(b"bar", b"7"), (b"baz", b"99"), (b"foo", b"42")]);
		corrupt_key_off(&mut payload, 1, u32::MAX);
		let w: IndexedMapWalker<(), ()> =
			IndexedMapWalker::from_payload_unvalidated(&payload).unwrap();
		// Collecting must not panic; corrupt ranges clamp to empty slices.
		let collected: Vec<(&[u8], &[u8])> = w.entries().unwrap().collect();
		assert_eq!(collected.len(), 3);
		assert!(collected.iter().any(|(k, _)| k.is_empty()));
	}

	/// Sorted order is `bar`(0), `baz`(1), `foo`(2).
	fn three_entry_map() -> Vec<u8> {
		build_indexed_map(&[(b"bar", b"7"), (b"baz", b"99"), (b"foo", b"42")])
	}

	#[test]
	fn hinted_lookup_hits_on_correct_hint() {
		let payload = three_entry_map();
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		// Learn the slot first (hint = None takes the plain binary search).
		let first = w.find_value_bytes_with_hint(None, |k| k.cmp(b"baz".as_slice())).unwrap();
		assert_eq!(first.value, Some(b"99".as_slice()));
		assert_eq!(first.hint, Some(1));
		// Probing that learned slot again must return the identical result.
		let second =
			w.find_value_bytes_with_hint(first.hint, |k| k.cmp(b"baz".as_slice())).unwrap();
		assert_eq!(second.value, Some(b"99".as_slice()));
		assert_eq!(second.hint, Some(1));
	}

	#[test]
	fn hinted_lookup_self_corrects_on_wrong_hint() {
		let payload = three_entry_map();
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		// Hint points at "bar" (slot 0) but the needle is "foo" (slot 2) —
		// e.g. a hint learned from a differently-shaped prior row.
		let out = w.find_value_bytes_with_hint(Some(0), |k| k.cmp(b"foo".as_slice())).unwrap();
		assert_eq!(out.value, Some(b"42".as_slice()));
		assert_eq!(out.hint, Some(2), "a wrong hint must self-correct to the real slot");
	}

	#[test]
	fn hinted_lookup_preserves_prior_hint_on_miss() {
		let payload = three_entry_map();
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		// A row missing the field entirely must not erase a hint that is
		// still correct for every other row.
		let out = w.find_value_bytes_with_hint(Some(1), |k| k.cmp(b"missing".as_slice())).unwrap();
		assert_eq!(out.value, None);
		assert_eq!(out.hint, Some(1), "a genuine miss must carry the old hint forward unchanged");
	}

	#[test]
	fn hinted_lookup_falls_through_on_out_of_range_hint() {
		let payload = three_entry_map();
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		// A stale hint from a smaller/differently-shaped map must be a
		// harmless miss, not a panic or an out-of-bounds read.
		let out = w.find_value_bytes_with_hint(Some(50), |k| k.cmp(b"foo".as_slice())).unwrap();
		assert_eq!(out.value, Some(b"42".as_slice()));
		assert_eq!(out.hint, Some(2));
	}

	#[test]
	fn hinted_lookup_no_hint_matches_plain_binary_search() {
		let payload = three_entry_map();
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		let plain = w.find_value_bytes(|k| k.cmp(b"bar".as_slice())).unwrap();
		let hinted = w.find_value_bytes_with_hint(None, |k| k.cmp(b"bar".as_slice())).unwrap();
		assert_eq!(hinted.value, plain);
		assert_eq!(hinted.hint, Some(0));
	}

	#[test]
	fn hinted_lookup_on_legacy_map_errors() {
		let payload = [0u8, 2, 0xAA, 0xBB];
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		assert!(w.find_value_bytes_with_hint(None, |_| Ordering::Equal).is_err());
	}

	#[test]
	fn hinted_lookup_on_unvalidated_map_ignores_corrupt_key_at_hinted_slot() {
		let mut payload = three_entry_map();
		// Corrupt "baz" (slot 1)'s key offset. Hinting slot 1 while
		// searching for "foo" must not error just because the hinted
		// slot's key range happens to be unreadable — it should fall
		// through to the (unaffected) binary search for "foo", which
		// never needs to touch slot 1 in a 3-entry search (mid = 1 is
		// probed first and IS the corrupted slot, so this also confirms
		// the corrupted-but-touched-by-search case still errors like
		// `find_value_bytes` does).
		corrupt_key_off(&mut payload, 1, u32::MAX);
		let w: IndexedMapWalker<(), ()> =
			IndexedMapWalker::from_payload_unvalidated(&payload).unwrap();
		assert!(matches!(
			w.find_value_bytes_with_hint(Some(1), |k| k.cmp(b"foo".as_slice())),
			Err(Error::OptimisedOffsetOutOfRange { .. })
		));
	}

	#[test]
	fn hinted_lookup_on_unvalidated_map_recovers_via_hint_when_search_would_hit_corruption() {
		let mut payload = three_entry_map();
		// Corrupt "baz" (slot 1) — a plain binary search for "foo" probes
		// mid = 1 first and would hit the corruption. Hinting the correct
		// slot (2) for "foo" up front means the hinted probe succeeds and
		// the corrupted slot is never touched.
		corrupt_key_off(&mut payload, 1, u32::MAX);
		let w: IndexedMapWalker<(), ()> =
			IndexedMapWalker::from_payload_unvalidated(&payload).unwrap();
		let out = w.find_value_bytes_with_hint(Some(2), |k| k.cmp(b"foo".as_slice())).unwrap();
		assert_eq!(out.value, Some(b"42".as_slice()));
		assert_eq!(out.hint, Some(2));
	}
}
