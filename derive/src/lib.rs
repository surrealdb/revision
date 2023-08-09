//! Exports the `Revisioned` procedural macro attribute, and the derive procedural
//! macro that automatically generates the Revisioned trait on structs and enums.
//!
//! The `Revisioned` trait is automatically implemented for the following primitives:
//! u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char,
//! String, Vec<T>, Arrays up to 32 elements, Option<T>, Box<T>, Bound<T>, Wrapping<T>,
//! (A, B), (A, B, C), (A, B, C, D), (A, B, C, D, E), Duration, HashMap<K, V>,
//! BTreeMap<K, V>, Decimal, regex::Regex, uuid::Uuid, chrono::DateTime<Utc>,
//! geo::Point, geo::LineString geo::Polygon, geo::MultiPoint, geo::MultiLineString,
//! and geo::MultiPolygon.

mod common;
mod descriptors;
mod fields;
mod helpers;

use common::Descriptor;
use darling::ast::NestedMeta;
use darling::{Error, FromMeta};
use descriptors::enum_desc::EnumDescriptor;
use descriptors::struct_desc::StructDescriptor;
use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{parse_macro_input, Item};

#[derive(Debug, FromMeta)]
struct Arguments {
	revision: u16,
	#[allow(dead_code)]
	expire: Option<u16>,
}

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
/// value is the same type as the field.
///
/// ```ignore
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
///     fn default_b(_revision: u16) -> u8 {
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
/// a removed struct field or a removed enum variant. If defined, the function
/// name needs to be specified as a string, and will be called when the field
/// existed at a previous revision, but no longer exists in the latest revision.
///
/// When defined on a removed struct field, the first function argument is the
/// `&mut self` of the struct to update, the second argument is the source
/// revision that was deserialized, and the third argument is the deserialized
/// value from the field which has been removed.
///
/// When defined on a removed enum variant field, the first function argument
/// is the source revision that was deserialized, and the second argument is a
/// tuple with the enum variant field values for the variant which has been
/// removed. If the enum variant is unit-like, then an empty tuple will be used
/// for the second argument.
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
/// #[revisioned(revision = 2)]
/// enum SomeTuple {
///     One,
///     #[revision(end = 2, convert_fn = "convert_variant_two")]
///     Two(i64, u32),
///     #[revision(start = 2)]
///     Three(i64, u64, bool),
/// }
///
/// impl SomeTuple {
///     fn convert_variant_two(_revision: u16, (a, b): (i64, u32)) -> Result<Self, Error> {
///         Ok(Self::Three(a, b as u64, true))
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn revisioned(attrs: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
	// Parse the current struct input
	let input: Item = parse_macro_input!(input as Item);

	// Store the macro position
	let span = input.span();

	// Parse the current struct input
	let attrs: proc_macro2::TokenStream = attrs.into();

	let attrs_span = attrs.span();

	// Parse the specified attributes
	let attrs = match NestedMeta::parse_meta_list(attrs) {
		Ok(v) => v,
		Err(e) => {
			return TokenStream::from(Error::from(e).write_errors());
		}
	};

	let _attrs = match Arguments::from_list(&attrs) {
		Ok(v) => v,
		Err(e) => {
			return TokenStream::from(e.write_errors());
		}
	};

	let (ident, generics, specified, descriptor) = match input {
		Item::Enum(ref enum_) => {
			let ident = enum_.ident.clone();
			let generics = enum_.generics.clone();
			let specified = enum_
				.attrs
				.iter()
				.find_map(|attr| {
					if attr.path().is_ident("revision") {
						let x: syn::LitInt = attr.parse_args().unwrap();
						let x = x.base10_parse::<u16>().unwrap();
						return Some(x);
					}
					None
				})
				.expect("Expected a revision identifier");

			let descriptor: Box<dyn Descriptor> = Box::new(EnumDescriptor::new(enum_));
			(ident, generics, specified, descriptor)
		}
		Item::Struct(ref struct_) => {
			let ident = struct_.ident.clone();
			let generics = struct_.generics.clone();
			let specified = struct_
				.attrs
				.iter()
				.find_map(|attr| {
					if attr.path().is_ident("revision") {
						let x: syn::LitInt = attr.parse_args().unwrap();
						let x = x.base10_parse::<u16>().unwrap();
						return Some(x);
					}
					None
				})
				.expect("Expected a revision identifier");

			let descriptor: Box<dyn Descriptor> = Box::new(StructDescriptor::new(struct_));
			(ident, generics, specified, descriptor)
		}
		_ => {
			return syn::Error::new(
				attrs_span,
				"the `revisioned` attribute can only be applied to enums or structs",
			)
			.into_compile_error()
			.into()
		}
	};
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

	/*
	// Parse the current struct name
	let ident = input.ident.clone();
	// Parse the current struct generics
	let generics = input.generics.clone();
	// Split the generics into impl, ty, and where
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
	// Calculate the specified struct version
	let specified = input
		.attrs
		.iter()
		.find_map(|attr| {
			if attr.path().is_ident("revision") {
				let x: syn::LitInt = attr.parse_args().unwrap();
				let x = x.base10_parse::<u16>().unwrap();
				return Some(x);
			}
			None
		})
		.expect("Expected a revision identifier");

	let descriptor: Box<dyn Descriptor> = match &input.data {
		syn::Data::Struct(v) => Box::new(StructDescriptor::new(v, ident.clone())),
		syn::Data::Enum(v) => Box::new(EnumDescriptor::new(v, ident.clone())),
		syn::Data::Union(_) => {
			return syn::Error::new(span, "Union serialization is not supported.")
				.to_compile_error()
				.into();
		}
	};
	*/

	let revision = descriptor.revision();
	let serializer = descriptor.generate_serializer();
	let deserializer = descriptor.generate_deserializer();

	if specified != revision {
		return syn::Error::new(
            span,
            format!("Expected struct revision {revision}, but found {specified}. Ensure fields are versioned correctly."),
        )
        .to_compile_error()
        .into();
	}

	(quote! {
		#input

        #[automatically_derived]
        impl #impl_generics revision::Revisioned for #ident #ty_generics #where_clause {
            /// Returns the current revision of this type.
            fn revision() -> u16 {
                #revision
            }
            /// Serializes the struct using the specficifed `writer`.
            fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> std::result::Result<(), revision::Error> {
                #serializer
            }
            /// Deserializes a new instance of the struct from the specficifed `reader`.
            fn deserialize_revisioned<R: std::io::Read>(mut reader: &mut R) -> std::result::Result<Self, revision::Error> {
                #deserializer
            }
        }
    })
    .into()
}
