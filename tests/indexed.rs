//! Hand-crafted byte-level tests for the indexed compound walkers.
//!
//! Build payloads directly, open walkers, exercise the access paths. These
//! tests stand alone — no derive macro, no surrounding envelope — so failures
//! point straight at the walker logic.

use std::cmp::Ordering;

use revision::optimised::indexed::seq_walk::FLAG_INDEXED;
use revision::optimised::{IndexedMapWalker, IndexedSeqWalker, IndexedStructWalker};
use revision::slice_reader::SliceReader;

fn varint(v: usize) -> Vec<u8> {
	match v {
		0..=250 => vec![v as u8],
		251..=65535 => {
			let mut out = vec![251u8];
			out.extend_from_slice(&(v as u16).to_le_bytes());
			out
		}
		_ => {
			let mut out = vec![252u8];
			out.extend_from_slice(&(v as u32).to_le_bytes());
			out
		}
	}
}

fn build_struct_payload(fields: &[&[u8]]) -> Vec<u8> {
	let n = fields.len();
	let prologue = n * 4;
	let mut offsets = Vec::with_capacity(n);
	let mut running = prologue as u32;
	for f in fields {
		offsets.push(running);
		running += f.len() as u32;
	}
	let mut out = Vec::with_capacity(running as usize);
	for o in &offsets {
		out.extend_from_slice(&o.to_le_bytes());
	}
	for f in fields {
		out.extend_from_slice(f);
	}
	out
}

fn build_indexed_seq(elements: &[&[u8]]) -> Vec<u8> {
	let n = elements.len();
	let mut out = vec![FLAG_INDEXED];
	out.extend_from_slice(&varint(n));
	let mut running = 0u32;
	for e in elements {
		out.extend_from_slice(&running.to_le_bytes());
		running += e.len() as u32;
	}
	for e in elements {
		out.extend_from_slice(e);
	}
	out
}

fn build_legacy_seq(legacy_body: &[u8], len: usize) -> Vec<u8> {
	let mut out = vec![0u8]; // flags = 0
	out.extend_from_slice(&varint(len));
	out.extend_from_slice(legacy_body);
	out
}

fn build_indexed_map(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
	let mut sorted: Vec<(&[u8], &[u8])> = entries.to_vec();
	sorted.sort_by(|a, b| a.0.cmp(b.0));
	let n = sorted.len();
	let mut out = vec![FLAG_INDEXED];
	out.extend_from_slice(&varint(n));
	let mut k_off = 0u32;
	let mut v_off = 0u32;
	let mut k_offsets = Vec::with_capacity(n);
	let mut v_offsets = Vec::with_capacity(n);
	for (k, v) in &sorted {
		k_offsets.push(k_off);
		v_offsets.push(v_off);
		k_off += k.len() as u32;
		v_off += v.len() as u32;
	}
	for i in 0..n {
		out.extend_from_slice(&k_offsets[i].to_le_bytes());
		out.extend_from_slice(&v_offsets[i].to_le_bytes());
	}
	// Region length pair
	out.extend_from_slice(&k_off.to_le_bytes());
	out.extend_from_slice(&v_off.to_le_bytes());
	// Dense keys
	for (k, _) in &sorted {
		out.extend_from_slice(k);
	}
	// Dense values
	for (_, v) in &sorted {
		out.extend_from_slice(v);
	}
	out
}

#[test]
fn struct_walker_accesses_all_fields_in_order() {
	let payload = build_struct_payload(&[b"alpha", b"bravo", b"charlie", b"delta"]);
	let w = IndexedStructWalker::<SliceReader>::from_payload(&payload, 2, 4).unwrap();
	for (i, expected) in [b"alpha".as_slice(), b"bravo", b"charlie", b"delta"].iter().enumerate() {
		assert_eq!(w.field_bytes(i as u16).unwrap(), *expected);
	}
}

#[test]
fn struct_walker_skip_is_free_constant_time() {
	let payload = build_struct_payload(&[b"a", b"b", b"c"]);
	let w = IndexedStructWalker::<SliceReader>::from_payload(&payload, 1, 3).unwrap();
	for i in 0..3 {
		w.skip_field(i).unwrap();
	}
}

