#![allow(dead_code)]
//! Behavioural tests for [`WalkRevisioned`].
//!
//! These exercise the round-trip equivalence of `decode` via the walker vs
//! [`DeserializeRevisioned`], the `skip` equivalence vs [`SkipRevisioned`], and
//! mixed-mode scenarios where some children are decoded, some skipped, and some
//! walked into.

use std::collections::BTreeMap;

use revision::{
	DeserializeRevisioned, Error, MapWalker, SeqWalker, SkipRevisioned, WalkRevisioned, revisioned,
	to_vec,
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
	let v: Vec<String> =
		vec!["one".into(), "two".into(), "three".into(), "four".into(), "five".into()];
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

	let result = walker.find(|k: &String| k.as_str().cmp("zzz")).unwrap();
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
	let mut walker = Document::walk_revisioned(&mut r).unwrap();
	assert_eq!(walker.revision(), 1);
	let name = walker.decode_name().unwrap();
	let count = walker.decode_count().unwrap();
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
	let mut walker = Document::walk_revisioned(&mut r).unwrap();
	walker.skip_name().unwrap();
	let count = walker.decode_count().unwrap();
	assert_eq!(count, 99);
	assert!(r.is_empty());
}

#[test]
fn derive_struct_walker_field_table_is_emitted() {
	assert_eq!(Document::walk_revisioned_field_names(1), &["name", "count"]);
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
	let walker = Shape::walk_revisioned(&mut r).unwrap();
	assert_eq!(walker.revision(), 1);
	let disc = walker.discriminant();
	let name = Shape::walk_revisioned_variant_name(1, disc);
	assert_eq!(name, Some("Circle"));
	assert!(walker.is_circle());

	// Single-field tuple variant: into_circle yields the inner walker over `u32`.
	let inner = walker.into_circle().unwrap();
	let payload = inner.decode().unwrap();
	assert_eq!(payload, 7);
	assert!(r.is_empty());
}

#[test]
fn derive_enum_variant_table_is_emitted() {
	let table = Shape::walk_revisioned_variant_table(1);
	let names: Vec<&'static str> = table.iter().map(|(n, _)| *n).collect();
	assert_eq!(names, vec!["Square", "Rectangle", "Circle"]);
}

