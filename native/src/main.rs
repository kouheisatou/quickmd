#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! QuickMD — 読むことだけに絞ったマークダウンと CSV のプレビュー。
//! WebView を使わず GPU へ直に描くので、立ち上がりが速い。

use std::path::PathBuf;
use std::time::Instant;

mod doc;
mod fonts;
mod mdtable;
mod menu;
#[cfg(target_os = "macos")]
mod macmenu;
mod openwith;
mod recent;
mod render;
mod settings;
mod sheet;
mod state;
mod style;
mod table;
mod welcome;

use doc::{Block, Doc, Kind};
use render::{Art, Renderer};
use std::cell::RefCell;
use std::rc::Rc;
use settings::Settings;

// ------------------------------------------------------------------ 起動の引数

struct Args {
    file: Option<PathBuf>,
    timing_out: Option<PathBuf>,
    exit_after_ready: bool,
    open_settings: bool,
}

fn parse_args() -> Args {
    let mut file = None;
    let mut timing_out = None;
    let mut exit_after_ready = false;
    let mut open_settings = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        if a == "--timing" {
            timing_out = it.next().map(PathBuf::from);
        } else if a == "--exit-after-ready" {
            exit_after_ready = true;
        } else if a == "--settings" {
            open_settings = true;
        } else if !a.starts_with('-') && file.is_none() {
            let p = PathBuf::from(&a);
            file = Some(if p.is_absolute() {
                p
            } else {
                std::env::current_dir().unwrap_or_default().join(p)
            });
        }
    }
    Args {
        file,
        timing_out,
        exit_after_ready,
        open_settings,
    }
}

// ------------------------------------------------------------------ アプリ本体

struct App {
    start: Instant,
    marks: Vec<(&'static str, f64)>,
    timing_out: Option<PathBuf>,
    exit_after_ready: bool,
    first_frame: bool,

    doc: Doc,
    cache: egui_commonmark::CommonMarkCache,
    settings: Settings,
    dark: bool,
    settings_open: bool,
    settings_just_opened: bool,
    renderer: Rc<RefCell<Renderer>>,
    positions: state::Positions,
    /// 開いた直後だけ、前に読んでいた位置へ戻す
    restore_scroll: Option<f32>,
    scroll_now: f32,
    scroll_saved: f32,
    saved_at: std::time::Instant,
    recent: recent::Recent,
    /// CSV を開いたときの表の状態
    sheet: sheet::Sheet,
    /// 窓の名前を付け替える必要があるか
    title_dirty: bool,
    #[cfg(target_os = "macos")]
    mac_menu: Option<macmenu::MacMenu>,
}

impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        args: &Args,
        start: Instant,
        mut marks: Vec<(&'static str, f64)>,
    ) -> Self {
        marks.push(("window_ready", ms(start)));
        fonts::install(&cc.egui_ctx);
        marks.push(("fonts", ms(start)));

        let settings = settings::load();
        let dark = match settings.theme.as_str() {
            "dark" => true,
            "light" => false,
            _ => cc.egui_ctx.theme() == egui::Theme::Dark,
        };
        style::apply(&cc.egui_ctx, dark, settings.font_size, settings.line_height);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        marks.push(("style", ms(start)));

        let doc = match &args.file {
            Some(p) => Doc::load(p, &settings.csv_encoding),
            None => Doc::empty(),
        };
        marks.push(("file_read", ms(start)));

        // 前に読んでいた位置を思い出す
        let positions = state::load();
        let restore_scroll = positions.get(&doc.path).filter(|v| *v > 1.0);

        Self {
            start,
            marks,
            timing_out: args.timing_out.clone(),
            exit_after_ready: args.exit_after_ready,
            first_frame: true,
            doc,
            cache: egui_commonmark::CommonMarkCache::default(),
            settings,
            dark,
            settings_open: args.open_settings,
            settings_just_opened: args.open_settings,
            renderer: Rc::new(RefCell::new(Renderer::default())),
            positions,
            restore_scroll,
            scroll_now: 0.0,
            scroll_saved: -1.0,
            saved_at: start,
            sheet: sheet::Sheet::default(),
            title_dirty: false,
            #[cfg(target_os = "macos")]
            mac_menu: macmenu::MacMenu::install(),
            recent: {
                let mut r = recent::load();
                if let Some(p) = &args.file {
                    r.push(p);
                }
                r
            },
        }
    }

    fn report(&mut self) {
        let total = ms(self.start);
        self.marks.push(("painted", total));
        if let Some(p) = &self.timing_out {
            let out = serde_json::json!({
                "total_ms": total,
                "marks": self.marks.iter()
                    .map(|(k, v)| serde_json::json!({"name": k, "ms": v}))
                    .collect::<Vec<_>>(),
            });
            let _ = std::fs::write(p, serde_json::to_string_pretty(&out).unwrap_or_default());
        }
    }

