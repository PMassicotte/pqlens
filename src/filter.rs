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

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use arrow::array::{ArrayRef, Int32Array, StringArray};

    #[test]
    fn finds_single_match() {
        let values = Arc::new(Int32Array::from(vec![1, 2, 3, 4, 5])) as ArrayRef;
        let indices = super::matching_rows(&values, "3").unwrap();

        assert_eq!(indices.len(), 1);
        assert_eq!(indices.values(), &[2]);
    }

    #[test]
    fn finds_two_match() {
        let values = Arc::new(Int32Array::from(vec![1, 2, 3, 4, 1])) as ArrayRef;
        let indices = super::matching_rows(&values, "1").unwrap();

        assert_eq!(indices.values(), &[0, 4]);
    }

    #[test]
    fn finds_nothing() {
        let values = Arc::new(Int32Array::from(vec![1, 2, 3, 4, 1])) as ArrayRef;
        let indices = super::matching_rows(&values, "10").unwrap();

        assert!(indices.is_empty());
    }

    #[test]
    fn finds_string_match() {
        let values = Arc::new(StringArray::from(vec!["foo", "bar", "baz", "foo"])) as ArrayRef;
        let indices = super::matching_rows(&values, "foo").unwrap();

        assert_eq!(indices.values(), &[0, 3]);
    }
}
