//! Cross-revision property tests.
//!
//! Generate large numbers of random instances of optimised-encoded types and
//! verify the invariants from the design hold for every one:
//!
//! 1. `to_vec → from_slice` round-trips to an equal value.
//! 2. `SkipRevisioned::skip_revisioned` advances the cursor by exactly the
//!    number of bytes the encoder produced.
//! 3. For enum types, the walker's `discriminant()` and `is_<variant>()`
//!    agree with the variant the value was constructed from.
//!
//! Uses a deterministic seed (checked in) so failures reproduce. Sample size
//! is 10_000 per type, taking under a second total in debug builds.

use rand::{Rng, SeedableRng, rngs::StdRng};
use revision::prelude::*;

const SEED: u64 = 0xDEADBEEF_CAFEBABE;
const SAMPLES: usize = 10_000;

// -----------------------------------------------------------------------------
// Fixtures
// -----------------------------------------------------------------------------

#[revisioned(revision(1, optimised))]
#[derive(Debug, Clone, PartialEq)]
struct OptStruct {
	a: u32,
	b: u32,
	c: u64,
	d: i32,
}

#[revisioned(revision(1, optimised, indexed_struct))]
#[derive(Debug, Clone, PartialEq)]
struct IndexedStruct {
	first: u32,
	second: String,
	third: i64,
	fourth: bool,
}

#[revisioned(revision(1, optimised))]
#[derive(Debug, Clone, PartialEq)]
enum OptEnum {
	#[revision(size = "inline")]
	Unit,
	#[revision(size = "fixed(8)")]
	Bytes([u8; 8]),
	#[revision(size = "varlen")]
	Text(String),
}

// -----------------------------------------------------------------------------
// Generators
// -----------------------------------------------------------------------------

fn random_string(rng: &mut StdRng) -> String {
	let len = rng.random_range(0..32);
	(0..len).map(|_| rng.random_range(b'a'..=b'z') as char).collect()
}

fn gen_opt_struct(rng: &mut StdRng) -> OptStruct {
	OptStruct {
		a: rng.random(),
		b: rng.random(),
		c: rng.random(),
		d: rng.random(),
	}
}

fn gen_indexed_struct(rng: &mut StdRng) -> IndexedStruct {
	IndexedStruct {
		first: rng.random(),
		second: random_string(rng),
		third: rng.random(),
		fourth: rng.random(),
	}
}

fn gen_opt_enum(rng: &mut StdRng) -> OptEnum {
	match rng.random_range(0..3) {
		0 => OptEnum::Unit,
		1 => OptEnum::Bytes(rng.random()),
		_ => OptEnum::Text(random_string(rng)),
	}
}

// -----------------------------------------------------------------------------
// Properties
// -----------------------------------------------------------------------------

fn check_round_trip<T>(value: &T)
where
	T: SerializeRevisioned + DeserializeRevisioned + PartialEq + std::fmt::Debug,
{
	let bytes = revision::to_vec(value).expect("encode");
	let decoded: T = revision::from_slice(&bytes).expect("decode");
	assert_eq!(&decoded, value, "round-trip mismatch");
}

fn check_skip_advances_exact_bytes<T>(value: &T)
where
	T: SerializeRevisioned + SkipRevisioned,
{
	let bytes = revision::to_vec(value).expect("encode");
	let original_len = bytes.len();
	let mut cursor: &[u8] = &bytes;
	<T as SkipRevisioned>::skip_revisioned(&mut cursor).expect("skip");
	assert_eq!(
		original_len - cursor.len(),
		original_len,
		"skip should advance through every byte the encoder wrote"
	);
	assert!(cursor.is_empty(), "skip should consume the entire encoded value");
}

#[test]
fn property_round_trip_opt_struct() {
	let mut rng = StdRng::seed_from_u64(SEED);
	for _ in 0..SAMPLES {
		check_round_trip(&gen_opt_struct(&mut rng));
	}
}

#[test]
fn property_skip_advances_exact_for_opt_struct() {
	let mut rng = StdRng::seed_from_u64(SEED);
	for _ in 0..SAMPLES {
		check_skip_advances_exact_bytes(&gen_opt_struct(&mut rng));
	}
}

#[test]
fn property_round_trip_indexed_struct() {
	let mut rng = StdRng::seed_from_u64(SEED);
	for _ in 0..SAMPLES {
		check_round_trip(&gen_indexed_struct(&mut rng));
	}
}

#[test]
fn property_skip_advances_exact_for_indexed_struct() {
	let mut rng = StdRng::seed_from_u64(SEED);
	for _ in 0..SAMPLES {
		check_skip_advances_exact_bytes(&gen_indexed_struct(&mut rng));
	}
}

#[test]
fn property_round_trip_opt_enum() {
	let mut rng = StdRng::seed_from_u64(SEED);
	for _ in 0..SAMPLES {
		check_round_trip(&gen_opt_enum(&mut rng));
	}
}

#[test]
fn property_skip_advances_exact_for_opt_enum() {
	let mut rng = StdRng::seed_from_u64(SEED);
	for _ in 0..SAMPLES {
		check_skip_advances_exact_bytes(&gen_opt_enum(&mut rng));
	}
}

#[test]
fn property_walker_discriminant_matches_variant() {
	let mut rng = StdRng::seed_from_u64(SEED);
	for _ in 0..SAMPLES {
		let v = gen_opt_enum(&mut rng);
		let bytes = revision::to_vec(&v).unwrap();
		let mut r: &[u8] = &bytes;
		let w = OptEnum::walk_revisioned(&mut r).unwrap();
		match &v {
			OptEnum::Unit => {
				assert!(w.is_unit());
				assert!(!w.is_bytes());
				assert!(!w.is_text());
			}
			OptEnum::Bytes(_) => {
				assert!(w.is_bytes());
				assert!(!w.is_unit());
				assert!(!w.is_text());
			}
			OptEnum::Text(_) => {
				assert!(w.is_text());
				assert!(!w.is_unit());
				assert!(!w.is_bytes());
			}
		}
	}
}

#[test]
fn property_walker_decode_variant_returns_inner_value() {
	let mut rng = StdRng::seed_from_u64(SEED);
	for _ in 0..SAMPLES {
		let v = gen_opt_enum(&mut rng);
		let bytes = revision::to_vec(&v).unwrap();
		let mut r: &[u8] = &bytes;
		let w = OptEnum::walk_revisioned(&mut r).unwrap();
		match v {
			OptEnum::Unit => {
				w.decode_unit().expect("Unit decode");
			}
			OptEnum::Bytes(b) => {
				let inner = w.decode_bytes().expect("Bytes decode");
				assert_eq!(inner, b);
			}
			OptEnum::Text(s) => {
				let inner = w.decode_text().expect("Text decode");
				assert_eq!(inner, s);
			}
		}
	}
}
