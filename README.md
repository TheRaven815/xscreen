<p align="center">
  <img src="assets/xscreen.png" alt="xscreen icon" width="88" height="88" />
</p>

<h1 align="center">xscreen</h1>

<p align="center"><strong>Capture, annotate, and save without leaving your workflow.</strong></p>
<p align="center">Portable screenshot studio for Windows 10 and 11 · x64 · No installer</p>

<p align="center">
  <a href="https://github.com/TheRaven815/xscreen/releases/latest">Download latest release</a>
  · <a href="#quick-start">Quick start</a>
  · <a href="#shortcuts">Shortcuts</a>
  · <a href="#build-from-source">Build from source</a>
</p>

## Download

Open [Releases](https://github.com/TheRaven815/xscreen/releases), download **`xscreen-windows-x64.exe`** under **Assets**, and run it. xscreen starts in the Windows notification area. Right-click its icon to open **Album**, **Editor**, or **Settings**. Closing the Studio window leaves xscreen running; choose **Exit** in the tray menu to quit.

To verify the download, run `Get-FileHash .\xscreen-windows-x64.exe -Algorithm SHA256` in PowerShell and compare the result with `SHA256SUMS.txt` from the same release.

### Update xscreen

In **Settings → Updates**, select **Check for updates**. If a newer stable release is available, you can read its release notes and download it. xscreen verifies the download against the release’s SHA-256 checksum. Select **Install and restart** to replace your portable EXE in place. Finish any active capture and save or discard Editor changes first. Your settings and screenshots stay where they are.

If the EXE is in a folder you cannot write to, download the new EXE from **Releases**, exit xscreen from the tray, and replace it manually.

## Quick start

1. Press `Print Screen` to freeze your displays, then drag to select a region. Hold `Alt` to snap to the window under the pointer.
2. Add annotations directly in the overlay, or press `Ctrl+E` to continue in **Editor** with your drawings still editable.
3. Copy your selection with `Enter` in the overlay or `Ctrl+C` in Editor. Use `Ctrl+S` to save a PNG. Open saved screenshots again from **Album**.

## What you can do

- **Capture across monitors.** Resize your selection with edge and corner handles. Use the optional pixel magnifier and color readout for precise crops.
- **Edit annotations after drawing.** Select, move, resize, recolor, duplicate, or remove arrows, shapes, pen strokes, highlights, text, numbered markers, and blur or mosaic regions. Undo and redo changes as you work.
- **Work with existing images.** Open a saved image from Album or paste one from the clipboard. Zoom, pan, crop, overwrite the PNG, or save a separate copy. Editor asks before discarding unsaved work on `Esc`.
- **Make it yours.** Choose a global shortcut, save folder, cursor visibility, and Windows startup behavior. Use light or dark mode in English or Türkçe.

## Shortcuts

| Shortcut | Where | Action |
| --- | --- | --- |
| `Print Screen` | Anywhere | Start capture with the default shortcut |
| `Alt` (hold) | Capture | Snap to window bounds |
| `Enter` | Capture | Copy selected region |
| `Ctrl+E` | Capture | Open selection in Editor |
| `Ctrl+V` | Editor | Paste image from clipboard |
| `V` / `C` | Editor | Select tool / Crop tool |
| `Ctrl+C` | Editor | Copy edited image |
| `Ctrl+S` | Capture / Editor | Save PNG |
| `Ctrl+Shift+S` | Editor | Save separate PNG copy |
| `Ctrl+Z` / `Ctrl+Y` | Capture / Editor | Undo / Redo |
| `Esc` | Editor | Exit active edit mode or return to Album; prompts if changes are unsaved |

If Windows or another app already uses `Print Screen`, choose a different shortcut in **Settings**.

## Privacy and storage

xscreen does not upload screenshots or send telemetry. Update checks and downloads contact GitHub only when you request them. Captures save to your configured folder (`Pictures\xscreen` by default). Settings and diagnostic logs stay under `%APPDATA%\xscreen`. Copying an image puts it on the Windows clipboard.

## Build from source

Use Windows with the stable Rust MSVC toolchain, Visual Studio C++ Build Tools, and a Windows SDK.

```powershell
cargo run --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

To build a portable x64 executable with a static C runtime:

```powershell
$env:CARGO_ENCODED_RUSTFLAGS = @(
    '-C', 'target-feature=+crt-static',
    '-C', 'strip=debuginfo',
    "--remap-path-prefix=$env:USERPROFILE=/build/user",
    "--remap-path-prefix=$($PWD.Path)=/build/xscreen"
) -join [char]31
cargo build --locked --release --target x86_64-pc-windows-msvc
Remove-Item Env:CARGO_ENCODED_RUSTFLAGS
```

Output: `target\x86_64-pc-windows-msvc\release\xscreen.exe`. Build on a clean CI runner or remap any additional local Cargo and Rust paths before sharing. Do not publish `.pdb` files or the whole build directory.

Optional [Windows smoke tests](tests/) interact with the desktop and clipboard. Review generated screenshots before sharing them.

## Publish a release

Push a stable version tag (`vMAJOR.MINOR.PATCH`) on a commit that contains the [release workflow](.github/workflows/release.yml). Use a new version for each release:

```bash
git tag -a v0.2.0 -m "xscreen v0.2.0"
git push origin v0.2.0
```

GitHub Actions uses the tag as the app and EXE version, tests the tagged code, and publishes a GitHub Release with generated notes, `xscreen-windows-x64.exe`, and `SHA256SUMS.txt`. Ordinary branch pushes do not create releases. Follow progress in the repository’s [Actions tab](https://github.com/TheRaven815/xscreen/actions/workflows/release.yml).
