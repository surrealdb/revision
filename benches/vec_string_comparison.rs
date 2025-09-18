use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

// Generate test data for benchmarking
fn generate_test_strings(size: usize) -> Vec<String> {
	let patterns = [
        "hello",
        "world",
        "this is a longer string with more content",
        "short",
        "medium length string with some data",
        "very very very very very very very very long string with lots and lots of content that should help test the performance characteristics",
        "unicode: 🚀🔥✨🌟💫⭐",
        "",
        "special chars: !@#$%^&*()_+-=[]{}|;':\",./<>?",
        "numbers and text: 123456789 mixed content 987654321",
    ];

	(0..size).map(|i| patterns[i % patterns.len()].to_string()).collect()
}

fn benchmark_serialization(c: &mut Criterion) {
	let sizes = [10, 100, 1000, 10000];

	let mut group = c.benchmark_group("Vec<String> Serialization");

	for &size in &sizes {
		let data = generate_test_strings(size);

		// Calculate approximate byte size for throughput
		let total_bytes: usize =
			data.iter().map(|s| s.len()).sum::<usize>() + data.len() * std::mem::size_of::<usize>();
		group.throughput(Throughput::Bytes(total_bytes as u64));

		// Benchmark Vec<String> serialization
		group.bench_with_input(BenchmarkId::new("Vec<String>", size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});
	}
	group.finish();
}

fn benchmark_deserialization(c: &mut Criterion) {
	let sizes = [10, 100, 1000, 10000];

	let mut group = c.benchmark_group("Vec<String> Deserialization");

	for &size in &sizes {
		let data = generate_test_strings(size);

		// Calculate approximate byte size for throughput
		let total_bytes: usize =
			data.iter().map(|s| s.len()).sum::<usize>() + data.len() * std::mem::size_of::<usize>();
		group.throughput(Throughput::Bytes(total_bytes as u64));

		// Pre-serialize data for deserialization benchmarks
		let serialized = revision::to_vec(&data).unwrap();

		group.bench_with_input(BenchmarkId::new("Regular", size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<String> =
					revision::from_slice(black_box(&serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

fn benchmark_roundtrip(c: &mut Criterion) {
	let sizes = [10, 100, 1000, 10000];

	let mut group = c.benchmark_group("Vec<String> Roundtrip");

	for &size in &sizes {
		let data = generate_test_strings(size);

		// Calculate approximate byte size for throughput
		let total_bytes: usize =
			data.iter().map(|s| s.len()).sum::<usize>() + data.len() * std::mem::size_of::<usize>();
		group.throughput(Throughput::Bytes(total_bytes as u64));

		group.bench_with_input(BenchmarkId::new("Regular", size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				let deserialized: Vec<String> =
					revision::from_slice(black_box(&serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

// Benchmark with different string characteristics
fn benchmark_string_patterns(c: &mut Criterion) {
	let mut group = c.benchmark_group("Vec<String> Pattern Analysis");

	let test_cases = [
		("short_strings", (0..1000).map(|i| format!("{}", i % 100)).collect::<Vec<_>>()),
		("long_strings", (0..100).map(|i| "x".repeat(100 + i).to_string()).collect::<Vec<_>>()),
		("mixed_lengths", {
			let mut vec = Vec::new();
			for i in 0..1000 {
				let len = match i % 4 {
					0 => 5,   // short
					1 => 25,  // medium
					2 => 100, // long
					_ => 200, // very long
				};
				vec.push("a".repeat(len));
			}
			vec
		}),
		(
			"unicode_heavy",
			(0..500).map(|i| format!("🚀{}✨{}🔥", i, "🌟".repeat(i % 10 + 1))).collect::<Vec<_>>(),
		),
	];

	for (pattern_name, data) in test_cases.iter() {
		// Calculate approximate byte size for throughput
		let total_bytes: usize =
			data.iter().map(|s| s.len()).sum::<usize>() + data.len() * std::mem::size_of::<usize>();
		group.throughput(Throughput::Bytes(total_bytes as u64));

		// Pre-serialize for deserialization benchmark
		let serialized = revision::to_vec(data).unwrap();

		group.bench_with_input(
			BenchmarkId::new(format!("{}_regular", pattern_name), "deserialize"),
			pattern_name,
			|b, _| {
				b.iter(|| {
					let deserialized: Vec<String> =
						revision::from_slice(black_box(&serialized)).unwrap();
					black_box(deserialized)
				})
			},
		);
	}
	group.finish();
}

criterion_group!(
	benches,
	benchmark_serialization,
	benchmark_deserialization,
	benchmark_roundtrip,
	benchmark_string_patterns
);
criterion_main!(benches);
