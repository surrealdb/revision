//! Varint encoding performance benchmarks
//!
//! This benchmark tests varint (variable-length integer) encoding performance by wrapping
//! integers in structs. This is necessary because `Vec<primitive>` uses specialized
//! implementations (direct memory copy on little-endian platforms) rather than varint encoding.
//!
//! By wrapping integers in a `Vec<Struct>` where the struct contains a single integer field,
//! we force the use of the generic serialization path which applies varint encoding to integers.
//!
//! The benchmarks test three distributions:
//! - **Small**: Values 0-250 (mostly 1-byte varint encoding)
//! - **Large**: Values near type MAX (maximum varint bytes)
//! - **Mixed**: 70% small, 20% medium, 10% large values (realistic distribution)

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rand::Rng;
use revision::prelude::*;
use std::hint::black_box;

// Wrapper structs to force generic (varint) serialization path
// These prevent the specialized Vec<T> implementations from being used

#[derive(Debug)]
#[revisioned(revision = 1)]
struct WrappedU16 {
	values: Vec<InnerU16>,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct InnerU16 {
	value: u16,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct WrappedU32 {
	values: Vec<InnerU32>,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct InnerU32 {
	value: u32,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct WrappedU64 {
	values: Vec<InnerU64>,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct InnerU64 {
	value: u64,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct WrappedU128 {
	values: Vec<InnerU128>,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct InnerU128 {
	value: u128,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct WrappedI16 {
	values: Vec<InnerI16>,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct InnerI16 {
	value: i16,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct WrappedI32 {
	values: Vec<InnerI32>,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct InnerI32 {
	value: i32,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct WrappedI64 {
	values: Vec<InnerI64>,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct InnerI64 {
	value: i64,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct WrappedI128 {
	values: Vec<InnerI128>,
}

#[derive(Debug)]
#[revisioned(revision = 1)]
struct InnerI128 {
	value: i128,
}

// Data distribution generators

/// Generate small values (0-250) where varint uses 1 byte
fn generate_small_values<T>(size: usize) -> Vec<T>
where
	T: From<u8> + Copy,
{
	let mut rng = rand::rng();
	(0..size).map(|_| T::from(rng.random_range(0u8..=250u8))).collect()
}

/// Generate large values near type MAX where varint has overhead
fn generate_large_u16(size: usize) -> Vec<u16> {
	let mut rng = rand::rng();
	(0..size).map(|_| rng.random_range(u16::MAX - 10000..=u16::MAX)).collect()
}

fn generate_large_u32(size: usize) -> Vec<u32> {
	let mut rng = rand::rng();
	(0..size).map(|_| rng.random_range(u32::MAX - 100000..=u32::MAX)).collect()
}

fn generate_large_u64(size: usize) -> Vec<u64> {
	let mut rng = rand::rng();
	(0..size).map(|_| rng.random_range(u64::MAX - 1000000..=u64::MAX)).collect()
}

fn generate_large_u128(size: usize) -> Vec<u128> {
	let mut rng = rand::rng();
	(0..size).map(|_| rng.random_range(u128::MAX - 10000000..=u128::MAX)).collect()
}

fn generate_large_i16(size: usize) -> Vec<i16> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			if rng.random_bool(0.5) {
				rng.random_range(i16::MIN..=i16::MIN + 10000)
			} else {
				rng.random_range(i16::MAX - 10000..=i16::MAX)
			}
		})
		.collect()
}

fn generate_large_i32(size: usize) -> Vec<i32> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			if rng.random_bool(0.5) {
				rng.random_range(i32::MIN..=i32::MIN + 100000)
			} else {
				rng.random_range(i32::MAX - 100000..=i32::MAX)
			}
		})
		.collect()
}

fn generate_large_i64(size: usize) -> Vec<i64> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			if rng.random_bool(0.5) {
				rng.random_range(i64::MIN..=i64::MIN + 1000000)
			} else {
				rng.random_range(i64::MAX - 1000000..=i64::MAX)
			}
		})
		.collect()
}

fn generate_large_i128(size: usize) -> Vec<i128> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			if rng.random_bool(0.5) {
				rng.random_range(i128::MIN..=i128::MIN + 10000000)
			} else {
				rng.random_range(i128::MAX - 10000000..=i128::MAX)
			}
		})
		.collect()
}

/// Generate mixed distribution: 70% small, 20% medium, 10% large
fn generate_mixed_u16(size: usize) -> Vec<u16> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			let p: f64 = rng.random();
			if p < 0.7 {
				rng.random_range(0..=250) // Small
			} else if p < 0.9 {
				rng.random_range(251..=10000) // Medium
			} else {
				rng.random_range(10001..=u16::MAX) // Large
			}
		})
		.collect()
}

