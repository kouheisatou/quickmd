//! 箇条書きと番号つきの並びを、自分で組んで描く。
//!
//! 変換器に任せると、丸や番号を「文字と同じ高さの枠」に描いたうえで、
//! その枠と本文の枠の高さが揃わず、縦の位置がずれる。
//! ここでは印と本文をひとつの行として並べ、どちらも同じ基準線に載せる。

use crate::mdtable::{inline, Part};
use crate::style;

pub struct Item {
    /// 元のファイルの何行目か。印の切り替えを書き戻すために使う。
    pub line: usize,
    /// 何段目か（0 から数える）
    pub depth: usize,
    /// 番号つきならその数。丸なら None。
    pub number: Option<u64>,
    /// チェックボックスの状態。無ければ None。
    pub checked: Option<bool>,
    /// 本文
    pub text: String,
}

pub struct List {
    pub items: Vec<Item>,
}

/// 行の頭が箇条書きの印で始まっているか。
pub fn is_item(line: &str) -> bool {
    head(line).is_some()
}

/// 印を読み、（段, 番号, 印, 本文）に分ける。
fn head(line: &str) -> Option<(usize, Option<u64>, Option<bool>, String)> {
    let indent = line.len() - line.trim_start().len();
    let t = line.trim_start();
    if t.is_empty() {
        return None;
    }

    // 丸の印
    let rest = if let Some(r) = t
        .strip_prefix("- ")
        .or_else(|| t.strip_prefix("* "))
        .or_else(|| t.strip_prefix("+ "))
    {
        return Some((indent / 2, None, task_mark(r), strip_task(r)));
    } else {
        t
    };

    // 番号の印
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let after = &rest[digits.len()..];
    let body = after
        .strip_prefix(". ")
        .or_else(|| after.strip_prefix(") "))?;
    let n: u64 = digits.parse().ok()?;
    Some((indent / 2, Some(n), task_mark(body), strip_task(body)))
}

fn task_mark(s: &str) -> Option<bool> {
    let t = s.trim_start();
    if t.starts_with("[ ] ") || t == "[ ]" {
        Some(false)
    } else if t.starts_with("[x] ") || t.starts_with("[X] ") || t == "[x]" || t == "[X]" {
        Some(true)
    } else {
        None
    }
}

fn strip_task(s: &str) -> String {
    let t = s.trim_start();
    for p in ["[ ] ", "[x] ", "[X] "] {
        if let Some(r) = t.strip_prefix(p) {
            return r.trim_end().to_string();
        }
    }
    if t == "[ ]" || t == "[x]" || t == "[X]" {
        return String::new();
    }
    t.trim_end().to_string()
}

/// 続いた行をひとまとまりの並びとして読む。
/// `start_line` は、この並びの1行目が元のファイルの何行目かである。
pub fn parse(src: &str, start_line: usize) -> Option<List> {
    let mut items = Vec::new();
    for (n, line) in src.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (depth, number, checked, text) = head(line)?;
        items.push(Item {
            line: start_line + n,
            depth,
            number,
            checked,
            text,
        });
    }
    (!items.is_empty()).then_some(List { items })
}

/// 印を押したことを、元のファイルへ書き戻す。
pub fn toggle_in_file(path: &std::path::Path, line_no: usize, now: bool) -> std::io::Result<()> {
    let text = std::fs::read_to_string(path)?;
    let ends_with_newline = text.ends_with('\n');
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let Some(line) = lines.get_mut(line_no) else {
        return Ok(());
    };

    // 行の中の `[ ]` と `[x]` を入れ替える。ほかは1文字も触らない。
    let (from, to) = if now { ("[x]", "[ ]") } else { ("[ ]", "[x]") };
    if let Some(at) = line.find(from).or_else(|| {
        if now {
            line.find("[X]")
        } else {
            None
        }
    }) {
        line.replace_range(at..at + 3, to);
    } else {
        return Ok(());
    }

    let mut out = lines.join("\n");
    if ends_with_newline {
        out.push('\n');
    }
    std::fs::write(path, out)
}

