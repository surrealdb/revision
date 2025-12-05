use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use revision::revisioned;
use std::hint::black_box;

// Custom struct for testing non-specialized types
#[revisioned(revision = 1)]
#[derive(Clone, Debug, PartialEq)]
struct CustomStruct {
	id: u64,
	name: String,
	value: f64,
	active: bool,
}

impl CustomStruct {
	fn new(id: u64) -> Self {
		Self {
			id,
			name: format!("Item_{}", id),
			value: id as f64 * 0.1,
			active: id.is_multiple_of(2),
		}
	}
}

// Generate test data for benchmarking
fn generate_custom_data(size: usize) -> Vec<CustomStruct> {
	(0..size).map(|i| CustomStruct::new(i as u64)).collect()
}

fn generate_i8_data(size: usize) -> Vec<i8> {
	(0..size).map(|i| (i % 256) as i8).collect()
}

fn generate_i32_data(size: usize) -> Vec<i32> {
	(0..size).map(|i| (i as i32).wrapping_sub(size as i32 / 2)).collect()
}

fn generate_i64_data(size: usize) -> Vec<i64> {
	(0..size).map(|i| (i as i64).wrapping_sub(size as i64 / 2)).collect()
}

fn generate_f32_data(size: usize) -> Vec<f32> {
	(0..size).map(|i| (i as f32) * 0.1).collect()
}

fn generate_f64_data(size: usize) -> Vec<f64> {
	(0..size).map(|i| (i as f64) * 0.1).collect()
}

#[cfg(feature = "rust_decimal")]
fn generate_decimal_data(size: usize) -> Vec<rust_decimal::Decimal> {
	use rust_decimal::Decimal;
	(0..size)
		.map(|i| Decimal::new(i as i64, 2)) // Creates decimal with 2 decimal places
		.collect()
}

fn benchmark_i8_serialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<i8> Serialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_i8_data(size);
		// i8 is 1 byte per element
		group.throughput(Throughput::Bytes(size as u64));

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});
	}
	group.finish();
}

fn benchmark_i8_deserialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<i8> Deserialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_i8_data(size);
		group.throughput(Throughput::Bytes(size as u64));

		// Pre-serialize data for deserialization benchmarks
		let serialized = revision::to_vec(&data).unwrap();

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<i8> = revision::from_slice(black_box(&serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

fn benchmark_i32_serialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<i32> Serialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_i32_data(size);
		// i32 is 4 bytes per element
		group.throughput(Throughput::Bytes((size * 4) as u64));

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});
	}
	group.finish();
}

fn benchmark_i32_deserialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<i32> Deserialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_i32_data(size);
		group.throughput(Throughput::Bytes((size * 4) as u64));

		let serialized = revision::to_vec(&data).unwrap();

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<i32> = revision::from_slice(black_box(&serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

fn benchmark_i64_serialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<i64> Serialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_i64_data(size);
		// i64 is 8 bytes per element
		group.throughput(Throughput::Bytes((size * 8) as u64));

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});
	}
	group.finish();
}

fn benchmark_i64_deserialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<i64> Deserialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_i64_data(size);
		group.throughput(Throughput::Bytes((size * 8) as u64));

		let serialized = revision::to_vec(&data).unwrap();

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<i64> = revision::from_slice(black_box(&serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

fn benchmark_f64_serialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<f64> Serialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_f64_data(size);
		// f64 is 8 bytes per element
		group.throughput(Throughput::Bytes((size * 8) as u64));

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});
	}
	group.finish();
}

fn benchmark_f64_deserialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<f64> Deserialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_f64_data(size);
		group.throughput(Throughput::Bytes((size * 8) as u64));

		let serialized = revision::to_vec(&data).unwrap();

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<f64> = revision::from_slice(black_box(&serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

fn benchmark_custom_struct_serialization(c: &mut Criterion) {
	let sizes = [100, 10_000];

	let mut group = c.benchmark_group("Vec<CustomStruct> Serialization (non-specialized)");

	for &size in &sizes {
		let data = generate_custom_data(size);
		// Estimate: ~30 bytes per struct (id:8 + name:~10 + value:8 + active:1)
		group.throughput(Throughput::Bytes((size * 30) as u64));

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});
	}
	group.finish();
}

fn benchmark_custom_struct_deserialization(c: &mut Criterion) {
	let sizes = [100, 10_000];

	let mut group = c.benchmark_group("Vec<CustomStruct> Deserialization (non-specialized)");

	for &size in &sizes {
		let data = generate_custom_data(size);
		group.throughput(Throughput::Bytes((size * 30) as u64));

		let serialized = revision::to_vec(&data).unwrap();

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<CustomStruct> =
					revision::from_slice(black_box(&serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

fn benchmark_f32_serialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<f32> Serialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_f32_data(size);
		// f32 is 4 bytes per element
		group.throughput(Throughput::Bytes((size * 4) as u64));

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});
	}
	group.finish();
}

fn benchmark_f32_deserialization(c: &mut Criterion) {
	let sizes = [100, 10_000, 1_000_000];

	let mut group = c.benchmark_group("Vec<f32> Deserialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_f32_data(size);
		group.throughput(Throughput::Bytes((size * 4) as u64));

		let serialized = revision::to_vec(&data).unwrap();

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<f32> = revision::from_slice(black_box(&serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

#[cfg(feature = "rust_decimal")]
fn benchmark_decimal_serialization(c: &mut Criterion) {
	let sizes = [100, 10_000];

	let mut group = c.benchmark_group("Vec<Decimal> Serialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_decimal_data(size);
		// Decimal is 16 bytes
		group.throughput(Throughput::Bytes((size * 16) as u64));

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let serialized = revision::to_vec(black_box(&data)).unwrap();
				black_box(serialized)
			})
		});
	}
	group.finish();
}

#[cfg(feature = "rust_decimal")]
fn benchmark_decimal_deserialization(c: &mut Criterion) {
	use rust_decimal::Decimal;

	let sizes = [100, 10_000];

	let mut group = c.benchmark_group("Vec<Decimal> Deserialization (specialisation comparison)");

	for &size in &sizes {
		let data = generate_decimal_data(size);
		group.throughput(Throughput::Bytes((size * 16) as u64));

		let serialized = revision::to_vec(&data).unwrap();

		group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
			b.iter(|| {
				let deserialized: Vec<Decimal> =
					revision::from_slice(black_box(&serialized)).unwrap();
				black_box(deserialized)
			})
		});
	}
	group.finish();
}

criterion_group!(
	benches,
	benchmark_i8_serialization,
	benchmark_i8_deserialization,
	benchmark_i32_serialization,
	benchmark_i32_deserialization,
	benchmark_i64_serialization,
	benchmark_i64_deserialization,
	benchmark_f64_serialization,
	benchmark_f64_deserialization,
	benchmark_f32_serialization,
	benchmark_f32_deserialization,
	benchmark_custom_struct_serialization,
	benchmark_custom_struct_deserialization
);

#[cfg(feature = "rust_decimal")]
criterion_group!(
	benches_decimal,
	benchmark_decimal_serialization,
	benchmark_decimal_deserialization
);

#[cfg(feature = "rust_decimal")]
criterion_main!(benches, benches_decimal);

#[cfg(not(feature = "rust_decimal"))]
criterion_main!(benches);
