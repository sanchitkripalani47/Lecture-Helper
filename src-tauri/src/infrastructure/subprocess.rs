use crate::domain::ProcessingError;
use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Locate an executable on `PATH` (or accept an explicit path).
pub fn which(program: &str) -> Option<PathBuf> {
    if program.contains('/') || program.contains('\\') {
        let candidate = PathBuf::from(program);
        return candidate.is_file().then_some(candidate);
    }

    let path = std::env::var_os("PATH")?;
    which_in(program, &path, &executable_extensions())
}

/// Search `path` for `program`, trying each suffix in turn.
///
/// Exposed separately from the environment so the lookup can be tested on any
/// platform. On Windows the binary on disk is `ffmpeg.exe`, not `ffmpeg`, so a
/// literal filename check would never find it.
fn which_in(program: &str, path: &OsStr, extensions: &[String]) -> Option<PathBuf> {
    std::env::split_paths(path).find_map(|directory| {
        extensions
            .iter()
            .map(|extension| directory.join(format!("{program}{extension}")))
            .find(|candidate| candidate.is_file())
    })
}

/// Suffixes to try when looking for a program.
///
/// Windows resolves executables through `PATHEXT`, so that is honoured rather
/// than hardcoding `.exe`. Every other platform has no suffix.
#[cfg(windows)]
fn executable_extensions() -> Vec<String> {
    let mut extensions = vec![String::new()];

    if let Some(pathext) = std::env::var_os("PATHEXT") {
        for entry in pathext.to_string_lossy().split(';') {
            let entry = entry.trim();
            if !entry.is_empty() {
                extensions.push(entry.to_lowercase());
            }
        }
    }

    // A machine without PATHEXT still needs the usual suspects.
    if extensions.len() == 1 {
        extensions.extend([".exe".to_string(), ".cmd".to_string(), ".bat".to_string()]);
    }

    extensions
}

#[cfg(not(windows))]
fn executable_extensions() -> Vec<String> {
    vec![String::new()]
}

#[derive(Debug)]
pub struct CommandResult {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Run a short-lived command and capture its output.
pub fn run_capture(program: &Path, args: &[OsString]) -> Result<CommandResult, ProcessingError> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|source| spawn_failure(program, source))?;

    Ok(CommandResult {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Run a long-running command, streaming stdout line by line so callers can
/// report progress, and honouring cooperative cancellation.
///
/// stderr is drained on a separate thread so a noisy tool cannot deadlock on a
/// full pipe while we are still reading stdout.
pub fn run_streaming<F>(
    program: &Path,
    args: &[OsString],
    is_cancelled: &dyn Fn() -> bool,
    mut on_line: F,
) -> Result<CommandResult, ProcessingError>
where
    F: FnMut(&str),
{
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| spawn_failure(program, source))?;

    let stderr = child.stderr.take().expect("stderr is piped");
    let stderr_handle = std::thread::spawn(move || {
        let mut buffer = String::new();
        let mut reader = BufReader::new(stderr);
        let _ = reader.read_to_string(&mut buffer);
        buffer
    });

    let stdout = child.stdout.take().expect("stdout is piped");
    let reader = BufReader::new(stdout);
    let mut killed = false;

    for line in reader.lines() {
        if is_cancelled() {
            killed = true;
            break;
        }
        match line {
            Ok(line) => on_line(&line),
            Err(_) => break,
        }
    }

    if killed {
        let _ = child.kill();
    }

    let status = child
        .wait()
        .map_err(|source| spawn_failure(program, source))?;
    let stderr = stderr_handle.join().unwrap_or_default();

    if is_cancelled() {
        return Err(ProcessingError::Cancelled);
    }

    Ok(CommandResult {
        code: status.code(),
        stdout: String::new(),
        stderr,
    })
}

/// A tool that was located on disk but could not be run.
fn spawn_failure(program: &Path, source: io::Error) -> ProcessingError {
    ProcessingError::ExternalToolFailed {
        program: program.display().to_string(),
        code: None,
        detail: format!("could not start the program: {source}"),
    }
}

/// Turn an unexpected exit code into a user-facing error with a trimmed detail.
pub fn external_failure(program: &str, result: &CommandResult) -> ProcessingError {
    let detail = result
        .stderr
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("the tool did not report a reason")
        .trim()
        .to_string();

    ProcessingError::ExternalToolFailed {
        program: program.to_string(),
        code: result.code,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::filesystem::TempDir;
    use std::fs;

    fn no_suffix() -> Vec<String> {
        vec![String::new()]
    }

    fn with_exe() -> Vec<String> {
        vec![String::new(), ".exe".to_string()]
    }

    #[test]
    fn finds_a_bare_executable_on_the_search_path() {
        let directory = TempDir::new("which-bare").expect("temp dir");
        let binary = directory.path().join("ffmpeg");
        fs::write(&binary, b"stub").expect("write");

        let found = which_in("ffmpeg", directory.path().as_os_str(), &no_suffix());
        assert_eq!(found, Some(binary));
    }

    #[test]
    fn finds_an_executable_that_carries_a_suffix() {
        // The Windows shape: the binary on disk is ffmpeg.exe.
        let directory = TempDir::new("which-exe").expect("temp dir");
        let binary = directory.path().join("ffmpeg.exe");
        fs::write(&binary, b"stub").expect("write");

        let found = which_in("ffmpeg", directory.path().as_os_str(), &with_exe());
        assert_eq!(found, Some(binary));
    }

    #[test]
    fn prefers_the_unsuffixed_name_when_both_exist() {
        let directory = TempDir::new("which-both").expect("temp dir");
        let bare = directory.path().join("ffmpeg");
        fs::write(&bare, b"stub").expect("write");
        fs::write(directory.path().join("ffmpeg.exe"), b"stub").expect("write");

        let found = which_in("ffmpeg", directory.path().as_os_str(), &with_exe());
        assert_eq!(found, Some(bare));
    }

    #[test]
    fn searches_every_directory_on_the_path() {
        let empty = TempDir::new("which-empty").expect("temp dir");
        let populated = TempDir::new("which-populated").expect("temp dir");
        let binary = populated.path().join("ffmpeg.exe");
        fs::write(&binary, b"stub").expect("write");

        let path = std::env::join_paths([empty.path(), populated.path()]).expect("join paths");
        let found = which_in("ffmpeg", &path, &with_exe());
        assert_eq!(found, Some(binary));
    }

    #[test]
    fn returns_none_when_nothing_matches() {
        let directory = TempDir::new("which-none").expect("temp dir");
        let found = which_in(
            "definitely-not-installed",
            directory.path().as_os_str(),
            &with_exe(),
        );
        assert_eq!(found, None);
    }

    #[test]
    fn an_explicit_path_is_used_as_given() {
        let directory = TempDir::new("which-explicit").expect("temp dir");
        let binary = directory.path().join("ffmpeg");
        fs::write(&binary, b"stub").expect("write");

        assert_eq!(which(binary.to_str().expect("utf-8 path")), Some(binary));
    }

    #[test]
    fn a_missing_explicit_path_is_not_found() {
        assert_eq!(which("/nowhere/at/all/ffmpeg"), None);
    }

    #[cfg(not(windows))]
    #[test]
    fn other_platforms_look_for_the_bare_name_only() {
        assert_eq!(executable_extensions(), vec![String::new()]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_always_considers_the_usual_suffixes() {
        let extensions = executable_extensions();
        assert!(extensions.contains(&".exe".to_string()));
    }
}
