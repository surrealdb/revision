use super::{ATTRIBUTE_NAME, END_REVISION, START_REVISION};
use crate::common::Exists;
use quote::format_ident;
use std::cmp::max;
use std::collections::hash_map::HashMap;
use syn::{Attribute, Ident, Lit};

/// Parses and returns a string literal attribute as an Ident.
pub(crate) fn get_ident_attr(attrs: &HashMap<String, Lit>, attr_name: &str) -> Option<Ident> {
	attrs.get(attr_name).map(|ident| match ident {
		Lit::Str(lit_str) => {
			format_ident!("{}", lit_str.value())
		}
		_ => panic!("{attr_name} must be the function name as a String."),
	})
}

/// Parses and returns the start revision as an integer.
pub(crate) fn get_start_revision(attrs: &HashMap<String, Lit>) -> Option<u16> {
	if let Some(start_revision) = attrs.get(START_REVISION) {
		return Some(match start_revision {
			Lit::Int(lit) => lit.base10_parse().unwrap(),
			_ => panic!("The start revision number must be an integer"),
		});
	}
	None
}

/// Parses and returns the end revision as an integer.
pub(crate) fn get_end_revision(attrs: &HashMap<String, Lit>) -> Option<u16> {
	if let Some(start_revision) = attrs.get(END_REVISION) {
		return Some(match start_revision {
			Lit::Int(lit) => lit.base10_parse().unwrap(),
			_ => panic!("The end revision number must be an integer."),
		});
	}
	None
}

/// Returns an attribute hash_map constructed by processing a vector of syn::Attribute.
pub(crate) fn parse_field_attributes(attributes: &[Attribute]) -> HashMap<String, Lit> {
	// Store attributes in a map
	let mut attrs = HashMap::new();
	// Parse the field attributes
	attributes.iter().for_each(|attr| {
		if attr.path().is_ident(ATTRIBUTE_NAME) {
			let _ = attr.parse_nested_meta(|meta| {
				// Parse the start attribute
				if meta.path.is_ident("start") {
					if let Ok(value) = meta.value() {
						let lit: Lit = value.parse().unwrap();
						attrs.insert("start".into(), lit);
					};
				}
				// Parse the end attribute
				if meta.path.is_ident("end") {
					if let Ok(value) = meta.value() {
						let lit: Lit = value.parse().unwrap();
						attrs.insert("end".into(), lit);
					};
				}
				// Parse the default_fn attribute
				if meta.path.is_ident("default_fn") {
					if let Ok(value) = meta.value() {
						let lit: Lit = value.parse().unwrap();
						attrs.insert("default_fn".into(), lit);
					};
				}
				// Parse the convert_fn attribute
				if meta.path.is_ident("convert_fn") {
					if let Ok(value) = meta.value() {
						let lit: Lit = value.parse().unwrap();
						attrs.insert("convert_fn".into(), lit);
					};
				}
				Ok(())
			});
		}
	});
	//
	attrs
}

/// Compute current struct revision by finding the latest field change revision.
pub(crate) fn compute_revision<T>(fields: &[T]) -> u16
where
	T: Exists,
{
	let mut revision = 1;
	for field in fields {
		let beg = field.start_revision();
		let end = field.end_revision();
		let sub = field.sub_revision();
		revision = max(revision, max(max(beg, end), sub));
	}
	revision
}
