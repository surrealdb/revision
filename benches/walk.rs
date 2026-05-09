//! Compare walker-based selective decoding against full
//! [`DeserializeRevisioned`] across three modes:
//!
//! - **Current-rev hot path**: walker reads the latest revision and decodes
//!   one targeted entry from a 128-entry map. The perf gate.
//! - **Older-rev wire path**: walker reads an older (additive) revision and
//!   exercises rev-aware branching. Should remain allocation-free.
//! - **Older-rev materialised path**: walker handles a `convert_fn`-bearing
//!   type at an older revision; this triggers internal materialisation
//!   (`deserialize` + `serialize`). Documented as the slow path.

use std::collections::BTreeMap;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use revision::{
	DeserializeRevisioned, MapWalker, SerializeRevisioned, WalkRevisioned, revisioned, to_vec,
};

// -----------------------------------------------------------------------------
// Fixture: SurrealDB-doc-shaped payload (current rev = 1, no convert_fn).
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
#[derive(Debug, Clone)]
struct Doc {
	table: BTreeMap<String, Payload>,
}

#[revisioned(revision = 1)]
#[derive(Debug, Clone)]
enum Payload {
	Small(Vec<u8>),
	Big(Wide),
}

#[revisioned(revision = 1)]
#[derive(Debug, Clone)]
struct Wide {
	filler: Vec<String>,
	target: i64,
}

const ENTRY_COUNT: usize = 128;
const SMALL_BLOB_LEN: usize = 192;
const TARGET_KEY: &str = "9999999999";

fn build_doc_payload() -> Vec<u8> {
	let mut table = BTreeMap::new();
	for i in 0..ENTRY_COUNT {
		table.insert(format!("{i:010}"), Payload::Small(vec![0x5e_u8; SMALL_BLOB_LEN]));
	}
	let wide = Wide {
		filler: vec!["payload".repeat(8); 32],
		target: -0x7080_9070_a0b1_c2d3,
	};
	table.insert(TARGET_KEY.into(), Payload::Big(wide));
	to_vec(&Doc {
		table,
	})
	.unwrap()
}

// -----------------------------------------------------------------------------
// Fixture: cross-rev additive (no convert_fn). Wire is rev 1; schema rev 2
// adds a defaulted field.
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
#[derive(Debug, Clone)]
struct AdditiveV1 {
	a: u32,
}

#[revisioned(revision = 2)]
#[derive(Debug, Clone)]
struct Additive {
	a: u32,
	#[revision(start = 2)]
	b: u32,
}

fn build_additive_v1_payload() -> Vec<u8> {
	to_vec(&AdditiveV1 {
		a: 42,
	})
	.unwrap()
}

// -----------------------------------------------------------------------------
// Fixture: cross-rev with convert_fn (forces materialised mode for older rev).
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
#[derive(Debug, Clone)]
struct ConvertedV1 {
	width: u32,
}

#[revisioned(revision = 2)]
#[derive(Debug, Clone)]
struct Converted {
	#[revision(end = 2, convert_fn = "convert_width")]
	width_old: u32,
	#[revision(start = 2)]
	width: u32,
	#[revision(start = 2)]
	height: u32,
}

impl Converted {
	fn convert_width(&mut self, _rev: u16, value: u32) -> Result<(), revision::Error> {
		self.width = value * 10;
		self.height = value + 1;
		Ok(())
	}
}

fn build_converted_v1_payload() -> Vec<u8> {
	to_vec(&ConvertedV1 {
		width: 5,
	})
	.unwrap()
}

// -----------------------------------------------------------------------------
// Benches
// -----------------------------------------------------------------------------

fn bench_extract_via_deserialize(c: &mut Criterion) {
	let bytes = build_doc_payload();
	c.bench_function("doc_extract_via_full_deserialize", |b| {
		b.iter(|| {
			let mut r = bytes.as_slice();
			let doc = Doc::deserialize_revisioned(&mut r).unwrap();
			black_box(&doc);
		})
	});
}

fn bench_extract_via_walker(c: &mut Criterion) {
	let bytes = build_doc_payload();
	c.bench_function("doc_extract_target_via_walker", |b| {
		b.iter(|| {
			let mut r = bytes.as_slice();
			let doc_walker = Doc::walk_revisioned(&mut r).unwrap();
			let map: MapWalker<String, Payload, _> = doc_walker.walk_table().unwrap();
			let handle =
				map.find(|k: &String| k.as_str().cmp(TARGET_KEY)).unwrap().expect("target key");
			let payload = handle.decode().unwrap();
			black_box(&payload);
		})
	});
}

fn bench_walker_older_rev_additive(c: &mut Criterion) {
	let bytes = build_additive_v1_payload();
	c.bench_function("additive_walker_older_wire_rev", |b| {
		b.iter(|| {
			let mut r = bytes.as_slice();
			let mut walker = Additive::walk_revisioned(&mut r).unwrap();
			// `a` exists at rev 1 → read from wire.
			let a = walker.decode_a().unwrap();
			// `b` was added at rev 2 → defaulted on rev-1 wire.
			let b = walker.decode_b().unwrap();
			black_box((a, b));
		})
	});
}

