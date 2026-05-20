//! Enum variants under `optimised` must declare a size class.
use revision::revisioned;

#[revisioned(revision(1, optimised))]
enum BadEnum {
	WithoutSizeAttribute,
}

fn main() {}
