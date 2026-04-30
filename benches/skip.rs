use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use revision::{
	DeserializeRevisioned, revisioned, skip_check_slice, skip_reader, skip_slice, to_vec,
};
use std::hint::black_box;
use std::io::Cursor;

#[revisioned(revision = 1)]
#[derive(Debug)]
struct BenchPayload {
	n: u32,
	s: String,
	v: Vec<i64>,
}

fn skip_benchmarks(c: &mut Criterion) {
	let payload = BenchPayload {
		n: 0xa5a5_feef,
		s: "lorem ipsum revision skip bench".into(),
		v: (-64i64..64).collect(),
	};
	let bytes = to_vec(&payload).unwrap();
	let mut grp = c.benchmark_group("skip_vs_deserialize");
	grp.throughput(criterion::Throughput::Elements(1));

	grp.bench_function(BenchmarkId::from_parameter("deserialize"), |b| {
		b.iter(|| {
			let mut r = black_box(bytes.as_slice());
			let _: BenchPayload = BenchPayload::deserialize_revisioned(&mut r).unwrap();
		});
	});

	grp.bench_function(BenchmarkId::from_parameter("skip_slice"), |b| {
		b.iter(|| {
			let n = skip_slice::<BenchPayload>(black_box(&bytes)).unwrap();
			assert_eq!(n, bytes.len());
		});
	});

	grp.bench_function(BenchmarkId::from_parameter("skip_reader_cursor"), |b| {
		b.iter(|| {
			let mut cur = Cursor::new(black_box(bytes.as_slice()));
			skip_reader::<BenchPayload, _>(&mut cur).unwrap();
		});
	});

	grp.bench_function(BenchmarkId::from_parameter("skip_check_slice"), |b| {
		b.iter(|| {
			let n = skip_check_slice::<BenchPayload>(black_box(&bytes)).unwrap();
			assert_eq!(n, bytes.len());
		});
	});

	grp.finish();
}

criterion_group!(benches, skip_benchmarks);
criterion_main!(benches);
