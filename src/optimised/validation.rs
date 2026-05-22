//! Eager prologue validators for indexed compounds.
//!
//! Each indexed walker calls into one of these on construction. The cost is
//! linear in entry count and paid once per opened compound, in exchange for
//! corruption detection at the earliest possible point and no per-probe
//! bounds checks downstream.

use crate::Error;

/// Decode a `u32_le` from `bytes` at `byte_offset`. The single primitive shared
/// by every indexed-walker / validator site that reads borrowed offset tables.
///
/// Caller is responsible for ensuring `byte_offset + 4 <= bytes.len()`; an
/// out-of-range index panics via standard slice bounds checking. All current
/// call sites either bounds-check the parent slice at construction
/// (`payload.len() < cursor + table_bytes`) or run after `validate_*_prologue`
/// has confirmed the table length, so the read is always in range.
#[inline]
pub(crate) fn decode_u32_le_at(bytes: &[u8], byte_offset: usize) -> u32 {
	u32::from_le_bytes([
		bytes[byte_offset],
		bytes[byte_offset + 1],
		bytes[byte_offset + 2],
		bytes[byte_offset + 3],
	])
}

/// Validate an indexed-struct prologue: offsets must be strictly monotonic and
/// the last offset must lie within `payload_len`.
///
/// `offset_bytes` is the raw `u32_le` offset table from the on-wire payload.
/// `count` is the number of entries and `stride` is the byte distance between
/// successive entries (4 for a contiguous offset table; 8 for an interleaved
/// `(key_off, val_off)` map table). Decoding entries on demand instead of
/// materialising a `Vec<u32>` avoids one allocation + copy per validation.
#[doc(hidden)]
pub fn validate_struct_prologue(
	offset_bytes: &[u8],
	count: usize,
	stride: usize,
	payload_len: u32,
) -> Result<(), Error> {
	let mut last: Option<u32> = None;
	for i in 0..count {
		let o = decode_u32_le_at(offset_bytes, i * stride);
		if o > payload_len {
			return Err(Error::OptimisedOffsetOutOfRange {
				offset: o,
				payload_len,
			});
		}
		if let Some(prev) = last
			&& o <= prev
		{
			return Err(Error::OptimisedOffsetsNonMonotonic);
		}
		last = Some(o);
	}
	Ok(())
}

/// Validate the parallel key/value offset tables in an indexed-map prologue.
///
/// `offset_table` holds the interleaved `(u32_le key_off, u32_le val_off)`
/// pairs (`count * 8` bytes); the key column lives at strides
/// `0, 8, 16, …` and the value column at `4, 12, 20, …`.
#[doc(hidden)]
pub fn validate_map_prologue(
	offset_table: &[u8],
	count: usize,
	keys_region_len: u32,
	vals_region_len: u32,
) -> Result<(), Error> {
	// Defensive smoke-test, not a wire-format guard: the only caller derives
	// both `offset_table` (sliced as `len * 8` bytes) and `count` from the same
	// payload length, so a real mismatch is unreachable. Kept so that any
	// future caller passing inconsistent arguments fails loudly here instead
	// of producing a slice-OOB panic deeper in the validator.
	if offset_table.len() != count.saturating_mul(8) {
		return Err(Error::OptimisedOffsetsNonMonotonic);
	}
	validate_struct_prologue(offset_table, count, 8, keys_region_len)?;
	validate_struct_prologue(&offset_table[4..], count, 8, vals_region_len)?;
	Ok(())
}

/// Validate that the dense keys region is strictly ascending by byte compare.
///
/// `offset_table` is the same interleaved byte slice passed to
/// [`validate_map_prologue`]; this routine reads only the key column
/// (stride 8) and slices each adjacent pair of keys from `keys_region` for
/// comparison.
#[doc(hidden)]
pub fn validate_key_region_ascending(
	keys_region: &[u8],
	offset_table: &[u8],
	count: usize,
) -> Result<(), Error> {
	if count < 2 {
		return Ok(());
	}
	// Carry `(prev_start, curr_start)` across iterations so each key offset is
	// decoded exactly once instead of three times. `validate_map_prologue`
	// already enforced monotonicity, so `curr_start > prev_start`. The last
	// entry runs to `keys_region.len()`; intermediate entries to the next
	// offset.
	let mut prev_start = decode_u32_le_at(offset_table, 0) as usize;
	let mut curr_start = decode_u32_le_at(offset_table, 8) as usize;
	for i in 1..count {
		let curr_end = if i + 1 < count {
			decode_u32_le_at(offset_table, (i + 1) * 8) as usize
		} else {
			keys_region.len()
		};
		let prev = &keys_region[prev_start..curr_start];
		let curr = &keys_region[curr_start..curr_end];
		if curr <= prev {
			return Err(Error::OptimisedKeyRegionNotAscending);
		}
		prev_start = curr_start;
		curr_start = curr_end;
	}
	Ok(())
}

