//! Walker support for optimised-encoded types.
//!
//! The walker's `walk_revisioned` constructor reads the u16 wire revision and
//! advances past the optimised envelope (`u32_le payload_length` + optional
//! indexed prologue) for revisions that opt into `optimised`.
//! Field reads on the resulting Wire walker then succeed as normal.

use revision::prelude::*;

#[revisioned(revision(1, optimised))]
struct OptStruct {
	a: u32,
	b: u32,
}

#[revisioned(revision(1, optimised, indexed_struct))]
struct IndexedStruct {
	a: u32,
	b: u32,
	c: u32,
}

#[revisioned(revision(1), revision(2, optimised))]
struct MixedHistory {
	a: u32,
	b: u32,
}

#[revisioned(revision(1, optimised))]
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
	// `decode_<variant>` works on every walker repr (Wire,
	// OptimisedBorrowed, ConvertedOwned), so it's the right way to
	// extract the inner value from an optimised enum walker.
	let inner = w.decode_varlen().unwrap();
	assert_eq!(inner, "hello");
}

#[test]
fn walker_decode_variant_works_on_legacy_enum() {
	// Sanity: decode_<variant> on a Wire (legacy) walker also works,
	// keeping the API symmetric.
	#[revisioned(revision(1))]
	#[derive(Debug)]
	enum LegacyEnum {
		Unit,
		Tup(u64),
	}

	let v = LegacyEnum::Tup(0xDEADBEEF);
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let w = LegacyEnum::walk_revisioned(&mut r).unwrap();
	let inner = w.decode_tup().unwrap();
	assert_eq!(inner, 0xDEADBEEF);
}

#[test]
fn walker_variant_view_works_on_optimised_enum() {
	// `<variant>_view` returns a VariantView holding the variant body
	// bytes. Works on every walker repr — Wire (legacy enums),
	// OptimisedBorrowed (optimised enums), and ConvertedOwned
	// (cross-rev `convert_fn`).
	let v = OptEnum::Varlen("hello".into());
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let w = OptEnum::walk_revisioned(&mut r).unwrap();
	let view = w.varlen_view().unwrap();
	// The view owns the variant's body bytes — caller can construct their
	// own walker / decoder from them.
	let body = view.as_bytes();
	let mut br: &[u8] = body;
	let inner: String =
		<String as revision::DeserializeRevisioned>::deserialize_revisioned(&mut br).unwrap();
	assert_eq!(inner, "hello");
}

#[test]
fn walker_variant_view_borrows_from_source_for_optimised_enum() {
	// Surrealdb-style descent pattern: walk into an optimised enum, get the
	// variant body as a borrowed slice, then construct an inner walker over
	// the borrowed bytes — zero allocations for the variant body.
	//
	// This is what the `WalkRevisioned: BorrowedReader` bound + Cow<'r, [u8]>
	// in the walker repr enables.
	#[revisioned(revision(1, optimised))]
	#[derive(Debug, PartialEq)]
	enum Value {
		#[revision(size = "inline")]
		Null,
		#[revision(size = "varlen")]
		Array(Vec<u32>),
	}

	let v = Value::Array(vec![1, 2, 3, 4, 5]);
	let bytes = revision::to_vec(&v).unwrap();

	// Outer walk: optimised enum, body bytes are borrowed from `bytes`.
	let mut r: &[u8] = &bytes;
	let w = Value::walk_revisioned(&mut r).unwrap();
	let view = w.array_view().unwrap();

	// `as_bytes` returns a slice into the source `bytes` — same buffer, no
	// copy. Verify three properties that together prove the no-alloc claim:
	//
	// 1. The body pointer lies inside the source buffer (not in a fresh allocation).
	// 2. The body length is strictly less than the source length (the source
	//    has at least the tag + length prefix on top of the body).
	// 3. The body bytes match the corresponding sub-slice of the source (would
	//    fail if the bytes were copied through a buffer that mangled them).
	let body: &[u8] = view.as_bytes();
	let src_start = bytes.as_ptr() as usize;
	let src_end = src_start + bytes.len();
	let body_start = body.as_ptr() as usize;
	let body_end = body_start + body.len();
	assert!(
		body_start >= src_start && body_end <= src_end,
		"view's bytes should lie inside the source buffer; got body={body_start:#x}..{body_end:#x}, source={src_start:#x}..{src_end:#x} (an alloc would land in a different range)"
	);
	assert!(
		body.len() < bytes.len(),
		"body ({}) must be strictly shorter than the full envelope ({}) — the envelope carries the tag + length prefix on top",
		body.len(),
		bytes.len()
	);
	let offset = body_start - src_start;
	assert_eq!(
		body,
		&bytes[offset..offset + body.len()],
		"body slice should equal the corresponding range of the source verbatim"
	);

	// Construct an inner walker over the borrowed body. Streaming descent.
	let mut cursor: &[u8] = body;
	let inner: Vec<u32> =
		<Vec<u32> as revision::DeserializeRevisioned>::deserialize_revisioned(&mut cursor).unwrap();
	assert_eq!(inner, vec![1, 2, 3, 4, 5]);
}

