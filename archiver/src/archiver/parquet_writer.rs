use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::ArrayRef;
use arrow::array::Float64Array;
use arrow::array::StringArray;
use arrow::array::TimestampMillisecondArray;
use arrow::datatypes::DataType;
use arrow::datatypes::Field;
use arrow::datatypes::Schema;
use arrow::datatypes::TimeUnit;
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use crate::archiver::ArchiverError;
use crate::archiver::vm_export::Row;

/// Long/narrow schema: one row per sample. `timestamp` is a real Parquet logical
/// timestamp (ms, UTC) so pandas/DuckDB read it without conversion.
fn schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new(
            "timestamp",
            DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())),
            false,
        ),
        Field::new("metric", DataType::Utf8, false),
        Field::new("value", DataType::Float64, false),
        Field::new("labels", DataType::Utf8, false),
    ]))
}

pub fn write_parquet(path: &Path, rows: &[Row]) -> Result<(), ArchiverError> {
    let schema = schema();

    let ts: ArrayRef = Arc::new(
        TimestampMillisecondArray::from(rows.iter().map(|r| r.ts_ms).collect::<Vec<_>>())
            .with_timezone("UTC"),
    );
    let metric: ArrayRef = Arc::new(StringArray::from(
        rows.iter().map(|r| r.metric.as_str()).collect::<Vec<_>>(),
    ));
    let value: ArrayRef = Arc::new(Float64Array::from(
        rows.iter().map(|r| r.value).collect::<Vec<_>>(),
    ));
    let labels: ArrayRef = Arc::new(StringArray::from(
        rows.iter().map(|r| r.labels.as_str()).collect::<Vec<_>>(),
    ));

    let batch = RecordBatch::try_new(schema.clone(), vec![ts, metric, value, labels])?;

    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();
    let file = File::create(path)?;
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}
