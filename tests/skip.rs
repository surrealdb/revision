#![allow(dead_code)]

use revision::{
	DeserializeRevisioned, Error, SerializeRevisioned, SliceReader, revisioned, to_vec,
};
use revision::{SkipCheckRevisioned, SkipRevisioned, skip_check_slice, skip_slice};
use std::ops::Range;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[revisioned(revision = 1)]
#[derive(Debug, Clone, PartialEq)]
struct EvolvingV1 {
	field_a: u32,
}

#[revisioned(revision = 2)]
#[derive(Debug, PartialEq)]
struct Evolving {
	field_a: u32,
	#[revision(start = 2)]
	field_b: u16,
}

#[revisioned(revision = 1)]
#[derive(Clone, Debug)]
enum EvolvingEnumV1Wire {
	Variant(u8),
}

#[revisioned(revision = 2)]
#[derive(Debug, PartialEq)]
enum EvolvingEnum {
	#[revision(end = 2, convert_fn = "conv_old")]
	V1(u8),
	#[revision(start = 2)]
	V2(u64),
}

impl EvolvingEnum {
	fn conv_old(_fields: EvolvingEnumV1Fields, _rev: u16) -> Result<Self, Error> {
		Ok(EvolvingEnum::V2(0))
	}
}

#[test]
fn skip_slice_matches_full_serialized_encoding() {
	let v = Evolving {
		field_a: 91,
		field_b: 7,
	};
	let bytes = to_vec(&v).unwrap();
	let n = skip_slice::<Evolving>(&bytes).unwrap();
	assert_eq!(n, bytes.len());
	let mut remainder = bytes.as_slice();
	Evolving::skip_revisioned(&mut remainder).unwrap();
	assert!(remainder.is_empty());
	let mut rem2 = bytes.as_slice();
	let _decoded = Evolving::deserialize_revisioned(&mut rem2).unwrap();
	assert!(rem2.is_empty());
}

#[test]
fn skip_current_accepts_prior_revision_wire() {
	let old = EvolvingV1 {
		field_a: 42,
	};
	let mut buf = Vec::new();
	old.serialize_revisioned(&mut buf).unwrap();
	assert_eq!(skip_slice::<Evolving>(&buf).unwrap(), buf.len());
	let deserialized = Evolving::deserialize_revisioned(&mut buf.as_slice()).unwrap();
	assert_eq!(deserialized.field_a, 42);
	assert_eq!(deserialized.field_b, 0);
}

#[test]
fn skip_enum_cross_revision_wire() {
	let mut buf = Vec::new();
	EvolvingEnumV1Wire::Variant(19).serialize_revisioned(&mut buf).unwrap();
	assert_eq!(skip_slice::<EvolvingEnum>(&buf).unwrap(), buf.len());
	let out = EvolvingEnum::deserialize_revisioned(&mut buf.as_slice()).unwrap();
	assert!(matches!(out, EvolvingEnum::V2(0)));
}

#[test]
fn utf8_fast_skip_allows_invalid_payload_skip_check_rejects() {
	let mut buf = revision::to_vec(&3usize).unwrap();
	buf.extend_from_slice(&[0xff, 0xff, 0xff]);
	assert!(String::deserialize_revisioned(&mut buf.as_slice()).is_err());

	let buf2 = buf.clone();
	assert!(String::skip_revisioned(&mut buf2.as_slice()).is_ok());

	let buf3 = buf.clone();
	assert!(String::skip_check_revisioned(&mut buf3.as_slice()).is_err());
}

#[test]
fn option_bad_tag_matches_between_skip_traits() {
	let bad = [2u8];
	assert!(matches!(
		<Option<u32> as SkipRevisioned>::skip_revisioned(&mut bad.as_slice()),
		Err(Error::Deserialize(_))
	));
	assert!(matches!(
		<Option<u32> as SkipCheckRevisioned>::skip_check_revisioned(&mut bad.as_slice()),
		Err(Error::Deserialize(_))
	));
}

