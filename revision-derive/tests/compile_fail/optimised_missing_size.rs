//! Enum variants under `encoding = "optimised"` must declare a size class.
use revision::revisioned;

#[revisioned(revision(1, encoding = "optimised"))]
enum BadEnum {
	WithoutSizeAttribute,
}

fn main() {}
