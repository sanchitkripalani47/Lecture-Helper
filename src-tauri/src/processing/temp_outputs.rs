use crate::domain::ProcessingError;
use std::fs;
use std::path::{Path, PathBuf};

/// A file written next to its final destination and only renamed into place on
/// success. If the guard is dropped without committing (error or cancellation)
/// the partial file is removed, so a failed run never leaves a broken PDF.
#[derive(Debug)]
pub struct TempOutput {
    final_path: PathBuf,
    temp_path: PathBuf,
    committed: bool,
}

impl TempOutput {
    pub fn new(final_path: &Path) -> Result<Self, ProcessingError> {
        let temp_path = temp_path_for(final_path);
        if let Some(parent) = temp_path.parent() {
            fs::create_dir_all(parent).map_err(|source| ProcessingError::OutputWrite {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        // Start from a clean slate so a stale partial file cannot be mistaken
        // for this run's output.
        let _ = fs::remove_file(&temp_path);
        Ok(Self {
            final_path: final_path.to_path_buf(),
            temp_path,
            committed: false,
        })
    }

    pub fn temp_path(&self) -> &Path {
        &self.temp_path
    }

    pub fn final_path(&self) -> &Path {
        &self.final_path
    }

    /// Atomically move the temporary file onto the destination.
    pub fn commit(mut self) -> Result<PathBuf, ProcessingError> {
        fs::rename(&self.temp_path, &self.final_path).map_err(|source| {
            ProcessingError::OutputWrite {
                path: self.final_path.clone(),
                source,
            }
        })?;
        self.committed = true;
        Ok(self.final_path.clone())
    }
}

impl Drop for TempOutput {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.temp_path);
        }
    }
}

/// `<final>.partial`, e.g. `Lecture 01.pdf.partial`.
pub fn temp_path_for(final_path: &Path) -> PathBuf {
    let mut name = final_path
        .file_name()
        .map(|value| value.to_os_string())
        .unwrap_or_default();
    name.push(".partial");
    final_path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lecture-helper-temp-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    #[test]
    fn names_the_partial_file_beside_the_output() {
        let path = temp_path_for(Path::new("/out/Course A/Lecture 01.pdf"));
        assert_eq!(path, PathBuf::from("/out/Course A/Lecture 01.pdf.partial"));
    }

    #[test]
    fn commit_renames_into_place() {
        let dir = scratch("commit");
        let final_path = dir.join("Lecture 01.pdf");
        let output = TempOutput::new(&final_path).expect("temp");
        fs::write(output.temp_path(), b"%PDF-1.4").expect("write");
        output.commit().expect("commit");
        assert!(final_path.is_file());
        assert!(!temp_path_for(&final_path).exists());
    }

    #[test]
    fn dropping_without_commit_cleans_up() {
        let dir = scratch("drop");
        let final_path = dir.join("Lecture 01.pdf");
        let temp = {
            let output = TempOutput::new(&final_path).expect("temp");
            let mut file = fs::File::create(output.temp_path()).expect("create");
            file.write_all(b"partial").expect("write");
            output.temp_path().to_path_buf()
        };
        assert!(!temp.exists(), "partial file should be removed on drop");
        assert!(!final_path.exists());
    }

    #[test]
    fn existing_output_is_untouched_when_commit_never_happens() {
        let dir = scratch("preserve");
        let final_path = dir.join("Lecture 01.pdf");
        fs::write(&final_path, b"original").expect("seed");
        {
            let _output = TempOutput::new(&final_path).expect("temp");
        }
        assert_eq!(fs::read(&final_path).expect("read"), b"original");
    }
}
