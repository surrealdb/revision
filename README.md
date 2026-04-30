<br>

<!-- <p align="center">
    <a href="https://github.com/surrealdb/revision#gh-dark-mode-only" target="_blank">
        <img width="200" src="/img/white/logo.svg" alt="Revision Logo">
    </a>
    <a href="https://github.com/surrealdb/revision#gh-light-mode-only" target="_blank">
        <img width="200" src="/img/black/logo.svg" alt="Revision Logo">
    </a>
</p> -->

<p align="center">A framework for revision-tolerant serialization and deserialization,
with support for schema evolution over time, allowing for easy revisioning of structs and enums for data storage requirements which need to support backwards
compatibility, but where the design of the data format evolves over time.</p>

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
