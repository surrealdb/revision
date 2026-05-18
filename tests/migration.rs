//! Cross-revision migration tests.
//!
//! These tests cover the upgrade paths between legacy and optimised wire
//! formats. The invariant from the design: **decoders for every history entry
//! must produce the same in-memory shape** — wire diversity, in-memory
//! convergence.

use revision::prelude::*;

// -----------------------------------------------------------------------------
// Fixtures
// -----------------------------------------------------------------------------
//
// Rev 1 (legacy) and Rev 2 (optimised) of the same struct. Both contain the
// same logical fields; we expect the in-memory representation produced by
// decoding rev-1 bytes to match the one produced by decoding rev-2 bytes.

#[revisioned(revision(1))]
#[derive(Debug, Clone, PartialEq)]
struct OnlyRev1 {
	a: u32,
	b: u32,
}

#[revisioned(revision(1), revision(2, encoding = "optimised"))]
#[derive(Debug, Clone, PartialEq)]
struct LegacyAndOptimised {
	a: u32,
	b: u32,
}

#[revisioned(revision(1, encoding = "optimised"))]
#[derive(Debug, Clone, PartialEq)]
struct OptimisedFromDayOne {
	a: u32,
	b: u32,
}

#[revisioned(revision(1), revision(2), revision(3, encoding = "optimised"))]
#[derive(Debug, Clone, PartialEq)]
struct ThreeRevisions {
	a: u32,
	b: u32,
}

// A type that grows a field across an encoding boundary.
#[revisioned(revision(1), revision(2, encoding = "optimised"))]
#[derive(Debug, Clone, PartialEq)]
struct GrowsAcrossBoundary {
	a: u32,
	#[revision(start = 2, default_fn = "default_b")]
	b: u32,
}

