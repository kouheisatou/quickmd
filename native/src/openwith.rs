//! ファイルを別のアプリへ渡す。編集はこのアプリでは行わない。

use std::path::Path;
use std::process::Command;

pub fn edit(path: &Path, editor: &str) {
    if path.as_os_str().is_empty() {
        return;
    }
    if editor.trim().is_empty() {
        open_default(path);
    } else {
        let _ = Command::new(editor.trim()).arg(path).spawn();
    }
}

pub fn reveal(path: &Path) {
    let target = path.parent().unwrap_or(path);
    open_default(target);
}

fn open_default(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open").arg(path).spawn();
}
