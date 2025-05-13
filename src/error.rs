use std::{io, str::Utf8Error};

/// An error which occurs when revisioned serialization / deserialization fails.
#[derive(Debug)]
pub enum Error {
	/// An IO error occured.
	Io(io::Error),
	/// Tried to deserialize a boolean value with an invalid byte value.
	InvalidBoolValue(u8),
	/// Deserialization encountered integer encoding which is not suported.
	InvalidIntegerEncoding,
	/// Deserialization encountered an integer with a value which did not fit the target type..
	IntegerOverflow,
	/// Path contains invalid utf-8 characters
	InvalidPath,
	/// Invalid character encoding
	InvalidCharEncoding,
	/// Error parsing a string
	Utf8Error(Utf8Error),
	/// Failed to serialize character.
	Serialize(String),
	/// Generic deserialization error.
	Deserialize(String),
	/// Semantic translation/validation error.
	Conversion(String),
}

impl std::error::Error for Error {
	#[inline]
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Error::Io(ref x) => Some(x),
			Error::Utf8Error(ref x) => Some(x),
			_ => None,
		}
	}
}

impl std::fmt::Display for Error {
	#[inline]
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
		match self {
			Self::Io(e) => write!(f, "An IO error occured: {}", e),
			Self::InvalidBoolValue(_) => {
				write!(f, "Tried to deserialize a boolean value with an invalid byte value.")
			}
			Self::InvalidIntegerEncoding => {
				write!(f, "Encountered invalid integer encoding.")
			}
			Self::IntegerOverflow => {
				write!(f, "Encountered integer which doesn't fit the target integer type during deserialization.")
			}
			Self::InvalidPath => {
				write!(f, "Path contained invalid UTF-8 characters.")
			}
			Self::InvalidCharEncoding => {
				write!(f, "Invalid character encoding.")
			}
			Self::Utf8Error(x) => {
				write!(f, "Invalid UTF-8 characters in string: {x}")
			}
			Self::Serialize(e) => write!(f, "A serialization error occured: {}", e),
			Self::Deserialize(e) => write!(f, "A deserialization error occured: {}", e),
			Self::Conversion(e) => write!(f, "A user generated conversion error occured: {}", e),
		}
	}
}