#[test]
fn struct_walker_handles_field_count_at_threshold_boundary() {
	let fields: Vec<&[u8]> =
		(0..revision::optimised::OFFSET_TABLE_MIN_LEN).map(|_| b"x".as_slice()).collect();
	let payload = build_struct_payload(&fields);
	let w =
		IndexedStructWalker::<SliceReader>::from_payload(&payload, 2, fields.len() as u16).unwrap();
	assert_eq!(w.field_count() as usize, revision::optimised::OFFSET_TABLE_MIN_LEN);
	assert_eq!(w.field_bytes(0).unwrap(), b"x");
	assert_eq!(
		w.field_bytes((revision::optimised::OFFSET_TABLE_MIN_LEN - 1) as u16).unwrap(),
		b"x"
	);
}

#[test]
fn seq_walker_indexed_path_random_access() {
	let payload = build_indexed_seq(&[b"one", b"two-two", b"three-three-three"]);
	let w: IndexedSeqWalker<()> = IndexedSeqWalker::from_payload(&payload).unwrap();
	assert!(w.is_indexed());
	assert_eq!(w.element_bytes(0).unwrap(), b"one");
	assert_eq!(w.element_bytes(2).unwrap(), b"three-three-three");
}

#[test]
fn seq_walker_legacy_path_returns_body() {
	let payload = build_legacy_seq(b"\x01\x02\x03\x04", 4);
	let w: IndexedSeqWalker<()> = IndexedSeqWalker::from_payload(&payload).unwrap();
	assert!(!w.is_indexed());
	assert_eq!(w.body(), &[1u8, 2, 3, 4]);
}

#[test]
fn map_walker_indexed_path_binary_search_finds_every_key() {
	let entries: Vec<(&[u8], &[u8])> = vec![
		(b"alpha", b"1"),
		(b"bravo", b"2"),
		(b"charlie", b"3"),
		(b"delta", b"4"),
		(b"echo", b"5"),
		(b"foxtrot", b"6"),
		(b"golf", b"7"),
		(b"hotel", b"8"),
	];
	let payload = build_indexed_map(&entries);
	let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
	for (k, v) in &entries {
		let value = w.find_value_bytes(|key| key.cmp(*k)).unwrap();
		assert_eq!(value.unwrap(), *v, "key {:?}", std::str::from_utf8(k).unwrap());
	}
}

#[test]
fn map_walker_returns_none_for_absent_key() {
	let payload = build_indexed_map(&[(b"a", b"1"), (b"b", b"2"), (b"c", b"3")]);
	let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
	assert!(w.find_value_bytes(|k| k.cmp(b"zz".as_slice())).unwrap().is_none());
}

#[test]
fn map_walker_iter_visits_pairs_in_ascending_order() {
	let payload = build_indexed_map(&[(b"c", b"3"), (b"a", b"1"), (b"b", b"2")]);
	let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
	let collected: Vec<(&[u8], &[u8])> = w.entries().unwrap().collect();
	assert_eq!(
		collected,
		vec![
			(b"a".as_slice(), b"1".as_slice()),
			(b"b".as_slice(), b"2".as_slice()),
			(b"c".as_slice(), b"3".as_slice()),
		]
	);
}

#[test]
fn map_walker_find_compares_via_user_predicate() {
	// Build a map with non-string keys (4-byte little-endian integers).
	let entries: Vec<(Vec<u8>, &[u8])> =
		(0u32..5).map(|i| (i.to_le_bytes().to_vec(), b"v".as_slice())).collect();
	let refs: Vec<(&[u8], &[u8])> = entries.iter().map(|(k, v)| (k.as_slice(), *v)).collect();
	let payload = build_indexed_map(&refs);
	let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
	let target = 3u32.to_le_bytes();
	let value = w.find_value_bytes(|key| key.cmp(&target[..])).unwrap();
	assert_eq!(value.unwrap(), b"v");
}

#[test]
fn empty_indexed_seq_is_handled() {
	let payload = build_indexed_seq(&[]);
	let w: IndexedSeqWalker<()> = IndexedSeqWalker::from_payload(&payload).unwrap();
	assert_eq!(w.len(), 0);
	assert!(w.is_empty());
}

#[test]
fn predicate_matching_via_partial_ordering_works() {
	// Ensure find correctly handles Less/Greater navigation by checking a key
	// that requires multiple binary-search steps to find.
	let keys: Vec<Vec<u8>> = (b'a'..=b'p').map(|c| vec![c]).collect();
	let entries: Vec<(&[u8], &[u8])> =
		keys.iter().map(|k| (k.as_slice(), b"x".as_slice())).collect();
	let payload = build_indexed_map(&entries);
	let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
	let value = w
		.find_value_bytes(|k| match k.first() {
			Some(&c) => c.cmp(&b'j'),
			None => Ordering::Less,
		})
		.unwrap();
	assert_eq!(value.unwrap(), b"x", "should find j among a-p");
}
