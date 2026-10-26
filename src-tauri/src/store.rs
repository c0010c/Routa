use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::model::{ListenerId, Model};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, specta::Type)]
pub struct Bytes {
    pub up: u64,
    pub down: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, specta::Type)]
pub struct TrafficTotals {
    pub total: Bytes,
    pub per_listener: HashMap<ListenerId, Bytes>,
}

pub struct Store {
    dir: PathBuf,
}

const STATE_FILE: &str = "state.json";
const TRAFFIC_FILE: &str = "traffic.json";
const CORE_DIR: &str = "mihomo";

fn invalid_data(path: &Path, e: serde_json::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}: {e}", path.display()),
    )
}

/// Missing file -> `Ok(None)`; unreadable or malformed file -> error (never a silent default).
fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| invalid_data(path, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Write `<path>.tmp`, flush + sync, then rename over the target.
fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let data = serde_json::to_vec_pretty(value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&data)?;
        f.flush()?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

impl Store {
    pub fn open(dir: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(dir.join(CORE_DIR))?;
        Ok(Self { dir })
    }

    pub fn load_model(&self) -> io::Result<Model> {
        Ok(read_json(&self.dir.join(STATE_FILE))?.unwrap_or_default())
    }

    pub fn save_model(&self, m: &Model) -> io::Result<()> {
        write_json_atomic(&self.dir.join(STATE_FILE), m)
    }

    pub fn load_totals(&self) -> io::Result<TrafficTotals> {
        Ok(read_json(&self.dir.join(TRAFFIC_FILE))?.unwrap_or_default())
    }

    pub fn save_totals(&self, t: &TrafficTotals) -> io::Result<()> {
        write_json_atomic(&self.dir.join(TRAFFIC_FILE), t)
    }

    pub fn config_path(&self) -> PathBuf {
        self.core_workdir().join("config.yaml")
    }

    pub fn core_workdir(&self) -> PathBuf {
        self.dir.join(CORE_DIR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_tmp() -> (tempfile::TempDir, Store) {
        let td = tempfile::tempdir().unwrap();
        let store = Store::open(td.path().join("data")).unwrap();
        (td, store)
    }

    #[test]
    fn open_creates_dirs_and_paths() {
        let (td, s) = open_tmp();
        assert!(td.path().join("data").is_dir());
        assert!(td.path().join("data").join("mihomo").is_dir());
        assert_eq!(s.core_workdir(), td.path().join("data").join("mihomo"));
        assert_eq!(
            s.config_path(),
            td.path().join("data").join("mihomo").join("config.yaml")
        );
    }

    #[test]
    fn load_missing_returns_default() {
        let (_td, s) = open_tmp();
        assert_eq!(s.load_model().unwrap(), Model::default());
        assert_eq!(s.load_totals().unwrap(), TrafficTotals::default());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let (_td, s) = open_tmp();
        let mut m = Model::default();
        m.settings.controller_port = 9123;
        s.save_model(&m).unwrap();
        assert_eq!(s.load_model().unwrap(), m);

        let mut t = TrafficTotals::default();
        t.total = Bytes { up: 1, down: 2 };
        t.per_listener
            .insert(ListenerId("l-1".into()), Bytes { up: 3, down: 4 });
        s.save_totals(&t).unwrap();
        assert_eq!(s.load_totals().unwrap(), t);
    }

    #[test]
    fn listener_id_map_key_is_plain_string() {
        let mut t = TrafficTotals::default();
        t.per_listener
            .insert(ListenerId("abc".into()), Bytes { up: 1, down: 2 });
        let v: serde_json::Value = serde_json::to_value(&t).unwrap();
        assert_eq!(v["per_listener"]["abc"]["down"], 2);
    }

    #[test]
    fn save_is_atomic_leaves_no_tmp() {
        let (td, s) = open_tmp();
        s.save_model(&Model::default()).unwrap();
        s.save_model(&Model::default()).unwrap();
        s.save_totals(&TrafficTotals::default()).unwrap();
        let names: Vec<String> = fs::read_dir(td.path().join("data"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"state.json".to_string()));
        assert!(names.contains(&"traffic.json".to_string()));
        assert!(!names.iter().any(|n| n.ends_with(".tmp")), "{names:?}");
    }

    #[test]
    fn corrupt_state_json_is_error_not_default() {
        let (td, s) = open_tmp();
        fs::write(td.path().join("data").join("state.json"), "{ not json").unwrap();
        let err = s.load_model().unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        // the corrupt file must be left untouched
        assert_eq!(
            fs::read_to_string(td.path().join("data").join("state.json")).unwrap(),
            "{ not json"
        );
    }

    #[test]
    fn corrupt_traffic_json_is_error_not_default() {
        let (td, s) = open_tmp();
        fs::write(td.path().join("data").join("traffic.json"), "").unwrap();
        let err = s.load_totals().unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
