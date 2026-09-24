pub mod reader;
use reader::ParquetPreview;

// ratatui table widgets, eventually

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let preview = ParquetPreview::from_file("./data/sample-users.parquet", 1)?;

    println!("{:?}", preview.header());

    for row in preview.rows() {
        println!("{:?}", row);
    }

    Ok(())
}
