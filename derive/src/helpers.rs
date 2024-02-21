use crate::common::Exists;

/// Compute current struct revision by finding the latest field change revision.
pub(crate) fn compute_revision<T>(fields: &[T]) -> u16
where
	T: Exists,
{
	let mut revision = 1;
	for field in fields {
		let beg = field.start_revision();
		let end = field.end_revision();
		let sub = field.sub_revision();
		revision = revision.max(beg).max(end.unwrap_or(0)).max(sub);
	}
	revision
}
