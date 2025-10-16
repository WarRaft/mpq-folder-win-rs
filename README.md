# MPQ Folder for Windows

Expose Blizzard MPQ archives (`.mpq`, `.w3m`, `.w3x`) as read‑only folders directly inside Windows Explorer.  
This repository contains the COM shell extension (`mpq_folder_win.dll`) and a lightweight installer CLI that registers the handler for the current user.

> **Status:** early prototype. Real MPQ parsing is not implemented yet – the handler always presents a placeholder `TEST.txt` file so the Explorer integration pipeline can be exercised end-to-end.

---

## What You Get Today

- **Explorer integration:** MPQ archives appear as folders; you can drill into them just like ZIP files.
- **Stub content:** until MPQ parsing lands, the handler shows a single `TEST.txt` with a short explanation of the placeholder state.
- **COM-friendly plumbing:** Implements `IStorage`, `IInitializeWith*`, and `IThumbnailProvider` so the eventual archive reader can plug in without touching registration code.
- **Per-user installer:** the `mpq-folder-win-installer.exe` CLI writes registry entries under `HKCU`, copies the DLL into `%LOCALAPPDATA%\mpq-folder-win\`, and can clear Explorer caches or toggle logging.

---

## Building From Source

### Requirements

- Rust 1.80+ with the Windows `x86_64-pc-windows-gnu` target installed (MSVC works too if you adjust the scripts).
- On macOS/Linux, the appropriate cross toolchain (see `build-settings.sh` for defaults; MinGW-W64 on macOS is supported).
- A recent `cargo` to run the helper scripts.

### Quick build

```bash
# Clone and enter the repo
git clone https://github.com/WarRaft/mpq-folder-win-rs.git
cd mpq-folder-win-rs

# Produce both DLL + installer under ./bin/
./build-only.sh
```

The script performs two cargo builds:
1. `mpq_folder_win.dll` (the COM library).
2. `mpq-folder-win-installer.exe` (the CLI) which embeds the freshly built DLL via `include_bytes!`.

Artifacts land in `bin/`.

---

## Installing / Uninstalling

1. Copy `mpq-folder-win-installer.exe` to your Windows machine (keep `mpq_folder_win.dll` alongside if you built manually).
2. Run the installer executable:
   - `Install (current user)` writes the DLL into `%LOCALAPPDATA%\mpq-folder-win\`, registers the COM class under `HKCU`, and associates `.mpq/.w3m/.w3x`.
   - `Uninstall` removes the registry keys for the handler.
   - Optional helpers are available for toggling logging, clearing caches, and restarting Explorer.
3. **Restart Explorer** (the installer has a menu option) to pick up the new handler.

Because registration lives under `HKCU`, no administrator privileges are required.

---

## Project Layout

| Path | Purpose |
|------|---------|
| `src/lib.rs` | Entry point for the COM DLL, exported class factory, and shared constants. |
| `src/mpq_shell_provider.rs` | Implementation of the shell handler (thumbnail stub + `IStorage` skeleton). |
| `src/archive.rs` | Current placeholder archive model returning the `TEST.txt` stub. |
| `src/bin/` | CLI installer utilities and actions. |
| `build-only.sh` | CI-friendly script to build both DLL and installer artifacts. |
| `assets/` | Icons and build logs for the installer phase. |

---

## Roadmap

- [ ] Implement real MPQ parsing (table of contents + stream extraction).
- [ ] Generate real thumbnails once archive contents are understood.
- [ ] Expand `IStorage` to support nested folders inside MPQ archives.
- [ ] Add automated tests around COM activation and registry handling.

Contributions are welcome – feel free to open issues or PRs on GitHub.

---

## Troubleshooting

- **Installer fails to build:** ensure the `x86_64-pc-windows-gnu` target is installed. The build script prints the exact command it runs.
- **Explorer still shows the old handler:** run the installer’s “Clear thumbnail cache” and “Restart Explorer” options.
- **Placeholder file only:** expected until full MPQ decoding is merged.

---

## License

MIT © Nazar “nazarpunk”  
See [LICENSE](LICENSE) for details.
