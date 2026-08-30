//! 上の帯のメニュー。
//!
//! macOS では画面いちばん上のメニューバーがそれにあたるので、帯は出さない。
//! Windows と Linux では、ウィンドウの中に自分で帯を描く。

use crate::style;

/// メニューから選ばれた行い。
#[derive(Clone, PartialEq)]
pub enum Action {
    NewWindow,
    OpenFile,
    OpenRecent(std::path::PathBuf),
    ClearRecent,
    Settings,
    Reload,
    EditExternal,
    RevealInFolder,
    Close,
}

/// この環境でウィンドウの中に帯を描くか。
///
/// macOS では画面いちばん上のメニューバーへ載せるので、帯は描かない。
/// ただし `QUICKMD_INLINE_MENU` を付けて起動したときは、確かめるために描く。
pub fn in_window() -> bool {
    if std::env::var_os("QUICKMD_INLINE_MENU").is_some() {
        return true;
    }
    !cfg!(target_os = "macos")
}

/// 帯の高さ。描かない環境では 0 になる。
pub fn height() -> f32 {
    if in_window() { 30.0 } else { 0.0 }
}

/// 上の帯を描く。押された項目を返す。
///
/// 三本線のボタンを1つだけ置き、押すと中の項目がぶら下がる。
/// ブラウザや今どきのアプリが取っている形に合わせている。
pub fn bar(
    ui: &mut egui::Ui,
    dark: bool,
    recent: &[std::path::PathBuf],
    has_doc: bool,
    title: &str,
) -> Option<Action> {
    if !in_window() {
        return None;
    }
    let l = style::look(dark);
    let mut action = None;

    let full = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(full, height()), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, l.bg_soft);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        egui::Stroke::new(1.0, l.line),
    );

    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(6.0, 3.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );

    egui::MenuBar::new().ui(&mut child, |ui| {
        ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
        ui.visuals_mut().widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        ui.visuals_mut().widgets.noninteractive.bg_stroke = egui::Stroke::NONE;

        ui.menu_button(egui::RichText::new("\u{2630}").size(15.0), |ui| {
            ui.set_min_width(190.0);

            ui.menu_button("ファイル", |ui| {
                if ui.button("新しいウィンドウ").clicked() {
                    action = Some(Action::NewWindow);
                    ui.close();
                }
                if ui.button("開く…").clicked() {
                    action = Some(Action::OpenFile);
                    ui.close();
                }
                ui.menu_button("最近開いたもの", |ui| {
                    if recent.is_empty() {
                        ui.add_enabled(false, egui::Button::new("まだありません"));
                    } else {
                        for p in recent.iter().take(15) {
                            let name = p
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if ui.button(name).on_hover_text(p.to_string_lossy()).clicked() {
                                action = Some(Action::OpenRecent(p.clone()));
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button("履歴を消す").clicked() {
                            action = Some(Action::ClearRecent);
                            ui.close();
                        }
                    }
                });
                ui.separator();
                if ui
                    .add_enabled(has_doc, egui::Button::new("閉じる"))
                    .clicked()
                {
                    action = Some(Action::Close);
                    ui.close();
                }
            });

            ui.menu_button("編集", |ui| {
                if ui
                    .add_enabled(has_doc, egui::Button::new("編集するアプリで開く"))
                    .clicked()
                {
                    action = Some(Action::EditExternal);
                    ui.close();
                }
                if ui
                    .add_enabled(has_doc, egui::Button::new("ファイルのある場所を開く"))
                    .clicked()
                {
                    action = Some(Action::RevealInFolder);
                    ui.close();
                }
            });

            ui.menu_button("表示", |ui| {
                if ui
                    .add_enabled(has_doc, egui::Button::new("読み直す"))
                    .clicked()
                {
                    action = Some(Action::Reload);
                    ui.close();
                }
                ui.separator();
                if ui.button("設定…").clicked() {
                    action = Some(Action::Settings);
                    ui.close();
                }
            });
        });

        // 開いているファイルの名前を、三本線の右に添える
        if !title.is_empty() {
            ui.add_space(6.0);
            ui.colored_label(l.fg_dim, egui::RichText::new(title).size(12.5));
        }
    });

    action
}
