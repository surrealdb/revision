use std::{fmt::Display, str::FromStr};

use proc_macro2::Span;
use syn::{
	parse::{Parse, ParseStream},
	punctuated::Punctuated,
	spanned::Spanned,
	token, Attribute, Error, LitInt, LitStr, Token,
};

mod kw {
	syn::custom_keyword!(start);
	syn::custom_keyword!(end);
	syn::custom_keyword!(convert_fn);
	syn::custom_keyword!(default_fn);
	syn::custom_keyword!(fields_name);
	syn::custom_keyword!(revision);
	syn::custom_keyword!(variant_index);
}

#[derive(Debug)]
pub struct ValueOption<K, V> {
	key: K,
	_eq: token::Eq,
	value: V,
}

impl<K, V> Parse for ValueOption<K, V>
where
	K: Parse,
	V: Parse,
{
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			key: input.parse()?,
			_eq: input.parse()?,
			value: input.parse()?,
		})
	}
}

#[derive(Debug)]
pub struct SpannedLit<V> {
	pub value: V,
	pub span: Span,
}

impl<V> Parse for SpannedLit<V>
where
	V: FromStr,
	V::Err: Display,
{
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let lit_int: LitInt = input.parse()?;
		let span = lit_int.span();
		let value = lit_int.base10_parse()?;
		Ok(Self {
			span,
			value,
		})
	}
}

pub trait AttributeOptions: Sized {
	type Option: Parse;

	fn finish(path: Span, options: Vec<Self::Option>) -> syn::Result<Self>;
}

/// Used for parsing attribute options directly instead of being wrapped in `#[revision(..)]`
pub struct Direct<O>(pub O);

impl<O> Parse for Direct<O>
where
	O: AttributeOptions,
{
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let span = input.span();
		let options = input.parse_terminated(|input| O::Option::parse(input), Token![,])?;
		let options = options.into_iter().collect::<Vec<O::Option>>();
		O::finish(span, options).map(Direct)
	}
}

#[derive(Debug)]
pub struct FilteredAttributes<O> {
	pub options: O,
	pub other: Vec<Attribute>,
}

impl<O: AttributeOptions> Parse for FilteredAttributes<O> {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let attrs = input.call(Attribute::parse_outer)?;
		let mut other = Vec::new();
		let mut options = Vec::new();
		for attr in attrs {
			if !attr.path().is_ident("revision") {
				other.push(attr);
				continue;
			}

			let parsed_options =
				attr.parse_args_with(Punctuated::<O::Option, Token![,]>::parse_terminated)?;
			options.extend(parsed_options.into_iter())
		}

		let options = O::finish(Span::call_site(), options)?;

		Ok(Self {
			options,
			other,
		})
	}
}

#[derive(Default, Debug)]
pub struct FieldOptions {
	pub start: Option<SpannedLit<usize>>,
	pub end: Option<SpannedLit<usize>>,
	pub convert: Option<LitStr>,
	pub default: Option<LitStr>,
}

impl FieldOptions {
	pub fn exists_at(&self, revision: usize) -> bool {
		self.start.as_ref().map(|x| x.value).unwrap_or(0) <= revision
			&& self.end.as_ref().map(|x| x.value).unwrap_or(usize::MAX) > revision
	}
}

pub enum FieldOption {
	Start(ValueOption<kw::start, SpannedLit<usize>>),
	End(ValueOption<kw::end, SpannedLit<usize>>),
	Convert(ValueOption<kw::convert_fn, LitStr>),
	Default(ValueOption<kw::default_fn, LitStr>),
}

impl Parse for FieldOption {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		if input.peek(kw::start) {
			return Ok(FieldOption::Start(input.parse()?));
		}
		if input.peek(kw::end) {
			return Ok(FieldOption::End(input.parse()?));
		}
		if input.peek(kw::convert_fn) {
			return Ok(FieldOption::Convert(input.parse()?));
		}
		if input.peek(kw::default_fn) {
			return Ok(FieldOption::Default(input.parse()?));
		}

		Err(input.error("invalid field option"))
	}
}

impl AttributeOptions for FieldOptions {
	type Option = FieldOption;

