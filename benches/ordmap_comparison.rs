use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use imbl::OrdMap as ImblOrdMap;
use revision::prelude::*;
use std::collections::BTreeMap;
use std::hint::black_box;

// =============================================================================
// Test Data Generation
// =============================================================================

fn generate_btreemap(size: usize) -> BTreeMap<String, i64> {
	(0..size).map(|i| (format!("key_{:08}", i), i as i64 * 31337)).collect()
}

fn generate_imbl_ordmap(size: usize) -> ImblOrdMap<String, i64> {
	(0..size).map(|i| (format!("key_{:08}", i), i as i64 * 31337)).collect()
}

// =============================================================================
// Benchmark: Map Building
// =============================================================================

fn bench_map_building(c: &mut Criterion) {
	let sizes = [100, 1_000, 10_000, 100_000];

	let mut group = c.benchmark_group("OrdMap Building");

	for &size in &sizes {
		group.throughput(Throughput::Elements(size as u64));

		// Benchmark BTreeMap building
		group.bench_with_input(BenchmarkId::new("BTreeMap", size), &size, |b, &size| {
			b.iter(|| {
				let map: BTreeMap<String, i64> =
					(0..size).map(|i| (format!("key_{:08}", i), i as i64 * 31337)).collect();
				black_box(map)
			})
		});

		// Benchmark imbl::OrdMap building
		group.bench_with_input(BenchmarkId::new("imbl::OrdMap", size), &size, |b, &size| {
			b.iter(|| {
				let map: ImblOrdMap<String, i64> =
					(0..size).map(|i| (format!("key_{:08}", i), i as i64 * 31337)).collect();
				black_box(map)
			})
		});
	}
	group.finish();
}

// =============================================================================
// Benchmark: Cloning
// =============================================================================

fn bench_map_cloning(c: &mut Criterion) {
	let sizes = [100, 1_000, 10_000, 100_000];

	let mut group = c.benchmark_group("OrdMap Cloning");

	for &size in &sizes {
		let btree = generate_btreemap(size);
		let imbl = generate_imbl_ordmap(size);

		group.throughput(Throughput::Elements(size as u64));

		// Benchmark BTreeMap cloning
		group.bench_with_input(BenchmarkId::new("BTreeMap", size), &btree, |b, map| {
			b.iter(|| black_box(map.clone()))
		});

		// Benchmark imbl::OrdMap cloning (should be very cheap - structural sharing)
		group.bench_with_input(BenchmarkId::new("imbl::OrdMap", size), &imbl, |b, map| {
			b.iter(|| black_box(map.clone()))
		});
	}
	group.finish();
}

// =============================================================================
// Benchmark: Clone and Modify (single field)
// =============================================================================

fn bench_clone_and_modify_single(c: &mut Criterion) {
	let sizes = [100, 1_000, 10_000, 100_000];

	let mut group = c.benchmark_group("OrdMap Clone+Modify Single");

	for &size in &sizes {
		let btree = generate_btreemap(size);
		let imbl = generate_imbl_ordmap(size);
		let modify_key = format!("key_{:08}", size / 2); // Modify middle element

		group.throughput(Throughput::Elements(size as u64));

		// Benchmark BTreeMap clone + modify
		group.bench_with_input(BenchmarkId::new("BTreeMap", size), &btree, |b, map| {
			b.iter(|| {
				let mut cloned = map.clone();
				cloned.insert(modify_key.clone(), 999999);
				black_box(cloned)
			})
		});

		// Benchmark imbl::OrdMap clone + modify (structural sharing)
		group.bench_with_input(BenchmarkId::new("imbl::OrdMap", size), &imbl, |b, map| {
			b.iter(|| {
				let cloned = map.update(modify_key.clone(), 999999);
				black_box(cloned)
			})
		});
	}
	group.finish();
}

// =============================================================================
// Benchmark: Clone and Modify (multiple fields)
// =============================================================================

