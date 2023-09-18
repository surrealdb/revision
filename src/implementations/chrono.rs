use super::super::Error;
use super::super::Revisioned;
use chrono::{offset::TimeZone, DateTime, Utc, NaiveDate, Datelike};

impl Revisioned for DateTime<Utc> {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.timestamp().serialize_revisioned(writer)?;
		self.timestamp_subsec_nanos().serialize_revisioned(writer)?;
		Ok(())
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let secs = <i64 as Revisioned>::deserialize_revisioned(reader)?;
		let nano = <u32 as Revisioned>::deserialize_revisioned(reader)?;
		Utc.timestamp_opt(secs, nano)
			.single()
			.ok_or_else(|| Error::Deserialize("invalid datetime".to_string()))
	}

	fn revision() -> u16 {
		1
	}
}

impl Revisioned for NaiveDate {
	#[inline]
	fn serialize_revisioned<W: std::io::Write>(&self, writer: &mut W) -> Result<(), Error> {
		self.year().serialize_revisioned(writer)?;
		self.month().serialize_revisioned(writer)?;
		self.day().serialize_revisioned(writer)?;
		Ok(())
	}

	#[inline]
	fn deserialize_revisioned<R: std::io::Read>(reader: &mut R) -> Result<Self, Error> {
		let year = <i32 as Revisioned>::deserialize_revisioned(reader)?;
		let month = <u32 as Revisioned>::deserialize_revisioned(reader)?;
		let day = <u32 as Revisioned>::deserialize_revisioned(reader)?;
		Ok(NaiveDate::from_ymd_opt(year, month, day)
			.ok_or_else(|| Error::Deserialize("invalid date".to_string()))?)
	}

	fn revision() -> u16 {
		1
	}
}

#[cfg(test)]
mod tests {
	use chrono::NaiveDate;
	use super::DateTime;
	use super::Revisioned;
	use super::Utc;

	#[test]
	fn test_datetime_min() {
		let val = DateTime::<Utc>::MIN_UTC;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 10);
		let out =
			<DateTime<Utc> as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_datetime_max() {
		let val = DateTime::<Utc>::MAX_UTC;
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 14);
		let out =
			<DateTime<Utc> as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_naive_date_min() {
		let val = NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 3);
		let out =
			<NaiveDate as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_naive_date_max() {
		let val = NaiveDate::from_ymd_opt(9999, 12, 31).unwrap();
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 5);
		let out =
			<NaiveDate as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}
}
