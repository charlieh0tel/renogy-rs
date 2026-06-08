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
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            crate::archiver::fsync_dir(parent)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use chrono::NaiveDate;

    #[test]
    fn missing_file_is_default() {
        let path = std::env::temp_dir().join("renogy-archiver-no-such-state.json");
        let _ = std::fs::remove_file(&path);
        let state = State::load(&path).unwrap();
        assert_eq!(state.last_exported_day, None);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let day = NaiveDate::from_ymd_opt(2026, 5, 31).unwrap();
        State {
            last_exported_day: Some(day),
        }
        .save(&path)
        .unwrap();
        assert_eq!(State::load(&path).unwrap().last_exported_day, Some(day));
    }
}
