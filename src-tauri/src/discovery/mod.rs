pub mod paths;
pub mod scanner;

pub use paths::{
    default_output_root, is_supported_video, map_output_path, pdf_path_for, relative_path,
    SUPPORTED_VIDEO_EXTENSIONS,
};
pub use scanner::{build_jobs, discover, scan_videos};
