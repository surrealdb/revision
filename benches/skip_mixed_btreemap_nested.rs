//! Mixed root `BTreeMap` (40 entries) with a nested `BTreeMap` (15 entries): full document
//! deserialize vs streaming (decode each key, skip non-target values, peel two targets).
//!
//! Root values use [`MixedRoot`] (scalars + one `Sub` inner map). Inner keys use [`MixedInner`].
//! Targets: `k30` → [`MixedRoot::Num`], `k08` → [`MixedRoot::Sub`] → `n11` → [`MixedInner::Real`].

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use revision::{DeserializeRevisioned, Error, SkipRevisioned, revisioned, to_vec};
use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::hint::black_box;
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

const ROOT_NUM_KEY: &str = "k30";
const ROOT_SUB_KEY: &str = "k08";
const NEST_REAL_KEY: &str = "n11";

const EXPECT_U64: u64 = 0xFEED_BEEF_CAFE_u64;

/// Decl order: `SysTime`, `Text`, `Num`, `Bytes`, `Tags`, `Real`, `Id`, `Sub` → `Num` is `2`, `Sub` is `7`.
const DISC_ROOT_NUM: u32 = 2;
const DISC_ROOT_SUB: u32 = 7;

/// Inner: `SysTime` … `Id` (`0`..=`6`). `Real` is `5`.
const DISC_INNER_REAL: u32 = 5;

#[revisioned(revision = 1)]
#[derive(Debug)]
enum MixedInner {
	SysTime(std::time::SystemTime),
	Text(String),
	Num(u64),
	Bytes(Vec<u8>),
	Tags(Vec<String>),
	Real(f64),
	Id(Uuid),
}

#[revisioned(revision = 1)]
#[derive(Debug)]
enum MixedRoot {
	SysTime(std::time::SystemTime),
	Text(String),
	Num(u64),
	Bytes(Vec<u8>),
	Tags(Vec<String>),
	Real(f64),
	Id(Uuid),
	Sub(BTreeMap<String, MixedInner>),
}

#[revisioned(revision = 1)]
#[derive(Debug)]
struct BenchDoc {
	root: BTreeMap<String, MixedRoot>,
}

fn inner_scalar_rotate(i: usize) -> MixedInner {
	let k = i % 7;
	match k {
		0 => MixedInner::SysTime(UNIX_EPOCH + Duration::from_secs(1_400_000_000 + i as u64)),
		1 => MixedInner::Text(format!("inner-t{i}|{}", "y".repeat(16))),
		2 => MixedInner::Num(i as u64 ^ 0xAA55),
		3 => MixedInner::Bytes(vec![(i as u8).wrapping_mul(3); 28]),
		4 => MixedInner::Tags((0..3).map(|j| format!("inner-{i}-{j}")).collect()),
		5 => MixedInner::Real((i as f64) * 0.25 + 0.5),
		6 => MixedInner::Id(Uuid::from_u128(0x6000_0000_0000_0000_u128 | i as u128)),
		_ => unreachable!(),
	}
}

fn build_inner_map() -> BTreeMap<String, MixedInner> {
	let mut m = BTreeMap::new();
	for i in 0..15 {
		let key = format!("n{i:02}");
		let v = if i == 11 {
			MixedInner::Real(TAU)
		} else {
			inner_scalar_rotate(i)
		};
		m.insert(key, v);
	}
	m
}

fn root_scalar_rotate(i: usize) -> MixedRoot {
	let k = i % 7;
	match k {
		0 => MixedRoot::SysTime(UNIX_EPOCH + Duration::from_secs(1_500_000_000 + i as u64)),
		1 => MixedRoot::Text(format!("root-t{i}|{}", "z".repeat(20))),
		2 => MixedRoot::Num(i as u64 ^ 0x55AA),
		3 => MixedRoot::Bytes(vec![(i as u8).wrapping_add(9); 40]),
		4 => MixedRoot::Tags((0..4).map(|j| format!("root-{i}-{j}")).collect()),
		5 => MixedRoot::Real((i as f64) * 0.125 + 1.0),
		6 => MixedRoot::Id(Uuid::from_u128(0x7000_0000_0000_0000_u128 | i as u128)),
		_ => unreachable!(),
	}
}

fn build_doc() -> BenchDoc {
	let mut root = BTreeMap::new();
	for i in 0..40 {
		let key = format!("k{i:02}");
		let v = match i {
			8 => MixedRoot::Sub(build_inner_map()),
			30 => MixedRoot::Num(EXPECT_U64),
			_ => root_scalar_rotate(i),
		};
		root.insert(key, v);
	}
	BenchDoc {
		root,
	}
}