#[test]
fn walker_decode_variant_errors_on_wrong_variant() {
	let v = OptEnum::Unit;
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let w = OptEnum::walk_revisioned(&mut r).unwrap();
	let err = w.decode_varlen().expect_err("Unit is not Varlen — should error");
	match err {
		revision::Error::Deserialize(msg) => {
			assert!(msg.contains("variant mismatch"), "expected variant-mismatch, got: {msg}");
		}
		other => panic!("expected Deserialize error, got {other:?}"),
	}
}

// ---------------------------------------------------------------------------
// `into_<field>_bytes` accessor — borrowed wire bytes without inner decode.
//
// This is the escape hatch for `indexed_struct` types whose plain (non-
// `indexed_*`) fields can't be walked via `into_walk_<field>` on an
// `IndexedBorrowed` parent. The caller takes the bytes and constructs a
// child walker themselves (`T::walk_revisioned(&mut bytes)`).
// ---------------------------------------------------------------------------

#[test]
fn into_field_bytes_on_indexed_borrowed_round_trips() {
	let v = IndexedStruct {
		a: 11,
		b: 22,
		c: 33,
	};
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let w = IndexedStruct::walk_revisioned(&mut r).unwrap();
	// Take the middle field's bytes and decode them independently — the
	// returned slice borrows from the original payload (`Cow::Borrowed`).
	let b_bytes = w.into_b_bytes().unwrap();
	let mut sub: &[u8] = &b_bytes;
	let b: u32 =
		<u32 as revision::DeserializeRevisioned>::deserialize_revisioned(&mut sub).unwrap();
	assert_eq!(b, 22);
}

#[test]
fn into_field_bytes_on_indexed_borrowed_handles_first_and_last_fields() {
	let v = IndexedStruct {
		a: 100,
		b: 200,
		c: 300,
	};
	let bytes = revision::to_vec(&v).unwrap();

	// First field (offset table → bytes[0..4]).
	{
		let mut r: &[u8] = &bytes;
		let w = IndexedStruct::walk_revisioned(&mut r).unwrap();
		let a_bytes = w.into_a_bytes().unwrap();
		let mut sub: &[u8] = &a_bytes;
		let a: u32 =
			<u32 as revision::DeserializeRevisioned>::deserialize_revisioned(&mut sub).unwrap();
		assert_eq!(a, 100);
	}
	// Last field (extends to payload end — exercises the open-ended slice).
	{
		let mut r: &[u8] = &bytes;
		let w = IndexedStruct::walk_revisioned(&mut r).unwrap();
		let c_bytes = w.into_c_bytes().unwrap();
		let mut sub: &[u8] = &c_bytes;
		let c: u32 =
			<u32 as revision::DeserializeRevisioned>::deserialize_revisioned(&mut sub).unwrap();
		assert_eq!(c, 300);
	}
}

#[test]
fn into_field_bytes_on_wire_round_trips() {
	// Optimised (no `indexed_struct`) — the walker enters the Wire arm
	// rather than IndexedBorrowed, so the accessor goes through the
	// remaining()-snapshot + skip-and-recover path.
	let v = OptStruct {
		a: 42,
		b: 100,
	};
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let w = OptStruct::walk_revisioned(&mut r).unwrap();
	let a_bytes = w.into_a_bytes().unwrap();
	// `a` is the first field — bytes should be exactly the u32_le encoding
	// of `a` (4 bytes under the default codec).
	let mut sub: &[u8] = &a_bytes;
	let a: u32 =
		<u32 as revision::DeserializeRevisioned>::deserialize_revisioned(&mut sub).unwrap();
	assert_eq!(a, 42);
	assert_eq!(sub.len(), 0, "into_a_bytes should return exactly the field's bytes");
}

#[test]
fn into_field_bytes_on_wire_returns_borrowed_not_owned() {
	// The Wire arm uses the `BorrowedReader` contract to lifetime-extend
	// the consumed slice — verify we get `Cow::Borrowed` and not an
	// allocated `Cow::Owned`.
	let v = OptStruct {
		a: 1,
		b: 2,
	};
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let w = OptStruct::walk_revisioned(&mut r).unwrap();
	let a_bytes = w.into_a_bytes().unwrap();
	assert!(matches!(a_bytes, std::borrow::Cow::Borrowed(_)));
}

#[test]
fn into_field_bytes_on_indexed_borrowed_returns_borrowed_not_owned() {
	let v = IndexedStruct {
		a: 1,
		b: 2,
		c: 3,
	};
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let w = IndexedStruct::walk_revisioned(&mut r).unwrap();
	let b_bytes = w.into_b_bytes().unwrap();
	assert!(matches!(b_bytes, std::borrow::Cow::Borrowed(_)));
}

#[test]
fn into_field_bytes_works_on_mixed_history_at_optimised_rev() {
	// `MixedHistory` is `revision(1), revision(2, optimised)` — the
	// encoder emits rev 2 (Wire arm with envelope-skipped reader).
	let v = MixedHistory {
		a: 7,
		b: 11,
	};
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let w = MixedHistory::walk_revisioned(&mut r).unwrap();
	let a_bytes = w.into_a_bytes().unwrap();
	let mut sub: &[u8] = &a_bytes;
	let a: u32 =
		<u32 as revision::DeserializeRevisioned>::deserialize_revisioned(&mut sub).unwrap();
	assert_eq!(a, 7);
}