    /// 別のファイルへ開き直す。
    fn open(&mut self, path: &std::path::Path) {
        self.remember_scroll(true);
        self.doc = Doc::load(path, &self.settings.csv_encoding);
        self.restore_scroll = self.positions.get(&self.doc.path).filter(|v| *v > 1.0);
        self.scroll_now = 0.0;
        self.scroll_saved = -1.0;
        self.recent.push(path);
        self.sheet = sheet::Sheet::default();
        self.title_dirty = true;
    }

    /// 開くファイルを選んでもらう。
    fn ask_open(&mut self, kind: Option<welcome::Pick>) {
        let mut d = rfd::FileDialog::new();
        d = match kind {
            Some(welcome::Pick::Table) => d
                .add_filter("CSV・TSV", &["csv", "tsv"])
                .add_filter("すべて", &["*"]),
            _ => d
                .add_filter("マークダウン", &["md", "markdown", "mdown", "mkd", "mdx"])
                .add_filter("CSV・TSV", &["csv", "tsv"])
                .add_filter("すべて", &["*"]),
        };
        if let Some(dir) = self.doc.path.parent() {
            if dir.is_dir() {
                d = d.set_directory(dir);
            }
        }
        if let Some(p) = d.pick_file() {
            self.open(&p);
        }
    }

