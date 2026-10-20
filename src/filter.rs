use arrow::array::{ArrayRef, BooleanArray, Scalar, StringArray, UInt32Array};
use arrow::compute::kernels::cmp::eq;
use arrow::compute::kernels::comparison::ilike;
use arrow::datatypes::DataType;
use arrow::error::ArrowError;

fn is_string_type(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
    )
}

fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());

    for c in s.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }

        out.push(c);
    }

    out
}

pub fn matching_rows_for(column: &ArrayRef, needle: &ArrayRef) -> Result<UInt32Array, ArrowError> {
    let needle = Scalar::new(needle.clone());

    let mask: BooleanArray = if is_string_type(column.data_type()) {
        ilike(column, &needle)?
    } else {
        eq(column, &needle)?
    };

    let indices = mask
        .iter()
        .enumerate()
        .filter_map(|(i, keep)| (keep == Some(true)).then_some(i as u32))
        .collect();

    Ok(indices)
}

/// Returns the indices of the rows in `column` that match the given `value`.
pub fn matching_rows(column: &ArrayRef, value: &str) -> Result<UInt32Array, ArrowError> {
    let text = if is_string_type(column.data_type()) {
        format!("%{}%", escape_like(value))
    } else {
        value.to_string()
    };

    // Cast the received text into the column type for which we are searching for
    let needle = arrow::compute::cast(&StringArray::from(vec![text]), column.data_type())?;
    matching_rows_for(column, &needle)
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

    #[test]
    fn finds_single_match_native_type() {
        let values = Arc::new(Int32Array::from(vec![1, 2, 3, 4, 5])) as ArrayRef;

        let needle = Arc::new(Int32Array::from(vec![5])) as ArrayRef;
        let indices = super::matching_rows_for(&values, &needle).unwrap();

        assert_eq!(indices.len(), 1);
        assert_eq!(indices.values(), &[4]);
    }

    #[test]
    fn partial_search_on_string() {
        let values = Arc::new(StringArray::from(vec!["foo", "bar", "baz", "foo"])) as ArrayRef;

        let indices = super::matching_rows(&values, "fo").unwrap();
        assert_eq!(indices.values(), &[0, 3]);

        let indices = super::matching_rows(&values, "ba").unwrap();
        assert_eq!(indices.values(), &[1, 2]);
    }

    #[test]
    fn partial_search_escaping() {
        let values = Arc::new(StringArray::from(vec!["foo", "bar", "baz", "%foo"])) as ArrayRef;

        // Search for `%fo` literally
        let indices = super::matching_rows(&values, "%fo").unwrap();
        assert_eq!(indices.values(), &[3]);
    }

    #[test]
    fn escape_like_works() {
        let search_text = "%foo";
        assert_eq!(super::escape_like(search_text), "\\%foo");

        let search_text = "_foo";
        assert_eq!(super::escape_like(search_text), "\\_foo");

        let search_text = "\\foo";
        assert_eq!(super::escape_like(search_text), "\\\\foo");
    }
}
