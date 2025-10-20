# MPQ Folder for Windows

Mount Blizzard MPQ archives (`.mpq`, `.w3m`, `.w3x`) as virtual drives in Windows Explorer using WinFsp.  
This repository contains the filesystem driver (`mpq-viewer.exe`) and an installer that registers file associations system-wide.

> **Status:** WinFsp-based architecture implemented. Real MPQ parsing deferred; currently displays placeholder `TEST.txt` to validate the virtual filesystem layer.
> 
> **Requirements:** 
> - WinFsp driver must be installed (free, open-source)
> - Administrator rights required for file association registration
> - **Must be built on Windows** (WinFsp SDK dependencies)

---

## What You Get Today

- **Virtual filesystem mounting:** MPQ archives mount as separate drive letters (e.g., `Z:\`) when double-clicked
- **Explorer integration:** Mounted archives appear as regular folders in Explorer
- **Placeholder content:** One file `TEST.txt` exposed inside every archive for testing
- **Simple registration:** File associations registered with 3 registry keys (vs 20+ in COM approach)
- **System-wide installer:** `mpq-folder-win-installer.exe` checks for WinFsp driver, registers `.mpq/.w3m/.w3x` associations

---

## Building From Source

### Requirements

- **Windows OS** (cannot cross-compile from macOS/Linux due to WinFsp SDK dependencies)
- Rust 1.80+ installed via [rustup](https://rustup.rs/)
- WinFsp SDK installed from https://github.com/winfsp/winfsp/releases/latest
  - Install both "Core" and "Developer" components
  - Installer automatically configures environment variables

### Build Steps

See [BUILD_WINDOWS.md](BUILD_WINDOWS.md) for detailed instructions.

Quick version:

```cmd
REM Clone the repository
git clone https://github.com/WarRaft/mpq-folder-win-rs.git
cd mpq-folder-win-rs

REM Build the viewer executable
cargo build --release --bin mpq-viewer

REM Build the installer
cargo build --release --bin mpq-folder-win-installer
```

Artifacts will be in `target/release/`:
- `mpq-viewer.exe` - Mounts MPQ archives as virtual drives
- `mpq-folder-win-installer.exe` - Registers file associations

---

## Installing / Uninstalling

1. **Install WinFsp** from https://github.com/winfsp/winfsp/releases/latest
   - Download and run the installer (~5MB)
   - No configuration needed

2. **Run the installer as administrator**:
   ```cmd
   REM Right-click → Run as administrator
   mpq-folder-win-installer.exe
   ```
   
   Available actions:
   - `Install` - Copies `mpq-viewer.exe` to `C:\Program Files\mpq-folder-win\` and registers file associations
   - `Uninstall` - Removes registry keys and deletes program files
   - `Restart Explorer` - Reloads Explorer to apply changes
   - `Exit` - Quit installer

3. **Test**: Double-click any `.mpq` file. It should mount as a virtual drive and open in Explorer.

**Note:** Administrator privileges are required for installation/uninstallation because registry changes are made to HKEY_LOCAL_MACHINE.

---

## Project Layout

| Path | Purpose |
|------|---------|
| `src/main.rs` | `mpq-viewer.exe` - Mounts MPQ archives via WinFsp, opens Explorer |
| `src/mpq_filesystem.rs` | FileSystemContext implementation (read, open, close, read_directory, get_volume_info) |
| `src/archive.rs` | Placeholder MPQ model (returns `TEST.txt` for now) |
| `src/lib.rs` | Shared constants (ProgID, extensions, app name) |
| `src/bin/installer.rs` | `mpq-folder-win-installer.exe` - Interactive installer menu |
| `src/bin/actions/` | Installer actions (install, uninstall, restart explorer) |
| `BUILD_WINDOWS.md` | Detailed build instructions for Windows |
| `WINFSP_MIGRATION.md` | Architecture documentation and rationale |

---

## How It Works

1. **File Association:** Double-clicking `.mpq` files launches `mpq-viewer.exe` with the file path
2. **WinFsp Mounting:** The viewer loads the archive and creates a virtual filesystem using WinFsp
3. **Drive Letter Assignment:** WinFsp automatically assigns an available drive letter (e.g., `Z:\`)
4. **Explorer Opens:** The mounted drive opens in Explorer automatically
5. **Unmount:** Press Enter in the viewer console to unmount and close

---

## Roadmap

- [ ] Integrate real MPQ parsing library (replace placeholder)
- [ ] Add compression support (ZLIB, BZIP2)
- [ ] Implement nested folder support inside archives
- [ ] Add thumbnail provider (optional COM component)
- [ ] Performance optimizations (caching, buffering)

Contributions welcome – open issues or PRs on GitHub!

---


## Troubleshooting

- **Build fails:** Ensure you're building on Windows with WinFsp SDK installed. Cross-compilation from macOS/Linux is not supported.
- **"WinFsp driver not found":** Install WinFsp from https://github.com/winfsp/winfsp/releases/latest
- **Installer permission error:** Right-click installer → Run as administrator
- **Double-click doesn't mount:** Check file associations in Registry: `HKEY_LOCAL_MACHINE\SOFTWARE\Classes\.mpq`
- **Drive doesn't appear:** Check WinFsp service is running: `sc query WinFsp.Launcher`
- **Only shows TEST.txt:** Expected - real MPQ parsing not yet implemented

---

## License

MIT © Nazar “nazarpunk”  
See [LICENSE](LICENSE) for details.
