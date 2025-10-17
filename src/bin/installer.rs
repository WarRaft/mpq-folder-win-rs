#![cfg(windows)]

use mpq_folder_win::log::log;
use std::{env, io, io::Write};

// Embedded DLL that you copy into ./bin/ at build time.
// The EXE will re-materialize it under %LOCALAPPDATA%\mpq-folder-win\
static DLL_BYTES: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/bin/mpq_folder_win.dll"));

// Single source of truth from the library (your keys module)
use crate::actiions::dialog::{Action, action_choose, action_execute};

#[path = "actions/mod.rs"]
mod actiions;

#[path = "utils/mod.rs"]
mod utils;

fn main() -> io::Result<()> {
    log("Installer started");
    loop {
        let (action, label) = action_choose()?;
        log(format!("Menu selection: {}", label));

        if action == Action::Exit {
            log("Installer exiting");
            break;
        }

        match action_execute(action) {
            Ok(()) => {
                // Only print success message for non-install/uninstall actions
                if action != crate::actiions::dialog::Action::Install && action != crate::actiions::dialog::Action::Uninstall {
                    log(format!("Action '{}' completed successfully", label));
                    println!("\nAction '{}' completed successfully", label);
                }
            }
            Err(err) => {
                log(format!("Action '{}' failed: {}", label, err));
                eprintln!("\nError: Action '{}' failed: {}", label, err);
                // Don't exit, just show error and return to menu
            }
        }

        pause("\nPress Enter to return to the menu...");
    }
    Ok(())
}

fn pause(msg: &str) {
    print!("{msg}");
    let _ = io::stdout().flush();
    // Use read_line to avoid printing localized messages from external tools
    let mut _buf = String::new();
    let _ = io::stdin().read_line(&mut _buf);
}
