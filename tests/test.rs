#![allow(clippy::excessive_precision)]
#![allow(clippy::box_collection)]

use revision::revisioned;
use revision::Error;
use revision::{DeserializeRevisioned, SerializeRevisioned};
use std::num::Wrapping;

#[revisioned(revision = 3)]
#[derive(Debug, PartialEq)]
pub enum TestEnum {
	Zero,
	#[revision(end = 2, convert_fn = "upgrade_one")]
	One(u32),
	#[revision(start = 2)]
	Two(u64),
	#[revision(start = 2)]
	Three {
		a: i64,
		#[revision(end = 3, convert_fn = "upgrade_three_b")]
		b: f32,
		#[revision(start = 3, default_fn = "default_three_c")]
		c: f64,
		#[revision(start = 3, default_fn = "default_three_d")]
		d: String,
	},
	#[revision(end = 3, convert_fn = "upgrade_four", fields_name = "OldTestEnumFourFields")]
	Four,
	#[revision(start = 3)]
	Four(usize),
	Five(#[revision(end = 3, convert_fn = "upgrade_five_field")] u64, #[revision(start = 3)] i64),
}

impl TestEnum {
	fn default_three_c(_revision: u16) -> Result<f64, Error> {
		Ok(0.0)
	}

	fn default_three_d(_revision: u16) -> Result<String, Error> {
		Ok("Foo".to_string())
	}

	fn upgrade_one(fields: TestEnumOneFields, _revision: u16) -> Result<Self, Error> {
		Ok(Self::Two(fields.0 as u64))
	}
	fn upgrade_three_b(
		fields: &mut TestEnumThreeFields,
		_revision: u16,
		value: f32,
	) -> Result<(), Error> {
		fields.c = value as f64;
		Ok(())
	}

	fn upgrade_four(_fields: OldTestEnumFourFields, _revision: u16) -> Result<TestEnum, Error> {
		Ok(TestEnum::Four(0))
	}

