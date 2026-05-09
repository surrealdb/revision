<br>

<!-- <p align="center">
    <a href="https://github.com/surrealdb/revision#gh-dark-mode-only" target="_blank">
        <img width="200" src="/img/white/logo.svg" alt="Revision Logo">
    </a>
    <a href="https://github.com/surrealdb/revision#gh-light-mode-only" target="_blank">
        <img width="200" src="/img/black/logo.svg" alt="Revision Logo">
    </a>
</p> -->

<p align="center">A framework for revision-tolerant serialization and deserialization, with support for schema evolution over time, allowing for easy revisioning of structs and enums for data storage requirements which need to support backwards compatibility, but where the design of the data format evolves over time.</p>

<br>

<p align="center">
    <a href="https://github.com/surrealdb/revision"><img src="https://img.shields.io/badge/status-beta-ff00bb.svg?style=flat-square"></a>
    &nbsp;
    <a href="https://docs.rs/revision/"><img src="https://img.shields.io/docsrs/revision?style=flat-square"></a>
    &nbsp;
    <a href="https://crates.io/crates/revision"><img src="https://img.shields.io/crates/v/revision?style=flat-square"></a>
    &nbsp;
    <a href="https://github.com/surrealdb/revision"><img src="https://img.shields.io/badge/license-Apache_License_2.0-00bfff.svg?style=flat-square"></a>
</p>

## Information

