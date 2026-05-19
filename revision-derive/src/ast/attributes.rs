use std::{collections::HashMap, fmt::Display, str::FromStr};

use proc_macro2::Span;
use syn::{
	Attribute, Error, LitBool, LitInt, LitStr, Token, parenthesized,
	parse::{Parse, ParseStream},
	punctuated::Punctuated,
	spanned::Spanned,
	token,
};

use super::history::{Encoding, HistoryEntry, MapEncoding, SeqEncoding, StructEncoding};

mod kw {
	syn::custom_keyword!(start);
	syn::custom_keyword!(end);
	syn::custom_keyword!(convert_fn);
	syn::custom_keyword!(default_fn);
	syn::custom_keyword!(fields_name);
	syn::custom_keyword!(revision);
	syn::custom_keyword!(variant_index);
	syn::custom_keyword!(order);
	syn::custom_keyword!(discriminant);
	syn::custom_keyword!(serialize);
	syn::custom_keyword!(deserialize);
	syn::custom_keyword!(skip);
	syn::custom_keyword!(walk);
	// Optimised-wire-format keywords.
	syn::custom_keyword!(encoding);
	syn::custom_keyword!(map);
	syn::custom_keyword!(seq);
	syn::custom_keyword!(size);
	// Per-field encoding flags for optimised revisions.
	syn::custom_keyword!(indexed_map);
	syn::custom_keyword!(indexed_seq);
	syn::custom_keyword!(indexed_set);
}

#[derive(Debug)]
pub struct GroupOption<K, V> {
	key: K,
	_paren: token::Paren,
	value: Punctuated<V, token::Comma>,
}

impl<K, V> Parse for GroupOption<K, V>
where
	K: Parse,
	V: Parse,
{
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let content;
		Ok(Self {
			key: input.parse()?,
			_paren: parenthesized!(content in input),
			value: content.parse_terminated(|x| x.parse(), Token![,])?,
		})
	}
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

#[derive(Debug, Clone)]
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
		let options = input.parse_terminated(O::Option::parse, Token![,])?;
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
			options.extend(parsed_options)
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
	/// `#[revision(indexed_map)]`: encode this field's `BTreeMap`-like value
	/// using the indexed-map wire format under optimised revisions. Has no
	/// effect on legacy revisions.
	pub indexed_map: bool,
	/// `#[revision(indexed_seq)]`: same, for sequence-shaped fields.
	pub indexed_seq: bool,
	/// `#[revision(indexed_set)]`: same, for set-shaped fields.
	pub indexed_set: bool,
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
	IndexedMap(kw::indexed_map),
	IndexedSeq(kw::indexed_seq),
	IndexedSet(kw::indexed_set),
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
		if input.peek(kw::indexed_map) {
			return Ok(FieldOption::IndexedMap(input.parse()?));
		}
		if input.peek(kw::indexed_seq) {
			return Ok(FieldOption::IndexedSeq(input.parse()?));
		}
		if input.peek(kw::indexed_set) {
			return Ok(FieldOption::IndexedSet(input.parse()?));
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
				FieldOption::IndexedMap(kw) => {
					if res.indexed_map {
						return Err(Error::new(kw.span(), "tried to set an option twice"));
					}
					if res.indexed_seq || res.indexed_set {
						return Err(Error::new(
							kw.span(),
							"a field can declare at most one of `indexed_map`, `indexed_seq`, `indexed_set`",
						));
					}
					res.indexed_map = true;
				}
				FieldOption::IndexedSeq(kw) => {
					if res.indexed_seq {
						return Err(Error::new(kw.span(), "tried to set an option twice"));
					}
					if res.indexed_map || res.indexed_set {
						return Err(Error::new(
							kw.span(),
							"a field can declare at most one of `indexed_map`, `indexed_seq`, `indexed_set`",
						));
					}
					res.indexed_seq = true;
				}
				FieldOption::IndexedSet(kw) => {
					if res.indexed_set {
						return Err(Error::new(kw.span(), "tried to set an option twice"));
					}
					if res.indexed_map || res.indexed_seq {
						return Err(Error::new(
							kw.span(),
							"a field can declare at most one of `indexed_map`, `indexed_seq`, `indexed_set`",
						));
					}
					res.indexed_set = true;
				}
			}
		}

		if let Some(kw) = end_kw
			&& res.convert.is_none()
		{
			return Err(Error::new(
				kw.span(),
				"setting a ending revision for a field also requires a convert_fn",
			));
		}

		Ok(res)
	}
}

