use crate::utils::notify_shell_assoc::notify_shell_assoc;
use mpq_folder_win::log::log;
use mpq_folder_win::{DEFAULT_PROGID, SUPPORTED_EXTENSIONS};
use std::{fs, io};
use winreg::RegKey;
use winreg::enums::HKEY_LOCAL_MACHINE;

pub fn uninstall() -> io::Result<()> {
    if !crate::utils::admin_check::is_running_as_admin() {
        eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
        eprintln!("║  ERROR: Administrator rights required                       ║");
        eprintln!("╚══════════════════════════════════════════════════════════════╝");
        eprintln!("\nUninstallation requires administrator privileges because:");
        eprintln!("  • Registry keys must be removed from HKLM (system-wide)");
        eprintln!("  • DLL must be deleted from C:\\Program Files\\mpq-folder-win\\");
        eprintln!("\nPlease close this installer and:");
        eprintln!("  → Right-click mpq-folder-win-installer.exe");
        eprintln!("  → Select 'Run as administrator'\n");
        log("Uninstall: Not running as administrator. Aborting.");
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Administrator rights required for uninstallation"));
    }
    if let Err(err) = uninstall_inner() {
        log(format!("Uninstall failed: {}", err));
        return Err(err);
    }
    println!("\n Uninstallation completed successfully!");
    println!("  → Registry keys removed from HKLM");
    println!("  → DLL removed from C:\\Program Files\\mpq-folder-win\\");
    println!("\nRecommended next step:");
    println!("  • Restart Explorer (use menu option) to complete cleanup\n");
    Ok(())
}

fn uninstall_inner() -> io::Result<()> {
    log("Uninstall: Removing MPQ Archive Viewer");

    let root = RegKey::predef(HKEY_LOCAL_MACHINE);
    let classes = root.open_subkey(r"SOFTWARE\Classes")?;

    // Remove ProgID
    log(format!("Removing ProgID: {}", DEFAULT_PROGID));
    let _ = classes.delete_subkey_all(DEFAULT_PROGID);

    // Remove file associations
    for ext in SUPPORTED_EXTENSIONS {
        log(format!("Removing extension: {}", ext));
        let _ = classes.delete_subkey_all(ext);
    }

    // Remove installed files
    let install_dir = r"C:\Program Files\mpq-folder-win";
    log(format!("Removing directory: {}", install_dir));
    if let Err(e) = fs::remove_dir_all(install_dir) {
        if e.kind() != io::ErrorKind::NotFound {
            eprintln!("Warning: Could not remove directory: {}", e);
            eprintln!("You may need to delete it manually: {}", install_dir);
        }
    }

    notify_shell_assoc();
    log("Uninstall completed");
    Ok(())
}
