use bincode::Options;
use criterion::{Criterion, criterion_group, criterion_main};
use rand::random;
use roaring::RoaringTreemap;
use std::time::SystemTime;

fn bench_roaring_serialization_benchmark() {
	let mut val = RoaringTreemap::new();
	for i in 0..1_000_000 {
		if random() {
			val.insert(i);
		}
	}
	// COLLECTING ELAPSED TIME AND SIZE

	//Bincode with default options is: Slower and bigger than direct serialization
	let bincode_elapsed;
	let bincode_size;
	{
		let mut mem: Vec<u8> = vec![];
		let t = SystemTime::now();
		bincode::serialize_into(&mut mem, &val).unwrap();
		bincode_elapsed = t.elapsed().unwrap();
		bincode_size = mem.len();
	}
	//Bincode with options is: As fast, but still bigger than direct serialization
	let bincode_options_elapsed;
	let bincode_options_size;
	{
		let mut mem: Vec<u8> = vec![];
		let t = SystemTime::now();
		bincode::options()
			.with_no_limit()
			.with_little_endian()
			.with_varint_encoding()
			.reject_trailing_bytes()
			.serialize_into(&mut mem, &val)
			.unwrap();
		bincode_options_elapsed = t.elapsed().unwrap();
		bincode_options_size = mem.len();
	}
	//Direct serialization  is : Faster and smaller
	let direct_elapsed;
	let direct_size;
	{
		let mut mem: Vec<u8> = vec![];
		let t = SystemTime::now();
		val.serialize_into(&mut mem).unwrap();
		direct_elapsed = t.elapsed().unwrap();
		direct_size = mem.len();
	}

	// ASSERTIONS
	assert!(
		direct_elapsed < bincode_elapsed,
		"direct_elapsed({direct_elapsed:?}) < bincode_elapsed({bincode_elapsed:?})"
	);
	let rate = direct_elapsed.as_micros() as f32 / bincode_options_elapsed.as_micros() as f32;
	assert!(rate < 1.1, "rate({rate}) < 1.1");
	// Direct is smaller
	assert!(
		direct_size < bincode_size,
		"direct_size({direct_size}) < bincode_size({bincode_size})"
	);
	assert!(
		direct_size < bincode_options_size,
		"direct_size({direct_size}) < bincode_options_size({bincode_options_size})"
	);
}

fn roaring_benchmark(c: &mut Criterion) {
	let mut group = c.benchmark_group("roaring_benchmark");
	group.sample_size(10);
	group.bench_function("bench_roaring_serialization_benchmark", |b| {
		b.iter(bench_roaring_serialization_benchmark)
	});
	group.finish();
}

criterion_group!(benches, roaring_benchmark);
criterion_main!(benches);
