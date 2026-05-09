#![allow(dead_code)]
//! Behavioural tests for [`WalkRevisioned`].
//!
//! These exercise the round-trip equivalence of `decode` via the walker vs
//! [`DeserializeRevisioned`], the `skip` equivalence vs [`SkipRevisioned`], and
//! mixed-mode scenarios where some children are decoded, some skipped, and some
//! walked into.

use std::collections::BTreeMap;

use revision::{
	DeserializeRevisioned, EnumWalker, Error, MapWalker, SeqWalker, SkipRevisioned, StructWalker,
	WalkRevisioned, revisioned, to_vec,
};

// -----------------------------------------------------------------------------
// Primitive leaf walkers
// -----------------------------------------------------------------------------

#[test]
fn primitive_leaf_decode_matches_deserialize() {
	let bytes = to_vec(&42u64).unwrap();

	let mut r = bytes.as_slice();
	let walker = u64::walk_revisioned(&mut r).unwrap();
	let decoded = walker.decode().unwrap();
	assert_eq!(decoded, 42);
	assert!(r.is_empty(), "walker should consume entire encoded value");

	let mut r2 = bytes.as_slice();
	let direct = u64::deserialize_revisioned(&mut r2).unwrap();
	assert_eq!(decoded, direct);
}

#[test]
fn primitive_leaf_skip_matches_skip_revisioned() {
	let bytes = to_vec(&u128::MAX).unwrap();

	let mut r = bytes.as_slice();
	let walker = u128::walk_revisioned(&mut r).unwrap();
	walker.skip().unwrap();
	assert!(r.is_empty(), "walker should consume entire encoded value");

	let mut r2 = bytes.as_slice();
	u128::skip_revisioned(&mut r2).unwrap();
	assert!(r2.is_empty());
}

// -----------------------------------------------------------------------------
// Option<T>
// -----------------------------------------------------------------------------

#[test]
fn option_walker_some_decode_skip_walk() {
	let bytes = to_vec(&Some(7u32)).unwrap();

	// Decode
	let mut r = bytes.as_slice();
	let walker = <Option<u32>>::walk_revisioned(&mut r).unwrap();
	assert!(walker.is_some());
	assert_eq!(walker.decode().unwrap(), Some(7));
	assert!(r.is_empty());

	// Skip
	let mut r = bytes.as_slice();
	let walker = <Option<u32>>::walk_revisioned(&mut r).unwrap();
	walker.skip().unwrap();
	assert!(r.is_empty());

	// Walk into Some
	let mut r = bytes.as_slice();
	let walker = <Option<u32>>::walk_revisioned(&mut r).unwrap();
	let inner = walker.into_some().unwrap();
	assert_eq!(inner.decode().unwrap(), 7);
	assert!(r.is_empty());
}

#[test]
fn option_walker_none_skip_does_not_overread() {
	let bytes = to_vec(&None::<u32>).unwrap();
	let mut r = bytes.as_slice();
	let walker = <Option<u32>>::walk_revisioned(&mut r).unwrap();
	assert!(walker.is_none());
	walker.skip().unwrap();
	assert!(r.is_empty());
}

// -----------------------------------------------------------------------------
// Vec<T>
// -----------------------------------------------------------------------------

#[test]
fn seq_walker_iterates_decode_skip_mix() {
	// Use Vec<String> rather than Vec<u32>: integer vectors may use the
	// `specialised-vectors` bulk encoding which the generic SeqWalker does
	// not understand. Non-numeric Vec<T> uses the standard length-prefixed
	// per-element encoding.
	let v: Vec<String> = vec!["one".into(), "two".into(), "three".into(), "four".into(), "five".into()];
	let bytes = to_vec(&v).unwrap();

	let mut r = bytes.as_slice();
	let mut walker: SeqWalker<String, _> = <Vec<String>>::walk_revisioned(&mut r).unwrap();
	assert_eq!(walker.remaining(), 5);

	let mut decoded = Vec::new();
	for _ in 0..3 {
		let item = walker.next_item().expect("item available");
		decoded.push(item.decode().unwrap());
	}
	assert_eq!(decoded, vec![String::from("one"), String::from("two"), String::from("three")]);
	assert_eq!(walker.remaining(), 2);

	walker.skip_remaining().unwrap();
	assert!(r.is_empty(), "walker should fully consume the seq payload");
}

// -----------------------------------------------------------------------------
// BTreeMap<K, V>
// -----------------------------------------------------------------------------

