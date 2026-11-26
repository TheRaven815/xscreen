# xscreen

Native screen capture and ShareX-inspired image annotation studio for Windows 10/11, written in Rust.

## Run

Needs the Rust MSVC toolchain and Visual Studio C++ Build Tools.

```powershell
cargo run --locked
```

- **Tray-first startup**: launches silently into the Windows system tray; closing the main studio window keeps `xscreen` active in the tray.
- **Compact Studio (`Album · Editor · Settings`)**: single `38px` top command bar with instant switching between **Album**, **Editor**, and **Settings**, plus quick **EN / TR** language toggle, **Dark / Light** theme toggle, and **Capture** trigger.
- **ShareX-style In-App Image Editor**:
  - Click any saved screenshot in **Album** (or press `Ctrl+E` during live capture, or `Ctrl+V` to paste from clipboard) to open it in the full in-app Editor.
  - **Object-based annotations (`Select` / `V`)**: click (or right-click from any tool) any drawn arrow, line, rectangle, ellipse, triangle, freehand stroke, highlighter, text, step number, or blur/mosaic region to move it, drag its endpoint/corner handles to resize, change its color, stroke width, outline/fill mode, or blur style live from the contextual property bar, double-click text to re-edit in place, duplicate (`Ctrl+D`), bring to front, or delete (`Delete`).
  - **Crop & Zoom/Pan**: interactive `Crop (C)` with rule-of-thirds guides and 8 resize handles (non-destructively shifts existing vector shapes and supports `Ctrl+Z` undo!), wheel zoom (`10%–800%`) around cursor, and middle-drag / `Space+Drag` panning.
  - **Save & Copy**: `Copy (Ctrl+C)` writes `CF_DIBV5` + `PNG` to clipboard; `Save (Ctrl+S)` atomically updates the opened PNG; `Save copy (Ctrl+Shift+S)` writes a new timestamped PNG in the capture folder.
  - **Unsaved edits**: `Esc` asks before returning to Album if the image has unsaved changes.
- **Live Multi-Monitor Capture Overlay**:
  - `Print Screen` (or configured global shortcut) freezes all attached monitors in parallel (`GDI` fast path for SDR, `DXGI Desktop Duplication` for HDR/wide-gamut).
  - Drag to select region, `Alt` to snap to window bounds (DWM extended frame), 8 resize handles, 8× pixel magnifier with `#RRGGBB` readout.
  - Includes `Select` tool + right-click object selection right on the frozen capture overlay, plus `Editor (Ctrl+E)` button to jump straight into the full Studio Editor with live vector shapes intact.
- **Bilingual & Themed**: **English** and **Türkçe** interface languages and **Dark / Light** themes apply across the Studio window, Capture Overlay, and Win32 system tray menu (`%APPDATA%\xscreen\config.json`).

## Local Data

xscreen does not upload screenshots or send telemetry. Settings and diagnostic logs stay under `%APPDATA%\xscreen`; saved screenshots go to the configured folder (by default, `Pictures\xscreen`). Copying an image places it on the Windows clipboard.

## Build & Check

```powershell
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

For a portable Windows MSVC build, use a static C runtime. Remap local source paths and strip debug information before sharing the executable:

```powershell
$env:CARGO_ENCODED_RUSTFLAGS = @(
    '-C', 'target-feature=+crt-static',
    '-C', 'strip=debuginfo',
    "--remap-path-prefix=$env:USERPROFILE=/build/user",
    "--remap-path-prefix=$($PWD.Path)=/build/xscreen"
) -join [char]31
cargo build --locked --release
Remove-Item Env:CARGO_ENCODED_RUSTFLAGS
```

The binary is `target\release\xscreen.exe`. If your Cargo or Rust installation is outside your user profile, remap those paths too or build on a fresh CI runner. Do not publish `.pdb` debug symbols or the build directory.

Optional Windows integration checks are in `tests/`. They use the interactive desktop; `album-smoke.ps1` moves the mouse, updates the clipboard, and writes screenshots under `dist\smoke\album` by default. Review any screenshots before sharing them.
