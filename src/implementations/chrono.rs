#![cfg(feature = "chrono")]

use super::super::Error;
use super::super::{DeserializeRevisioned, Revisioned, SerializeRevisioned};
use chrono::{offset::TimeZone, DateTime, Datelike, Duration, NaiveDate, NaiveTime, Timelike, Utc};

impl SerializeRevisioned for DateTime<Utc> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.timestamp().serialize_revisioned(writer)?;
		self.timestamp_subsec_nanos().serialize_revisioned(writer)?;
		Ok(())
	}
}

impl DeserializeRevisioned for DateTime<Utc> {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let secs = <i64 as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		let nano = <u32 as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		Utc.timestamp_opt(secs, nano)
			.single()
			.ok_or_else(|| Error::Deserialize("invalid datetime".to_string()))
	}
}

impl Revisioned for DateTime<Utc> {
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for NaiveDate {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.year().serialize_revisioned(writer)?;
		self.month().serialize_revisioned(writer)?;
		self.day().serialize_revisioned(writer)?;
		Ok(())
	}
}

impl DeserializeRevisioned for NaiveDate {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let year = <i32 as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		let month = <u32 as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		let day = <u32 as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		NaiveDate::from_ymd_opt(year, month, day)
			.ok_or_else(|| Error::Deserialize("invalid date".to_string()))
	}
}

impl Revisioned for NaiveDate {
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for NaiveTime {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.hour().serialize_revisioned(writer)?;
		self.minute().serialize_revisioned(writer)?;
		self.second().serialize_revisioned(writer)?;
		self.nanosecond().serialize_revisioned(writer)?;
		Ok(())
	}
}

impl DeserializeRevisioned for NaiveTime {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let hour = <u32 as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		let minute = <u32 as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		let second = <u32 as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		let nano = <u32 as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		NaiveTime::from_hms_nano_opt(hour, minute, second, nano)
			.ok_or_else(|| Error::Deserialize("invalid time".to_string()))
	}
}

impl Revisioned for NaiveTime {
	fn revision() -> u16 {
		1
	}
}

impl SerializeRevisioned for Duration {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		let mut secs = self.num_seconds();
		let mut nano = self.subsec_nanos();

		if nano < 0 {
			secs = secs
				.checked_sub(1)
				.ok_or_else(|| Error::Serialize("invalid duration".to_string()))?;
			nano = nano
				.checked_add(1_000_000_000)
				.ok_or_else(|| Error::Serialize("invalid duration".to_string()))?;
		}

		secs.serialize_revisioned(writer)?;
		nano.serialize_revisioned(writer)?;

		Ok(())
	}
}

impl DeserializeRevisioned for Duration {
	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let secs = <i64 as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		let nano = <i32 as DeserializeRevisioned>::deserialize_revisioned(reader)?;
		let nano =
			u32::try_from(nano).map_err(|_| Error::Deserialize("invalid duration".to_string()))?;

		Duration::new(secs, nano).ok_or_else(|| Error::Deserialize("invalid duration".to_string()))
	}
}

impl Revisioned for Duration {
	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_datetime_min() {
		let val = DateTime::<Utc>::MIN_UTC;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 10);
		let out =
			<DateTime<Utc> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_datetime_max() {
		let val = DateTime::<Utc>::MAX_UTC;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 14);
		let out =
			<DateTime<Utc> as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
				.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_naive_date_min() {
		let val = NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 3);
		let out = <NaiveDate as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_naive_date_max() {
		let val = NaiveDate::from_ymd_opt(9999, 12, 31).unwrap();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 5);
		let out = <NaiveDate as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_naive_time_min() {
		let val = NaiveTime::from_hms_nano_opt(0, 0, 0, 0).unwrap();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 4);
		let out = <NaiveTime as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_naive_time_max() {
		let val = NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999).unwrap();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 8);
		let out = <NaiveTime as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_duration_min() {
		let val = Duration::MIN;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 14);
		let out = <Duration as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_duration_zero() {
		let val = Duration::zero();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 2);
		let out = <Duration as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_duration_max() {
		let val = Duration::MAX;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 14);
		let out = <Duration as DeserializeRevisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}
}
