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

	assert_eq!(
		d1,
		ThreeRevisions {
			a: 7,
			b: 8,
		}
	);
	assert_eq!(
		d2,
		ThreeRevisions {
			a: 9,
			b: 10,
		}
	);
	assert_eq!(
		d3,
		ThreeRevisions {
			a: 11,
			b: 12,
		}
	);

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
//
// The pins below capture bytes under the **default varint** integer
// encoding. Under `--features fixed-width-encoding` the same logical
// values serialise to different byte counts (every u16/u32 becomes
// fixed-width LE), so these pins are intentionally varint-only. A
// parallel set of pins for `fixed-width-encoding` would belong here
// gated the other way if downstream users adopt that feature.

#[cfg(not(feature = "fixed-width-encoding"))]
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

#[cfg(not(feature = "fixed-width-encoding"))]
#[test]
fn pin_optimised_rev1_struct_layout() {
	// Bytes captured from `revision::to_vec(&OptimisedFromDayOne { a: 7, b: 11 })`.
	// Layout: revision (1) | u32_le payload_length | varint(a=7) | varint(b=11)
	// Payload is 2 bytes (one varint each), so length = 2.
	let pinned: &[u8] = &[
		1, // u16 revision varint = 1
		2, 0, 0, 0,  // u32_le payload length = 2
		7,  // varint a
		11, // varint b
	];
	let decoded: OptimisedFromDayOne = revision::from_slice(pinned).unwrap();
	assert_eq!(decoded.a, 7);
	assert_eq!(decoded.b, 11);
}

#[cfg(not(feature = "fixed-width-encoding"))]
#[revisioned(revision(1, encoding = "optimised", indexed_struct))]
#[derive(Debug, PartialEq)]
struct PinIndexedStruct {
	a: u32,
	b: u32,
	c: u32,
}

#[cfg(not(feature = "fixed-width-encoding"))]
#[test]
fn pin_optimised_indexed_struct_layout() {
	// Bytes captured from `revision::to_vec(&PinIndexedStruct { a: 7, b: 11, c: 19 })`.
	// Layout:
	//   u16 revision varint            (1 byte: rev=1)
	//   u32_le payload length          (4 bytes: prologue 12 + body 3 = 15)
	//   u32_le offset table * 3        (12 bytes: 12, 13, 14)
	//   field bytes                    (3 bytes: 7, 11, 19 as varints)
	let pinned: &[u8] = &[
		1, // u16 revision varint = 1
		15, 0, 0, 0, // u32_le payload length = 15 (3*4 offsets + 3 field bytes)
		12, 0, 0, 0, // offsets[0] = 12 (start of field a, just past prologue)
		13, 0, 0, 0, // offsets[1] = 13 (start of field b)
		14, 0, 0, 0,  // offsets[2] = 14 (start of field c)
		7,  // field a = 7 (varint)
		11, // field b = 11 (varint)
		19, // field c = 19 (varint)
	];
	let decoded: PinIndexedStruct = revision::from_slice(pinned).unwrap();
	assert_eq!(decoded.a, 7);
	assert_eq!(decoded.b, 11);
	assert_eq!(decoded.c, 19);
}

#[cfg(not(feature = "fixed-width-encoding"))]
#[revisioned(revision(1, encoding = "optimised"))]
#[derive(Debug, PartialEq)]
enum PinOptimisedEnum {
	#[revision(size = "varlen")]
	Greeting(String),
}

#[cfg(not(feature = "fixed-width-encoding"))]
#[test]
fn pin_optimised_varlen_enum_variant_layout() {
	// Bytes captured from `revision::to_vec(&PinOptimisedEnum::Greeting("hi".into()))`.
	// Layout:
	//   u16 revision varint            (1 byte: rev=1)
	//   u8 tag                         (1 byte: variant_id=0 + size_class=Varlen)
	//   u32_le body length             (4 bytes: 3 = varint(len=2) + "hi")
	//   body                           (varint(2) + 'h' + 'i')
	//
	// Tag layout: variant_id in low 5 bits, size_class in bits 5..=6.
	// Greeting is variant id 0; Varlen = 0b10. So tag = (0b10 << 5) | 0 = 0x40 = 64.
	let pinned: &[u8] = &[
		1,  // u16 revision varint = 1
		64, // optimised tag: variant_id=0, size_class=Varlen
		3, 0, 0, 0, // u32_le body length = 3
		2, // varint string length = 2
		b'h', b'i', // string bytes
	];
	let decoded: PinOptimisedEnum = revision::from_slice(pinned).unwrap();
	assert_eq!(decoded, PinOptimisedEnum::Greeting("hi".into()));
}