    /// メニューで選ばれたことを行う。
    fn run_action(&mut self, ctx: &egui::Context, action: menu::Action) {
        match action {
            menu::Action::NewWindow => open_new_window(std::path::Path::new("")),
            menu::Action::OpenFile => self.ask_open(None),
            menu::Action::OpenRecent(p) => self.open(&p),
            menu::Action::ClearRecent => self.recent.clear(),
            menu::Action::Settings => {
                self.settings_open = true;
                self.settings_just_opened = true;
            }
            menu::Action::Reload => self.reload(),
            menu::Action::EditExternal => openwith::edit(&self.doc.path, &self.settings.editor),
            menu::Action::RevealInFolder => openwith::reveal(&self.doc.path),
            menu::Action::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    fn reload(&mut self) {
        let keep = self.scroll_now;
        self.doc = Doc::load(&self.doc.path.clone(), &self.settings.csv_encoding);
        self.restore_scroll = Some(keep);
    }

    /// 読みかけの位置を残す。閉じ方によらず残るよう、途中でも折を見て書く。
    fn remember_scroll(&mut self, force: bool) {
        if (self.scroll_now - self.scroll_saved).abs() < 1.0 {
            return;
        }
        if !force && self.saved_at.elapsed().as_secs_f32() < 1.5 {
            return;
        }
        self.positions.set(&self.doc.path, self.scroll_now);
        self.positions.save();
        self.scroll_saved = self.scroll_now;
        self.saved_at = std::time::Instant::now();
    }

    fn apply_style(&mut self, ctx: &egui::Context) {
        self.dark = match self.settings.theme.as_str() {
            "dark" => true,
            "light" => false,
            _ => ctx.system_theme().unwrap_or(egui::Theme::Light) == egui::Theme::Dark,
        };
        style::apply(
            ctx,
            self.dark,
            self.settings.font_size,
            self.settings.line_height,
        );
    }

    /// 本文のリンクが押されたときの行き先を決める。
    /// マークダウンと CSV は新しいウィンドウで開き、それ以外は OS の既定のアプリへ渡す。
    fn handle_link(&mut self, ctx: &egui::Context) {
        // egui はリンクの押下を「命令」として溜める。ここで横取りして自分で行き先を決める。
        let url = ctx.output_mut(|o| {
            let mut found = None;
            o.commands.retain(|c| match c {
                egui::OutputCommand::OpenUrl(u) => {
                    found = Some(u.url.clone());
                    false
                }
                _ => true,
            });
            found
        });
        let Some(url) = url else {
            return;
        };

        // 同じ文書の中の飛び先は、リンクではなく巻き取りで扱う
        if url.starts_with('#') {
            return;
        }

        let remote = url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("mailto:")
            || url.starts_with("data:");
        if remote {
            openwith::open_url(&url);
            return;
        }

        let base = self
            .doc
            .path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let target = doc::resolve(&base, url.trim_start_matches("file://"));

        if !target.exists() {
            return;
        }
        if target.is_dir() {
            openwith::open_folder(&target);
            return;
        }
        if doc::is_readable(&target) {
            open_new_window(&target);
        } else if doc::media_kind(&target.to_string_lossy()) == Some(doc::MediaKind::Video) {
            play_video(&target);
        } else {
            openwith::edit(&target, "");
        }
    }

    // ----------------------------------------------------------------- 本文

    fn body(&mut self, ui: &mut egui::Ui) {
        let list = self.recent.list();
        let has_doc = !self.doc.path.as_os_str().is_empty();

        // macOS では画面上端のメニューから来る
        #[cfg(target_os = "macos")]
        {
            let picked = if let Some(m) = self.mac_menu.as_mut() {
                m.sync_recent(&list);
                m.poll()
            } else {
                None
            };
            if let Some(a) = picked {
                let ctx = ui.ctx().clone();
                self.run_action(&ctx, a);
                return;
            }
        }

        if let Some(a) = menu::bar(ui, self.dark, &list, has_doc, &self.doc.name) {
            let ctx = ui.ctx().clone();
            self.run_action(&ctx, a);
            return;
        }

        if !has_doc {
            if let Some(pick) = welcome::show(ui, self.dark, &list) {
                match pick {
                    welcome::Pick::Recent(p) => self.open(&p),
                    other => self.ask_open(Some(other)),
                }
            }
            return;
        }

        if let Some(err) = &self.doc.error {
            ui.add_space(24.0);
            ui.heading("開けませんでした");
            ui.add_space(8.0);
            ui.label(self.doc.path.to_string_lossy().to_string());
            ui.label(err.clone());
            return;
        }

        match self.doc.kind {
            Kind::Table => self.table_body(ui),
            _ => self.markdown_body(ui),
        }
    }

    fn markdown_body(&mut self, ui: &mut egui::Ui) {
        let width = self.settings.content_width;
        let dark = self.dark;
        let base = self.doc.path.parent().map(|p| p.to_path_buf());

        // self のフィールドを別々に借りて渡す（まとめて借りると中で衝突する）
        let blocks = &self.doc.blocks;
        let cache = &mut self.cache;
        let renderer = &self.renderer;
        let settings = &self.settings;

        let wrap = self.settings.wrap;
        // 折り返さないときは、いちばん長い行に合わせて横へ広げる。
        // その幅を先に決めておかないと、毎回の描き直しで幅が揺れる。
        let long_line = if wrap {
            0.0
        } else {
            longest_line_width(ui, blocks)
        };

        let mut area = egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .hscroll(!wrap);
        if let Some(y) = self.restore_scroll {
            area = area.vertical_scroll_offset(y);
        }
        let out = area
            .show(ui, |ui| {
                let avail = ui.available_width();
                let (pad, inner) = if wrap {
                    let pad = ((avail - width) / 2.0).max(16.0);
                    (pad, (avail - pad * 2.0).max(200.0))
                } else {
                    (32.0, long_line.max(avail - 64.0))
                };
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_max_width(inner);
                        // 箇条書きの丸は文字と同じ高さの枠に描かれる。
                        // 文字を枠の上端へ合わせると、丸と文字の高さが揃う。
                        ui.style_mut().override_text_valign = Some(egui::Align::TOP);
                        ui.add_space(28.0);
                        for block in blocks {
                            match block {
                                Block::Markdown(src) => {
                                    let src = fix_relative_images(src, base.as_deref());
                                    let ctx = ui.ctx().clone();
                                    let r = renderer.clone();
                                    let enable_math = settings.enable_math;
                                    let em = settings.font_size;
                                    // 第3引数はインラインかどうかである（真ならインライン）
                                    let math = move |ui: &mut egui::Ui, tex: &str, inline: bool| {
                                        draw_math(ui, &ctx, &r, tex, !inline, dark, enable_math, em);
                                    };
                                    egui_commonmark::CommonMarkViewer::new()
                                        .max_image_width(Some(inner as usize))
                                        .render_math_fn(Some(&math))
                                        .show(ui, cache, &src);
                                }
                                Block::Table(src) => {
                                    draw_table(ui, cache, src, dark, inner);
                                }
                                Block::Code { lang, src } => {
                                    draw_code(ui, lang, src, dark, inner);
                                }
                                Block::Media {
                                    alt,
                                    link,
                                    path,
                                    kind,
                                } => {
                                    draw_media(
                                        ui, renderer, alt, link, path, *kind, dark, inner,
                                    );
                                }
                                Block::Mermaid { src, .. } => {
                                    draw_mermaid(
                                        ui,
                                        renderer,
                                        src,
                                        dark,
                                        settings.enable_mermaid,
                                        inner,
                                        settings.font_size,
                                    );
                                }
                            }
                        }
                        ui.add_space(72.0);
                    });
                });
            });

        self.scroll_now = out.state.offset.y;
        // 戻すのは開いた直後の1回だけにする。以降は手で動かせるようにする。
        if self.restore_scroll.is_some() && out.content_size.y > 1.0 {
            self.restore_scroll = None;
        }
    }

    fn table_body(&mut self, ui: &mut egui::Ui) {
        let Some(t) = &self.doc.table else { return };
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 12,
                bottom: 12,
            })
            .show(ui, |ui| {
                self.sheet.show(ui, t, self.dark);
            });
    }

    // --------------------------------------------------------------- 設定の窓

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        let mut changed = false;

        // 親の真ん中へ置く。位置を決めないと画面の外へ出ることがある。
        let just_opened = self.settings_just_opened;
        self.settings_just_opened = false;
        let dark = self.dark;
        let font_size = self.settings.font_size;
        let line_height = self.settings.line_height;
        let size = egui::vec2(600.0, 780.0);
        let pos = ctx
            .input(|i| i.viewport().outer_rect)
            .map(|r| r.center() - size / 2.0)
            .unwrap_or(egui::pos2(120.0, 120.0));

        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("settings"),
            egui::ViewportBuilder::default()
                .with_title("QuickMD の設定")
                .with_inner_size(size)
                .with_min_inner_size([420.0, 360.0])
                .with_position(pos)
                .with_visible(true)
                .with_active(true),
            |ctx, _class| {
                // 子の窓は色の設定を引き継がないので、ここでも同じものを当てる
                style::apply(ctx, dark, font_size, line_height);
                // 親の上に重なるので、開いた直後は手前へ出す
                if just_opened {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(style::look(dark).bg))
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                ui.add_space(24.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(30.0);
                                    ui.vertical(|ui| {
                                        changed |=
                                            settings_form(ui, &mut self.settings, self.dark);
                                    });
                                });
                                ui.add_space(30.0);
                            });
                    });
                if ctx.input(|i| i.viewport().close_requested()) {
                    open = false;
                }
            },
        );

        if changed {
            settings::save(&self.settings);
            let before = self.dark;
            self.apply_style(ctx);
            if before != self.dark {
                self.renderer.borrow_mut().forget_all();
            }
            if matches!(self.doc.kind, Kind::Table) {
                self.reload();
            }
        }
        if !open {
            self.settings_open = false;
        }
    }

    // ------------------------------------------------------------- キーの割り当て

    fn keys(&mut self, ctx: &egui::Context) {
        let (cmd, keys) = ctx.input(|i| {
            (
                i.modifiers.command,
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Key {
                            key, pressed: true, ..
                        } => Some(*key),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            )
        });

        for key in keys {
            match key {
                egui::Key::W if cmd => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                egui::Key::Comma if cmd => {
                    self.settings_open = true;
                    self.settings_just_opened = true;
                }
                egui::Key::E => openwith::edit(&self.doc.path, &self.settings.editor),
                egui::Key::O => openwith::reveal(&self.doc.path),
                egui::Key::R => self.reload(),
                egui::Key::Q => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                egui::Key::Escape if !self.settings_open => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close)
                }
                _ => {}
            }
        }
    }
}

