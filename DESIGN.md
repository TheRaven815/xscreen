---
name: xscreen
description: Compact Windows screenshot studio with top-bar navigation, ShareX-inspired object annotation editor, bilingual EN/TR support, and multi-monitor capture overlay.
colors:
  dark-canvas: "#151619"
  dark-bg: "#1c1d21"
  dark-bar: "#24262b"
  dark-panel: "#2b2e34"
  dark-text: "#eceef2"
  dark-muted: "#969da8"
  dark-line: "#383c45"
  dark-hover: "#363a43"
  dark-select: "#1e4268"
  dark-accent: "#0e70c0"
  dark-accent-bright: "#40a0f5"
  dark-accent-fg: "#ffffff"
  dark-success: "#238636"
  dark-error: "#e5534b"
  light-canvas: "#dee2e8"
  light-bg: "#f3f4f6"
  light-bar: "#ffffff"
  light-panel: "#f8f9fb"
  light-text: "#16191e"
  light-muted: "#525866"
  light-line: "#d0d5dd"
  light-hover: "#e9ecf1"
  light-select: "#dbeafe"
  light-accent: "#0067c0"
  light-accent-bright: "#0067c0"
  light-accent-fg: "#ffffff"
  light-success: "#1a7f37"
  light-error: "#cf222e"
  dark-capture-panel: "#22242a"
  dark-capture-surface: "#1e2025"
  dark-capture-raised: "#2a2d34"
  dark-capture-field: "#16171b"
  dark-capture-hover: "#343841"
  dark-capture-select: "#1e4268"
  dark-capture-edge: "#383c45"
  dark-capture-text: "#eceef2"
  dark-capture-muted: "#949ba6"
  dark-capture-primary: "#fafbfd"
  dark-capture-accent: "#40a0f5"
  dark-capture-accent-solid: "#0e70c0"
  light-capture-panel: "#ffffff"
  light-capture-surface: "#fcfdff"
  light-capture-raised: "#ffffff"
  light-capture-field: "#f3f4f6"
  light-capture-hover: "#e9ecf1"
  light-capture-select: "#dbeafe"
  light-capture-edge: "#d0d5dd"
  light-capture-text: "#16191e"
  light-capture-muted: "#525866"
  light-capture-primary: "#101216"
  light-capture-accent: "#0067c0"
  light-capture-accent-solid: "#0067c0"
typography:
  section:
    fontFamily: Segoe UI
    fontSize: 13.5px
  body:
    fontFamily: Segoe UI
    fontSize: 12.5px
  button:
    fontFamily: Segoe UI
    fontSize: 11.5px
  label:
    fontFamily: Segoe UI
    fontSize: 11.5px
  caption:
    fontFamily: Segoe UI
    fontSize: 11px
  monospace:
    fontFamily: Consolas
    fontSize: 11px
rounded:
  control: 3px
  surface: 4px
  capture-bar: 5px
spacing:
  tight: 3px
  compact: 5px
  item: 6px
  grid: 8px
  bar-x: 10px
  content-x: 12px
  settings-x: 18px
components:
  button-accent-dark:
    backgroundColor: "{colors.dark-accent}"
    textColor: "{colors.dark-accent-fg}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    padding: "4px 9px"
  button-accent-light:
    backgroundColor: "{colors.light-accent}"
    textColor: "{colors.light-accent-fg}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    padding: "4px 9px"
  button-subtle-dark:
    backgroundColor: "{colors.dark-panel}"
    textColor: "{colors.dark-text}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    padding: "4px 9px"
  button-subtle-light:
    backgroundColor: "{colors.light-panel}"
    textColor: "{colors.light-text}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    padding: "4px 9px"
---

# Design System: xscreen

## Overview

**Creative North Star: "Graphite Precision — ShareX-grade Screenshot & Annotation Studio"**

`xscreen` combines an instant Lightshot-fast multi-monitor screen capture overlay with a compact, professional desktop studio (`Album · Editor · Settings`). Rather than wide sidebars or rounded SaaS dashboard cards, the interface uses a single `38px` top command bar, `3px–4px` micro-radius technical controls, `1px` structural hairlines, and a ShareX-inspired two-tier Image Editor with non-destructive object selection, resize handles, crop, and zoom/pan.

All surfaces—main studio window, in-app Image Editor, floating capture overlay, and native Win32 system tray menu—follow the user's selected **Dark / Light** theme and **English / Türkçe** interface language.

### App Icon

Four crop corners on a graphite tile, with the top-right corner in Cobalt Active. `assets/xscreen.svg` is the source artwork; export it to `assets/xscreen.png` (256 px for Studio windows) and `assets/xscreen.ico` (16–256 px for Windows Explorer and the system tray). Keep the transparent margin and check legibility at 16 px.

