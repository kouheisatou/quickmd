//! 最近開いたファイルの控え。開始の画面とメニューから使う。

use std::path::{Path, PathBuf};

const MAX: usize = 20;

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct Recent {
    #[serde(default)]
    files: Vec<String>,
}

fn path() -> PathBuf {
    crate::settings::dir().join("recent.json")
}

pub fn load() -> Recent {
    match std::fs::read_to_string(path()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Recent::default(),
    }
}

impl Recent {
    /// 新しい順に並んだ一覧。今も在るものだけを返す。
    pub fn list(&self) -> Vec<PathBuf> {
        self.files
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .collect()
    }

    pub fn push(&mut self, file: &Path) {
        if file.as_os_str().is_empty() {
            return;
        }
        let s = file.to_string_lossy().to_string();
        self.files.retain(|f| *f != s);
        self.files.insert(0, s);
        self.files.truncate(MAX);
        self.save();
    }

    pub fn clear(&mut self) {
        self.files.clear();
        self.save();
    }

    fn save(&self) {
        let _ = std::fs::create_dir_all(crate::settings::dir());
        if let Ok(text) = serde_json::to_string(self) {
            let _ = std::fs::write(path(), text);
        }
    }
}
