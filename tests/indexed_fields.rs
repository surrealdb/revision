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
	// `walk_fields` returns an IndexedMapView; the caller borrows an
	// IndexedMapWalker from it and can binary-search keys directly without
	// fully materialising the map.
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

	// New: walk into the indexed field directly.
	let view = w.walk_fields().unwrap();
	let map_walker = view.walker().unwrap();
	let mut target_bytes = Vec::new();
	<String as SerializeRevisioned>::serialize_revisioned(&"delta".to_string(), &mut target_bytes)
		.unwrap();
	let value_bytes = map_walker.find_value_bytes(|k| k.cmp(target_bytes.as_slice())).unwrap();
	let value_bytes = value_bytes.expect("delta should be findable via binary search");
	// Decode the value bytes.
	let mut vr: &[u8] = value_bytes;
	let v: u32 = <u32 as revision::DeserializeRevisioned>::deserialize_revisioned(&mut vr).unwrap();
	assert_eq!(v, 3, "delta was inserted with value 3");
}

#[test]
fn indexed_seq_walker_can_random_access_elements() {
	// 8+ elements to engage the indexed path (OFFSET_TABLE_MIN_LEN = 8).
	// Below that threshold the encoder emits a legacy `(elem)*` body that
	// the walker can still iterate but not random-access.
	let tags: Vec<String> = (0..10).map(|i| format!("tag-{i}")).collect();
	let doc = Doc {
		id: 0,
		fields: BTreeMap::new(),
		summary: String::new(),
		tags: tags.clone(),
	};
	let bytes = revision::to_vec(&doc).unwrap();
	let mut r: &[u8] = &bytes;
	let mut w = Doc::walk_revisioned(&mut r).unwrap();
	w.skip_id().unwrap();
	w.skip_fields().unwrap();
	w.skip_summary().unwrap();

	let view = w.walk_tags().unwrap();
	let seq_walker = view.walker().unwrap();
	assert_eq!(seq_walker.len(), 10);
	assert!(seq_walker.is_indexed(), "10 >= threshold: should be indexed");
	// Read element 5 — random access by index, O(1).
	let bytes = seq_walker.element_bytes(5).unwrap();
	let mut r: &[u8] = bytes;
	let v: String =
		<String as revision::DeserializeRevisioned>::deserialize_revisioned(&mut r).unwrap();
	assert_eq!(v, "tag-5");
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

#[test]
fn indexed_map_works_for_std_hashmap() {
	use std::collections::HashMap;

	#[revisioned(revision(1, encoding = "optimised"))]
	#[derive(Debug, Clone, PartialEq)]
	struct WithHashMap {
		#[revision(indexed_map)]
		fields: HashMap<String, u32>,
	}

	let mut fields = HashMap::new();
	for (i, s) in ["alpha", "bravo", "charlie", "delta"].iter().enumerate() {
		fields.insert(s.to_string(), i as u32);
	}
	let v = WithHashMap {
		fields: fields.clone(),
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: WithHashMap = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded.fields, fields);
}

#[cfg(feature = "imbl")]
#[test]
fn indexed_map_works_for_imbl_ordmap() {
	#[revisioned(revision(1, encoding = "optimised"))]
	#[derive(Debug, Clone, PartialEq)]
	struct WithOrdMap {
		#[revision(indexed_map)]
		fields: imbl::OrdMap<String, u32>,
	}

	let fields: imbl::OrdMap<String, u32> = ["alpha", "bravo", "charlie", "delta"]
		.iter()
		.enumerate()
		.map(|(i, s)| (s.to_string(), i as u32))
		.collect();
	let v = WithOrdMap {
		fields: fields.clone(),
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: WithOrdMap = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded.fields, fields);
}

#[cfg(feature = "imbl")]
#[test]
fn indexed_map_works_for_imbl_hashmap() {
	#[revisioned(revision(1, encoding = "optimised"))]
	#[derive(Debug, Clone, PartialEq)]
	struct WithHashMap {
		#[revision(indexed_map)]
		fields: imbl::HashMap<String, u32>,
	}

	let fields: imbl::HashMap<String, u32> = ["alpha", "bravo", "charlie", "delta"]
		.iter()
		.enumerate()
		.map(|(i, s)| (s.to_string(), i as u32))
		.collect();
	let v = WithHashMap {
		fields: fields.clone(),
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: WithHashMap = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded.fields, fields);
}

#[test]
fn indexed_set_works_for_btreeset() {
	use std::collections::BTreeSet;

	#[revisioned(revision(1, encoding = "optimised"))]
	#[derive(Debug, Clone, PartialEq)]
	struct WithSet {
		#[revision(indexed_set)]
		tags: BTreeSet<String>,
	}

	let mut tags = BTreeSet::new();
	for s in &["alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel"] {
		tags.insert(s.to_string());
	}
	let v = WithSet {
		tags: tags.clone(),
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: WithSet = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded.tags, tags);
}

#[test]
fn indexed_set_works_for_hashset() {
	use std::collections::HashSet;

	#[revisioned(revision(1, encoding = "optimised"))]
	#[derive(Debug, Clone, PartialEq)]
	struct WithSet {
		#[revision(indexed_set)]
		ids: HashSet<u64>,
	}

	let ids: HashSet<u64> = (0u64..10).collect();
	let v = WithSet {
		ids: ids.clone(),
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: WithSet = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded.ids, ids);
}

#[test]
fn indexed_set_walker_can_find_membership() {
	use revision::optimised::IndexedSeqWalker;
	use std::collections::BTreeSet;

	let set: BTreeSet<String> =
		["alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel"]
			.iter()
			.map(|s| s.to_string())
			.collect();
	let mut bytes = Vec::new();
	revision::optimised::indexed::serialize_indexed_set_iter(set.iter(), &mut bytes).unwrap();

	let walker: IndexedSeqWalker<String> = IndexedSeqWalker::from_payload(&bytes).unwrap();
	assert!(walker.is_indexed());
	assert_eq!(walker.len(), 8);
	// Verify the bytes are byte-sorted: iterating element_bytes(i) gives
	// ascending sequences.
	let mut prev: &[u8] = &[];
	for i in 0..8 {
		let b = walker.element_bytes(i).unwrap();
		assert!(b > prev, "element bytes must be strictly ascending");
		prev = b;
	}
}

#[cfg(feature = "imbl")]
#[test]
fn indexed_set_works_for_imbl_ordset() {
	#[revisioned(revision(1, encoding = "optimised"))]
	#[derive(Debug, Clone, PartialEq)]
	struct WithSet {
		#[revision(indexed_set)]
		tags: imbl::OrdSet<String>,
	}
	let tags: imbl::OrdSet<String> =
		["alpha", "bravo", "charlie", "delta"].iter().map(|s| s.to_string()).collect();
	let v = WithSet {
		tags: tags.clone(),
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: WithSet = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded.tags, tags);
}

#[cfg(feature = "imbl")]
#[test]
fn indexed_seq_works_for_imbl_vector() {
	#[revisioned(revision(1, encoding = "optimised"))]
	#[derive(Debug, Clone, PartialEq)]
	struct WithVector {
		#[revision(indexed_seq)]
		tags: imbl::Vector<String>,
	}

	let tags: imbl::Vector<String> =
		["one", "two", "three"].iter().map(|s| s.to_string()).collect();
	let v = WithVector {
		tags: tags.clone(),
	};
	let bytes = revision::to_vec(&v).unwrap();
	let decoded: WithVector = revision::from_slice(&bytes).unwrap();
	assert_eq!(decoded.tags, tags);
}
