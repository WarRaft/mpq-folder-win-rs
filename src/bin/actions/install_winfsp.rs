use crate::EXE_BYTES;
use crate::utils::notify_shell_assoc::notify_shell_assoc;
use crate::utils::regedit::Rk;
use mpq_folder_win::log::log;
use mpq_folder_win::{DEFAULT_PROGID, SUPPORTED_EXTENSIONS};
use std::path::PathBuf;
use std::{fs, io};
use winreg::RegKey;
use winreg::enums::HKEY_LOCAL_MACHINE;

const WINFSP_DOWNLOAD_URL: &str = "https://github.com/winfsp/winfsp/releases/latest";

pub fn install() -> io::Result<()> {
    if !crate::utils::admin_check::is_running_as_admin() {
        eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
        eprintln!("║  ERROR: Administrator rights required                        ║");
        eprintln!("╚══════════════════════════════════════════════════════════════╝");
        eprintln!("\nInstallation requires administrator privileges because:");
        eprintln!("  • EXE must be copied to C:\\Program Files\\mpq-folder-win\\");
        eprintln!("  • Registry keys must be written to HKLM (system-wide)");
        eprintln!("\nPlease close this installer and:");
        eprintln!("  • Right-click mpq-folder-win-installer.exe");
        eprintln!("  • Select 'Run as administrator'\n");
        log("Install: Not running as administrator. Aborting.");
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Administrator rights required for installation"));
    }
    
    // Check for WinFsp driver
    if !check_winfsp_installed() {
        eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
        eprintln!("║  WARNING: WinFsp driver not found                            ║");
        eprintln!("╚══════════════════════════════════════════════════════════════╝");
        eprintln!("\nThis application requires WinFsp to mount MPQ archives.");
        eprintln!("\nTo install WinFsp:");
        eprintln!("  1. Download from: {}", WINFSP_DOWNLOAD_URL);
        eprintln!("  2. Install WinFsp (it's free and open-source)");
        eprintln!("  3. Run this installer again");
        eprintln!("\nAlternatively, you can continue installation now");
        eprintln!("and install WinFsp later (the app won't work until WinFsp is installed).\n");
        
        print!("Continue anyway? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("\nInstallation cancelled.");
            return Ok(());
        }
    }
    
    if let Err(err) = install_inner() {
        log(format!("Install failed: {err}"));
        return Err(err);
    }
    
    println!("\n✓ Installation completed successfully!");
    println!("  → MPQ Viewer installed to C:\\Program Files\\mpq-folder-win\\");
    println!("  → File associations registered for .mpq, .w3m, .w3x");
    println!("\nYou can now double-click on MPQ files to mount and browse them!\n");
    Ok(())
}

fn check_winfsp_installed() -> bool {
    log("Checking for WinFsp driver...");
    
    // Check for WinFsp registry key
    let paths = [
        r"SOFTWARE\WinFsp",
        r"SOFTWARE\WOW6432Node\WinFsp",
    ];
    
    for path in &paths {
        if let Ok(hklm) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(path) {
            if let Ok(install_dir) = hklm.get_value::<String, _>("InstallDir") {
                log(format!("WinFsp found at: {}", install_dir));
                return true;
            }
        }
    }
    
    log("WinFsp not found in registry");
    false
}

fn install_inner() -> io::Result<()> {
    log("Install: Starting WinFsp-based MPQ viewer installation");

    // Clean up old registrations if they exist
    pre_clean_registry()?;

    // Install the viewer executable
    let exe_path: PathBuf = {
        let base = PathBuf::from(r"C:\Program Files\mpq-folder-win");
        fs::create_dir_all(&base)?;
        let path = base.join("mpq-viewer.exe");
        
        // Try to remove old file first
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
        
        log(format!("Writing EXE to {}", path.display()));
        fs::write(&path, EXE_BYTES)?;
        
        path
    };

    let exe_path_str = exe_path.to_string_lossy().to_string();
    log(format!("EXE installed: {}", exe_path_str));

    // Register ProgID
    register_progid(&exe_path_str)?;

    // Register file associations
    for ext in SUPPORTED_EXTENSIONS {
        register_extension(ext)?;
    }

    // Notify shell of changes
    notify_shell_assoc();
    log("Install: completed successfully");

    Ok(())
}

fn pre_clean_registry() -> io::Result<()> {
    log("Pre-clean: removing old registrations");
    
    // Remove old COM-based registrations
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcr = hklm.open_subkey(r"SOFTWARE\Classes")?;
    
    // Clean up old CLSID if it exists
    let _ = hkcr.delete_subkey_all(r"CLSID\{45F174D2-D3E0-4A6C-9255-3D4F6510F3DA}");
    
    // Clean up old ProgID structure
    let _ = hkcr.delete_subkey_all(DEFAULT_PROGID);
    
    // Clean up extensions
    for ext in SUPPORTED_EXTENSIONS {
        let _ = hkcr.delete_subkey_all(ext);
    }
    
    log("Pre-clean: completed");
    Ok(())
}

fn register_progid(exe_path: &str) -> io::Result<()> {
    log(format!("Registering ProgID: {}", DEFAULT_PROGID));
    
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let classes = hklm.open_subkey(r"SOFTWARE\Classes")?;
    
    // Create ProgID
    let progid = classes.create_subkey(DEFAULT_PROGID)?.0;
    progid.set_value("", &"MPQ Archive")?;
    
    // DefaultIcon
    let default_icon = progid.create_subkey("DefaultIcon")?.0;
    default_icon.set_value("", &format!("{},0", exe_path))?;
    
    // shell\open\command
    let command = progid.create_subkey(r"shell\open\command")?.0;
    command.set_value("", &format!(r#""{}" "%1""#, exe_path))?;
    
    log("ProgID registered");
    Ok(())
}

fn register_extension(ext: &str) -> io::Result<()> {
    log(format!("Registering extension: {}", ext));
    
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let classes = hklm.open_subkey(r"SOFTWARE\Classes")?;
    
    // Create extension key
    let ext_key = classes.create_subkey(ext)?.0;
    ext_key.set_value("", &DEFAULT_PROGID)?;
    
    log(format!("Extension {} -> {}", ext, DEFAULT_PROGID));
    Ok(())
}
