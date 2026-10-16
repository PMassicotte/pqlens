use crate::sorter::{SortDir, SortState};
use arrow::array::{
    ArrayRef, BooleanArray, RecordBatch, RecordBatchReader, Scalar, StringArray, UInt32Array,
};
use arrow::compute::kernels::cmp::eq;
use arrow::compute::{SortOptions, concat_batches, sort_to_indices};
use arrow::error::ArrowError;
use arrow::util::display::{ArrayFormatter, FormatOptions};
use parquet::arrow::ProjectionMask;
use parquet::arrow::arrow_reader::{
    ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
};
use parquet::file::metadata::PageIndexPolicy;
use std::fs::File;

use arrow::compute::take_record_batch;
use parquet::arrow::arrow_reader::RowSelection;

const VIEWS_PER_CHUNK: usize = 100;

pub struct ParquetPreview {
    path: String,
    rows_per_view: usize,
    metadata: ArrowReaderMetadata,
    row_group_starts: Vec<usize>,
    chunk: RecordBatch,
    chunk_idx: Option<usize>,
    header: Vec<String>,
    column_types: Vec<String>,
    rows: Vec<Vec<String>>,
    view: RecordBatch,
    global_order: Option<UInt32Array>,
    current_view: usize,
    sort_state: Option<SortState>,
}

impl ParquetPreview {
    pub fn header(&self) -> &[String] {
        &self.header
    }

    pub fn total_rows(&self) -> usize {
        self.metadata.metadata().file_metadata().num_rows() as usize
    }

    // Number of rows when a filter is applied, otherwise the same as total_rows
    pub fn visible_rows(&self) -> usize {
        match &self.global_order {
            Some(order) => order.len(),
            None => self.total_rows(),
        }
    }

    pub fn column_types(&self) -> &[String] {
        &self.column_types
    }

    pub fn sort_state(&self) -> Option<SortState> {
        self.sort_state
    }

    pub fn cycle_sort(&mut self, col: usize) -> Result<(), Box<dyn std::error::Error>> {
        self.sort_state = SortState::next(self.sort_state, col);
        self.apply_sort()?;

        self.load_view(0)
    }

    pub fn current_view(&self) -> usize {
        self.current_view
    }

    pub fn num_views(&self) -> usize {
        self.visible_rows().div_ceil(self.rows_per_view)
    }

    fn apply_sort(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(s) = self.sort_state {
            self.sort_by(s.col(), s.dir() == SortDir::Desc)
        } else {
            self.clear_sort();
            Ok(())
        }
    }

    pub fn next_view(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.current_view + 1 < self.num_views() {
            self.load_view(self.current_view + 1)?;
        }

        Ok(())
    }

    pub fn previous_view(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.current_view > 0 {
            self.load_view(self.current_view - 1)?;
        }

        Ok(())
    }

    pub fn first_view(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.load_view(0)
    }

    pub fn last_view(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.load_view(self.num_views().saturating_sub(1))
    }

    // Only called when there is no global order, so we can read a contiguous chunk of rows from the
    // parquet file.
    fn view_contiguous(&mut self, idx: usize) -> Result<RecordBatch, Box<dyn std::error::Error>> {
        let chunk_idx = idx / VIEWS_PER_CHUNK;
        let chunk_rows = VIEWS_PER_CHUNK * self.rows_per_view;

        if self.chunk_idx != Some(chunk_idx) {
            let start = chunk_idx * chunk_rows;
            let len = chunk_rows.min(self.total_rows().saturating_sub(start));
            self.chunk = self.read_rows(start, len)?;
            self.chunk_idx = Some(chunk_idx);
        }

        let offset = (idx % VIEWS_PER_CHUNK) * self.rows_per_view;
        let len = self
            .rows_per_view
            .min(self.chunk.num_rows().saturating_sub(offset));

        Ok(self.chunk.slice(offset, len))
    }

    fn read_rows_at(&self, indices: &[u32]) -> Result<RecordBatch, Box<dyn std::error::Error>> {
        let schema = self.metadata.schema().clone();

        if indices.is_empty() {
            return Ok(RecordBatch::new_empty(schema));
        }

        let mut sorted = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();

        let ranges = sorted.iter().map(|&i| i as usize..i as usize + 1);

        let selection = RowSelection::from_consecutive_ranges(ranges, self.total_rows());

        let batches = ParquetRecordBatchReaderBuilder::new_with_metadata(
            File::open(&self.path)?,
            self.metadata.clone(),
        )
        .with_row_selection(selection)
        .build()?
        .collect::<Result<Vec<_>, _>>()?;

        let batch = concat_batches(&schema, &batches)?;

        let positions: UInt32Array = indices
            .iter()
            .map(|i| sorted.binary_search(i).unwrap() as u32)
            .collect();

        Ok(take_record_batch(&batch, &positions)?)
    }

    fn load_view(&mut self, idx: usize) -> Result<(), Box<dyn std::error::Error>> {
        self.view = match &self.global_order {
            None => self.view_contiguous(idx)?,
            Some(order) => {
                // NOTE: This will basically read the first 45 rows of the sorted order. I will
                // eventually revisit that.
                let start = idx * self.rows_per_view;
                let end = (start + self.rows_per_view).min(order.len());
                self.read_rows_at(&order.values()[start..end])?
            }
        };

        self.current_view = idx;
        self.rows = format_rows(&self.view)?;

        Ok(())
    }

