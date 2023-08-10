/// Describes a structure and it's fields.
pub(crate) struct GenericDescriptor<T> {
	pub ident: syn::Ident,
	pub vis: syn::Visibility,
	pub attrs: Vec<syn::Attribute>,
	pub generics: syn::Generics,
	pub revision: u16,
	pub fields: Vec<T>,
	pub simple: bool,
}

/// An interface for generating serialzer and deserializer
/// implementations for a Rust data type.
pub trait Descriptor {
	/// Returns the serializer code block as a token stream.
	fn generate_serializer(&self) -> proc_macro2::TokenStream;
	/// Returns the deserializer code block as a token stream.
	fn generate_deserializer(&self) -> proc_macro2::TokenStream;
	/// Returns the curent revision.
	fn revision(&self) -> u16;

	fn reexpand(&self) -> proc_macro2::TokenStream;
}

/// A trait that enables checking whether a certain field
/// exists at a specified revision.
pub(crate) trait Exists {
	// Get the start revision for this field
	fn start_revision(&self) -> u16;
	// Get the end revision for this field
	fn end_revision(&self) -> u16;
	// Get any sub revision for this field
	fn sub_revision(&self) -> u16;
	// Check if this field exists for this revision
	fn exists_at(&self, revision: u16) -> bool {
		// All fields have an initial start revision
		revision >= self.start_revision()
        // Not all fields have an end revision specified
        && (0 == self.end_revision()
                || (self.end_revision() > 0 && revision < self.end_revision()))
	}
}

#[cfg(test)]
mod tests {
	use super::Exists;

	#[test]
	fn test_exists_at() {
		impl Exists for u32 {
			fn start_revision(&self) -> u16 {
				3
			}

			fn end_revision(&self) -> u16 {
				5
			}

			fn sub_revision(&self) -> u16 {
				0
			}
		}

		let test = 1234;
		assert!(!test.exists_at(2));
		assert!(test.exists_at(3));
		assert!(test.exists_at(4));
		assert!(!test.exists_at(5));
		assert!(!test.exists_at(6));
	}
}