/// Validate an indexed-seq prologue.
///
/// `elem_offset_bytes` is the contiguous `count * 4` byte offset table from
/// the on-wire payload (stride = 4).
#[doc(hidden)]
#[inline]
pub fn validate_seq_prologue(
	elem_offset_bytes: &[u8],
	count: usize,
	payload_len: u32,
) -> Result<(), Error> {
	validate_struct_prologue(elem_offset_bytes, count, 4, payload_len)
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Pack a `&[u32]` into a contiguous `Vec<u8>` of `u32_le` entries
	/// (stride = 4) for the validators' on-wire shape.
	fn pack_offsets(offsets: &[u32]) -> Vec<u8> {
		let mut out = Vec::with_capacity(offsets.len() * 4);
		for &o in offsets {
			out.extend_from_slice(&o.to_le_bytes());
		}
		out
	}

	/// Pack parallel `(key_off, val_off)` columns into the interleaved
	/// stride-8 layout the map prologue expects.
	fn pack_interleaved(key_offsets: &[u32], val_offsets: &[u32]) -> Vec<u8> {
		assert_eq!(key_offsets.len(), val_offsets.len());
		let mut out = Vec::with_capacity(key_offsets.len() * 8);
		for (k, v) in key_offsets.iter().zip(val_offsets.iter()) {
			out.extend_from_slice(&k.to_le_bytes());
			out.extend_from_slice(&v.to_le_bytes());
		}
		out
	}

	#[test]
	fn struct_prologue_accepts_monotonic_in_range() {
		let bytes = pack_offsets(&[0, 4, 12, 20]);
		assert!(validate_struct_prologue(&bytes, 4, 4, 24).is_ok());
	}

	#[test]
	fn struct_prologue_rejects_out_of_range() {
		let bytes = pack_offsets(&[0, 4, 100]);
		let err = validate_struct_prologue(&bytes, 3, 4, 50).unwrap_err();
		assert!(matches!(
			err,
			Error::OptimisedOffsetOutOfRange {
				offset: 100,
				payload_len: 50
			}
		));
	}

	#[test]
	fn struct_prologue_rejects_non_monotonic() {
		let dup = pack_offsets(&[0, 4, 4, 8]);
		assert!(matches!(
			validate_struct_prologue(&dup, 4, 4, 16).unwrap_err(),
			Error::OptimisedOffsetsNonMonotonic
		));
		let desc = pack_offsets(&[8, 4]);
		assert!(matches!(
			validate_struct_prologue(&desc, 2, 4, 16).unwrap_err(),
			Error::OptimisedOffsetsNonMonotonic
		));
	}

	#[test]
	fn map_prologue_rejects_mismatched_lengths() {
		// Defensive smoke-test for the `offset_table.len() != count * 8` guard.
		// Unreachable from real callers (both arguments come from the same
		// payload length) but kept so a future caller passing inconsistent
		// arguments fails with a typed error instead of a slice-OOB panic.
		// Simulated by packing 2 pairs (16 bytes) but claiming count=3.
		let bytes = pack_interleaved(&[0, 4], &[0, 4]);
		assert!(matches!(
			validate_map_prologue(&bytes, 3, 8, 12).unwrap_err(),
			Error::OptimisedOffsetsNonMonotonic
		));
	}

	#[test]
	fn key_region_ascending_accepts_sorted() {
		// keys: "a", "b", "c" packed contiguously
		let keys = b"abc";
		let table = pack_interleaved(&[0, 1, 2], &[0, 0, 0]);
		assert!(validate_key_region_ascending(keys, &table, 3).is_ok());
	}

	#[test]
	fn key_region_ascending_rejects_unsorted() {
		// keys: "b", "a"
		let keys = b"ba";
		let table = pack_interleaved(&[0, 1], &[0, 0]);
		assert!(matches!(
			validate_key_region_ascending(keys, &table, 2).unwrap_err(),
			Error::OptimisedKeyRegionNotAscending
		));
	}

	#[test]
	fn key_region_ascending_rejects_duplicates() {
		let keys = b"aa";
		let table = pack_interleaved(&[0, 1], &[0, 0]);
		assert!(matches!(
			validate_key_region_ascending(keys, &table, 2).unwrap_err(),
			Error::OptimisedKeyRegionNotAscending
		));
	}

	#[test]
	fn key_region_ascending_handles_empty() {
		assert!(validate_key_region_ascending(&[], &[], 0).is_ok());
	}

	#[test]
	fn key_region_ascending_handles_single() {
		let table = pack_interleaved(&[0], &[0]);
		assert!(validate_key_region_ascending(b"x", &table, 1).is_ok());
	}
}
