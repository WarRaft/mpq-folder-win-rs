use crate::archive::{MpqArchiveDescriptor, MpqEntry};
use crate::log::log;
use std::ffi::c_void;
use std::sync::Arc;
use winfsp::filesystem::{DirInfo, DirMarker, FileInfo, FileSecurity, FileSystemContext, OpenFileInfo, VolumeInfo};
use winfsp::{FspError, Result, U16CStr};
use windows::Win32::Foundation::STATUS_SUCCESS;
use winfsp_sys::{FILE_ACCESS_RIGHTS, FILE_FLAGS_AND_ATTRIBUTES};

/// File context representing an open file or directory in the MPQ archive
pub struct MpqFileContext {
    /// Full path relative to archive root (e.g., "folder/file.txt" or "folder/")
    path: String,
    /// True if this is a directory
    is_directory: bool,
    /// For files: reference to the entry data
    entry: Option<Arc<MpqEntry>>,
}

impl MpqFileContext {
    fn new_file(path: String, entry: Arc<MpqEntry>) -> Self {
        Self {
            path,
            is_directory: false,
            entry: Some(entry),
        }
    }

    fn new_directory(path: String) -> Self {
        Self {
            path,
            is_directory: true,
            entry: None,
        }
    }
}

/// WinFsp filesystem implementation for MPQ archives
pub struct MpqFileSystem {
    /// Archive descriptor with all entries
    descriptor: Arc<MpqArchiveDescriptor>,
    /// Source archive path for logging
    archive_path: String,
}

impl MpqFileSystem {
    pub fn new(archive_path: String) -> Result<Self> {
        log(format!("MpqFileSystem::new: loading {}", archive_path));
        
        let descriptor = MpqArchiveDescriptor::load_from_path(&archive_path)
            .map_err(|e| {
                log(format!("Failed to load MPQ: {}", e));
                FspError::from_ntstatus(0xC0000001) // STATUS_UNSUCCESSFUL
            })?;
        
        log(format!("MpqFileSystem: loaded {} entries", descriptor.entries().len()));
        
        Ok(Self {
            descriptor: Arc::new(descriptor),
            archive_path,
        })
    }

    /// Check if a path is a directory in the archive
    fn is_directory(&self, path: &str) -> bool {
        // Root is always a directory
        if path.is_empty() || path == "/" || path == "\\" {
            return true;
        }

        let normalized = self.normalize_path(path);
        
        // Check if any entry starts with this prefix
        self.descriptor.entries().iter().any(|entry| {
            let entry_path = entry.path.replace('\\', "/");
            entry_path.starts_with(&normalized) && entry_path.len() > normalized.len()
        })
    }

    /// Normalize path: convert to forward slashes, ensure trailing slash for dirs
    fn normalize_path(&self, path: &str) -> String {
        let mut normalized = path.replace('\\', "/");
        if normalized.starts_with('/') {
            normalized = normalized[1..].to_string();
        }
        if !normalized.is_empty() && !normalized.ends_with('/') {
            normalized.push('/');
        }
        normalized
    }

    /// List immediate children of a directory
    fn list_children(&self, dir_path: &str) -> Vec<(String, bool)> {
        let prefix = self.normalize_path(dir_path);
        let prefix_len = prefix.len();
        
        let mut children: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        for entry in self.descriptor.entries() {
            let entry_path = entry.path.replace('\\', "/");
            
            if !entry_path.starts_with(&prefix) {
                continue;
            }
            
            let remainder = &entry_path[prefix_len..];
            if remainder.is_empty() {
                continue;
            }
            
            // Find first component
            if let Some(slash_pos) = remainder.find('/') {
                // This is a subdirectory
                let dir_name = &remainder[..slash_pos];
                children.insert(dir_name.to_string());
            } else {
                // This is a file
                children.insert(remainder.to_string());
            }
        }
        
        // Convert to vec with is_directory flag
        children.into_iter()
            .map(|name| {
                let full_path = format!("{}{}", prefix, name);
                let is_dir = self.is_directory(&full_path);
                (name, is_dir)
            })
            .collect()
    }
}

impl FileSystemContext for MpqFileSystem {
    type FileContext = MpqFileContext;

    fn get_security_by_name(
        &self,
        file_name: &U16CStr,
        _security_descriptor: Option<&mut [c_void]>,
        _resolve_reparse_points: impl FnOnce(&U16CStr) -> Option<FileSecurity>,
    ) -> Result<FileSecurity> {
        let path = file_name.to_string_lossy();
        log(format!("get_security_by_name: {}", path));
        
        // Return basic security for read-only files
        Ok(FileSecurity::new(
            0x00000080, // FILE_ATTRIBUTE_NORMAL
            false,      // reparse point
        ))
    }

