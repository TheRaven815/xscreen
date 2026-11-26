use crate::{i18n::Lang, native::Hotkey};
use std::path::{Path, PathBuf};

/// Kalıcı ayar. `%APPDATA%/xscreen/config.json`. Bozuksa varsayılan, eski dosya `.bak`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub hotkey: Hotkey,
    /// Empty: `Pictures/xscreen`.
    pub save_dir: String,
    pub copy_png: bool,
    pub include_cursor: bool,
    pub show_magnifier: bool,
    pub run_at_startup: bool,
    pub copy_and_save: bool,
    /// İlk tray bildirimi gösterildi. Bir kez.
    pub greeted: bool,
    pub light_theme: bool,
    pub lang: Lang,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: Hotkey::default(),
            save_dir: String::new(),
            copy_png: true,
            include_cursor: false,
            show_magnifier: false,
            run_at_startup: false,
            copy_and_save: false,
            greeted: false,
            light_theme: false,
            lang: Lang::En,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match parse(&text) {
            Some(config) => config,
            None => {
                let _ = std::fs::rename(&path, path.with_extension("json.bak"));
                Self::default()
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path().ok_or_else(|| std::io::Error::other("APPDATA yok"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        atomic_write(&path, &self.to_json())
    }

    pub fn save_folder(&self) -> Option<PathBuf> {
        if self.save_dir.is_empty() {
            return crate::native::pictures_dir();
        }
        Some(PathBuf::from(&self.save_dir))
    }

    fn to_json(&self) -> String {
        format!(
            "{{\n  \"hotkey_modifiers\": {},\n  \"hotkey_key\": {},\n  \"save_dir\": \"{}\",\n  \"copy_png\": {},\n  \"include_cursor\": {},\n  \"show_magnifier\": {},\n  \"run_at_startup\": {},\n  \"copy_and_save\": {},\n  \"greeted\": {},\n  \"light_theme\": {},\n  \"lang\": \"{}\"\n}}\n",
            self.hotkey.modifiers,
            self.hotkey.key,
            json_escape(&self.save_dir),
            bool_json(self.copy_png),
            bool_json(self.include_cursor),
            bool_json(self.show_magnifier),
            bool_json(self.run_at_startup),
            bool_json(self.copy_and_save),
            bool_json(self.greeted),
            bool_json(self.light_theme),
            self.lang.code(),
        )
    }
}

pub fn config_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("xscreen").join("config.json"))
}

fn atomic_write(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("json.part");
    std::fs::write(&tmp, text)?;
    if std::fs::rename(&tmp, path).is_err() {
        // Windows: hedef varsa rename olmaz.
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp, path)?;
    }
    Ok(())
}

fn bool_json(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

fn parse(text: &str) -> Option<Config> {
    let mut config = Config::default();
    let mut saw_key = false;
    for (key, value) in fields(text)? {
        match key {
            "hotkey_modifiers" => config.hotkey.modifiers = value.parse().ok()?,
            "hotkey_key" => {
                config.hotkey.key = value.parse().ok()?;
                saw_key = true;
            }
            "save_dir" => config.save_dir = unescape(value)?,
            "copy_png" => config.copy_png = parse_bool(value)?,
            "include_cursor" => config.include_cursor = parse_bool(value)?,
            "show_magnifier" => config.show_magnifier = parse_bool(value)?,
            "run_at_startup" => config.run_at_startup = parse_bool(value)?,
            "english" => {
                if let Some(en) = parse_bool(value) {
                    config.lang = if en { Lang::En } else { Lang::Tr };
                }
            }
            "lang" => config.lang = Lang::from_code(&unescape(value)?),
            "copy_and_save" => config.copy_and_save = parse_bool(value)?,
            "greeted" => config.greeted = parse_bool(value)?,
            "light_theme" => config.light_theme = parse_bool(value)?,
            _ => {}
        }
    }
    if !saw_key || !known_key(config.hotkey.key) {
        return None;
    }
    Some(config)
}

fn known_key(key: u32) -> bool {
    key == 0x2c || (0x70..=0x7b).contains(&key)
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Düz nesne. İç içe yok. Bilinmeyen anahtar atlanır.
fn fields(text: &str) -> Option<Vec<(&str, &str)>> {
    let text = text.trim_start_matches('\u{feff}').trim();
    let text = text.strip_prefix('{')?.strip_suffix('}')?;
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\n' | b'\r' | b'\t' | b',') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] != b'"' {
            return None;
        }
        let (key, next) = quoted(text, i)?;
        i = next;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\n' | b'\r' | b'\t') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b':' {
            return None;
        }
        i += 1;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\n' | b'\r' | b'\t') {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        if bytes[i] == b'"' {
            let (value, next) = quoted(text, i)?;
            out.push((key, value));
            i = next;
        } else {
            let start = i;
            while i < bytes.len() && !matches!(bytes[i], b',' | b' ' | b'\n' | b'\r' | b'\t') {
                i += 1;
            }
            out.push((key, &text[start..i]));
        }
    }
    Some(out)
}

fn quoted(text: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = text.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some((&text[start + 1..i], i + 1)),
            _ => i += 1,
        }
    }
    None
}

fn unescape(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next()? {
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            other => out.push(other),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_config_defaults_to_dark_theme_and_english() {
        let config = parse(r#"{"hotkey_key":44,"save_dir":"","copy_png":true}"#).unwrap();
        assert!(!config.light_theme);
        assert!(!config.show_magnifier);
        assert_eq!(config.lang, Lang::En);
        assert!(config.copy_png);
        assert_eq!(config.hotkey.key, 44);
    }

    #[test]
    fn roundtrip_keeps_fields_and_rejects_garbage() {
        let config = Config {
            hotkey: Hotkey {
                modifiers: 2,
                key: 0x70,
            },
            save_dir: r"C:\Resimler\x".into(),
            copy_png: false,
            include_cursor: true,
            show_magnifier: true,
            run_at_startup: true,
            copy_and_save: true,
            greeted: true,
            light_theme: true,
            lang: Lang::Tr,
        };
        let parsed = parse(&config.to_json()).unwrap();
        assert_eq!(parsed, config);
        assert!(parse("{").is_none());
        assert!(parse(r#"{ "hotkey_key": 1 }"#).is_none());
        let dir = std::env::temp_dir().join(format!("xscreen-cfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        atomic_write(&path, &config.to_json()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), config.to_json());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
