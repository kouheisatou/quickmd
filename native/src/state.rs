//! 前に開いたときの読みかけの位置を覚えておく。
//! 同じファイルを開き直したとき、続きから読めるようにするためである。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

const MAX_ENTRIES: usize = 500;

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct Positions {
    /// ファイルのパス → 巻き取った量と、そのときの更新時刻
    #[serde(default)]
    entries: HashMap<String, Entry>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Entry {
    pub scroll: f32,
    pub at: u64,
}

fn path() -> PathBuf {
    crate::settings::dir().join("positions.json")
}

pub fn load() -> Positions {
    match std::fs::read_to_string(path()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Positions::default(),
    }
}

impl Positions {
    pub fn get(&self, file: &Path) -> Option<f32> {
        self.entries
            .get(&file.to_string_lossy().to_string())
            .map(|e| e.scroll)
    }

    pub fn set(&mut self, file: &Path, scroll: f32) {
        if file.as_os_str().is_empty() {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.entries.insert(
            file.to_string_lossy().to_string(),
            Entry { scroll, at: now },
        );

        // 際限なく増やさない。古いものから落とす。
        if self.entries.len() > MAX_ENTRIES {
            let mut all: Vec<(String, u64)> =
                self.entries.iter().map(|(k, v)| (k.clone(), v.at)).collect();
            all.sort_by_key(|(_, at)| *at);
            for (k, _) in all.into_iter().take(self.entries.len() - MAX_ENTRIES) {
                self.entries.remove(&k);
            }
        }
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(crate::settings::dir());
        if let Ok(text) = serde_json::to_string(self) {
            let _ = std::fs::write(path(), text);
        }
    }
}