fn generate_mixed_u32(size: usize) -> Vec<u32> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			let p: f64 = rng.random();
			if p < 0.7 {
				rng.random_range(0..=250) // Small
			} else if p < 0.9 {
				rng.random_range(251..=100000) // Medium
			} else {
				rng.random_range(100001..=u32::MAX) // Large
			}
		})
		.collect()
}

fn generate_mixed_u64(size: usize) -> Vec<u64> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			let p: f64 = rng.random();
			if p < 0.7 {
				rng.random_range(0..=250) // Small
			} else if p < 0.9 {
				rng.random_range(251..=1000000) // Medium
			} else {
				rng.random_range(1000001..=u64::MAX) // Large
			}
		})
		.collect()
}

fn generate_mixed_u128(size: usize) -> Vec<u128> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			let p: f64 = rng.random();
			if p < 0.7 {
				rng.random_range(0..=250) // Small
			} else if p < 0.9 {
				rng.random_range(251..=10000000) // Medium
			} else {
				rng.random_range(10000001..=u128::MAX) // Large
			}
		})
		.collect()
}

fn generate_mixed_i16(size: usize) -> Vec<i16> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			let p: f64 = rng.random();
			if p < 0.7 {
				rng.random_range(-125..=125) // Small
			} else if p < 0.9 {
				rng.random_range(-5000..=5000) // Medium
			} else if rng.random_bool(0.5) {
				rng.random_range(i16::MIN..=-5001) // Large negative
			} else {
				rng.random_range(5001..=i16::MAX) // Large positive
			}
		})
		.collect()
}

fn generate_mixed_i32(size: usize) -> Vec<i32> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			let p: f64 = rng.random();
			if p < 0.7 {
				rng.random_range(-125..=125) // Small
			} else if p < 0.9 {
				rng.random_range(-50000..=50000) // Medium
			} else if rng.random_bool(0.5) {
				rng.random_range(i32::MIN..=-50001) // Large negative
			} else {
				rng.random_range(50001..=i32::MAX) // Large positive
			}
		})
		.collect()
}

fn generate_mixed_i64(size: usize) -> Vec<i64> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			let p: f64 = rng.random();
			if p < 0.7 {
				rng.random_range(-125..=125) // Small
			} else if p < 0.9 {
				rng.random_range(-500000..=500000) // Medium
			} else if rng.random_bool(0.5) {
				rng.random_range(i64::MIN..=-500001) // Large negative
			} else {
				rng.random_range(500001..=i64::MAX) // Large positive
			}
		})
		.collect()
}

fn generate_mixed_i128(size: usize) -> Vec<i128> {
	let mut rng = rand::rng();
	(0..size)
		.map(|_| {
			let p: f64 = rng.random();
			if p < 0.7 {
				rng.random_range(-125..=125) // Small
			} else if p < 0.9 {
				rng.random_range(-5000000..=5000000) // Medium
			} else if rng.random_bool(0.5) {
				rng.random_range(i128::MIN..=-5000001) // Large negative
			} else {
				rng.random_range(5000001..=i128::MAX) // Large positive
			}
		})
		.collect()
}

// Benchmark macros