	fn finish(_span: Span, options: Vec<Self::Option>) -> syn::Result<Self> {
		let mut res = FieldOptions::default();

		let mut end_kw = None;

		for option in options {
			match option {
				FieldOption::Start(x) => {
					if res.start.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					res.start = Some(x.value);
				}
				FieldOption::End(x) => {
					if res.end.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					end_kw = Some(x.key);
					res.end = Some(x.value);
				}
				FieldOption::Convert(x) => {
					if res.convert.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					res.convert = Some(x.value);
				}
				FieldOption::Default(x) => {
					if res.default.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					res.default = Some(x.value);
				}
			}
		}

		if let Some(kw) = end_kw {
			if res.convert.is_none() {
				return Err(Error::new(
					kw.span(),
					"setting a ending revision for a field also requires a convert_fn",
				));
			}
		}

		Ok(res)
	}
}

#[derive(Debug)]
pub struct ItemOptions {
	pub revision: Option<usize>,
}

pub enum ItemOption {
	Revision(ValueOption<kw::revision, LitInt>),
}

impl Parse for ItemOption {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		if input.peek(kw::revision) {
			return Ok(ItemOption::Revision(input.parse()?));
		}

		return Err(input.error("invalid item option"));
	}
}

impl AttributeOptions for ItemOptions {
	type Option = ItemOption;

	fn finish(_path: Span, options: Vec<Self::Option>) -> syn::Result<Self> {
		let mut revision = None;

		for option in options {
			match option {
				ItemOption::Revision(x) => {
					if revision.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}

					revision = Some(x.value.base10_parse()?);
				}
			}
		}

		Ok(Self {
			revision,
		})
	}
}

#[derive(Default, Debug)]
pub struct VariantOptions {
	pub start: Option<SpannedLit<usize>>,
	pub end: Option<SpannedLit<usize>>,
	pub convert: Option<LitStr>,
	pub default: Option<LitStr>,
	pub fields_name: Option<LitStr>,
}

impl VariantOptions {
	pub fn exists_at(&self, revision: usize) -> bool {
		self.start.as_ref().map(|x| x.value).unwrap_or(0) <= revision
			&& self.end.as_ref().map(|x| x.value).unwrap_or(usize::MAX) > revision
	}
}

pub enum VariantOption {
	Start(ValueOption<kw::start, SpannedLit<usize>>),
	End(ValueOption<kw::end, SpannedLit<usize>>),
	Convert(ValueOption<kw::convert_fn, LitStr>),
	Default(ValueOption<kw::default_fn, LitStr>),
	Fields(ValueOption<kw::fields_name, LitStr>),
}

impl Parse for VariantOption {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		if input.peek(kw::start) {
			return Ok(VariantOption::Start(input.parse()?));
		}
		if input.peek(kw::end) {
			return Ok(VariantOption::End(input.parse()?));
		}
		if input.peek(kw::convert_fn) {
			return Ok(VariantOption::Convert(input.parse()?));
		}
		if input.peek(kw::default_fn) {
			return Ok(VariantOption::Default(input.parse()?));
		}
		if input.peek(kw::fields_name) {
			return Ok(VariantOption::Fields(input.parse()?));
		}

		Err(input.error("invalid field option"))
	}
}

impl AttributeOptions for VariantOptions {
	type Option = VariantOption;
	fn finish(_span: Span, options: Vec<Self::Option>) -> syn::Result<Self> {
		let mut res = VariantOptions::default();

		let mut end_kw = None;

		for option in options {
			match option {
				VariantOption::Start(x) => {
					if res.start.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					res.start = Some(x.value);
				}
				VariantOption::End(x) => {
					if res.end.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					end_kw = Some(x.key);
					res.end = Some(x.value);
				}
				VariantOption::Convert(x) => {
					if res.convert.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					res.convert = Some(x.value);
				}
				VariantOption::Default(x) => {
					if res.default.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					res.default = Some(x.value);
				}
				VariantOption::Fields(x) => {
					if res.fields_name.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					res.fields_name = Some(x.value);
				}
			}
		}

		if let Some(kw) = end_kw {
			if res.convert.is_none() {
				return Err(Error::new(
					kw.span(),
					"setting a ending revision for a variant also requires a convert_fn",
				));
			}
		}

		Ok(res)
	}
}