pub mod archive;
mod class_factory;
mod dll_export;
pub mod log;
mod mpq_shell_provider;
pub mod utils;

#[cfg(not(target_pointer_width = "64"))]
compile_error!("mpq-folder-win must be built for 64-bit targets");

use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use windows::core::HRESULT;

const CLASS_E_CLASSNOTAVAILABLE: HRESULT = HRESULT(0x80040111u32 as i32);

static DLL_LOCK_COUNT: AtomicU32 = AtomicU32::new(0);

struct ProviderState {
    path_utf8: Option<String>,
    stream_data: Option<Arc<[u8]>>,
    archive: Arc<archive::MpqArchiveDescriptor>,
}

impl Default for ProviderState {
    fn default() -> Self {
        Self { path_utf8: None, stream_data: None, archive: Arc::new(archive::MpqArchiveDescriptor::placeholder("MPQ handler not initialized")) }
    }
}

/// Common constants for MPQ Folder handler registration.
/// Shared between the COM DLL and the installer. No duplicated string CLSIDs.
use windows::core::GUID;

/// Shell Thumbnail Provider category (Implemented Categories + ShellEx binding).
/// - HKCR\CLSID\{CLSID}\Implemented Categories\{SHELL_THUMB_HANDLER_CATID}
/// - HKCR\<.ext | ProgID>\ShellEx\{SHELL_THUMB_HANDLER_CATID} = {CLSID}
pub const SHELL_THUMB_HANDLER_CATID: GUID = GUID::from_u128(0xE357FCCD_A995_4576_B01F_234630154E96);

/// Shell Preview Handler category.
/// - HKCR\\CLSID\\{CLSID}\\Implemented Categories\\{SHELL_PREVIEW_HANDLER_CATID}
/// - HKCR\\<.ext | ProgID>\\ShellEx\\{SHELL_PREVIEW_HANDLER_CATID} = {CLSID}
pub const SHELL_PREVIEW_HANDLER_CATID: GUID = GUID::from_u128(0x8895B1C6_B41F_4C1C_A562_0D564250836F);

/// CLSID of this provider. Must match DLL exports and registry bindings.
pub const CLSID_MPQ_FOLDER: GUID = GUID::from_u128(0x45F174D2_D3E0_4A6C_9255_3D4F6510F3DA);

/// ProgID bound to `.mpq` family (HKCR\WarRaft.MPQArchive; HKCR\.mpq -> WarRaft.MPQArchive).
pub const DEFAULT_PROGID: &str = "WarRaft.MPQArchive";

/// File extensions supported by the handler.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[".mpq", ".w3m", ".w3x"];

/// Human-friendly provider name (HKCR\CLSID\{CLSID}\(Default)).
pub const FRIENDLY_NAME: &str = "MPQ Archive Folder Handler";
