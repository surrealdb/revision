//! A field may opt into at most one of `indexed_map` / `indexed_seq` /
//! `indexed_set`. Combining them is a compile error — the wire format
//! they each select is mutually exclusive (different layouts for the
//! field payload).
use revision::revisioned;

#[revisioned(revision(1, optimised))]
struct BadDoc {
	#[revision(indexed_map, indexed_seq)]
	field: std::collections::BTreeMap<String, u32>,
}

fn main() {}
