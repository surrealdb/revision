use revision::revisioned;
use revision::Error;
use revision::Revisioned;
use std::num::Wrapping;

#[derive(Debug, PartialEq)]
#[revisioned(revision = 3)]
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
		#[revision(start = 3)]
		c: f64,
		#[revision(start = 3)]
		d: String,
	},
}

impl TestEnum {
	fn upgrade_one(_revision: u16, (v0,): (u32,)) -> Result<TestEnum, Error> {
		Ok(Self::Two(v0 as u64))
	}
	fn upgrade_three_b(&mut self, _revision: u16, value: f32) -> Result<(), Error> {
		match self {
			TestEnum::Three {
				ref mut c,
				..
			} => {
				*c = value as f64;
			}
			_ => unreachable!(),
		}
		Ok(())
	}
}

#[derive(Debug, Default, PartialEq)]
#[revisioned(revision = 1)]
pub struct TestUnit;

#[derive(Debug, Default, PartialEq)]
#[revisioned(revision = 1)]
pub struct TestTuple(pub Vec<i64>);

// Used to serialize the struct at revision 1
#[derive(Debug, PartialEq)]
#[revisioned(revision = 1)]
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
#[derive(Debug, PartialEq)]
#[revisioned(revision = 2)]
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
#[derive(Debug, PartialEq)]
#[revisioned(revision = 3)]
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
	tuple_1: TestTuple,
	#[allow(clippy::box_collection)] // we want to explicitly test Box
	box_1: Box<String>,
	wrapping_1: Wrapping<u32>,
}

#[derive(Debug, PartialEq)]
#[revisioned(revision = 4)]
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
	tuple_1: TestTuple,
	#[allow(clippy::box_collection)] // we want to explicitly test Box
	box_1: Box<String>,
	#[revision(start = 3, default_fn = "default_wrapping_1")]
	wrapping_1: Wrapping<u32>,
}

impl Tester4 {
	pub fn default_bool(_revision: u16) -> bool {
		true
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
	pub fn default_wrapping_1(_revision: u16) -> Wrapping<u32> {
		Wrapping(19348719)
	}
	pub fn default_tuple_1(_revision: u16) -> TestTuple {
		TestTuple(vec![1039481, 30459830])
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
		tuple_1: TestTuple(vec![1039481, 30459830]),
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
		vec_1: vec!['a', 'b', 'c'],
		unit_1: TestUnit {},
		tuple_1: TestTuple(vec![1039481, 30459830]),
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
		isize_1: 0,
		u16_1: 19357,
		u64_1: 0,
		i8_1: 123,
		i16_1: 0,
		i32_1: 5283715,
		i64_1: 0,
		f32_1: 6739457.293487,
		f64_1: 394857394.987219847,
		char_1: 'x',
		bool_1: true,
		string_1: String::from("this is a test"),
		enum_1: TestEnum::Zero,
		option_1: Some(17),
		vec_1: vec![],
		unit_1: TestUnit {},
		tuple_1: TestTuple(vec![1039481, 30459830]),
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
