#![cfg(feature = "skip")]
#![allow(dead_code)]

use revision::{DeserializeRevisioned, Error, SerializeRevisioned, revisioned, to_vec};
use revision::{SkipCheckRevisioned, SkipRevisioned, skip_slice};

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
fn eof_on_empty_reads_for_primitive_skips_deserialize_alignment() {
	let mut cursor: &[u8] = &[];
	let r_skip = <u64 as SkipRevisioned>::skip_revisioned(&mut cursor);
	let mut cursor2: &[u8] = &[];
	let r_deser = <u64 as DeserializeRevisioned>::deserialize_revisioned(&mut cursor2);
	assert!(r_skip.is_err());
	assert!(r_deser.is_err());
}
