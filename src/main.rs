#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "windows"))]
compile_error!("xscreen currently supports Windows 10/11 only.");

mod album;
mod app;
mod capture;
mod config;
mod edit;
mod i18n;
mod native;
mod startup;
mod studio;
mod update;

fn main() {
    if let Some(success) = update::helper_mode() {
        std::process::exit(i32::from(!success));
    }
    // Pencere veya GDI çağrısından önce; winit'in varsayılanına bağlı kalma.
    unsafe {
        use windows::Win32::UI::HiDpi::*;
        if let Err(error) =
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
            && !AreDpiAwarenessContextsEqual(
                GetThreadDpiAwarenessContext(),
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            )
            .as_bool()
        {
            eprintln!("Could not enable per-monitor DPI v2: {error}");
            return;
        }
    }
    install_panic_hook();
    // Hata günlüğü dışında ekran görüntüsü veya kullanıcı verisi yazılmaz.
    let log = std::env::var_os("APPDATA").and_then(|dir| {
        let dir = std::path::PathBuf::from(dir).join("xscreen");
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("error.log"))
            .ok()
    });
    if let Some(log) = log {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::ERROR)
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(log))
            .try_init();
    }
    let (updates, pending_install) = std::sync::mpsc::channel();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("xscreen")
            .with_position([-32000.0, -32000.0])
            .with_inner_size([1.0, 1.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_visible(false)
            .with_taskbar(false)
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/xscreen.png"))
                    .expect("embedded xscreen icon must be a valid PNG"),
            ),
        ..Default::default()
    };
    if let Err(error) = eframe::run_native(
        "xscreen",
        options,
        Box::new(move |cc| {
            let app = app::App::new(cc, updates)?;
            update::cleanup_after_start();
            Ok(Box::new(app))
        }),
    ) {
        tracing::error!(%error, "app failed to start");
        eprintln!("xscreen: {error}");
    }
    if let Ok((staged, lang)) = pending_install.try_recv() {
        update::install_after_exit(staged, lang);
    }
}

fn crash_text(prior: &str, line: &str) -> String {
    let mut lines: Vec<&str> = prior.lines().filter(|l| !l.is_empty()).collect();
    lines.push(line.trim_end());
    if lines.len() > 20 {
        lines = lines.split_off(lines.len() - 20);
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Son 20 çökme. Rapor göndermez.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("panic");
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "?".into());
        if let Some(dir) = std::env::var_os("APPDATA") {
            let path = std::path::PathBuf::from(dir)
                .join("xscreen")
                .join("crash.log");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let line = format!(
                "{} {loc} {payload}\n",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            );
            let prior = std::fs::read_to_string(&path).unwrap_or_default();
            let _ = std::fs::write(&path, crash_text(&prior, &line));
        }
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::crash_text;

    #[test]
    fn crash_log_keeps_last_20() {
        let prior = (1..=20)
            .map(|n| format!("{n} a"))
            .collect::<Vec<_>>()
            .join("\n");
        let text = crash_text(&prior, "21 boom");
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 20);
        assert_eq!(lines[0], "2 a");
        assert_eq!(lines[19], "21 boom");
    }
}
