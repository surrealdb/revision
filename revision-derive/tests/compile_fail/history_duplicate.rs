//! Duplicate revision numbers are rejected.
use revision::revisioned;

#[revisioned(revision(1), revision(1))]
struct BadDup {
	a: u32,
}

fn main() {}
