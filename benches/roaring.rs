use bincode::Options;
use criterion::{criterion_group, criterion_main, Criterion};
use rand::random;
use roaring::RoaringTreemap;
use std::time::SystemTime;

fn bench_roaring_serialization_time_benchmark() {
	let mut val = RoaringTreemap::new();
	for i in 0..1_000_000 {
		if random() {
			val.insert(i);
		}
	}
	// COLLECTING ELAPSED TIME

	//Bincode with default options is slower than direct serialization
	let bincode_elapsed;
	{
		let mut mem: Vec<u8> = vec![];
		let t = SystemTime::now();
		bincode::serialize_into(&mut mem, &val).unwrap();
		bincode_elapsed = t.elapsed().unwrap();
	}
	//Bincode with options is: As fast, but still bigger than direct serialization
	let bincode_options_elapsed;
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
	}
	//Direct serialization  is : Faster and smaller
	let direct_elapsed;
	{
		let mut mem: Vec<u8> = vec![];
		let t = SystemTime::now();
		val.serialize_into(&mut mem).unwrap();
		direct_elapsed = t.elapsed().unwrap();
	}

	// ASSERTIONS

	println!("Bincode::default, Bincode::options, Direct, Ratio direct/bincode::options");
	// Direct is faster
	println!(
		"Elapsed - {} > {} > {} - {}",
		bincode_elapsed.as_micros(),
		bincode_options_elapsed.as_micros(),
		direct_elapsed.as_micros(),
		direct_elapsed.as_micros() as f32 / bincode_options_elapsed.as_micros() as f32
	);
	assert!(direct_elapsed < bincode_elapsed);
	assert!((direct_elapsed.as_micros() as f32 / bincode_options_elapsed.as_micros() as f32) < 1.1);
}

fn criterion_benchmark(c: &mut Criterion) {
	c.bench_function("bench_roaring_serialization_time_benchmark", |b| {
		b.iter(bench_roaring_serialization_time_benchmark)
	});
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
