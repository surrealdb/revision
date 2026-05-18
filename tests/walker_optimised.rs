//! Walker support for optimised-encoded types.
//!
//! The walker's `walk_revisioned` constructor reads the u16 wire revision and
//! advances past the optimised envelope (`u32_le payload_length` + optional
//! indexed prologue) for revisions that opt into `encoding = "optimised"`.
//! Field reads on the resulting Wire walker then succeed as normal.

use revision::prelude::*;

#[revisioned(revision(1, encoding = "optimised"))]
struct OptStruct {
	a: u32,
	b: u32,
}

#[revisioned(revision(1, encoding = "optimised", struct = "indexed"))]
struct IndexedStruct {
	a: u32,
	b: u32,
	c: u32,
}

#[revisioned(revision(1), revision(2, encoding = "optimised"))]
struct MixedHistory {
	a: u32,
	b: u32,
}

#[revisioned(revision(1, encoding = "optimised"))]
enum OptEnum {
	#[revision(size = "inline")]
	Unit,
	#[revision(size = "varlen")]
	Varlen(String),
}

#[test]
fn walker_decodes_optimised_struct() {
	let v = OptStruct {
		a: 42,
		b: 100,
	};
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let mut w = OptStruct::walk_revisioned(&mut r).unwrap();
	let a = w.decode_a().unwrap();
	let b = w.decode_b().unwrap();
	assert_eq!(a, 42);
	assert_eq!(b, 100);
}

#[test]
fn walker_decodes_indexed_optimised_struct() {
	let v = IndexedStruct {
		a: 10,
		b: 20,
		c: 30,
	};
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let mut w = IndexedStruct::walk_revisioned(&mut r).unwrap();
	let a = w.decode_a().unwrap();
	let b = w.decode_b().unwrap();
	let c = w.decode_c().unwrap();
	assert_eq!(a, 10);
	assert_eq!(b, 20);
	assert_eq!(c, 30);
}

#[test]
fn walker_handles_mixed_history_optimised_rev() {
	// Encoded at rev 2 (optimised); walker advances past the envelope.
	let v = MixedHistory {
		a: 7,
		b: 11,
	};
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let mut w = MixedHistory::walk_revisioned(&mut r).unwrap();
	assert_eq!(w.decode_a().unwrap(), 7);
	assert_eq!(w.decode_b().unwrap(), 11);
}

#[test]
fn walker_handles_mixed_history_legacy_rev() {
	// Shadow type at rev 1 (legacy) — walker stays on the Wire path with no
	// envelope skipping required.
	#[revisioned(revision(1))]
	struct ShadowRev1 {
		a: u32,
		b: u32,
	}
	let s = ShadowRev1 {
		a: 7,
		b: 11,
	};
	let bytes = revision::to_vec(&s).unwrap();
	let mut r: &[u8] = &bytes;
	let mut w = MixedHistory::walk_revisioned(&mut r).unwrap();
	assert_eq!(w.decode_a().unwrap(), 7);
	assert_eq!(w.decode_b().unwrap(), 11);
}

#[test]
fn walker_on_optimised_enum_decodes_unit_variant() {
	let v = OptEnum::Unit;
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let w = OptEnum::walk_revisioned(&mut r).unwrap();
	// Unit is the first declared variant — its discriminant is 0 by default.
	assert_eq!(w.discriminant(), 0);
	assert!(w.is_unit());
	assert!(!w.is_varlen());
}

#[test]
fn walker_on_optimised_enum_decodes_varlen_variant() {
	let v = OptEnum::Varlen("hello".into());
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let w = OptEnum::walk_revisioned(&mut r).unwrap();
	// Varlen is the second variant — discriminant 1.
	assert_eq!(w.discriminant(), 1);
	assert!(w.is_varlen());
	assert!(!w.is_unit());
	// Drilling in via `into_<variant>` is not supported on a materialised
	// walker (which is what the optimised enum path produces); the caller
	// is expected to fall through to `DeserializeRevisioned` for the
	// full value. Verify the error message points the user there.
	let err = w
		.into_varlen()
		.err()
		.expect("into_varlen on optimised enum should error pending lifetime work");
	match err {
		revision::Error::Conversion(msg) => {
			assert!(
				msg.contains("DeserializeRevisioned"),
				"error message should redirect to DeserializeRevisioned, got: {msg}"
			);
		}
		other => panic!("expected Conversion error, got {other:?}"),
	}
}