    fn sort_by(&mut self, col: usize, descending: bool) -> Result<(), Box<dyn std::error::Error>> {
        let column = self.read_column(col)?;

        let opts = SortOptions {
            descending,
            nulls_first: false,
        };

        let idx = sort_to_indices(&column, Some(opts), None)?;

        self.global_order = Some(idx);

        Ok(())
    }

    /// Clears any sorting and resets the order to the original row order.
    fn clear_sort(&mut self) {
        self.global_order = None;
    }

    pub fn clear_filter(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.global_order = None;
        self.apply_sort()?;
        self.load_view(0)
    }

    /// Returns an iterator over the rows of the current view, already in display order.
    pub fn rows(&self) -> impl Iterator<Item = &Vec<String>> {
        self.rows.iter()
    }

    /// Reads `len` rows starting at row `start`, decoding only the row groups that contain them.
    fn read_rows(
        &self,
        start: usize,
        len: usize,
    ) -> Result<RecordBatch, Box<dyn std::error::Error>> {
        let schema = self.metadata.schema().clone();

        if len == 0 {
            return Ok(RecordBatch::new_empty(schema));
        }

        let end = start + len;
        let first_rg = self.row_group_starts.partition_point(|&s| s <= start) - 1;
        let last_rg = self.row_group_starts.partition_point(|&s| s < end) - 1;

        let batches = ParquetRecordBatchReaderBuilder::new_with_metadata(
            File::open(&self.path)?,
            self.metadata.clone(),
        )
        .with_row_groups((first_rg..=last_rg).collect())
        .with_offset(start - self.row_group_starts[first_rg])
        .with_limit(len)
        .with_batch_size(len)
        .build()?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(concat_batches(&schema, &batches)?)
    }

    // Read just 1 column from the parquet file, it will be used to sort the whole file
    pub fn read_column(&self, col: usize) -> Result<ArrayRef, Box<dyn std::error::Error>> {
        let projection_mask = ProjectionMask::roots(self.metadata.parquet_schema(), [col]);

        let reader = ParquetRecordBatchReaderBuilder::new_with_metadata(
            File::open(&self.path)?,
            self.metadata.clone(),
        )
        .with_projection(projection_mask)
        .build()?;

        let schema = reader.schema();
        let batches = reader.collect::<Result<Vec<_>, _>>()?;

        let column = concat_batches(&schema, &batches)?.column(0).clone();

        Ok(column)
    }

    fn filter_rows(
        &self,
        col: usize,
        value: &str,
    ) -> Result<UInt32Array, Box<dyn std::error::Error>> {
        let column = self.read_column(col)?;

        // Convert the user input (String) into the real column type
        let needle = arrow::compute::cast(&StringArray::from(vec![value]), column.data_type())?;

        // Need this because eq expect same size array, but here needle is only of length 1
        let mask: BooleanArray = eq(&column, &Scalar::new(needle))?;

        let indices = mask
            .iter()
            .enumerate()
            .filter_map(|(i, keep)| (keep == Some(true)).then_some(i as u32))
            .collect();

        Ok(indices)
    }

    fn apply_filter(&mut self, col: usize, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        let idx = self.filter_rows(col, value)?;

        self.global_order = Some(idx);

        Ok(())
    }

    pub fn set_filter(
        &mut self,
        col: usize,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.apply_filter(col, value)?;
        self.load_view(0)
    }

    pub fn from_file(path: &str, rows_per_view: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let options = ArrowReaderOptions::new().with_page_index_policy(PageIndexPolicy::Optional);
        let metadata = ArrowReaderMetadata::load(&File::open(path)?, options)?;
        let schema = metadata.schema().clone();

        // Precompute the starting row index of each row group for later use in determining which
        // row groups to read for a given batch of rows.
        let row_group_starts = metadata
            .metadata()
            .row_groups()
            .iter()
            .scan(0, |acc, rg| {
                let start = *acc;
                *acc += rg.num_rows() as usize;
                Some(start)
            })
            .collect();

        let header = schema
            .fields()
            .iter()
            .map(|field| field.name().clone())
            .collect();

        let column_types = metadata
            .schema()
            .fields
            .iter()
            .map(|f| f.data_type().to_string())
            .collect();

        let mut preview = ParquetPreview {
            path: path.to_string(),
            rows_per_view,
            metadata,
            row_group_starts,
            chunk: RecordBatch::new_empty(schema.clone()),
            chunk_idx: None,
            header,
            column_types,
            rows: Vec::new(),
            view: RecordBatch::new_empty(schema),
            global_order: None,
            current_view: 0,
            sort_state: None,
        };

        preview.load_view(0)?;

        Ok(preview)
    }
}

fn format_rows(batch: &RecordBatch) -> Result<Vec<Vec<String>>, ArrowError> {
    let format_options = FormatOptions::default();

    let formatters: Vec<_> = batch
        .columns()
        .iter()
        .map(|column| ArrayFormatter::try_new(column, &format_options))
        .collect::<Result<_, _>>()?;

    let rows = (0..batch.num_rows())
        .map(|row_index| {
            formatters
                .iter()
                .map(|formatter| formatter.value(row_index).to_string())
                .collect()
        })
        .collect();

    Ok(rows)
}
