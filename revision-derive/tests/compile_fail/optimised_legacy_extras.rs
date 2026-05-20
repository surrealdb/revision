//! Encoding-specific flags (`indexed_struct` and the per-field
//! `indexed_map` / `indexed_seq` / `indexed_set` markers) require the
//! `optimised` flag on the same revision entry. Using `indexed_struct`
//! on a legacy (default) entry is a compile error.
use revision::revisioned;

#[revisioned(revision(1, indexed_struct))]
struct BadLegacyExtras {
	a: u32,
}

fn main() {}
