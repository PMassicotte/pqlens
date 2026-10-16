use arrow::array::{ArrayRef, BooleanArray, Scalar, StringArray, UInt32Array};
use arrow::compute::kernels::cmp::eq;
use arrow::error::ArrowError;

/// Returns the indices of the rows in `column` that match the given `value`.
pub fn matching_rows(column: &ArrayRef, value: &str) -> Result<UInt32Array, ArrowError> {
    // Convert the user input (String) into the real column type
    let needle = arrow::compute::cast(&StringArray::from(vec![value]), column.data_type())?;

    // Need this because eq expect same size array, but here needle is only of length 1
    let mask: BooleanArray = eq(column, &Scalar::new(needle))?;

    let indices = mask
        .iter()
        .enumerate()
        .filter_map(|(i, keep)| (keep == Some(true)).then_some(i as u32))
        .collect();

    Ok(indices)
}
