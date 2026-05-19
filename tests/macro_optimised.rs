//! End-to-end tests for `optimised` codegen on structs.
//!
//! These tests prove the macro emits a runtime-functional optimised encoding:
//! `u16 revision || u32_le payload_length || [optional prologue] || fields`.

use revision::prelude::*;

/// Bytes the outer `u16` revision header occupies. Under the default varint
/// encoding rev 1 packs to 1 byte; under `fixed-width-encoding` every u16
/// is 2 bytes. Compute it at runtime so byte-count assertions work under
/// either feature flag.
fn rev_header_size() -> usize {
	let mut buf = Vec::new();
	1u16.serialize_revisioned(&mut buf).unwrap();
	buf.len()
}

#[revisioned(revision(1, optimised))]
#[derive(Debug, Clone, PartialEq)]
struct SimpleOptimised {
	a: u32,
	b: u32,
}

#[revisioned(revision(1, optimised, indexed_struct))]
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
	let rh = rev_header_size();
	// Skip past the revision header (1 byte varint, 2 bytes fixed-width).
	// Next 4 bytes: u32_le payload length.
	let payload_len = u32::from_le_bytes(bytes[rh..rh + 4].try_into().unwrap()) as usize;
	assert_eq!(bytes.len(), rh + 4 + payload_len, "total bytes = revision + length + payload");
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
	// revision header | u32_le length | [3 * u32_le offset table] | fields
	let rh = rev_header_size();
	let payload_len = u32::from_le_bytes(bytes[rh..rh + 4].try_into().unwrap()) as usize;
	let payload = &bytes[rh + 4..rh + 4 + payload_len];
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

// `[u8; N]` serialises as exactly N raw bytes under SerializeRevisioned,
// which makes it a clean fit for `fixed(N)`. Primitive integer types use
// varint encoding so they don't have a statically-known byte length and
// shouldn't be used inside a fixed-size variant directly.
#[revisioned(revision(1, optimised))]
#[derive(Debug, Clone, PartialEq)]
enum OptimisedEnum {
	#[revision(size = "inline")]
	Unit,
	#[revision(size = "fixed(8)")]
	WithBytes([u8; 8]),
	#[revision(size = "varlen")]
	WithString(String),
	#[revision(size = "varlen")]
	WithPair {
		a: u32,
		b: u32,
	},
}

#[test]
fn optimised_enum_round_trips_all_size_classes() {
	let cases = vec![
		OptimisedEnum::Unit,
		OptimisedEnum::WithBytes([1, 2, 3, 4, 5, 6, 7, 8]),
		OptimisedEnum::WithString("hello, optimised world".to_string()),
		OptimisedEnum::WithPair {
			a: 1,
			b: 2,
		},
	];
	for original in cases {
		let bytes = revision::to_vec(&original).unwrap();
		let decoded: OptimisedEnum = revision::from_slice(&bytes).unwrap();
		assert_eq!(original, decoded);
	}
}

#[test]
fn optimised_enum_inline_variant_is_just_header_plus_tag() {
	// revision header + u8 tag.
	let bytes = revision::to_vec(&OptimisedEnum::Unit).unwrap();
	assert_eq!(
		bytes.len(),
		rev_header_size() + 1,
		"Inline variant should be revision header + tag only"
	);
}

#[test]
fn optimised_enum_fixed_variant_has_no_length_prefix() {
	let bytes = revision::to_vec(&OptimisedEnum::WithBytes([0xAA; 8])).unwrap();
	// revision header + tag (1) + 8-byte payload, no u32_le length.
	assert_eq!(
		bytes.len(),
		rev_header_size() + 1 + 8,
		"Fixed variant should not carry a length prefix"
	);
}

#[test]
fn optimised_enum_varlen_variant_has_u32_le_length() {
	let s = "x".repeat(100);
	let bytes = revision::to_vec(&OptimisedEnum::WithString(s.clone())).unwrap();
	// revision header + tag (1) + u32_le length (4) + body.
	let rh = rev_header_size();
	let body_len = u32::from_le_bytes(bytes[rh + 1..rh + 1 + 4].try_into().unwrap()) as usize;
	assert_eq!(bytes.len(), rh + 1 + 4 + body_len);
}

#[test]
fn optimised_enum_skip_advances_past_record() {
	// Encode an enum, then a sentinel; skip the enum and verify sentinel reads back.
	let v = OptimisedEnum::WithString("payload".to_string());
	let mut bytes = revision::to_vec(&v).unwrap();
	let sentinel: u32 = 0xDEADBEEF;
	bytes.extend_from_slice(&revision::to_vec(&sentinel).unwrap());
	let mut cursor: &[u8] = &bytes;
	<OptimisedEnum as SkipRevisioned>::skip_revisioned(&mut cursor).unwrap();
	let remaining: u32 = revision::from_slice(cursor).unwrap();
	assert_eq!(remaining, sentinel);
}
