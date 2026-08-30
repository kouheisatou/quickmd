//! macOS の画面いちばん上に出すメニューバー。
//!
//! macOS では、メニューはウィンドウの中ではなく画面の上端に置くのが作法である。
//! そこだけ OS の仕組みへ載せ、選ばれた項目は他の環境と同じ `menu::Action` にして返す。

use crate::menu::Action;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct MacMenu {
    /// 押された項目と、それが表す行いの対応
    actions: HashMap<MenuId, Action>,
    /// 最近開いたものの入れ物。中身は開くたびに作り直す。
    recent_menu: Submenu,
    recent_now: Vec<PathBuf>,
    _menu: Menu,
}

fn cmd(code: Code) -> Accelerator {
    Accelerator::new(Some(Modifiers::META), code)
}

fn cmd_shift(code: Code) -> Accelerator {
    Accelerator::new(Some(Modifiers::META | Modifiers::SHIFT), code)
}

impl MacMenu {
    pub fn install() -> Option<Self> {
        let menu = Menu::new();
        let mut actions = HashMap::new();

        // アプリの名前のメニュー。終了は OS の作法どおりの項目を使う。
        let app = Submenu::new("QuickMD", true);
        let settings = MenuItem::new("設定…", true, Some(cmd(Code::Comma)));
        actions.insert(settings.id().clone(), Action::Settings);
        app.append_items(&[
            &settings,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ])
        .ok()?;

        let new_window = MenuItem::new("新しいウィンドウ", true, Some(cmd(Code::KeyN)));
        actions.insert(new_window.id().clone(), Action::NewWindow);
        let open = MenuItem::new("開く…", true, Some(cmd(Code::KeyO)));
        actions.insert(open.id().clone(), Action::OpenFile);

        let recent_menu = Submenu::new("最近開いたもの", true);

        let edit_external = MenuItem::new("編集するアプリで開く", true, Some(cmd(Code::KeyE)));
        actions.insert(edit_external.id().clone(), Action::EditExternal);
        let reveal = MenuItem::new(
            "ファイルのある場所を開く",
            true,
            Some(cmd_shift(Code::KeyR)),
        );
        actions.insert(reveal.id().clone(), Action::RevealInFolder);

        let file = Submenu::new("ファイル", true);
        file.append_items(&[
            &new_window,
            &open,
            &recent_menu,
            &PredefinedMenuItem::separator(),
            &edit_external,
            &reveal,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::close_window(Some("閉じる")),
        ])
        .ok()?;

        let edit = Submenu::new("編集", true);
        edit.append_items(&[
            &PredefinedMenuItem::copy(Some("コピー")),
            &PredefinedMenuItem::select_all(Some("すべて選択")),
        ])
        .ok()?;

        let reload = MenuItem::new("読み直す", true, Some(cmd(Code::KeyR)));
        actions.insert(reload.id().clone(), Action::Reload);
        let view = Submenu::new("表示", true);
        view.append_items(&[&reload]).ok()?;

        menu.append_items(&[&app, &file, &edit, &view]).ok()?;
        menu.init_for_nsapp();

        Some(Self {
            actions,
            recent_menu,
            recent_now: Vec::new(),
            _menu: menu,
        })
    }

    /// 最近開いたものの中身を、いまの一覧に合わせる。
    pub fn sync_recent(&mut self, list: &[PathBuf]) {
        if self.recent_now == list {
            return;
        }
        self.recent_now = list.to_vec();

        while self.recent_menu.remove_at(0).is_some() {}
        self.actions.retain(|_, a| {
            !matches!(a, Action::OpenRecent(_) | Action::ClearRecent)
        });

        if list.is_empty() {
            let empty = MenuItem::new("まだありません", false, None);
            let _ = self.recent_menu.append(&empty);
            return;
        }
        for p in list.iter().take(15) {
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let item = MenuItem::new(name, true, None);
            self.actions
                .insert(item.id().clone(), Action::OpenRecent(p.clone()));
            let _ = self.recent_menu.append(&item);
        }
        let _ = self.recent_menu.append(&PredefinedMenuItem::separator());
        let clear = MenuItem::new("履歴を消す", true, None);
        self.actions.insert(clear.id().clone(), Action::ClearRecent);
        let _ = self.recent_menu.append(&clear);
    }

    /// 押された項目を取り出す。
    pub fn poll(&self) -> Option<Action> {
        while let Ok(event) = muda::MenuEvent::receiver().try_recv() {
            if let Some(a) = self.actions.get(&event.id) {
                return Some(a.clone());
            }
        }
        None
    }
}
