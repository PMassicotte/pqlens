use crate::sorter::{SortDir, SortState};
use arrow::compute::{SortOptions, concat_batches, sort_to_indices};
use arrow::error::ArrowError;
use arrow::record_batch::RecordBatch;
use arrow::util::display::{ArrayFormatter, FormatOptions};
use parquet::arrow::arrow_reader::{
    ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
};
use parquet::file::metadata::PageIndexPolicy;
use std::fs::File;

const BATCHES_PER_CHUNK: usize = 100;

pub struct ParquetPreview {
    path: String,
    batch_size: usize,
    metadata: ArrowReaderMetadata,
    row_group_starts: Vec<usize>,
    chunk: RecordBatch,
    chunk_idx: Option<usize>,
    header: Vec<String>,
    column_types: Vec<String>,
    rows: Vec<Vec<String>>,
    view: RecordBatch,
    order: Vec<usize>,
    current_batch: usize,
    sort_state: Option<SortState>,
}

impl ParquetPreview {
    pub fn header(&self) -> &[String] {
        &self.header
    }

    pub fn total_rows(&self) -> usize {
        self.metadata.metadata().file_metadata().num_rows() as usize
    }

    pub fn column_types(&self) -> &[String] {
        &self.column_types
    }

    pub fn sort_state(&self) -> Option<SortState> {
        self.sort_state
    }

    pub fn cycle_sort(&mut self, col: usize) -> Result<(), ArrowError> {
        self.sort_state = SortState::next(self.sort_state, col);
        self.apply_sort()
    }

    pub fn current_batch(&self) -> usize {
        self.current_batch
    }

    pub fn num_batches(&self) -> usize {
        self.total_rows().div_ceil(self.batch_size)
    }

    fn apply_sort(&mut self) -> Result<(), ArrowError> {
        match self.sort_state {
            Some(s) => self.sort_by(s.col(), s.dir() == SortDir::Desc),
            None => {
                self.clear_sort();
                Ok(())
            }
        }
    }

    pub fn next_view(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.current_batch + 1 < self.num_batches() {
            self.load_view(self.current_batch + 1)?;
        }

        Ok(())
    }

    pub fn previous_view(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.current_batch > 0 {
            self.load_view(self.current_batch - 1)?;
        }

        Ok(())
    }

    pub fn first_view(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.load_view(0)
    }

    pub fn last_view(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.load_view(self.num_batches().saturating_sub(1))
    }

    fn load_view(&mut self, idx: usize) -> Result<(), Box<dyn std::error::Error>> {
        let chunk_idx = idx / BATCHES_PER_CHUNK;
        let chunk_rows = BATCHES_PER_CHUNK * self.batch_size;

        if self.chunk_idx != Some(chunk_idx) {
            let start = chunk_idx * chunk_rows;
            let len = chunk_rows.min(self.total_rows().saturating_sub(start));
            self.chunk = self.read_rows(start, len)?;
            self.chunk_idx = Some(chunk_idx);
        }

        let offset = (idx % BATCHES_PER_CHUNK) * self.batch_size;

        let len = self
            .batch_size
            .min(self.chunk.num_rows().saturating_sub(offset));

        self.view = self.chunk.slice(offset, len);
        self.current_batch = idx;
        self.rows = format_rows(&self.view)?;
        self.apply_sort()?;

        Ok(())
    }

    fn sort_by(&mut self, col: usize, descending: bool) -> Result<(), ArrowError> {
        let opts = SortOptions {
            descending,
            nulls_first: false,
        };

        let idx = sort_to_indices(self.view.column(col), Some(opts), None)?;

        self.order = idx.values().iter().map(|&i| i as usize).collect();

        Ok(())
    }

    /// Clears any sorting and resets the order to the original row order.
    fn clear_sort(&mut self) {
        self.order = (0..self.rows.len()).collect();
    }

    /// Returns an iterator over the rows in the current order.
    pub fn rows(&self) -> impl Iterator<Item = &Vec<String>> {
        self.order.iter().map(|&i| &self.rows[i])
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

    pub fn from_file(path: &str, batch_size: usize) -> Result<Self, Box<dyn std::error::Error>> {
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
            batch_size,
            metadata,
            row_group_starts,
            chunk: RecordBatch::new_empty(schema.clone()),
            chunk_idx: None,
            header,
            column_types,
            rows: Vec::new(),
            view: RecordBatch::new_empty(schema),
            order: Vec::new(),
            current_batch: 0,
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
