use crate::utils::notify_shell_assoc::notify_shell_assoc;

use mpq_folder_win::log::log;
use mpq_folder_win::utils::guid::GuidExt;
use mpq_folder_win::{CLSID_MPQ_FOLDER, DEFAULT_PROGID, SHELL_PREVIEW_HANDLER_CATID, SHELL_THUMB_HANDLER_CATID, SUPPORTED_EXTENSIONS};
use std::io;
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

pub fn uninstall() -> io::Result<()> {
    if let Err(err) = uninstall_inner() {
        log(format!("Uninstall failed: {}", err));
    }
    Ok(())
}

fn uninstall_inner() -> io::Result<()> {
    log("Uninstall (current user): start — removing shell bindings.");

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let progid = DEFAULT_PROGID;

    let handler_clsid = CLSID_MPQ_FOLDER.to_braced_upper();
    let thumb_catid = SHELL_THUMB_HANDLER_CATID.to_braced_upper();
    let preview_catid = SHELL_PREVIEW_HANDLER_CATID.to_braced_upper();

    let del_tree = |path: &str| -> io::Result<()> {
        match root.delete_subkey_all(path) {
            Ok(()) => log(format!("Removed key tree: {}", path)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                log(format!("Key missing (skip): {}", path));
            }
            Err(e) => return Err(e),
        }
        Ok(())
    };

    let del_value = |key_path: &str, value_name: &str| -> io::Result<()> {
        match root.open_subkey_with_flags(key_path, KEY_READ | KEY_SET_VALUE) {
            Ok(key) => match key.delete_value(value_name) {
                Ok(()) => log(format!("Removed value: {} \\ {}", key_path, value_name)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    log(format!("Value missing (skip): {} \\ {}", key_path, value_name));
                }
                Err(e) => return Err(e),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                log(format!("Key missing (skip): {}", key_path));
            }
            Err(e) => return Err(e),
        }
        Ok(())
    };

    let remove_shellex = |root_path: &str, cat: &str| -> io::Result<()> {
        let target = format!(r"{}\ShellEx\{}", root_path, cat);
        del_tree(&target)
    };

    let approved_path = r"Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Approved";
    del_value(approved_path, handler_clsid.as_str())?;

    // ProgID bindings
    remove_shellex(&format!(r"Software\Classes\{}", progid), &thumb_catid)?;
    remove_shellex(&format!(r"Software\Classes\{}", progid), &preview_catid)?;
    remove_shellex(&format!(r"Software\Classes\{}", progid), "StorageHandler")?;
    del_value(&format!(r"Software\Classes\{}", progid), "CLSID")?;
    del_value(&format!(r"Software\Classes\{}", progid), "FriendlyTypeName")?;
    del_tree(&format!(r"Software\Classes\{}\PersistentHandler", progid))?;
    del_tree(&format!(r"Software\Classes\{}\shell", progid))?;

    for ext in SUPPORTED_EXTENSIONS {
        remove_shellex(&format!(r"Software\Classes\{}", ext), &thumb_catid)?;
        remove_shellex(&format!(r"Software\Classes\{}", ext), &preview_catid)?;
        remove_shellex(&format!(r"Software\Classes\{}", ext), "StorageHandler")?;
        del_tree(&format!(r"Software\Classes\{}\shell", ext))?;
        remove_shellex(&format!(r"Software\Classes\SystemFileAssociations\{}", ext), &thumb_catid)?;
        remove_shellex(&format!(r"Software\Classes\SystemFileAssociations\{}", ext), &preview_catid)?;
        remove_shellex(&format!(r"Software\Classes\SystemFileAssociations\{}", ext), "StorageHandler")?;

        del_value(r"Software\Microsoft\Windows\CurrentVersion\Explorer\ThumbnailHandlers", ext)?;
        del_tree(&format!(r"Software\Classes\{}\PersistentHandler", ext))?;
    }

    del_tree(&format!(r"Software\Classes\CLSID\{}", handler_clsid))?;

    notify_shell_assoc("uninstall");
    log("Uninstall completed (HKCU). Thumbnail preview bindings removed.");
    Ok(())
}
