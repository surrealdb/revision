//! Indexed compound walkers and encode helpers.
//!
//! Below the threshold, indexed compounds fall back to the legacy `(K, V)*` or
//! `(elem)*` body — the prologue's `flags.0` bit is `0`. At or above the threshold,
//! the prologue carries offset tables that allow O(1)/O(log n) random access.
//!
//! Sequences and sets whose elements all serialise to the same non-zero width
//! carry a single stride varint instead of that table, marked by the prologue's
//! `flags.1` bit; random access stays O(1) as `i * stride`. See
//! [`seq_walk`] for the layout and the feature gate on emitting it.

#[doc(hidden)]
pub mod map_walk;
#[doc(hidden)]
pub mod seq_walk;
#[doc(hidden)]
pub mod serialize;
#[doc(hidden)]
pub mod struct_walk;

pub use map_walk::{HintedLookup, IndexedMapWalker};
pub use seq_walk::IndexedSeqWalker;
pub use serialize::{
	IndexedMapEncoded, IndexedMapView, IndexedSeqEncoded, IndexedSeqView, IndexedSetEncoded,
	IndexedSetView, VariantView, deserialize_indexed_map, deserialize_indexed_seq,
	deserialize_indexed_set, serialize_indexed_entries, serialize_indexed_map,
	serialize_indexed_seq, serialize_indexed_seq_iter, serialize_indexed_seq_iter_strided,
	serialize_indexed_set_iter, serialize_indexed_set_iter_strided, skip_indexed_map,
	skip_indexed_seq, skip_indexed_set,
};
pub use struct_walk::IndexedStructWalker;

/// Minimum entry count that triggers the indexed prologue.
///
/// Below this threshold the indexed encoders fall back to a legacy-shape body and
/// the indexed walkers fall back to linear scans. Compile-time constant so the
/// macro can inline the branch.
pub const OFFSET_TABLE_MIN_LEN: usize = 8;
