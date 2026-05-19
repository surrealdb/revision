//! `#[revision(fixed)]` is purely syntactic — it doesn't see through type
//! aliases. A `MyId` alias for `u32` cannot carry the attribute even though
//! the underlying type is one we support; the macro can't tell at parse
//! time and would silently fall through to the default varint encoding,
//! defeating the attribute's purpose. Reject explicitly so the user knows
//! to spell the bare name.
use revision::revisioned;

type MyId = u32;

#[revisioned(revision = 1)]
struct BadAliased {
	#[revision(fixed)]
	id: MyId,
}

fn main() {}
