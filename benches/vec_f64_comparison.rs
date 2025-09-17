use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use revision::specialised::RevisionSpecialisedVecF64;
use std::hint::black_box;

// Generate test data for benchmarking
fn generate_test_data(size: usize) -> Vec<f64> {
	(0..size)
		.map(|i| {
			// Create diverse f64 values including special cases
			match i % 7 {
				0 => (i as f64) * 3.14159,
				1 => -(i as f64) / 2.718,
				2 => (i as f64).sqrt(),
				3 => (i as f64).ln(),
				4 => (i as f64).sin(),
				5 => {
					if i % 100 == 0 {
						f64::NAN
					} else {
						(i as f64) * 1e-10
					}
				}
				6 => {
					if i % 150 == 0 {
						f64::INFINITY
					} else {
						(i as f64) * 1e10
					}
				}
				_ => i as f64,
			}
		})
		.collect()
}

fn benchmark_serialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<f64> Serialization");

	for &size in &sizes {
		let data = generate_test_data(size);
		// f64 is 8 bytes per element
		group.throughput(Throughput::Bytes((size * 8) as u64));

		// Benchmark regular Vec<f64> serialization
		group.bench_with_input(BenchmarkId::new("Regular", size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});

		// Benchmark RevisionSpecialisedVecF64 serialization
		let specialized_data = RevisionSpecialisedVecF64::from_vec(data.clone());
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

	let mut group = c.benchmark_group("Vec<f64> Deserialization");

	for &size in &sizes {
		let data = generate_test_data(size);
		// f64 is 8 bytes per element
		group.throughput(Throughput::Bytes((size * 8) as u64));

		// Pre-serialize data for deserialization benchmarks
		let regular_serialized = revision::to_vec(&data).unwrap();
		let specialized_data = RevisionSpecialisedVecF64::from_vec(data.clone());
		let specialized_serialized = revision::to_vec(&specialized_data).unwrap();

		// Benchmark regular Vec<f64> deserialization
		group.bench_with_input(BenchmarkId::new("Regular", size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<f64> =
					revision::from_slice(black_box(&regular_serialized)).unwrap();
				black_box(deserialized)
			})
		});

		// Benchmark RevisionSpecialisedVecF64 deserialization
		group.bench_with_input(BenchmarkId::new("Specialized", size), &size, |b, _| {
			b.iter(|| {
				let deserialized: RevisionSpecialisedVecF64 =
					revision::from_slice(black_box(&specialized_serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

criterion_group!(benches, benchmark_serialization, benchmark_deserialization,);
criterion_main!(benches);
