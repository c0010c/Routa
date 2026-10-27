//! Portable mode: a `portable` marker file next to the exe moves all data
//! (store, core workdir, WebView2 profile) into `<exe dir>\data`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const MARKER: &str = "portable";
const DATA_DIR: &str = "data";
const WEBVIEW_DIR: &str = "webview";

/// `Some(<exe_dir>\data)` when the marker file sits next to the exe.
pub fn data_dir_for(exe_dir: &Path) -> Option<PathBuf> {
    exe_dir.join(MARKER).is_file().then(|| exe_dir.join(DATA_DIR))
}

/// The WebView2 profile folder inside a portable data dir.
pub fn webview_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(WEBVIEW_DIR)
}

/// Creates the data dir and proves it is writable, so a read-only location
/// fails loudly at startup instead of silently losing settings later.
pub fn prepare(data_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(data_dir)?;
    let probe = data_dir.join(".write-test");
    fs::write(&probe, b"")?;
    fs::remove_file(&probe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_marker_means_not_portable() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(data_dir_for(tmp.path()), None);
    }

    #[test]
    fn marker_file_selects_data_dir_next_to_exe() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(MARKER), "").unwrap();
        assert_eq!(data_dir_for(tmp.path()), Some(tmp.path().join("data")));
    }

    #[test]
    fn marker_must_be_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join(MARKER)).unwrap();
        assert_eq!(data_dir_for(tmp.path()), None);
    }

    #[test]
    fn webview_dir_lives_inside_data_dir() {
        let data = Path::new(r"C:\app\data");
        assert_eq!(webview_dir(data), PathBuf::from(r"C:\app\data\webview"));
    }

    #[test]
    fn prepare_creates_dir_and_leaves_no_probe() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        prepare(&data).unwrap();
        assert!(data.is_dir());
        assert_eq!(fs::read_dir(&data).unwrap().count(), 0);
        prepare(&data).unwrap(); // idempotent
    }

    #[test]
    fn prepare_fails_when_dir_cannot_be_created() {
        let tmp = tempfile::tempdir().unwrap();
        let blocker = tmp.path().join("file");
        fs::write(&blocker, "").unwrap();
        assert!(prepare(&blocker.join("data")).is_err());
    }
}
