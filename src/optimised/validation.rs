//! Eager prologue validators for indexed compounds.
//!
//! Each indexed walker calls into one of these on construction. The cost is
//! linear in entry count and paid once per opened compound, in exchange for
//! corruption detection at the earliest possible point and no per-probe
//! bounds checks downstream.

use crate::Error;

/// Validate an indexed-struct prologue: offsets must be strictly monotonic and
/// the last offset must lie within `payload_len`.
#[doc(hidden)]
pub fn validate_struct_prologue(offsets: &[u32], payload_len: u32) -> Result<(), Error> {
	let mut last: Option<u32> = None;
	for &o in offsets {
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
#[doc(hidden)]
pub fn validate_map_prologue(
	key_offsets: &[u32],
	val_offsets: &[u32],
	keys_region_len: u32,
	vals_region_len: u32,
) -> Result<(), Error> {
	if key_offsets.len() != val_offsets.len() {
		return Err(Error::OptimisedOffsetsNonMonotonic);
	}
	validate_struct_prologue(key_offsets, keys_region_len)?;
	validate_struct_prologue(val_offsets, vals_region_len)?;
	Ok(())
}

/// Validate that the dense keys region is strictly ascending by byte compare.
#[doc(hidden)]
pub fn validate_key_region_ascending(keys_region: &[u8], key_offsets: &[u32]) -> Result<(), Error> {
	let len = key_offsets.len();
	for i in 1..len {
		let prev_start = key_offsets[i - 1] as usize;
		let curr_start = key_offsets[i] as usize;
		// `validate_map_prologue` already enforced monotonicity, so curr_start > prev_start.
		// Last entry runs to keys_region.len(); intermediate to next offset.
		let prev_end = curr_start;
		let curr_end = if i + 1 < len {
			key_offsets[i + 1] as usize
		} else {
			keys_region.len()
		};
		let prev = &keys_region[prev_start..prev_end];
		let curr = &keys_region[curr_start..curr_end];
		if curr <= prev {
			return Err(Error::OptimisedKeyRegionNotAscending);
		}
	}
	Ok(())
}

/// Validate an indexed-seq prologue: same shape as a struct prologue.
#[doc(hidden)]
#[inline]
pub fn validate_seq_prologue(elem_offsets: &[u32], payload_len: u32) -> Result<(), Error> {
	validate_struct_prologue(elem_offsets, payload_len)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn struct_prologue_accepts_monotonic_in_range() {
		assert!(validate_struct_prologue(&[0, 4, 12, 20], 24).is_ok());
	}

	#[test]
	fn struct_prologue_rejects_out_of_range() {
		let err = validate_struct_prologue(&[0, 4, 100], 50).unwrap_err();
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
		assert!(matches!(
			validate_struct_prologue(&[0, 4, 4, 8], 16).unwrap_err(),
			Error::OptimisedOffsetsNonMonotonic
		));
		assert!(matches!(
			validate_struct_prologue(&[8, 4], 16).unwrap_err(),
			Error::OptimisedOffsetsNonMonotonic
		));
	}

	#[test]
	fn map_prologue_rejects_mismatched_lengths() {
		assert!(matches!(
			validate_map_prologue(&[0, 4], &[0, 4, 8], 8, 12).unwrap_err(),
			Error::OptimisedOffsetsNonMonotonic
		));
	}

	#[test]
	fn key_region_ascending_accepts_sorted() {
		// keys: "a", "b", "c" packed contiguously
		let keys = b"abc";
		let offsets = [0u32, 1, 2];
		assert!(validate_key_region_ascending(keys, &offsets).is_ok());
	}

	#[test]
	fn key_region_ascending_rejects_unsorted() {
		// keys: "b", "a"
		let keys = b"ba";
		let offsets = [0u32, 1];
		assert!(matches!(
			validate_key_region_ascending(keys, &offsets).unwrap_err(),
			Error::OptimisedKeyRegionNotAscending
		));
	}

	#[test]
	fn key_region_ascending_rejects_duplicates() {
		let keys = b"aa";
		let offsets = [0u32, 1];
		assert!(matches!(
			validate_key_region_ascending(keys, &offsets).unwrap_err(),
			Error::OptimisedKeyRegionNotAscending
		));
	}

	#[test]
	fn key_region_ascending_handles_empty() {
		assert!(validate_key_region_ascending(&[], &[]).is_ok());
	}

	#[test]
	fn key_region_ascending_handles_single() {
		assert!(validate_key_region_ascending(b"x", &[0]).is_ok());
	}
}
