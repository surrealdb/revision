use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use serde::{Deserialize, Serialize};
use std::hint::black_box;

// Wrapper type for bincode comparison
#[derive(Serialize, Deserialize)]
struct BincodeVecI128(Vec<i128>);

// Generate test data for benchmarking
fn generate_test_data(size: usize) -> Vec<i128> {
	(0..size).map(|i| (i as i128).wrapping_sub(size as i128 / 2)).collect()
}

fn benchmark_serialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<i128> Serialization");

	for &size in &sizes {
		let data = generate_test_data(size);
		// i128 is 16 bytes per element
		group.throughput(Throughput::Bytes((size * 16) as u64));

		// Benchmark revision Vec<i128> serialization
		group.bench_with_input(BenchmarkId::new("Revision", size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});

		// Benchmark bincode serialization for comparison
		let bincode_data = BincodeVecI128(data.clone());
		group.bench_with_input(BenchmarkId::new("Bincode", size), &size, |b, _| {
			b.iter(|| {
				let serialized = bincode::serialize(black_box(&bincode_data)).unwrap();
				black_box(serialized)
			})
		});
	}
	group.finish();
}

fn benchmark_deserialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<i128> Deserialization");

	for &size in &sizes {
		let data = generate_test_data(size);
		// i128 is 16 bytes per element
		group.throughput(Throughput::Bytes((size * 16) as u64));

		// Pre-serialize data for deserialization benchmarks
		let revision_serialized = revision::to_vec(&data).unwrap();
		let bincode_data = BincodeVecI128(data.clone());
		let bincode_serialized = bincode::serialize(&bincode_data).unwrap();

		// Benchmark revision Vec<i128> deserialization
		group.bench_with_input(BenchmarkId::new("Revision", size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<i128> =
					revision::from_slice(black_box(&revision_serialized)).unwrap();
				black_box(deserialized)
			})
		});

		// Benchmark bincode deserialization for comparison
		group.bench_with_input(BenchmarkId::new("Bincode", size), &size, |b, _| {
			b.iter(|| {
				let deserialized: BincodeVecI128 =
					bincode::deserialize(black_box(&bincode_serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

criterion_group!(benches, benchmark_serialization, benchmark_deserialization);
criterion_main!(benches);