#[test]
fn skip_revisioned_slice_large_string_matches_skip_slice() {
	let payload = "z".repeat(70_000);
	let bytes = to_vec(&payload).unwrap();
	assert_eq!(skip_slice::<String>(&bytes).unwrap(), bytes.len());
	let mut sr = SliceReader::new(bytes.as_slice());
	String::skip_revisioned_slice(&mut sr).unwrap();
	assert_eq!(sr.remaining().len(), 0);
	let mut sr2 = SliceReader::new(bytes.as_slice());
	String::skip_revisioned(&mut sr2).unwrap();
	assert_eq!(sr2.remaining().len(), 0);
}

#[test]
fn skip_slice_length_matches_serialise_across_primitive_samples() {
	for n in [0u32, 1u32, u32::MAX - 1, u32::MAX] {
		let bytes = to_vec(&n).unwrap();
		assert_eq!(skip_slice::<u32>(&bytes).unwrap(), bytes.len());
	}
	for n in [-1i128, 0, i128::MAX] {
		let bytes = to_vec(&n).unwrap();
		assert_eq!(skip_slice::<i128>(&bytes).unwrap(), bytes.len());
	}
	let long = "α".repeat(2000);
	let bytes = to_vec(&long).unwrap();
	assert_eq!(skip_slice::<String>(&bytes).unwrap(), bytes.len());
	let t = (-7i64, vec![42u64, u64::MAX], Some("nest".to_string()));
	let bytes = to_vec(&t).unwrap();
	assert_eq!(skip_slice::<(i64, Vec<u64>, Option<String>)>(&bytes).unwrap(), bytes.len());
}

#[test]
fn skip_slice_range_and_system_time_align_with_serialisation() {
	let r = 42u64..99u64;
	let bytes_r = to_vec(&r).unwrap();
	assert_eq!(skip_slice::<Range<u64>>(&bytes_r).unwrap(), bytes_r.len());
	assert_eq!(skip_check_slice::<Range<u64>>(&bytes_r).unwrap(), bytes_r.len());

	let t = UNIX_EPOCH + Duration::from_secs(12_345) + Duration::from_nanos(987_654_321);
	let bytes_t = to_vec(&t).unwrap();
	assert_eq!(skip_slice::<SystemTime>(&bytes_t).unwrap(), bytes_t.len());
	assert_eq!(skip_check_slice::<SystemTime>(&bytes_t).unwrap(), bytes_t.len());

	let range_str = "hello".to_string().."zebra".to_string();
	let bytes_rs = to_vec(&range_str).unwrap();
	assert_eq!(skip_slice::<Range<String>>(&bytes_rs).unwrap(), bytes_rs.len());
}

#[test]
fn eof_on_empty_reads_for_primitive_skips_deserialize_alignment() {
	let mut cursor: &[u8] = &[];
	let r_skip = <u64 as SkipRevisioned>::skip_revisioned(&mut cursor);
	let mut cursor2: &[u8] = &[];
	let r_deser = <u64 as DeserializeRevisioned>::deserialize_revisioned(&mut cursor2);
	assert!(r_skip.is_err());
	assert!(r_deser.is_err());
}

#[test]
fn skip_map_value_small_matches_deserialize_consumption() {
	use std::collections::BTreeMap;

	#[revisioned(revision = 1)]
	enum MapValueBench {
		Small(Vec<u8>),
		Big(u8),
	}

	#[revisioned(revision = 1)]
	struct MapBenchRoot {
		table: BTreeMap<String, MapValueBench>,
	}

	let mut table = BTreeMap::new();
	table.insert("a".into(), MapValueBench::Small(vec![0x5e_u8; 192]));
	table.insert("z".into(), MapValueBench::Big(7));
	let root = MapBenchRoot {
		table,
	};
	let bytes = to_vec(&root).unwrap();

	let mut r = bytes.as_slice();
	assert_eq!(u16::deserialize_revisioned(&mut r).unwrap(), 1);
	let n = usize::deserialize_revisioned(&mut r).unwrap();
	assert_eq!(n, 2);
	let k0 = String::deserialize_revisioned(&mut r).unwrap();
	assert_eq!(k0, "a");
	let mut r_skip = r;
	let mut r_deser = r;
	<MapValueBench as SkipRevisioned>::skip_revisioned(&mut r_skip).unwrap();
	let _v0 = MapValueBench::deserialize_revisioned(&mut r_deser).unwrap();
	assert_eq!(
		r_skip.len(),
		r_deser.len(),
		"skip vs deser must consume same bytes for Small variant"
	);
}