impl eframe::App for App {
    /// 別のウィンドウ（設定）は、本文を描き始める前にここで出す。
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.keys(ctx);
        self.settings_window(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.title_dirty {
            self.title_dirty = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title(&self.doc.name)));
        }
        self.body(ui);
        self.handle_link(&ctx);

        self.remember_scroll(false);

        if self.first_frame {
            self.first_frame = false;
            self.report();
            if self.exit_after_ready {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.remember_scroll(true);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = style::look(self.dark).bg;
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }
}

// --------------------------------------------------------------------- 部品

/// 名前の欄の幅。すべての行でこれに揃える。
const LABEL_W: f32 = 140.0;
/// 1行の高さ。名前と操作部の中心をこの高さの真ん中で合わせる。
const ROW_H: f32 = 28.0;
/// 選ぶ欄・書き込む欄の幅。
const FIELD_W: f32 = 250.0;

/// 設定の中身を、名前と操作部の2列で組む。
/// 表組みに載せているので、どの行でも操作部の左端が同じ位置から始まる。
fn settings_form(ui: &mut egui::Ui, s: &mut Settings, dark: bool) -> bool {
    let l = style::look(dark);
    let mut changed = false;

    egui::Grid::new("settings")
        .num_columns(2)
        .min_col_width(LABEL_W)
        .spacing([18.0, 6.0])
        .show(ui, |ui| {
            section(ui, &l, "表示", true);

            changed |= row(ui, "テーマ", |ui| {
                let mut c = false;
                egui::ComboBox::from_id_salt("theme")
                    .width(FIELD_W)
                    .selected_text(match s.theme.as_str() {
                        "dark" => "暗い",
                        "light" => "明るい",
                        _ => "画面の設定に合わせる",
                    })
                    .show_ui(ui, |ui| {
                        for (v, label) in [
                            ("light", "明るい"),
                            ("dark", "暗い"),
                            ("system", "画面の設定に合わせる"),
                        ] {
                            c |= ui.selectable_value(&mut s.theme, v.into(), label).changed();
                        }
                    });
                c
            });

            changed |= slider_row(ui, "文字の大きさ", &mut s.font_size, 12.0..=24.0, 0);
            changed |= slider_row(ui, "行の高さ", &mut s.line_height, 1.3..=2.2, 2);
            changed |= slider_row(ui, "本文の幅", &mut s.content_width, 560.0..=1600.0, 0);

            section(ui, &l, "本文の中身", false);

            changed |= row(ui, "数式を描く", |ui| {
                ui.checkbox(&mut s.enable_math, "").changed()
            });
            changed |= row(ui, "Mermaid の図を描く", |ui| {
                ui.checkbox(&mut s.enable_mermaid, "").changed()
            });
            changed |= row(ui, "CSV の文字コード", |ui| {
                let mut c = false;
                egui::ComboBox::from_id_salt("csv_encoding")
                    .width(FIELD_W)
                    .selected_text(if s.csv_encoding == "auto" {
                        "自動で見分ける".to_string()
                    } else {
                        s.csv_encoding.clone()
                    })
                    .show_ui(ui, |ui| {
                        for (v, label) in [
                            ("auto", "自動で見分ける"),
                            ("UTF-8", "UTF-8"),
                            ("Shift_JIS", "Shift_JIS"),
                            ("EUC-JP", "EUC-JP"),
                            ("UTF-16LE", "UTF-16LE"),
                        ] {
                            c |= ui
                                .selectable_value(&mut s.csv_encoding, v.into(), label)
                                .changed();
                        }
                    });
                c
            });

            section(ui, &l, "編集", false);

            changed |= row(ui, "編集を開くアプリ", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut s.editor)
                        .hint_text("空なら OS の既定のアプリ")
                        .desired_width(FIELD_W),
                )
                .changed()
            });
            note(
                ui,
                &l,
                "E キーを押すと、このアプリがファイルを開きます。QuickMD 自身は編集しません。",
            );

            section(ui, &l, "キー操作", false);

            for (k, v) in [
                ("E", "編集するアプリで開く"),
                ("O", "ファイルのある場所を開く"),
                ("R", "読み直す"),
                (
                    if cfg!(target_os = "macos") {
                        "⌘ ,"
                    } else {
                        "Ctrl + ,"
                    },
                    "この設定を開く",
                ),
                ("Q / Esc", "閉じる"),
            ] {
                row(ui, k, |ui| {
                    ui.colored_label(l.fg_dim, v);
                });
            }
        });

    changed
}

