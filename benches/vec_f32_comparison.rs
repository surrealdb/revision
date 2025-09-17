use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use revision::specialised::RevisionSpecialisedVecF32;
use std::hint::black_box;

// Generate test data for benchmarking
fn generate_test_data(size: usize) -> Vec<f32> {
	(0..size)
		.map(|i| {
			// Create diverse f32 values including special cases
			match i % 7 {
				0 => (i as f32) * 3.14159,
				1 => -(i as f32) / 2.718,
				2 => (i as f32).sqrt(),
				3 => (i as f32).ln(),
				4 => (i as f32).sin(),
				5 => {
					if i % 100 == 0 {
						f32::NAN
					} else {
						(i as f32) * 1e-10
					}
				}
				6 => {
					if i % 150 == 0 {
						f32::INFINITY
					} else {
						(i as f32) * 1e10
					}
				}
				_ => i as f32,
			}
		})
		.collect()
}

fn benchmark_serialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<f32> Serialization");

	for &size in &sizes {
		let data = generate_test_data(size);
		// f32 is 8 bytes per element
		group.throughput(Throughput::Bytes((size * 8) as u64));

		// Benchmark regular Vec<f32> serialization
		group.bench_with_input(BenchmarkId::new("Regular", size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});

		// Benchmark RevisionSpecialisedVecF32 serialization
		let specialized_data = RevisionSpecialisedVecF32::from_vec(data.clone());
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

	let mut group = c.benchmark_group("Vec<f32> Deserialization");

	for &size in &sizes {
		let data = generate_test_data(size);
		// f32 is 8 bytes per element
		group.throughput(Throughput::Bytes((size * 8) as u64));

		// Pre-serialize data for deserialization benchmarks
		let regular_serialized = revision::to_vec(&data).unwrap();
		let specialized_data = RevisionSpecialisedVecF32::from_vec(data.clone());
		let specialized_serialized = revision::to_vec(&specialized_data).unwrap();

		// Benchmark regular Vec<f32> deserialization
		group.bench_with_input(BenchmarkId::new("Regular", size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<f32> =
					revision::from_slice(black_box(&regular_serialized)).unwrap();
				black_box(deserialized)
			})
		});

		// Benchmark RevisionSpecialisedVecF32 deserialization
		group.bench_with_input(BenchmarkId::new("Specialized", size), &size, |b, _| {
			b.iter(|| {
				let deserialized: RevisionSpecialisedVecF32 =
					revision::from_slice(black_box(&specialized_serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

criterion_group!(benches, benchmark_serialization, benchmark_deserialization,);
criterion_main!(benches);
