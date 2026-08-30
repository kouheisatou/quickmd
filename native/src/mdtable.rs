//! マークダウンの表を、自分で組んで描く。
//!
//! 変換器に任せると、列の幅を測る相手が「無限の広さ」になってしまい、
//! 桁がずれたり字が重なったりする。表だけは自分で幅を決め、
//! 本文の幅からはみ出したときだけ、そこを横へ流せるようにする。

use crate::style;

#[derive(Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}

pub struct Table {
    pub header: Vec<String>,
    pub align: Vec<Align>,
    pub rows: Vec<Vec<String>>,
    pub cols: usize,
}

/// `| … |` の並びを読む。表として成り立たないときは None を返す。
pub fn parse(src: &str) -> Option<Table> {
    let lines: Vec<&str> = src.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() < 2 {
        return None;
    }

    let header = split_row(lines[0]);
    let align = parse_delimiter(lines[1])?;
    if header.is_empty() {
        return None;
    }

    let rows: Vec<Vec<String>> = lines[2..].iter().map(|l| split_row(l)).collect();
    let cols = header
        .len()
        .max(align.len())
        .max(rows.iter().map(|r| r.len()).max().unwrap_or(0));

    Some(Table {
        header,
        align,
        rows,
        cols,
    })
}

/// 区切りの行（`|---|:--:|`）から、列ごとの寄せを読む。
fn parse_delimiter(line: &str) -> Option<Vec<Align>> {
    let cells = split_row(line);
    if cells.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(cells.len());
    for c in cells {
        let t = c.trim();
        let body = t.trim_start_matches(':').trim_end_matches(':');
        if body.is_empty() || !body.chars().all(|ch| ch == '-') {
            return None;
        }
        out.push(match (t.starts_with(':'), t.ends_with(':')) {
            (true, true) => Align::Center,
            (false, true) => Align::Right,
            _ => Align::Left,
        });
    }
    Some(out)
}

/// 1行を縦棒で切る。逆斜線で打ち消した縦棒は、文字として扱う。
fn split_row(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);

    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    if next == '|' {
                        cur.push('|');
                    } else {
                        cur.push('\\');
                        cur.push(next);
                    }
                } else {
                    cur.push('\\');
                }
            }
            '|' => cells.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    cells.push(cur);
    cells.into_iter().map(|c| c.trim().to_string()).collect()
}

// ------------------------------------------------------------------ 描くところ

/// 表を描く。本文の幅に収まらないときだけ、そこを横へ流せるようにする。
pub fn draw(ui: &mut egui::Ui, t: &Table, dark: bool, max_width: f32, id: egui::Id) {
    let l = style::look(dark);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let pad = 12.0;

    // 列の幅は中身から決める。長すぎる列は途中で頭打ちにする。
    let widths = measure(ui, t, &font, pad);
    let total: f32 = widths.iter().sum::<f32>() + widths.len() as f32;

    ui.add_space(8.0);
    let show = |ui: &mut egui::Ui| {
        ui.set_min_width(total);
        grid(ui, t, &l, &widths, pad, id);
    };

    if total > max_width {
        egui::ScrollArea::horizontal()
            .id_salt(id)
            .max_width(max_width)
            .auto_shrink([false, true])
            .show(ui, show);
    } else {
        show(ui);
    }
    ui.add_space(8.0);
}

/// 中身の長さから、列ごとの幅を決める。
/// 実際に描くときと同じフォントで測る。コードは等幅なので幅が変わる。
fn measure(ui: &egui::Ui, t: &Table, font: &egui::FontId, pad: f32) -> Vec<f32> {
    /// 1つの列がとってよい上限。これを超えたら、その列の中で折り返す。
    const MAX_COL: f32 = 420.0;
    const MIN_COL: f32 = 48.0;

    let mono = egui::TextStyle::Monospace.resolve(ui.style());
    let mut widths = vec![0.0f32; t.cols];
    let mut rows: Vec<&Vec<String>> = vec![&t.header];
    rows.extend(t.rows.iter());

    for row in rows {
        for (i, cell) in row.iter().enumerate().take(t.cols) {
            let w: f32 = inline(cell)
                .into_iter()
                .map(|part| {
                    let (text, f) = match part {
                        Part::Text { text, code: true, .. } => (text, &mono),
                        Part::Text { text, .. } => (text, font),
                        Part::Link { text, .. } => (text, font),
                    };
                    ui.painter()
                        .layout_no_wrap(text, f.clone(), egui::Color32::WHITE)
                        .rect
                        .width()
                })
                .sum();
            widths[i] = widths[i].max(w);
        }
    }
    widths
        .into_iter()
        .map(|w| (w + pad * 2.0).clamp(MIN_COL, MAX_COL))
        .collect()
}