#[derive(Debug)]
pub struct ItemOptions {
	/// Legacy `revision = N` attribute, retained for the existing call sites in
	/// `expand/mod.rs`. New code should consume [`history`](Self::history).
	#[allow(dead_code)]
	pub revision: Option<usize>,
	/// Resolved revision history. Always populated by [`finish`](Self::finish):
	/// either from a legacy `revision = N` (synthesised as N all-legacy entries),
	/// or from one or more new-style `revision(N, ...)` entries.
	pub history: Vec<HistoryEntry>,
	pub serialize: bool,
	pub deserialize: bool,
	pub skip: Option<bool>,
	pub walk: Option<bool>,
}

#[allow(dead_code)]
impl ItemOptions {
	/// The latest revision number in this type's history, if any was specified.
	#[inline]
	pub fn current_revision(&self) -> Option<usize> {
		self.history.last().map(|h| h.revision.value)
	}
}

pub enum ItemOption {
	Revision(ValueOption<kw::revision, LitInt>),
	RevisionEntry(RevisionEntryGroup),
	Serialize(ValueOption<kw::serialize, LitBool>),
	Deserialize(ValueOption<kw::deserialize, LitBool>),
	Skip(ValueOption<kw::skip, LitBool>),
	Walk(ValueOption<kw::walk, LitBool>),
}

/// Parsed `revision(N, encoding = "...", map = "...", seq = "...", struct = "...")`.
pub struct RevisionEntryGroup {
	pub kw: kw::revision,
	pub _paren: token::Paren,
	pub revision: SpannedLit<usize>,
	pub options: Vec<RevisionEntryOption>,
}

pub enum RevisionEntryOption {
	Encoding(ValueOption<kw::encoding, LitStr>),
	Map(ValueOption<kw::map, LitStr>),
	Seq(ValueOption<kw::seq, LitStr>),
	Struct(ValueOption<Token![struct], LitStr>),
}

impl Parse for RevisionEntryGroup {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let kw: kw::revision = input.parse()?;
		let content;
		let paren = parenthesized!(content in input);
		let revision: SpannedLit<usize> = content.parse()?;
		let mut options = Vec::new();
		while !content.is_empty() {
			content.parse::<Token![,]>()?;
			if content.is_empty() {
				break;
			}
			options.push(content.parse()?);
		}
		Ok(Self {
			kw,
			_paren: paren,
			revision,
			options,
		})
	}
}

impl Parse for RevisionEntryOption {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		if input.peek(kw::encoding) {
			return Ok(RevisionEntryOption::Encoding(input.parse()?));
		}
		if input.peek(kw::map) {
			return Ok(RevisionEntryOption::Map(input.parse()?));
		}
		if input.peek(kw::seq) {
			return Ok(RevisionEntryOption::Seq(input.parse()?));
		}
		if input.peek(Token![struct]) {
			return Ok(RevisionEntryOption::Struct(input.parse()?));
		}
		Err(input.error(
			"invalid `revision(...)` option (expected `encoding`, `map`, `seq`, or `struct`)",
		))
	}
}

impl Parse for ItemOption {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		if input.peek(kw::revision) {
			// Distinguish `revision = N` (legacy) from `revision(N, ...)` (new).
			if input.peek2(token::Paren) {
				return Ok(ItemOption::RevisionEntry(input.parse()?));
			}
			return Ok(ItemOption::Revision(input.parse()?));
		}
		if input.peek(kw::serialize) {
			return Ok(ItemOption::Serialize(input.parse()?));
		}
		if input.peek(kw::deserialize) {
			return Ok(ItemOption::Deserialize(input.parse()?));
		}
		if input.peek(kw::skip) {
			return Ok(ItemOption::Skip(input.parse()?));
		}
		if input.peek(kw::walk) {
			return Ok(ItemOption::Walk(input.parse()?));
		}

		Err(input.error("invalid item option"))
	}
}

