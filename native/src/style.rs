//! 画面の色と字の大きさ。読むためのアプリなので、本文の読みやすさだけを見て決める。

use egui::{Color32, FontFamily, FontId, TextStyle};

pub struct Look {
    pub bg: Color32,
    pub bg_soft: Color32,
    pub fg: Color32,
    pub fg_dim: Color32,
    pub line: Color32,
    pub accent: Color32,
}

/// 本文と同じ行の高さになる、等幅の字の大きさを求める。
/// 行の高さは字の大きさにほぼ比例するので、比から逆算して少しだけ詰める。
fn fit_mono_size(ctx: &egui::Context, body_size: f32) -> f32 {
    let body = ctx.fonts_mut(|f| f.row_height(&FontId::new(body_size, FontFamily::Proportional)));
    let mono = ctx.fonts_mut(|f| f.row_height(&FontId::new(body_size, FontFamily::Monospace)));
    if mono <= 0.0 || body <= 0.0 {
        return body_size - 2.0;
    }
    let mut size = body_size * (body / mono);
    // 求めた大きさで測り直し、はみ出していたら少しずつ削る
    for _ in 0..12 {
        let h = ctx.fonts_mut(|f| f.row_height(&FontId::new(size, FontFamily::Monospace)));
        if h <= body {
            break;
        }
        size -= 0.25;
    }
    size.clamp(8.0, body_size)
}

pub fn look(dark: bool) -> Look {
    if dark {
        Look {
            bg: Color32::from_rgb(27, 29, 34),
            bg_soft: Color32::from_rgb(35, 38, 45),
            fg: Color32::from_rgb(215, 219, 224),
            fg_dim: Color32::from_rgb(139, 146, 156),
            line: Color32::from_rgb(51, 56, 66),
            accent: Color32::from_rgb(110, 168, 254),
        }
    } else {
        Look {
            bg: Color32::from_rgb(255, 255, 255),
            bg_soft: Color32::from_rgb(246, 247, 249),
            fg: Color32::from_rgb(31, 35, 40),
            fg_dim: Color32::from_rgb(107, 114, 128),
            line: Color32::from_rgb(216, 220, 225),
            accent: Color32::from_rgb(11, 98, 214),
        }
    }
}

/// 画面の見た目を当てる。
///
/// `measure` は、フォントの大きさを実際に測ってよいかである。
/// 立ち上がりの最初の1回は、まだフォントが用意されていないので測れない。
pub fn apply(ctx: &egui::Context, dark: bool, font_size: f32, line_height: f32, measure: bool) {
    let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
    ctx.set_theme(theme);

    let l = look(dark);
    let mut style = (*ctx.style_of(theme)).clone();

    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.panel_fill = l.bg;
    style.visuals.window_fill = l.bg;
    style.visuals.extreme_bg_color = l.bg_soft;
    style.visuals.faint_bg_color = l.bg_soft;
    // 文字の色は種類ごとに決める。全部を1色で塗ると、リンクの色まで消えてしまう。
    style.visuals.widgets.noninteractive.fg_stroke.color = l.fg;
    style.visuals.widgets.inactive.fg_stroke.color = l.fg;
    style.visuals.widgets.hovered.fg_stroke.color = l.fg;
    style.visuals.widgets.active.fg_stroke.color = l.fg;
    style.visuals.hyperlink_color = l.accent;
    style.visuals.weak_text_alpha = 0.7;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, l.line);
    style.visuals.window_stroke = egui::Stroke::new(1.0, l.line);

    // 巻き取りの棒。既定のままだと本文の脇で目立ちすぎる。
    style.visuals.widgets.inactive.bg_fill = l.line;
    style.visuals.widgets.hovered.bg_fill = l.fg_dim;
    style.visuals.widgets.active.bg_fill = l.fg_dim;

    let p = FontFamily::Proportional;
    let m = FontFamily::Monospace;

    // 等幅は、本文と行の高さが揃うように大きさを決める。
    // 揃っていないと、文の中にコードが混ざったときに行がでこぼこになり、
    // コードの塊では行どうしが重なる。
    let mono_size = if measure {
        fit_mono_size(ctx, font_size)
    } else {
        font_size - 2.0
    };

    style.text_styles = [
        (TextStyle::Body, FontId::new(font_size, p.clone())),
        (TextStyle::Button, FontId::new(font_size - 1.0, p.clone())),
        (TextStyle::Small, FontId::new(font_size - 3.0, p.clone())),
        (TextStyle::Heading, FontId::new(font_size * 1.7, p.clone())),
        (TextStyle::Monospace, FontId::new(mono_size, m)),
    ]
    .into();

    // 行の高さは、字の大きさに対する比で決める。
    // egui では要素と要素の間隔なので、行間そのものより効きが強い。半分にして合わせる。
    style.spacing.item_spacing =
        egui::vec2(6.0, (font_size * (line_height - 1.0) * 0.5).max(2.0));
    style.spacing.indent = 22.0;
    style.spacing.scroll.bar_width = 9.0;
    style.spacing.scroll.floating = false;
    style.spacing.scroll.bar_inner_margin = 3.0;

    // 開発中に出る当たり枠の知らせは、読む邪魔になるので出さない。
    // この欄は開発用のビルドにしか無いので、そのときだけ触る。
    #[cfg(debug_assertions)]
    {
        style.debug.debug_on_hover = false;
        style.debug.show_expand_width = false;
        style.debug.show_expand_height = false;
        style.debug.show_resize = false;
        style.debug.show_interactive_widgets = false;
        style.debug.show_unaligned = false;
    }

    ctx.set_style_of(theme, style);
}
