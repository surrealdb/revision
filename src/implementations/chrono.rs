use super::super::Error;
use super::super::Revisioned;
use chrono::{offset::TimeZone, DateTime, Utc};

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

#[cfg(test)]
mod tests {

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
}