`Revision` is a framework for revision-tolerant serialization and deserialization with support for schema evolution over time. It allows for easy revisioning of structs and enums for data storage requirements which need to support backwards compatibility, but where the design of the data structures evolve over time. Revision enables data that was serialized at older revisions to be seamlessly deserialized and converted into the latest data structures. It uses [bincode](https://crates.io/crates/bincode) for serialization and deserialization. 

The `Revisioned` trait is automatically implemented for the following primitives: `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `i8`, `i16`, `i32`, `i64`, `i128`, `isize`, `f32`, `f64`, `char`, `String`, `Vec<T>`, Arrays up to 32 elements, `Option<T>`, `Box<T>`, `Bound<T>`, `Wrapping<T>`, `Reverse<T>`, `(A, B)`, `(A, B, C)`, `(A, B, C, D)`, `(A, B, C, D, E)`, `Duration`, `HashMap<K, V>`, `BTreeMap<K, V>`, `HashSet<T>`, `BTreeSet<T>`, `BinaryHeap<T>`, `Result<T, E>`, `Cow<'_, T>`, `Decimal`, `regex::Regex`, `uuid::Uuid`, `chrono::Duration`, `chrono::DateTime<Utc>`, `geo::Point`, `geo::LineString` `geo::Polygon`, `geo::MultiPoint`, `geo::MultiLineString`, `geo::MultiPolygon`, and `ordered_float::NotNan`.

## Feature Flags

Revision supports the following feature flags:

- **`specialised-vectors`** (default): Enables specialised implementations for certain vector types that provide serialisation and deserialisation performance improvements.
- **`fixed-width-encoding`**: Uses fixed-width encoding for integers instead of variable-length encoding. By default, Revision uses variable-length encoding which is more space-efficient for small values but has overhead for large values. With this feature enabled, all integers use their full size (2 bytes for `u16`/`i16`, 4 bytes for `u32`/`i32`, 8 bytes for `u64`/`i64`, 16 bytes for `u128`/`i128`), providing predictable serialization sizes, and improved serialisation and deserialisation performance.
- **`skip`** (disabled by default): Enables `SkipRevisioned` / `SkipCheckRevisioned`, `skip_slice` / `skip_check_slice` (plus `skip_reader` / `skip_check_reader` aliases), slice fast paths, and matching derive output (`#[revisioned(..., skip = false)]` opts out per type). Library crates should forward `skip = ["revision/skip"]` and document `features = ["skip"]` for dependents; see **Skipping encoded values** below.

### Integer Encoding Trade-offs

**Variable-length encoding (default)**:
- Small values (0-250) use only 1 byte
- More compact for typical workloads with mostly small values
- Variable serialization size based on value magnitude
- Slight overhead for very large values

**Fixed-width encoding (`fixed-width-encoding` feature)**:
- Predictable, constant serialization size per type
- No branching or size checks during encoding/decoding
- Less compact for small values
- More efficient for workloads with large values

### Benchmarking

To compare variable-length vs fixed-width encoding performance:

```bash
# Benchmark with default variable-length encoding
cargo bench --bench varint_comparison

# Benchmark with fixed-width encoding
cargo bench --bench varint_comparison --features fixed-width-encoding
```

The `varint_comparison` benchmark tests serialization and deserialization performance across different data distributions (small values, large values, and mixed distributions) for all integer types.

## Inspiration

This code takes inspiration from the [Versionize](https://github.com/firecracker-microvm/versionize) library developed for [Amazon Firecracker](https://github.com/firecracker-microvm/firecracker) snapshot-restore development previews.

## Revision in action

```rust
use revision::Error;
use revision::revisioned;

// The test structure is at revision 3.
#[revisioned(revision = 3)]
#[derive(Debug, PartialEq)]
pub struct TestStruct {
    a: u32,
    #[revision(start = 2, end = 3, convert_fn = "convert_b")]
    b: u8,
    #[revision(start = 3)]
    c: u64,
    #[revision(start = 3, default_fn = "default_c")]
    d: String,
}

impl TestStruct {
    // Used to set the default value for a newly added field.
    fn default_c(_revision: u16) -> Result<String, Error> {
        Ok("test_string".to_owned())
    }
    // Used to convert the field from an old revision to the latest revision
    fn convert_b(&mut self, _revision: u16, value: u8) -> Result<(), Error> {
        self.c = value as u64;
        Ok(())
    }
}

// The test structure is at revision 3.
#[revisioned(revision = 3)]
#[derive(Debug, PartialEq)]
pub enum TestEnum {
    #[revision(end = 2, convert_fn = "upgrade_zero")]
    Zero,
    #[revision(end = 2, convert_fn = "upgrade_one")]
    One(u32),
    #[revision(start = 2)]
    Two(u64),
    #[revision(start = 2)]
    Three {
        a: i64,
        #[revision(end = 3, convert_fn = "upgrade_three_b")]
        b: f32,
        #[revision(start = 2)]
        c: rust_decimal::Decimal,
        #[revision(start = 3)]
        d: String,
    },
}

impl TestEnum {
    // Used to convert an old enum variant into a new variant.
    fn upgrade_zero(_: TestEnumZeroFields, _revision: u16) -> Result<TestEnum, Error> {
        Ok(Self::Two(0))
    }
    // Used to convert an old enum variant into a new variant.
    fn upgrade_one(f: TestEnumOneFields, _revision: u16) -> Result<TestEnum, Error> {
        Ok(Self::Two(f.0 as u64))
    }
    // Used to convert the field from an old revision to the latest revision
    fn upgrade_three_b(
        res: &mut TestEnumThreeFields,
        _revision: u16,
        value: f32,
    ) -> Result<(), Error> {
        res.c = value.into();
        Ok(())
    }
}
```

## Skipping encoded values

Use the **`skip`** feature when you handle revisioned bytes but only need to extract certain fields from the binary data - without deserializing full structs or maps into memory.

### Extracting one field from a struct

A `#[revisioned]` struct is laid out as **struct revision (`u16`)**, then **fields in source order**. Read only what you need and call `SkipRevisioned::skip_revisioned` on `&mut reader` for the rest (or use `skip_slice::<T>` to skip a whole nested value in one go when you have a sub-slice).

```rust
use revision::{DeserializeRevisioned, Error, SkipRevisioned, revisioned, to_vec};

#[revisioned(revision = 1)]
struct Row {
    // Large field we do not want to allocate when we only need `id`.
    blob: Vec<u8>,
    id: u64,
}

fn read_row_id_only(mut reader: &[u8]) -> Result<u64, Error> {
    let _struct_revision = u16::deserialize_revisioned(&mut reader)?;
    <Vec<u8> as SkipRevisioned>::skip_revisioned(&mut reader)?;
    u64::deserialize_revisioned(&mut reader)
}

let row = Row {
    blob: vec![1, 2, 3],
    id: 42,
};
let bytes = to_vec(&row).unwrap();
assert_eq!(read_row_id_only(&bytes).unwrap(), 42);
```

### Extracting one entry from a `BTreeMap`

Maps are encoded as **length (`usize`)**, then **key / value** pairs in sorted key order. Typical pattern: deserialize each key, compare, deserialize the value you care about, otherwise skip the value with the appropriate `skip_revisioned` call.

```rust
use revision::{DeserializeRevisioned, Error, SkipRevisioned, revisioned, to_vec};
use std::collections::BTreeMap;

#[revisioned(revision = 1)]
struct Config {
    values: BTreeMap<String, u64>,
}

fn get_u64(mut reader: &[u8], wanted: &str) -> Result<u64, Error> {
    let _struct_revision = u16::deserialize_revisioned(&mut reader)?;
    let n = usize::deserialize_revisioned(&mut reader)?;
    for _ in 0..n {
        let key = String::deserialize_revisioned(&mut reader)?;
        if key == wanted {
            return u64::deserialize_revisioned(&mut reader);
        }
        <u64 as SkipRevisioned>::skip_revisioned(&mut reader)?;
    }
    Err(Error::Deserialize(format!("missing key `{wanted}`")))
}

let cfg = Config {
    values: BTreeMap::from([
        ("noise".into(), 0),
        ("answer".into(), 99),
    ]),
};
let bytes = to_vec(&cfg).unwrap();
assert_eq!(get_u64(&bytes, "answer").unwrap(), 99);
```

For **map values that are themselves `#[revisioned]` enums or structs**, deserialize the discriminant / nested revision as you would when fully deserializing, and call `MyValue::skip_revisioned` on entries you discard (see `benches/skip_mixed_btreemap_nested.rs`).

Use **`skip_check_*`** when you want validation that matches stricter deserialize checks (e.g. UTF-8 for `String`). Disable skip for a type with `#[revisioned(revision = N, skip = false)]`.

## Walking encoded values

`WalkRevisioned` is a higher-level companion to `SkipRevisioned`: it lets a caller progress **element-by-element** through revisioned bytes, deciding per-element whether to **decode**, **skip**, or **walk into** further structure — without rewriting the byte-arithmetic by hand each time. The trait sits between `DeserializeRevisioned` (decode the entire value) and `SkipRevisioned` (consume the whole encoding).

The derive macro emits `WalkRevisioned` for every `#[revisioned(...)]` type by default (controlled by the same flag as `deserialize`). Opt out per type with `#[revisioned(revision = N, walk = false)]`.

For each `#[revisioned(...)]` type the derive emits a per-type walker (`<TypeName>Walker<'r, R>`) with named per-field / per-variant methods. This is in addition to the generic `StructWalker` / `EnumWalker` / `MapWalker` / `SeqWalker` types that hand-written `WalkRevisioned` impls can return.

### Walking a struct

```rust
use revision::{WalkRevisioned, revisioned, to_vec};

#[revisioned(revision = 1)]
struct Row {
    blob: Vec<u8>,
    id: u64,
}

fn read_row_id_only(mut reader: &[u8]) -> Result<u64, revision::Error> {
    let mut walker = Row::walk_revisioned(&mut reader)?;
    walker.skip_blob()?;
    walker.decode_id()
}
```

### Walking a map

`BTreeMap<K, V>` returns a `MapWalker` whose `next_entry` borrows one key/value pair at a time. Decode the key, then either decode/skip/walk the value before moving on:

```rust
use revision::{MapWalker, WalkRevisioned, to_vec};
use std::collections::BTreeMap;

let mut map: BTreeMap<String, u64> = BTreeMap::new();
map.insert("noise".into(), 0);
map.insert("answer".into(), 99);
let bytes = to_vec(&map).unwrap();

let mut reader = bytes.as_slice();
let mut walker: MapWalker<String, u64, _> = <BTreeMap<String, u64>>::walk_revisioned(&mut reader)?;
let mut found = None;
while let Some(mut entry) = walker.next_entry() {
    let k = entry.decode_key()?;
    if k == "answer" {
        found = Some(entry.decode_value()?);
    } else {
        entry.skip_value()?;
    }
}
assert_eq!(found, Some(99));
```

### Walking an enum

For each variant, the derive emits an `into_<variant>` consuming method that descends into the variant's payload (for unit and single-field tuple variants), and a per-revision `walk_revisioned_variant_name(wire_rev, disc)` lookup:

```rust
use revision::{WalkRevisioned, revisioned, to_vec};

#[revisioned(revision = 1)]
#[derive(Debug, PartialEq)]
enum Shape {
    Square(u32),
    Rectangle { w: u32, h: u32 },
    Circle(u32),
}

let bytes = to_vec(&Shape::Circle(7)).unwrap();
let mut reader = bytes.as_slice();
let walker = Shape::walk_revisioned(&mut reader)?;
if walker.is_circle() {
    let inner = walker.into_circle()?;
    let radius = inner.decode()?;
    assert_eq!(radius, 7);
}
```

### Walking across revisions

`WalkRevisioned` honours the same cross-revision contract as `DeserializeRevisioned`: any wire revision in `1..=current` is accepted, and the walker presents the **latest schema** view. There are two internal modes:

- **Wire mode** (the fast path) is used when the wire revision matches the current schema, and for any older revision of a type that does **not** use `convert_fn`. Per-field methods branch on `wire_rev` against the field's `start` annotation: fields added after the wire revision are synthesised via `Default::default()` (or the user-supplied `default_fn`); no allocations.
- **Materialised mode** is used when the wire revision differs from the current schema *and* the type has at least one `convert_fn`. The walker internally calls `Self::deserialize_revisioned` (which honours `convert_fn`), re-encodes the result at the current revision, and then byte-walks those new bytes. The user-facing API is identical; the cost is a single `Vec<u8>` allocation plus the deserialize/serialize roundtrip.

The walker's mode selection happens at construction; per-method code paths do not branch beyond a single match on the internal repr.

```rust
use revision::{WalkRevisioned, revisioned, to_vec};

#[revisioned(revision = 1)]
struct ShapeV1 {
    kind: u8,
}

#[revisioned(revision = 2)]
struct Shape {
    kind: u8,
    #[revision(start = 2)]
    flags: u8,
}

let bytes = to_vec(&ShapeV1 { kind: 3 }).unwrap();
let mut r = bytes.as_slice();
let mut walker = Shape::walk_revisioned(&mut r)?;
let kind = walker.decode_kind()?;   // exists at all revisions
let flags = walker.decode_flags()?; // synthesised default at wire rev 1
assert_eq!((kind, flags), (3, 0));
```

### Performance characteristics

| Path | Cost |
| --- | --- |
| Wire rev = current | identical to the current-rev hot path; per-field methods inline |
| Wire rev < current, type without `convert_fn` | one extra branch per field; allocation-free |
| Wire rev < current, type with `convert_fn` | `deserialize + serialize + walk`; rare in practice |

### Zero-copy peeking

When a walker visits a value whose wire format is `usize len || raw bytes` — a string, a `Vec<u8>`, a `PathBuf`, or any newtype wrapping one — the caller usually wants to compare those bytes against a needle, hash them, or stream them somewhere. Decoding the value just to throw the owned `String` / `Vec<u8>` / `Bytes` away is pure overhead.

Two small traits unlock zero-copy peeking on those payloads:

| Trait | Implemented for | Purpose |
| --- | --- | --- |
| [`BorrowedReader`] | `&[u8]`, [`SliceReader`] | A `Read` whose buffer is addressable, so a slice of upcoming bytes can be borrowed without copying. |
| [`LengthPrefixedBytes`] | `String`, `&str`, `Box<str>`, `Arc<str>`, `Cow<'_, str>`, `Vec<u8>`, `Vec<i8>`, `PathBuf`, `bytes::Bytes` (feature-gated), and downstream newtypes | Marker: this type's `SerializeRevisioned` writes exactly `usize len || raw bytes`. Does **not** apply to derived `#[revisioned(...)]` types — they prepend a `u16` revision header. |

When **both** are satisfied, walkers expose the following methods:

| Walker | Method | Reader bound | Element bound |
| --- | --- | --- | --- |
| [`LeafWalker<T>`] | [`with_bytes`] | `BorrowedReader` | `T: LengthPrefixedBytes` |
| [`MapWalker<K, V>`] | [`find_bytes`] | `BorrowedReader` | `K: LengthPrefixedBytes` |
| [`MapEntry<K, V>`] | [`with_key_bytes`] | `BorrowedReader` | `K: LengthPrefixedBytes` |
| [`MapEntry<K, V>`] | [`with_value_bytes`] | `BorrowedReader` | `V: LengthPrefixedBytes` |
| [`SeqItem<T>`] | [`with_bytes`] | `BorrowedReader` | `T: LengthPrefixedBytes` |

[`BorrowedReader`]: crate::BorrowedReader
[`LengthPrefixedBytes`]: crate::LengthPrefixedBytes
[`LeafWalker<T>`]: crate::LeafWalker
[`MapWalker<K, V>`]: crate::MapWalker
[`MapEntry<K, V>`]: crate::MapEntry
[`SeqItem<T>`]: crate::SeqItem
[`with_bytes`]: crate::LeafWalker::with_bytes
[`find_bytes`]: crate::MapWalker::find_bytes
[`with_key_bytes`]: crate::MapEntry::with_key_bytes
[`with_value_bytes`]: crate::MapEntry::with_value_bytes
[`DeserializeRevisioned`]: crate::DeserializeRevisioned
[`SkipRevisioned`]: crate::SkipRevisioned
[`MapWalker::find`]: crate::MapWalker::find
[`LeafWalker`]: crate::LeafWalker
[`MapWalker`]: crate::MapWalker
[`next_entry`]: crate::MapWalker::next_entry

#### Worked example: matching a map key by raw bytes

`MapWalker::find_bytes` is the direct analogue of `find`, but the predicate sees the key's wire bytes instead of a decoded `K`:

```rust
use std::collections::BTreeMap;
use revision::{MapWalker, WalkRevisioned, to_vec};

let mut table = BTreeMap::new();
table.insert("alpha".to_string(), 1u32);
table.insert("delta".to_string(), 2);
table.insert("zeta".to_string(), 3);
let bytes = to_vec(&table).unwrap();

let mut r = bytes.as_slice();
let walker: MapWalker<String, u32, _> =
    <BTreeMap<String, u32>>::walk_revisioned(&mut r).unwrap();

// Compare keys as `&[u8]` — no Strand / String allocated per visit.
let value = walker
    .find_bytes(|k| k.cmp(b"delta".as_slice()))
    .unwrap()
    .map(|leaf| leaf.decode())
    .transpose()
    .unwrap();

assert_eq!(value, Some(2));
```

#### Worked example: peeking a single key during streaming iteration

`MapEntry::with_key_bytes` is the per-entry counterpart. Use it when iterating with `next_entry` and you want to decide what to do with the value based on the key's bytes:

```rust
use std::collections::BTreeMap;
use revision::{MapWalker, WalkRevisioned, to_vec};

let mut table = BTreeMap::new();
table.insert("alpha".to_string(), 1u32);
table.insert("beta".to_string(), 2);
table.insert("gamma".to_string(), 3);
let bytes = to_vec(&table).unwrap();

let mut r = bytes.as_slice();
let mut walker: MapWalker<String, u32, _> =
    <BTreeMap<String, u32>>::walk_revisioned(&mut r).unwrap();

let mut beta = None;
while let Some(mut entry) = walker.next_entry() {
    let is_target = entry.with_key_bytes(|k| k == b"beta").unwrap();
    if is_target {
        beta = Some(entry.decode_value().unwrap());
    } else {
        entry.skip_value().unwrap();
    }
}
assert_eq!(beta, Some(2));
```

#### Worked example: filtering a map by value bytes

`MapEntry::with_value_bytes` mirrors `with_key_bytes` for the value slot. Useful when the key has already been handled (decoded or skipped) and the caller wants to filter based on the value's raw bytes:

```rust
use std::collections::BTreeMap;
use revision::{MapWalker, WalkRevisioned, to_vec};

let mut table: BTreeMap<String, Vec<u8>> = BTreeMap::new();
table.insert("a".into(), b"first-value".to_vec());
table.insert("b".into(), b"target-value".to_vec());
let bytes = to_vec(&table).unwrap();

let mut r = bytes.as_slice();
let mut walker: MapWalker<String, Vec<u8>, _> =
    <BTreeMap<String, Vec<u8>>>::walk_revisioned(&mut r).unwrap();

let mut hits = 0;
while let Some(mut entry) = walker.next_entry() {
    entry.skip_key().unwrap();
    if entry.with_value_bytes(|raw| raw.starts_with(b"target")).unwrap() {
        hits += 1;
    }
}
assert_eq!(hits, 1);
```

#### Worked example: scanning a sequence of strings

`SeqItem::with_bytes` lets a scan over `Vec<String>` (or any `SeqWalker` whose item type implements `LengthPrefixedBytes`) compare items as raw bytes without paying for a per-item allocation:

```rust
use revision::{SeqWalker, WalkRevisioned, to_vec};

let v = vec!["alpha".to_string(), "beta".into(), "gamma".into()];
let bytes = to_vec(&v).unwrap();

let mut r = bytes.as_slice();
let mut walker: SeqWalker<String, _> =
    <Vec<String>>::walk_revisioned(&mut r).unwrap();

let mut found = false;
while let Some(item) = walker.next_item() {
    if item.with_bytes(|s| s == b"beta").unwrap() {
        found = true;
    }
}
assert!(found);
```

#### When zero-copy peeking does **not** apply

- The reader is a streaming source (`std::fs::File`, `TcpStream`, …). `BorrowedReader` is only implemented for slice-backed readers.
- The element type is a derived `#[revisioned(...)]` type. Its wire format includes a `u16` revision header followed by the body, not bare length-prefixed bytes; use `decode` / `walk` and let the walker read past the header.
- The element is a primitive numeric (`u32`, `f64`, …) or a fixed-size array. There is no length prefix; the wire bytes are the value bytes. Use `decode` directly.

### Limitations

- **Untrusted inputs:** Wire lengths are `usize` length prefixes like everywhere else in `revision`; they bound how much is read, skipped, or materialised. Walkers add **no** extra caps or validation — same trust model as [`DeserializeRevisioned`] / [`SkipRevisioned`].
- **[`MapWalker::find`] / [`find_bytes`]:** On a match you only get a [`LeafWalker`] for that entry's value. The method consumes the [`MapWalker`]; you cannot resume [`next_entry`] on it. Key–value pairs that sort after the match remain on the underlying reader for other callers, not for the same walker instance (by design).
- **[`LengthPrefixedBytes`] on custom types:** The marker must match the type's real `SerializeRevisioned` layout (`usize len || raw bytes`). A wrong impl breaks [`with_bytes`] / [`find_bytes`] and related paths — it is an explicit contract, not something the library can detect.

- `walk_<field>` consumes the parent walker; it is supported in wire mode and errors with `Error::Conversion` in materialised mode (older revs of `convert_fn`-bearing types). Callers that hit the materialised path should `decode_<field>` instead — they already paid the deserialize cost during walker construction.
- `into_<variant>` is currently emitted for unit variants and single-field tuple variants. Multi-field tuple variants and struct variants are reachable via `discriminant()` + `decode_<field>` on the underlying bytes.
- `Vec<T>` for primitive numeric `T` uses the `specialised-vectors` bulk encoding when that Cargo feature is enabled (the default). [`SeqWalker::new`] detects those element types and returns [`Error::Deserialize`] **before** reading the sequence length, leaving the reader unchanged — use [`DeserializeRevisioned`] or [`SkipRevisioned`] instead. With `specialised-vectors` disabled, walking numeric `Vec`s uses the same per-element layout as other sequences.
- [`MapEntry`] methods enforce key/value ordering in every build: calling `decode_value` before `decode_key` / `skip_key`, or repeating `decode_key`, returns [`Error::Deserialize`] without advancing the reader when the check fails before I/O.
- [`SeqItem::walk`], [`MapEntry::walk_value`], and [`StructWalker::walk`] advance counters (`remaining`, `position`) only after `walk_revisioned` succeeds, so a failed nested walk does not desynchronise the parent walker from the byte stream.
- Types declared `#[revisioned(serialize = false)]` cannot materialise; if they also use `convert_fn`, walking older wire revisions will not be supported.
