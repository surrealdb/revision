use super::super::Error;
use super::super::Revisioned;

macro_rules! impl_tuple {
	($($n:ident),*$(,)?) => {
		impl_tuple!{@marker $($n,)*}
	};

	($($n:ident,)* @marker $head:ident, $($tail:ident,)*) => {
		impl<$($n),*> Revisioned for ($($n,)*)
			where $($n: Revisioned),*
		{
			fn revision() -> u16{
				1
			}

			#[inline]
			#[allow(non_snake_case)]
			fn serialize_revisioned<W: std::io::Write>(&self, _writer: &mut W) -> Result<(), Error> {
				let ($(ref $n,)*) = *self;
				$(
					$n.serialize_revisioned(_writer)?;
				)*
				Ok(())
			}

			#[inline]
			#[allow(non_snake_case)]
			fn deserialize_revisioned<R: std::io::Read>(_reader: &mut R) -> Result<Self, Error> {
				$(
					let $n = Revisioned::deserialize_revisioned(_reader)?;
				)*
				Ok(($($n,)*))
			}
		}

		impl_tuple!{$($n,)* $head, @marker $($tail,)*}

	};
	($($n:ident,)* @marker) => {
		impl<$($n),*> Revisioned for ($($n),*)
			where $($n: Revisioned),*
		{
			fn revision() -> u16{
				1
			}

			#[inline]
			#[allow(non_snake_case)]
			fn serialize_revisioned<W: std::io::Write>(&self, _writer: &mut W) -> Result<(), Error> {
				let ($(ref $n),*) = self;
				$(
					$n.serialize_revisioned(_writer)?;
				)*
				Ok(())
			}

			#[inline]
			#[allow(non_snake_case)]
			fn deserialize_revisioned<R: std::io::Read>(_reader: &mut R) -> Result<Self, Error> {
				$(
					let $n = Revisioned::deserialize_revisioned(_reader)?;
				)*
				Ok(($($n),*))
			}
		}
	};
}

impl_tuple! { A,B,C,D,E,F }

#[cfg(test)]
mod tests {

	use crate::implementations::assert_bincode_compat;

	use super::Revisioned;

	#[test]
	fn test_tuple_2() {
		let val = (String::from("test"), true);
		assert_bincode_compat(&val);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 6);
		let out =
			<(String, bool) as Revisioned>::deserialize_revisioned(&mut mem.as_slice()).unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_tuple_3() {
		let val = (String::from("test"), true, 1419247293847192847.13947134978139487);
		assert_bincode_compat(&val);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 14);
		let out = <(String, bool, f64) as Revisioned>::deserialize_revisioned(&mut mem.as_slice())
			.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_tuple_4() {
		let val = (String::from("test"), true, 1419247293847192847.13947134978139487, Some('t'));
		assert_bincode_compat(&val);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 16);
		let out = <(String, bool, f64, Option<char>) as Revisioned>::deserialize_revisioned(
			&mut mem.as_slice(),
		)
		.unwrap();
		assert_eq!(val, out);
	}

	#[test]
	fn test_tuple_5() {
		let val = (
			String::from("test"),
			true,
			1419247293847192847.13947134978139487,
			Some('t'),
			vec![4u8, 19u8, 133u8],
		);
		assert_bincode_compat(&val);
		let mut mem: Vec<u8> = vec![];
		val.serialize_revisioned(&mut mem).unwrap();
		assert_eq!(mem.len(), 20);
		let out =
			<(String, bool, f64, Option<char>, Vec<u8>) as Revisioned>::deserialize_revisioned(
				&mut mem.as_slice(),
			)
			.unwrap();
		assert_eq!(val, out);
	}
}
