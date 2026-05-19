//! `#[revision(specialised)]` per-field attribute forces bulk `Vec<T>`
//! encoding regardless of the crate-wide `specialised-vectors` cargo feature.
//!
//! The test asserts that the bulk-encoded Vec round-trips correctly when
//! explicitly opted in.

use revision::prelude::*;

#[revisioned(revision = 1)]
#[derive(Debug, PartialEq)]
struct Doc {
	#[revision(specialised)]
	values: Vec<u32>,
}

#[test]
fn specialised_attr_round_trips_vec_u32() {
	let v = Doc {
		values: vec![1, 2, 3, 4, 5, 100, 1000, 1_000_000, u32::MAX],
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: Doc = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, v);
}

#[test]
fn specialised_attr_wire_shape_is_length_plus_bulk_le_bytes() {
	let v = Doc {
		values: vec![0x11_22_33_44u32, 0x55_66_77_88, 0x99_AA_BB_CC],
	};
	let bytes = revision::to_vec(&v).unwrap();

	// Outer envelope is u16 revision header.
	let rev_len = {
		let mut buf = Vec::new();
		<u16 as SerializeRevisioned>::serialize_revisioned(&1u16, &mut buf).unwrap();
		buf.len()
	};
	// `Vec<T>` wire shape is `usize len || T-data`. With specialised, the
	// T-data is `len * 4` bytes of little-endian u32s (on LE platforms).
	let len_bytes = {
		let mut buf = Vec::new();
		<usize as SerializeRevisioned>::serialize_revisioned(&3usize, &mut buf).unwrap();
		buf.len()
	};
	let body_offset = rev_len + len_bytes;
	let expected_body = [
		0x44, 0x33, 0x22, 0x11, // 0x11_22_33_44 little-endian
		0x88, 0x77, 0x66, 0x55, // 0x55_66_77_88 little-endian
		0xCC, 0xBB, 0xAA, 0x99, // 0x99_AA_BB_CC little-endian
	];
	if cfg!(target_endian = "little") {
		assert_eq!(&bytes[body_offset..body_offset + 12], &expected_body[..]);
	}
}

#[test]
fn specialised_attr_works_for_multiple_primitive_vecs() {
	#[revisioned(revision = 1)]
	#[derive(Debug, PartialEq)]
	struct ManyVecs {
		#[revision(specialised)]
		a: Vec<u32>,
		#[revision(specialised)]
		b: Vec<u64>,
		#[revision(specialised)]
		c: Vec<i32>,
		#[revision(specialised)]
		d: Vec<f32>,
	}

	let v = ManyVecs {
		a: vec![1, 2, 3],
		b: vec![100, 200, 300],
		c: vec![-1, -2, -3],
		d: vec![1.0, 2.5, 3.14],
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: ManyVecs = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, v);
}
