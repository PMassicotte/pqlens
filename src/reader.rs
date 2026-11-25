use crate::filter;
use crate::geo::Geometry;
use crate::sorter::{SortDir, SortState};
use arrow::array::Array;
use arrow::array::AsArray;
use arrow::array::{ArrayRef, RecordBatch, RecordBatchReader, UInt32Array};
use arrow::compute::{SortOptions, concat_batches, sort_to_indices};
use arrow::compute::{partition, take};
use arrow::error::ArrowError;
use arrow::util::display::{ArrayFormatter, FormatOptions};
use parquet::arrow::ProjectionMask;
use parquet::arrow::arrow_reader::{
    ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
    statistics::StatisticsConverter,
};
use parquet::file::metadata::PageIndexPolicy;
use serde_json::Value;
use std::fs::File;
use std::path::{Path, PathBuf};

use arrow::compute::take_record_batch;
use parquet::arrow::arrow_reader::RowSelection;

const VIEWS_PER_CHUNK: usize = 100;

#[derive(Debug)]
struct GeoInfo {
    encoding: String,
}

pub struct ParquetPreview {
    path: PathBuf,
    rows_per_view: usize,
    metadata: ArrowReaderMetadata,
    row_group_starts: Vec<usize>,
    chunk: RecordBatch,
    chunk_idx: Option<usize>,
    header: Vec<String>,
    column_types: Vec<String>,
    geo_info: Vec<Option<GeoInfo>>,
    rows: Vec<Vec<String>>,
    view: RecordBatch,
    sorted_idx: Option<UInt32Array>,
    filtered_idx: Option<UInt32Array>,
    current_view: usize,
    sort_state: Option<SortState>,
}

impl ParquetPreview {
    pub fn header(&self) -> &[String] {
        &self.header
    }

