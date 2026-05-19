# Changelog

## 0.23.0 (unreleased)

The headline of this release is the optimised wire format — an opt-in
encoding that gives O(1) skip, optional offset-table prologues for
O(1)/O(log n) random access, and a tagged-value envelope for enums.
Existing types using `#[revisioned(revision = N)]` continue to work
unchanged on the wire; the new behaviour is opt-in per revision.

### Added

#### New `#[revisioned(...)]` history syntax

- `#[revisioned(revision(N, encoding = "optimised", struct = "indexed"))]`
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

Variants of an `encoding = "optimised"` enum declare a tag class:

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
  — works for both Wire and Materialised paths (including the
  optimised enum's tag-byte slurp), unlike `into_<variant>` which
  is Wire-only.
- `<variant>_view(self) -> Result<OwnedVariantView<T>, Error>` —
  returns an owned wrapper around the variant payload bytes;
  callers construct their own walker / decoder against it.
- `walk_<field>` / `into_walk_<field>` for `indexed_map` /
  `indexed_seq` / `indexed_set` fields return
  `OwnedIndexedMapView<K, V>` / `OwnedIndexedSeqView<T>` /
  `OwnedIndexedSetView<T>` — each owns the field's canonical wire
  bytes and exposes `.walker()` to borrow the appropriate
  indexed walker.
- The macro-generated walker for `struct = "indexed"` types now
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
    revision(2, encoding = "optimised", struct = "indexed"),
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
