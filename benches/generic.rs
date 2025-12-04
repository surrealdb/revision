use bytes::Bytes;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use revision::prelude::*;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet};
use std::time::Duration;

// A comprehensive struct that uses many of the types supported by revision
#[derive(Debug)]
#[revisioned(revision = 1)]
struct ComplexData {
	// Primitive integers
	tiny_int: u8,
	small_int: u16,
	medium_int: u32,
	large_int: u64,
	huge_int: u128,
	size_int: usize,

	// Signed integers
	tiny_signed: i8,
	small_signed: i16,
	medium_signed: i32,
	large_signed: i64,
	huge_signed: i128,
	size_signed: isize,

	// Floating point
	float32: f32,
	float64: f64,

	// Character and boolean
	character: char,
	flag: bool,

	// Strings and collections
	text: String,
	bytes: Bytes,
	bytes_vec: Vec<u8>,
	numbers: Vec<i32>,

	// Additional vector types for comprehensive testing
	boolean_flags: Vec<bool>,
	tiny_signed_vec: Vec<i8>,
	small_unsigned_vec: Vec<u16>,
	large_numbers_vec: Vec<i64>,
	huge_numbers_vec: Vec<u128>,
	float_values_vec: Vec<f32>,
	double_precision_vec: Vec<f64>,
	string_list: Vec<String>,

	// Arrays
	small_array: [u8; 4],
	medium_array: [i32; 16],

	// Optional values
	maybe_text: Option<String>,
	maybe_number: Option<u64>,

	// Boxed values
	boxed_data: String,

	// Tuples
	pair: (String, i32),
	triple: (u8, u16, u32),
	quad: (bool, char, f32, String),
	quint: (i8, i16, i32, i64, String),

	// Collections with complex keys/values
	string_to_int: HashMap<String, i32>,
	ordered_map: BTreeMap<String, Vec<u8>>,
	unique_strings: HashSet<String>,
	ordered_numbers: BTreeSet<i32>,
	priority_queue: BinaryHeap<i32>,

	// Results
	success_result: Result<String, i32>,
	error_result: Result<u8, String>,

	// Duration
	time_duration: Duration,

	// Cow
	borrowed_or_owned: Cow<'static, str>,
}

