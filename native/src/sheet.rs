//! CSV を、簡単な表計算のように見せる画面。
//!
//! マークダウンの表（`mdtable`）とは作りが違う。あちらは文章の一部として読むもの、
//! こちらは中身を選んで写したり、並べ替えたりして「使う」ものである。
//! そのため縦線を引き、行の番号と列の見出しを常に出し、範囲で選べるようにしている。

use crate::style;
use crate::table::Table;

/// 並べ替えの向き。
#[derive(Clone, Copy, PartialEq)]
pub enum Order {
    Asc,
    Desc,
}

pub struct Sheet {
    /// 1行目を見出しとして扱うか
    pub header_row: bool,
    /// 並べ替えの基準にしている列と、その向き
    pub sort: Option<(usize, Order)>,
    /// 選び始めたます目（行, 列）
    anchor: Option<(usize, usize)>,
    /// 選び終わったます目
    focus: Option<(usize, usize)>,
    /// 引きずっている最中か
    dragging: bool,
    /// 並べ替えたあとの行の並び。中身は元の行の番号。
    order: Vec<usize>,
    /// この並びを作ったときの条件
    order_key: (bool, Option<(usize, Order)>),
}

impl Default for Sheet {
    fn default() -> Self {
        Self {
            header_row: true,
            sort: None,
            anchor: None,
            focus: None,
            dragging: false,
            order: Vec::new(),
            order_key: (true, None),
        }
    }
}

/// 列の幅。中身から決めるが、上限と下限を設ける。
const MIN_COL: f32 = 64.0;
const MAX_COL: f32 = 360.0;
const NUM_COL: f32 = 52.0;

