# Changelog

## 0.28.0 — bounds-checked indexed-map lookups

Lets consumers skip the O(len) indexed-map prologue validation on the hot
path without risking a process abort. Driven by a SurrealDB no-index scan
profile where `IndexedMapWalker::from_payload`'s eager validation
(`validate_map_prologue` + `validate_key_region_ascending`) accounted for
~6.8 % of total CPU per record — five times the cost of the binary-search
lookup it guarded — because it walks every offset and every key while the
lookup only touches O(log len) of them.

The validation existed because `find_value_bytes` / `entries` sliced the
dense key/value regions with *unchecked* offset values: a corrupt offset
would slice out of bounds and, under `panic = 'abort'`, abort the whole
process. Bounds-checking those slices at the point of use removes that
hazard, so `from_payload_unvalidated` is now safe to use on untrusted bytes
(callers protected by an upstream integrity check — e.g. storage-engine
block checksums — can take the fast path and fall back to a full decode on
the access-time error).

### Changed

- **`IndexedMapWalker::find_value_bytes`** now returns
  `Error::OptimisedOffsetOutOfRange` instead of panicking when a key/value
  offset is past its region or non-monotonic. Walkers opened via
  `from_payload` are unaffected (their prologue is validated, so the error
  arm is unreachable).
- **`IndexedMapWalker::entries`** clamps an out-of-range or inverted
  `(start, end)` to an empty slice for that entry instead of panicking.
- **`from_payload_unvalidated`** is no longer documented as "panics on
  malformed input" — the accessors are now bounds-checked at the point of
  use. The only weaker guarantee versus `from_payload` is that a corrupt
  *non-ascending* key region can make a binary-search lookup report a
  present key as absent (no panic, no out-of-bounds read).

The wire format and all public signatures are unchanged.

## 0.27.0 — alloc-free indexed deserialize

