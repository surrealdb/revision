//! `seq = "indexed"` at the type level (on a history entry) is not supported;
//! use the per-field `#[revision(indexed_seq)]` attribute instead.
use revision::revisioned;

#[revisioned(revision(1, optimised, seq = "indexed"))]
struct BadTypeLevelSeq {
	a: u32,
}

fn main() {}