macro_rules! bench_unsigned {
	($name:ident, $ty:ident, $size_multiplier:expr) => {
		paste::item! {
			fn [<benchmark_ $name _serialization>](c: &mut Criterion) {
				let sizes = [100, 10_000, 1_000_000];
				let mut group = c.benchmark_group(format!("{} Varint Serialization", stringify!([<$ty>])));

				for &size in &sizes {
					// Small values - wrapped in struct to force varint encoding
					let raw_data = generate_small_values::<$ty>(size);
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					group.throughput(Throughput::Bytes((size * $size_multiplier) as u64));
					group.bench_with_input(BenchmarkId::new("Small", size), &data, |b, data| {
						b.iter(|| {
							let serialized = revision::to_vec(black_box(data)).unwrap();
							black_box(serialized)
						})
					});

					// Large values
					let raw_data = [<generate_large_ $ty>](size);
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					group.bench_with_input(BenchmarkId::new("Large", size), &data, |b, data| {
						b.iter(|| {
							let serialized = revision::to_vec(black_box(data)).unwrap();
							black_box(serialized)
						})
					});

					// Mixed values
					let raw_data = [<generate_mixed_ $ty>](size);
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					group.bench_with_input(BenchmarkId::new("Mixed", size), &data, |b, data| {
						b.iter(|| {
							let serialized = revision::to_vec(black_box(data)).unwrap();
							black_box(serialized)
						})
					});
				}
				group.finish();
			}

			fn [<benchmark_ $name _deserialization>](c: &mut Criterion) {
				let sizes = [100, 10_000, 1_000_000];
				let mut group = c.benchmark_group(format!("{} Varint Deserialization", stringify!([<$ty>])));

				for &size in &sizes {
					group.throughput(Throughput::Bytes((size * $size_multiplier) as u64));

					// Small values
					let raw_data = generate_small_values::<$ty>(size);
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					let serialized = revision::to_vec(&data).unwrap();
					group.bench_with_input(BenchmarkId::new("Small", size), &serialized, |b, serialized| {
						b.iter(|| {
							let deserialized: [<Wrapped $ty:upper>] = revision::from_slice(black_box(serialized)).unwrap();
							black_box(deserialized)
						})
					});

					// Large values
					let raw_data = [<generate_large_ $ty>](size);
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					let serialized = revision::to_vec(&data).unwrap();
					group.bench_with_input(BenchmarkId::new("Large", size), &serialized, |b, serialized| {
						b.iter(|| {
							let deserialized: [<Wrapped $ty:upper>] = revision::from_slice(black_box(serialized)).unwrap();
							black_box(deserialized)
						})
					});

					// Mixed values
					let raw_data = [<generate_mixed_ $ty>](size);
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					let serialized = revision::to_vec(&data).unwrap();
					group.bench_with_input(BenchmarkId::new("Mixed", size), &serialized, |b, serialized| {
						b.iter(|| {
							let deserialized: [<Wrapped $ty:upper>] = revision::from_slice(black_box(serialized)).unwrap();
							black_box(deserialized)
						})
					});
				}
				group.finish();
			}
		}
	};
}

