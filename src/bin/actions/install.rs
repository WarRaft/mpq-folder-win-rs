use crate::DLL_BYTES;
use crate::utils::notify_shell_assoc::notify_shell_assoc;
use crate::utils::regedit::Rk;
use mpq_folder_win::log::log;
use mpq_folder_win::utils::guid::GuidExt;
use mpq_folder_win::{CLSID_MPQ_FOLDER, DEFAULT_PROGID, FRIENDLY_NAME, SHELL_PREVIEW_HANDLER_CATID, SHELL_THUMB_HANDLER_CATID, SUPPORTED_EXTENSIONS};
use std::path::PathBuf;
use std::{env, fs, io};
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

pub fn install() -> io::Result<()> {
    if let Err(err) = install_inner() {
        log(format!("Install failed: {err}"));
    }
    Ok(())
}

fn install_inner() -> io::Result<()> {
    log("Install (current user, MPQ archive folder handler): start");

    let dll_path: PathBuf = {
        let base = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Users\Default\AppData\Local"));
        let dir = base.join("mpq-folder-win");
        fs::create_dir_all(&dir).map_err(|e| {
            log(format!("Failed to create dir {}: {e}", dir.display()));
            e
        })?;
        let path = dir.join("mpq_folder_win.dll");
        log(format!("Writing DLL {} ({} bytes)", path.display(), DLL_BYTES.len()));
        fs::write(&path, DLL_BYTES).map_err(|e| {
            log(format!("Failed to write DLL {}: {e}", path.display()));
            e
        })?;
        log("DLL materialized");
        path
    };

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let progid = DEFAULT_PROGID;

    let handler_clsid = CLSID_MPQ_FOLDER.to_braced_upper();
    let thumb_catid = SHELL_THUMB_HANDLER_CATID.to_braced_upper();
    let preview_catid = SHELL_PREVIEW_HANDLER_CATID.to_braced_upper();

    log(format!("Using CLSID={} categories: THUMB={} PREVIEW={}", handler_clsid, thumb_catid, preview_catid));

    let del_tree = |path: &str| -> io::Result<()> {
        match root.delete_subkey_all(path) {
            Ok(()) => log(format!("Pre-clean: removed {}", path)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => log(format!("Pre-clean: missing {}", path)),
            Err(e) => return Err(e),
        }
        Ok(())
    };
    let del_value = |key_path: &str, value_name: &str| -> io::Result<()> {
        match root.open_subkey_with_flags(key_path, KEY_READ | KEY_SET_VALUE) {
            Ok(key) => match key.delete_value(value_name) {
                Ok(()) => log(format!("Pre-clean: removed value {}\\{}", key_path, value_name)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    log(format!("Pre-clean: value missing {}\\{}", key_path, value_name));
                }
                Err(e) => return Err(e),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => log(format!("Pre-clean: missing {}", key_path)),
            Err(e) => return Err(e),
        }
        Ok(())
    };

    log("Pre-clean: start");

    del_tree(&format!(r"Software\Classes\CLSID\{}", handler_clsid))?;

    for ext in SUPPORTED_EXTENSIONS {
        for path in [format!(r"Software\Classes\{}\ShellEx\{}", ext, thumb_catid), format!(r"Software\Classes\{}\ShellEx\{}", ext, preview_catid), format!(r"Software\Classes\SystemFileAssociations\{}\ShellEx\{}", ext, thumb_catid), format!(r"Software\Classes\SystemFileAssociations\{}\ShellEx\{}", ext, preview_catid), format!(r"Software\Classes\{}\PersistentHandler", ext)] {
            let _ = del_tree(&path);
        }

        del_value(r"Software\Microsoft\Windows\CurrentVersion\Explorer\ThumbnailHandlers", ext)?;
    }

    del_value(r"Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Approved", handler_clsid.as_str())?;

    log("Pre-clean: done");

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
        let shellex = pid.sub("ShellEx")?;
        shellex
            .sub(&thumb_catid)?
            .set_default(handler_clsid.as_str())?;
        shellex
            .sub(&preview_catid)?
            .set_default(handler_clsid.as_str())?;
        shellex
            .sub("StorageHandler")?
            .set_default(handler_clsid.as_str())?;
        pid.sub("PersistentHandler")?
            .set_default(handler_clsid.as_str())?;

        let shell = pid.sub("shell")?;
        let open = shell.sub("open")?;
        open.set_default("Open")?;
        open
            .sub("command")?
            .set_default(r#"%SystemRoot%\Explorer.exe /idlist,%I,%L"#)?;
        let explore = shell.sub("explore")?;
        explore.set_default("Explore")?;
        explore
            .sub("command")?
            .set_default(r#"%SystemRoot%\Explorer.exe /idlist,%I,%L"#)?;
    }

    for ext in SUPPORTED_EXTENSIONS {
        log(format!("Registering extension binding for {}", ext));
        let ext_key = Rk::open(&root, format!(r"Software\Classes\{}", ext))?;
        ext_key.set_default(progid)?;
        ext_key.set("PerceivedType", "compressed")?;
        ext_key.set("Content Type", "application/x-mpq")?;
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

        let ext_shell = ext_key.sub("shell")?;
        let ext_open = ext_shell.sub("open")?;
        ext_open.set_default("Open")?;
        ext_open
            .sub("command")?
            .set_default(r#"%SystemRoot%\Explorer.exe /idlist,%I,%L"#)?;

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
    log("Installed in HKCU (MPQ archive handler). Restart Explorer to refresh.");
    Ok(())
}
