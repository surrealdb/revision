//! Revision-history attribute model.
//!
//! Replaces the previous "single revision number per type" model with a list of
//! entries, each carrying its own encoding choices. Legacy `#[revisioned(revision = N)]`
//! syntax is preserved by synthesising a fully-`Legacy` history of length `N`.

use proc_macro2::Span;

use super::attributes::SpannedLit;

/// Encoding strategy for one revision entry.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Encoding {
	/// The encoding used by the crate up to v0.22 — bincode-style fields in source
	/// order, varint lengths, no envelope.
	Legacy,
	/// Optimised wire format: tagged ADT values, `u32_le` length-prefixed compounds,
	/// optional indexed prologues.
	Optimised,
}

/// Per-entry map encoding choice (only meaningful under `Optimised`).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MapEncoding {
	Default,
	Indexed,
}

/// Per-entry sequence encoding choice (only meaningful under `Optimised`).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SeqEncoding {
	Default,
	Indexed,
}

/// Per-entry struct encoding choice (only meaningful under `Optimised`).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StructEncoding {
	Default,
	Indexed,
}

/// Per-entry length encoding choice.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LengthEncoding {
	Varint,
	U32Le,
}

/// One revision in a type's history. Encodes both the revision number and the
/// wire-format choices that revision uses.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HistoryEntry {
	pub revision: SpannedLit<usize>,
	pub encoding: Encoding,
	pub map: MapEncoding,
	pub seq: SeqEncoding,
	pub struct_kind: StructEncoding,
	pub length: LengthEncoding,
	/// `true` when this entry was synthesised from a legacy `revision = N`
	/// attribute. Diagnostics use this to point the user at the right syntax.
	pub from_legacy: bool,
	pub span: Span,
}

#[allow(dead_code)]
impl HistoryEntry {
	/// Build a legacy entry — used both during synthesis from `revision = N`
	/// and when explicit `revision(N)` (no `encoding`) appears in the new syntax.
	pub fn legacy(revision: SpannedLit<usize>, from_legacy: bool) -> Self {
		let span = revision.span;
		Self {
			revision,
			encoding: Encoding::Legacy,
			map: MapEncoding::Default,
			seq: SeqEncoding::Default,
			struct_kind: StructEncoding::Default,
			length: LengthEncoding::Varint,
			from_legacy,
			span,
		}
	}

	/// Whether this entry uses the optimised wire format.
	#[inline]
	pub fn is_optimised(&self) -> bool {
		matches!(self.encoding, Encoding::Optimised)
	}
}