#[test]
fn skip_map_walk_128_smalls_aligned() {
	use std::collections::BTreeMap;

	#[revisioned(revision = 1)]
	enum MapValueBench {
		Small(Vec<u8>),
		Big(u8),
	}

	#[revisioned(revision = 1)]
	struct MapBenchRoot {
		table: BTreeMap<String, MapValueBench>,
	}

	const SMALL_MAP_ENTRIES: usize = 128;
	const SMALL_BLOB_LEN: usize = 192;
	const MAP_TARGET_KEY: &str = "9999999999";

	let mut table = BTreeMap::new();
	for i in 0..SMALL_MAP_ENTRIES {
		table.insert(format!("{i:010}"), MapValueBench::Small(vec![0x5E_u8; SMALL_BLOB_LEN]));
	}
	table.insert(MAP_TARGET_KEY.into(), MapValueBench::Big(0));
	let root = MapBenchRoot {
		table,
	};
	let bytes = to_vec(&root).unwrap();

	let mut r_skip = bytes.as_slice();
	let mut r_deser = bytes.as_slice();
	let _rv = MapBenchRoot::deserialize_revisioned(&mut r_deser).unwrap();

	let _root = u16::deserialize_revisioned(&mut r_skip).unwrap();
	let len = usize::deserialize_revisioned(&mut r_skip).unwrap();
	assert_eq!(len, SMALL_MAP_ENTRIES + 1);
	for idx in 0..len {
		let k = String::deserialize_revisioned(&mut r_skip).unwrap();
		let expect_key = if idx < SMALL_MAP_ENTRIES {
			format!("{idx:010}")
		} else {
			MAP_TARGET_KEY.to_string()
		};
		assert_eq!(k, expect_key, "key order at iteration {idx}");
		MapValueBench::skip_revisioned(&mut r_skip).unwrap();
	}
	assert!(r_skip.is_empty(), "walker should consume entire map payload");
	assert!(r_deser.is_empty(), "parallel full deser sanity");
}

#[test]
fn skip_map_walk_128_smalls_big_mid_node_aligned() {
	use std::collections::BTreeMap;

	#[revisioned(revision = 1)]
	struct DeepLeaf {
		filler: Vec<u8>,
		target: i64,
	}

	#[revisioned(revision = 1)]
	struct MidNode {
		filler: Vec<String>,
		child: DeepLeaf,
	}

	#[revisioned(revision = 1)]
	enum MapValueBench {
		Small(Vec<u8>),
		Big(MidNode),
	}

	#[revisioned(revision = 1)]
	struct MapBenchRoot {
		table: BTreeMap<String, MapValueBench>,
	}

	let expected_child = -0x7080_9070_a0b1_c2d3_i64;
	let mid = MidNode {
		filler: vec![],
		child: DeepLeaf {
			filler: vec![0xf0_u8; 16],
			target: expected_child,
		},
	};

	const SMALL_MAP_ENTRIES: usize = 128;
	const SMALL_BLOB_LEN: usize = 192;
	const MAP_TARGET_KEY: &str = "9999999999";

	let mut table = BTreeMap::new();
	for i in 0..SMALL_MAP_ENTRIES {
		table.insert(format!("{i:010}"), MapValueBench::Small(vec![0x5E_u8; SMALL_BLOB_LEN]));
	}
	table.insert(MAP_TARGET_KEY.into(), MapValueBench::Big(mid));

	let root = MapBenchRoot {
		table,
	};
	let bytes = to_vec(&root).unwrap();

	let mut r = bytes.as_slice();
	let _root = u16::deserialize_revisioned(&mut r).unwrap();
	let len = usize::deserialize_revisioned(&mut r).unwrap();
	assert_eq!(len, SMALL_MAP_ENTRIES + 1);

	for idx in 0..len {
		let k = String::deserialize_revisioned(&mut r).unwrap();
		let expect_key = if idx < SMALL_MAP_ENTRIES {
			format!("{idx:010}")
		} else {
			MAP_TARGET_KEY.to_string()
		};
		assert_eq!(k, expect_key, "iteration {idx}");
		MapValueBench::skip_revisioned(&mut r).unwrap();
	}
	assert!(
		r.is_empty(),
		"walker should consume full map incl. Big(MidNode); leftover {} bytes",
		r.len()
	);
}

