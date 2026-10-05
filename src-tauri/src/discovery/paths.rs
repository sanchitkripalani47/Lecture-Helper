use crate::domain::ProcessingError;
use std::path::{Component, Path, PathBuf};

pub const SUPPORTED_VIDEO_EXTENSIONS: [&str; 5] = ["mp4", "mov", "mkv", "avi", "webm"];

/// Case-insensitive check against the supported container formats.
pub fn is_supported_video(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => {
            let lowered = ext.to_ascii_lowercase();
            SUPPORTED_VIDEO_EXTENSIONS.contains(&lowered.as_str())
        }
        None => false,
    }
}

/// Strip the extension and append `.pdf`.
///
/// Uses the path's own file stem so names containing spaces, parentheses,
/// Unicode, or multiple dots are preserved exactly.
pub fn pdf_path_for(source: &Path) -> PathBuf {
    source.with_extension("pdf")
}

/// The path of `source` relative to `input_root`.
pub fn relative_path(input_root: &Path, source: &Path) -> Result<PathBuf, ProcessingError> {
    // When a single video was selected it *is* the root, so it is known by its
    // own file name rather than by a path relative to a folder.
    if source == input_root {
        return source
            .file_name()
            .map(PathBuf::from)
            .ok_or_else(|| ProcessingError::UnsafeOutputPath {
                path: source.to_path_buf(),
                reason: "the selected video has no file name".to_string(),
            });
    }

    source
        .strip_prefix(input_root)
        .map(|value| value.to_path_buf())
        .map_err(|_| ProcessingError::UnsafeOutputPath {
            path: source.to_path_buf(),
            reason: "the file is not inside the selected input folder".to_string(),
        })
}

/// Default output root: a sibling of the input folder named `<name> - Processed`.
pub fn default_output_root(input_root: &Path) -> PathBuf {
    let name = input_root
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "Processed".to_string());
    let parent = input_root.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{} - Processed", name))
}

/// Mirror `source` under `output_root`, preserving relative directories and
/// replacing only the extension.
///
/// The output root may be a sibling folder (the default) or the lecture folder
/// itself, in which case each PDF lands beside its video. Only a destination
/// that would overwrite the source video is refused.
pub fn map_output_path(
    input_root: &Path,
    output_root: &Path,
    source: &Path,
) -> Result<PathBuf, ProcessingError> {
    let relative = relative_path(input_root, source)?;
    let mirrored = output_root.join(relative);
    let output = pdf_path_for(&mirrored);

    if output == source {
        return Err(ProcessingError::UnsafeOutputPath {
            path: output,
            reason: "the destination would overwrite the source video".to_string(),
        });
    }

    if !has_normal_components(output_root) {
        return Err(ProcessingError::UnsafeOutputPath {
            path: output,
            reason: "the output folder contains an unsupported path segment".to_string(),
        });
    }

    Ok(output)
}

fn has_normal_components(path: &Path) -> bool {
    path.components()
        .all(|component| !matches!(component, Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_required_extensions_case_insensitively() {
        for name in [
            "a.mp4", "a.MP4", "a.mov", "a.MOV", "a.mkv", "a.avi", "a.webm", "a.WebM",
        ] {
            assert!(
                is_supported_video(Path::new(name)),
                "{name} should be supported"
            );
        }
    }

    #[test]
    fn rejects_unsupported_files() {
        for name in ["a.txt", "a.pdf", "a.mp3", "a", "a.mp4.bak"] {
            assert!(
                !is_supported_video(Path::new(name)),
                "{name} should be ignored"
            );
        }
    }

    #[test]
    fn keeps_awkward_filenames_intact() {
        let source = Path::new("/in/Course A/Lecture (01) - intro.v2.mp4");
        let mapped = pdf_path_for(source);
        assert_eq!(
            mapped,
            PathBuf::from("/in/Course A/Lecture (01) - intro.v2.pdf")
        );
    }

    #[test]
    fn mirrors_relative_directories() {
        let input = Path::new("/Users/me/My Videos");
        let output = Path::new("/Users/me/My Videos - Processed");
        let source = Path::new("/Users/me/My Videos/Course B/Lecture 01.mp4");
        let mapped = map_output_path(input, output, source).expect("maps");
        assert_eq!(
            mapped,
            PathBuf::from("/Users/me/My Videos - Processed/Course B/Lecture 01.pdf")
        );
    }

    #[test]
    fn preserves_unicode_names() {
        let input = Path::new("/in");
        let output = Path::new("/out");
        let source = Path::new("/in/课程 1/Übung 2.mkv");
        let mapped = map_output_path(input, output, source).expect("maps");
        assert_eq!(mapped, PathBuf::from("/out/课程 1/Übung 2.pdf"));
    }

    #[test]
    fn default_output_root_is_a_sibling() {
        let root = default_output_root(Path::new("/Users/me/My Videos"));
        assert_eq!(root, PathBuf::from("/Users/me/My Videos - Processed"));
    }

    #[test]
    fn allows_a_subfolder_of_the_input_folder() {
        let input = Path::new("/in");
        let output = Path::new("/in/processed");
        let source = Path::new("/in/Lecture 01.mp4");
        let mapped = map_output_path(input, output, source).expect("maps");
        assert_eq!(mapped, PathBuf::from("/in/processed/Lecture 01.pdf"));
    }

    #[test]
    fn a_single_selected_video_is_known_by_its_file_name() {
        let file = Path::new("/in/Course B/Lecture 01.mp4");
        assert_eq!(
            relative_path(file, file).expect("relative"),
            PathBuf::from("Lecture 01.mp4")
        );
    }

    #[test]
    fn a_single_video_lands_in_the_chosen_folder() {
        let file = Path::new("/in/Course B/Lecture 01.mp4");
        let output = Path::new("/out");
        let mapped = map_output_path(file, output, file).expect("maps");
        assert_eq!(mapped, PathBuf::from("/out/Lecture 01.pdf"));
    }

    #[test]
    fn a_single_video_can_land_beside_itself() {
        let file = Path::new("/in/Course B/Lecture 01.mp4");
        let folder = Path::new("/in/Course B");
        let mapped = map_output_path(file, folder, file).expect("maps");
        assert_eq!(mapped, PathBuf::from("/in/Course B/Lecture 01.pdf"));
    }

    #[test]
    fn writes_beside_the_video_when_the_output_is_the_lecture_folder() {
        let input = Path::new("/in");
        let source = Path::new("/in/Course B/Lecture 01.mp4");
        let mapped = map_output_path(input, input, source).expect("maps");
        assert_eq!(mapped, PathBuf::from("/in/Course B/Lecture 01.pdf"));
    }

    #[test]
    fn refuses_parent_directory_output() {
        let input = Path::new("/in");
        let output = Path::new("/out/../in");
        let source = Path::new("/in/Lecture 01.mp4");
        assert!(map_output_path(input, output, source).is_err());
    }
}
