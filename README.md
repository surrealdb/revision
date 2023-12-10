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

The `Revisioned` trait is automatically implemented for the following primitives: `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `i8`, `i16`, `i32`, `i64`, `i128`, `isize`, `f32`, `f64`, `char`, `String`, `Vec<T>`, Arrays up to 32 elements, `Option<T>`, `Box<T>`, `Bound<T>`, `Wrapping<T>`, `Reverse<T>`, `(A, B)`, `(A, B, C)`, `(A, B, C, D)`, `(A, B, C, D, E)`, `Duration`, `HashMap<K, V>`, `BTreeMap<K, V>`, `HashSet<T>`, `BTreeSet<T>`, `BinaryHeap<T>`, `Result<T, E>`, `Cow<'_, T>`, `Decimal`, `regex::Regex`, `uuid::Uuid`, `chrono::DateTime<Utc>`, `geo::Point`, `geo::LineString` `geo::Polygon`, `geo::MultiPoint`, `geo::MultiLineString`, `geo::MultiPolygon`, and `ordered_float::NotNan`.

## Inspiration

This code takes inspiration from the [Versionize](https://github.com/firecracker-microvm/versionize) library developed for [Amazon Firecracker](https://github.com/firecracker-microvm/firecracker) snapshot-restore development previews.

## Revision in action

```rust
use revision::Error;
use revision::revisioned;

// The test structure is at revision 3.
#[derive(Debug, PartialEq)]
#[revisioned(revision = 3)]
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
    fn default_c(_revision: u16) -> String {
        "test_string".to_owned()
    }
    // Used to convert the field from an old revision to the latest revision
    fn convert_b(&mut self, _revision: u16, value: u8) -> Result<(), Error> {
        self.c = value as u64;
        Ok(())
    }
}

// The test structure is at revision 3.
#[derive(Debug, PartialEq)]
#[revisioned(revision = 3)]
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
        #[revision(end = 2, convert_fn = "upgrade_three_b")]
        b: f32,
        #[revision(start = 2)]
        c: rust_decimal::Decimal,
        #[revision(start = 3)]
        d: String
    },
}

impl TestEnum {
    // Used to convert an old enum variant into a new variant.
    fn upgrade_zero((): ()) -> Result<TestEnum, Error> {
        Ok(Self::Two(0))
    }
    // Used to convert an old enum variant into a new variant.
    fn upgrade_one((v0,): (u32,)) -> Result<TestEnum, Error> {
        Ok(Self::Two(v0 as u64))
    }
    // Used to convert the field from an old revision to the latest revision
    fn upgrade_three_b(&mut self, _revision: u16, value: f32) -> Result<(), Error> {
        match self {
            TestEnum::Three {
                ref mut c,
                ..
            } => {
                *c = value.into();
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}
```
