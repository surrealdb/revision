//! Mixing legacy `revision = N` with new `revision(N)` entries is rejected.
use revision::revisioned;

#[revisioned(revision = 2, revision(1))]
struct BadMix {
	a: u32,
}

fn main() {}