fn bench_walker_older_rev_convert_fn(c: &mut Criterion) {
	let bytes = build_converted_v1_payload();
	c.bench_function("convert_fn_walker_older_wire_rev", |b| {
		b.iter(|| {
			let mut r = bytes.as_slice();
			// `Converted` has a `convert_fn`; rev-1 wire forces the
			// materialised path inside `walk_revisioned`.
			let mut walker = Converted::walk_revisioned(&mut r).unwrap();
			let w = walker.decode_width().unwrap();
			let h = walker.decode_height().unwrap();
			black_box((w, h));
		})
	});
}

/// Reference: deserialize + re-serialize cost on the same convert_fn input,
/// approximating the materialised path's intrinsic cost.
fn bench_deserialize_plus_reserialize(c: &mut Criterion) {
	let bytes = build_converted_v1_payload();
	c.bench_function("convert_fn_deserialize_then_serialize_baseline", |b| {
		b.iter(|| {
			let mut r = bytes.as_slice();
			let value = Converted::deserialize_revisioned(&mut r).unwrap();
			let mut buf = Vec::new();
			value.serialize_revisioned(&mut buf).unwrap();
			black_box(&buf);
		})
	});
}

// -----------------------------------------------------------------------------
// Nested fused-flat: scope an N-key scan under a navigation prefix.
//
// Models the SurrealDB shape `WHERE outer.a = ? AND outer.b = ? AND ...`
// where `outer` is a nested `BTreeMap<String, T>`. Compares two pipelines:
//
//   - **Materialise + scan**: deserialize the navigated `BTreeMap` fully,
//     then look up each needle key in the in-memory map. This is the
//     pre-fix behaviour from the prior commit.
//   - **Walker streaming scan**: walk into the navigated map and stream
//     entries; only matched values are decoded. This is the post-fix
//     behaviour that the SurrealDB pre-decode filter now uses.
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
#[derive(Debug, Clone)]
struct NestedDoc {
	outer: BTreeMap<String, NestedPayload>,
}

#[revisioned(revision = 1)]
#[derive(Debug, Clone)]
struct NestedPayload {
	score: i64,
	tag: String,
}

const NESTED_ENTRY_COUNT: usize = 64;
const NESTED_TAG_LEN: usize = 24;
const NESTED_NEEDLES: &[&str] = &["alpha", "delta", "omega", "zeta"];

fn build_nested_payload() -> Vec<u8> {
	let mut outer = BTreeMap::new();
	for i in 0..NESTED_ENTRY_COUNT {
		outer.insert(
			format!("k{i:04}"),
			NestedPayload {
				score: i as i64,
				tag: "x".repeat(NESTED_TAG_LEN),
			},
		);
	}
	for (i, n) in NESTED_NEEDLES.iter().enumerate() {
		outer.insert(
			(*n).to_string(),
			NestedPayload {
				score: 1000 + i as i64,
				tag: "match".to_string(),
			},
		);
	}
	to_vec(&NestedDoc {
		outer,
	})
	.unwrap()
}

fn bench_nested_fused_via_materialise(c: &mut Criterion) {
	let bytes = build_nested_payload();
	c.bench_function("nested_fused_materialise_then_scan", |b| {
		b.iter(|| {
			// Old shape: deserialize the navigated map, then look up
			// each needle key in the in-memory map and read its value.
			let mut r = bytes.as_slice();
			let doc = NestedDoc::deserialize_revisioned(&mut r).unwrap();
			let mut hits: Vec<Option<i64>> = Vec::with_capacity(NESTED_NEEDLES.len());
			for needle in NESTED_NEEDLES {
				let v = doc.outer.get(*needle).map(|p| p.score);
				hits.push(v);
			}
			black_box(&hits);
		})
	});
}

fn bench_nested_fused_via_walker(c: &mut Criterion) {
	let bytes = build_nested_payload();
	c.bench_function("nested_fused_walker_streaming_scan", |b| {
		b.iter(|| {
			// New shape: walk into the navigated map, stream entries,
			// decode only matched values.
			let mut r = bytes.as_slice();
			let doc_walker = NestedDoc::walk_revisioned(&mut r).unwrap();
			let mut map: MapWalker<String, NestedPayload, _> = doc_walker.walk_outer().unwrap();
			let mut needles_iter = NESTED_NEEDLES.iter().peekable();
			let mut hits: Vec<Option<i64>> = vec![None; NESTED_NEEDLES.len()];
			let mut idx = 0usize;
			while let Some(mut entry) = map.next_entry() {
				let key = entry.decode_key().unwrap();
				while let Some(&&n) = needles_iter.peek() {
					if n < key.as_str() {
						idx += 1;
						needles_iter.next();
					} else {
						break;
					}
				}
				if needles_iter.peek().is_some_and(|&&n| n == key.as_str()) {
					let payload = entry.decode_value().unwrap();
					hits[idx] = Some(payload.score);
					idx += 1;
					needles_iter.next();
				} else {
					entry.skip_value().unwrap();
				}
				if needles_iter.peek().is_none() {
					break;
				}
			}
			black_box(&hits);
		})
	});
}

criterion_group!(
	walk_bench,
	bench_extract_via_deserialize,
	bench_extract_via_walker,
	bench_walker_older_rev_additive,
	bench_walker_older_rev_convert_fn,
	bench_deserialize_plus_reserialize,
	bench_nested_fused_via_materialise,
	bench_nested_fused_via_walker,
);
criterion_main!(walk_bench);
