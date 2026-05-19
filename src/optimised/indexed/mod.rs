//! Indexed compound walkers and encode helpers.
//!
//! Below the threshold, indexed compounds fall back to the legacy `(K, V)*` or
//! `(elem)*` body — the prologue's `flags.0` bit is `0`. At or above the threshold,
//! the prologue carries offset tables that allow O(1)/O(log n) random access.

#[doc(hidden)]
pub mod map_walk;
#[doc(hidden)]
pub mod seq_walk;
#[doc(hidden)]
pub mod serialize;
#[doc(hidden)]
pub mod struct_walk;

pub use map_walk::IndexedMapWalker;
pub use seq_walk::IndexedSeqWalker;
pub use serialize::{
	IndexedMapEncoded, IndexedSeqEncoded, IndexedSetEncoded, IndexedMapView,
	IndexedSeqView, IndexedSetView, VariantView, deserialize_indexed_map,
	deserialize_indexed_seq, deserialize_indexed_set, serialize_indexed_entries,
	serialize_indexed_map, serialize_indexed_seq, serialize_indexed_seq_iter,
	serialize_indexed_set_iter, skip_indexed_map, skip_indexed_seq, skip_indexed_set,
};
pub use struct_walk::IndexedStructWalker;

/// Minimum entry count that triggers the indexed prologue.
///
/// Below this threshold the indexed encoders fall back to a legacy-shape body and
/// the indexed walkers fall back to linear scans. Compile-time constant so the
/// macro can inline the branch.
pub const OFFSET_TABLE_MIN_LEN: usize = 8;