impl GrowsAcrossBoundary {
	fn default_b(_revision: u16) -> Result<u32, revision::Error> {
		Ok(42)
	}
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[test]
fn legacy_to_optimised_decoder_produces_same_value() {
	// Encode rev-1 bytes using a "legacy-only" type with the same shape as
	// `LegacyAndOptimised` at rev 1. Decode those bytes through the new type
	// at rev 1; we should get the same logical value back.
	let rev1 = OnlyRev1 {
		a: 100,
		b: 200,
	};
	let rev1_bytes = revision::to_vec(&rev1).unwrap();
	let decoded: LegacyAndOptimised = revision::from_slice(&rev1_bytes).unwrap();
	assert_eq!(decoded.a, 100);
	assert_eq!(decoded.b, 200);
}

#[test]
fn optimised_encode_then_decode_matches_legacy_value() {
	// Encode at rev 2 (optimised) and decode; the in-memory value must equal
	// the same value encoded at rev 1.
	let v = LegacyAndOptimised {
		a: 100,
		b: 200,
	};
	let optimised_bytes = revision::to_vec(&v).unwrap();
	let decoded: LegacyAndOptimised = revision::from_slice(&optimised_bytes).unwrap();
	assert_eq!(decoded, v);

	// And legacy bytes from OnlyRev1 also decode to the same in-memory value.
	let legacy_bytes = revision::to_vec(&OnlyRev1 {
		a: 100,
		b: 200,
	})
	.unwrap();
	let decoded_legacy: LegacyAndOptimised = revision::from_slice(&legacy_bytes).unwrap();
	assert_eq!(decoded_legacy, v);
}

#[test]
fn current_encode_always_uses_latest_revision() {
	// Encoding with the optimised type must produce rev-2 bytes (the latest).
	let v = LegacyAndOptimised {
		a: 1,
		b: 2,
	};
	let bytes = revision::to_vec(&v).unwrap();
	// First byte is the u16 revision varint. `2` packs to 1 byte (varint <= 250).
	assert_eq!(bytes[0], 2u8, "encoded bytes should declare rev 2");
}

#[test]
fn optimised_from_day_one_round_trips() {
	let v = OptimisedFromDayOne {
		a: 999,
		b: 1000,
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: OptimisedFromDayOne = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, v);
	// First byte is rev = 1.
	assert_eq!(bytes[0], 1u8);
}

#[test]
fn three_revision_history_decodes_each_arm() {
	// We can't easily encode at rev 1 or rev 2 with this type (it always
	// encodes at the latest = rev 3). But we can encode at the matching
	// "shadow" types and decode through `ThreeRevisions`.

	#[revisioned(revision(1))]
	#[derive(Debug)]
	struct Shadow1 {
		a: u32,
		b: u32,
	}

	#[revisioned(revision(1), revision(2))]
	#[derive(Debug)]
	struct Shadow2 {
		a: u32,
		b: u32,
	}

	let s1 = Shadow1 {
		a: 7,
		b: 8,
	};
	let s2 = Shadow2 {
		a: 9,
		b: 10,
	};

	let bytes1 = revision::to_vec(&s1).unwrap();
	let bytes2 = revision::to_vec(&s2).unwrap();
	let bytes3 = revision::to_vec(&ThreeRevisions {
		a: 11,
		b: 12,
	})
	.unwrap();

	// All three decode through ThreeRevisions to the right values.
	let d1: ThreeRevisions = revision::from_slice(&bytes1).unwrap();
	let d2: ThreeRevisions = revision::from_slice(&bytes2).unwrap();
	let d3: ThreeRevisions = revision::from_slice(&bytes3).unwrap();

	assert_eq!(d1, ThreeRevisions {
		a: 7,
		b: 8,
	});
	assert_eq!(d2, ThreeRevisions {
		a: 9,
		b: 10,
	});
	assert_eq!(d3, ThreeRevisions {
		a: 11,
		b: 12,
	});

	// Encoding always emits rev 3.
	assert_eq!(bytes3[0], 3u8);
}

#[test]
fn field_lifecycle_crosses_encoding_boundary() {
	// At rev 1 (legacy, no field b), the decoder synthesises b via default_b.
	#[revisioned(revision(1))]
	#[derive(Debug)]
	struct OnlyA {
		a: u32,
	}

	let bytes_rev1 = revision::to_vec(&OnlyA {
		a: 50,
	})
	.unwrap();
	let decoded: GrowsAcrossBoundary = revision::from_slice(&bytes_rev1).unwrap();
	assert_eq!(decoded.a, 50);
	assert_eq!(decoded.b, 42, "default_b should fill in the field absent at rev 1");

	// At rev 2 (optimised, b is present), the field is decoded from wire.
	let v = GrowsAcrossBoundary {
		a: 60,
		b: 70,
	};
	let bytes_rev2 = revision::to_vec(&v).unwrap();
	let decoded: GrowsAcrossBoundary = revision::from_slice(&bytes_rev2).unwrap();
	assert_eq!(decoded, v);
}

// -----------------------------------------------------------------------------
// Byte-pin tests (regression guard against silent wire-format drift)
// -----------------------------------------------------------------------------
//
// Each `pin_*` test captures a known wire byte sequence and asserts that
// today's binary decodes it identically. If a future change shifts the wire
// shape without bumping a revision, these tests fail — the same role
// `revision-lock --check` plays for the downstream surrealdb tree.
//
// NEVER edit a pin to fix a failing test; only ever add new pins for new
// revisions. The whole point of the pin is to make wire drift visible.

#[test]
fn pin_legacy_rev1_decodes_into_legacy_and_optimised_type() {
	// Bytes captured from `revision::to_vec(&OnlyRev1 { a: 1, b: 2 })`.
	let pinned: &[u8] = &[
		1, // u16 revision varint = 1
		1, // u32 varint a = 1
		2, // u32 varint b = 2
	];
	let decoded: LegacyAndOptimised = revision::from_slice(pinned).unwrap();
	assert_eq!(decoded.a, 1);
	assert_eq!(decoded.b, 2);
}

#[test]
fn pin_optimised_rev1_struct_layout() {
	// Bytes captured from `revision::to_vec(&OptimisedFromDayOne { a: 7, b: 11 })`.
	// Layout: revision (1) | u32_le payload_length | varint(a=7) | varint(b=11)
	// Payload is 2 bytes (one varint each), so length = 2.
	let pinned: &[u8] = &[
		1, // u16 revision varint = 1
		2, 0, 0, 0, // u32_le payload length = 2
		7,  // varint a
		11, // varint b
	];
	let decoded: OptimisedFromDayOne = revision::from_slice(pinned).unwrap();
	assert_eq!(decoded.a, 7);
	assert_eq!(decoded.b, 11);
}
