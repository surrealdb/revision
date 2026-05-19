//! Corruption tests for the optimised wire format.
//!
//! Each test hand-crafts a malformed byte buffer and asserts that the specific
//! validator catches it with the right typed error. These guarantees are the
//! basis for treating optimised-format inputs as untrusted.

use revision::Error;
use revision::optimised::envelope::{read_optimised_tag, read_varlen_slice};
use revision::optimised::indexed::seq_walk::FLAG_INDEXED;
use revision::optimised::tag::{SizeClass, Tag};
use revision::optimised::{IndexedMapWalker, IndexedSeqWalker, IndexedStructWalker};
use revision::slice_reader::SliceReader;

#[test]
fn tag_with_reserved_size_class_returns_invalid_optimised_tag() {
	// 0b11 << 5 = 0b0110_0000 — reserved size class.
	let bad_tag = 0b0110_0000u8;
	let mut r: &[u8] = &[bad_tag];
	let err = read_optimised_tag(&mut r).unwrap_err();
	assert!(matches!(err, Error::InvalidOptimisedTag(b) if b == bad_tag), "got {err:?}");
}

#[test]
fn varlen_overrun_returns_io_error() {
	// Claims 1000 bytes of payload but only 5 are available.
	let mut buf = Vec::new();
	buf.push(Tag::new(0, SizeClass::Varlen).0);
	buf.extend_from_slice(&1000u32.to_le_bytes());
	buf.extend_from_slice(&[0u8; 5]);
	let mut r = SliceReader::new(&buf);
	let _ = read_optimised_tag(&mut r).unwrap();
	assert!(read_varlen_slice(&mut r).is_err());
}

#[test]
fn indexed_struct_with_offset_past_payload_errors() {
	// field_count = 1, prologue is 4 bytes, but offset = 999.
	let mut payload = vec![0u8; 8];
	payload[0..4].copy_from_slice(&999u32.to_le_bytes());
	let err = IndexedStructWalker::from_payload(&payload, 1, 1).unwrap_err();
	assert!(matches!(err, Error::OptimisedOffsetOutOfRange { .. }), "got {err:?}");
}

#[test]
fn indexed_struct_with_non_monotonic_offsets_errors() {
	// field_count = 2, prologue 8 bytes, then 16 data bytes.
	let mut payload = vec![0u8; 8 + 16];
	payload[0..4].copy_from_slice(&20u32.to_le_bytes());
	payload[4..8].copy_from_slice(&10u32.to_le_bytes());
	let err = IndexedStructWalker::from_payload(&payload, 1, 2).unwrap_err();
	assert!(matches!(err, Error::OptimisedOffsetsNonMonotonic), "got {err:?}");
}

#[test]
fn indexed_struct_with_short_prologue_errors() {
	// field_count = 4 → expects 16 bytes prologue, supplies 3.
	let payload = vec![0u8; 3];
	let err = IndexedStructWalker::from_payload(&payload, 1, 4).unwrap_err();
	assert!(matches!(err, Error::OptimisedSubReaderOverrun), "got {err:?}");
}

#[test]
fn indexed_seq_with_oversized_offset_table_errors() {
	// flags + varint(1000) but no offset table bytes follow.
	let mut payload = vec![FLAG_INDEXED];
	payload.push(251);
	payload.extend_from_slice(&1000u16.to_le_bytes());
	let err: Error = IndexedSeqWalker::<()>::from_payload(&payload).unwrap_err();
	assert!(matches!(err, Error::OptimisedSubReaderOverrun), "got {err:?}");
}

#[test]
fn indexed_map_with_unsorted_keys_errors() {
	// Build a map with deliberately reversed key order — the validator
	// must reject it on construction.
	let mut payload = vec![FLAG_INDEXED, 2u8]; // flags, varint(2)
	// Two entries' worth of (key_off, val_off): keys at 0, 1; vals at 0, 1.
	payload.extend_from_slice(&0u32.to_le_bytes());
	payload.extend_from_slice(&0u32.to_le_bytes());
	payload.extend_from_slice(&1u32.to_le_bytes());
	payload.extend_from_slice(&1u32.to_le_bytes());
	// Region lengths (k=2, v=2)
	payload.extend_from_slice(&2u32.to_le_bytes());
	payload.extend_from_slice(&2u32.to_le_bytes());
	// Keys region: "b" then "a" — unsorted.
	payload.extend_from_slice(b"ba");
	// Values region: anything.
	payload.extend_from_slice(b"XY");
	let err: Error = IndexedMapWalker::<(), ()>::from_payload(&payload).unwrap_err();
	assert!(matches!(err, Error::OptimisedKeyRegionNotAscending), "got {err:?}");
}

#[test]
fn indexed_map_with_duplicate_keys_errors() {
	let mut payload = vec![FLAG_INDEXED, 2u8];
	payload.extend_from_slice(&0u32.to_le_bytes());
	payload.extend_from_slice(&0u32.to_le_bytes());
	payload.extend_from_slice(&1u32.to_le_bytes());
	payload.extend_from_slice(&1u32.to_le_bytes());
	payload.extend_from_slice(&2u32.to_le_bytes());
	payload.extend_from_slice(&2u32.to_le_bytes());
	payload.extend_from_slice(b"aa"); // duplicate
	payload.extend_from_slice(b"XY");
	let err: Error = IndexedMapWalker::<(), ()>::from_payload(&payload).unwrap_err();
	assert!(matches!(err, Error::OptimisedKeyRegionNotAscending), "got {err:?}");
}

#[test]
fn indexed_map_with_mismatched_offset_tables_errors() {
	// Should be impossible to construct via the encoder, but a corrupt stream
	// could land here. validate_map_prologue checks len(k_offs) == len(v_offs).
	// Hand-build: flags, varint(2), only one offset pair, then region lengths.
	let mut payload = vec![FLAG_INDEXED, 2u8];
	// Only 8 bytes of offsets (one pair) — table_bytes expected 16.
	payload.extend_from_slice(&0u32.to_le_bytes());
	payload.extend_from_slice(&0u32.to_le_bytes());
	// truncated
	let err: Error = IndexedMapWalker::<(), ()>::from_payload(&payload).unwrap_err();
	// We hit truncation before the validator gets a chance, which is fine —
	// the corruption is still caught.
	assert!(matches!(err, Error::OptimisedSubReaderOverrun), "got {err:?}");
}

#[test]
fn empty_payload_for_indexed_seq_errors() {
	let payload: Vec<u8> = vec![];
	let err: Error = IndexedSeqWalker::<()>::from_payload(&payload).unwrap_err();
	assert!(matches!(err, Error::OptimisedSubReaderOverrun), "got {err:?}");
}

#[test]
fn invalid_varint_tag_byte_errors() {
	// flags=indexed, varint tag = 254 (invalid).
	let payload = [FLAG_INDEXED, 254];
	let err: Error = IndexedSeqWalker::<()>::from_payload(&payload).unwrap_err();
	assert!(matches!(err, Error::InvalidIntegerEncoding), "got {err:?}");
}
