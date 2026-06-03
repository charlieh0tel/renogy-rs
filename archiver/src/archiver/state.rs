use std::path::Path;

use chrono::NaiveDate;
use serde::Deserialize;
use serde::Serialize;

use crate::archiver::ArchiverError;

/// Durable record of export progress. `last_exported_day` is the high-water mark; it
/// is the source of truth for idempotency and survives staging files being pulled and
/// deleted by the archive host.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub last_exported_day: Option<NaiveDate>,
}

impl State {
    pub fn load(path: &Path) -> Result<Self, ArchiverError> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Atomically persist via temp file + fsync + rename so a crash never leaves a
    /// truncated state file.
    pub fn save(&self, path: &Path) -> Result<(), ArchiverError> {
        let tmp = path.with_extension("json.tmp");
        let data = serde_json::to_vec_pretty(self)?;
        std::fs::write(&tmp, &data)?;
        std::fs::File::open(&tmp)?.sync_all()?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}
