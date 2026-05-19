//! Deeply-nested struct benchmarks: where optimised skip's O(1) win is visible.
//!
//! Pairs of depth-1/3/5 nested structs, one legacy and one optimised. Skip
//! cost under legacy scales with the byte count of the nested payload;
//! under optimised it's a single u32_le read + advance regardless of depth.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use revision::prelude::*;

// -----------------------------------------------------------------------------
// Depth 1 (8 u64 fields, ~80 bytes)
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
struct Leaf1Legacy {
	a: u64,
	b: u64,
	c: u64,
	d: u64,
	e: u64,
	f: u64,
	g: u64,
	h: u64,
}

#[revisioned(revision(1, encoding = "optimised"))]
struct Leaf1Opt {
	a: u64,
	b: u64,
	c: u64,
	d: u64,
	e: u64,
	f: u64,
	g: u64,
	h: u64,
}

// -----------------------------------------------------------------------------
// Depth 3 (4 levels deep)
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
struct D3Legacy {
	leaf: Leaf1Legacy,
	tag: u32,
}

#[revisioned(revision(1, encoding = "optimised"))]
struct D3Opt {
	leaf: Leaf1Opt,
	tag: u32,
}

#[revisioned(revision = 1)]
struct D3LegacyOuter {
	inner: D3Legacy,
	tag: u32,
}

#[revisioned(revision(1, encoding = "optimised"))]
struct D3OptOuter {
	inner: D3Opt,
	tag: u32,
}

// -----------------------------------------------------------------------------
// Depth 5 (6 levels deep)
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
struct D5Legacy {
	mid: D3LegacyOuter,
	tag: u32,
}

#[revisioned(revision(1, encoding = "optimised"))]
struct D5Opt {
	mid: D3OptOuter,
	tag: u32,
}

#[revisioned(revision = 1)]
struct D5LegacyOuter {
	deep: D5Legacy,
	tag: u32,
}

#[revisioned(revision(1, encoding = "optimised"))]
struct D5OptOuter {
	deep: D5Opt,
	tag: u32,
}

// -----------------------------------------------------------------------------
// Fixture constructors
// -----------------------------------------------------------------------------

fn sample_leaf1_legacy() -> Leaf1Legacy {
	Leaf1Legacy {
		a: 1,
		b: 2,
		c: 3,
		d: 4,
		e: 5,
		f: 6,
		g: 7,
		h: 8,
	}
}
fn sample_leaf1_opt() -> Leaf1Opt {
	Leaf1Opt {
		a: 1,
		b: 2,
		c: 3,
		d: 4,
		e: 5,
		f: 6,
		g: 7,
		h: 8,
	}
}
fn sample_d3_legacy() -> D3LegacyOuter {
	D3LegacyOuter {
		inner: D3Legacy {
			leaf: sample_leaf1_legacy(),
			tag: 9,
		},
		tag: 10,
	}
}
fn sample_d3_opt() -> D3OptOuter {
	D3OptOuter {
		inner: D3Opt {
			leaf: sample_leaf1_opt(),
			tag: 9,
		},
		tag: 10,
	}
}
fn sample_d5_legacy() -> D5LegacyOuter {
	D5LegacyOuter {
		deep: D5Legacy {
			mid: sample_d3_legacy(),
			tag: 11,
		},
		tag: 12,
	}
}
fn sample_d5_opt() -> D5OptOuter {
	D5OptOuter {
		deep: D5Opt {
			mid: sample_d3_opt(),
			tag: 11,
		},
		tag: 12,
	}
}

// -----------------------------------------------------------------------------
// Skip benches across depths
// -----------------------------------------------------------------------------

fn bench_skip_by_depth(c: &mut Criterion) {
	let d1_legacy = revision::to_vec(&sample_leaf1_legacy()).unwrap();
	let d1_opt = revision::to_vec(&sample_leaf1_opt()).unwrap();
	let d3_legacy = revision::to_vec(&sample_d3_legacy()).unwrap();
	let d3_opt = revision::to_vec(&sample_d3_opt()).unwrap();
	let d5_legacy = revision::to_vec(&sample_d5_legacy()).unwrap();
	let d5_opt = revision::to_vec(&sample_d5_opt()).unwrap();

	eprintln!("\n=== Nested struct skip cost ===");
	eprintln!("  depth 1  legacy: {} bytes  | optimised: {} bytes", d1_legacy.len(), d1_opt.len());
	eprintln!("  depth 3  legacy: {} bytes  | optimised: {} bytes", d3_legacy.len(), d3_opt.len());
	eprintln!("  depth 5  legacy: {} bytes  | optimised: {} bytes", d5_legacy.len(), d5_opt.len());
	eprintln!();

	let mut group = c.benchmark_group("skip_nested_depth1");
	group.throughput(Throughput::Bytes(d1_legacy.len() as u64));
	group.bench_function("legacy", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&d1_legacy);
			<Leaf1Legacy as SkipRevisioned>::skip_revisioned(&mut r).unwrap();
		})
	});
	group.bench_function("optimised", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&d1_opt);
			<Leaf1Opt as SkipRevisioned>::skip_revisioned(&mut r).unwrap();
		})
	});
	group.finish();

	let mut group = c.benchmark_group("skip_nested_depth3");
	group.throughput(Throughput::Bytes(d3_legacy.len() as u64));
	group.bench_function("legacy", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&d3_legacy);
			<D3LegacyOuter as SkipRevisioned>::skip_revisioned(&mut r).unwrap();
		})
	});
	group.bench_function("optimised", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&d3_opt);
			<D3OptOuter as SkipRevisioned>::skip_revisioned(&mut r).unwrap();
		})
	});
	group.finish();

	let mut group = c.benchmark_group("skip_nested_depth5");
	group.throughput(Throughput::Bytes(d5_legacy.len() as u64));
	group.bench_function("legacy", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&d5_legacy);
			<D5LegacyOuter as SkipRevisioned>::skip_revisioned(&mut r).unwrap();
		})
	});
	group.bench_function("optimised", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&d5_opt);
			<D5OptOuter as SkipRevisioned>::skip_revisioned(&mut r).unwrap();
		})
	});
	group.finish();
}

criterion_group!(benches, bench_skip_by_depth);
criterion_main!(benches);