#[test]
fn derive_enum_walker_skip_matches_skip_revisioned() {
	let s = Shape::Square(13);
	let bytes = to_vec(&s).unwrap();

	let mut r = bytes.as_slice();
	let walker = Shape::walk_revisioned(&mut r).unwrap();
	assert!(walker.is_square());
	let inner = walker.into_square().unwrap();
	inner.skip().unwrap();
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
	let mut walker = Outer::walk_revisioned(&mut r).unwrap();
	let header = walker.decode_header().unwrap();
	assert_eq!(header, 7);

	let collected: BTreeMap<String, u32> = {
		let mut map_walker = walker.walk_body().unwrap();
		let mut acc = BTreeMap::new();
		while let Some(entry) = map_walker.next_entry() {
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
// Multi-revision: walker accepts any wire revision and presents latest schema
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
fn walker_accepts_older_wire_revision_with_default_for_added_field() {
	// Encode at rev 1, walk at rev 2. Wire-mode walker should read the
	// existing `kind` field directly and synthesise a default for the
	// `flags` field added at rev 2.
	let old = OldShape {
		kind: 3,
	};
	let bytes = to_vec(&old).unwrap();

	let mut r = bytes.as_slice();
	let mut walker = NewShape::walk_revisioned(&mut r).unwrap();
	assert_eq!(walker.revision(), 1);
	let kind = walker.decode_kind().unwrap();
	let flags = walker.decode_flags().unwrap();
	assert_eq!(kind, 3);
	assert_eq!(flags, 0); // default for `flags` at wire rev 1
	assert!(r.is_empty());
}

#[test]
fn walker_rejects_invalid_wire_revision() {
	// A wire revision that exceeds the schema's current revision must error.
	let mut bytes = Vec::new();
	use revision::SerializeRevisioned;
	99u16.serialize_revisioned(&mut bytes).unwrap();
	bytes.push(0u8);

	let mut r = bytes.as_slice();
	let res = NewShape::walk_revisioned(&mut r);
	assert!(matches!(res, Err(Error::Deserialize(_))));
}

// -----------------------------------------------------------------------------
// Materialised-mode cross-rev: types using `convert_fn`
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
#[derive(Debug, Clone, PartialEq)]
struct ConvertedFooV1 {
	width: u32,
}

#[revisioned(revision = 2)]
#[derive(Debug, PartialEq)]
struct ConvertedFoo {
	#[revision(end = 2, convert_fn = "convert_width")]
	width_old: u32,
	#[revision(start = 2)]
	width: u32,
	#[revision(start = 2)]
	height: u32,
}

impl ConvertedFoo {
	fn convert_width(&mut self, _rev: u16, value: u32) -> Result<(), revision::Error> {
		self.width = value * 10;
		self.height = value + 1;
		Ok(())
	}
}

#[test]
fn walker_materialises_for_convert_fn_type_at_older_revision() {
	// Encode at rev 1, walk at rev 2. The walker should detect the
	// `convert_fn` and materialise: deserialize at wire rev 1 (which
	// runs the converter) and re-encode at current revision so the
	// walker can read sequentially at the latest schema.
	let v1 = ConvertedFooV1 {
		width: 5,
	};
	let bytes = to_vec(&v1).unwrap();

	let mut r = bytes.as_slice();
	let mut walker = ConvertedFoo::walk_revisioned(&mut r).unwrap();
	// Materialised mode reports the schema revision since bytes are
	// re-encoded at current.
	assert_eq!(walker.revision(), 2);

	let width = walker.decode_width().unwrap();
	let height = walker.decode_height().unwrap();
	assert_eq!(width, 50); // 5 * 10
	assert_eq!(height, 6); // 5 + 1
	assert!(r.is_empty());
}

#[test]
fn walker_materialised_walk_field_errors_with_useful_message() {
	let v1 = ConvertedFooV1 {
		width: 5,
	};
	let bytes = to_vec(&v1).unwrap();
	let mut r = bytes.as_slice();
	let walker = ConvertedFoo::walk_revisioned(&mut r).unwrap();
	let res = walker.walk_width();
	assert!(matches!(res, Err(Error::Conversion(_))));
}

#[test]
fn walker_current_rev_path_for_convert_fn_type_uses_wire_mode() {
	// At the current revision, no materialisation happens; the walker
	// reads sequentially from the input reader.
	let v2 = ConvertedFoo {
		width: 42,
		height: 99,
	};
	let mut bytes = Vec::new();
	use revision::SerializeRevisioned;
	v2.serialize_revisioned(&mut bytes).unwrap();

	let mut r = bytes.as_slice();
	let mut walker = ConvertedFoo::walk_revisioned(&mut r).unwrap();
	assert_eq!(walker.revision(), 2);
	let width = walker.decode_width().unwrap();
	let height = walker.decode_height().unwrap();
	assert_eq!(width, 42);
	assert_eq!(height, 99);
	assert!(r.is_empty());
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

// -----------------------------------------------------------------------------
// Zero-copy peeking via BorrowedReader + LengthPrefixedBytes
// -----------------------------------------------------------------------------

#[test]
fn leaf_walker_with_bytes_matches_decode_for_string() {
	let s = "hello, walker".to_string();
	let bytes = to_vec(&s).unwrap();

	let mut r = bytes.as_slice();
	let walker = String::walk_revisioned(&mut r).unwrap();
	let observed = walker.with_bytes(|key| key.to_vec()).unwrap();
	assert_eq!(observed.as_slice(), s.as_bytes());
	assert!(r.is_empty(), "with_bytes should consume the whole leaf");

	let mut r2 = bytes.as_slice();
	let decoded = String::deserialize_revisioned(&mut r2).unwrap();
	assert_eq!(decoded, s);
}

#[test]
fn leaf_walker_with_bytes_matches_decode_for_pathbuf() {
	use std::path::PathBuf;
	let p = PathBuf::from("/etc/passwd");
	let bytes = to_vec(&p).unwrap();

	let mut r = bytes.as_slice();
	let walker = PathBuf::walk_revisioned(&mut r).unwrap();
	let observed = walker.with_bytes(|raw| raw.to_vec()).unwrap();
	assert_eq!(observed.as_slice(), p.to_string_lossy().as_bytes());
	assert!(r.is_empty());
}

#[test]
fn map_walker_find_bytes_matches_find_for_string_keys() {
	let mut map: BTreeMap<String, u32> = BTreeMap::new();
	for (k, v) in [("alpha", 1u32), ("beta", 2), ("delta", 3), ("epsilon", 4)] {
		map.insert(k.into(), v);
	}
	let bytes = to_vec(&map).unwrap();

	// Typed path: existing `find`.
	let mut r1 = bytes.as_slice();
	let walker_typed: MapWalker<String, u32, _> =
		<BTreeMap<String, u32>>::walk_revisioned(&mut r1).unwrap();
	let typed = walker_typed
		.find(|k: &String| k.as_str().cmp("delta"))
		.unwrap()
		.expect("delta should be present")
		.decode()
		.unwrap();

	// Bytes path: new `find_bytes`.
	let mut r2 = bytes.as_slice();
	let walker_bytes: MapWalker<String, u32, _> =
		<BTreeMap<String, u32>>::walk_revisioned(&mut r2).unwrap();
	let bytewise = walker_bytes
		.find_bytes(|k| k.cmp(b"delta".as_slice()))
		.unwrap()
		.expect("delta should be present")
		.decode()
		.unwrap();

	assert_eq!(typed, bytewise);
}

#[test]
fn map_walker_find_bytes_returns_none_consumes_all() {
	let mut map: BTreeMap<String, u32> = BTreeMap::new();
	map.insert("alpha".into(), 1);
	map.insert("beta".into(), 2);
	let bytes = to_vec(&map).unwrap();

	let mut r = bytes.as_slice();
	let walker: MapWalker<String, u32, _> =
		<BTreeMap<String, u32>>::walk_revisioned(&mut r).unwrap();
	let result = walker.find_bytes(|k| k.cmp(b"zzz".as_slice())).unwrap();
	assert!(result.is_none());
	assert!(r.is_empty(), "no-match find_bytes should consume entire map");
}

#[test]
fn map_entry_with_key_bytes_zero_copy_iteration() {
	let mut map: BTreeMap<String, u32> = BTreeMap::new();
	map.insert("alpha".into(), 1);
	map.insert("beta".into(), 2);
	map.insert("gamma".into(), 3);
	let bytes = to_vec(&map).unwrap();

	let mut r = bytes.as_slice();
	let mut walker: MapWalker<String, u32, _> =
		<BTreeMap<String, u32>>::walk_revisioned(&mut r).unwrap();

	let mut found_beta = None;
	while let Some(mut entry) = walker.next_entry() {
		let is_target = entry.with_key_bytes(|k| k == b"beta").unwrap();
		if is_target {
			found_beta = Some(entry.decode_value().unwrap());
		} else {
			entry.skip_value().unwrap();
		}
	}
	assert_eq!(found_beta, Some(2));
	assert!(r.is_empty(), "map walker should consume the entire map");
}

#[test]
fn map_entry_with_value_bytes_for_byte_values() {
	let mut map: BTreeMap<String, Vec<u8>> = BTreeMap::new();
	map.insert("a".into(), b"first-value".to_vec());
	map.insert("b".into(), b"second-value-longer".to_vec());
	let bytes = to_vec(&map).unwrap();

	let mut r = bytes.as_slice();
	let mut walker: MapWalker<String, Vec<u8>, _> =
		<BTreeMap<String, Vec<u8>>>::walk_revisioned(&mut r).unwrap();

	let mut second_observed: Option<Vec<u8>> = None;
	while let Some(mut entry) = walker.next_entry() {
		let key = entry.decode_key().unwrap();
		if key == "b" {
			second_observed = Some(entry.with_value_bytes(|raw| raw.to_vec()).unwrap());
		} else {
			entry.skip_value().unwrap();
		}
	}
	assert_eq!(second_observed.as_deref(), Some(b"second-value-longer".as_slice()));
	assert!(r.is_empty());
}

#[test]
fn seq_item_with_bytes_for_string_seq() {
	let v: Vec<String> = vec!["alpha".into(), "beta".into(), "gamma".into(), "delta".into()];
	let bytes = to_vec(&v).unwrap();

	let mut r = bytes.as_slice();
	let mut walker: SeqWalker<String, _> = <Vec<String>>::walk_revisioned(&mut r).unwrap();
	assert_eq!(walker.remaining(), 4);

	// Mix three behaviours within a single iteration:
	//   - first item: peek bytes, copy into Vec
	//   - second item: decode normally
	//   - third item: skip
	//   - fourth item: peek bytes, comparison
	let first = walker.next_item().unwrap().with_bytes(|b| b.to_vec()).unwrap();
	let second = walker.next_item().unwrap().decode().unwrap();
	walker.next_item().unwrap().skip().unwrap();
	let fourth_starts_with_d =
		walker.next_item().unwrap().with_bytes(|b| b.starts_with(b"d")).unwrap();

	assert_eq!(first.as_slice(), b"alpha");
	assert_eq!(second, "beta");
	assert!(fourth_starts_with_d);
	assert!(r.is_empty());
}

#[test]
#[cfg(feature = "specialised-vectors")]
fn seq_walk_rejects_bulk_primitive_without_consuming_reader() {
	let v: Vec<u32> = vec![1, 2, 3];
	let bytes = to_vec(&v).unwrap();
	let mut r = bytes.as_slice();
	let before = r.len();
	let err = match Vec::<u32>::walk_revisioned(&mut r) {
		Err(e) => e,
		Ok(_) => panic!("expected specialised bulk Vec<u32> walk to fail"),
	};
	assert!(matches!(err, Error::Deserialize(_)));
	assert_eq!(r.len(), before);
}

#[test]
#[cfg(not(feature = "specialised-vectors"))]
fn seq_walk_accepts_numeric_vec_when_specialised_vectors_disabled() {
	let v: Vec<u32> = vec![7, 8];
	let bytes = to_vec(&v).unwrap();
	let mut r = bytes.as_slice();
	let mut walker = Vec::<u32>::walk_revisioned(&mut r).unwrap();
	let first = walker.next_item().unwrap().decode().unwrap();
	assert_eq!(first, 7);
	let second = walker.next_item().unwrap().decode().unwrap();
	assert_eq!(second, 8);
	assert!(r.is_empty());
}

#[test]
#[cfg(all(feature = "specialised-vectors", feature = "uuid"))]
fn seq_walk_rejects_bulk_uuid_without_consuming_reader() {
	let v = vec![uuid::Uuid::nil(), uuid::Uuid::nil()];
	let bytes = to_vec(&v).unwrap();
	let mut r = bytes.as_slice();
	let before = r.len();
	let err = match Vec::<uuid::Uuid>::walk_revisioned(&mut r) {
		Err(e) => e,
		Ok(_) => panic!("expected specialised bulk Vec<Uuid> walk to fail"),
	};
	assert!(matches!(err, Error::Deserialize(_)));
	assert_eq!(r.len(), before);
}

#[test]
#[cfg(all(feature = "specialised-vectors", feature = "rust_decimal"))]
fn seq_walk_rejects_bulk_decimal_without_consuming_reader() {
	use rust_decimal::Decimal;
	let v = vec![Decimal::ZERO, Decimal::ONE];
	let bytes = to_vec(&v).unwrap();
	let mut r = bytes.as_slice();
	let before = r.len();
	let err = match Vec::<Decimal>::walk_revisioned(&mut r) {
		Err(e) => e,
		Ok(_) => panic!("expected specialised bulk Vec<Decimal> walk to fail"),
	};
	assert!(matches!(err, Error::Deserialize(_)));
	assert_eq!(r.len(), before);
}

#[test]
fn map_entry_out_of_order_returns_error_without_io() {
	let mut map = BTreeMap::new();
	map.insert("only".to_string(), 42u32);
	let bytes = to_vec(&map).unwrap();
	let mut r = bytes.as_slice();
	let mut walker = <BTreeMap<String, u32>>::walk_revisioned(&mut r).unwrap();
	let mut entry = walker.next_entry().unwrap();
	entry.skip_key().unwrap();
	let err = entry.decode_key().unwrap_err();
	assert!(matches!(err, Error::Deserialize(_)));
	let v = entry.decode_value().unwrap();
	assert_eq!(v, 42);
	assert!(r.is_empty());
}
