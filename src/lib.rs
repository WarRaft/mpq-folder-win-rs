// MPQ Archive Viewer with WinFsp
// Core modules
pub mod archive;
pub mod log;
pub mod utils;

// WinFsp filesystem implementation
#[cfg(windows)]
pub mod mpq_filesystem;

// Configuration constants
/// ProgID bound to `.mpq` family (HKCR\WarRaft.MPQArchive; HKCR\.mpq -> WarRaft.MPQArchive).
pub const DEFAULT_PROGID: &str = "WarRaft.MPQArchive";

/// File extensions supported by the handler.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[".mpq", ".w3m", ".w3x"];

/// Human-friendly application name
pub const APP_NAME: &str = "MPQ Archive Viewer";
