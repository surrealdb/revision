//! `#[revision(fixed)]` is only valid on primitive integer fields
//! (`u32`, `i32`, `u64`, `i64`, `u128`, `i128`). Applying it to other types
//! is a compile error.
use revision::revisioned;

#[revisioned(revision = 1)]
struct BadFixed {
	#[revision(fixed)]
	name: String,
}

fn main() {}
