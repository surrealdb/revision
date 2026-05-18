//! End-to-end tests for `encoding = "optimised"` codegen on structs.
//!
//! These tests prove the macro emits a runtime-functional optimised encoding:
//! `u16 revision || u32_le payload_length || [optional prologue] || fields`.

use revision::prelude::*;

#[revisioned(revision(1, encoding = "optimised"))]
#[derive(Debug, Clone, PartialEq)]
struct SimpleOptimised {
	a: u32,
	b: u32,
}

#[revisioned(revision(1, encoding = "optimised", struct = "indexed"))]
#[derive(Debug, Clone, PartialEq)]
struct IndexedOptimised {
	a: u32,
	b: u32,
	c: u32,
}

#[test]
fn optimised_struct_round_trips() {
	let original = SimpleOptimised {
		a: 0xDEADBEEF,
		b: 0xCAFEBABE,
	};
	let bytes = revision::to_vec(&original).unwrap();
	let decoded: SimpleOptimised = revision::from_slice(&bytes).unwrap();
	assert_eq!(original, decoded);
}

#[test]
fn optimised_struct_wire_starts_with_revision_then_length() {
	let v = SimpleOptimised {
		a: 1,
		b: 2,
	};
	let bytes = revision::to_vec(&v).unwrap();
	// First byte: revision varint header for `1`.
	assert_eq!(bytes[0], 1u8, "revision varint should be 1");
	// Next 4 bytes: u32_le payload length.
	let payload_len = u32::from_le_bytes(bytes[1..5].try_into().unwrap()) as usize;
	assert_eq!(
		bytes.len(),
		1 + 4 + payload_len,
		"total bytes = revision + length + payload"
	);
}

#[test]
fn indexed_optimised_struct_round_trips() {
	let original = IndexedOptimised {
		a: 10,
		b: 20,
		c: 30,
	};
	let bytes = revision::to_vec(&original).unwrap();
	let decoded: IndexedOptimised = revision::from_slice(&bytes).unwrap();
	assert_eq!(original, decoded);
}

#[test]
fn indexed_optimised_struct_has_offset_table_prologue() {
	let v = IndexedOptimised {
		a: 0,
		b: 0,
		c: 0,
	};
	let bytes = revision::to_vec(&v).unwrap();
	// revision varint(1) | u32_le length | [3 * u32_le offset table] | fields
	let payload_len = u32::from_le_bytes(bytes[1..5].try_into().unwrap()) as usize;
	let payload = &bytes[5..5 + payload_len];
	let off_a = u32::from_le_bytes(payload[0..4].try_into().unwrap());
	let off_b = u32::from_le_bytes(payload[4..8].try_into().unwrap());
	let off_c = u32::from_le_bytes(payload[8..12].try_into().unwrap());
	assert_eq!(off_a, 12, "field a starts immediately after the 12-byte prologue");
	assert!(off_b > off_a);
	assert!(off_c > off_b);
}

#[test]
fn optimised_skip_advances_full_record() {
	// Two consecutive records: SimpleOptimised, then a sentinel u32.
	let v = SimpleOptimised {
		a: 1,
		b: 2,
	};
	let mut bytes = revision::to_vec(&v).unwrap();
	let sentinel: u32 = 0xDEADBEEF;
	bytes.extend_from_slice(&revision::to_vec(&sentinel).unwrap());

	// SkipRevisioned should consume only the first record.
	let mut cursor: &[u8] = &bytes;
	<SimpleOptimised as SkipRevisioned>::skip_revisioned(&mut cursor).unwrap();
	let remaining: u32 = revision::from_slice(cursor).unwrap();
	assert_eq!(remaining, sentinel);
}

#[test]
fn optimised_struct_preserves_field_values_across_a_few_sizes() {
	for (a, b) in [(0u32, 0u32), (1, 1), (u32::MAX, u32::MIN), (12345, 67890)] {
		let v = SimpleOptimised {
			a,
			b,
		};
		let bytes = revision::to_vec(&v).unwrap();
		let decoded: SimpleOptimised = revision::from_slice(&bytes).unwrap();
		assert_eq!(decoded, v);
	}
}
