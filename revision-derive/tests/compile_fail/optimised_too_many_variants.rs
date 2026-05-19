//! Optimised enums allow at most 32 variants alive at any revision.
use revision::revisioned;

#[revisioned(revision(1, encoding = "optimised"))]
enum TooManyVariants {
	#[revision(size = "inline")] V00,
	#[revision(size = "inline")] V01,
	#[revision(size = "inline")] V02,
	#[revision(size = "inline")] V03,
	#[revision(size = "inline")] V04,
	#[revision(size = "inline")] V05,
	#[revision(size = "inline")] V06,
	#[revision(size = "inline")] V07,
	#[revision(size = "inline")] V08,
	#[revision(size = "inline")] V09,
	#[revision(size = "inline")] V10,
	#[revision(size = "inline")] V11,
	#[revision(size = "inline")] V12,
	#[revision(size = "inline")] V13,
	#[revision(size = "inline")] V14,
	#[revision(size = "inline")] V15,
	#[revision(size = "inline")] V16,
	#[revision(size = "inline")] V17,
	#[revision(size = "inline")] V18,
	#[revision(size = "inline")] V19,
	#[revision(size = "inline")] V20,
	#[revision(size = "inline")] V21,
	#[revision(size = "inline")] V22,
	#[revision(size = "inline")] V23,
	#[revision(size = "inline")] V24,
	#[revision(size = "inline")] V25,
	#[revision(size = "inline")] V26,
	#[revision(size = "inline")] V27,
	#[revision(size = "inline")] V28,
	#[revision(size = "inline")] V29,
	#[revision(size = "inline")] V30,
	#[revision(size = "inline")] V31,
	#[revision(size = "inline")] V32, // 33rd variant — one too many.
}

fn main() {}