#[test]
#[ignore = "heavy allocation (~350 KiB payloads); run with --ignored --release"]
fn skip_map_walk_matches_bench_wide_mid_payload() {
	use std::collections::BTreeMap;

	#[revisioned(revision = 1)]
	struct DeepLeaf {
		filler: Vec<u8>,
		target: i64,
	}

	#[revisioned(revision = 1)]
	struct MidNode {
		filler: Vec<String>,
		child: DeepLeaf,
	}

	#[revisioned(revision = 1)]
	enum MapValueBench {
		Small(Vec<u8>),
		Big(MidNode),
	}

	#[revisioned(revision = 1)]
	struct MapBenchRoot {
		table: BTreeMap<String, MapValueBench>,
	}

	const MID_STRING_COUNT: usize = 64;
	const MID_STRING_BODY: usize = 1024;
	const LEAF_FILL_LEN: usize = 256 * 1024;

	fn build_mid_node(target: i64) -> MidNode {
		let leaf_body = MID_STRING_BODY.saturating_sub(32);
		let mid_strings: Vec<String> =
			(0..MID_STRING_COUNT).map(|i| format!("mid-{i}|{}", "x".repeat(leaf_body))).collect();
		MidNode {
			filler: mid_strings,
			child: DeepLeaf {
				filler: vec![0xF0; LEAF_FILL_LEN],
				target,
			},
		}
	}

	const SMALL_MAP_ENTRIES: usize = 128;
	const SMALL_BLOB_LEN: usize = 192;
	const MAP_TARGET_KEY: &str = "9999999999";

	let expected = -0x7080_9070_a0b1_c2d3_i64;
	let mid = build_mid_node(expected);
	let mut table = BTreeMap::new();
	for i in 0..SMALL_MAP_ENTRIES {
		table.insert(format!("{i:010}"), MapValueBench::Small(vec![0x5E_u8; SMALL_BLOB_LEN]));
	}
	table.insert(MAP_TARGET_KEY.into(), MapValueBench::Big(mid));

	let bytes = to_vec(&MapBenchRoot {
		table,
	})
	.unwrap();

	let mut r = bytes.as_slice();
	let _root = u16::deserialize_revisioned(&mut r).unwrap();
	let len = usize::deserialize_revisioned(&mut r).unwrap();
	assert_eq!(len, SMALL_MAP_ENTRIES + 1);

	for idx in 0..len {
		let k = String::deserialize_revisioned(&mut r).unwrap();
		let expect_key = if idx < SMALL_MAP_ENTRIES {
			format!("{idx:010}")
		} else {
			MAP_TARGET_KEY.to_string()
		};
		assert_eq!(k, expect_key, "iteration {idx}");
		MapValueBench::skip_revisioned(&mut r).unwrap();
	}
	assert!(r.is_empty(), "walker leftover {} bytes", r.len());

	let mut rr = bytes.as_slice();
	MapBenchRoot::deserialize_revisioned(&mut rr).unwrap();
	assert!(rr.is_empty(), "deserialize root should consume all bytes");
}

#[test]
fn map_value_revisioned_enum_wire_prefix_is_type_rev_then_variant_disc() {
	#[revisioned(revision = 1)]
	enum MapValueBench {
		Small(Vec<u8>),
		Big(u8),
	}
	let small = MapValueBench::Small(vec![0x5e_u8; 192]);
	let w = to_vec(&small).unwrap();

	let mut r = w.as_slice();
	let ty_rev = u16::deserialize_revisioned(&mut r).unwrap();
	assert_eq!(ty_rev, 1, "outer type revision");
	let disc = u32::deserialize_revisioned(&mut r).unwrap();
	assert_eq!(disc, 0, "Small variant discriminant");
	let payload = Vec::<u8>::deserialize_revisioned(&mut r).unwrap();
	assert_eq!(payload, vec![0x5e_u8; 192]);
	assert!(r.is_empty(), "leftover {} bytes", r.len());
}
