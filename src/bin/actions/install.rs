use crate::DLL_BYTES;
use crate::utils::notify_shell_assoc::notify_shell_assoc;
use crate::utils::regedit::Rk;
use mpq_folder_win::log::log;
use mpq_folder_win::utils::guid::GuidExt;
use mpq_folder_win::{CLSID_MPQ_FOLDER, DEFAULT_PROGID, FRIENDLY_NAME, SHELL_PREVIEW_HANDLER_CATID, SHELL_THUMB_HANDLER_CATID, SUPPORTED_EXTENSIONS};
use std::path::PathBuf;
use std::{fs, io};
use winreg::RegKey;
use winreg::enums::HKEY_LOCAL_MACHINE;

pub fn install() -> io::Result<()> {
    if !crate::utils::admin_check::is_running_as_admin() {
        eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
        eprintln!("║  ERROR: Administrator rights required                        ║");
        eprintln!("╚══════════════════════════════════════════════════════════════╝");
        eprintln!("\nInstallation requires administrator privileges because:");
        eprintln!("  • DLL must be copied to C:\\Program Files\\mpq-folder-win\\");
        eprintln!("  • Registry keys must be written to HKLM (system-wide)");
        eprintln!("\nPlease close this installer and:");
        eprintln!("  • Right-click mpq-folder-win-installer.exe");
        eprintln!("  • Select 'Run as administrator'\n");
        log("Install: Not running as administrator. Aborting.");
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Administrator rights required for installation"));
    }
    if let Err(err) = install_inner() {
        log(format!("Install failed: {err}"));
        return Err(err);
    }
    println!("\n Installation completed successfully!");
    println!("  → DLL installed to C:\\Program Files\\mpq-folder-win\\");
    println!("  → Registry keys written to HKLM");
    println!("\nRecommended next steps:");
    println!("  1. Restart Explorer (use menu option)");
    println!("  2. Try opening an MPQ/W3M/W3X file\n");
    Ok(())
}

