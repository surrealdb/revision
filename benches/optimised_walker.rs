//! Walker performance: walker construction + selective field decode.
//!
//! Compares (a) full DeserializeRevisioned + field extract against
//! (b) walker construction + decode_<field>, for both legacy and optimised
//! encodings. Quantifies the value of the walker abstraction for the
//! "I only want one field" use case.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use revision::prelude::*;

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

#[revisioned(revision(1, encoding = "optimised"))]
#[derive(Clone)]
struct WideOpt {
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
fn sample_opt() -> WideOpt {
	WideOpt {
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

fn bench_walk_then_decode_first_field(c: &mut Criterion) {
	let legacy_bytes = revision::to_vec(&sample_legacy()).unwrap();
	let opt_bytes = revision::to_vec(&sample_opt()).unwrap();

	let mut group = c.benchmark_group("walker_decode_first_field");
	group.throughput(Throughput::Elements(1));

	group.bench_function("legacy_walker", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&legacy_bytes);
			let mut w = WideLegacy::walk_revisioned(&mut r).unwrap();
			black_box(w.decode_a().unwrap());
		})
	});

	group.bench_function("optimised_walker", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&opt_bytes);
			let mut w = WideOpt::walk_revisioned(&mut r).unwrap();
			black_box(w.decode_a().unwrap());
		})
	});

	group.bench_function("legacy_full_decode", |b| {
		b.iter(|| {
			let v: WideLegacy = revision::from_slice(black_box(&legacy_bytes)).unwrap();
			black_box(v.a);
		})
	});

	group.bench_function("optimised_full_decode", |b| {
		b.iter(|| {
			let v: WideOpt = revision::from_slice(black_box(&opt_bytes)).unwrap();
			black_box(v.a);
		})
	});

	group.finish();
}

fn bench_walker_skip_to_last_field(c: &mut Criterion) {
	let legacy_bytes = revision::to_vec(&sample_legacy()).unwrap();
	let opt_bytes = revision::to_vec(&sample_opt()).unwrap();

	let mut group = c.benchmark_group("walker_decode_last_field");
	group.throughput(Throughput::Elements(1));

	// Walker reaches `h` by sequentially skipping a..g first.
	group.bench_function("legacy_walker", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&legacy_bytes);
			let mut w = WideLegacy::walk_revisioned(&mut r).unwrap();
			w.skip_a().unwrap();
			w.skip_b().unwrap();
			w.skip_c().unwrap();
			w.skip_d().unwrap();
			w.skip_e().unwrap();
			w.skip_f().unwrap();
			w.skip_g().unwrap();
			black_box(w.decode_h().unwrap());
		})
	});

	group.bench_function("optimised_walker", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&opt_bytes);
			let mut w = WideOpt::walk_revisioned(&mut r).unwrap();
			w.skip_a().unwrap();
			w.skip_b().unwrap();
			w.skip_c().unwrap();
			w.skip_d().unwrap();
			w.skip_e().unwrap();
			w.skip_f().unwrap();
			w.skip_g().unwrap();
			black_box(w.decode_h().unwrap());
		})
	});

	group.finish();
}

criterion_group!(benches, bench_walk_then_decode_first_field, bench_walker_skip_to_last_field);
criterion_main!(benches);
