//! Nested **struct** document: deep `Vec<u8>` / `Vec<String>` fill, then probe `nested.child.target`.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use revision::{DeserializeRevisioned, Error, SkipRevisioned, revisioned, to_vec};
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

/// After a nested `MidNode`’s revision `u16` on the wire, skip to leaf `target`.
fn extract_i64_after_mid_revision_prefix(mut reader: &[u8]) -> Result<i64, Error> {
	let _mid_revision = u16::deserialize_revisioned(&mut reader)?;
	<Vec<String> as SkipRevisioned>::skip_revisioned(&mut reader)?;
	let _leaf_revision = u16::deserialize_revisioned(&mut reader)?;
	<Vec<u8> as SkipRevisioned>::skip_revisioned(&mut reader)?;
	i64::deserialize_revisioned(&mut reader)
}

#[revisioned(revision = 1)]
#[derive(Debug)]
struct RootDoc {
	filler: Vec<u8>,
	nested: MidNode,
}

/// Root tier: bulk opaque bytes (~512 KiB).
const ROOT_FILL_LEN: usize = 512 * 1024;

fn build_deep_payload(target: i64) -> RootDoc {
	RootDoc {
		filler: vec![0xAE; ROOT_FILL_LEN],
		nested: build_mid_node(target),
	}
}

/// Struct root: revision, skip root `Vec<u8>`, then mid subtree.
fn extract_deep_target_via_skip(mut reader: &[u8]) -> Result<i64, Error> {
	let _root_revision = u16::deserialize_revisioned(&mut reader)?;
	<Vec<u8> as SkipRevisioned>::skip_revisioned(&mut reader)?;
	extract_i64_after_mid_revision_prefix(reader)
}

fn nested_deep_i64_struct_benches(c: &mut Criterion) {
	let expected = -0x7080_9070_a0b1_c2d3_i64;
	let payload = build_deep_payload(expected);
	let bytes = to_vec(&payload).unwrap();

	let full = RootDoc::deserialize_revisioned(&mut black_box(bytes.as_slice())).unwrap();
	assert_eq!(full.nested.child.target, expected);
	assert_eq!(extract_deep_target_via_skip(bytes.as_slice()).unwrap(), expected);

	let mut grp = c.benchmark_group("nested_deep_i64_struct");
	grp.throughput(Throughput::Bytes(bytes.len() as u64));

	grp.bench_function(BenchmarkId::from_parameter("deserialize_full_then_field"), |b| {
		b.iter(|| {
			let doc = RootDoc::deserialize_revisioned(&mut black_box(bytes.as_slice())).unwrap();
			black_box(doc.nested.child.target)
		});
	});

	grp.bench_function(BenchmarkId::from_parameter("skip_prefixes_then_deser_i64"), |b| {
		b.iter(|| {
			let v = extract_deep_target_via_skip(black_box(bytes.as_slice())).unwrap();
			black_box(v)
		});
	});

	grp.finish();
}

criterion_group!(benches, nested_deep_i64_struct_benches);
criterion_main!(benches);
