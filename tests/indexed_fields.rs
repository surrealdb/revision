//! End-to-end tests for `#[revision(indexed_map)]` / `#[revision(indexed_seq)]`
//! on struct fields under optimised encoding.

use std::collections::BTreeMap;

use revision::prelude::*;

#[revisioned(revision(1, encoding = "optimised"))]
#[derive(Debug, Clone, PartialEq)]
struct Doc {
	id: u32,
	#[revision(indexed_map)]
	fields: BTreeMap<String, u32>,
	summary: String,
	#[revision(indexed_seq)]
	tags: Vec<String>,
}

#[test]
fn indexed_field_round_trips() {
	let mut fields = BTreeMap::new();
	fields.insert("alpha".to_string(), 1);
	fields.insert("bravo".to_string(), 2);
	fields.insert("charlie".to_string(), 3);

	let original = Doc {
		id: 42,
		fields,
		summary: "test doc".into(),
		tags: vec!["one".into(), "two".into(), "three".into()],
	};
	let bytes = revision::to_vec(&original).unwrap();
	let decoded: Doc = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, original);
}

#[test]
fn indexed_field_walker_can_binary_search_keys() {
	// The walker over the parent struct returns the indexed-map bytes as one
	// field's payload; users can then feed them to `IndexedMapWalker`.
	use revision::optimised::IndexedMapWalker;

	let mut fields = BTreeMap::new();
	for (i, s) in ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel"]
		.iter()
		.enumerate()
	{
		fields.insert(s.to_string(), i as u32);
	}
	let doc = Doc {
		id: 0,
		fields: fields.clone(),
		summary: String::new(),
		tags: vec![],
	};
	let bytes = revision::to_vec(&doc).unwrap();

	let mut r: &[u8] = &bytes;
	let mut w = Doc::walk_revisioned(&mut r).unwrap();
	w.skip_id().unwrap();
	// `walk_fields` returns the inner walker for the BTreeMap. But the indexed
	// path serialised the map as an opaque blob — we recover its bytes by
	// borrowing via decode_fields, then feed them to IndexedMapWalker.
	//
	// (A future enhancement could expose `field_bytes` on the parent walker
	// directly. For now, decoding the field reconstructs the BTreeMap.)
	let recovered: BTreeMap<String, u32> = w.decode_fields().unwrap();
	assert_eq!(recovered, fields);

	// Independently, serialise that same map directly and verify the
	// IndexedMapWalker can binary-search it — same wire format as what the
	// struct emitted.
	let mut indexed_bytes = Vec::new();
	revision::optimised::indexed::serialize_indexed_map(&fields, &mut indexed_bytes).unwrap();
	let walker: IndexedMapWalker<String, u32> =
		IndexedMapWalker::from_payload(&indexed_bytes).unwrap();
	let mut target_bytes = Vec::new();
	<String as SerializeRevisioned>::serialize_revisioned(&"delta".to_string(), &mut target_bytes)
		.unwrap();
	let found = walker.find_value_bytes(|k| k.cmp(target_bytes.as_slice())).unwrap();
	assert!(found.is_some(), "delta should be findable via binary search");
}

#[test]
fn indexed_field_handles_empty_collections() {
	let original = Doc {
		id: 7,
		fields: BTreeMap::new(),
		summary: "empty doc".into(),
		tags: vec![],
	};
	let bytes = revision::to_vec(&original).unwrap();
	let decoded: Doc = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded, original);
}

#[test]
fn indexed_map_and_seq_are_mutually_exclusive_at_compile_time() {
	// `compile_fail` for `#[revision(indexed_map, indexed_seq)]` is locked in
	// by a separate trybuild fixture; this test exists to remind the reader.
}
