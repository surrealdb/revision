//! End-to-end tests for `#[revision(indexed_seq, strided)]` /
//! `#[revision(indexed_set, strided)]` on struct fields.
//!
//! The attribute is the whole opt-in surface: a field that declares `strided`
//! may drop the per-element offset table when its elements all serialise to
//! the same width, and one that does not keeps the offset table whatever the
//! data looks like. Both shapes decode through the same reader, so these tests
//! assert the *shape* as well as the round trip — a round trip alone cannot
//! tell the two apart.

use std::collections::BTreeSet;

use revision::optimised::IndexedSeqWalker;
use revision::prelude::*;

#[revisioned(revision(1, optimised))]
#[derive(Debug, Clone, PartialEq)]
struct Plain {
	#[revision(indexed_seq)]
	values: Vec<u32>,
}

#[revisioned(revision(1, optimised))]
#[derive(Debug, Clone, PartialEq)]
struct Strided {
	#[revision(indexed_seq, strided)]
	values: Vec<u32>,
}

#[revisioned(revision(1, optimised))]
#[derive(Debug, Clone, PartialEq)]
struct StridedSet {
	#[revision(indexed_set, strided)]
	values: BTreeSet<u32>,
}

#[test]
fn strided_field_drops_the_offset_table() {
	let values: Vec<u32> = (0u32..16).collect();

	let plain = revision::to_vec(&Plain {
		values: values.clone(),
	})
	.unwrap();
	let strided = revision::to_vec(&Strided {
		values: values.clone(),
	})
	.unwrap();

	assert!(
		strided.len() < plain.len(),
		"strided field should be smaller: {} vs {}",
		strided.len(),
		plain.len()
	);

	// Both decode back to the same values.
	assert_eq!(revision::from_slice::<Plain>(&plain).unwrap().values, values);
	assert_eq!(revision::from_slice::<Strided>(&strided).unwrap().values, values);
}

#[test]
fn only_the_declaring_field_changes_shape() {
	let values: Vec<u32> = (0u32..16).collect();

	let plain_bytes = revision::to_vec(&Plain {
		values: values.clone(),
	})
	.unwrap();
	let mut r: &[u8] = &plain_bytes;
	let mut w = Plain::walk_revisioned(&mut r).unwrap();
	let view = w.walk_values().unwrap();
	let walker: IndexedSeqWalker<u32> = IndexedSeqWalker::from_payload(view.as_bytes()).unwrap();
	assert_eq!(walker.stride(), None, "a field without `strided` keeps the offset table");

	let strided_bytes = revision::to_vec(&Strided {
		values: values.clone(),
	})
	.unwrap();
	let mut r: &[u8] = &strided_bytes;
	let mut w = Strided::walk_revisioned(&mut r).unwrap();
	let view = w.walk_values().unwrap();
	let walker: IndexedSeqWalker<u32> = IndexedSeqWalker::from_payload(view.as_bytes()).unwrap();
	assert!(walker.stride().is_some(), "a field declaring `strided` drops the offset table");
}

#[test]
fn strided_set_field_round_trips() {
	// Values whose byte order differs from their numeric order, so the
	// encoder's sort is load-bearing on the strided path too.
	let original = StridedSet {
		values: [256u32, 257, 258, 259, 260, 261, 511, 512].into_iter().collect(),
	};
	let bytes = revision::to_vec(&original).unwrap();
	assert_eq!(revision::from_slice::<StridedSet>(&bytes).unwrap(), original);
}

#[test]
fn mixed_width_elements_fall_back_to_the_offset_table() {
	// `strided` is permission, not obligation: elements of differing widths
	// cannot share a stride, so the offset table is still emitted.
	#[revisioned(revision(1, optimised))]
	#[derive(Debug, Clone, PartialEq)]
	struct Mixed {
		#[revision(indexed_seq, strided)]
		values: Vec<String>,
	}

	let original = Mixed {
		values: (0..8).map(|i| "ab".repeat(i + 1)).collect(),
	};
	let bytes = revision::to_vec(&original).unwrap();
	let mut r: &[u8] = &bytes;
	let mut w = Mixed::walk_revisioned(&mut r).unwrap();
	let view = w.walk_values().unwrap();
	let walker: IndexedSeqWalker<String> = IndexedSeqWalker::from_payload(view.as_bytes()).unwrap();
	assert_eq!(walker.stride(), None, "mixed widths cannot be strided");
	assert_eq!(revision::from_slice::<Mixed>(&bytes).unwrap(), original);
}
