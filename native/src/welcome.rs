//! 何も開いていないときの画面。開くための入口と、最近開いたものを並べる。

use crate::style;
use std::path::PathBuf;

pub enum Pick {
    Markdown,
    Table,
    Recent(PathBuf),
}

pub fn show(ui: &mut egui::Ui, dark: bool, recent: &[PathBuf]) -> Option<Pick> {
    let l = style::look(dark);
    let mut pick = None;

    let avail = ui.available_width();
    let inner: f32 = 460.0;
    let pad = ((avail - inner) / 2.0).max(24.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: pad as i8,
                    right: pad as i8,
                    top: 64,
                    bottom: 48,
                })
                .show(ui, |ui| {

                    ui.label(egui::RichText::new("QuickMD").size(30.0).strong());
                    ui.add_space(4.0);
                    ui.colored_label(
                        l.fg_dim,
                        "マークダウンと CSV を、読むためだけに開きます。",
                    );

                    ui.add_space(28.0);
                    if big_button(ui, &l, "マークダウンを開く", ".md .markdown .mdx").clicked() {
                        pick = Some(Pick::Markdown);
                    }
                    ui.add_space(10.0);
                    if big_button(ui, &l, "CSV を開く", ".csv .tsv").clicked() {
                        pick = Some(Pick::Table);
                    }

                    ui.add_space(36.0);
                    ui.colored_label(
                        l.fg_dim,
                        egui::RichText::new("最近開いたもの").size(12.0),
                    );
                    ui.add_space(8.0);

                    if recent.is_empty() {
                        ui.colored_label(
                            l.fg_dim,
                            "まだありません。上のボタンから開くか、\nこの窓へファイルを落としてください。",
                        );
                    } else {
                        for p in recent.iter().take(12) {
                            if recent_row(ui, &l, p).clicked() {
                                pick = Some(Pick::Recent(p.clone()));
                            }
                        }
                    }
                });
        });

    pick
}

fn big_button(
    ui: &mut egui::Ui,
    l: &style::Look,
    title: &str,
    sub: &str,
) -> egui::Response {
    let h = 58.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let fill = if response.hovered() {
        l.bg_soft
    } else {
        l.bg
    };
    let stroke = if response.hovered() { l.accent } else { l.line };
    let p = ui.painter();
    p.rect_filled(rect, 8.0, fill);
    p.rect_stroke(rect, 8.0, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    p.text(
        egui::pos2(rect.left() + 20.0, rect.center().y - 9.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(15.0),
        l.fg,
    );
    p.text(
        egui::pos2(rect.left() + 20.0, rect.center().y + 11.0),
        egui::Align2::LEFT_CENTER,
        sub,
        egui::FontId::proportional(11.5),
        l.fg_dim,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn recent_row(ui: &mut egui::Ui, l: &style::Look, path: &std::path::Path) -> egui::Response {
    let h = 40.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, l.bg_soft);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let dir = path
        .parent()
        .map(|s| shorten(&s.to_string_lossy()))
        .unwrap_or_default();

    let p = ui.painter();
    p.text(
        egui::pos2(rect.left() + 10.0, rect.center().y - 8.0),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(13.5),
        l.fg,
    );
    p.text(
        egui::pos2(rect.left() + 10.0, rect.center().y + 9.0),
        egui::Align2::LEFT_CENTER,
        dir,
        egui::FontId::proportional(11.0),
        l.fg_dim,
    );
    response
}

/// 長い場所は、家のしるしで縮めて出す。
fn shorten(path: &str) -> String {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let s = if !home.is_empty() && path.starts_with(&home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    };
    if s.chars().count() > 64 {
        let tail: String = s.chars().rev().take(60).collect::<Vec<_>>().into_iter().rev().collect();
        format!("…{tail}")
    } else {
        s
    }
}
