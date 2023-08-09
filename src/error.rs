use thiserror::Error;

/// An error which occurs when revisioned serialization / deserialization fails.
#[derive(Error, Debug, PartialEq)]
pub enum Error {
	/// An IO error occured.
	Io(i32),
	/// Generic serialization error.
	Serialize(String),
	/// Generic deserialization error.
	Deserialize(String),
	/// Semantic translation/validation error.
	Conversion(String),
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
		match self {
			Self::Io(e) => write!(f, "An IO error occured: {}", e),
			Self::Serialize(e) => write!(f, "A serialization error occured: {}", e),
			Self::Deserialize(e) => write!(f, "A deserialization error occured: {}", e),
			Self::Conversion(e) => write!(f, "A user generated conversion error occured: {}", e),
		}
	}
}
