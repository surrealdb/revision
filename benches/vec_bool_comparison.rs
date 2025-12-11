use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use revision::prelude::*;

fn bench_vec_bool_serialize(c: &mut Criterion) {
	let mut group = c.benchmark_group("vec_bool_serialize");

	for size in [100, 1000, 10000, 100000].iter() {
		// Test different patterns
		let alternating: Vec<bool> = (0..*size).map(|i| i % 2 == 0).collect();
		let all_true = vec![true; *size];
		let all_false = vec![false; *size];
		let random_pattern: Vec<bool> = (0..*size).map(|i| (i * 7919) % 3 == 0).collect();

		group.bench_with_input(BenchmarkId::new("alternating", size), &alternating, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});

		group.bench_with_input(BenchmarkId::new("all_true", size), &all_true, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});

		group.bench_with_input(BenchmarkId::new("all_false", size), &all_false, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});

		group.bench_with_input(BenchmarkId::new("random", size), &random_pattern, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});
	}
	group.finish();
}

fn bench_vec_bool_deserialize(c: &mut Criterion) {
	let mut group = c.benchmark_group("vec_bool_deserialize");

	for size in [100, 1000, 10000, 100000].iter() {
		let alternating: Vec<bool> = (0..*size).map(|i| i % 2 == 0).collect();
		let all_true = vec![true; *size];
		let all_false = vec![false; *size];
		let random_pattern: Vec<bool> = (0..*size).map(|i| (i * 7919) % 3 == 0).collect();

		// Pre-serialize
		let mut alternating_bytes = Vec::new();
		alternating.serialize_revisioned(&mut alternating_bytes).unwrap();

		let mut all_true_bytes = Vec::new();
		all_true.serialize_revisioned(&mut all_true_bytes).unwrap();

		let mut all_false_bytes = Vec::new();
		all_false.serialize_revisioned(&mut all_false_bytes).unwrap();

		let mut random_bytes = Vec::new();
		random_pattern.serialize_revisioned(&mut random_bytes).unwrap();

		group.bench_with_input(
			BenchmarkId::new("alternating", size),
			&alternating_bytes,
			|b, data| {
				b.iter(|| {
					let mut cursor = data.as_slice();
					Vec::<bool>::deserialize_revisioned(&mut cursor).unwrap()
				})
			},
		);

		group.bench_with_input(BenchmarkId::new("all_true", size), &all_true_bytes, |b, data| {
			b.iter(|| {
				let mut cursor = data.as_slice();
				Vec::<bool>::deserialize_revisioned(&mut cursor).unwrap()
			})
		});

		group.bench_with_input(BenchmarkId::new("all_false", size), &all_false_bytes, |b, data| {
			b.iter(|| {
				let mut cursor = data.as_slice();
				Vec::<bool>::deserialize_revisioned(&mut cursor).unwrap()
			})
		});

		group.bench_with_input(BenchmarkId::new("random", size), &random_bytes, |b, data| {
			b.iter(|| {
				let mut cursor = data.as_slice();
				Vec::<bool>::deserialize_revisioned(&mut cursor).unwrap()
			})
		});
	}
	group.finish();
}

fn bench_vec_bool_roundtrip(c: &mut Criterion) {
	let mut group = c.benchmark_group("vec_bool_roundtrip");

	for size in [100, 1000, 10000].iter() {
		let data: Vec<bool> = (0..*size).map(|i| (i * 7919) % 3 == 0).collect();

		group.bench_with_input(BenchmarkId::new("roundtrip", size), &data, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				let mut cursor = buffer.as_slice();
				let reconstructed: Vec<bool> =
					Vec::<bool>::deserialize_revisioned(&mut cursor).unwrap();
				reconstructed
			})
		});
	}
	group.finish();
}

fn bench_vec_bool_space_efficiency(c: &mut Criterion) {
	let group = c.benchmark_group("vec_bool_space_efficiency");

	// Demonstrate space savings
	for size in [100, 1000, 10000, 100000].iter() {
		let data: Vec<bool> = (0..*size).map(|i| i % 2 == 0).collect();
		let mut buffer = Vec::new();
		data.serialize_revisioned(&mut buffer).unwrap();

		let savings_percent = (1.0 - (buffer.len() as f64 / *size as f64)) * 100.0;

		println!(
			"Vec<bool> size={}: {} bytes (naive would be {} bytes, {:.1}% savings)",
			size,
			buffer.len(),
			size,
			savings_percent
		);
	}

	group.finish();
}

criterion_group!(
	benches,
	bench_vec_bool_serialize,
	bench_vec_bool_deserialize,
	bench_vec_bool_roundtrip,
	bench_vec_bool_space_efficiency
);
criterion_main!(benches);
