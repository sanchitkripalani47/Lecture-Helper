use crate::domain::ProcessingError;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch directory removed when it goes out of scope, so interrupted runs
/// do not leave extracted frames behind.
#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> io::Result<Self> {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), unique));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// List a directory's files, sorted by name, for deterministic frame ordering.
pub fn sorted_files(directory: &Path) -> Result<Vec<PathBuf>, ProcessingError> {
    let entries = fs::read_dir(directory).map_err(|source| ProcessingError::InputUnreadable {
        path: directory.to_path_buf(),
        source,
    })?;

    let mut files: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| ProcessingError::InputUnreadable {
            path: directory.to_path_buf(),
            source,
        })?;
        if entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_dir_is_removed_on_drop() {
        let path = {
            let dir = TempDir::new("lecture-helper-test").expect("temp dir");
            fs::write(dir.path().join("frame.jpg"), b"x").expect("write");
            dir.path().to_path_buf()
        };
        assert!(!path.exists());
    }

    #[test]
    fn sorted_files_reads_in_order() {
        let dir = TempDir::new("lecture-helper-sort").expect("temp dir");
        fs::write(dir.path().join("frame_000002.jpg"), b"b").expect("write");
        fs::write(dir.path().join("frame_000001.jpg"), b"a").expect("write");
        let files = sorted_files(dir.path()).expect("list");
        assert!(files[0].ends_with("frame_000001.jpg"));
        assert!(files[1].ends_with("frame_000002.jpg"));
    }
}
