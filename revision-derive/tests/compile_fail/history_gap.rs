//! Revisions in a history must be contiguous from 1; (1, 3) is rejected.
use revision::revisioned;

#[revisioned(revision(1), revision(3))]
struct BadGap {
	a: u32,
}

fn main() {}
