use arrow::util::display::{ArrayFormatter, FormatOptions};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

pub struct ParquetPreview {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl ParquetPreview {
    pub fn header(&self) -> &[String] {
        &self.header
    }

    pub fn rows(&self) -> &[Vec<String>] {
        &self.rows
    }

    pub fn from_file(path: &str, batch_size: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;

        let mut preview = ParquetPreview {
            header: Vec::new(),
            rows: Vec::new(),
        };

        preview.header = builder
            .schema()
            .fields()
            .iter()
            .map(|field| field.name().clone())
            .collect();

        // Just use the first batch for now
        let reader = builder.with_batch_size(batch_size).build()?;
        for batch in reader.take(20) {
            let batch = batch?;
            preview.build_from_record_batch(&batch)?;
        }

        Ok(preview)
    }

    fn build_from_record_batch(
        &mut self,
        batch: &arrow::record_batch::RecordBatch,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let format_options = FormatOptions::default();

        let formatters: Vec<_> = batch
            .columns()
            .iter()
            .map(|column| ArrayFormatter::try_new(column, &format_options))
            .collect::<Result<_, _>>()?;

        // Extract rows
        for row_index in 0..batch.num_rows() {
            let row = formatters
                .iter()
                .map(|formatter| formatter.value(row_index).to_string())
                .collect();
            self.rows.push(row);
        }

        Ok(())
    }
}