/// 段ごとの下げ幅。
const INDENT: f32 = 24.0;
/// 印を置く欄の幅。
const MARK_W: f32 = 22.0;

/// 描いた結果。印を押されたら、その行と新しい状態を返す。
pub struct Toggled {
    pub line: usize,
    pub now: bool,
}

pub fn draw(ui: &mut egui::Ui, list: &List, dark: bool) -> Option<Toggled> {
    let l = style::look(dark);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let line_h = ui.text_style_height(&egui::TextStyle::Body);
    let mut toggled = None;

    ui.add_space(6.0);
    for item in &list.items {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.add_space(item.depth as f32 * INDENT);

            // 印の欄。本文の1行目と同じ高さを取り、その中で縦の真ん中に置く。
            let sense = if item.checked.is_some() {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            };
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(MARK_W, line_h), sense);
            let mid = egui::pos2(rect.right() - 7.0, rect.center().y);

            // チェックボックスのときは、点を出さずにチェックボックスだけを出す
            match (item.checked, item.number) {
                (Some(done), _) => {
                    draw_check(ui.painter(), rect, done, &l, resp.hovered());
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        toggled = Some(Toggled {
                            line: item.line,
                            now: !done,
                        });
                    }
                }
                (None, Some(n)) => {
                    ui.painter().text(
                        egui::pos2(rect.right() - 2.0, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        format!("{n}."),
                        font.clone(),
                        l.fg,
                    );
                }
                (None, None) => {
                    // 段が深いほど小さくする。中抜きの丸は、線のぶん大きめに描かないと
                    // 文字の「o」に見えてしまう。
                    let r = (line_h * 0.16).clamp(3.0, 4.5);
                    if item.depth % 2 == 0 {
                        ui.painter().circle_filled(mid, r, l.fg);
                    } else {
                        ui.painter().circle_stroke(
                            mid,
                            r * 0.95,
                            egui::Stroke::new(1.6, l.fg),
                        );
                    }
                }
            }

            ui.add_space(8.0);
            // 本文。印と同じ行に載せる。
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                    for part in inline(&item.text) {
                        match part {
                            Part::Text {
                                text,
                                bold,
                                italic,
                                code,
                                strike,
                            } => {
                                let mut rt = egui::RichText::new(text).color(l.fg);
                                if bold {
                                    rt = rt.strong();
                                }
                                if italic {
                                    rt = rt.italics();
                                }
                                if strike {
                                    rt = rt.strikethrough();
                                }
                                if code {
                                    rt = rt.monospace().background_color(l.bg_soft);
                                }
                                ui.label(rt);
                            }
                            Part::Link { text, url } => {
                                ui.hyperlink_to(
                                    egui::RichText::new(text).color(l.accent),
                                    url,
                                );
                            }
                        }
                    }
                });
            });
        });
        ui.add_space(2.0);
    }
    ui.add_space(6.0);
    toggled
}

/// チェックボックスを描く。
fn draw_check(
    p: &egui::Painter,
    rect: egui::Rect,
    done: bool,
    l: &style::Look,
    hovered: bool,
) {
    let s = (rect.height() * 0.62).clamp(11.0, 15.0);
    let box_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 8.0, rect.center().y),
        egui::vec2(s, s),
    );
    if done {
        let c = if hovered { l.accent.gamma_multiply(0.8) } else { l.accent };
        p.rect_filled(box_rect, 3.0, c);
        let ctr = box_rect.center();
        let w = s * 0.28;
        p.add(egui::Shape::line(
            vec![
                egui::pos2(ctr.x - w, ctr.y),
                egui::pos2(ctr.x - w * 0.15, ctr.y + w * 0.8),
                egui::pos2(ctr.x + w, ctr.y - w * 0.7),
            ],
            egui::Stroke::new(1.8, egui::Color32::WHITE),
        ));
    } else {
        p.rect_stroke(
            box_rect,
            3.0,
            egui::Stroke::new(1.3, if hovered { l.accent } else { l.fg_dim }),
            egui::StrokeKind::Inside,
        );
    }
}
