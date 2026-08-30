//! 設定の読み書き。設定画面が触る値はここに集める。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// dark / light / system
    pub theme: String,
    pub font_size: f32,
    pub line_height: f32,
    /// 本文の幅（ピクセル）
    pub content_width: f32,
    /// 編集を開くアプリ。空なら OS の既定のアプリを使う。
    pub editor: String,
    pub enable_math: bool,
    pub enable_mermaid: bool,
    /// CSV の文字コード。auto なら見分ける。
    pub csv_encoding: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "light".into(),
            font_size: 16.0,
            line_height: 1.6,
            content_width: 860.0,
            editor: String::new(),
            enable_math: true,
            enable_mermaid: true,
            csv_encoding: "auto".into(),
        }
    }
}

pub fn dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Ok(v) = std::env::var("APPDATA") {
        return PathBuf::from(v).join("QuickMD");
    }
    #[cfg(target_os = "macos")]
    if let Ok(v) = std::env::var("HOME") {
        return PathBuf::from(v)
            .join("Library")
            .join("Application Support")
            .join("QuickMD");
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(v) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(v).join("QuickMD");
        }
        if let Ok(v) = std::env::var("HOME") {
            return PathBuf::from(v).join(".config").join("QuickMD");
        }
    }
    std::env::temp_dir().join("QuickMD")
}

fn path() -> PathBuf {
    dir().join("settings.json")
}

pub fn load() -> Settings {
    match std::fs::read_to_string(path()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(s: &Settings) {
    let _ = std::fs::create_dir_all(dir());
    if let Ok(text) = serde_json::to_string_pretty(s) {
        let _ = std::fs::write(path(), text);
    }
}
