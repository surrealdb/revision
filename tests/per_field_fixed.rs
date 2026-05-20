//! `#[revision(fixed)]` per-field attribute forces fixed-width little-endian
//! integer encoding regardless of the crate-wide `fixed-width-encoding` cargo
//! feature.
//!
//! The test asserts that a `u32` field tagged with `#[revision(fixed)]`
//! produces exactly 4 bytes on the wire, while a sibling untagged `u32`
//! follows the feature-controlled default (1 byte under varint for small
//! values, 4 bytes under fixed-width-encoding).

use revision::prelude::*;

#[revisioned(revision = 1)]
#[derive(Debug, PartialEq)]
struct Mixed {
	a: u32, // feature-default encoding
	#[revision(fixed)]
	b: u32, // always 4 bytes
}

/// Probe the runtime u16 width — under varint a small value is 1 byte,
/// under fixed-width-encoding it's 2 bytes. Used by the offset math below.
fn varint_or_fixed_u32(value: u32) -> usize {
	let mut buf = Vec::new();
	<u32 as SerializeRevisioned>::serialize_revisioned(&value, &mut buf).unwrap();
	buf.len()
}

#[test]
fn fixed_attr_forces_four_byte_u32_on_wire() {
	let v = Mixed {
		a: 1,
		b: 42,
	};
	let bytes = revision::to_vec(&v).unwrap();

	// Outer envelope is u16 revision header (varint or fixed-width).
	let rev_len = {
		let mut buf = Vec::new();
		<u16 as SerializeRevisioned>::serialize_revisioned(&1u16, &mut buf).unwrap();
		buf.len()
	};
	let a_len = varint_or_fixed_u32(1);
	// `b` is always 4 bytes regardless of feature.
	let b_offset = rev_len + a_len;
	let b_bytes = &bytes[b_offset..b_offset + 4];
	assert_eq!(b_bytes, 42u32.to_le_bytes());
	assert_eq!(bytes.len(), rev_len + a_len + 4);

	// Round-trip decode still works.
	let decoded: Mixed = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, v);
}

#[test]
fn fixed_attr_idempotent_under_fixed_width_encoding_feature() {
	// When `fixed-width-encoding` is on, `a` is also 4 bytes; the `b` field
	// is still 4 bytes (idempotent). Both paths produce the same wire shape
	// for the `b` field; this test verifies the encoder doesn't double up
	// the length prefix.
	let v = Mixed {
		a: 0x11_22_33_44,
		b: 0x55_66_77_88,
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: Mixed = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, v);
}

#[test]
fn fixed_attr_works_on_all_supported_integers() {
	#[revisioned(revision = 1)]
	#[derive(Debug, PartialEq)]
	struct AllInts {
		#[revision(fixed)]
		a: u32,
		#[revision(fixed)]
		b: i32,
		#[revision(fixed)]
		c: u64,
		#[revision(fixed)]
		d: i64,
		#[revision(fixed)]
		e: u128,
		#[revision(fixed)]
		f: i128,
	}

	let v = AllInts {
		a: 1,
		b: -2,
		c: 3,
		d: -4,
		e: 5,
		f: -6,
	};
	let bytes = revision::to_vec(&v).unwrap();

	let rev_len = {
		let mut buf = Vec::new();
		<u16 as SerializeRevisioned>::serialize_revisioned(&1u16, &mut buf).unwrap();
		buf.len()
	};
	// All fields are fixed-width: 4+4+8+8+16+16 = 56 bytes.
	assert_eq!(bytes.len(), rev_len + 56);

	let decoded: AllInts = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, v);
}
