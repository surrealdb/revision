//! Exports the `Revisioned` procedural macro attribute, and the derive procedural
//! macro that automatically generates the Revisioned trait on structs and enums.
//!
//! The `Revisioned` trait is automatically implemented for the following primitives:
//! u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char,
//! String, Vec<T>, Arrays up to 32 elements, Option<T>, Box<T>, Bound<T>, Wrapping<T>,
//! (A, B), (A, B, C), (A, B, C, D), (A, B, C, D, E), Duration, HashMap<K, V>,
//! BTreeMap<K, V>, Result<T, E>, Cow<'_, T>, Decimal, regex::Regex, uuid::Uuid,
//! chrono::Duration, chrono::DateTime<Utc>, geo::Point, geo::LineString geo::Polygon,
//! geo::MultiPoint, geo::MultiLineString, and geo::MultiPolygon.

use proc_macro::TokenStream;

mod ast;
mod expand;

/// Generates serialization and deserialization code as an implementation of
/// the `Revisioned` trait for structs and enums.
///
/// This procedural macro attribute currently analyses the struct field and
/// enum variant revisions, and generates custom serializer and deserializer
/// implementations for each version. In the future, this procedural macro
/// will also automatically remove old struct fields entirely, reducing the
/// memory size of the struct, and ensuring that field types can be changed.
///
/// This macro works by generating a single serializer implementation for the
/// latest revision of a struct, and multiple deserializer implementations for
/// each historical revision of a struct. There is no limit to the maximum
/// number of revisions that are possible to be defined for a struct or enum.
///
/// ## Revisioned requirements
///
/// Currently, all struct field values, and all enum variant fields need to
/// implement the `Revisioned` trait. This is already implemented for a number
/// of primitive and custom types. In addition, the `Revisioned` derive macro
/// can not be used with generics.
///
/// ## Attribute annotations
///
/// To facilitate version tolerant serialization "history metadata" is attached
/// to the structure or enum. This is done by using the `revision` attribute for
/// each field. In the below example a new field is added to the structure
/// starting with version 2: `#[revision(start = 2)]`. The struct revision must
/// match the maximum computed revision of every struct field or enum variant.
///
/// ```ignore
/// use revision::revisioned;
///
/// #[derive(Debug)]
/// #[revisioned(revision = 2)]
/// struct Test {
///     a: u32,
///     #[revision(start = 2)]
///     b: u8,
/// }
/// ```
///
/// Multiple version annotations can be defined for a field, like for example:
/// `#[revision(start = 2, end = 3)]`. Field was added in structure version 2
/// and removed in version 3. The generated code will ensure that this field
/// will only be deserialized for version 2 of the structure.
///
/// ## Supported field attributes and usage
///
/// The struct field and enum variant `revision` attribute accepts several key-
/// value pairs to be specified in order to support struct revisions, default
/// values for newly added fields, and value conversion for old fields which
/// have been removed. The macro will automatically detect whether a conversion
/// function is required for a removed field or variant.
///
/// ### start/end
///
/// Defines the field revision lifetime. Fields can be added by specifing the
/// `start` revision number of the structure when first defining them and can
/// be removed from serialization logic by adding an `end` revision number.
///
/// For example: `#[revision(start = 2, end = 4)]`. The field would be present
/// in the structure at revisions 2 and 3, but starting with revision 4 it would
/// no longer be serialized or deserialized.
///
/// ### default_fn
///
/// Provides an initialization value for a field when deserializing from an
/// older structure version which does not contain this field. If not specified
/// the `Default` trait is used to initialize the field.
///
/// The function name needs to be specified as a string. The first function
/// argument is the source revision that is being deserialized, and the return
/// value is the same type as the field or an error.
///
/// ```ignore
/// use revision::Error;
/// use revision::revisioned;
///
/// #[derive(Debug)]
/// #[revisioned(revision = 2)]
/// struct TestStruct {
///     a: u32,
///     #[version(start = 2, default_fn = "default_b")]
///     b: u8,
/// }
///
/// impl TestStruct {
///     fn default_b(_revision: u16) -> Result<u8, Error> {
///         12u8
///     }
/// }
/// ```
///
/// ### convert_fn
///
/// If defined, the method is called when the field existed at some previous
/// revision, but no longer exists in the latest revision. The implementation
/// and behaviour is slightly different depending on whether it is applied to
/// a removed struct field or a removed enum variant or a removed field from an
/// enum variant. If defined, the function name needs to be specified as a
/// string, and will be called when the field existed at a previous revision,
/// but no longer exists in the latest revision.
///
/// When defined on a removed struct field, the first function argument is the
/// `&mut self` of the struct to update, the second argument is the source
/// revision that was deserialized, and the third argument is the deserialized
/// value from the field which has been removed.
///
/// When working with an enum variant the convert function works with a fields
/// struct. This is a generated structure which has the same fields as the enum
/// variant. By default this struct is named
/// '`{enum name}{variant name}Fields`', this name can be changed with the
/// `fields_name` if desired.
///
/// When a field in a variant is removed the convert
/// function takes a mutable reference to this fields struct as its first
/// argument, it's second argument is the revision from which this field is
/// being deserialized and it's third argument is the deserialized value.
///
/// When the entire variant is remove the first argument is the fields
/// struct with it's fields containing the values of the deserialized removed
/// variant. In both situations the convert_fn function takes as a second
/// argument the revision from which this was serialized. The function should
/// return a result with either the right deserialized value or an error.
///
/// ```ignore
/// use revision::Error;
/// use revision::revisioned;
///
/// #[derive(Debug)]
/// #[revisioned(revision = 2)]
/// struct SomeStruct {
///     some_u32: u32,
///     #[version(end = 2, convert_fn = "convert_some_u16")]
///     some_u16: u16,
///     #[revision(start = 2)]
///     some_u64: u64,
/// }
///
/// impl SomeStruct {
///     fn convert_some_u16(&mut self, _revision: u16, value: u16) -> Result<(), Error> {
///         self.some_u64 = self.some_u16 as u64;
///         Ok(())
///     }
/// }
///
/// #[derive(Debug)]
/// #[revisioned(revision = 3)]
/// enum SomeTuple {
///     One,
///     #[revision(end = 2, convert_fn = "convert_variant_two")]
///     Two(i64, u32),
///     #[revision(start = 2)]
///     Three(i64, u64, #[revision(end = 3, convert_fn = "convert_variant_three_field")] bool),
/// }
///
/// impl SomeTuple {
///     fn convert_variant_two(fields: SomeTupleTwoFields, _revision: u16) -> Result<Self, Error> {
///         Ok(Self::Three(fields.a, fields.b as u64, true))
///     }
///
///     fn convert_variant_three_field(fields: &mut SomeTupleTwoFields, _revision: u16, v: bool) -> Result<(), Error> {
///			Ok(())
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn revisioned(attrs: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
	match expand::revision(attrs.into(), input.into()) {
		Ok(x) => x.into(),
		Err(e) => e.into_compile_error().into(),
	}
}