fn build_history_entry(group: RevisionEntryGroup) -> syn::Result<HistoryEntry> {
	let span = group.kw.span();
	let mut entry = HistoryEntry::legacy(group.revision, false);
	entry.span = span;
	let mut saw_encoding = false;
	for opt in group.options {
		match opt {
			RevisionEntryOption::Encoding(v) => {
				saw_encoding = true;
				match v.value.value().as_str() {
					"legacy" => entry.encoding = Encoding::Legacy,
					"optimised" => entry.encoding = Encoding::Optimised,
					other => {
						return Err(Error::new(
							v.value.span(),
							format!(
								"unknown encoding `{other}` (expected `legacy` or `optimised`)"
							),
						));
					}
				}
			}
			RevisionEntryOption::Map(v) => match v.value.value().as_str() {
				"default" => entry.map = MapEncoding::Default,
				"indexed" => {
					return Err(Error::new(
						v.value.span(),
						"type-level `map = \"indexed\"` is not supported; use the per-field attribute `#[revision(indexed_map)]` on each map-shaped field that should use indexed encoding instead",
					));
				}
				other => {
					return Err(Error::new(
						v.value.span(),
						format!("unknown map encoding `{other}` (expected `default`)"),
					));
				}
			},
			RevisionEntryOption::Seq(v) => match v.value.value().as_str() {
				"default" => entry.seq = SeqEncoding::Default,
				"indexed" => {
					return Err(Error::new(
						v.value.span(),
						"type-level `seq = \"indexed\"` is not supported; use the per-field attribute `#[revision(indexed_seq)]` on each sequence-shaped field that should use indexed encoding instead",
					));
				}
				other => {
					return Err(Error::new(
						v.value.span(),
						format!("unknown seq encoding `{other}` (expected `default`)"),
					));
				}
			},
			RevisionEntryOption::Struct(v) => match v.value.value().as_str() {
				"default" => entry.struct_kind = StructEncoding::Default,
				"indexed" => entry.struct_kind = StructEncoding::Indexed,
				other => {
					return Err(Error::new(
						v.value.span(),
						format!(
							"unknown struct encoding `{other}` (expected `default` or `indexed`)"
						),
					));
				}
			},
		}
	}
	// Reject per-encoding attrs on a legacy entry — keeps the AST clean.
	if (!saw_encoding || entry.encoding == Encoding::Legacy)
		&& (entry.map != MapEncoding::Default
			|| entry.seq != SeqEncoding::Default
			|| entry.struct_kind != StructEncoding::Default)
	{
		return Err(Error::new(
			entry.span,
			"encoding-specific attributes (`map`, `seq`, `struct`) require `encoding = \"optimised\"` on the same revision entry",
		));
	}
	Ok(entry)
}

impl AttributeOptions for ItemOptions {
	type Option = ItemOption;

	fn finish(path: Span, options: Vec<Self::Option>) -> syn::Result<Self> {
		let mut revision = None;
		let mut serialize = true;
		let mut deserialize = true;
		let mut skip = None;
		let mut walk = None;
		let mut new_entries: Vec<HistoryEntry> = Vec::new();
		let mut new_entries_span: Option<Span> = None;

		for option in options {
			match option {
				ItemOption::Revision(x) => {
					if revision.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}

					revision = Some(x.value.base10_parse()?);
				}
				ItemOption::RevisionEntry(group) => {
					if new_entries_span.is_none() {
						new_entries_span = Some(group.kw.span());
					}
					new_entries.push(build_history_entry(group)?);
				}
				ItemOption::Serialize(x) => {
					serialize = x.value.value();
				}
				ItemOption::Deserialize(x) => {
					deserialize = x.value.value();
				}
				ItemOption::Skip(x) => {
					if skip.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					skip = Some(x.value.value());
				}
				ItemOption::Walk(x) => {
					if walk.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					walk = Some(x.value.value());
				}
			}
		}

		let history = resolve_history(path, revision, new_entries, new_entries_span)?;
		// `current_revision` is the latest history entry's revision number.
		let current = history.last().map(|h| h.revision.value);

		Ok(Self {
			revision: current,
			history,
			serialize,
			deserialize,
			skip,
			walk,
		})
	}
}

/// Combine legacy `revision = N` and new-style `revision(N, ...)` entries into a
/// validated history. Returns an empty `Vec` if neither was supplied — callers
/// combining multiple [`ItemOptions`] sources will detect missing revisions at
/// that level (see `expand/mod.rs`).
fn resolve_history(
	path: Span,
	legacy_revision: Option<usize>,
	new_entries: Vec<HistoryEntry>,
	new_entries_span: Option<Span>,
) -> syn::Result<Vec<HistoryEntry>> {
	match (legacy_revision, new_entries.is_empty()) {
		(Some(_), false) => Err(Error::new(
			new_entries_span.unwrap_or(path),
			"cannot mix legacy `revision = N` with new-style `revision(N, ...)` entries on the same type",
		)),
		(Some(n), true) => {
			if n == 0 {
				return Err(Error::new(path, "revision numbers must start at 1"));
			}
			let mut history = Vec::with_capacity(n);
			for i in 1..=n {
				let lit = SpannedLit {
					value: i,
					span: path,
				};
				history.push(HistoryEntry::legacy(lit, true));
			}
			Ok(history)
		}
		(None, false) => {
			let mut sorted = new_entries;
			sorted.sort_by_key(|h| h.revision.value);
			for (idx, entry) in sorted.iter().enumerate() {
				let expected = idx + 1;
				if entry.revision.value != expected {
					if idx > 0 && entry.revision.value == sorted[idx - 1].revision.value {
						return Err(Error::new(
							entry.revision.span,
							format!("duplicate revision number {}", entry.revision.value),
						));
					}
					return Err(Error::new(
						entry.revision.span,
						format!(
							"revisions must be contiguous starting from 1 (expected {expected}, found {})",
							entry.revision.value
						),
					));
				}
			}
			Ok(sorted)
		}
		(None, true) => Ok(Vec::new()),
	}
}

