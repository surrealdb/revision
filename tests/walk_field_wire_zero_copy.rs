//! `walk_<field>` on `indexed_map` / `indexed_seq` / `indexed_set` fields takes
//! a zero-copy path even when the parent walker is `Wire` repr (i.e. an
//! optimised struct **without** `indexed_struct`).
//!
//! Mechanism: the macro emits a `skip_indexed_*` call bracketed by
//! `BorrowedReader::remaining()` snapshots, then derives the field's exact
//! wire bytes from the difference. The bytes are borrowed from the source
//! buffer via the same unsafe lifetime-extension pattern used by
//! `read_borrowed_bytes`.
//!
//! The pointer-aliasing assertion confirms the view's bytes lie inside the
//! original source buffer — i.e. no copy happened.

use std::collections::BTreeMap;

use revision::prelude::*;

#[revisioned(revision(1, encoding = "optimised"))]
#[derive(Debug, Clone, PartialEq)]
struct SequentialDoc {
	id: u32,
	#[revision(indexed_map)]
	fields: BTreeMap<String, u32>,
	#[revision(indexed_seq)]
	tags: Vec<String>,
}

fn make_doc() -> SequentialDoc {
	let mut fields = BTreeMap::new();
	for s in ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel"] {
		fields.insert(s.to_string(), s.len() as u32);
	}
	SequentialDoc {
		id: 42,
		fields,
		tags: (0..10).map(|i| format!("tag-{i}")).collect(),
	}
}

fn ptr_inside(haystack: &[u8], needle: &[u8]) -> bool {
	let start = haystack.as_ptr() as usize;
	let end = start + haystack.len();
	let p = needle.as_ptr() as usize;
	p >= start && p <= end
}

#[test]
fn walk_indexed_map_field_borrows_from_wire_parent() {
	let doc = make_doc();
	let bytes = revision::to_vec(&doc).unwrap();

	// Parent is `Wire` repr (sequential optimised struct, no `struct =
	// "indexed"`). Per commit 13 of the design refactor, walk_<field> should
	// still borrow the field's bytes directly from `bytes` — no allocation.
	let mut r: &[u8] = &bytes;
	let mut w = SequentialDoc::walk_revisioned(&mut r).unwrap();
	w.skip_id().unwrap();
	let view = w.walk_fields().unwrap();
	let body = view.as_bytes();

	assert!(
		ptr_inside(&bytes, body),
		"view bytes must lie inside source buffer (no copy); body ptr=0x{:x} range=0x{:x}..0x{:x}",
		body.as_ptr() as usize,
		bytes.as_ptr() as usize,
		bytes.as_ptr() as usize + bytes.len(),
	);

	// And the view's walker still finds keys by binary search.
	let map_walker = view.walker().unwrap();
	let mut key_buf = Vec::new();
	<String as SerializeRevisioned>::serialize_revisioned(&"delta".to_string(), &mut key_buf)
		.unwrap();
	let value_bytes = map_walker.find_value_bytes(|k| k.cmp(key_buf.as_slice())).unwrap();
	let mut vr: &[u8] = value_bytes.unwrap();
	let v: u32 = <u32 as DeserializeRevisioned>::deserialize_revisioned(&mut vr).unwrap();
	assert_eq!(v, "delta".len() as u32);
}

#[test]
fn walk_indexed_seq_field_borrows_from_wire_parent() {
	let doc = make_doc();
	let bytes = revision::to_vec(&doc).unwrap();

	let mut r: &[u8] = &bytes;
	let mut w = SequentialDoc::walk_revisioned(&mut r).unwrap();
	w.skip_id().unwrap();
	w.skip_fields().unwrap();
	let view = w.walk_tags().unwrap();
	let body = view.as_bytes();

	assert!(ptr_inside(&bytes, body), "tags view must borrow from source buffer");

	let seq = view.walker().unwrap();
	assert_eq!(seq.len(), 10);
	assert!(seq.is_indexed());
	let mut elt: &[u8] = seq.element_bytes(5).unwrap();
	let s: String = <String as DeserializeRevisioned>::deserialize_revisioned(&mut elt).unwrap();
	assert_eq!(s, "tag-5");
}

#[test]
fn walk_field_advances_parent_cursor_correctly() {
	// After walk_<field>(), the parent walker's cursor must be past the
	// field's bytes — same invariant as the slower decode+re-encode path.
	// We verify by reading the NEXT field successfully.
	let doc = make_doc();
	let bytes = revision::to_vec(&doc).unwrap();

	let mut r: &[u8] = &bytes;
	let mut w = SequentialDoc::walk_revisioned(&mut r).unwrap();
	w.skip_id().unwrap();
	let _view = w.walk_fields().unwrap();
	// Should now cleanly walk `tags`.
	let _view2 = w.walk_tags().unwrap();
}