/// 1つのます目。高さを揃えたうえで、中身を縦の真ん中へ置く。
fn cell<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.set_min_height(ROW_H);
        add(ui)
    })
    .inner
}

/// まとまりの見出し。見出しの上には1行ぶんの間を空ける。
fn section(ui: &mut egui::Ui, l: &style::Look, title: &str, first: bool) {
    if !first {
        ui.allocate_space(egui::vec2(0.0, 14.0));
        ui.label("");
        ui.end_row();
    }
    cell(ui, |ui| {
        ui.colored_label(l.fg_dim, egui::RichText::new(title).size(12.0));
    });
    ui.label("");
    ui.end_row();
}

/// 操作部の欄にだけ置く補足。名前の欄は空にして、左端を操作部へ揃える。
fn note(ui: &mut egui::Ui, l: &style::Look, text: &str) {
    ui.label("");
    ui.add(egui::Label::new(egui::RichText::new(text).size(12.0).color(l.fg_dim)).wrap());
    ui.end_row();
}

/// 「名前｜操作部」の1行。
fn row<R>(ui: &mut egui::Ui, name: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    cell(ui, |ui| {
        ui.label(name);
    });
    let out = cell(ui, add);
    ui.end_row();
    out
}

fn slider_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    decimals: usize,
) -> bool {
    row(ui, label, |ui| {
        let changed = ui
            .add_sized(
                [FIELD_W - 54.0, 18.0],
                egui::Slider::new(value, range)
                    .show_value(false)
                    .trailing_fill(true),
            )
            .changed();
        let text = format!("{value:.decimals$}");
        ui.allocate_ui_with_layout(
            egui::vec2(46.0, ROW_H),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.label(text);
            },
        );
        changed
    })
}

