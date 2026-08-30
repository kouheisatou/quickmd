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

/// 上の帯の高さ。ここが窓の題名の帯そのものになる。
pub const BAR_H: f32 = 36.0;

/// 窓そのものへの指示。題名の帯を自分で描くので、ここも自分で受け持つ。
pub enum Window {
    Drag,
    ToggleMaximize,
    Minimize,
    Close,
}

/// 窓の上の帯を描く。押された項目を返す。
///
/// OS の題名の帯は消してあり、この帯がその代わりになる。
/// 左に三本線のボタン、その右に開いているファイルの名前、右端に窓のボタンを置く。
pub fn bar(
    ui: &mut egui::Ui,
    dark: bool,
    recent: &[std::path::PathBuf],
    has_doc: bool,
    title: &str,
    maximized: bool,
) -> (Option<Action>, Option<Window>) {
    if !in_window() {
        return (None, None);
    }
    let l = style::look(dark);
    let mut action = None;
    let mut window = None;

    let full = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(full, BAR_H),
        egui::Sense::click_and_drag(),
    );
    ui.painter().rect_filled(rect, 0.0, l.bg_soft);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        egui::Stroke::new(1.0, l.line),
    );

    // 帯の空いているところを引きずると窓が動く。二度押しで最大と元に戻す。
    if resp.drag_started() {
        window = Some(Window::Drag);
    }
    if resp.double_clicked() {
        window = Some(Window::ToggleMaximize);
    }

    // ---- 左：三本線のボタン
    let mut left = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_size(
                rect.min + egui::vec2(6.0, 4.0),
                egui::vec2(rect.width() * 0.7, BAR_H - 8.0),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );

    egui::MenuBar::new().ui(&mut left, |ui| {
        ui.spacing_mut().button_padding = egui::vec2(9.0, 5.0);
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

        if !title.is_empty() {
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(egui::RichText::new(title).size(12.5).color(l.fg))
                    .truncate()
                    .selectable(false),
            );
        }
    });

    // ---- 右：窓のボタン（最小化・最大化・閉じる）
    let mut right = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_max(
                egui::pos2(rect.right() - 140.0, rect.top()),
                rect.max,
            ))
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    right.spacing_mut().item_spacing.x = 0.0;

    if window_button(&mut right, "\u{2715}", l.fg, true).clicked() {
        window = Some(Window::Close);
    }
    let mark = if maximized { "\u{2750}" } else { "\u{25a1}" };
    if window_button(&mut right, mark, l.fg, false).clicked() {
        window = Some(Window::ToggleMaximize);
    }
    if window_button(&mut right, "\u{2500}", l.fg, false).clicked() {
        window = Some(Window::Minimize);
    }

    (action, window)
}

/// 窓のボタン1つ。閉じるだけは、触れたときに赤くする。
fn window_button(
    ui: &mut egui::Ui,
    mark: &str,
    fg: egui::Color32,
    is_close: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(44.0, BAR_H), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hovered = response.hovered();
    let p = ui.painter();
    if hovered {
        let bg = if is_close {
            egui::Color32::from_rgb(232, 17, 35)
        } else {
            egui::Color32::from_gray(128).gamma_multiply(0.25)
        };
        p.rect_filled(rect, 0.0, bg);
    }
    let color = if hovered && is_close {
        egui::Color32::WHITE
    } else {
        fg
    };
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        mark,
        egui::FontId::proportional(12.0),
        color,
    );
    response
}