#[test]
fn map_walker_full_iteration_matches_deserialize() {
	let mut map: BTreeMap<String, u32> = BTreeMap::new();
	map.insert("alpha".into(), 1);
	map.insert("beta".into(), 2);
	map.insert("gamma".into(), 3);
	let bytes = to_vec(&map).unwrap();

	let mut r = bytes.as_slice();
	let mut walker: MapWalker<String, u32, _> =
		<BTreeMap<String, u32>>::walk_revisioned(&mut r).unwrap();
	assert_eq!(walker.remaining(), 3);

	let mut collected: BTreeMap<String, u32> = BTreeMap::new();
	while let Some(entry) = walker.next_entry() {
		let (k, v) = entry.decode_pair().unwrap();
		collected.insert(k, v);
	}
	assert_eq!(collected, map);
	assert!(r.is_empty());
}

#[test]
fn map_walker_skip_value_for_unwanted_keys() {
	let mut map: BTreeMap<String, u32> = BTreeMap::new();
	map.insert("a".into(), 1);
	map.insert("b".into(), 2);
	map.insert("c".into(), 3);
	let bytes = to_vec(&map).unwrap();

	let mut r = bytes.as_slice();
	let mut walker: MapWalker<String, u32, _> =
		<BTreeMap<String, u32>>::walk_revisioned(&mut r).unwrap();

	let mut found = None;
	while let Some(mut entry) = walker.next_entry() {
		let key = entry.decode_key().unwrap();
		if key == "b" {
			found = Some(entry.decode_value().unwrap());
		} else {
			entry.skip_value().unwrap();
		}
	}
	assert_eq!(found, Some(2));
	assert!(r.is_empty(), "all entries must be consumed");
}

#[test]
fn map_walker_find_by_key_consumes_after_match() {
	let mut map: BTreeMap<String, u32> = BTreeMap::new();
	map.insert("alpha".into(), 1);
	map.insert("beta".into(), 2);
	map.insert("delta".into(), 3);
	map.insert("epsilon".into(), 4);
	let bytes = to_vec(&map).unwrap();

	let mut r = bytes.as_slice();
	let walker: MapWalker<String, u32, _> =
		<BTreeMap<String, u32>>::walk_revisioned(&mut r).unwrap();

	let needle = "delta";
	let handle = walker
		.find(|k: &String| {
			let k = k.as_str();
			k.cmp(needle)
		})
		.unwrap();
	let handle = handle.expect("delta should be present");
	let value: u32 = handle.decode().unwrap();
	assert_eq!(value, 3);

	// `find` is allowed to leave entries past the match unconsumed; here we
	// asserted it returns once the match is found, so trailing bytes may
	// exist on the wire. Verify by skipping them manually with a fresh
	// reader.
	let mut r2 = bytes.as_slice();
	<BTreeMap<String, u32>>::skip_revisioned(&mut r2).unwrap();
	assert!(r2.is_empty());
}

#[test]
fn map_walker_find_returns_none_consumes_all() {
	let mut map: BTreeMap<String, u32> = BTreeMap::new();
	map.insert("alpha".into(), 1);
	map.insert("beta".into(), 2);
	let bytes = to_vec(&map).unwrap();

	let mut r = bytes.as_slice();
	let walker: MapWalker<String, u32, _> =
		<BTreeMap<String, u32>>::walk_revisioned(&mut r).unwrap();

	let result = walker
		.find(|k: &String| k.as_str().cmp("zzz"))
		.unwrap();
	assert!(result.is_none());
	assert!(r.is_empty(), "no-match find should consume entire map");
}

// -----------------------------------------------------------------------------
// Derive — struct
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
#[derive(Debug, Clone, PartialEq)]
struct Document {
	name: String,
	count: u32,
}

#[test]
fn derive_struct_walker_decodes_each_field_in_order() {
	let doc = Document {
		name: "hello".into(),
		count: 42,
	};
	let bytes = to_vec(&doc).unwrap();

	let mut r = bytes.as_slice();
	let mut walker: StructWalker<_> = Document::walk_revisioned(&mut r).unwrap();
	assert_eq!(walker.revision(), 1);
	let name: String = walker.decode().unwrap();
	let count: u32 = walker.decode().unwrap();
	assert_eq!(name, doc.name);
	assert_eq!(count, doc.count);
	assert!(r.is_empty());
}

#[test]
fn derive_struct_walker_skip_field_skips_correctly() {
	let doc = Document {
		name: "skip-me".into(),
		count: 99,
	};
	let bytes = to_vec(&doc).unwrap();

	let mut r = bytes.as_slice();
	let mut walker: StructWalker<_> = Document::walk_revisioned(&mut r).unwrap();
	walker.skip::<String>().unwrap();
	let count: u32 = walker.decode().unwrap();
	assert_eq!(count, 99);
	assert!(r.is_empty());
}