impl ComplexData {
	fn generate_sample(size_factor: usize) -> Self {
		let mut string_to_int = HashMap::new();
		let mut ordered_map = BTreeMap::new();
		let mut unique_strings = HashSet::new();
		let mut ordered_numbers = BTreeSet::new();
		let mut priority_queue = BinaryHeap::new();

		// Generate collections based on size factor
		for i in 0..size_factor {
			let key = format!("key_{}", i);
			let value = i as i32;

			string_to_int.insert(key.clone(), value);
			ordered_map.insert(key.clone(), vec![i as u8; (i % 10) + 1]);
			unique_strings.insert(key);
			ordered_numbers.insert(value);
			priority_queue.push(value);
		}

		// Create a sample with realistic data
		ComplexData {
			tiny_int: 255,
			small_int: 65535,
			medium_int: 4294967295,
			large_int: 18446744073709551615,
			huge_int: 340282366920938463463374607431768211455,
			size_int: 1000000,

			tiny_signed: -128,
			small_signed: -32768,
			medium_signed: -2147483648,
			large_signed: -9223372036854775808,
			huge_signed: -170141183460469231731687303715884105728,
			size_signed: -500000,

			float32: std::f32::consts::PI,
			float64: std::f64::consts::E,

			character: '🦀',
			flag: true,

			text: "The quick brown fox jumps over the lazy dog".repeat(size_factor / 10 + 1),
			bytes: (0..size_factor).map(|i| (i % 256) as u8).collect(),
			bytes_vec: (0..size_factor).map(|i| (i % 256) as u8).collect(),
			numbers: (0..size_factor).map(|i| i as i32).collect(),

			// Initialize additional vector types with varying patterns
			boolean_flags: (0..size_factor).map(|i| i % 3 == 0).collect(),
			tiny_signed_vec: (0..size_factor)
				.map(|i| (i as i8).wrapping_mul(3).wrapping_sub(100))
				.collect(),
			small_unsigned_vec: (0..size_factor).map(|i| ((i * 7 + 13) % 65536) as u16).collect(),
			large_numbers_vec: (0..size_factor).map(|i| (i as i64) * 1_000_000_007).collect(),
			huge_numbers_vec: (0..size_factor)
				.map(|i| (i as u128).wrapping_mul(340_282_366_920_938_463_463_374_607_431_768_211))
				.collect(),
			float_values_vec: (0..size_factor).map(|i| (i as f32) * 0.01 + 1.414).collect(),
			double_precision_vec: (0..size_factor)
				.map(|i| (i as f64) * 0.001 + std::f64::consts::E)
				.collect(),
			string_list: (0..size_factor)
				.map(|i| match i % 5 {
					0 => format!("item_{}", i),
					1 => format!("long_descriptive_name_with_underscores_{}", i),
					2 => "🦀 Rust".to_string(),
					3 => "αβγδε Greek letters".to_string(),
					4 => format!("mixed_content_{}_{}_end", i, i * 2),
					_ => unreachable!(),
				})
				.collect(),

			small_array: [1, 2, 3, 4],
			medium_array: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],

			maybe_text: Some("Optional string content".to_string()),
			maybe_number: Some(42),

			boxed_data: "Boxed string data".to_string(),

			pair: ("pair_key".to_string(), 123),
			triple: (1, 2, 3),
			quad: (true, 'A', 1.5, "quad_string".to_string()),
			quint: (1, 2, 3, 4, "quint_string".to_string()),

			string_to_int,
			ordered_map,
			unique_strings,
			ordered_numbers,
			priority_queue,

			success_result: Ok("Success message".to_string()),
			error_result: Err("Error message".to_string()),

			time_duration: Duration::from_secs(3600),

			borrowed_or_owned: Cow::Owned("static string".to_string()),
		}
	}
}

fn bench_complex_data_serialization(c: &mut Criterion) {
	let mut group = c.benchmark_group("complex_data_serialization");

	// Test different data sizes
	for size in [10, 100, 1000, 10000, 100000].iter() {
		let data = ComplexData::generate_sample(*size);

		group.bench_with_input(BenchmarkId::new("serialize", size), &data, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});
	}
	group.finish();
}

fn bench_complex_data_deserialization(c: &mut Criterion) {
	let mut group = c.benchmark_group("complex_data_deserialization");

	// Pre-serialize data for deserialization benchmarks
	let serialized_data: Vec<(usize, Vec<u8>)> = [10, 100, 1000, 10000, 100000]
		.iter()
		.map(|&size| {
			let data = ComplexData::generate_sample(size);
			let mut buffer = Vec::new();
			data.serialize_revisioned(&mut buffer).unwrap();
			(size, buffer)
		})
		.collect();

	for (size, buffer) in serialized_data.iter() {
		group.bench_with_input(BenchmarkId::new("deserialize", size), buffer, |b, buffer| {
			b.iter(|| {
				let mut cursor = buffer.as_slice();
				ComplexData::deserialize_revisioned(&mut cursor).unwrap()
			})
		});
	}
	group.finish();
}

fn bench_complex_data_roundtrip(c: &mut Criterion) {
	let mut group = c.benchmark_group("complex_data_roundtrip");

	for size in [10, 100, 1000].iter() {
		let data = ComplexData::generate_sample(*size);

		group.bench_with_input(BenchmarkId::new("roundtrip", size), &data, |b, data| {
			b.iter(|| {
				// Serialize
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();

				// Deserialize
				let mut cursor = buffer.as_slice();
				let reconstructed: ComplexData =
					ComplexData::deserialize_revisioned(&mut cursor).unwrap();

				// Verify integrity for most fields (BinaryHeap doesn't implement PartialEq)
				debug_assert_eq!(data.tiny_int, reconstructed.tiny_int);
				debug_assert_eq!(data.text, reconstructed.text);
				reconstructed
			})
		});
	}
	group.finish();
}