impl Sheet {
    /// 見出しの行と、中身の行を取り出す。
    fn parts<'a>(&self, t: &'a Table) -> (Option<&'a Vec<String>>, Vec<&'a Vec<String>>) {
        if self.header_row {
            (Some(&t.header), t.rows.iter().collect())
        } else {
            let mut all: Vec<&Vec<String>> = vec![&t.header];
            all.extend(t.rows.iter());
            (None, all)
        }
    }

    /// 並べ替えた行の順番を、必要なら作り直す。
    fn ensure_order(&mut self, t: &Table) {
        let key = (self.header_row, self.sort);
        let (_, rows) = self.parts(t);
        if self.order_key == key && self.order.len() == rows.len() {
            return;
        }
        self.order_key = key;
        self.order = (0..rows.len()).collect();

        let Some((col, dir)) = self.sort else {
            return;
        };
        self.order.sort_by(|a, b| {
            let x = rows[*a].get(col).map(String::as_str).unwrap_or("");
            let y = rows[*b].get(col).map(String::as_str).unwrap_or("");
            let ord = compare(x, y);
            if dir == Order::Asc { ord } else { ord.reverse() }
        });
    }

    /// 選んでいる範囲。
    fn range(&self) -> Option<(usize, usize, usize, usize)> {
        let (ar, ac) = self.anchor?;
        let (fr, fc) = self.focus?;
        Some((ar.min(fr), ar.max(fr), ac.min(fc), ac.max(fc)))
    }

    /// 選んでいるところを、表計算へ貼れる形にする。
    fn selected_text(&self, t: &Table) -> Option<String> {
        let (r0, r1, c0, c1) = self.range()?;
        let (_, rows) = self.parts(t);
        let mut out = String::new();
        for r in r0..=r1 {
            let Some(i) = self.order.get(r) else { continue };
            let Some(row) = rows.get(*i) else { continue };
            let line: Vec<&str> = (c0..=c1)
                .map(|c| row.get(c).map(String::as_str).unwrap_or(""))
                .collect();
            out.push_str(&line.join("\t"));
            out.push('\n');
        }
        Some(out)
    }

    pub fn show(&mut self, ui: &mut egui::Ui, t: &Table, dark: bool) {
        self.ensure_order(t);
        let l = style::look(dark);
        let font = egui::TextStyle::Body.resolve(ui.style());
        let widths = self.measure(ui, t, &font);
        let row_h = ui.text_style_height(&egui::TextStyle::Body) + 10.0;

        self.toolbar(ui, &l, t);

        let (header, rows) = self.parts(t);
        let order = self.order.clone();
        let total: f32 = NUM_COL + widths.iter().sum::<f32>();

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show_rows(ui, row_h, order.len(), |ui, range| {
                ui.set_min_width(total);
                // 見出しの行は、巻き取っても上に残す
                if let Some(h) = header {
                    self.header_line(ui, &l, h, &widths, row_h, t.cols);
                }
                for r in range {
                    let Some(i) = order.get(r) else { continue };
                    let Some(row) = rows.get(*i) else { continue };
                    self.data_line(ui, &l, row, &widths, row_h, t.cols, r);
                }
            });

        // 写す。egui は Cmd/Ctrl + C を「写す」という知らせに変えて渡してくる。
        let asked = ui.input(|i| {
            i.events.iter().any(|e| matches!(e, egui::Event::Copy))
                || (i.modifiers.command && i.key_pressed(egui::Key::C))
        });
        if asked {
            if let Some(text) = self.selected_text(t) {
                ui.ctx().copy_text(text);
            }
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.colored_label(l.fg_dim, t.summary());
            if let Some((r0, r1, c0, c1)) = self.range() {
                ui.colored_label(
                    l.fg_dim,
                    format!("／選んでいる範囲 {} 行 × {} 列", r1 - r0 + 1, c1 - c0 + 1),
                );
            }
        });
        if t.truncated {
            ui.colored_label(l.fg_dim, "行が多いため、先頭だけを表示しています。");
        }
    }

    // ------------------------------------------------------------ 上の操作の帯

    fn toolbar(&mut self, ui: &mut egui::Ui, l: &style::Look, t: &Table) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.header_row, "1行目を見出しにする");
            ui.add_space(12.0);
            if self.sort.is_some() && ui.button("並び順を戻す").clicked() {
                self.sort = None;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(self.range().is_some(), egui::Button::new("選んだところを写す"))
                    .clicked()
                {
                    if let Some(text) = self.selected_text(t) {
                        ui.ctx().copy_text(text);
                    }
                }
                if ui.button("すべて選ぶ").clicked() {
                    let (_, rows) = self.parts(t);
                    self.anchor = Some((0, 0));
                    self.focus = Some((rows.len().saturating_sub(1), t.cols.saturating_sub(1)));
                }
            });
        });
        ui.add_space(6.0);
        let _ = l;
    }

    // ------------------------------------------------------------ 見出しの行

    fn header_line(
        &mut self,
        ui: &mut egui::Ui,
        l: &style::Look,
        header: &[String],
        widths: &[f32],
        row_h: f32,
        cols: usize,
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.set_min_height(row_h);

            self.cell_frame(ui, NUM_COL, row_h, l.bg_soft, l, |ui| {
                let _ = ui;
            });

            for c in 0..cols {
                let name = header.get(c).cloned().unwrap_or_default();
                let arrow = match self.sort {
                    Some((i, Order::Asc)) if i == c => " ▲",
                    Some((i, Order::Desc)) if i == c => " ▼",
                    _ => "",
                };
                let w = widths[c];
                let resp = self
                    .cell_frame(ui, w, row_h, l.bg_soft, l, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("{name}{arrow}")).strong().color(l.fg),
                            )
                            .truncate()
                            .selectable(false),
                        );
                    })
                    .interact(egui::Sense::click());
                if resp.clicked() {
                    self.sort = match self.sort {
                        Some((i, Order::Asc)) if i == c => Some((c, Order::Desc)),
                        Some((i, Order::Desc)) if i == c => None,
                        _ => Some((c, Order::Asc)),
                    };
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        });
    }

    // -------------------------------------------------------------- 中身の行

    #[allow(clippy::too_many_arguments)]
    fn data_line(
        &mut self,
        ui: &mut egui::Ui,
        l: &style::Look,
        row: &[String],
        widths: &[f32],
        row_h: f32,
        cols: usize,
        r: usize,
    ) {
        let sel = self.range();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.set_min_height(row_h);

            // 行の番号
            self.cell_frame(ui, NUM_COL, row_h, l.bg_soft, l, |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new((r + 1).to_string()).color(l.fg_dim).size(11.5),
                    )
                    .selectable(false),
                );
            });

            for c in 0..cols {
                let text = row.get(c).cloned().unwrap_or_default();
                let inside = sel
                    .map(|(r0, r1, c0, c1)| r >= r0 && r <= r1 && c >= c0 && c <= c1)
                    .unwrap_or(false);
                let fill = if inside {
                    l.accent.gamma_multiply(0.22)
                } else if r % 2 == 1 {
                    l.bg_soft.gamma_multiply(0.5)
                } else {
                    l.bg
                };
                let numeric = crate::table::is_numeric(&text);
                let w = widths[c];

                let resp = self.cell_frame(ui, w, row_h, fill, l, |ui| {
                    let layout = if numeric {
                        egui::Layout::right_to_left(egui::Align::Center)
                    } else {
                        egui::Layout::left_to_right(egui::Align::Center)
                    };
                    ui.with_layout(layout, |ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(&text).color(l.fg))
                                .truncate()
                                .selectable(false),
                        );
                    });
                })
                .interact(egui::Sense::click_and_drag());

                // 引きずっている間、指の下にあるます目まで選ぶ。
                // 引きずり中は他のます目が「触れられている」とみなされないので、
                // 指の場所と枠の重なりを自分で見る。
                if resp.drag_started() || (resp.clicked() && !self.dragging) {
                    self.anchor = Some((r, c));
                    self.focus = Some((r, c));
                    self.dragging = true;
                }
                if self.dragging {
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        if resp.rect.contains(pos) {
                            self.focus = Some((r, c));
                        }
                    }
                }
            }
        });

        if ui.input(|i| i.pointer.primary_released()) {
            self.dragging = false;
        }
    }

    /// ます目1つ。縦線と塗りをここで受け持つ。
    fn cell_frame<R>(
        &self,
        ui: &mut egui::Ui,
        w: f32,
        h: f32,
        fill: egui::Color32,
        l: &style::Look,
        add: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            p.rect_filled(rect, 0.0, fill);
            // 縦線と横線。表計算のます目に見えるよう、どちらも引く。
            let stroke = egui::Stroke::new(1.0, l.line);
            p.line_segment(
                [rect.right_top(), rect.right_bottom()],
                stroke,
            );
            p.line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                stroke,
            );
        }
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink2(egui::vec2(7.0, 2.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        add(&mut child);
        response
    }

    /// 列の幅を中身から決める。
    fn measure(&self, ui: &egui::Ui, t: &Table, font: &egui::FontId) -> Vec<f32> {
        let mut widths = vec![0.0f32; t.cols];
        let (header, rows) = self.parts(t);

        let mut sample: Vec<&Vec<String>> = Vec::new();
        if let Some(h) = header {
            sample.push(h);
        }
        // 行が多い表で全部を測ると遅くなるので、先頭の200行から決める
        sample.extend(rows.iter().take(200).copied());

        for row in sample {
            for (i, cell) in row.iter().enumerate().take(t.cols) {
                let g = ui
                    .painter()
                    .layout_no_wrap(cell.clone(), font.clone(), egui::Color32::WHITE);
                widths[i] = widths[i].max(g.rect.width());
            }
        }
        widths
            .into_iter()
            .map(|w| (w + 20.0).clamp(MIN_COL, MAX_COL))
            .collect()
    }
}

/// 中身を見て、数として比べられるならその順、無理なら文字の順で比べる。
fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    let na = to_number(a);
    let nb = to_number(b);
    match (na, nb) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

fn to_number(s: &str) -> Option<f64> {
    let t = s.trim().replace(',', "");
    if t.is_empty() {
        return None;
    }
    let t = t.trim_end_matches('%');
    t.parse::<f64>().ok()
}