fn bench_clone_and_modify_multiple(c: &mut Criterion) {
	let sizes = [100, 1_000, 10_000, 100_000];
	let num_modifications = 10;

	let mut group = c.benchmark_group("OrdMap Clone+Modify 10 Fields");

	for &size in &sizes {
		let btree = generate_btreemap(size);
		let imbl = generate_imbl_ordmap(size);

		// Generate keys to modify (spread across the map)
		let modify_keys: Vec<String> = (0..num_modifications)
			.map(|i| format!("key_{:08}", (i * size) / num_modifications))
			.collect();

		group.throughput(Throughput::Elements(size as u64));

		// Benchmark BTreeMap clone + multiple modifications
		group.bench_with_input(BenchmarkId::new("BTreeMap", size), &btree, |b, map| {
			b.iter(|| {
				let mut cloned = map.clone();
				for (i, key) in modify_keys.iter().enumerate() {
					cloned.insert(key.clone(), (i * 999999) as i64);
				}
				black_box(cloned)
			})
		});

		// Benchmark imbl::OrdMap with multiple modifications
		group.bench_with_input(BenchmarkId::new("imbl::OrdMap", size), &imbl, |b, map| {
			b.iter(|| {
				let mut cloned = map.clone();
				for (i, key) in modify_keys.iter().enumerate() {
					cloned = cloned.update(key.clone(), (i * 999999) as i64);
				}
				black_box(cloned)
			})
		});
	}
	group.finish();
}

// =============================================================================
// Benchmark: Insert New Key
// =============================================================================

fn bench_clone_and_insert(c: &mut Criterion) {
	let sizes = [100, 1_000, 10_000, 100_000];

	let mut group = c.benchmark_group("OrdMap Clone+Insert New");

	for &size in &sizes {
		let btree = generate_btreemap(size);
		let imbl = generate_imbl_ordmap(size);
		let new_key = "new_key_insert".to_string();

		group.throughput(Throughput::Elements(size as u64));

		// Benchmark BTreeMap clone + insert new key
		group.bench_with_input(BenchmarkId::new("BTreeMap", size), &btree, |b, map| {
			b.iter(|| {
				let mut cloned = map.clone();
				cloned.insert(new_key.clone(), 123456789);
				black_box(cloned)
			})
		});

		// Benchmark imbl::OrdMap clone + insert new key
		group.bench_with_input(BenchmarkId::new("imbl::OrdMap", size), &imbl, |b, map| {
			b.iter(|| {
				let cloned = map.update(new_key.clone(), 123456789);
				black_box(cloned)
			})
		});
	}
	group.finish();
}

// =============================================================================
// Benchmark: Serialization
// =============================================================================

fn bench_serialization(c: &mut Criterion) {
	let sizes = [100, 1_000, 10_000, 100_000];

	let mut group = c.benchmark_group("OrdMap Serialization");

	for &size in &sizes {
		let btree = generate_btreemap(size);
		let imbl = generate_imbl_ordmap(size);

		// Estimate bytes: key (~12 bytes avg) + value (8 bytes) per entry
		group.throughput(Throughput::Elements(size as u64));

		// Benchmark BTreeMap serialization
		group.bench_with_input(BenchmarkId::new("BTreeMap", size), &btree, |b, map| {
			b.iter(|| {
				let mut buffer = Vec::new();
				map.serialize_revisioned(&mut buffer).unwrap();
				black_box(buffer)
			})
		});

		// Benchmark imbl::OrdMap serialization
		group.bench_with_input(BenchmarkId::new("imbl::OrdMap", size), &imbl, |b, map| {
			b.iter(|| {
				let mut buffer = Vec::new();
				map.serialize_revisioned(&mut buffer).unwrap();
				black_box(buffer)
			})
		});
	}
	group.finish();
}

// =============================================================================
// Benchmark: Deserialization
// =============================================================================

fn bench_deserialization(c: &mut Criterion) {
	let sizes = [100, 1_000, 10_000, 100_000];

	let mut group = c.benchmark_group("OrdMap Deserialization");

	for &size in &sizes {
		let btree = generate_btreemap(size);
		let imbl = generate_imbl_ordmap(size);

		// Pre-serialize data
		let mut btree_serialized = Vec::new();
		btree.serialize_revisioned(&mut btree_serialized).unwrap();

		let mut imbl_serialized = Vec::new();
		imbl.serialize_revisioned(&mut imbl_serialized).unwrap();

		group.throughput(Throughput::Elements(size as u64));

		// Benchmark BTreeMap deserialization
		group.bench_with_input(
			BenchmarkId::new("BTreeMap", size),
			&btree_serialized,
			|b, serialized| {
				b.iter(|| {
					let mut cursor = serialized.as_slice();
					let deserialized: BTreeMap<String, i64> =
						BTreeMap::deserialize_revisioned(&mut cursor).unwrap();
					black_box(deserialized)
				})
			},
		);

		// Benchmark imbl::OrdMap deserialization
		group.bench_with_input(
			BenchmarkId::new("imbl::OrdMap", size),
			&imbl_serialized,
			|b, serialized| {
				b.iter(|| {
					let mut cursor = serialized.as_slice();
					let deserialized: ImblOrdMap<String, i64> =
						ImblOrdMap::deserialize_revisioned(&mut cursor).unwrap();
					black_box(deserialized)
				})
			},
		);
	}
	group.finish();
}