/// Variant-level size class for optimised-encoded enums.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VariantSize {
	Inline,
	Fixed(u8),
	Varlen,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SpannedSize {
	pub size: VariantSize,
	pub span: Span,
}

#[derive(Default, Debug)]
pub struct VariantOptions {
	pub start: Option<SpannedLit<usize>>,
	pub end: Option<SpannedLit<usize>>,
	pub convert: Option<LitStr>,
	pub default: Option<LitStr>,
	pub fields_name: Option<LitStr>,
	pub overrides: HashMap<usize, VariantOverrides>,
	/// Size class declaration for optimised encoding. Validated against the
	/// type's `HistoryEntry` list by the `ValidateOptimised` pass.
	pub size: Option<SpannedSize>,
}

#[derive(Default, Debug)]
pub struct VariantOverrides {
	pub revision: Option<SpannedLit<usize>>,
	pub discriminant: Option<SpannedLit<u32>>,
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
	Override(GroupOption<Token![override], VariantOverride>),
	Size(ValueOption<kw::size, LitStr>),
}

pub enum VariantOverride {
	Discriminant(ValueOption<kw::discriminant, SpannedLit<u32>>),
	Revision(ValueOption<kw::revision, SpannedLit<usize>>),
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
		if input.peek(Token![override]) {
			return Ok(VariantOption::Override(input.parse()?));
		}
		if input.peek(kw::size) {
			return Ok(VariantOption::Size(input.parse()?));
		}

		Err(input.error("invalid field option"))
	}
}

impl Parse for VariantOverride {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		if input.peek(kw::discriminant) {
			return Ok(VariantOverride::Discriminant(input.parse()?));
		}
		if input.peek(kw::revision) {
			return Ok(VariantOverride::Revision(input.parse()?));
		}
		Err(input.error("invalid field override"))
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
				VariantOption::Size(x) => {
					if res.size.is_some() {
						return Err(Error::new(x.key.span(), "tried to set an option twice"));
					}
					let span = x.value.span();
					let parsed =
						parse_variant_size(&x.value).map_err(|msg| Error::new(span, msg))?;
					res.size = Some(SpannedSize {
						size: parsed,
						span,
					});
				}
				VariantOption::Override(x) => {
					let mut overrides = VariantOverrides::default();
					for x in x.value.into_iter() {
						match x {
							VariantOverride::Discriminant(x) => {
								if overrides.discriminant.is_some() {
									return Err(Error::new(
										x.key.span(),
										"tried to set an override option twice",
									));
								}
								overrides.discriminant = Some(x.value);
							}
							VariantOverride::Revision(x) => {
								if overrides.revision.is_some() {
									return Err(Error::new(
										x.key.span(),
										"tried to set an override option twice",
									));
								}
								overrides.revision = Some(x.value);
							}
						}
					}
					let Some(revision) = overrides.revision.as_ref() else {
						return Err(Error::new(
							x.key.span(),
							"missing the revision on which the override applies",
						));
					};
					let revision = revision.value;
					res.overrides.insert(revision, overrides);
				}
			}
		}

		if let Some(kw) = end_kw
			&& res.convert.is_none()
		{
			return Err(Error::new(
				kw.span(),
				"setting a ending revision for a variant also requires a convert_fn",
			));
		}

		Ok(res)
	}
}

/// Parse `"inline"`, `"varlen"`, or `"fixed(N)"` into a [`VariantSize`].
fn parse_variant_size(lit: &LitStr) -> Result<VariantSize, String> {
	let raw = lit.value();
	let s = raw.trim();
	if s == "inline" {
		return Ok(VariantSize::Inline);
	}
	if s == "varlen" {
		return Ok(VariantSize::Varlen);
	}
	if let Some(rest) = s.strip_prefix("fixed(")
		&& let Some(num_str) = rest.strip_suffix(')')
	{
		let n: u8 = num_str
			.trim()
			.parse()
			.map_err(|_| format!("invalid `fixed(N)` size: `{num_str}` is not a u8"))?;
		return Ok(VariantSize::Fixed(n));
	}
	Err(format!("unknown size class `{s}` (expected `inline`, `varlen`, or `fixed(N)`)"))
}
