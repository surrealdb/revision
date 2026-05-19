//! `map = "indexed"` at the type level (on a history entry) is not supported.
//! The macro can't tell `BTreeMap`-shaped fields apart from any other type, so
//! the user must opt individual fields in via `#[revision(indexed_map)]`.
use revision::revisioned;

#[revisioned(revision(1, optimised, map = "indexed"))]
struct BadTypeLevelMap {
	a: u32,
}

fn main() {}