macro_rules! bench_signed {
	($name:ident, $ty:ident, $size_multiplier:expr) => {
		paste::item! {
			fn [<benchmark_ $name _serialization>](c: &mut Criterion) {
				let sizes = [100, 10_000, 1_000_000];
				let mut group = c.benchmark_group(format!("{} Varint Serialization", stringify!([<$ty>])));

				for &size in &sizes {
					group.throughput(Throughput::Bytes((size * $size_multiplier) as u64));

					// Small values - wrapped in struct to force varint encoding
					let raw_data: Vec<$ty> = (0..size).map(|i| ((i % 250) as i32 - 125) as $ty).collect();
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					group.bench_with_input(BenchmarkId::new("Small", size), &data, |b, data| {
						b.iter(|| {
							let serialized = revision::to_vec(black_box(data)).unwrap();
							black_box(serialized)
						})
					});

					// Large values
					let raw_data = [<generate_large_ $ty>](size);
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					group.bench_with_input(BenchmarkId::new("Large", size), &data, |b, data| {
						b.iter(|| {
							let serialized = revision::to_vec(black_box(data)).unwrap();
							black_box(serialized)
						})
					});

					// Mixed values
					let raw_data = [<generate_mixed_ $ty>](size);
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					group.bench_with_input(BenchmarkId::new("Mixed", size), &data, |b, data| {
						b.iter(|| {
							let serialized = revision::to_vec(black_box(data)).unwrap();
							black_box(serialized)
						})
					});
				}
				group.finish();
			}

			fn [<benchmark_ $name _deserialization>](c: &mut Criterion) {
				let sizes = [100, 10_000, 1_000_000];
				let mut group = c.benchmark_group(format!("{} Varint Deserialization", stringify!([<$ty>])));

				for &size in &sizes {
					group.throughput(Throughput::Bytes((size * $size_multiplier) as u64));

					// Small values
					let raw_data: Vec<$ty> = (0..size).map(|i| ((i % 250) as i32 - 125) as $ty).collect();
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					let serialized = revision::to_vec(&data).unwrap();
					group.bench_with_input(BenchmarkId::new("Small", size), &serialized, |b, serialized| {
						b.iter(|| {
							let deserialized: [<Wrapped $ty:upper>] = revision::from_slice(black_box(serialized)).unwrap();
							black_box(deserialized)
						})
					});

					// Large values
					let raw_data = [<generate_large_ $ty>](size);
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					let serialized = revision::to_vec(&data).unwrap();
					group.bench_with_input(BenchmarkId::new("Large", size), &serialized, |b, serialized| {
						b.iter(|| {
							let deserialized: [<Wrapped $ty:upper>] = revision::from_slice(black_box(serialized)).unwrap();
							black_box(deserialized)
						})
					});

					// Mixed values
					let raw_data = [<generate_mixed_ $ty>](size);
					let data = [<Wrapped $ty:upper>] {
						values: raw_data.into_iter().map(|value| [<Inner $ty:upper>] { value }).collect(),
					};
					let serialized = revision::to_vec(&data).unwrap();
					group.bench_with_input(BenchmarkId::new("Mixed", size), &serialized, |b, serialized| {
						b.iter(|| {
							let deserialized: [<Wrapped $ty:upper>] = revision::from_slice(black_box(serialized)).unwrap();
							black_box(deserialized)
						})
					});
				}
				group.finish();
			}
		}
	};
}

// Generate benchmarks for all integer types
bench_unsigned!(u16, u16, 2);
bench_unsigned!(u32, u32, 4);
bench_unsigned!(u64, u64, 8);
bench_unsigned!(u128, u128, 16);

bench_signed!(i16, i16, 2);
bench_signed!(i32, i32, 4);
bench_signed!(i64, i64, 8);
bench_signed!(i128, i128, 16);

criterion_group!(benches_u16, benchmark_u16_serialization, benchmark_u16_deserialization);
criterion_group!(benches_u32, benchmark_u32_serialization, benchmark_u32_deserialization);
criterion_group!(benches_u64, benchmark_u64_serialization, benchmark_u64_deserialization);
criterion_group!(benches_u128, benchmark_u128_serialization, benchmark_u128_deserialization);
criterion_group!(benches_i16, benchmark_i16_serialization, benchmark_i16_deserialization);
criterion_group!(benches_i32, benchmark_i32_serialization, benchmark_i32_deserialization);
criterion_group!(benches_i64, benchmark_i64_serialization, benchmark_i64_deserialization);
criterion_group!(benches_i128, benchmark_i128_serialization, benchmark_i128_deserialization);

criterion_main!(
	benches_u16,
	benches_u32,
	benches_u64,
	benches_u128,
	benches_i16,
	benches_i32,
	benches_i64,
	benches_i128
);
