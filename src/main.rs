use mpq_folder_win::log::log;
use mpq_folder_win::mpq_filesystem::MpqFileSystem;
use std::io::{self, Write};
use winfsp::host::{FileSystemHost, VolumeParams};
use winfsp::{winfsp_init_or_die, FspError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get MPQ path from command line
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-mpq-file>", args[0]);
        eprintln!("\nThis program mounts MPQ archives as virtual drives using WinFsp.");
        eprintln!("Double-click on .mpq files to automatically mount and browse.");
        std::process::exit(1);
    }
    
    let mpq_path = &args[1];
    
    println!("MPQ Archive Viewer");
    println!("==================");
    println!("Archive: {}", mpq_path);
    println!();
    
    // Initialize WinFsp
    log("Initializing WinFsp...");
    winfsp_init_or_die();
    
    // Create MPQ filesystem
    log(format!("Loading MPQ archive: {}", mpq_path));
    let mpq_fs = MpqFileSystem::new(mpq_path.clone())
        .map_err(|e| format!("Failed to load MPQ archive: {:?}", e))?;
    
    // Configure volume parameters
    let volume_params = VolumeParams::new()
        .volume_label("MPQ Archive")
        .prefix(None) // Auto-assign drive letter
        .file_system_name("MPQ-WinFsp")
        .sector_size(512)
        .sectors_per_allocation_unit(1)
        .volume_creation_time(0)
        .volume_serial_number(0)
        .transact_timeout(10000)
        .irp_timeout(60000)
        .irp_capacity(1000)
        .file_info_timeout(1000)
        .case_sensitive_search(false)
        .case_preserved_names(true)
        .unicode_on_disk(true)
        .persistent_acls(false)
        .post_cleanup_when_modified_only(true)
        .um_file_context_is_user_context2(true)
        .build();
    
    // Mount the filesystem
    log("Mounting MPQ archive...");
    let host = FileSystemHost::new(mpq_fs, volume_params)
        .map_err(|e| format!("Failed to create filesystem host: {:?}", e))?;
    
    let mount_point = host.mount_point()
        .ok_or("Failed to get mount point")?;
    
    println!("✓ Archive mounted at: {}", mount_point);
    println!();
    
    // Open Explorer to show the mounted archive
    log(format!("Opening Explorer at {}", mount_point));
    std::process::Command::new("explorer.exe")
        .arg(&mount_point)
        .spawn()
        .map_err(|e| format!("Failed to open Explorer: {}", e))?;
    
    println!("Explorer opened. The archive is now accessible as a drive.");
    println!();
    println!("Press Enter to unmount and exit...");
    
    // Wait for user input
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    println!();
    println!("Unmounting...");
    log("Unmounting MPQ archive");
    
    // Host is automatically unmounted when dropped
    drop(host);
    
    println!("✓ Archive unmounted successfully");
    log("MPQ viewer exiting");
    
    Ok(())
}