fn install_inner() -> io::Result<()> {
    log("Install (admin, MPQ archive folder handler, HKLM): start");

    // Program Files path
    let dll_path: PathBuf = {
        let base = PathBuf::from(r"C:\Program Files\mpq-folder-win");
        fs::create_dir_all(&base).map_err(|e| {
            log(format!("Failed to create dir {}: {e}", base.display()));
            e
        })?;
        let path = base.join("mpq_folder_win.dll");
        log(format!("Writing DLL {} ({} bytes)", path.display(), DLL_BYTES.len()));
        fs::write(&path, DLL_BYTES).map_err(|e| {
            log(format!("Failed to write DLL {}: {e}", path.display()));
            e
        })?;
        log("DLL materialized");
        path
    };

    let root = RegKey::predef(HKEY_LOCAL_MACHINE);
    let progid = DEFAULT_PROGID;

    let handler_clsid = CLSID_MPQ_FOLDER.to_braced_upper();
    let thumb_catid = SHELL_THUMB_HANDLER_CATID.to_braced_upper();
    let preview_catid = SHELL_PREVIEW_HANDLER_CATID.to_braced_upper();
    // StorageHandler GUID used by ZIP/CAB
    let storage_handler_guid = "{E88DCCE0-B7B3-11d1-A9F0-00AA0060FA31}";

    log(format!("Using CLSID={} categories: THUMB={} PREVIEW={}", handler_clsid, thumb_catid, preview_catid));

    // No pre-cleaning of old keys (per user request)

    {
        log("Approving MPQ shell handler");
        let approved = Rk::open(&root, r"Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Approved")?;
        approved.set(&handler_clsid, FRIENDLY_NAME)?;
    }

    {
        log("Registering handler CLSID tree");
        let cls = Rk::open(&root, format!(r"Software\Classes\CLSID\{}", handler_clsid))?;
        cls.set_default(FRIENDLY_NAME)?;
        cls.set("DisableProcessIsolation", 1u32)?;
        let inproc = cls.sub("InprocServer32")?;
        inproc.set_default(dll_path.as_os_str())?;
        inproc.set("ThreadingModel", "Apartment")?;
        let _ = cls.sub(&format!(r"Implemented Categories\{}", thumb_catid))?;
        // IShellFolder registration for Explorer integration (folder behavior like ZIP)
        let shellfolder = cls.sub("ShellFolder")?;
        // SFGAO_FOLDER | SFGAO_FILESYSTEM | SFGAO_FILESYSANCESTOR | SFGAO_HASSUBFOLDER | SFGAO_BROWSABLE
        shellfolder.set("Attributes", 0xF0400044u32)?;
        shellfolder.set("WantsFORPARSING", "")?;
        shellfolder.set("HideOnDesktopPerUser", "")?;
        shellfolder.set("SortOrderIndex", 66u32)?;
        shellfolder.set("InfoTip", "Displays contents of MPQ archives")?;
        // StorageHandler GUID for ZIP-like behavior
        cls.sub("ShellEx")?
            .sub(storage_handler_guid)?
            .set_default(handler_clsid.as_str())?;
        let _ = cls.sub(&format!(r"Implemented Categories\{{00021493-0000-0000-C000-000000000046}}"))?; // CATID_ShellFolder
    }

    {
        log("Binding ProgID ShellEx handlers");
        let pid = Rk::open(&root, format!(r"Software\Classes\{}", progid))?;
        if pid
            .get::<String>("")
            .map(|s| s.trim_matches(char::from(0)).is_empty())
            .unwrap_or(true)
        {
            pid.set_default(FRIENDLY_NAME)?;
        }
        pid.set("CLSID", handler_clsid.as_str())?;
        pid.set("FriendlyTypeName", FRIENDLY_NAME)?;
        // Mark as shortcut/folder (like ZIP files)
        pid.set("IsShortcut", "")?;
        let shellex = pid.sub("ShellEx")?;
        // StorageHandler GUID for ZIP-like behavior
        shellex
            .sub(storage_handler_guid)?
            .set_default(handler_clsid.as_str())?;
        shellex
            .sub(&thumb_catid)?
            .set_default(handler_clsid.as_str())?;
        shellex
            .sub(&preview_catid)?
            .set_default(handler_clsid.as_str())?;
        shellex
            .sub("StorageHandler")?
            .set_default(handler_clsid.as_str())?;
        // FolderShortcut: critical for folder behavior (like ZIP)
        shellex
            .sub("FolderShortcut")?
            .set_default(handler_clsid.as_str())?;
        // Junction Point Handler (Folder Shortcut GUID)
        shellex
            .sub("{0AFACED1-E828-11D1-9187-B532F1E9575D}")?
            .set_default(handler_clsid.as_str())?;
        // Namespace Extension for IShellFolder
        shellex
            .sub("{00021500-0000-0000-C000-000000000046}")?
            .set_default(handler_clsid.as_str())?;
        pid.sub("PersistentHandler")?
            .set_default(handler_clsid.as_str())?;
        pid.sub("DefaultIcon")?
            .set_default("%SystemRoot%\\System32\\shell32.dll,3")?;

        let shell = pid.sub("shell")?;
        let open = shell.sub("open")?;
        open.set_default("Open")?;
        // Use /idlist for ZIP-like folder behavior
        open.sub("command")?
            .set_default(format!("explorer.exe ::{}%1", handler_clsid))?;
        let explore = shell.sub("explore")?;
        explore.set_default("Explore")?;
        explore
            .sub("command")?
            .set_default(format!("explorer.exe ::{}%1", handler_clsid))?;
    }

    for ext in SUPPORTED_EXTENSIONS {
        log(format!("Registering extension binding for {}", ext));
        let ext_key = Rk::open(&root, format!(r"Software\Classes\{}", ext))?;
        ext_key.set_default(progid)?;
        ext_key.set("PerceivedType", "compressed")?;
        ext_key.set("Content Type", "application/x-mpq")?;
        // Mark as shortcut/folder at extension level
        ext_key.set("IsShortcut", "")?;
        // StorageHandler GUID for ZIP-like behavior
        Rk::open(&root, format!(r"Software\Classes\{}\ShellEx", ext))?
            .sub(storage_handler_guid)?
            .set_default(handler_clsid.as_str())?;
        ext_key
            .sub("PersistentHandler")?
            .set_default(handler_clsid.as_str())?;

        let ext_sx = Rk::open(&root, format!(r"Software\Classes\{}\ShellEx", ext))?;
        ext_sx
            .sub(&thumb_catid)?
            .set_default(handler_clsid.as_str())?;
        ext_sx
            .sub(&preview_catid)?
            .set_default(handler_clsid.as_str())?;
        ext_sx
            .sub("StorageHandler")?
            .set_default(handler_clsid.as_str())?;
        // FolderShortcut: critical for folder behavior (like ZIP)
        ext_sx
            .sub("FolderShortcut")?
            .set_default(handler_clsid.as_str())?;
        // Junction Point Handler (Folder Shortcut GUID)
        ext_sx
            .sub("{0AFACED1-E828-11D1-9187-B532F1E9575D}")?
            .set_default(handler_clsid.as_str())?;
        // Namespace Extension for IShellFolder
        ext_sx
            .sub("{00021500-0000-0000-C000-000000000046}")?
            .set_default(handler_clsid.as_str())?;

        let ext_shell = ext_key.sub("shell")?;
        let ext_open = ext_shell.sub("open")?;
        ext_open.set_default("Open")?;
        // Use /idlist for ZIP-like folder behavior
        ext_open
            .sub("command")?
            .set_default(format!("explorer.exe ::{}%1", handler_clsid))?;

        let sfa_sx = Rk::open(&root, format!(r"Software\Classes\SystemFileAssociations\{}\ShellEx", ext))?;
        sfa_sx
            .sub(&thumb_catid)?
            .set_default(handler_clsid.as_str())?;
        sfa_sx
            .sub(&preview_catid)?
            .set_default(handler_clsid.as_str())?;
        sfa_sx
            .sub("StorageHandler")?
            .set_default(handler_clsid.as_str())?;

        Rk::open(&root, r"Software\Microsoft\Windows\CurrentVersion\Explorer\ThumbnailHandlers")?.set(ext, handler_clsid.as_str())?;
    }

    if let Ok((adv, _)) = root.create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced") {
        log("Setting Explorer Advanced toggles for previews/thumbnails");
        let _ = adv.set_value("ShowPreviewHandlers", &1u32);
        let _ = adv.set_value("IconsOnly", &0u32);
        let _ = adv.set_value("DisableThumbnails", &0u32);
        let _ = adv.set_value("DisableThumbnailCache", &0u32);
        let _ = adv.set_value("DisableThumbnailsOnNetworkFolders", &0u32);
    }

    notify_shell_assoc("install");
    log("Installed in HKLM (MPQ archive handler). Restart Explorer to refresh.");
    Ok(())
}