    pub fn schema(&self) -> String {
        self.metadata
            .schema()
            .fields()
            .iter()
            .map(|field| format!("{}: {}", field.name(), field.data_type()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn total_rows(&self) -> usize {
        self.metadata.metadata().file_metadata().num_rows() as usize
    }

    pub fn column_min_max(&self, col: usize) -> Result<(String, String), color_eyre::Report> {
        if self.metadata.metadata().num_row_groups() == 0 {
            return Ok(("N/A".to_string(), "N/A".to_string()));
        }

        let sc = StatisticsConverter::try_new(
            &self.header[col],
            self.metadata.schema(),
            self.metadata.parquet_schema(),
        )?;

        let mins = sc.row_group_mins(self.metadata.metadata().row_groups())?;
        let maxes = sc.row_group_maxes(self.metadata.metadata().row_groups())?;

        let min_idx = sort_to_indices(
            &mins,
            Some(SortOptions {
                descending: false,
                nulls_first: false,
            }),
            Some(1),
        )?
        .value(0);

        let max_idx = sort_to_indices(
            &maxes,
            Some(SortOptions {
                descending: true,
                nulls_first: false,
            }),
            Some(1),
        )?
        .value(0);

        let min_formatted = ArrayFormatter::try_new(&mins, &FormatOptions::default())?
            .value(min_idx as usize)
            .to_string();

        let max_formatted = ArrayFormatter::try_new(&maxes, &FormatOptions::default())?
            .value(max_idx as usize)
            .to_string();

        Ok((min_formatted, max_formatted))
    }

    // Number of rows when a filter is applied, otherwise the same as total_rows
    pub fn visible_rows(&self) -> usize {
        match &self.filtered_idx {
            Some(order) => order.len(),
            None => self.total_rows(),
        }
    }

    pub fn is_filtered(&self) -> bool {
        self.filtered_idx.is_some()
    }

    pub fn column_types(&self) -> &[String] {
        &self.column_types
    }

    pub fn sort_state(&self) -> Option<SortState> {
        self.sort_state
    }

    pub fn cycle_sort(&mut self, col: usize) -> Result<(), color_eyre::Report> {
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

    fn apply_sort(&mut self) -> Result<(), color_eyre::Report> {
        if let Some(s) = self.sort_state {
            self.sort_by(s.col(), s.dir() == SortDir::Desc)
        } else {
            self.clear_sort();
            Ok(())
        }
    }

    pub fn next_view(&mut self) -> Result<(), color_eyre::Report> {
        if self.current_view + 1 < self.num_views() {
            self.load_view(self.current_view + 1)?;
        }

        Ok(())
    }

    pub fn previous_view(&mut self) -> Result<(), color_eyre::Report> {
        if self.current_view > 0 {
            self.load_view(self.current_view - 1)?;
        }

        Ok(())
    }

    pub fn first_view(&mut self) -> Result<(), color_eyre::Report> {
        self.load_view(0)
    }

    pub fn last_view(&mut self) -> Result<(), color_eyre::Report> {
        self.load_view(self.num_views().saturating_sub(1))
    }

    // Only called when there is no global order, so we can read a contiguous chunk of rows from the
    // parquet file.
    fn view_contiguous(&mut self, idx: usize) -> Result<RecordBatch, color_eyre::Report> {
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

    pub fn filter_by_cell(&mut self, row: usize, col: usize) -> Result<(), color_eyre::Report> {
        let selected_value = self.view.column(col).slice(row, 1);
        let col = self.read_column(col)?;

        // filter::matching_rows(&col, value)?;

        let idx = filter::matching_rows_for(&col, &selected_value)?;

        self.filtered_idx = Some(idx);

        self.apply_sort()?;

        self.load_view(0)
    }

    fn read_rows_at(&self, indices: &[u32]) -> Result<RecordBatch, color_eyre::Report> {
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

    /// Filter to keep the distinct values on the selected column
    pub fn filter_distinct(&mut self, col: usize) -> Result<(), color_eyre::Report> {
        let column = self.read_column(col)?;

        let order = sort_to_indices(&column, None, None)?;
        let sorted = take(&column, &order, None)?;
        let partitions = partition(&[sorted])?;

        let mut rows: Vec<u32> = partitions
            .ranges()
            .iter()
            .map(|r| order.value(r.start))
            .collect();

        rows.sort_unstable();

        self.filtered_idx = Some(rows.into());
        self.apply_sort()?;

        self.load_view(0)
    }

    fn load_view(&mut self, idx: usize) -> Result<(), color_eyre::Report> {
        let order = self.sorted_idx.as_ref().or(self.filtered_idx.as_ref());

        self.view = match order {
            None => self.view_contiguous(idx)?,
            Some(order) => {
                let start = idx * self.rows_per_view;
                let end = (start + self.rows_per_view).min(order.len());
                self.read_rows_at(&order.values()[start..end])?
            }
        };

        self.current_view = idx;
        self.rows = format_rows(&self.view, &self.geo_info)?;

        Ok(())
    }

    fn sort_by(&mut self, col: usize, descending: bool) -> Result<(), color_eyre::Report> {
        let column = self.read_column(col)?;

        let opts = SortOptions {
            descending,
            nulls_first: false,
        };

        let idx = sort_to_indices(&column, Some(opts), None)?;

        // Only keep indices in the filter (if one is active)
        let idx = if let Some(filter) = self.filtered_idx.as_ref() {
            idx.values()
                .iter()
                .filter(|i| filter.values().binary_search(i).is_ok())
                .copied()
                .collect()
        } else {
            idx
        };

        self.sorted_idx = Some(idx);

        Ok(())
    }

    /// Clears any sorting and resets the order to the original row order.
    pub fn clear_sort(&mut self) {
        self.sorted_idx = None;
    }

    pub fn clear_filter(&mut self) -> Result<(), color_eyre::Report> {
        self.filtered_idx = None;
        self.apply_sort()?;
        self.load_view(0)
    }

    /// Returns an iterator over the rows of the current view, already in display order.
    pub fn rows(&self) -> impl Iterator<Item = &Vec<String>> {
        self.rows.iter()
    }

    /// Reads `len` rows starting at row `start`, decoding only the row groups that contain them.
    fn read_rows(&self, start: usize, len: usize) -> Result<RecordBatch, color_eyre::Report> {
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
    pub fn read_column(&self, col: usize) -> Result<ArrayRef, color_eyre::Report> {
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

    fn apply_filter(&mut self, col: usize, value: &str) -> Result<(), color_eyre::Report> {
        let column = self.read_column(col)?;

        self.filtered_idx = Some(filter::matching_rows(&column, value)?);

        Ok(())
    }

    pub fn set_filter(&mut self, col: usize, value: &str) -> Result<(), color_eyre::Report> {
        self.apply_filter(col, value)?;
        self.apply_sort()?;
        self.load_view(0)
    }

    pub fn from_file(path: &Path, rows_per_view: usize) -> Result<Self, color_eyre::Report> {
        let options = ArrowReaderOptions::new().with_page_index_policy(PageIndexPolicy::Optional);

        // Try to load the arrow with the Arrow metadata, use the parquet metadata otherwise
        let metadata = ArrowReaderMetadata::load(&File::open(path)?, options).or_else(|_| {
            let options = ArrowReaderOptions::new()
                .with_page_index_policy(PageIndexPolicy::Optional)
                .with_skip_arrow_metadata(true);

            ArrowReaderMetadata::load(&File::open(path)?, options)
        })?;

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

        // Check for geo information in the parquet footer
        let geo_columns: Option<Value> = metadata
            .metadata()
            .file_metadata()
            .key_value_metadata()
            .and_then(|kvs| kvs.iter().find(|kv| kv.key == "geo"))
            .and_then(|kv| kv.value.as_deref())
            .and_then(|json| serde_json::from_str::<Value>(json).ok())
            .and_then(|parsed| parsed.get("columns").cloned());

        let mut geo_info: Vec<Option<GeoInfo>> = Vec::new();

        for field in schema.fields() {
            match &geo_columns {
                Some(cols) => match cols.get(field.name()) {
                    Some(b) => match b.get("encoding").and_then(|e| e.as_str()) {
                        Some(enc) => geo_info.push(Some(GeoInfo {
                            encoding: enc.to_string(),
                        })),
                        None => geo_info.push(None),
                    },
                    None => geo_info.push(None),
                },
                None => {
                    geo_info.push(None);
                }
            }
        }

        let mut preview = ParquetPreview {
            path: path.to_path_buf(),
            rows_per_view,
            metadata,
            row_group_starts,
            chunk: RecordBatch::new_empty(schema.clone()),
            chunk_idx: None,
            header,
            column_types,
            geo_info,
            rows: Vec::new(),
            view: RecordBatch::new_empty(schema),
            sorted_idx: None,
            filtered_idx: None,
            current_view: 0,
            sort_state: None,
        };

        preview.load_view(0)?;

        Ok(preview)
    }
}

// 1. Get the column index with enumerate
//
// Change formatters.iter() to formatters.iter().enumerate(). The closure then receives a pair, (col_index, formatter), instead of just formatter.
//
// 2. Inside the closure, use col_index to decide
//
// - Is geo_info[col_index] a WKB geometry? If yes:
//   - batch.column(col_index) gives the column's array,
//   - .as_binary::<i32>() turns it into a binary array (import the AsArray trait for this),
//   - if .is_null(row_index), return an empty string,
//   - otherwise .value(row_index) gives the &[u8], so call your geo.rs decoder with it.
// - Otherwise: formatter.value(row_index).to_string(), exactly as now.
//
//  Not every column is WKB. In GeoParquet 1.1, the encoding your geo_info stores can be "WKB" or a
//  native GeoArrow encoding like "point" or "polygon". Those native encodings are Arrow struct and
//  list columns, not binary. Only take the as_binary path when the  encoding is "WKB".

//
// https://geoparquet.org/releases/v1.1.0/

fn format_rows(
    batch: &RecordBatch,
    geo_info: &[Option<GeoInfo>],
) -> Result<Vec<Vec<String>>, ArrowError> {
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
                .enumerate()
                .map(|(col_index, formatter)| {
                    eprintln!("Encoding: {:?}", geo_info[col_index]);
                    if let Some(geo) = &geo_info[col_index]
                        && geo.encoding == "WKB"
                    {
                        let Some(array) = batch.column(col_index).as_binary_opt::<i32>() else {
                            return formatter.value(row_index).to_string(); // not Binary: show as-is
                        };

                        if array.is_null(row_index) {
                            return String::new();
                        }

                        let wkb: &[u8] = array.value(row_index);
                        let geom = Geometry::from_wkb(wkb);

                        match geom {
                            Geometry::Point(p) => format!("POINT({} {})", p.x, p.y),
                            Geometry::LineString(ls) => {
                                let coords: Vec<String> = ls
                                    .points
                                    .iter()
                                    .map(|p| format!("{} {}", p.x, p.y))
                                    .collect();
                                format!("LINESTRING({})", coords.join(", "))
                            }
                            Geometry::Polygon(poly) => {
                                let rings: Vec<String> = poly
                                    .rings
                                    .iter()
                                    .map(|ring| {
                                        let coords: Vec<String> = ring
                                            .iter()
                                            .map(|p| format!("{} {}", p.x, p.y))
                                            .collect();
                                        format!("({})", coords.join(", "))
                                    })
                                    .collect();
                                format!("POLYGON({})", rings.join(", "))
                            }
                            Geometry::UnimplementedType(ty) => {
                                format!("Unsupported geometry type {ty}")
                            }
                        }
                    } else {
                        formatter.value(row_index).to_string()
                    }
                })
                .collect()
        })
        .collect();

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::ParquetPreview;
    use std::path::Path;

    fn sample_preview() -> ParquetPreview {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/sample-users.parquet");
        ParquetPreview::from_file(&path, 45).unwrap()
    }

    #[test]
    fn min_max_on_sample_data_i64() {
        let preview = sample_preview();

        let min_max = preview.column_min_max(0).unwrap();

        assert_eq!(min_max, ("1".to_string(), "1000".to_string()));
    }

    #[test]
    fn min_max_on_sample_data_utf8() {
        let preview = sample_preview();

        let min_max = preview.column_min_max(4).unwrap();

        assert_eq!(min_max, ("Beijing".to_string(), "Tokyo".to_string()));
    }
}
