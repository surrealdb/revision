//! Regression coverage for the O(1) / O(1 element) skip fast path on indexed
//! bodies (see `skip_indexed_map`, `skip_indexed_seq`, `skip_indexed_set` in
//! `src/optimised/indexed/serialize.rs`).
//!
//! Each test verifies two things:
//!
//! 1. After `skip_*` the reader's cursor sits exactly at the end of the
//!    encoded payload (no over- or under-consume).
//! 2. The behaviour is identical on the legacy (sub-threshold) and indexed
//!    (≥ OFFSET_TABLE_MIN_LEN) shapes — the encoder picks one based on length,
//!    the reader must handle both.

use std::collections::{BTreeMap, BTreeSet};

use revision::optimised::indexed::{
	IndexedMapEncoded, IndexedSeqEncoded, IndexedSetEncoded, deserialize_indexed_map,
	deserialize_indexed_seq, deserialize_indexed_set, serialize_indexed_map, serialize_indexed_seq,
	serialize_indexed_set_iter, skip_indexed_map, skip_indexed_seq, skip_indexed_set,
};

/// Build the indexed-map payload, skip it, assert the cursor is at EOF and a
/// round-trip via deserialize gives the same value.
fn round_trip_map<K, V>(map: BTreeMap<K, V>)
where
	K: revision::SerializeRevisioned
		+ revision::DeserializeRevisioned
		+ revision::SkipRevisioned
		+ Ord
		+ Clone
		+ std::fmt::Debug
		+ Eq,
	V: revision::SerializeRevisioned
		+ revision::DeserializeRevisioned
		+ revision::SkipRevisioned
		+ Clone
		+ std::fmt::Debug
		+ Eq,
{
	let mut bytes = Vec::new();
	serialize_indexed_map(&map, &mut bytes).unwrap();

	// 1. Skip path must end at EOF.
	let mut r: &[u8] = &bytes;
	skip_indexed_map::<K, V, _>(&mut r).unwrap();
	assert!(r.is_empty(), "skip_indexed_map left {} bytes ({} entries)", r.len(), map.len(),);

	// 2. Deserialize must still round-trip.
	let mut r: &[u8] = &bytes;
	let decoded: BTreeMap<K, V> = deserialize_indexed_map(&mut r).unwrap();
	assert!(r.is_empty(), "deserialize_indexed_map left bytes");
	assert_eq!(decoded, map);
}

fn round_trip_seq<T>(items: Vec<T>)
where
	T: revision::SerializeRevisioned
		+ revision::DeserializeRevisioned
		+ revision::SkipRevisioned
		+ Clone
		+ std::fmt::Debug
		+ Eq,
{
	let mut bytes = Vec::new();
	serialize_indexed_seq(&items, &mut bytes).unwrap();

	let mut r: &[u8] = &bytes;
	skip_indexed_seq::<T, _>(&mut r).unwrap();
	assert!(r.is_empty(), "skip_indexed_seq left {} bytes ({} elements)", r.len(), items.len(),);

	let mut r: &[u8] = &bytes;
	let decoded: Vec<T> = deserialize_indexed_seq(&mut r).unwrap();
	assert!(r.is_empty(), "deserialize_indexed_seq left bytes");
	assert_eq!(decoded, items);
}

// -----------------------------------------------------------------------------
// Map
// -----------------------------------------------------------------------------

#[test]
fn skip_indexed_map_legacy_body_string_to_u64() {
	// 3 entries < OFFSET_TABLE_MIN_LEN(=8) → legacy `(K,V)*` shape.
	let mut m = BTreeMap::new();
	m.insert("alpha".to_string(), 1u64);
	m.insert("bravo".to_string(), 2u64);
	m.insert("charlie".to_string(), 3u64);
	round_trip_map(m);
}

#[test]
fn skip_indexed_map_indexed_body_fixed_keys_fixed_values() {
	// 16 entries ≥ threshold → indexed shape (offset table + region lengths).
	let mut m = BTreeMap::new();
	for i in 0u32..16 {
		m.insert(i, (i as i64) * -7 + 1);
	}
	round_trip_map(m);
}