/// 罫線と中身を描く。
fn grid(
    ui: &mut egui::Ui,
    t: &Table,
    l: &style::Look,
    widths: &[f32],
    pad: f32,
    id: egui::Id,
) {
    let stroke = egui::Stroke::new(1.0, l.line);
    let total: f32 = widths.iter().sum::<f32>() + widths.len() as f32;

    // 外枠は描かない。行の下と列の境だけを引く。
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        ui.set_max_width(total);

        row(ui, t, l, widths, pad, None, stroke, id);
        for (n, r) in t.rows.iter().enumerate() {
            row(ui, t, l, widths, pad, Some((n, r)), stroke, id);
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut egui::Ui,
    t: &Table,
    l: &style::Look,
    widths: &[f32],
    pad: f32,
    body: Option<(usize, &Vec<String>)>,
    stroke: egui::Stroke,
    id: egui::Id,
) {
    let is_header = body.is_none();
    let cells: &Vec<String> = match body {
        Some((_, r)) => r,
        None => &t.header,
    };
    let index = body.map(|(n, _)| n).unwrap_or(0);

    // 1行おきに薄く塗って、横に長い表でも目が迷わないようにする
    let fill = if is_header {
        Some(l.bg_soft)
    } else if index % 2 == 1 {
        Some(l.bg_soft.gamma_multiply(0.5))
    } else {
        None
    };

    // 塗りは中身より先に敷きたいが、大きさは中身を描くまで分からない。
    // そこで場所だけ先に取っておき、あとから中身を入れる。
    let bg = ui.painter().add(egui::Shape::Noop);
    let lines = ui.painter().add(egui::Shape::Noop);

    let response = ui
        .horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            for i in 0..t.cols {
                let w = widths[i];
                let text = cells.get(i).cloned().unwrap_or_default();
                let align = t.align.get(i).copied().unwrap_or(Align::Left);
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: pad as i8,
                        right: pad as i8,
                        top: 7,
                        bottom: 7,
                    })
                    .show(ui, |ui| {
                        ui.set_width(w - pad * 2.0);
                        cell(ui, &text, l, align, is_header, id.with((index, i)));
                    });
            }
        })
        .response;

    let full = egui::Rect::from_min_size(
        response.rect.min,
        egui::vec2(
            widths.iter().sum::<f32>(),
            response.rect.height(),
        ),
    );

    if let Some(c) = fill {
        ui.painter()
            .set(bg, egui::Shape::rect_filled(full, 0.0, c));
    }

    // 行の下の線だけを引く。縦線と外枠は引かない（文章の一部として読むものだからである）。
    let mut shapes: Vec<egui::Shape> = Vec::with_capacity(1);
    let last = !is_header && index + 1 == t.rows.len();
    if !last {
        shapes.push(egui::Shape::line_segment(
            [
                egui::pos2(full.left(), full.bottom()),
                egui::pos2(full.right(), full.bottom()),
            ],
            stroke,
        ));
    }
    ui.painter().set(lines, egui::Shape::Vec(shapes));
}

/// ます目の中身。太字・コード・打ち消し・リンクをその場で組む。
fn cell(
    ui: &mut egui::Ui,
    text: &str,
    l: &style::Look,
    align: Align,
    is_header: bool,
    id: egui::Id,
) {
    let layout = match align {
        Align::Left => egui::Layout::left_to_right(egui::Align::TOP),
        Align::Center => egui::Layout::top_down(egui::Align::Center),
        Align::Right => egui::Layout::top_down(egui::Align::Max),
    };
    let _ = id;

    ui.with_layout(layout.with_main_wrap(true), |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
        if align == Align::Left {
            for part in inline(text) {
                draw_part(ui, part, l, is_header);
            }
        } else {
            // 中央と右へ寄せるときは、いったん横並びにしてから寄せる
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                for part in inline(text) {
                    draw_part(ui, part, l, is_header);
                }
            });
        }
    });
}

