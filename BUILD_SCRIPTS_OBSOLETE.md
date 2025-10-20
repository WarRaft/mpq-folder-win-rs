# Build Scripts Status

The following bash scripts were used for cross-compilation from macOS/Linux but are **now obsolete** after migrating to WinFsp:

- `build-only.sh` - Cross-compiles DLL (no longer possible)
- `build-publish.sh` - Publishes releases (needs update for EXE artifacts)
- `build-settings.sh` - Cross-compilation settings (not needed)
- `build-winfsp.sh` - Attempts cross-compilation of WinFsp (fails on non-Windows)

## Why They Don't Work

WinFsp's build dependencies (`winfsp-sys/build.rs`) require:
- Windows OS (checks `cfg!(target_os = "windows")`)
- WinFsp SDK installed on the **build** machine (not target)
- Windows Registry access to find WinFsp installation path

Build scripts execute on the **host** OS (macOS/Linux), not the **target** OS (Windows), making cross-compilation impossible.

## Current Build Method

See [BUILD_WINDOWS.md](BUILD_WINDOWS.md) for instructions on building natively on Windows.

Quick version:
```cmd
cargo build --release --bin mpq-viewer
cargo build --release --bin mpq-folder-win-installer
```

## Keeping These Scripts

These scripts are kept for historical reference and may be adapted in the future for:
- GitHub Actions workflows with `runs-on: windows-latest`
- Automated release publishing
- Version bumping automation

They should **not** be used for local development builds.
