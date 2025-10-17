use crate::log::log;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

const PLACEHOLDER_FILE_NAME: &str = "TEST.txt";
const PLACEHOLDER_HEADER: &str = "MPQ archive preview is not implemented yet.";

#[derive(Debug, Clone)]
pub struct MpqEntry {
    pub path: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub data: Arc<[u8]>,
}

impl MpqEntry {
    pub fn from_bytes(path: String, bytes: Vec<u8>) -> Self {
        let len = bytes.len() as u64;
        Self { path, uncompressed_size: len, compressed_size: len, data: Arc::from(bytes.into_boxed_slice()) }
    }

    pub fn from_text(path: impl Into<String>, text: String) -> Self {
        Self::from_bytes(path.into(), text.into_bytes())
    }
}

#[derive(Debug, Clone)]
pub struct MpqArchiveDescriptor {
    pub entries: Arc<[MpqEntry]>,
}

impl MpqArchiveDescriptor {
    pub fn new(entries: Vec<MpqEntry>) -> Self {
        Self { entries: Arc::from(entries.into_boxed_slice()) }
    }

    pub fn placeholder(message: impl Into<String>) -> Self {
        let body = format!("{header}\r\n{details}\r\n", header = PLACEHOLDER_HEADER, details = message.into());
        Self::new(vec![MpqEntry::from_text(PLACEHOLDER_FILE_NAME, body)])
    }

    pub fn placeholder_from_path(path: &str) -> Self {
        Self::placeholder(format!("Source archive path: {path}"))
    }

    pub fn placeholder_from_stream(len: usize) -> Self {
        Self::placeholder(format!("Source archive provided via stream ({} bytes).", len))
    }

    pub fn load_from_path(path: &str) -> Result<Self, MpqArchiveError> {
        log(format!("MpqArchiveDescriptor::load_from_path (placeholder) path={}", path));
        Ok(Self::placeholder_from_path(path))
    }

    pub fn load_from_bytes(bytes: Arc<[u8]>) -> Result<Self, MpqArchiveError> {
        log(format!("MpqArchiveDescriptor::load_from_bytes (placeholder) size={}", bytes.len()));
        Ok(Self::placeholder_from_stream(bytes.len()))
    }

    pub fn entries(&self) -> &[MpqEntry] {
        &self.entries
    }

    pub fn total_uncompressed_size(&self) -> u64 {
        self.entries.iter().map(|e| e.uncompressed_size).sum()
    }

    pub fn find_entry(&self, name: &str) -> Option<&MpqEntry> {
        self.entries
            .iter()
            .find(|entry| entry.path.eq_ignore_ascii_case(name))
    }
}

/// Errors encountered while preparing MPQ metadata for the shell provider.
#[derive(Debug)]
pub enum MpqArchiveError {
    Io(std::io::Error),
    Unsupported(&'static str),
    Corrupted(String),
}

impl Display for MpqArchiveError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MpqArchiveError::Io(err) => write!(f, "I/O error: {}", err),
            MpqArchiveError::Unsupported(reason) => write!(f, "Unsupported archive: {}", reason),
            MpqArchiveError::Corrupted(detail) => write!(f, "Corrupted archive: {}", detail),
        }
    }
}

impl std::error::Error for MpqArchiveError {}

impl From<std::io::Error> for MpqArchiveError {
    fn from(err: std::io::Error) -> Self {
        MpqArchiveError::Io(err)
    }
}
// No external MPQ backend is used at this stage.