    fn open(
        &self,
        file_name: &U16CStr,
        _create_options: u32,
        _granted_access: FILE_ACCESS_RIGHTS,
        file_info: &mut OpenFileInfo,
    ) -> Result<Self::FileContext> {
        let path = file_name.to_string_lossy();
        log(format!("open: {}", path));
        
        let normalized = path.trim_start_matches('\\').replace('\\', "/");
        
        // Check if root
        if normalized.is_empty() {
            file_info.set_file_attributes(0x00000010); // FILE_ATTRIBUTE_DIRECTORY
            file_info.set_file_size(0);
            return Ok(MpqFileContext::new_directory(String::new()));
        }
        
        // Try to find as file
        if let Some(entry) = self.descriptor.find_entry(&normalized) {
            file_info.set_file_attributes(0x00000080); // FILE_ATTRIBUTE_NORMAL
            file_info.set_file_size(entry.uncompressed_size);
            return Ok(MpqFileContext::new_file(normalized, Arc::clone(&entry.data)));
        }
        
        // Check if directory
        if self.is_directory(&normalized) {
            file_info.set_file_attributes(0x00000010); // FILE_ATTRIBUTE_DIRECTORY
            file_info.set_file_size(0);
            return Ok(MpqFileContext::new_directory(normalized));
        }
        
        log(format!("open: file not found: {}", path));
        Err(FspError::from_ntstatus(0xC0000034)) // STATUS_OBJECT_NAME_NOT_FOUND
    }

    fn close(&self, _context: Self::FileContext) {
        // Nothing to clean up
    }

    fn get_file_info(&self, context: &Self::FileContext, file_info: &mut FileInfo) -> Result<()> {
        if context.is_directory {
            file_info.set_file_attributes(0x00000010); // FILE_ATTRIBUTE_DIRECTORY
            file_info.set_file_size(0);
        } else if let Some(entry) = &context.entry {
            file_info.set_file_attributes(0x00000080); // FILE_ATTRIBUTE_NORMAL
            file_info.set_file_size(entry.uncompressed_size);
        }
        Ok(())
    }

    fn read(&self, context: &Self::FileContext, buffer: &mut [u8], offset: u64) -> Result<u32> {
        if context.is_directory {
            return Err(FspError::from_ntstatus(0xC00000BA)); // STATUS_FILE_IS_A_DIRECTORY
        }
        
        let entry = context.entry.as_ref()
            .ok_or_else(|| FspError::from_ntstatus(0xC0000001))?; // STATUS_UNSUCCESSFUL
        
        let data = &entry.data;
        let start = offset as usize;
        
        if start >= data.len() {
            return Ok(0);
        }
        
        let end = (start + buffer.len()).min(data.len());
        let bytes_to_copy = end - start;
        
        buffer[..bytes_to_copy].copy_from_slice(&data[start..end]);
        
        Ok(bytes_to_copy as u32)
    }

    fn read_directory(
        &self,
        context: &Self::FileContext,
        _pattern: Option<&U16CStr>,
        marker: DirMarker,
        buffer: &mut [u8],
    ) -> Result<u32> {
        if !context.is_directory {
            return Err(FspError::from_ntstatus(0xC0000103)); // STATUS_NOT_A_DIRECTORY
        }
        
        let children = self.list_children(&context.path);
        
        // DirInfo handles the buffer filling
        let mut dir_info = DirInfo::new(buffer);
        
        let start_index = match marker {
            DirMarker::Index(idx) => idx as usize,
            _ => 0,
        };
        
        for (i, (name, is_dir)) in children.iter().enumerate().skip(start_index) {
            let name_utf16: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let name_u16cstr = U16CStr::from_slice_truncate(&name_utf16)
                .map_err(|_| FspError::from_ntstatus(0xC0000001))?;
            
            let file_attrs = if *is_dir { 0x00000010 } else { 0x00000080 };
            let file_size = if *is_dir { 0 } else {
                self.descriptor.find_entry(&format!("{}{}", context.path, name))
                    .map(|e| e.uncompressed_size)
                    .unwrap_or(0)
            };
            
            if !dir_info.write(file_attrs, file_size, 0, 0, 0, 0, name_u16cstr) {
                break;
            }
        }
        
        Ok(dir_info.bytes_written())
    }

    fn get_volume_info(&self, volume_info: &mut VolumeInfo) -> Result<()> {
        volume_info.set_total_size(self.descriptor.total_uncompressed_size());
        volume_info.set_free_size(0); // Read-only
        volume_info.set_volume_label("MPQ Archive");
        Ok(())
    }
}
