use crate::sorter::{SortDir, SortState};
use arrow::compute::{SortOptions, sort_to_indices};
use arrow::error::ArrowError;
use arrow::record_batch::RecordBatch;
use arrow::util::display::{ArrayFormatter, FormatOptions};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

pub struct ParquetPreview {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
    batches: Vec<RecordBatch>,
    order: Vec<usize>,
    current_batch: usize,
    sort_state: Option<SortState>,
}

impl ParquetPreview {
    pub fn header(&self) -> &[String] {
        &self.header
    }

    pub fn total_rows(&self) -> usize {
        self.batches.iter().map(|batch| batch.num_rows()).sum()
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
        self.batches.len()
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

    pub fn next_batch(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.current_batch + 1 < self.batches.len() {
            self.current_batch += 1;
            self.rows = format_rows(&self.batches[self.current_batch])?;
            self.apply_sort()?;
        }

        Ok(())
    }

    pub fn previous_batch(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.current_batch > 0 {
            self.current_batch -= 1;
            self.rows = format_rows(&self.batches[self.current_batch])?;
            self.apply_sort()?;
        }

        Ok(())
    }

    fn sort_by(&mut self, col: usize, descending: bool) -> Result<(), ArrowError> {
        let opts = SortOptions {
            descending,
            nulls_first: false,
        };

        let idx = sort_to_indices(
            self.batches[self.current_batch].column(col),
            Some(opts),
            None,
        )?;

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

    pub fn from_file(path: &str, batch_size: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        let schema = builder.schema().clone();

        let header = schema
            .fields()
            .iter()
            .map(|field| field.name().clone())
            .collect();

        // Only preview the first batches for now
        let reader = builder.with_batch_size(batch_size).build()?;
        let batches: Vec<RecordBatch> = reader.collect::<Result<_, _>>()?;
        let rows = format_rows(&batches[0])?;

        let mut preview = ParquetPreview {
            header,
            rows,
            batches,
            order: Vec::new(),
            current_batch: 0,
            sort_state: None,
        };

        preview.clear_sort();

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