/// Mermaid の図。画面に入ってから初めて作らせる。
fn draw_mermaid(
    ui: &mut egui::Ui,
    renderer: &Rc<RefCell<Renderer>>,
    src: &str,
    dark: bool,
    enabled: bool,
    width: f32,
    em: f32,
) {
    let l = style::look(dark);
    ui.add_space(10.0);

    if !enabled {
        show_source(ui, src, &l, "Mermaid の図（描かない設定です）");
        return;
    }

    // まだ図が無いあいだの置き場所。ここが画面に入ったら作りにいく。
    let ready = renderer.borrow().peek("mermaid", src, dark, false);
    if let Some(tex) = ready {
        draw_texture(ui, &tex, width);
        ui.add_space(10.0);
        return;
    }

    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 120.0), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let art = renderer
        .borrow_mut()
        .ask(ui.ctx(), "mermaid", src, dark, false, em);
    if let Art::Ready(tex) = &art {
        // できあがっていたら、次の描き直しで本物が出る
        let _ = tex;
        ui.ctx().request_repaint();
    }
    let p = ui.painter();
    p.rect_filled(rect, 6.0, l.bg_soft);
    let text = match &art {
        Art::Failed(e) => format!("図を作れませんでした: {e}"),
        _ => "図を作っています…".to_string(),
    };
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(13.0),
        l.fg_dim,
    );
    ui.add_space(10.0);
}

/// コードを枠に入れて描く。色は付けず、そのままの文字で出す。
/// 右上の写しボタンで、中身をまるごと写し取れる。
fn draw_code(ui: &mut egui::Ui, lang: &str, src: &str, dark: bool, width: f32) {
    let l = style::look(dark);
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let body = src.trim_end_matches('\n').to_string();

    ui.add_space(8.0);
    egui::Frame::new()
        .fill(l.bg_soft)
        .stroke(egui::Stroke::new(1.0, l.line))
        .corner_radius(6)
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 8,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_min_width(width - 26.0);

            // 言語の名前を左に、写しボタンを右に置く
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                if !lang.is_empty() {
                    ui.colored_label(l.fg_dim, egui::RichText::new(lang).size(11.0));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let copied = ui.ctx().data(|d| {
                        d.get_temp::<f64>(egui::Id::new(("copied", &body)))
                            .unwrap_or(f64::NEG_INFINITY)
                    });
                    let now = ui.input(|i| i.time);
                    let label = if now - copied < 1.5 { "写しました" } else { "写す" };
                    if ui
                        .add(egui::Button::new(egui::RichText::new(label).size(11.0)).small())
                        .clicked()
                    {
                        ui.ctx().copy_text(body.clone());
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(egui::Id::new(("copied", &body)), now);
                        });
                    }
                });
            });
            ui.add_space(4.0);

            egui::ScrollArea::horizontal()
                .id_salt(egui::Id::new(("code", &body)))
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&body).font(font.clone()).color(l.fg),
                        )
                        .selectable(true),
                    );
                });
        });
    ui.add_space(8.0);
}

/// 本文に埋め込まれた絵・動画・音を描く。右クリックで保存や場所開きができる。
fn draw_media(
    ui: &mut egui::Ui,
    renderer: &Rc<RefCell<Renderer>>,
    alt: &str,
    link: &str,
    path: &std::path::Path,
    kind: doc::MediaKind,
    dark: bool,
    width: f32,
) {
    let l = style::look(dark);
    let exists = path.exists();
    ui.add_space(10.0);

    let response = if kind == doc::MediaKind::Image && exists {
        let uri = format!("file://{}", path.to_string_lossy());
        // 大きさは自分で決める。任せると、読み終わるまで高さがゼロのままになる。
        let natural = match ui
            .ctx()
            .try_load_image(&uri, egui::SizeHint::Scale(1.0.into()))
        {
            Ok(egui::load::ImagePoll::Ready { image }) => {
                Some(egui::vec2(image.width() as f32, image.height() as f32))
            }
            _ => None,
        };
        match natural {
            Some(size) => {
                let scale = (width / size.x).min(1.0);
                ui.add(
                    egui::Image::from_uri(uri)
                        .fit_to_exact_size(size * scale)
                        .corner_radius(6)
                        .sense(egui::Sense::click()),
                )
            }
            None => {
                // まだ読めていないあいだの場所取り。次に描くときに本物が入る。
                ui.ctx().request_repaint();
                let (rect, r) =
                    ui.allocate_exact_size(egui::vec2(width, 160.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 6.0, l.bg_soft);
                r
            }
        }
    } else if kind == doc::MediaKind::Video && exists {
        draw_video(ui, renderer, path, dark, width)
    } else {
        // 音と、見つからないものは札で示す。
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| link.to_string());
        let mark = match kind {
            doc::MediaKind::Video => "▶",
            doc::MediaKind::Audio => "♪",
            doc::MediaKind::Image => "▨",
        };
        let text = if exists {
            format!("{mark}  {name}")
        } else {
            format!("{mark}  {name}（見つかりません）")
        };
        egui::Frame::new()
            .fill(l.bg_soft)
            .stroke(egui::Stroke::new(1.0, l.line))
            .corner_radius(6)
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_min_width(width - 30.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(text).color(if exists {
                        l.fg
                    } else {
                        l.fg_dim
                    }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if exists && ui.button("開く").clicked() {
                            openwith::edit(path, "");
                        }
                    });
                });
                if !alt.is_empty() {
                    ui.add_space(2.0);
                    ui.colored_label(l.fg_dim, egui::RichText::new(alt).size(12.0));
                }
            })
            .response
            .interact(egui::Sense::click())
    };

    // 絵の説明は絵の下に添える。札のほうは枠の中で出している。
    if (kind == doc::MediaKind::Image || kind == doc::MediaKind::Video)
        && exists
        && !alt.is_empty()
    {
        ui.add_space(4.0);
        ui.colored_label(l.fg_dim, egui::RichText::new(alt).size(12.0));
    }

    if exists {
        let p = path.to_path_buf();
        response.context_menu(|ui| {
            if ui.button("名前を付けて保存…").clicked() {
                save_as(&p);
                ui.close();
            }
            if ui.button("ファイルのある場所を開く").clicked() {
                openwith::reveal(&p);
                ui.close();
            }
            if ui.button("既定のアプリで開く").clicked() {
                openwith::edit(&p, "");
                ui.close();
            }
            if ui.button("場所を写す").clicked() {
                ui.ctx().copy_text(p.to_string_lossy().to_string());
                ui.close();
            }
        });
    }
    ui.add_space(10.0);
}

