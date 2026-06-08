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

#[cfg(test)]
mod tests {
    use super::schema;
    use super::write_parquet;
    use crate::archiver::vm_export::Row;
    use arrow::array::Array;
    use arrow::array::Float64Array;
    use arrow::array::StringArray;
    use arrow::datatypes::DataType;
    use arrow::datatypes::TimeUnit;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::fs::File;

    #[test]
    fn schema_timestamp_is_utc_millis() {
        let field = schema().field(0).clone();
        assert_eq!(field.name(), "timestamp");
        assert_eq!(
            field.data_type(),
            &DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into()))
        );
    }

    #[test]
    fn write_then_read_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("day.parquet");
        let rows = vec![
            Row {
                ts_ms: 1_700_000_000_000,
                metric: "renogy_soc_percent_value".to_string(),
                value: 55.0,
                labels: r#"{"battery":"SN1"}"#.to_string(),
            },
            Row {
                ts_ms: 1_700_000_001_000,
                metric: "renogy_module_voltage_value".to_string(),
                value: 13.2,
                labels: String::new(),
            },
        ];
        write_parquet(&path, &rows).unwrap();

        let mut reader = ParquetRecordBatchReaderBuilder::try_new(File::open(&path).unwrap())
            .unwrap()
            .build()
            .unwrap();
        let batch = reader.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 2);

        let metric = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let value = batch
            .column(2)
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap();
        assert_eq!(metric.value(0), "renogy_soc_percent_value");
        assert_eq!(value.value(0), 55.0);
        assert_eq!(metric.value(1), "renogy_module_voltage_value");
    }
}