pub enum Part {
    Text {
        text: String,
        bold: bool,
        italic: bool,
        code: bool,
        strike: bool,
    },
    Link {
        text: String,
        url: String,
    },
}

fn draw_part(ui: &mut egui::Ui, part: Part, l: &style::Look, is_header: bool) {
    match part {
        Part::Text {
            text,
            bold,
            italic,
            code,
            strike,
        } => {
            let mut rt = egui::RichText::new(text).color(l.fg);
            if bold || is_header {
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
            let rt = egui::RichText::new(text).color(l.accent);
            ui.hyperlink_to(rt, url);
        }
    }
}

/// ます目の文字を、書き方ごとの切れ端に分ける。
pub fn inline(src: &str) -> Vec<Part> {
    let mut out = Vec::new();
    let b: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut plain = String::new();

    let flush = |plain: &mut String, out: &mut Vec<Part>| {
        if !plain.is_empty() {
            out.push(Part::Text {
                text: std::mem::take(plain),
                bold: false,
                italic: false,
                code: false,
                strike: false,
            });
        }
    };

    while i < b.len() {
        // リンク
        if b[i] == '[' {
            if let Some((text, url, next)) = read_link(&b, i) {
                flush(&mut plain, &mut out);
                out.push(Part::Link { text, url });
                i = next;
                continue;
            }
        }
        // コード
        if b[i] == '`' {
            if let Some((text, next)) = read_between(&b, i, '`', 1) {
                flush(&mut plain, &mut out);
                out.push(Part::Text {
                    text,
                    bold: false,
                    italic: false,
                    code: true,
                    strike: false,
                });
                i = next;
                continue;
            }
        }
        // 太字
        if b[i] == '*' && i + 1 < b.len() && b[i + 1] == '*' {
            if let Some((text, next)) = read_between(&b, i, '*', 2) {
                flush(&mut plain, &mut out);
                out.push(Part::Text {
                    text,
                    bold: true,
                    italic: false,
                    code: false,
                    strike: false,
                });
                i = next;
                continue;
            }
        }
        // 打ち消し
        if b[i] == '~' && i + 1 < b.len() && b[i + 1] == '~' {
            if let Some((text, next)) = read_between(&b, i, '~', 2) {
                flush(&mut plain, &mut out);
                out.push(Part::Text {
                    text,
                    bold: false,
                    italic: false,
                    code: false,
                    strike: true,
                });
                i = next;
                continue;
            }
        }
        // 斜体
        if b[i] == '*' {
            if let Some((text, next)) = read_between(&b, i, '*', 1) {
                flush(&mut plain, &mut out);
                out.push(Part::Text {
                    text,
                    bold: false,
                    italic: true,
                    code: false,
                    strike: false,
                });
                i = next;
                continue;
            }
        }
        plain.push(b[i]);
        i += 1;
    }
    flush(&mut plain, &mut out);
    if out.is_empty() {
        out.push(Part::Text {
            text: String::new(),
            bold: false,
            italic: false,
            code: false,
            strike: false,
        });
    }
    out
}

/// 同じ印で挟まれたところを読む。
fn read_between(b: &[char], start: usize, mark: char, n: usize) -> Option<(String, usize)> {
    let open = start + n;
    let mut i = open;
    while i + n <= b.len() {
        if b[i..].iter().take(n).all(|c| *c == mark) && i > open {
            return Some((b[open..i].iter().collect(), i + n));
        }
        i += 1;
    }
    None
}

/// `[文字](行き先)` を読む。
fn read_link(b: &[char], start: usize) -> Option<(String, String, usize)> {
    let mut i = start + 1;
    while i < b.len() && b[i] != ']' {
        i += 1;
    }
    if i + 1 >= b.len() || b[i + 1] != '(' {
        return None;
    }
    let text: String = b[start + 1..i].iter().collect();
    let mut j = i + 2;
    while j < b.len() && b[j] != ')' {
        j += 1;
    }
    if j >= b.len() {
        return None;
    }
    let url: String = b[i + 2..j].iter().collect();
    let url = url.split_whitespace().next().unwrap_or("").to_string();
    Some((text, url, j + 1))
}