/// 動画。画面に入ったときだけ1コマを作らせ、それをサムネとして出す。
/// 押すと、別のウィンドウが立ち上がって再生が始まる。
fn draw_video(
    ui: &mut egui::Ui,
    renderer: &Rc<RefCell<Renderer>>,
    path: &std::path::Path,
    dark: bool,
    width: f32,
) -> egui::Response {
    let l = style::look(dark);
    let key = path.to_string_lossy().to_string();

    // できていればそれを出す。無ければ場所を取り、見えたときに頼む。
    let ready = renderer.borrow().peek("thumb", &key, false, false);
    let (rect, response, tex) = match ready {
        Some(tex) => {
            let ppp = ui.ctx().pixels_per_point().clamp(1.0, 3.0);
            let size = tex.size_vec2() / ppp;
            let scale = (width / size.x).min(1.0);
            let (rect, r) = ui.allocate_exact_size(size * scale, egui::Sense::click());
            (rect, r, Some(tex))
        }
        None => {
            let (rect, r) =
                ui.allocate_exact_size(egui::vec2(width, width * 9.0 / 16.0), egui::Sense::click());
            if ui.is_rect_visible(rect) {
                if let Art::Failed(e) = renderer.borrow_mut().ask_thumb(ui.ctx(), &key) {
                    let _ = e;
                }
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
            }
            (rect, r, None)
        }
    };

    if !ui.is_rect_visible(rect) {
        return response;
    }

    let p = ui.painter();
    match &tex {
        Some(tex) => {
            p.image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            p.rect_filled(rect, 6.0, egui::Color32::from_black_alpha(40));
        }
        None => {
            p.rect_filled(rect, 6.0, l.bg_soft);
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "動画を読み込んでいます…",
                egui::FontId::proportional(13.0),
                l.fg_dim,
            );
        }
    }

    // 真ん中の再生の印
    if tex.is_some() {
        let r = (rect.height() * 0.16).clamp(22.0, 44.0);
        let c = rect.center();
        p.circle_filled(c, r, egui::Color32::from_black_alpha(150));
        let s = r * 0.5;
        p.add(egui::Shape::convex_polygon(
            vec![
                c + egui::vec2(-s * 0.55, -s),
                c + egui::vec2(-s * 0.55, s),
                c + egui::vec2(s * 0.9, 0.0),
            ],
            egui::Color32::WHITE,
            egui::Stroke::NONE,
        ));
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        play_video(path);
    }
    response
}

/// 再生のためのウィンドウを立てる。OS が持つ再生の仕組みをそのまま使う。
fn play_video(path: &std::path::Path) {
    let Some(exe) = render::helper_path() else {
        // 係が見つからないときは、OS の既定のアプリへ渡す
        openwith::edit(path, "");
        return;
    };
    let _ = std::process::Command::new(exe)
        .arg("--play")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// 保存先を選んでもらい、元のファイルをそこへ写す。
fn save_as(src: &std::path::Path) {
    let name = src
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "無題".into());
    if let Some(dest) = rfd::FileDialog::new().set_file_name(&name).save_file() {
        if let Err(e) = std::fs::copy(src, &dest) {
            eprintln!("QuickMD: 保存できませんでした: {e}");
        }
    }
}

