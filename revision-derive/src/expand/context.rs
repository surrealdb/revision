//! Per-revision encoding context threaded through codegen visitors.
//!
//! Each `HistoryEntry` in a type's revision history carries its own encoding
//! choices (legacy vs optimised, indexed prologues). The visitors receive
//! these choices as an [`EncodingContext`] so they can dispatch to the right
//! codegen path.

use crate::ast::history::{Encoding, HistoryEntry, MapEncoding, SeqEncoding, StructEncoding};

/// Snapshot of one history entry's encoding choices, plus the revision number.
///
/// `Copy` so visitors can carry it as a value without extra lifetimes.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct EncodingContext {
	pub revision: u16,
	pub encoding: Encoding,
	pub map: MapEncoding,
	pub seq: SeqEncoding,
	pub struct_kind: StructEncoding,
}

#[allow(dead_code)]
impl EncodingContext {
	pub fn from_entry(entry: &HistoryEntry) -> Self {
		Self {
			revision: entry.revision.value as u16,
			encoding: entry.encoding,
			map: entry.map,
			seq: entry.seq,
			struct_kind: entry.struct_kind,
		}
	}

	/// Legacy context for `revision = N` syntax-driven entries.
	pub fn legacy(revision: u16) -> Self {
		Self {
			revision,
			encoding: Encoding::Legacy,
			map: MapEncoding::Default,
			seq: SeqEncoding::Default,
			struct_kind: StructEncoding::Default,
		}
	}

	#[inline]
	pub fn is_optimised(&self) -> bool {
		matches!(self.encoding, Encoding::Optimised)
	}

	#[inline]
	pub fn struct_is_indexed(&self) -> bool {
		matches!(self.struct_kind, StructEncoding::Indexed)
	}
}
