use arrow::util::pretty::pretty_format_batches;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::open(
        "/media/work/others/axel_vernay/pft_phyto_arctic/data/clean/pft/parquet/20030402.parquet",
    )?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    let schema = builder.schema();

    println!("Schema: {:?}", schema);

    let reader = builder.with_batch_size(1).build()?;

    // for (i, batch) in reader.enumerate() {
    //     let batch = batch.unwrap();
    //     println!("Batch {}: ", i);
    // }

    for batch in reader.take(1) {
        let batch = batch?;
        println!("{}", pretty_format_batches(&[batch])?);
    }

    Ok(())
}