	fn upgrade_five_field(
		fields: &mut TestEnumFiveFields,
		_revision: u16,
		v: u64,
	) -> Result<(), Error> {
		fields.0 = v as i64;
		Ok(())
	}
}

#[revisioned(revision = 1)]
#[derive(Debug, Default, PartialEq)]
pub struct TestUnit;

#[revisioned(revision = 1)]
#[derive(Debug, Default, PartialEq)]
pub struct TestTuple1(pub Vec<i64>);

#[revisioned(revision = 2)]
#[derive(Debug, Default, PartialEq)]
pub struct TestTuple2(
	#[revision(end = 2, convert_fn = "convert_tuple")] pub Vec<i64>,
	#[revision(start = 2)] pub Vec<f64>,
);

impl TestTuple2 {
	fn convert_tuple(&mut self, _revision: u16, old: Vec<i64>) -> Result<(), Error> {
		self.0 = old.into_iter().map(|v| v as f64).collect();
		Ok(())
	}
}

// Used to serialize the struct at revision 1
#[revisioned(revision = 1)]
#[derive(Debug, PartialEq)]
pub struct Tester1 {
	#[revision(start = 1)] // used to force the version to 1
	usize_1: usize,
	u16_1: u16,
	u64_1: u64,
	i8_1: i8,
	i32_1: i32,
	f32_1: f32,
	f64_1: f64,
	char_1: char,
	string_1: String,
	enum_1: TestEnum,
	option_1: Option<u8>,
	#[allow(clippy::box_collection)] // we want to explicitly test Box
	box_1: Box<String>,
}

// Used to serialize the struct at revision 2
#[revisioned(revision = 2)]
#[derive(Debug, PartialEq)]
pub struct Tester2 {
	#[revision(start = 2)] // used to force the version to 2
	usize_1: usize,
	isize_1: isize,
	u16_1: u16,
	u64_1: u64,
	i8_1: i8,
	i16_1: i16,
	i32_1: i32,
	i64_1: i64,
	f32_1: f32,
	f64_1: f64,
	char_1: char,
	bool_1: bool,
	string_1: String,
	enum_1: TestEnum,
	option_1: Option<u8>,
	#[allow(clippy::box_collection)] // we want to explicitly test Box
	box_1: Box<String>,
}

// Used to serialize the struct at revision 3
#[revisioned(revision = 3)]
#[derive(Debug, PartialEq)]
pub struct Tester3 {
	#[revision(start = 3)] // used to force the version to 3
	usize_1: usize,
	isize_1: isize,
	u16_1: u16,
	i8_1: i8,
	i32_1: i32,
	f32_1: f32,
	f64_1: f64,
	char_1: char,
	bool_1: bool,
	string_1: String,
	enum_1: TestEnum,
	option_1: Option<u8>,
	vec_1: Vec<char>,
	unit_1: TestUnit,
	tuple_1: TestTuple1,
	#[allow(clippy::box_collection)] // we want to explicitly test Box
	box_1: Box<String>,
	wrapping_1: Wrapping<u32>,
}

#[revisioned(revision = 4)]
#[derive(Debug, PartialEq)]
pub struct Tester4 {
	usize_1: usize,
	#[revision(start = 2, end = 4, convert_fn = "convert_isize_1")]
	isize_1: isize,
	u16_1: u16,
	#[revision(end = 3, convert_fn = "convert_u64_1")]
	u64_1: u64,
	i8_1: i8,
	#[revision(start = 2, end = 3, convert_fn = "convert_i16_1")]
	i16_1: i16,
	i32_1: i32,
	#[revision(start = 2, end = 3, convert_fn = "convert_i64_1")]
	i64_1: i64,
	f32_1: f32,
	f64_1: f64,
	char_1: char,
	#[revision(start = 2, default_fn = "default_bool")]
	bool_1: bool,
	string_1: String,
	enum_1: TestEnum,
	option_1: Option<u8>,
	#[revision(start = 3, end = 4, convert_fn = "convert_vec_1")]
	vec_1: Vec<char>,
	#[revision(start = 3)]
	unit_1: TestUnit,
	#[revision(start = 3, default_fn = "default_tuple_1")]
	tuple_1: TestTuple1,
	#[allow(clippy::box_collection)] // we want to explicitly test Box
	box_1: Box<String>,
	#[revision(start = 3, default_fn = "default_wrapping_1")]
	wrapping_1: Wrapping<u32>,
}

#[revisioned(revision = 1)]
#[derive(Debug, PartialEq)]
pub struct TestSerializeAndDeserialize {
	usize_1: usize,
}

#[revisioned(revision = 1, serialize = false)]
#[derive(Debug, PartialEq)]
pub struct TestDeserializeOnly {
	usize_1: usize,
}

impl PartialEq<TestDeserializeOnly> for TestSerializeAndDeserialize {
	fn eq(&self, other: &TestDeserializeOnly) -> bool {
		self.usize_1 == other.usize_1
	}
}

impl PartialEq<TestSerializeAndDeserialize> for TestDeserializeOnly {
	fn eq(&self, other: &TestSerializeAndDeserialize) -> bool {
		self.usize_1 == other.usize_1
	}
}

#[revisioned(revision = 1, deserialize = false)]
#[derive(Debug, PartialEq)]
pub struct TestSerializeOnly {
	usize_1: usize,
}

impl PartialEq<TestSerializeOnly> for TestSerializeAndDeserialize {
	fn eq(&self, other: &TestSerializeOnly) -> bool {
		self.usize_1 == other.usize_1
	}
}

impl PartialEq<TestSerializeAndDeserialize> for TestSerializeOnly {
	fn eq(&self, other: &TestSerializeAndDeserialize) -> bool {
		self.usize_1 == other.usize_1
	}
}

impl Tester4 {
	pub fn default_bool(_revision: u16) -> Result<bool, revision::Error> {
		Ok(true)
	}
	pub fn convert_isize_1(&self, _revision: u16, _value: isize) -> Result<(), revision::Error> {
		Ok(())
	}
	pub fn convert_u64_1(&self, _revision: u16, _value: u64) -> Result<(), revision::Error> {
		Ok(())
	}
	pub fn convert_i16_1(&self, _revision: u16, _value: i16) -> Result<(), revision::Error> {
		Ok(())
	}
	pub fn convert_i64_1(&self, _revision: u16, _value: i64) -> Result<(), revision::Error> {
		Ok(())
	}
	pub fn convert_vec_1(&self, _revision: u16, _value: Vec<char>) -> Result<(), revision::Error> {
		Ok(())
	}
	pub fn default_wrapping_1(_revision: u16) -> Result<Wrapping<u32>, revision::Error> {
		Ok(Wrapping(19348719))
	}
	pub fn default_tuple_1(_revision: u16) -> Result<TestTuple1, revision::Error> {
		Ok(TestTuple1(vec![1039481, 30459830]))
	}
}

#[test]
fn test_enum() {
	// Version 1
	let version_1 = Tester1 {
		usize_1: 57918374,
		u16_1: 19357,
		u64_1: 194712409848,
		i8_1: 123,
		i32_1: 5283715,
		f32_1: 6739457.293487,
		f64_1: 394857394.987219847,
		char_1: 'x',
		string_1: String::from("this is a test"),
		enum_1: TestEnum::Zero,
		option_1: Some(17),
		box_1: Box::new(String::from("this was a test")),
	};
	let mut data_1: Vec<u8> = vec![];
	let result = version_1.serialize_revisioned(&mut data_1);
	assert!(result.is_ok());
	let result = Tester1::deserialize_revisioned(&mut data_1.as_slice());
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), version_1);
	// Version 2
	let version_2 = Tester2 {
		usize_1: 57918374,
		isize_1: 18540294,
		u16_1: 19357,
		u64_1: 194712409848,
		i8_1: 123,
		i16_1: 32753,
		i32_1: 5283715,
		i64_1: 194738194731,
		f32_1: 6739457.293487,
		f64_1: 394857394.987219847,
		char_1: 'x',
		bool_1: true,
		string_1: String::from("this is a test"),
		enum_1: TestEnum::Zero,
		option_1: Some(17),
		box_1: Box::new(String::from("this was a test")),
	};
	let mut data_2: Vec<u8> = vec![];
	let result = version_2.serialize_revisioned(&mut data_2);
	assert!(result.is_ok());
	let result = Tester2::deserialize_revisioned(&mut data_2.as_slice());
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), version_2);
	// Version 3
	let version_3 = Tester3 {
		usize_1: 57918374,
		isize_1: 18540294,
		u16_1: 19357,
		i8_1: 123,
		i32_1: 5283715,
		f32_1: 6739457.293487,
		f64_1: 394857394.987219847,
		char_1: 'x',
		bool_1: true,
		string_1: String::from("this is a test"),
		enum_1: TestEnum::Zero,
		option_1: Some(17),
		vec_1: vec!['a', 'b', 'c'],
		unit_1: TestUnit {},
		tuple_1: TestTuple1(vec![1039481, 30459830]),
		box_1: Box::new(String::from("this was a test")),
		wrapping_1: Wrapping(19348719),
	};
	let mut data_3: Vec<u8> = vec![];
	let result = version_3.serialize_revisioned(&mut data_3);
	assert!(result.is_ok());
	let result = Tester3::deserialize_revisioned(&mut data_3.as_slice());
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), version_3);
	// Version 4
	let version_4 = Tester4 {
		usize_1: 57918374,
		u16_1: 19357,
		i8_1: 123,
		i32_1: 5283715,
		f32_1: 6739457.293487,
		f64_1: 394857394.987219847,
		char_1: 'x',
		bool_1: true,
		string_1: String::from("this is a test"),
		enum_1: TestEnum::Zero,
		option_1: Some(17),
		unit_1: TestUnit {},
		tuple_1: TestTuple1(vec![1039481, 30459830]),
		box_1: Box::new(String::from("this was a test")),
		wrapping_1: Wrapping(19348719),
	};
	let mut data_4: Vec<u8> = vec![];
	let result = version_4.serialize_revisioned(&mut data_4);
	assert!(result.is_ok());
	let result = Tester4::deserialize_revisioned(&mut data_4.as_slice());
	assert!(result.is_ok());
	// Version final
	let version_final = Tester4 {
		usize_1: 57918374,
		u16_1: 19357,
		i8_1: 123,
		i32_1: 5283715,
		f32_1: 6739457.293487,
		f64_1: 394857394.987219847,
		char_1: 'x',
		bool_1: true,
		string_1: String::from("this is a test"),
		enum_1: TestEnum::Zero,
		option_1: Some(17),
		unit_1: TestUnit {},
		tuple_1: TestTuple1(vec![1039481, 30459830]),
		box_1: Box::new(String::from("this was a test")),
		wrapping_1: Wrapping(19348719),
	};
	assert_eq!(result.unwrap(), version_final);
	//
	let result = Tester4::deserialize_revisioned(&mut data_1.as_slice());
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), version_final);
	//
	let result = Tester4::deserialize_revisioned(&mut data_2.as_slice());
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), version_final);
	//
	let result = Tester4::deserialize_revisioned(&mut data_3.as_slice());
	println!("{:?}", result);
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), version_final);
	//
	let result = Tester4::deserialize_revisioned(&mut data_4.as_slice());
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), version_final);
}

#[test]
fn test_serialize_disabled() {
	let val = TestSerializeAndDeserialize {
		usize_1: 57918374,
	};
	let mut mem: Vec<u8> = vec![];
	val.serialize_revisioned(&mut mem).unwrap();
	assert_eq!(mem.len(), 6);

	let out = TestDeserializeOnly::deserialize_revisioned(&mut mem.as_slice()).unwrap();
	assert_eq!(val, out);
}

#[test]
fn test_deserialize_disabled() {
	let val = TestSerializeOnly {
		usize_1: 57918374,
	};
	let mut mem: Vec<u8> = vec![];
	val.serialize_revisioned(&mut mem).unwrap();
	assert_eq!(mem.len(), 6);

	let out = TestSerializeAndDeserialize::deserialize_revisioned(&mut mem.as_slice()).unwrap();
	assert_eq!(val, out);
}
