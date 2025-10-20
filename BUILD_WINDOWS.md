# Building on Windows

This project uses WinFsp which **can only be compiled on Windows**.

## Prerequisites

1. **Install Rust**
   ```powershell
   # Download and run: https://rustup.rs/
   # Or use winget:
   winget install Rustlang.Rustup
   ```

2. **Install WinFsp SDK**
   - Download from: https://github.com/winfsp/winfsp/releases/latest
   - Run `winfsp-*.msi` installer
   - Install both "Core" and "Developer" components
   - Default installation path: `C:\Program Files (x86)\WinFsp`

3. **Install MinGW toolchain** (if using GNU target)
   ```powershell
   rustup target add x86_64-pc-windows-gnu
   ```

## Building

```powershell
# Navigate to project directory
cd path\to\mpq-folder-win-rs

# Build viewer executable
cargo build --release --bin mpq-viewer

# Build installer executable  
cargo build --release --bin mpq-folder-win-installer

# Artifacts will be in:
# - target\release\mpq-viewer.exe
# - target\release\mpq-folder-win-installer.exe
```

## Alternative: Use MSVC toolchain

If you have Visual Studio installed, you can use the MSVC toolchain:

```powershell
# Add MSVC target
rustup target add x86_64-pc-windows-msvc

# Build
cargo build --release --target x86_64-pc-windows-msvc
```

## Installation

1. Run `mpq-folder-win-installer.exe` as Administrator
2. If WinFsp driver is not installed, installer will show download link
3. Install WinFsp driver first, then run installer again
4. Installer will:
   - Copy `mpq-viewer.exe` to `C:\Program Files\mpq-folder-win\`
   - Register `.mpq`, `.w3m`, `.w3x` file associations

## Testing

1. Create or obtain a test `.mpq` file
2. Double-click on the `.mpq` file
3. Expected behavior:
   - Console window opens showing mount progress
   - Archive mounts as drive letter (e.g., `Z:\`)
   - Explorer opens automatically showing archive contents
   - Press Enter in console to unmount

## Troubleshooting

### Build errors about WinFsp not found

Make sure WinFsp SDK is installed with "Developer" component.
Check that registry key exists:
```
HKEY_LOCAL_MACHINE\SOFTWARE\WinFsp
```

### "WinFSP is only supported on Windows" error

You are trying to build on macOS/Linux. This project **must** be built on Windows.

### Linker errors

If using GNU toolchain, make sure MinGW is installed:
```powershell
# Install MSYS2 first: https://www.msys2.org/
# Then in MSYS2 terminal:
pacman -S mingw-w64-x86_64-gcc
```

Or switch to MSVC toolchain (easier on Windows).

## Development Workflow

Since this project can only be built on Windows:

1. Develop code on macOS (code editing, structure, logic)
2. Commit and push to GitHub
3. Pull on Windows machine
4. Build and test on Windows
5. Repeat

Or use a Windows VM on macOS for faster iteration.

## Files Structure After Build

```
target/
└── release/
    ├── mpq-viewer.exe          # Main viewer (~2-5MB)
    ├── mpq-folder-win-installer.exe  # Installer (~5-8MB, embeds viewer)
    └── *.pdb                   # Debug symbols (optional)
```

## Next Steps

After successful build and test:

1. Integrate real MPQ parser (replace placeholder in `src/archive.rs`)
2. Add compression support (ZLIB, BZIP2, etc.)
3. Optimize performance (caching, buffering)
4. Add thumbnail provider integration (optional)

See `WINFSP_MIGRATION.md` for more details about the architecture.
