pub mod parquet_writer;
pub mod state;
pub mod vm_export;

use std::path::Path;
use std::path::PathBuf;

use chrono::Duration;
use chrono::NaiveDate;
use chrono::Utc;
use thiserror::Error;

/// Lower bound for first-run earliest-day auto-detection. Renogy hardware predates
/// this by nowhere near, so it bounds the binary search cheaply.
const SEARCH_FROM: (i32, u32, u32) = (2015, 1, 1);

#[derive(Debug, Error)]
pub enum ArchiverError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("arrow: {0}")]
    Arrow(#[from] arrow::error::ArrowError),
    #[error("parquet: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),
}

pub struct ExportConfig {
    pub vm_addr: String,
    pub staging_dir: PathBuf,
    pub state_file: PathBuf,
    pub start_date: Option<NaiveDate>,
    pub max_days: Option<usize>,
}

fn file_name(day: NaiveDate) -> String {
    format!("renogy_{}.parquet", day.format("%Y-%m-%d"))
}

/// Parse a staging/archive filename back to its day, or `None` if it doesn't match.
pub fn parse_day_from_file(name: &str) -> Option<NaiveDate> {
    let stem = name.strip_prefix("renogy_")?.strip_suffix(".parquet")?;
    NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok()
}

/// Export every unarchived day from `last_exported_day + 1` (or the first-run start)
/// through `today - 1`. Read-only against VM; never deletes staged files. State
/// advances only after a day is fully and successfully handled, so a failed VM read
/// aborts without skipping the day.
pub async fn run_export(cfg: &ExportConfig) -> Result<(), ArchiverError> {
    std::fs::create_dir_all(&cfg.staging_dir)?;
    let client = reqwest::Client::new();
    let today = Utc::now().date_naive();
    let last_full = today - Duration::days(1);

    let mut state = state::State::load(&cfg.state_file)?;

    // Readiness guard against silent data loss: if we have exported before but VM now
    // reports no renogy series at all, it is mid-restart/replay or pointed at the wrong
    // target. A successful-but-empty `/api/v1/export` for a day is indistinguishable
    // from a real empty day and would advance state past it permanently, so bail
    // without advancing and let the next run retry once VM is serving data again.
    if state.last_exported_day.is_some()
        && !vm_export::series_exists_through(&client, &cfg.vm_addr, today).await?
    {
        tracing::warn!("VM reports no renogy series; skipping export run to avoid skipping days");
        return Ok(());
    }

    let mut day = match state.last_exported_day {
        Some(d) => d + Duration::days(1),
        None => match cfg.start_date {
            Some(sd) => sd,
            None => {
                let from = NaiveDate::from_ymd_opt(SEARCH_FROM.0, SEARCH_FROM.1, SEARCH_FROM.2)
                    .expect("valid search-from date");
                match vm_export::earliest_day(&client, &cfg.vm_addr, from, today).await? {
                    Some(d) => {
                        tracing::info!("First run: earliest data in VM is {}", d);
                        d
                    }
                    None => {
                        tracing::info!("No renogy data in VM yet; nothing to export");
                        return Ok(());
                    }
                }
            }
        },
    };

    let mut written = 0usize;
    while day <= last_full {
        if let Some(max) = cfg.max_days
            && written >= max
        {
            tracing::info!("Reached --max-days {max}; stopping before {day}");
            break;
        }

        let path = cfg.staging_dir.join(file_name(day));
        if path.exists() {
            tracing::debug!("{} already staged; advancing state", path.display());
        } else {
            // A VM error here propagates and leaves state untouched: the day is retried
            // next run and never silently skipped.
            let rows = vm_export::export_day(&client, &cfg.vm_addr, day).await?;
            if rows.is_empty() {
                tracing::info!("{day}: no samples (empty day)");
            } else {
                let tmp = cfg.staging_dir.join(format!(".{}.tmp", file_name(day)));
                parquet_writer::write_parquet(&tmp, &rows)?;
                std::fs::File::open(&tmp)?.sync_all()?;
                std::fs::rename(&tmp, &path)?;
                tracing::info!("{day}: wrote {} rows -> {}", rows.len(), path.display());
                written += 1;
            }
        }

        state.last_exported_day = Some(day);
        state.save(&cfg.state_file)?;
        day += Duration::days(1);
    }

    Ok(())
}

/// Print export progress: high-water mark and the staged-but-not-yet-pulled backlog.
/// Archive completeness/gap auditing lives on the archive host (puller `status`).
pub fn run_status(staging_dir: &Path, state_file: &Path) -> Result<(), ArchiverError> {
    let state = state::State::load(state_file)?;
    match state.last_exported_day {
        Some(d) => println!("last_exported_day: {d}"),
        None => println!("last_exported_day: (none - nothing exported yet)"),
    }

    let mut staged: Vec<NaiveDate> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(staging_dir) {
        for entry in entries.flatten() {
            if let Some(d) = parse_day_from_file(&entry.file_name().to_string_lossy()) {
                staged.push(d);
            }
        }
    }
    staged.sort();

    match (staged.first(), staged.last()) {
        (Some(first), Some(last)) => println!(
            "staged (un-pulled) files: {} ({first} .. {last})",
            staged.len()
        ),
        _ => println!("staged (un-pulled) files: 0"),
    }
    Ok(())
}