/// 同じアプリをもう1つ立ち上げる。ファイルを渡さなければ、開くための画面が出る。
fn open_new_window(path: &std::path::Path) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = std::process::Command::new(exe);
    if !path.as_os_str().is_empty() {
        cmd.arg(path);
    }
    let _ = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// 表。自分で組んで描く。表として読めなければ、そのまま本文として渡す。
fn draw_table(
    ui: &mut egui::Ui,
    cache: &mut egui_commonmark::CommonMarkCache,
    src: &str,
    dark: bool,
    width: f32,
) {
    match mdtable::parse(src) {
        Some(t) => mdtable::draw(ui, &t, dark, width, egui::Id::new(("table", src))),
        None => {
            egui_commonmark::CommonMarkViewer::new().show(ui, cache, src);
        }
    }
}

/// 折り返さないときの、本文の幅。いちばん長い行に合わせる。
/// 表とコードは自分で横へ流すので、ここでは数えない。
fn longest_line_width(ui: &egui::Ui, blocks: &[Block]) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut widest: f32 = 0.0;
    for block in blocks {
        let Block::Markdown(src) = block else { continue };
        for line in src.lines() {
            let t = line.trim_end();
            if t.is_empty() || t.starts_with('|') {
                continue;
            }
            let galley = ui
                .painter()
                .layout_no_wrap(t.to_string(), font.clone(), egui::Color32::WHITE);
            widest = widest.max(galley.rect.width());
        }
    }
    // 見出しは本文より大きいので、少し余裕を持たせる
    (widest * 1.15 + 40.0).min(6000.0)
}

/// できた絵を、本文の幅に収めて描く。
fn draw_texture(ui: &mut egui::Ui, tex: &egui::TextureHandle, max_width: f32) {
    let ppp = ui.ctx().pixels_per_point();
    let size = tex.size_vec2() / ppp.clamp(1.0, 3.0);
    let scale = (max_width / size.x).min(1.0);
    ui.add(egui::Image::new(tex).fit_to_exact_size(size * scale));
}

fn show_source(ui: &mut egui::Ui, src: &str, l: &style::Look, title: &str) {
    egui::Frame::new()
        .fill(l.bg_soft)
        .stroke(egui::Stroke::new(1.0, l.line))
        .corner_radius(6)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.colored_label(l.fg_dim, title);
            ui.add_space(4.0);
            ui.monospace(src.trim());
        });
    ui.add_space(8.0);
}

/// 数式。式が本文に出てきたときだけ作らせる。
fn draw_math(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    renderer: &Rc<RefCell<Renderer>>,
    tex: &str,
    display: bool,
    dark: bool,
    enabled: bool,
    em: f32,
) {
    if !enabled {
        ui.monospace(tex);
        return;
    }
    match renderer.borrow_mut().ask(ctx, "math", tex, dark, display, em) {
        Art::Ready(handle) => {
            let ppp = ui.ctx().pixels_per_point().clamp(1.0, 3.0);
            ui.add(egui::Image::new(&handle).fit_to_exact_size(handle.size_vec2() / ppp));
        }
        _ => {
            ui.weak(tex);
        }
    }
}

/// 相対パスの画像を、そのファイルのある場所から見た絶対パスへ直す。
fn fix_relative_images(src: &str, base: Option<&std::path::Path>) -> String {
    let Some(base) = base else {
        return src.to_string();
    };
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(i) = rest.find("](") {
        let (head, tail) = rest.split_at(i + 2);
        out.push_str(head);
        let end = match tail.find(')') {
            Some(e) => e,
            None => {
                out.push_str(tail);
                return out;
            }
        };
        let link = &tail[..end];
        let is_remote = link.starts_with("http://")
            || link.starts_with("https://")
            || link.starts_with("data:")
            || link.starts_with("file://")
            || link.starts_with('#')
            || link.starts_with('/');
        if is_remote || link.is_empty() {
            out.push_str(link);
        } else {
            let p = base.join(link);
            out.push_str(&format!("file://{}", p.to_string_lossy()));
        }
        out.push_str(&tail[end..end + 1]);
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// 窓の名前。開いていないときはアプリ名だけにする。
fn window_title(name: &str) -> String {
    if name.is_empty() {
        "QuickMD".to_string()
    } else {
        format!("QuickMD — {name}")
    }
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn main() -> eframe::Result {
    let start = Instant::now();
    let args = parse_args();
    let mut marks = vec![("args", ms(start))];

    let title = window_title(
        &args
            .file
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
    );

    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 820.0])
            .with_min_inner_size([420.0, 320.0])
            .with_title(title),
        ..Default::default()
    };
    marks.push(("opts", ms(start)));

    eframe::run_native(
        "QuickMD",
        opts,
        Box::new(move |cc| Ok(Box::new(App::new(cc, &args, start, marks)))),
    )
}
