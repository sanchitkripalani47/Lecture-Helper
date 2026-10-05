use crate::discovery::paths::{is_supported_video, map_output_path, relative_path};
use crate::domain::{
    models::{BatchOptions, ScanResult, VideoJob},
    ProcessingError,
};
use std::fs;
use std::path::{Path, PathBuf};

/// Collect supported videos, in a deterministic order.
///
/// The selection may be a single video file or a folder. A folder is walked
/// recursively; directory symlinks are not followed so a cyclic link cannot
/// trap the scan. Extension matching is case-insensitive, and unsupported
/// files are ignored.
pub fn scan_videos(input_root: &Path) -> Result<Vec<PathBuf>, ProcessingError> {
    let metadata =
        fs::symlink_metadata(input_root).map_err(|source| ProcessingError::InputUnreadable {
            path: input_root.to_path_buf(),
            source,
        })?;

    if metadata.is_file() {
        if !is_supported_video(input_root) {
            return Err(ProcessingError::UnsupportedInput {
                path: input_root.to_path_buf(),
            });
        }
        return Ok(vec![input_root.to_path_buf()]);
    }

    if !metadata.is_dir() {
        return Err(ProcessingError::NotADirectory {
            path: input_root.to_path_buf(),
        });
    }

    let mut found: Vec<PathBuf> = Vec::new();
    let mut pending: Vec<PathBuf> = vec![input_root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        let entries =
            fs::read_dir(&directory).map_err(|source| ProcessingError::InputUnreadable {
                path: directory.clone(),
                source,
            })?;

        for entry in entries {
            let entry = entry.map_err(|source| ProcessingError::InputUnreadable {
                path: directory.clone(),
                source,
            })?;
            let file_type =
                entry
                    .file_type()
                    .map_err(|source| ProcessingError::InputUnreadable {
                        path: entry.path(),
                        source,
                    })?;

            if file_type.is_symlink() {
                continue;
            }

            let path = entry.path();
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file() && is_supported_video(&path) {
                found.push(path);
            }
        }
    }

    // Cache the keys so the relative-path string is built once per file rather
    // than once per comparison.
    found.sort_by_cached_key(|path| sort_key(input_root, path));
    Ok(found)
}

fn sort_key(input_root: &Path, path: &Path) -> String {
    relative_path(input_root, path)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

/// Turn a discovery pass into queue-ready jobs with resolved output paths.
pub fn build_jobs(options: &BatchOptions) -> Result<Vec<VideoJob>, ProcessingError> {
    let videos = scan_videos(&options.input_root)?;
    let mut jobs = Vec::with_capacity(videos.len());

    for source in videos {
        let relative = relative_path(&options.input_root, &source)?;
        let output = map_output_path(&options.input_root, &options.output_root, &source)?;
        let output_exists = output.is_file();
        jobs.push(VideoJob::new(source, relative, output, output_exists));
    }

    Ok(jobs)
}

/// Discovery entry point used by the `discover_jobs` command.
pub fn discover(options: &BatchOptions) -> Result<ScanResult, ProcessingError> {
    let jobs = build_jobs(options)?;
    if jobs.is_empty() {
        return Ok(ScanResult::Empty {
            input_root: options.input_root.clone(),
        });
    }
    Ok(ScanResult::Ready { jobs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::ExistingOutputPolicy;
    use std::fs::File;

    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "lecture-helper-{}-{}",
                name,
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("create temp root");
            Self { root }
        }

        fn file(&self, relative: &str) -> PathBuf {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            File::create(&path).expect("create file");
            path
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn options(input: &Path, output: &Path) -> BatchOptions {
        BatchOptions {
            input_root: input.to_path_buf(),
            output_root: output.to_path_buf(),
            existing_output_policy: ExistingOutputPolicy::Skip,
        }
    }

    #[test]
    fn scans_recursively_and_ignores_unsupported_files() {
        let tree = TempTree::new("scan");
        tree.file("Course B/Lecture 01.mp4");
        tree.file("Course B/Lecture 02.mov");
        tree.file("Course A/Lecture 01.mkv");
        tree.file("Course A/notes.txt");
        tree.file("README.md");

        let found = scan_videos(&tree.root).expect("scan");
        let names: Vec<String> = found
            .iter()
            .map(|path| {
                path.strip_prefix(&tree.root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();

        assert_eq!(
            names,
            vec![
                "Course A/Lecture 01.mkv",
                "Course B/Lecture 01.mp4",
                "Course B/Lecture 02.mov",
            ]
        );
    }

    #[test]
    fn returns_empty_when_no_videos_exist() {
        let tree = TempTree::new("empty");
        tree.file("notes.txt");
        let result = discover(&options(&tree.root, &tree.root.join("out"))).expect("discover");
        assert!(matches!(result, ScanResult::Empty { .. }));
    }

    #[test]
    fn a_single_selected_video_is_the_whole_queue() {
        let tree = TempTree::new("single");
        let video = tree.file("My Videos/Course B/Lecture 01.mp4");
        // A sibling that must not be pulled in when one file was selected.
        tree.file("My Videos/Course B/Lecture 02.mp4");

        let output = tree.root.join("Processed");
        let jobs = build_jobs(&options(&video, &output)).expect("build jobs");

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].source_path, video);
        assert_eq!(jobs[0].relative_path, PathBuf::from("Lecture 01.mp4"));
        assert_eq!(jobs[0].output_path, output.join("Lecture 01.pdf"));
        assert_eq!(jobs[0].id, "Lecture 01.mp4");
    }

    #[test]
    fn a_selected_file_that_is_not_a_video_is_rejected() {
        let tree = TempTree::new("unsupported");
        let file = tree.file("notes.txt");
        assert!(matches!(
            scan_videos(&file),
            Err(ProcessingError::UnsupportedInput { .. })
        ));
    }

    #[test]
    fn builds_jobs_with_mirrored_output_paths() {
        let tree = TempTree::new("jobs");
        tree.file("Course B/Lecture 01.mp4");
        tree.file("Course B/Lecture 02.mp4");
        let output = tree.root.join("Processed");

        let jobs = build_jobs(&options(&tree.root, &output)).expect("build jobs");
        assert_eq!(jobs.len(), 2);
        assert_eq!(
            jobs[0].relative_path,
            PathBuf::from("Course B/Lecture 01.mp4")
        );
        assert_eq!(
            jobs[0].output_path,
            output.join("Course B").join("Lecture 01.pdf")
        );
        assert!(!jobs[0].output_exists);
    }

    #[test]
    fn flags_existing_output() {
        let tree = TempTree::new("existing");
        tree.file("Course B/Lecture 01.mp4");
        let output = tree.root.join("Processed");
        tree.file("Processed/Course B/Lecture 01.pdf");

        let jobs = build_jobs(&options(&tree.root, &output)).expect("build jobs");
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].output_exists);
    }
}
