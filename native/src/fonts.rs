//! 画面に日本語を出すためのフォントを、OS から借りてくる。
//! 埋め込むと実行ファイルが数十MBになるので、置いてあるものを読む。

const CANDIDATES: &[(&str, u32)] = &[
    // Windows
    ("C:\\Windows\\Fonts\\YuGothM.ttc", 0),
    ("C:\\Windows\\Fonts\\YuGothR.ttc", 0),
    ("C:\\Windows\\Fonts\\meiryo.ttc", 0),
    ("C:\\Windows\\Fonts\\msgothic.ttc", 0),
    // macOS
    ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
    ("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", 0),
    ("/Library/Fonts/Osaka.ttf", 0),
    // Linux
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/truetype/fonts-japanese-gothic.ttf", 0),
];

pub fn install(ctx: &egui::Context) {
    let Some((path, index)) = CANDIDATES
        .iter()
        .find(|(p, _)| std::path::Path::new(p).exists())
    else {
        return;
    };
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };

    let mut data = egui::FontData::from_owned(bytes);
    data.index = *index;

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("jp".to_owned(), std::sync::Arc::new(data));
    // 本文はこのフォントで一通り描く。英数字だけ別のフォントになると、
    // 大きさとベースラインが揃わず、同じ行の中でちぐはぐに見える。
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "jp".to_owned());
    // 等幅のほうは、コードの見た目を保つために元のフォントを先に使い、
    // 日本語が出てきたときだけこちらへ落とす。
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("jp".to_owned());
    ctx.set_fonts(fonts);
}