// -----------------------------------------------------------------------------
// Mixed-history nested-type tests
// -----------------------------------------------------------------------------
//
// Each nested type dispatches on its own history independently of the outer
// type's history. An outer type with a single legacy revision can transparently
// hold an inner type whose latest revision is optimised, and vice versa.

#[revisioned(revision(1))]
#[derive(Debug, Clone, PartialEq)]
struct InnerLegacy {
	x: u32,
	y: u32,
}

#[revisioned(revision(1), revision(2, encoding = "optimised"))]
#[derive(Debug, Clone, PartialEq)]
struct InnerOptimised {
	x: u32,
	y: u32,
}

#[revisioned(revision(1))]
#[derive(Debug, Clone, PartialEq)]
struct OuterLegacy {
	tag: u32,
	inner: InnerOptimised,
}

#[revisioned(revision(1), revision(2, encoding = "optimised"))]
#[derive(Debug, Clone, PartialEq)]
struct OuterOptimised {
	tag: u32,
	inner: InnerLegacy,
}

#[test]
fn legacy_outer_with_optimised_inner_round_trips() {
	let v = OuterLegacy {
		tag: 0xAA,
		inner: InnerOptimised {
			x: 1,
			y: 2,
		},
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: OuterLegacy = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, v);
	// Outer carries rev 1 (legacy single revision).
	assert_eq!(bytes[0], 1u8);
}

#[test]
fn optimised_outer_with_legacy_inner_round_trips() {
	let v = OuterOptimised {
		tag: 0xBB,
		inner: InnerLegacy {
			x: 3,
			y: 4,
		},
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: OuterOptimised = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, v);
	// Outer carries rev 2 (latest optimised revision).
	assert_eq!(bytes[0], 2u8);
}

// -----------------------------------------------------------------------------
// Variant lifecycle crossing the encoding boundary
// -----------------------------------------------------------------------------

#[revisioned(revision(1), revision(2, encoding = "optimised"))]
#[derive(Debug, Clone, PartialEq)]
enum VariantLifecycle {
	#[revision(end = 2, convert_fn = "migrate_old", size = "fixed(8)")]
	Old([u8; 8]),
	#[revision(start = 1, size = "fixed(8)")]
	New([u8; 8]),
}

impl VariantLifecycle {
	fn migrate_old(
		fields: VariantLifecycleOldFields,
		_revision: u16,
	) -> Result<Self, revision::Error> {
		// Treat the wire `Old(bytes)` as if it were `New(bytes)` — same payload
		// shape, just renamed.
		Ok(VariantLifecycle::New(fields.0))
	}
}

#[test]
fn variant_removed_across_optimised_boundary_routes_through_convert_fn() {
	// Encode a rev-1 (legacy) `Old` variant using a shadow type that still has it.
	#[revisioned(revision(1))]
	#[derive(Debug)]
	enum Shadow {
		Old([u8; 8]),
		#[allow(dead_code)]
		New([u8; 8]),
	}

	let bytes = revision::to_vec(&Shadow::Old([1, 2, 3, 4, 5, 6, 7, 8])).unwrap();
	let decoded: VariantLifecycle = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, VariantLifecycle::New([1, 2, 3, 4, 5, 6, 7, 8]));
}

#[test]
fn nested_optimised_both_levels_round_trips() {
	#[revisioned(revision(1, encoding = "optimised"))]
	#[derive(Debug, Clone, PartialEq)]
	struct Inner2 {
		a: u32,
	}

	#[revisioned(revision(1, encoding = "optimised"))]
	#[derive(Debug, Clone, PartialEq)]
	struct Outer2 {
		tag: u32,
		inner: Inner2,
	}

	let v = Outer2 {
		tag: 7,
		inner: Inner2 {
			a: 42,
		},
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: Outer2 = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, v);
}
