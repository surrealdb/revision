//! Late-field access on a wide nested struct: three approaches compared.
//!
//! Fixture: 25-field struct with three nested 4-field sub-structs, mixed
//! primitive and `String` fields, and the search target (`target`) placed at
//! field index 22 — deliberately near the end so the legacy walker has to
//! drag through almost every preceding field to reach it.
//!
//! Approaches:
//!
//! 1. **Allocate** — `revision::from_slice::<Wide25Legacy>` materialises the
//!    full struct into owned values, then accesses `.target`.
//! 2. **Skip** — walker over the legacy bytes, calling `skip_<field>` 22
//!    times to advance through preceding fields, then `decode_target`.
//! 3. **Zero-copy jump** — same logical value encoded under optimised +
//!    `indexed_struct`, then `IndexedStructWalker::field_bytes(22)` to
//!    pull the target's raw bytes via the offset table (O(1)), and a
//!    `memcmp` to a pre-serialised target value.
//!
//! The third path is what surrealdb's pre-decode-filter wants: peek at a
//! single field's bytes without materialising any of the rest.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use revision::optimised::IndexedStructWalker;
use revision::prelude::*;

// -----------------------------------------------------------------------------
// Fixture: 25 fields, three nested sub-structs, target at index 22
// -----------------------------------------------------------------------------

#[revisioned(revision = 1)]
#[derive(Clone)]
struct NestedLegacy {
	a: u32,
	b: String,
	c: bool,
	d: u64,
}

#[revisioned(revision(1, optimised))]
#[derive(Clone)]
struct NestedOpt {
	a: u32,
	b: String,
	c: bool,
	d: u64,
}

#[revisioned(revision = 1)]
#[derive(Clone)]
struct Wide25Legacy {
	id: u32,
	flag1: bool,
	counter1: u64,
	label1: String,
	score: i32,
	flag2: bool,
	counter2: u64,
	label2: String,
	rating: f64,
	flag3: bool,
	inner1: NestedLegacy,
	inner2: NestedLegacy,
	inner3: NestedLegacy,
	counter3: u64,
	label3: String,
	flag4: bool,
	counter4: u64,
	inner4: NestedLegacy,
	inner5: NestedLegacy,
	flag5: bool,
	counter5: u64,
	label4: String,
	// ↓ search target — field index 22 of 25 (0-based: 22)
	target: String,
	score2: i32,
	final_label: String,
}

#[revisioned(revision(1, optimised, indexed_struct))]
#[derive(Clone)]
struct Wide25IndexedOpt {
	id: u32,
	flag1: bool,
	counter1: u64,
	label1: String,
	score: i32,
	flag2: bool,
	counter2: u64,
	label2: String,
	rating: f64,
	flag3: bool,
	inner1: NestedOpt,
	inner2: NestedOpt,
	inner3: NestedOpt,
	counter3: u64,
	label3: String,
	flag4: bool,
	counter4: u64,
	inner4: NestedOpt,
	inner5: NestedOpt,
	flag5: bool,
	counter5: u64,
	label4: String,
	target: String, // ← field index 22
	score2: i32,
	final_label: String,
}

const TARGET_VALUE: &str = "surrealdb-record-target-7";

fn nested_legacy() -> NestedLegacy {
	NestedLegacy {
		a: 0xCAFE,
		b: "nested-label".into(),
		c: true,
		d: 0xDEADBEEF_CAFEBABE,
	}
}
fn nested_opt() -> NestedOpt {
	NestedOpt {
		a: 0xCAFE,
		b: "nested-label".into(),
		c: true,
		d: 0xDEADBEEF_CAFEBABE,
	}
}

fn sample_legacy() -> Wide25Legacy {
	Wide25Legacy {
		id: 42,
		flag1: true,
		counter1: 100,
		label1: "field-one".into(),
		score: -7,
		flag2: false,
		counter2: 200,
		label2: "field-two".into(),
		rating: 3.5,
		flag3: true,
		inner1: nested_legacy(),
		inner2: nested_legacy(),
		inner3: nested_legacy(),
		counter3: 300,
		label3: "field-three".into(),
		flag4: false,
		counter4: 400,
		inner4: nested_legacy(),
		inner5: nested_legacy(),
		flag5: true,
		counter5: 500,
		label4: "field-four".into(),
		target: TARGET_VALUE.into(),
		score2: 999,
		final_label: "the-end".into(),
	}
}

fn sample_opt() -> Wide25IndexedOpt {
	Wide25IndexedOpt {
		id: 42,
		flag1: true,
		counter1: 100,
		label1: "field-one".into(),
		score: -7,
		flag2: false,
		counter2: 200,
		label2: "field-two".into(),
		rating: 3.5,
		flag3: true,
		inner1: nested_opt(),
		inner2: nested_opt(),
		inner3: nested_opt(),
		counter3: 300,
		label3: "field-three".into(),
		flag4: false,
		counter4: 400,
		inner4: nested_opt(),
		inner5: nested_opt(),
		flag5: true,
		counter5: 500,
		label4: "field-four".into(),
		target: TARGET_VALUE.into(),
		score2: 999,
		final_label: "the-end".into(),
	}
}

// -----------------------------------------------------------------------------
// Benchmarks
// -----------------------------------------------------------------------------

