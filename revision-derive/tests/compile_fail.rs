//! Drive [`trybuild`] over `tests/compile_fail/*.rs`.
//!
//! Each fixture contains an attribute usage that should reject at the
//! macro expansion phase. Running `cargo test --test compile_fail`
//! compiles each fixture and asserts the actual stderr matches the
//! checked-in `.stderr` file alongside it.

#[test]
fn compile_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/compile_fail/*.rs");
}