// =============================================================================
// Benchmark: Full Roundtrip (serialize + deserialize)
// =============================================================================

fn bench_roundtrip(c: &mut Criterion) {
	let sizes = [100, 1_000, 10_000];

	let mut group = c.benchmark_group("OrdMap Roundtrip");

	for &size in &sizes {
		let btree = generate_btreemap(size);
		let imbl = generate_imbl_ordmap(size);

		group.throughput(Throughput::Elements(size as u64));

		// Benchmark BTreeMap roundtrip
		group.bench_with_input(BenchmarkId::new("BTreeMap", size), &btree, |b, map| {
			b.iter(|| {
				let mut buffer = Vec::new();
				map.serialize_revisioned(&mut buffer).unwrap();
				let mut cursor = buffer.as_slice();
				let deserialized: BTreeMap<String, i64> =
					BTreeMap::deserialize_revisioned(&mut cursor).unwrap();
				black_box(deserialized)
			})
		});

		// Benchmark imbl::OrdMap roundtrip
		group.bench_with_input(BenchmarkId::new("imbl::OrdMap", size), &imbl, |b, map| {
			b.iter(|| {
				let mut buffer = Vec::new();
				map.serialize_revisioned(&mut buffer).unwrap();
				let mut cursor = buffer.as_slice();
				let deserialized: ImblOrdMap<String, i64> =
					ImblOrdMap::deserialize_revisioned(&mut cursor).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

// =============================================================================
// Benchmark: Iteration Performance
// =============================================================================

fn bench_iteration(c: &mut Criterion) {
	let sizes = [100, 1_000, 10_000, 100_000];

	let mut group = c.benchmark_group("OrdMap Iteration");

	for &size in &sizes {
		let btree = generate_btreemap(size);
		let imbl = generate_imbl_ordmap(size);

		group.throughput(Throughput::Elements(size as u64));

		// Benchmark BTreeMap iteration
		group.bench_with_input(BenchmarkId::new("BTreeMap", size), &btree, |b, map| {
			b.iter(|| {
				let sum: i64 = map.values().sum();
				black_box(sum)
			})
		});

		// Benchmark imbl::OrdMap iteration
		group.bench_with_input(BenchmarkId::new("imbl::OrdMap", size), &imbl, |b, map| {
			b.iter(|| {
				let sum: i64 = map.values().sum();
				black_box(sum)
			})
		});
	}
	group.finish();
}

// =============================================================================
// Benchmark: Lookup Performance
// =============================================================================

fn bench_lookup(c: &mut Criterion) {
	let sizes = [100, 1_000, 10_000, 100_000];

	let mut group = c.benchmark_group("OrdMap Lookup");

	for &size in &sizes {
		let btree = generate_btreemap(size);
		let imbl = generate_imbl_ordmap(size);

		// Keys to look up (spread across the map)
		let lookup_keys: Vec<String> =
			(0..100).map(|i| format!("key_{:08}", (i * size) / 100)).collect();

		group.throughput(Throughput::Elements(100)); // 100 lookups

		// Benchmark BTreeMap lookup
		group.bench_with_input(
			BenchmarkId::new("BTreeMap", size),
			&(&btree, &lookup_keys),
			|b, (map, keys)| {
				b.iter(|| {
					let sum: i64 = keys.iter().filter_map(|k| map.get(k)).sum();
					black_box(sum)
				})
			},
		);

		// Benchmark imbl::OrdMap lookup
		group.bench_with_input(
			BenchmarkId::new("imbl::OrdMap", size),
			&(&imbl, &lookup_keys),
			|b, (map, keys)| {
				b.iter(|| {
					let sum: i64 = keys.iter().filter_map(|k| map.get(k)).sum();
					black_box(sum)
				})
			},
		);
	}
	group.finish();
}

criterion_group!(
	benches,
	bench_map_building,
	bench_map_cloning,
	bench_clone_and_modify_single,
	bench_clone_and_modify_multiple,
	bench_clone_and_insert,
	bench_serialization,
	bench_deserialization,
	bench_roundtrip,
	bench_iteration,
	bench_lookup,
);
criterion_main!(benches);
