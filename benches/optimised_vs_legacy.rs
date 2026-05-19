//! Direct legacy vs optimised wire-format comparison.
//!
//! Pairs of identically-shaped types — one with `revision = N` (legacy) and
//! one with `revision(N, optimised, ...)` — exercise the encode,
//! decode, skip, and wire-size paths side-by-side. Criterion groups read as
//! "operation on type X under legacy vs optimised".

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use revision::prelude::*;

// -----------------------------------------------------------------------------
// Fixtures
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
#[derive(Clone)]
struct WideLegacy {
	a: u32,
	b: u32,
	c: u32,
	d: u32,
	e: u32,
	f: u32,
	g: u32,
	h: u32,
}

#[revisioned(revision(1, optimised))]
#[derive(Clone)]
struct WideOptimised {
	a: u32,
	b: u32,
	c: u32,
	d: u32,
	e: u32,
	f: u32,
	g: u32,
	h: u32,
}

#[revisioned(revision(1, optimised, indexed_struct))]
#[derive(Clone)]
struct WideIndexed {
	a: u32,
	b: u32,
	c: u32,
	d: u32,
	e: u32,
	f: u32,
	g: u32,
	h: u32,
}

fn sample_legacy() -> WideLegacy {
	WideLegacy {
		a: 0xDEADBEEF,
		b: 0xCAFEBABE,
		c: 1,
		d: 2,
		e: 3,
		f: 4,
		g: 5,
		h: 6,
	}
}

fn sample_optimised() -> WideOptimised {
	WideOptimised {
		a: 0xDEADBEEF,
		b: 0xCAFEBABE,
		c: 1,
		d: 2,
		e: 3,
		f: 4,
		g: 5,
		h: 6,
	}
}

fn sample_indexed() -> WideIndexed {
	WideIndexed {
		a: 0xDEADBEEF,
		b: 0xCAFEBABE,
		c: 1,
		d: 2,
		e: 3,
		f: 4,
		g: 5,
		h: 6,
	}
}

// -----------------------------------------------------------------------------
// Encode benches
// -----------------------------------------------------------------------------

fn bench_encode(c: &mut Criterion) {
	let mut group = c.benchmark_group("encode_wide_struct");
	let legacy_value = sample_legacy();
	let optimised_value = sample_optimised();
	let indexed_value = sample_indexed();

	group.throughput(Throughput::Elements(1));

	group.bench_function("legacy", |b| {
		b.iter(|| {
			let bytes = revision::to_vec(black_box(&legacy_value)).unwrap();
			black_box(bytes);
		});
	});

	group.bench_function("optimised_sequential", |b| {
		b.iter(|| {
			let bytes = revision::to_vec(black_box(&optimised_value)).unwrap();
			black_box(bytes);
		});
	});

	group.bench_function("optimised_indexed", |b| {
		b.iter(|| {
			let bytes = revision::to_vec(black_box(&indexed_value)).unwrap();
			black_box(bytes);
		});
	});

	group.finish();
}

// -----------------------------------------------------------------------------
// Decode benches
// -----------------------------------------------------------------------------

fn bench_decode(c: &mut Criterion) {
	let mut group = c.benchmark_group("decode_wide_struct");
	let legacy_bytes = revision::to_vec(&sample_legacy()).unwrap();
	let optimised_bytes = revision::to_vec(&sample_optimised()).unwrap();
	let indexed_bytes = revision::to_vec(&sample_indexed()).unwrap();

	group.throughput(Throughput::Elements(1));

	group.bench_function("legacy", |b| {
		b.iter(|| {
			let v: WideLegacy = revision::from_slice(black_box(&legacy_bytes)).unwrap();
			black_box(v);
		});
	});

	group.bench_function("optimised_sequential", |b| {
		b.iter(|| {
			let v: WideOptimised = revision::from_slice(black_box(&optimised_bytes)).unwrap();
			black_box(v);
		});
	});

	group.bench_function("optimised_indexed", |b| {
		b.iter(|| {
			let v: WideIndexed = revision::from_slice(black_box(&indexed_bytes)).unwrap();
			black_box(v);
		});
	});

	group.finish();
}

// -----------------------------------------------------------------------------
// Skip benches — optimised's headline win
// -----------------------------------------------------------------------------

fn bench_skip(c: &mut Criterion) {
	let mut group = c.benchmark_group("skip_wide_struct");
	let legacy_bytes = revision::to_vec(&sample_legacy()).unwrap();
	let optimised_bytes = revision::to_vec(&sample_optimised()).unwrap();
	let indexed_bytes = revision::to_vec(&sample_indexed()).unwrap();

	group.throughput(Throughput::Bytes(legacy_bytes.len() as u64));

	group.bench_function("legacy", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&legacy_bytes);
			<WideLegacy as SkipRevisioned>::skip_revisioned(&mut r).unwrap();
		});
	});

	group.bench_function("optimised_sequential", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&optimised_bytes);
			<WideOptimised as SkipRevisioned>::skip_revisioned(&mut r).unwrap();
		});
	});

	group.bench_function("optimised_indexed", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&indexed_bytes);
			<WideIndexed as SkipRevisioned>::skip_revisioned(&mut r).unwrap();
		});
	});

	group.finish();
}

// -----------------------------------------------------------------------------
// Wire-size comparison (informational; runs once)
// -----------------------------------------------------------------------------

fn bench_wire_size(c: &mut Criterion) {
	let legacy_size = revision::to_vec(&sample_legacy()).unwrap().len();
	let optimised_size = revision::to_vec(&sample_optimised()).unwrap().len();
	let indexed_size = revision::to_vec(&sample_indexed()).unwrap().len();

	eprintln!("\n=== Wire size (8-field wide struct) ===");
	eprintln!("  legacy:               {legacy_size} bytes");
	eprintln!(
		"  optimised sequential: {optimised_size} bytes ({:+} vs legacy)",
		optimised_size as i64 - legacy_size as i64
	);
	eprintln!(
		"  optimised indexed:    {indexed_size} bytes ({:+} vs legacy)",
		indexed_size as i64 - legacy_size as i64
	);
	eprintln!();

	// Run a no-op timing so Criterion is happy.
	c.bench_function("wire_size_report", |b| b.iter(|| black_box(legacy_size)));
}

criterion_group!(benches, bench_encode, bench_decode, bench_skip, bench_wire_size);
criterion_main!(benches);
