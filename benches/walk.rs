//! Compare walker-based selective decoding against full
//! [`DeserializeRevisioned`] for the SurrealDB-doc shape (length-prefixed
//! map of String keys with mixed-size payloads).

use std::collections::BTreeMap;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use revision::{DeserializeRevisioned, MapWalker, WalkRevisioned, revisioned, to_vec};

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

fn build_payload() -> Vec<u8> {
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

fn bench_extract_via_deserialize(c: &mut Criterion) {
	let bytes = build_payload();
	c.bench_function("doc_extract_via_full_deserialize", |b| {
		b.iter(|| {
			let mut r = bytes.as_slice();
			let doc = Doc::deserialize_revisioned(&mut r).unwrap();
			black_box(&doc);
		})
	});
}

fn bench_extract_via_walker(c: &mut Criterion) {
	let bytes = build_payload();
	c.bench_function("doc_extract_target_via_walker", |b| {
		b.iter(|| {
			let mut r = bytes.as_slice();
			let mut doc_walker = Doc::walk_revisioned(&mut r).unwrap();
			// Doc has a single field `table: BTreeMap<String, Payload>`
			// Walk into it as a map, find the target key, decode that one
			// payload.
			let map: MapWalker<String, Payload, _> =
				doc_walker.walk::<BTreeMap<String, Payload>>().unwrap();
			let handle = map
				.find(|k: &String| k.as_str().cmp(TARGET_KEY))
				.unwrap()
				.expect("target key");
			let payload = handle.decode().unwrap();
			black_box(&payload);
		})
	});
}

criterion_group!(walk_bench, bench_extract_via_deserialize, bench_extract_via_walker);
criterion_main!(walk_bench);