## Colors

Frontmatter tokens map directly to `Palette` in `src/ui.rs` and `CapturePalette` in `src/app.rs`.

### Primary & Semantic
- **Cobalt Active (`*-accent`, `*-accent-bright`, `*-accent-fg`)**: Armed tool cells, active top-bar tab, toggle switches, keyboard focus rings, and selection frame accents—directly echoing ShareX's unmistakable active-tool clarity.
- **Save / Commit (`*-success`)**: Dedicated emerald fill reserved for destructive-safe commit actions (`Save`, `Apply crop`).
- **Danger (`*-error`)**: File deletion, shape deletion, and inline error banners.

### Neutral Layers
- **Drafting Canvas (`*-canvas`)**: Recessed backdrop behind images in the Editor (paired with a subtle two-tone checkerboard inside image bounds) and inside Album thumbnail wells.
- **Window Ground (`*-bg`)**: Album grid and Settings page background.
- **Command Bar (`*-bar`)**: Top navigation bar, Editor two-tier toolbars, Settings preference groups, and bottom `24px` status bar.
- **Control Surface (`*-panel`, `*-hover`, `*-line`)**: Resting buttons, search/folder fields, and `1px` crisp structural borders.

## Typography

- **Segoe UI** (`C:\Windows\Fonts\segoeui.ttf`) drives all UI labels, tabs, buttons, and bilingual English/Turkish copy.
- **Consolas** (`C:\Windows\Fonts\consola.ttf`) renders pixel dimensions (`1920 × 1080 px`), cursor coordinates `(X, Y)`, hex colors `#RRGGBB`, and zoom percentages (`100%`) with stable tabular alignment.

## Layout & Surfaces

- **Window Geometry**: Default `1020×660` logical px, minimum `680×460` logical px, native Windows title bar and resizable border.
- **Top Command Bar (`38px`)**: Brand mark, segmented `Album · Editor · Settings` tabs, contextual middle slot (filename filter + Refresh/Folder on Album; active filename + modified indicator on Editor), quick `EN/TR` language toggle, `Dark/Light` theme toggle, and `Capture` primary button.
- **Album**: Virtualized adaptive grid (`columns = max(floor((w + 8) / 188), 1)`), box-averaged crisp thumbnails, single-line metadata footer, top-right hover quick actions (`Copy`, `Open`, `Delete`), right-click context menu, and single-click opening directly into the **Editor**.
- **In-App Image Editor**:
  - **Row 1 (`34px`)**: `Select (V)`, `Rect (R)`, `Ellipse (E)`, `Triangle`, `Line (L)`, `Arrow (A)`, `Pen (P)`, `Highlighter (H)`, `Text (T)`, `Step Number (N)`, `Blur/Mosaic (B)`, `Crop (C)`, `Undo/Redo`, `Clear`, `Paste (Ctrl+V)`, and right-aligned `Copy`, `Save`, `Save copy`.
  - **Row 2 (`32px`)**: Contextual property bar adapting dynamically to `Crop` mode, the currently **selected annotation object**, or the active drawing tool (color popover + 8 quick swatches, stroke/font slider, `Outline / Fill` toggle, `Soft / Mosaic / Heavy` blur mode selector, and `Duplicate / To front / Delete` object actions).
  - **Canvas**: Smooth wheel zoom (`10%–800%`) around cursor, middle-drag or `Space+Drag` pan, checkerboard alpha backdrop, ShareX-style dashed selection bounding boxes with white circular resize/endpoint handles, double-click in-place text re-editing, and rule-of-thirds interactive `Crop`.
- **Settings**: Centered `660px` max-width structured preference groups (`General`, `Capture & Shortcut`, `Storage & Clipboard`, `Updates`) with `1px` row dividers and compact `36×20` toggle switches. Updates shows the installed version and offers separate check, download, and restart actions, with inline status and errors.
- **Bottom Status Bar (`24px`)**: Active shortcut & tray status or transient feedback on the left; live cursor `(X, Y)`, image `W × H px`, and `- / 100% / + / Fit` zoom controls on the right when in Editor.

## Do's and Don'ts

### Do:
- **Do** keep controls compact (`26px` height, `3px` radius) with `1px` crisp borders and consistent stroke icons.
- **Do** preserve non-destructive object selection (`Select` tool + right-click any shape) in both the in-app Editor and the live Capture Overlay.
- **Do** keep every user-facing string wired through `src/i18n.rs` (`English` and `Türkçe`).

### Don't:
- **Don't** reintroduce wide sidebars, large marketing empty states, nested SaaS cards, gradient washes, or eyebrow kickers.
- **Don't** flatten annotations prematurely on Crop—translate existing shapes so they remain selectable and undoable.