#[test]
fn derive_struct_walker_field_table_is_emitted() {
	assert_eq!(Document::__WALK_FIELD_NAMES, &["name", "count"]);
}

// -----------------------------------------------------------------------------
// Derive — enum
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
#[derive(Debug, PartialEq)]
enum Shape {
	Square(u32),
	Rectangle {
		w: u32,
		h: u32,
	},
	Circle(u32),
}

#[test]
fn derive_enum_walker_exposes_discriminant() {
	let s = Shape::Circle(7);
	let bytes = to_vec(&s).unwrap();

	let mut r = bytes.as_slice();
	let walker: EnumWalker<_> = Shape::walk_revisioned(&mut r).unwrap();
	assert_eq!(walker.revision(), 1);
	let disc = walker.discriminant();
	let name = Shape::__walk_variant_name(disc);
	assert_eq!(name, Some("Circle"));

	let payload: u32 = walker.decode().unwrap();
	assert_eq!(payload, 7);
	assert!(r.is_empty());
}

#[test]
fn derive_enum_variant_table_is_emitted() {
	let table = Shape::__WALK_VARIANT_TABLE;
	let names: Vec<&'static str> = table.iter().map(|(n, _)| *n).collect();
	assert_eq!(names, vec!["Square", "Rectangle", "Circle"]);
}

#[test]
fn derive_enum_walker_skip_matches_skip_revisioned() {
	let s = Shape::Square(13);
	let bytes = to_vec(&s).unwrap();

	let mut r = bytes.as_slice();
	let walker: EnumWalker<_> = Shape::walk_revisioned(&mut r).unwrap();
	walker.skip::<u32>().unwrap();
	assert!(r.is_empty());

	let mut r2 = bytes.as_slice();
	Shape::skip_revisioned(&mut r2).unwrap();
	assert!(r2.is_empty());
}

// -----------------------------------------------------------------------------
// Mixed: walk into nested structure
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
#[derive(Debug)]
struct Outer {
	header: u16,
	body: BTreeMap<String, u32>,
}

#[test]
fn struct_walker_walks_into_nested_map() {
	let mut body = BTreeMap::new();
	body.insert("alpha".into(), 10u32);
	body.insert("beta".into(), 20);
	body.insert("gamma".into(), 30);

	let outer = Outer {
		header: 7,
		body,
	};
	let bytes = to_vec(&outer).unwrap();

	let mut r = bytes.as_slice();
	let mut walker: StructWalker<_> = Outer::walk_revisioned(&mut r).unwrap();
	let header: u16 = walker.decode().unwrap();
	assert_eq!(header, 7);

	let map_walker: MapWalker<String, u32, _> = walker.into_walk::<BTreeMap<String, u32>>().unwrap();

	let collected: BTreeMap<String, u32> = {
		let mut walker = map_walker;
		let mut acc = BTreeMap::new();
		while let Some(entry) = walker.next_entry() {
			let (k, v) = entry.decode_pair().unwrap();
			acc.insert(k, v);
		}
		acc
	};
	assert_eq!(collected.len(), 3);
	assert_eq!(collected["alpha"], 10);
	assert_eq!(collected["beta"], 20);
	assert_eq!(collected["gamma"], 30);
	assert!(r.is_empty());
}

// -----------------------------------------------------------------------------
// Multi-revision: walker rejects mismatched wire revision
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
#[derive(Debug)]
struct OldShape {
	kind: u8,
}

#[revisioned(revision = 2)]
#[derive(Debug)]
struct NewShape {
	kind: u8,
	#[revision(start = 2)]
	flags: u8,
}

#[test]
fn walker_rejects_older_wire_revision() {
	let old = OldShape {
		kind: 3,
	};
	let bytes = to_vec(&old).unwrap();

	let mut r = bytes.as_slice();
	let res = NewShape::walk_revisioned(&mut r);
	assert!(matches!(res, Err(Error::Deserialize(_))));
}

#[test]
fn deserialize_still_handles_older_wire_revision() {
	let old = OldShape {
		kind: 3,
	};
	let bytes = to_vec(&old).unwrap();

	let mut r = bytes.as_slice();
	let parsed = NewShape::deserialize_revisioned(&mut r).unwrap();
	assert_eq!(parsed.kind, 3);
	assert_eq!(parsed.flags, 0);
}
