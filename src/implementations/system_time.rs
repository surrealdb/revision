use crate::DeserializeRevisioned;
use crate::SerializeRevisioned;

use super::super::Error;
use super::super::Revisioned;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

impl SerializeRevisioned for SystemTime {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		let duration =
			self.duration_since(UNIX_EPOCH).map_err(|e| Error::Serialize(e.to_string()))?;
		duration.as_secs().serialize_revisioned(writer)?;
		duration.subsec_nanos().serialize_revisioned(writer)
	}
}

impl DeserializeRevisioned for SystemTime {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let secs = u64::deserialize_revisioned(reader)?;
		let nanos = u32::deserialize_revisioned(reader)?;
		Ok(UNIX_EPOCH + Duration::new(secs, nanos))
	}
}

impl Revisioned for SystemTime {
	#[inline]
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Error;

	#[test]
	fn test_system_time_now() {
		let val = SystemTime::now();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_system_time_epoch() {
		let val = UNIX_EPOCH;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_system_time_future() {
		let val = UNIX_EPOCH + Duration::new(u32::MAX as u64, 999_999_999);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_system_time_pre_epoch() {
		let val = UNIX_EPOCH - Duration::new(1, 0);
		let mut mem: Vec<u8> = vec![];
		let result = val.serialize_revisioned(&mut mem);
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), Error::Serialize(_)));
	}

	#[test]
	fn test_system_time_one_sec_after_epoch() {
		let val = UNIX_EPOCH + Duration::new(1, 0);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_system_time_max_nanos() {
		let val = UNIX_EPOCH + Duration::new(0, 999_999_999);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_system_time_large_secs() {
		let large_secs = i64::MAX as u64;
		let val = UNIX_EPOCH + Duration::new(large_secs, 999_999_999);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_system_time_revision() {
		assert_eq!(<SystemTime as Revisioned>::revision(), 1);
	}

	#[test]
	fn test_system_time_serialized_bytes() {
		let val = UNIX_EPOCH + Duration::new(12345, 678);
		let mut time_bytes: Vec<u8> = vec![];
		val.serialize_revisioned(&mut time_bytes).unwrap();

		let duration = val.duration_since(UNIX_EPOCH).unwrap();
		let mut expected_bytes: Vec<u8> = vec![];
		duration.as_secs().serialize_revisioned(&mut expected_bytes).unwrap();
		duration.subsec_nanos().serialize_revisioned(&mut expected_bytes).unwrap();

		assert_eq!(time_bytes, expected_bytes);
	}

	#[test]
	fn test_system_time_truncated_data() {
		let val = UNIX_EPOCH + Duration::new(12345, 678);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();

		let truncated = &mem[..2];
		let result =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut truncated.as_ref());
		assert!(result.is_err());
	}

	#[test]
	fn test_system_time_empty_reader() {
		let empty: &[u8] = &[];
		let result =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut empty.as_ref());
		assert!(result.is_err());
	}

	#[test]
	fn test_system_time_known_timestamp() {
		let secs_2000_01_01 = 946684800u64;
		let val = UNIX_EPOCH + Duration::new(secs_2000_01_01, 0);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);

		let duration = out.duration_since(UNIX_EPOCH).unwrap();
		assert_eq!(duration.as_secs(), secs_2000_01_01);
		assert_eq!(duration.subsec_nanos(), 0);
	}

	#[test]
	fn test_system_time_with_nanos() {
		let val = UNIX_EPOCH + Duration::new(1000, 123_456_789);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);

		let duration = out.duration_since(UNIX_EPOCH).unwrap();
		assert_eq!(duration.as_secs(), 1000);
		assert_eq!(duration.subsec_nanos(), 123_456_789);
	}

	#[test]
	fn test_system_time_one_nanosecond() {
		let val = UNIX_EPOCH + Duration::new(0, 1);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		let out =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_system_time_epoch_bytes() {
		let val = UNIX_EPOCH;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();

		let mut expected: Vec<u8> = vec![];
		0u64.serialize_revisioned(&mut expected).unwrap();
		0u32.serialize_revisioned(&mut expected).unwrap();

		assert_eq!(mem, expected);
	}

	#[test]
	fn test_system_time_partial_secs_truncated() {
		let val = UNIX_EPOCH + Duration::new(12345, 678);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();

		let secs_only = &mem[..mem.len() - 1];
		let result =
			<SystemTime as DeserializeRevisioned>::deserialize_revisioned(&mut secs_only.as_ref());
		assert!(result.is_err());
	}
}