fn extract_two_via_full(mut r: &[u8]) -> (u64, f64) {
	let doc = BenchDoc::deserialize_revisioned(&mut r).unwrap();
	let n = match doc.root.get(ROOT_NUM_KEY).expect("root u64") {
		MixedRoot::Num(v) => *v,
		other => panic!("expected Num at {ROOT_NUM_KEY}: {other:?}"),
	};
	let inner = match doc.root.get(ROOT_SUB_KEY).expect("nested map") {
		MixedRoot::Sub(m) => m,
		other => panic!("expected Sub at {ROOT_SUB_KEY}: {other:?}"),
	};
	let f = match inner.get(NEST_REAL_KEY).expect("nested f64") {
		MixedInner::Real(v) => *v,
		other => panic!("expected Real at {NEST_REAL_KEY}: {other:?}"),
	};
	(n, f)
}

/// Stream outer map: skip values until [`ROOT_NUM_KEY`] (read `u64`) and [`ROOT_SUB_KEY`] (stream inner map for [`NEST_REAL_KEY`]).
fn extract_two_via_stream(mut reader: &[u8]) -> Result<(u64, f64), Error> {
	let _doc_rev = u16::deserialize_revisioned(&mut reader)?;
	let len = usize::deserialize_revisioned(&mut reader)?;
	let mut out_u: Option<u64> = None;
	let mut out_f: Option<f64> = None;
	for _ in 0..len {
		let k = String::deserialize_revisioned(&mut reader)?;
		if k == ROOT_NUM_KEY {
			let _rv = u16::deserialize_revisioned(&mut reader)?;
			let disc = u32::deserialize_revisioned(&mut reader)?;
			if disc != DISC_ROOT_NUM {
				return Err(Error::Deserialize("bench: root num disc".into()));
			}
			out_u = Some(u64::deserialize_revisioned(&mut reader)?);
			if out_f.is_some() {
				break;
			}
			continue;
		}
		if k == ROOT_SUB_KEY {
			let _rv = u16::deserialize_revisioned(&mut reader)?;
			let disc = u32::deserialize_revisioned(&mut reader)?;
			if disc != DISC_ROOT_SUB {
				return Err(Error::Deserialize("bench: root sub disc".into()));
			}
			let inner_len = usize::deserialize_revisioned(&mut reader)?;
			for _ in 0..inner_len {
				let ik = String::deserialize_revisioned(&mut reader)?;
				if ik == NEST_REAL_KEY {
					let _irv = u16::deserialize_revisioned(&mut reader)?;
					let idisc = u32::deserialize_revisioned(&mut reader)?;
					if idisc != DISC_INNER_REAL {
						return Err(Error::Deserialize("bench: inner real disc".into()));
					}
					out_f = Some(f64::deserialize_revisioned(&mut reader)?);
				} else {
					MixedInner::skip_revisioned(&mut reader)?;
				}
			}
			if out_u.is_some() && out_f.is_some() {
				break;
			}
			continue;
		}
		MixedRoot::skip_revisioned(&mut reader)?;
	}
	let u = out_u.ok_or_else(|| Error::Deserialize("bench: missing root u64".into()))?;
	let f = out_f.ok_or_else(|| Error::Deserialize("bench: missing nested f64".into()))?;
	Ok((u, f))
}

fn mixed_nested_two_field_benches(c: &mut Criterion) {
	let doc = build_doc();
	let bytes = to_vec(&doc).unwrap();
	let full = extract_two_via_full(bytes.as_slice());
	let stream = extract_two_via_stream(bytes.as_slice()).unwrap();
	assert_eq!(full.0, stream.0);
	assert_eq!(full.0, EXPECT_U64);
	assert!((full.1 - stream.1).abs() < f64::EPSILON);
	assert!((full.1 - TAU).abs() < f64::EPSILON);

	let mut grp = c.benchmark_group("mixed_btreemap_nested_two_fields");
	grp.throughput(Throughput::Bytes(bytes.len() as u64));

	grp.bench_function(BenchmarkId::from_parameter("deserialize_full_doc_two_lookups"), |b| {
		b.iter(|| {
			let (u, f) = extract_two_via_full(black_box(bytes.as_slice()));
			black_box((u, f))
		});
	});

	grp.bench_function(BenchmarkId::from_parameter("stream_maps_skip_then_peel_two"), |b| {
		b.iter(|| {
			let (u, f) = extract_two_via_stream(black_box(bytes.as_slice())).unwrap();
			black_box((u, f))
		});
	});

	grp.finish();
}

criterion_group!(benches, mixed_nested_two_field_benches);
criterion_main!(benches);
