//! Encoding-specific attributes (`map`, `seq`, `struct`) require
//! `optimised` on the same revision entry.
use revision::revisioned;

#[revisioned(revision(1, map = "indexed"))]
struct BadLegacyExtras {
	a: u32,
}

fn main() {}