fn bench_size_comparison(c: &mut Criterion) {
	let mut group = c.benchmark_group("size_comparison");

	let data = ComplexData::generate_sample(100);

	// Revision serialization
	let mut revision_buffer = Vec::new();
	data.serialize_revisioned(&mut revision_buffer).unwrap();

	println!("Size comparison for ComplexData(100): Revision={} bytes", revision_buffer.len());

	group.bench_function("revision_serialize", |b| {
		b.iter(|| {
			let mut buffer = Vec::new();
			data.serialize_revisioned(&mut buffer).unwrap();
			buffer
		})
	});

	group.finish();
}

// Vector-specific benchmarks for SIMD optimization testing
fn bench_vec_u8(c: &mut Criterion) {
	let mut group = c.benchmark_group("vec_u8_comparison");

	// Test different vector sizes to see SIMD impact
	for size in [100, 1000, 10000, 100000].iter() {
		let data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();

		group.bench_with_input(BenchmarkId::new("serialize", size), &data, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});

		// Pre-serialize for deserialization bench
		let mut serialized = Vec::new();
		data.serialize_revisioned(&mut serialized).unwrap();

		group.bench_with_input(
			BenchmarkId::new("deserialize", size),
			&serialized,
			|b, serialized| {
				b.iter(|| {
					let mut cursor = serialized.as_slice();
					Vec::<u8>::deserialize_revisioned(&mut cursor).unwrap()
				})
			},
		);
	}
	group.finish();
}

fn bench_vec_i32(c: &mut Criterion) {
	let mut group = c.benchmark_group("vec_i32_comparison");

	for size in [100, 1000, 10000, 100000].iter() {
		let data: Vec<i32> = (0..*size).map(|i| i * 2 - *size).collect();

		group.bench_with_input(BenchmarkId::new("serialize", size), &data, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});

		let mut serialized = Vec::new();
		data.serialize_revisioned(&mut serialized).unwrap();

		group.bench_with_input(
			BenchmarkId::new("deserialize", size),
			&serialized,
			|b, serialized| {
				b.iter(|| {
					let mut cursor = serialized.as_slice();
					Vec::<i32>::deserialize_revisioned(&mut cursor).unwrap()
				})
			},
		);
	}
	group.finish();
}

fn bench_vec_f32(c: &mut Criterion) {
	let mut group = c.benchmark_group("vec_f32_comparison");

	for size in [100, 1000, 10000, 100000].iter() {
		let data: Vec<f32> = (0..*size).map(|i| (i as f32) * 0.1 + std::f32::consts::PI).collect();

		group.bench_with_input(BenchmarkId::new("serialize", size), &data, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});

		let mut serialized = Vec::new();
		data.serialize_revisioned(&mut serialized).unwrap();

		group.bench_with_input(
			BenchmarkId::new("deserialize", size),
			&serialized,
			|b, serialized| {
				b.iter(|| {
					let mut cursor = serialized.as_slice();
					Vec::<f32>::deserialize_revisioned(&mut cursor).unwrap()
				})
			},
		);
	}
	group.finish();
}

fn bench_vec_f64(c: &mut Criterion) {
	let mut group = c.benchmark_group("vec_f64_comparison");

	for size in [100, 1000, 10000, 100000].iter() {
		let data: Vec<f64> = (0..*size).map(|i| (i as f64) * 0.001 + std::f64::consts::E).collect();

		group.bench_with_input(BenchmarkId::new("serialize", size), &data, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});

		let mut serialized = Vec::new();
		data.serialize_revisioned(&mut serialized).unwrap();

		group.bench_with_input(
			BenchmarkId::new("deserialize", size),
			&serialized,
			|b, serialized| {
				b.iter(|| {
					let mut cursor = serialized.as_slice();
					Vec::<f64>::deserialize_revisioned(&mut cursor).unwrap()
				})
			},
		);
	}
	group.finish();
}

