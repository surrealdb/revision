use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use revision::specialised::RevisionSpecialisedVecU8;
use std::hint::black_box;

// Generate test data for benchmarking
fn generate_test_data(size: usize) -> Vec<u8> {
	(0..size).map(|i| (i % 256) as u8).collect()
}

fn benchmark_serialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<u8> Serialization");

	for &size in &sizes {
		let data = generate_test_data(size);
		group.throughput(Throughput::Bytes(size as u64));

		// Benchmark regular Vec<u8> serialization
		group.bench_with_input(BenchmarkId::new("Regular", size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});

		// Benchmark RevisionSpecialisedVecU8 serialization
		let specialized_data = RevisionSpecialisedVecU8::from_vec(data.clone());
		group.bench_with_input(BenchmarkId::new("Specialized", size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&specialized_data)).unwrap();
				black_box(serialized)
			})
		});
	}
	group.finish();
}

fn benchmark_deserialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<u8> Deserialization");

	for &size in &sizes {
		let data = generate_test_data(size);
		group.throughput(Throughput::Bytes(size as u64));

		// Pre-serialize data for deserialization benchmarks
		let regular_serialized = revision::to_vec(&data).unwrap();
		let specialized_data = RevisionSpecialisedVecU8::from_vec(data.clone());
		let specialized_serialized = revision::to_vec(&specialized_data).unwrap();

		// Benchmark regular Vec<u8> deserialization
		group.bench_with_input(BenchmarkId::new("Regular", size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<u8> =
					revision::from_slice(black_box(&regular_serialized)).unwrap();
				black_box(deserialized)
			})
		});

		// Benchmark RevisionSpecialisedVecU8 deserialization
		group.bench_with_input(BenchmarkId::new("Specialized", size), &size, |b, _| {
			b.iter(|| {
				let deserialized: RevisionSpecialisedVecU8 =
					revision::from_slice(black_box(&specialized_serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

criterion_group!(benches, benchmark_serialization, benchmark_deserialization,);
criterion_main!(benches);
