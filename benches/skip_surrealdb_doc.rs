//! **SurrealDB-style** stored row skip vs full decode: heterogeneous `Value`-like enum entries in a
//! string-keyed map (`VecMap` / `BTreeMap` revision wire), late `Int` field probe.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use revision::{DeserializeRevisioned, Error, SkipRevisioned, revisioned, to_vec};
use std::collections::BTreeMap;
use std::hint::black_box;

/// Subset of heterogeneous `Value`-like variants (discriminants match declaration order).
#[revisioned(revision = 1)]
#[derive(Debug, Clone)]
enum SurrealBenchValue {
	Nope,
	Nullish,
	Bool(bool),
	Int(i64),
	Str(String),
	Arr(Vec<SurrealBenchValue>),
	Obj(DocumentBodyBench),
}

/// In-memory object = `VecMap<String, SurrealBenchValue>` wire (see surrealdb_collections).
#[revisioned(revision = 1)]
#[derive(Debug, Clone)]
struct DocumentBodyBench {
	entries: BTreeMap<String, SurrealBenchValue>,
}

/// Top-level stored value: unset row vs document body — analogous to `Value::None` vs `Value::Object`.
#[revisioned(revision = 1)]
#[derive(Debug, Clone)]
enum StoredDocBench {
	Unset,
	Doc(DocumentBodyBench),
}

/// Decl order: `Nope`, `Nullish`, `Bool`, `Int`, `Str`, `Arr`, `Obj` → `Int` is `3`.
const SURREAL_DISC_INT: u32 = 3;
/// Decl order: `Unset`, `Doc` → `Doc` is `1`.
const STORED_DISC_DOC: u32 = 1;

/// Field name chosen to sort after typical `field_*` and `"m_*"` prefixes.
const SURREAL_TARGET_FIELD: &str = "z_latency_ms";

fn build_surreal_style_doc(target_ms: i64) -> StoredDocBench {
	let mut entries = BTreeMap::new();
	const DOC_ROWS: usize = 96;
	const FIELD_STR_LEN: usize = 384;
	for i in 0..DOC_ROWS {
		entries.insert(format!("field_{i:04}"), SurrealBenchValue::Str("s".repeat(FIELD_STR_LEN)));
	}
	let arr_elems: Vec<SurrealBenchValue> =
		(0..48).map(|t| SurrealBenchValue::Str(format!("tok_{t}|{}", "β".repeat(64)))).collect();
	entries.insert("m_tokens".into(), SurrealBenchValue::Arr(arr_elems));
	entries.insert(SURREAL_TARGET_FIELD.into(), SurrealBenchValue::Int(target_ms));
	StoredDocBench::Doc(DocumentBodyBench {
		entries,
	})
}

fn read_latency_ms_via_skip(mut reader: &[u8]) -> Result<i64, Error> {
	let _stored_rev = u16::deserialize_revisioned(&mut reader)?;
	let variant = u32::deserialize_revisioned(&mut reader)?;
	if variant != STORED_DISC_DOC {
		return Err(Error::Deserialize("stored doc bench: expected Doc root variant".into()));
	}
	let _body_rev = u16::deserialize_revisioned(&mut reader)?;
	let len = usize::deserialize_revisioned(&mut reader)?;
	for _ in 0..len {
		let k = String::deserialize_revisioned(&mut reader)?;
		if k == SURREAL_TARGET_FIELD {
			let disc = u32::deserialize_revisioned(&mut reader)?;
			if disc != SURREAL_DISC_INT {
				return Err(Error::Deserialize("stored doc bench: bad value variant".into()));
			}
			return i64::deserialize_revisioned(&mut reader);
		}
		SurrealBenchValue::skip_revisioned(&mut reader)?;
	}
	Err(Error::Deserialize("stored doc bench: latency field missing".into()))
}

fn read_latency_ms_full(mut bytes: &[u8]) -> i64 {
	let doc = StoredDocBench::deserialize_revisioned(&mut bytes).unwrap();
	match doc {
		StoredDocBench::Doc(body) => {
			match body.entries.get(SURREAL_TARGET_FIELD).expect("latency key") {
				SurrealBenchValue::Int(v) => *v,
				_ => panic!("expected Int field"),
			}
		}
		StoredDocBench::Unset => panic!("unexpected Unset variant"),
	}
}

fn surrealdb_style_doc_predicate_benches(c: &mut Criterion) {
	let expected = 284_019_763_928_334_772_i64;
	let payload = build_surreal_style_doc(expected);
	let bytes = to_vec(&payload).unwrap();

	assert_eq!(read_latency_ms_full(bytes.as_slice()), expected);
	assert_eq!(read_latency_ms_via_skip(bytes.as_slice()).unwrap(), expected);

	let mut grp = c.benchmark_group("surrealdb_style_doc_predicate_int64");
	grp.throughput(Throughput::Bytes(bytes.len() as u64));

	grp.bench_function(BenchmarkId::from_parameter("deserialize_full_then_get_field"), |b| {
		b.iter(|| {
			let ms = read_latency_ms_full(black_box(bytes.as_slice()));
			black_box(ms)
		});
	});

	grp.bench_function(BenchmarkId::from_parameter("skip_until_field_then_deser_i64"), |b| {
		b.iter(|| {
			let ms = read_latency_ms_via_skip(black_box(bytes.as_slice())).unwrap();
			black_box(ms)
		});
	});

	grp.finish();
}

criterion_group!(benches, surrealdb_style_doc_predicate_benches);
criterion_main!(benches);