fn bench_vec_bool(c: &mut Criterion) {
	let mut group = c.benchmark_group("vec_bool_comparison");

	for size in [100, 1000, 10000, 100000].iter() {
		let data: Vec<bool> = (0..*size).map(|i| i % 2 == 0).collect();

		group.bench_with_input(BenchmarkId::new("serialize", size), &data, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});

		let mut serialized = Vec::new();
		data.serialize_revisioned(&mut serialized).unwrap();

		group.bench_with_input(
			BenchmarkId::new("deserialize", size),
			&serialized,
			|b, serialized| {
				b.iter(|| {
					let mut cursor = serialized.as_slice();
					Vec::<bool>::deserialize_revisioned(&mut cursor).unwrap()
				})
			},
		);
	}
	group.finish();
}

fn bench_vec_string(c: &mut Criterion) {
	let mut group = c.benchmark_group("vec_string_comparison");

	for size in [10, 100, 1000, 10000].iter() {
		let data: Vec<String> = (0..*size)
			.map(|i| match i % 4 {
				0 => format!("short_{}", i),
				1 => format!("medium_length_string_{}", i),
				2 => format!("very_long_string_with_lots_of_content_and_repeated_patterns_{}", i),
				_ => "🦀 unicode string with emoji and special chars: αβγδε 你好世界 🌟✨"
					.to_string(),
			})
			.collect();

		group.bench_with_input(BenchmarkId::new("serialize", size), &data, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});

		let mut serialized = Vec::new();
		data.serialize_revisioned(&mut serialized).unwrap();

		group.bench_with_input(
			BenchmarkId::new("deserialize", size),
			&serialized,
			|b, serialized| {
				b.iter(|| {
					let mut cursor = serialized.as_slice();
					Vec::<String>::deserialize_revisioned(&mut cursor).unwrap()
				})
			},
		);
	}
	group.finish();
}

fn bench_vec_u64(c: &mut Criterion) {
	let mut group = c.benchmark_group("vec_u64_comparison");

	for size in [100, 1000, 10000, 100000].iter() {
		let data: Vec<u64> = (0..*size).map(|i| (i as u64) * 12345678901234567).collect();

		group.bench_with_input(BenchmarkId::new("serialize", size), &data, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});

		let mut serialized = Vec::new();
		data.serialize_revisioned(&mut serialized).unwrap();

		group.bench_with_input(
			BenchmarkId::new("deserialize", size),
			&serialized,
			|b, serialized| {
				b.iter(|| {
					let mut cursor = serialized.as_slice();
					Vec::<u64>::deserialize_revisioned(&mut cursor).unwrap()
				})
			},
		);
	}
	group.finish();
}

fn bench_vec_i16(c: &mut Criterion) {
	let mut group = c.benchmark_group("vec_i16_comparison");

	for size in [100, 1000, 10000, 100000].iter() {
		let data: Vec<i16> =
			(0..*size).map(|i| ((i as i16) * 3 - (*size as i16 / 2)).wrapping_mul(7)).collect();

		group.bench_with_input(BenchmarkId::new("serialize", size), &data, |b, data| {
			b.iter(|| {
				let mut buffer = Vec::new();
				data.serialize_revisioned(&mut buffer).unwrap();
				buffer
			})
		});

		let mut serialized = Vec::new();
		data.serialize_revisioned(&mut serialized).unwrap();

		group.bench_with_input(
			BenchmarkId::new("deserialize", size),
			&serialized,
			|b, serialized| {
				b.iter(|| {
					let mut cursor = serialized.as_slice();
					Vec::<i16>::deserialize_revisioned(&mut cursor).unwrap()
				})
			},
		);
	}
	group.finish();
}

criterion_group!(
	benches,
	bench_complex_data_serialization,
	bench_complex_data_deserialization,
	bench_complex_data_roundtrip,
	bench_size_comparison,
	bench_vec_u8,
	bench_vec_i32,
	bench_vec_f32,
	bench_vec_f64,
	bench_vec_bool,
	bench_vec_string,
	bench_vec_u64,
	bench_vec_i16
);
criterion_main!(benches);