#[test]
fn skip_indexed_map_indexed_body_variable_length_keys_and_values() {
	// Indexed body with String keys and String values of varying lengths
	// — exercises the dense-region-length path against multi-byte varints in
	// the entry payloads.
	let mut m = BTreeMap::new();
	for i in 0..12 {
		let key = format!("k{}-{}", i, "x".repeat(i));
		let val = format!("v{}-{}", i, "y".repeat(i * 2));
		m.insert(key, val);
	}
	round_trip_map(m);
}

#[test]
fn skip_indexed_map_empty() {
	let m: BTreeMap<String, u32> = BTreeMap::new();
	round_trip_map(m);
}

#[test]
fn skip_indexed_map_just_above_threshold() {
	// At exactly OFFSET_TABLE_MIN_LEN the indexed path engages; check the
	// boundary explicitly.
	let mut m = BTreeMap::new();
	for i in 0u32..8 {
		m.insert(format!("key-{i:02}"), i);
	}
	round_trip_map(m);
}

#[test]
fn skip_indexed_map_just_below_threshold() {
	// 7 < OFFSET_TABLE_MIN_LEN: legacy body.
	let mut m = BTreeMap::new();
	for i in 0u32..7 {
		m.insert(format!("key-{i:02}"), i);
	}
	round_trip_map(m);
}

// -----------------------------------------------------------------------------
// Seq
// -----------------------------------------------------------------------------

#[test]
fn skip_indexed_seq_legacy_body() {
	// 3 < threshold → legacy shape.
	round_trip_seq::<u32>(vec![10, 20, 30]);
}

#[test]
fn skip_indexed_seq_indexed_body_primitives() {
	let v: Vec<u64> = (0u64..32).map(|i| i * 0xDEAD_BEEF).collect();
	round_trip_seq(v);
}

#[test]
fn skip_indexed_seq_indexed_body_variable_length_elements() {
	// String elements of varying lengths — exercises the "skip the last
	// element via T::skip_revisioned" branch with a non-trivial element body.
	let v: Vec<String> =
		(0..16).map(|i| format!("element-{i}-{}", "padding".repeat(i % 4))).collect();
	round_trip_seq(v);
}

#[test]
fn skip_indexed_seq_at_threshold() {
	let v: Vec<u32> = (0u32..8).collect();
	round_trip_seq(v);
}

#[test]
fn skip_indexed_seq_just_below_threshold() {
	let v: Vec<u32> = (0u32..7).collect();
	round_trip_seq(v);
}

#[test]
fn skip_indexed_seq_empty() {
	let v: Vec<u32> = Vec::new();
	round_trip_seq(v);
}

// -----------------------------------------------------------------------------
// Set (shares wire format with seq)
// -----------------------------------------------------------------------------

#[test]
fn skip_indexed_set_indexed_body_primitives() {
	let s: BTreeSet<i64> = (0i64..24).map(|i| i * 13 - 7).collect();
	let mut bytes = Vec::new();
	serialize_indexed_set_iter(s.iter(), &mut bytes).unwrap();

	let mut r: &[u8] = &bytes;
	skip_indexed_set::<i64, _>(&mut r).unwrap();
	assert!(r.is_empty(), "skip_indexed_set left {} bytes", r.len());

	let mut r: &[u8] = &bytes;
	let decoded: BTreeSet<i64> = deserialize_indexed_set(&mut r).unwrap();
	assert!(r.is_empty());
	assert_eq!(decoded, s);
}

#[test]
fn skip_indexed_set_below_threshold() {
	let s: BTreeSet<i64> = (0i64..4).collect();
	let mut bytes = Vec::new();
	serialize_indexed_set_iter(s.iter(), &mut bytes).unwrap();

	let mut r: &[u8] = &bytes;
	skip_indexed_set::<i64, _>(&mut r).unwrap();
	assert!(r.is_empty());
}

// -----------------------------------------------------------------------------
// Sibling-field framing
// -----------------------------------------------------------------------------