fn bench_alloc_legacy(c: &mut Criterion) {
	let bytes = revision::to_vec(&sample_legacy()).unwrap();
	let expected = TARGET_VALUE.to_string();

	let mut group = c.benchmark_group("late_field_access");
	group.throughput(Throughput::Bytes(bytes.len() as u64));

	group.bench_function("1_alloc_legacy_full_decode", |b| {
		b.iter(|| {
			let decoded: Wide25Legacy = revision::from_slice(black_box(&bytes)).unwrap();
			let matches = decoded.target == expected;
			black_box(matches);
		});
	});

	// -------------------------------------------------------------------------
	// (2) Walker skip-through on the legacy encoding.
	// -------------------------------------------------------------------------
	group.bench_function("2_skip_legacy_walker", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&bytes);
			let mut w = Wide25Legacy::walk_revisioned(&mut r).unwrap();
			w.skip_id().unwrap();
			w.skip_flag1().unwrap();
			w.skip_counter1().unwrap();
			w.skip_label1().unwrap();
			w.skip_score().unwrap();
			w.skip_flag2().unwrap();
			w.skip_counter2().unwrap();
			w.skip_label2().unwrap();
			w.skip_rating().unwrap();
			w.skip_flag3().unwrap();
			w.skip_inner1().unwrap();
			w.skip_inner2().unwrap();
			w.skip_inner3().unwrap();
			w.skip_counter3().unwrap();
			w.skip_label3().unwrap();
			w.skip_flag4().unwrap();
			w.skip_counter4().unwrap();
			w.skip_inner4().unwrap();
			w.skip_inner5().unwrap();
			w.skip_flag5().unwrap();
			w.skip_counter5().unwrap();
			w.skip_label4().unwrap();
			let decoded = w.decode_target().unwrap();
			let matches = decoded == expected;
			black_box(matches);
		});
	});

	group.finish();
}

fn bench_optimised_walker_jump(c: &mut Criterion) {
	// (4) Macro-generated walker on the indexed-struct payload.
	// After A (O(1) field access in the walker), the macro-emitted
	// `decode_target` should match the hand-rolled `IndexedStructWalker`
	// jump within noise.
	let bytes = revision::to_vec(&sample_opt()).unwrap();
	let expected = TARGET_VALUE.to_string();

	let mut group = c.benchmark_group("late_field_access");
	group.throughput(Throughput::Bytes(bytes.len() as u64));

	group.bench_function("4_macro_walker_optimised_indexed", |b| {
		b.iter(|| {
			let mut r: &[u8] = black_box(&bytes);
			let mut w = Wide25IndexedOpt::walk_revisioned(&mut r).unwrap();
			let decoded = w.decode_target().unwrap();
			let matches = decoded == expected;
			black_box(matches);
		});
	});

	group.finish();
}

fn bench_optimised_jump(c: &mut Criterion) {
	let bytes = revision::to_vec(&sample_opt()).unwrap();

	// Optimised wire layout for a `indexed_struct` record:
	//   u16 revision      (1 byte under varint, 2 under fixed-width-encoding)
	//   u32_le payload_length     (always 4 bytes by spec)
	//   [u32_le; field_count] offset table
	//   fields...
	//
	// The IndexedStructWalker expects its payload slice to begin with the
	// offset table. Strip the revision header + u32_le length prefix.
	// Compute the revision header size at runtime so the bench works under
	// either varint or fixed-width-encoding.
	let payload_offset = {
		let mut rev_buf = Vec::new();
		<u16 as SerializeRevisioned>::serialize_revisioned(&1u16, &mut rev_buf).unwrap();
		rev_buf.len() + 4
	};

	// Pre-serialise the expected target value so the comparison is a pure
	// byte-slice memcmp — no decode, no allocation.
	let mut target_bytes = Vec::new();
	<String as SerializeRevisioned>::serialize_revisioned(
		&TARGET_VALUE.to_string(),
		&mut target_bytes,
	)
	.unwrap();

	let mut group = c.benchmark_group("late_field_access");
	group.throughput(Throughput::Bytes(bytes.len() as u64));

	group.bench_function("3_zero_copy_jump_optimised", |b| {
		b.iter(|| {
			let payload: &[u8] = black_box(&bytes[payload_offset..]);
			// Construct the indexed walker over the payload slice. Validates
			// the prologue once; subsequent field_bytes() calls are O(1).
			let w = IndexedStructWalker::from_payload(payload, 1, 25).unwrap();
			let field_bytes = w.field_bytes(22).unwrap();
			// Zero-copy comparison: raw byte equality between the field's
			// encoded slice and the pre-serialised target. No allocation,
			// no decode of `target` (just a memcmp).
			let matches = field_bytes == target_bytes.as_slice();
			black_box(matches);
		});
	});

	group.finish();
}

// -----------------------------------------------------------------------------
// Wire-size reporting (informational, prints to stderr)
// -----------------------------------------------------------------------------

fn report_sizes(_c: &mut Criterion) {
	let legacy_bytes = revision::to_vec(&sample_legacy()).unwrap();
	let opt_bytes = revision::to_vec(&sample_opt()).unwrap();
	eprintln!();
	eprintln!("=== Wire size (25-field nested struct, target at index 22) ===");
	eprintln!("  legacy:            {} bytes", legacy_bytes.len());
	eprintln!(
		"  optimised+indexed: {} bytes (+{} for envelope + offset table)",
		opt_bytes.len(),
		opt_bytes.len() as i64 - legacy_bytes.len() as i64,
	);
	eprintln!();
}

criterion_group!(
	benches,
	report_sizes,
	bench_alloc_legacy,
	bench_optimised_walker_jump,
	bench_optimised_jump,
);
criterion_main!(benches);
