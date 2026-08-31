//! 画面に日本語を出すためのフォントを、OS から借りてくる。
//! 埋め込むと実行ファイルが数十MBになるので、置いてあるものを読む。

/// 探す順に並べた候補。（場所, 面の番号）
///
/// **順番が大事である。** 中国語や韓国語のフォントも同じ字を持っているため、
/// 先に見つかったほうが使われると、字の形がその国のものになってしまう。
/// 日本語のために作られたものを、必ず先に置く。
const CANDIDATES: &[(&str, u32)] = &[
    // ---- Windows
    ("C:\\Windows\\Fonts\\YuGothM.ttc", 0),   // 游ゴシック Medium
    ("C:\\Windows\\Fonts\\YuGothR.ttc", 0),   // 游ゴシック Regular
    ("C:\\Windows\\Fonts\\meiryo.ttc", 0),    // メイリオ
    ("C:\\Windows\\Fonts\\msgothic.ttc", 0),  // MS ゴシック
    // ---- macOS
    ("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", 0),
    ("/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc", 0),
    ("/System/Library/Fonts/ヒラギノ丸ゴ ProN W4.ttc", 0),
    ("/Library/Fonts/Osaka.ttf", 0),
    // ---- Linux
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-JP-Regular.otf", 0),
    ("/usr/share/fonts/opentype/noto/NotoSansCJKjp-Regular.otf", 0),
    ("/usr/share/fonts/truetype/fonts-japanese-gothic.ttf", 0),
    ("/usr/share/fonts/opentype/ipafont-gothic/ipagp.ttf", 0),
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
];

/// 等幅の候補。日本語も等幅で持っているものを選ぶ。
/// 英数字と日本語で別のフォントになると、同じ行の中でベースラインがずれる。
const MONO_CANDIDATES: &[(&str, u32)] = &[
    // ---- Windows
    ("C:\\Windows\\Fonts\\msgothic.ttc", 0),   // MS ゴシック（等幅）
    ("C:\\Windows\\Fonts\\BIZ-UDGothicR.ttc", 0),
    // ---- macOS
    // 標準では日本語の等幅フォントが無い。Osaka-Mono を入れている環境でだけ拾う。
    ("/Library/Fonts/Osaka.ttf", 0),
    ("/System/Library/Fonts/Supplemental/Osaka.ttf", 0),
    // ---- Linux
    ("/usr/share/fonts/opentype/noto/NotoSansMonoCJKjp-Regular.otf", 0),
    ("/usr/share/fonts/truetype/fonts-japanese-mincho.ttf", 0),
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
    // 等幅は、日本語も等幅で持つフォントがあればそれだけを使う。
    // 英数字と日本語で別のフォントになると、同じ行でベースラインがずれる。
    // 日本語を持たないサブセットのフォントが混ざることがあるので、
    // 中身の大きさで見分ける（日本語を持つものは数MBある）。
    let mono = MONO_CANDIDATES
        .iter()
        .filter(|(p, _)| std::path::Path::new(p).exists())
        .find_map(|(p, i)| {
            let b = std::fs::read(p).ok()?;
            (b.len() > 500_000).then_some((b, *i))
        });

    match mono {
        Some((bytes, index)) => {
            let mut d = egui::FontData::from_owned(bytes);
            d.index = index;
            fonts
                .font_data
                .insert("jp-mono".to_owned(), std::sync::Arc::new(d));
            let m = fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default();
            m.insert(0, "jp-mono".to_owned());
            m.push("jp".to_owned());
        }
        None => {
            // 等幅の日本語フォントが無いときは、英数字は等幅のまま、
            // 日本語だけをこのフォントで描く。ベースラインは両者で揃っている。
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("jp".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}
