//! Behavioural equivalence: legacy `revision = N` syntax must produce the same
//! wire output and decoding behaviour as the new `revision(N)` syntax with all
//! entries left at the default (legacy) encoding.

use revision::prelude::*;

#[revisioned(revision = 2)]
#[derive(Debug, PartialEq)]
struct LegacyForm {
	a: u32,
	#[revision(start = 2, default_fn = "default_b")]
	b: u16,
}

impl LegacyForm {
	fn default_b(_revision: u16) -> Result<u16, revision::Error> {
		Ok(7)
	}
}

#[revisioned(revision(1), revision(2))]
#[derive(Debug, PartialEq)]
struct NewForm {
	a: u32,
	#[revision(start = 2, default_fn = "default_b")]
	b: u16,
}

impl NewForm {
	fn default_b(_revision: u16) -> Result<u16, revision::Error> {
		Ok(7)
	}
}

#[test]
fn new_form_produces_same_revision_constant() {
	assert_eq!(LegacyForm::revision(), NewForm::revision());
	assert_eq!(LegacyForm::REVISION, NewForm::REVISION);
}

#[test]
fn new_form_produces_byte_identical_serialisation_to_legacy() {
	let legacy = LegacyForm {
		a: 12345,
		b: 67,
	};
	let new = NewForm {
		a: 12345,
		b: 67,
	};
	let legacy_bytes = revision::to_vec(&legacy).unwrap();
	let new_bytes = revision::to_vec(&new).unwrap();
	assert_eq!(legacy_bytes, new_bytes, "the two syntaxes must agree on the wire");
}

#[test]
fn new_form_round_trip_matches_legacy() {
	let legacy = LegacyForm {
		a: 999,
		b: 11,
	};
	let bytes = revision::to_vec(&legacy).unwrap();
	let new: NewForm = revision::from_slice(&bytes).unwrap();
	assert_eq!(new.a, 999);
	assert_eq!(new.b, 11);
}

#[test]
fn new_form_default_fn_synthesises_missing_field_at_older_revision() {
	#[revisioned(revision(1))]
	#[derive(Debug)]
	struct V1 {
		a: u32,
	}

	let bytes = revision::to_vec(&V1 {
		a: 42,
	})
	.unwrap();
	let upgraded: NewForm = revision::from_slice(&bytes).unwrap();
	assert_eq!(upgraded.a, 42);
	assert_eq!(upgraded.b, 7, "default_fn should fill in the field absent at rev 1");
}
