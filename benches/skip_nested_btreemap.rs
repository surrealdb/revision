//! Nested **`BTreeMap<String, …>`** branch: many `Small` payloads, then one `Big(MidNode)` under a distant key.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use revision::{DeserializeRevisioned, Error, SkipRevisioned, revisioned, to_vec};
use std::collections::BTreeMap;
use std::hint::black_box;

#[revisioned(revision = 1)]
#[derive(Debug)]
struct DeepLeaf {
	filler: Vec<u8>,
	target: i64,
}

#[revisioned(revision = 1)]
#[derive(Debug)]
struct MidNode {
	filler: Vec<String>,
	child: DeepLeaf,
}

/// Middle tier: separate allocated strings (~64 × ~1 KiB).
const MID_STRING_COUNT: usize = 64;
const MID_STRING_BODY: usize = 1024;
/// Leaf tier before the `i64` (~256 KiB).
const LEAF_FILL_LEN: usize = 256 * 1024;

fn build_mid_node(target: i64) -> MidNode {
	let leaf_body = MID_STRING_BODY.saturating_sub(32);
	let mid_strings: Vec<String> =
		(0..MID_STRING_COUNT).map(|i| format!("mid-{i}|{}", "x".repeat(leaf_body))).collect();
	MidNode {
		filler: mid_strings,
		child: DeepLeaf {
			filler: vec![0xF0; LEAF_FILL_LEN],
			target,
		},
	}
}

#[revisioned(revision = 1)]
#[derive(Debug)]
enum MapValueBench {
	Small(Vec<u8>),
	Big(MidNode),
}

#[revisioned(revision = 1)]
#[derive(Debug)]
struct MapBenchRoot {
	table: BTreeMap<String, MapValueBench>,
}

/// Small keys `format!("{:010}", 0 .. SMALL_MAP_ENTRIES-1)`; target must sort after all of those.
const SMALL_MAP_ENTRIES: usize = 128;
/// Payload size for each `Small` blob.
const SMALL_BLOB_LEN: usize = 192;
const MAP_TARGET_KEY: &str = "9999999999";

fn build_map_payload(target: i64) -> MapBenchRoot {
	let mid = build_mid_node(target);
	let mut table = BTreeMap::new();
	for i in 0..SMALL_MAP_ENTRIES {
		table.insert(format!("{i:010}"), MapValueBench::Small(vec![0x5E_u8; SMALL_BLOB_LEN]));
	}
	table.insert(MAP_TARGET_KEY.into(), MapValueBench::Big(mid));
	MapBenchRoot {
		table,
	}
}

/// Walk map entries in wire order until `MAP_TARGET_KEY`, skipping each value via [`SkipRevisioned`].
/// After the target key: read **`u16`** type revision then **`u32`** variant discriminator for `Big`,
/// then deserialize [`MidNode`] (revisioned enums always prefix payload with type revision).
fn extract_deep_target_via_btreemap_skip(mut reader: &[u8]) -> Result<i64, Error> {
	let _root_revision = u16::deserialize_revisioned(&mut reader)?;
	let len = usize::deserialize_revisioned(&mut reader)?;
	for _ in 0..len {
		let k = String::deserialize_revisioned(&mut reader)?;
		if k == MAP_TARGET_KEY {
			let _map_value_revision = u16::deserialize_revisioned(&mut reader)?;
			let big_disc = u32::deserialize_revisioned(&mut reader)?;
			debug_assert_eq!(big_disc, 1, "MapValueBench::Big discriminant");
			let mid = MidNode::deserialize_revisioned(&mut reader)?;
			return Ok(mid.child.target);
		}
		<MapValueBench as SkipRevisioned>::skip_revisioned(&mut reader)?;
	}
	Err(Error::Deserialize("benchmark BTreeMap missing target entry".into()))
}

fn nested_deep_i64_btreemap_benches(c: &mut Criterion) {
	let expected = -0x7080_9070_a0b1_c2d3_i64;
	let payload = build_map_payload(expected);
	let bytes = to_vec(&payload).unwrap();

	assert_eq!(extract_deep_target_via_btreemap_skip(bytes.as_slice()).unwrap(), expected);

	let mut root_slice = bytes.as_slice();
	let full = MapBenchRoot::deserialize_revisioned(&mut root_slice).unwrap();
	let hit = match full.table.get(MAP_TARGET_KEY).expect("missing key") {
		MapValueBench::Big(mid) => mid.child.target,
		MapValueBench::Small(_) => panic!("unexpected Small at target key"),
	};
	assert_eq!(hit, expected);

	let mut grp = c.benchmark_group("nested_deep_i64_btreemap");
	grp.throughput(Throughput::Bytes(bytes.len() as u64));

	grp.bench_function(BenchmarkId::from_parameter("deserialize_full_then_field"), |b| {
		b.iter(|| {
			let doc =
				MapBenchRoot::deserialize_revisioned(&mut black_box(bytes.as_slice())).unwrap();
			let v = match doc.table.get(MAP_TARGET_KEY).expect("missing key") {
				MapValueBench::Big(mid) => mid.child.target,
				MapValueBench::Small(_) => panic!("unexpected variant"),
			};
			black_box(v)
		});
	});

	grp.bench_function(BenchmarkId::from_parameter("skip_prefixes_then_deser_i64"), |b| {
		b.iter(|| {
			let v = extract_deep_target_via_btreemap_skip(black_box(bytes.as_slice())).unwrap();
			black_box(v)
		});
	});

	grp.finish();
}

criterion_group!(benches, nested_deep_i64_btreemap_benches);
criterion_main!(benches);