/// `skip_indexed_*` must consume *exactly* the indexed payload — neither
/// peeking into the following bytes nor leaving any trailing dense bytes
/// untouched. We verify by following the skip with a sibling read.
#[test]
fn skip_indexed_map_leaves_sibling_bytes_intact() {
	let mut m = BTreeMap::new();
	for i in 0u32..12 {
		m.insert(format!("k-{i:03}"), i as i64 * 17);
	}
	let mut bytes = Vec::new();
	serialize_indexed_map(&m, &mut bytes).unwrap();
	// Append a trailing "sibling" payload — a literal `u32_le` sentinel.
	let sentinel: u32 = 0xCAFE_BABE;
	bytes.extend_from_slice(&sentinel.to_le_bytes());

	let mut r: &[u8] = &bytes;
	skip_indexed_map::<String, i64, _>(&mut r).unwrap();
	assert_eq!(r.len(), 4, "skip should leave exactly 4 trailing bytes");
	assert_eq!(u32::from_le_bytes(r.try_into().unwrap()), sentinel);
}

#[test]
fn skip_indexed_seq_leaves_sibling_bytes_intact() {
	let v: Vec<String> = (0..16).map(|i| format!("seq-{i:02}-{}", "p".repeat(i))).collect();
	let mut bytes = Vec::new();
	serialize_indexed_seq(&v, &mut bytes).unwrap();
	let sentinel: u32 = 0x1234_5678;
	bytes.extend_from_slice(&sentinel.to_le_bytes());

	let mut r: &[u8] = &bytes;
	skip_indexed_seq::<String, _>(&mut r).unwrap();
	assert_eq!(r.len(), 4, "skip should leave exactly 4 trailing bytes");
	assert_eq!(u32::from_le_bytes(r.try_into().unwrap()), sentinel);
}

// -----------------------------------------------------------------------------
// Cross-check: skip via trait method matches skip via free function
// -----------------------------------------------------------------------------

#[test]
fn skip_indexed_map_via_trait_method_matches_free_function() {
	let mut m = BTreeMap::new();
	for i in 0u32..10 {
		m.insert(format!("kkk-{i}"), i as u64);
	}
	let mut bytes = Vec::new();
	<BTreeMap<String, u64> as IndexedMapEncoded>::serialize_indexed_map(&m, &mut bytes).unwrap();

	let mut r_free: &[u8] = &bytes;
	skip_indexed_map::<String, u64, _>(&mut r_free).unwrap();
	let consumed_free = bytes.len() - r_free.len();

	let mut r_trait: &[u8] = &bytes;
	<BTreeMap<String, u64> as IndexedMapEncoded>::skip_indexed_map(&mut r_trait).unwrap();
	let consumed_trait = bytes.len() - r_trait.len();

	assert_eq!(consumed_free, consumed_trait);
	assert_eq!(consumed_free, bytes.len());
}

#[test]
fn skip_indexed_seq_via_trait_method_matches_free_function() {
	let v: Vec<i32> = (0i32..16).map(|i| i * -5).collect();
	let mut bytes = Vec::new();
	<Vec<i32> as IndexedSeqEncoded>::serialize_indexed_seq(&v, &mut bytes).unwrap();

	let mut r_free: &[u8] = &bytes;
	skip_indexed_seq::<i32, _>(&mut r_free).unwrap();
	let consumed_free = bytes.len() - r_free.len();

	let mut r_trait: &[u8] = &bytes;
	<Vec<i32> as IndexedSeqEncoded>::skip_indexed_seq(&mut r_trait).unwrap();
	let consumed_trait = bytes.len() - r_trait.len();

	assert_eq!(consumed_free, consumed_trait);
	assert_eq!(consumed_free, bytes.len());
}

#[test]
fn skip_indexed_set_via_trait_method_matches_free_function() {
	let s: BTreeSet<String> = (0..10).map(|i| format!("set-{i:02}")).collect();
	let mut bytes = Vec::new();
	<BTreeSet<String> as IndexedSetEncoded>::serialize_indexed_set(&s, &mut bytes).unwrap();

	let mut r_free: &[u8] = &bytes;
	skip_indexed_set::<String, _>(&mut r_free).unwrap();
	let consumed_free = bytes.len() - r_free.len();

	let mut r_trait: &[u8] = &bytes;
	<BTreeSet<String> as IndexedSetEncoded>::skip_indexed_set(&mut r_trait).unwrap();
	let consumed_trait = bytes.len() - r_trait.len();

	assert_eq!(consumed_free, consumed_trait);
	assert_eq!(consumed_free, bytes.len());
}