Removes the per-call heap allocation from the indexed-body **deserialize** path, completing the work [#67](https://github.com/surrealdb/revision/pull/67) started for the **skip** path. The wire format and all public signatures are unchanged.

### Performance

- **`deserialize_indexed_map` / `deserialize_indexed_seq` (and the `HashMap` `IndexedMapEncoded` impl) no longer allocate a `vec![0u8; n]` discard buffer** to step over the offset-table prologue. They now use `slice_reader::advance_read`, the same fixed 4 KB stack-buffer read loop the other `SkipRevisioned` impls use — one fewer heap allocation per indexed map/seq decoded, no behavioural change. The reader bound stays `R: Read` (unlike the `skip_indexed_*` fast path, sequential decode can't pointer-bump), so this is not a breaking change.

## 0.26.0 — O(1) skip for indexed bodies

Fast-path the per-record cost of skipping `#[revision(indexed_map)]` /
`#[revision(indexed_seq)]` / `#[revision(indexed_set)]` fields. Driven
by a SurrealDB no-index scan profile where `skip_indexed_map` accounted
for ~15 % of total CPU — per-entry `K::skip_revisioned` /
`V::skip_revisioned` walks plus a `vec![0u8; len*8 + 8]` discard buffer
allocation per call. The wire format is unchanged; only the parser
fast-paths.

### Changed (breaking, source-level)

- **`IndexedMapEncoded::skip_indexed_map` / `IndexedSeqEncoded::skip_indexed_seq` /
  `IndexedSetEncoded::skip_indexed_set` now bound the reader type as
  `R: BorrowedReader` instead of `R: Read`.** The wire format is
  unchanged; the bound tightens because the fast paths use
  `BorrowedReader::advance` (a pointer-bump on slice-backed readers)
  to jump past the offset table and dense regions without copying or
  allocating. Downstream hand-written impls (e.g. SurrealDB's
  `VecMap` / `VecSet`) need a one-line bound update; everything that
  flows through the macro-generated walker code already used
  `BorrowedReader`-bounded readers, so consumer-side callers built on
  the macro need no changes.

### Performance

- **`skip_indexed_map` is now O(1) on indexed bodies.** The prologue
  carries the dense regions' total byte lengths (`keys_region_len` and
  `vals_region_len` as `u32_le`); the new implementation reads those,
  jumps past the offset table and both dense regions via
  `BorrowedReader::advance`, and never invokes `K`'s or `V`'s
  `SkipRevisioned` impl. Profile contribution of `Value::skip_revisioned`
  inside `skip_indexed_map` drops out entirely on indexed bodies.
- **`skip_indexed_seq` / `skip_indexed_set` reduced from O(N) entry
  skips to a single entry skip.** The seq wire format records only
  per-element offsets (no total dense length), so the new path reads
  the last offset, advances to the start of the final element, then
  calls `T::skip_revisioned` once.
- **Removed the `vec![0u8; n]` discard allocation in the skip path.**
  The old indexed-body skip path allocated a discard buffer per call to
  `read_exact` past the offset table. The new path advances the cursor
  directly. The free helper `advance_read` (used by other
  `SkipRevisioned` impls) already used a stack buffer; the indexed-map
  / indexed-seq paths now bypass it altogether via the borrowed reader.
- **Blanket `BorrowedReader for &mut R`** added so the macro-emitted
  walker code (which holds `reader: &'r mut R` and matches with
  `&mut self.repr`, producing a `&mut &'r mut R` binding) can call
  through `skip_indexed_*` without an explicit reborrow.

### Wire-format compatibility

The on-wire layout for `indexed_map`, `indexed_seq`, and `indexed_set`
is unchanged. Bytes produced by previous releases deserialise identically against
this version and vice versa; only the parser-side skip path changes.

## 0.23.0 (unreleased) — design refactor follow-up

A second pass over the optimised wire-format work. Several design choices
from the initial PR got reconsidered after self-review and an independent
code review; this release lands the cleanups together with two new
per-field encoding-override attributes.

### Added

- **`#[revision(fixed)]`** per-field attribute that forces fixed-width
  little-endian encoding for primitive integer fields (`u32`/`i32`/
  `u64`/`i64`/`u128`/`i128`), regardless of the crate-wide
  `fixed-width-encoding` cargo feature. A non-integer field tagged
  `fixed` is a compile error pointing at the field.
- **`#[revision(specialised)]`** per-field attribute that forces bulk
  `Vec<T>` encoding for primitive `T` (the same set already handled by
  `specialised-vectors`), regardless of the crate-wide
  `specialised-vectors` cargo feature. A `Vec<T>` where `T` isn't in
  the bulk-encoded list is a compile error from the trait bound.
- **`IndexedStructWalker::from_payload_unvalidated`** (and matching
  `IndexedMapWalker::from_payload_unvalidated` /
  `IndexedSeqWalker::from_payload_unvalidated`) — opt-in constructors
  that skip the O(N) prologue validation for trusted bytes. The
  validating `from_payload` stays the default.
- **Multi-field optimised enum variants get `<variant>_view`** returning
  the variant body bytes as a `VariantView<'r, ()>`. Callers decode the
  fields sequentially from the borrowed slice. Single-field tuple
  variants already had this; the surface is now uniform.

### Changed (breaking, alpha)

- **Walker view types renamed**: `OwnedVariantView` → `VariantView`,
  `OwnedIndexedMapView` → `IndexedMapView`,
  `OwnedIndexedSeqView` → `IndexedSeqView`,
  `OwnedIndexedSetView` → `IndexedSetView`. The `Owned` prefix lied
  after they switched to `Cow` storage; the new names are honest.
- **Walker repr field is now private** (`#[doc(hidden)] pub repr` →
  private). All access via accessor methods.
- **Walker repr's `Materialised` variant split** into `IndexedBorrowed`
  (struct walker) / `OptimisedBorrowed` (enum walker) and
  `ConvertedOwned`. The runtime `offsets: Option<u16>` and
  `bytes: Cow<'r, [u8]>` discriminants are gone; each variant has a
  fixed shape.

### Performance

- **Static `[SizeClass; 32]` table per optimised enum** for walker
  construction dispatch. N variant-branches collapse to a static array
  lookup + 3-arm match.
- **Zero-copy `walk_<field>` for indexed-struct parents.** When the
  parent walker is `IndexedBorrowed`, `walk_<field>` for `indexed_map` /
  `indexed_seq` / `indexed_set` fields extracts the field's bytes
  directly from the parent's offset table (`Cow::Borrowed`), skipping
  the decode + re-encode round-trip.
- **Zero-copy `walk_<field>` for Wire-repr parents too.** When the
  parent walker is `Wire` (sequential optimised or current-rev
  legacy), the macro emits a `skip_indexed_*` call bracketed by
  `BorrowedReader::remaining()` snapshots and borrows the field's
  exact wire bytes from the difference. Same zero-allocation result
  as the indexed-struct path, just derived by skip+slice instead of
  offset-table lookup. Only the rare `ConvertedOwned` (cross-rev
  `convert_fn` re-encode) path still allocates, because its bytes
  are owned by the walker and don't outlive `self`.
- **New trait method `BorrowedReader::remaining()`** returns the
  unconsumed tail as a borrowed slice — enables the Wire-repr
  fast path. Default impl returns `&[]`; `&[u8]` and `SliceReader<'a>`
  override with the actual tail.

### Pin tests

Added pinned byte-sequence tests for two more wire shapes that the
initial PR didn't cover: indexed-struct (with offset prologue) and
varlen optimised-enum variant. These join the existing legacy-rev1 and
optimised-rev1-sequential pins. See `tests/migration.rs`.

### Reconsidered from the original "what I'd do differently" list

- **Did not unify the three indexed encoder traits** (`IndexedMapEncoded`,
  `IndexedSeqEncoded`, `IndexedSetEncoded`). After closer inspection the
  apparent duplication reflects three genuinely different wire layouts
  (paired key/value offsets vs single element offsets vs sorted-on-encode
  elements), not redundant scaffolding. Unifying via a `Layout`
  associated type would add generic complexity without reducing the
  substantive logic.
- **Did not add a per-type `OFFSET_TABLE_MIN_LEN` override.** The global
  threshold of 8 is workload-defensible and no caller has asked for
  tuning it.

## 0.23.0-pre (previous PR)

The headline of this release is the optimised wire format — an opt-in
encoding that gives O(1) skip, optional offset-table prologues for
O(1)/O(log n) random access, and a tagged-value envelope for enums.
Existing types using `#[revisioned(revision = N)]` continue to work
unchanged on the wire; the new behaviour is opt-in per revision.

### Added

#### New `#[revisioned(...)]` history syntax

- `#[revisioned(revision(N, optimised, indexed_struct))]`
  declares one revision's encoding choices. Multiple `revision(...)`
  entries on the same type spell a contiguous history; the parser
  rejects gaps, duplicates, and mixing legacy `revision = N` with the
  new form. See [README §Optimised wire format] for a walkthrough.
- Legacy `#[revisioned(revision = N)]` syntax is unchanged — it
  normalises internally to N legacy entries and emits byte-identical
  wire output.
- Type-level `map = "indexed"` / `seq = "indexed"` are rejected at
  parse time; use the per-field attributes below instead.

#### Per-field encoding attributes

Inside an optimised revision, individual fields opt into specialised
encodings via `#[revision(…)]`:

- `indexed_map` — `BTreeMap` / `HashMap` / `imbl::OrdMap` /
  `imbl::HashMap` get a sorted offset-table layout for `O(log n)`
  binary-search lookup via `IndexedMapWalker`.
- `indexed_seq` — `Vec` / `imbl::Vector` get an offset table for
  `O(1)` random access via `IndexedSeqWalker`.
- `indexed_set` — `BTreeSet` / `HashSet` / `imbl::OrdSet` /
  `imbl::HashSet` get an indexed-seq layout with elements
  byte-sorted, enabling membership-by-bytes via the same walker.

The encoders fall back to a legacy `(K, V)*` / `(elem)*` body when
the collection has fewer than `OFFSET_TABLE_MIN_LEN` (= 8) entries —
the offset table would be pure overhead at those sizes.

#### Per-variant size class for optimised enums

Variants of an `optimised` enum declare a tag class:

- `#[revision(size = "inline")]` — unit variants, 1 byte total on
  the wire (just the tag).
- `#[revision(size = "fixed(N)")]` — body serialises to exactly N
  bytes (verified via `debug_assert_eq!`).
- `#[revision(size = "varlen")]` — body preceded by a `u32_le`
  length prefix, O(1) skip.

5 bits of the tag byte hold the variant id (max 32 variants per
optimised enum); the remaining 2 hold the size class.

#### Walker additions

- `decode_<variant>(self) -> Result<InnerT, Error>` on enum walkers
  — works on every walker repr (Wire, OptimisedBorrowed,
  ConvertedOwned), unlike `into_<variant>` which is Wire-only.
- `<variant>_view(self) -> Result<OwnedVariantView<T>, Error>` —
  returns an owned wrapper around the variant payload bytes;
  callers construct their own walker / decoder against it.
- `walk_<field>` / `into_walk_<field>` for `indexed_map` /
  `indexed_seq` / `indexed_set` fields return
  `OwnedIndexedMapView<K, V>` / `OwnedIndexedSeqView<T>` /
  `OwnedIndexedSetView<T>` — each owns the field's canonical wire
  bytes and exposes `.walker()` to borrow the appropriate
  indexed walker.
- The macro-generated walker for `indexed_struct` types now
  reads any field in O(1) via the offset table (previously it walked
  fields sequentially after advancing past the prologue). 5× faster
  for late-field access; see `benches/late_field_access.rs`.

#### Runtime modules

A new top-level `revision::optimised` module exposes the wire-format
primitives directly:

- `tag::{Tag, SizeClass}`, `envelope::{encode_inline, encode_fixed,
  encode_varlen, read_optimised_tag, read_varlen_slice,
  skip_varlen}` — the tagged-value envelope used by enum codegen.
- `indexed::{IndexedStructWalker, IndexedMapWalker,
  IndexedSeqWalker}` — random-access walkers over indexed payloads.
- `indexed::{IndexedMapEncoded, IndexedSeqEncoded,
  IndexedSetEncoded}` — traits the per-field attributes route
  through.
- `indexed::{serialize_indexed_map, serialize_indexed_seq,
  serialize_indexed_set_iter, serialize_indexed_entries,
  deserialize_indexed_map, deserialize_indexed_seq,
  deserialize_indexed_set, skip_indexed_map, skip_indexed_seq,
  skip_indexed_set}` — free helpers for hand-written impls.

### Changed

- `Error` is now `#[non_exhaustive]`. Five new variants for
  optimised-format errors: `InvalidOptimisedTag`,
  `OptimisedOffsetOutOfRange`, `OptimisedOffsetsNonMonotonic`,
  `OptimisedKeyRegionNotAscending`, `OptimisedSubReaderOverrun`.
  Downstream `match Error { ... }` code needs a wildcard arm.
- **`WalkRevisioned` now requires `BorrowedReader` instead of
  `Read`.** Callers passing `&[u8]` are unaffected (`&[u8]`
  implements `BorrowedReader`). Callers passing `File`,
  `TcpStream`, or other non-slice sources need to buffer first:
  ```rust,ignore
  let mut buf = Vec::new();
  source.read_to_end(&mut buf)?;
  let walker = MyType::walk_revisioned(&mut buf.as_slice())?;
  ```
  This is a one-line adjustment per call site. The motivation:
  revisioned compounds always carry their byte-length up front, so
  the full payload has to be buffered before a walk can begin
  anyway — this just makes the buffering explicit at the call
  site. In return, the walker can borrow variant bodies and
  indexed payloads directly from the source slice instead of
  copying them into per-walk `Vec<u8>` allocations.
  `SerializeRevisioned` and `DeserializeRevisioned` keep their
  `Read` / `Write` bounds unchanged.
- **Walker view types gained an `'r` lifetime parameter** and now
  hold `Cow<'r, [u8]>` instead of `Vec<u8>`. Affects
  `OwnedVariantView<'r, T>`, `OwnedIndexedMapView<'r, K, V>`,
  `OwnedIndexedSeqView<'r, T>`, `OwnedIndexedSetView<'r, T>`. When
  the walker's source is slice-backed (the common case) the view
  borrows directly — no copy. The `Vec::new(...)` constructors
  changed signature to take `Cow<'r, [u8]>`. Callers reaching for
  these views by name in type signatures or constructors need to
  add the lifetime parameter.
- **Optimised enum and indexed-struct walks no longer allocate** a
  `Vec<u8>` to hold the body bytes — they borrow directly from the
  source via `BorrowedReader::peek_bytes + advance`. The cross-
  revision `convert_fn` round-trip still allocates (rare cold
  path). The `late_field_access/4_macro_walker_optimised_indexed`
  bench dropped from ~80ns to ~40ns (49% faster) as a result.

### Migration

For most users the upgrade is **no change** — legacy
`#[revisioned(revision = N)]` continues to produce byte-identical
output. To opt in to the optimised format for new revisions, add a
history entry:

```rust,ignore
#[revisioned(
    revision(1),                                           // existing on-disk data
    revision(2, optimised, indexed_struct),
)]
struct Wide {
    id: u32,
    #[revision(indexed_map)] tags: BTreeMap<String, Value>,
    /* ... */
}
```

Bytes from rev 1 (already on disk) keep decoding through the rev-1
arm; all new writes serialise at rev 2 with the optimised envelope
and indexed prologue. Walker code that read rev-1 records continues
to work — the walker accepts both shapes.

## 0.18.0

### Added

- Optional feature **`skip`**: traits `SkipRevisioned` / `SkipCheckRevisioned`, free helpers and `revision-derive` support for skipping encoded values (`skip = false` on `#[revisioned(...)]` to opt out).
- `SkipRevisioned::skip_revisioned_slice`, used by `skip_slice` so in-memory payloads can skip bulk byte runs via `SliceReader::consume`; derived types mirror per-field skips on the slice path; `skip_reader` / `skip_check_reader` aliases; regression tests including large UTF-8 strings and nested composites; extended skip benchmarks.
