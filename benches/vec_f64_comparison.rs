use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde::{Deserialize, Serialize};
use std::hint::black_box;

// Wrapper type for bincode comparison
#[derive(Serialize, Deserialize)]
struct BincodeVecF64(Vec<f64>);

// Generate test data for benchmarking
fn generate_test_data(size: usize) -> Vec<f64> {
	(0..size)
		.map(|i| {
			// Create diverse f64 values including special cases
			match i % 7 {
				0 => (i as f64) * std::f64::consts::PI,
				1 => -(i as f64) / std::f64::consts::E,
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

		// Benchmark revision Vec<f64> serialization
		group.bench_with_input(BenchmarkId::new("Revision", size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});

		// Benchmark bincode serialization for comparison
		let bincode_data = BincodeVecF64(data.clone());
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

	let mut group = c.benchmark_group("Vec<f64> Deserialization");

	for &size in &sizes {
		let data = generate_test_data(size);
		// f64 is 8 bytes per element
		group.throughput(Throughput::Bytes((size * 8) as u64));

		// Pre-serialize data for deserialization benchmarks
		let revision_serialized = revision::to_vec(&data).unwrap();
		let bincode_data = BincodeVecF64(data.clone());
		let bincode_serialized = bincode::serialize(&bincode_data).unwrap();

		// Benchmark revision Vec<f64> deserialization
		group.bench_with_input(BenchmarkId::new("Revision", size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<f64> =
					revision::from_slice(black_box(&revision_serialized)).unwrap();
				black_box(deserialized)
			})
		});

		// Benchmark bincode deserialization for comparison
		group.bench_with_input(BenchmarkId::new("Bincode", size), &size, |b, _| {
			b.iter(|| {
				let deserialized: BincodeVecF64 =
					bincode::deserialize(black_box(&bincode_serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

criterion_group!(benches, benchmark_serialization, benchmark_deserialization);
criterion_main!(benches);
