//! 開いたファイルを、描く単位へ切り分ける。
//! Mermaid の図・コード・埋め込みの絵は自分で描くので、ここで切り出しておく。

use std::path::{Path, PathBuf};

pub enum Block {
    /// そのまま描くマークダウン
    Markdown(String),
    /// Mermaid の図。画面に入ったときに初めて作る。
    Mermaid { src: String },
    /// コード。色は付けず、そのままの文字で出す。
    Code { lang: String, src: String },
    /// 表。本文が折り返さない設定でも、ここだけは横へ流せるようにする。
    Table(String),
    /// 箇条書きと番号つきの並び。丸や番号の位置を揃えるため、自分で描く。
    List(crate::mdlist::List),
    /// 引用。中に置いた箇条書きや見出しも、まとめて縦棒の内側に入れる。
    Quote {
        /// 何段目の引用か（0 から数える）
        depth: usize,
        /// `>` を取り除いた中身。この中をもう一度切り分けて描く。
        inner: Vec<Block>,
    },
    /// 1行に置かれた絵・動画・音。右クリックで保存できるよう自分で描く。
    Media {
        alt: String,
        /// もとの書き方（相対のまま）
        link: String,
        /// 開いているファイルから見て解いた場所
        path: PathBuf,
        kind: MediaKind,
    },
}

#[derive(Clone, Copy, PartialEq)]
pub enum MediaKind {
    Image,
    Video,
    Audio,
}

pub const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "ico", "tif", "tiff"];
pub const VIDEO_EXT: &[&str] = &["mp4", "webm", "mov", "m4v", "ogv", "mkv", "avi"];
pub const AUDIO_EXT: &[&str] = &["mp3", "wav", "m4a", "ogg", "oga", "flac", "aac"];

pub fn media_kind(link: &str) -> Option<MediaKind> {
    let clean = link.split(['?', '#']).next().unwrap_or(link);
    let e = clean.rsplit('.').next()?.to_ascii_lowercase();
    if IMAGE_EXT.contains(&e.as_str()) {
        Some(MediaKind::Image)
    } else if VIDEO_EXT.contains(&e.as_str()) {
        Some(MediaKind::Video)
    } else if AUDIO_EXT.contains(&e.as_str()) {
        Some(MediaKind::Audio)
    } else {
        None
    }
}

pub enum Kind {
    Markdown,
    Table,
    Unsupported,
}

pub struct Doc {
    pub path: PathBuf,
    pub name: String,
    pub kind: Kind,
    pub blocks: Vec<Block>,
    pub table: Option<crate::table::Table>,
    pub error: Option<String>,
}

pub const TABLE_EXT: &[&str] = &["csv", "tsv"];
pub const MD_EXT: &[&str] = &["md", "markdown", "mdown", "mkd", "mdx"];

/// このアプリで開けるものか。
pub fn is_readable(p: &Path) -> bool {
    ext(p)
        .map(|e| MD_EXT.contains(&e.as_str()) || TABLE_EXT.contains(&e.as_str()))
        .unwrap_or(false)
}

fn ext(p: &Path) -> Option<String> {
    p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase())
}

impl Doc {
    pub fn empty() -> Self {
        Self {
            path: PathBuf::new(),
            name: String::new(),
            kind: Kind::Markdown,
            blocks: vec![Block::Markdown(
                "# ファイルが指定されていません\n\nマークダウンか CSV のファイルをこのアプリで開いてください。\n".into(),
            )],
            table: None,
            error: None,
        }
    }

    pub fn load(path: &Path, csv_encoding: &str) -> Self {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let e = ext(path).unwrap_or_default();

        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(err) => {
                return Self {
                    path: path.to_path_buf(),
                    name,
                    kind: Kind::Unsupported,
                    blocks: Vec::new(),
                    table: None,
                    error: Some(err.to_string()),
                };
            }
        };

        if TABLE_EXT.contains(&e.as_str()) {
            let table = crate::table::Table::parse(&bytes, &e, csv_encoding);
            return Self {
                path: path.to_path_buf(),
                name,
                kind: Kind::Table,
                blocks: Vec::new(),
                table: Some(table),
                error: None,
            };
        }

        let text = decode(&bytes);
        let base = path.parent().unwrap_or(Path::new("."));
        Self {
            path: path.to_path_buf(),
            name,
            kind: Kind::Markdown,
            blocks: split_blocks(&text, base),
            table: None,
            error: None,
        }
    }
}

/// 相対の書き方を、開いているファイルから見た場所へ直す。
pub fn resolve(base: &Path, link: &str) -> PathBuf {
    let decoded = percent_decode(link);
    let p = Path::new(&decoded);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    };
    let mut parts: Vec<std::ffi::OsString> = Vec::new();
    for c in joined.components() {
        match c {
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::CurDir => {}
            other => parts.push(other.as_os_str().to_os_string()),
        }
    }
    let mut out = PathBuf::new();
    for part in parts {
        out.push(part);
    }
    out
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let h = (b[i + 1] as char).to_digit(16);
            let l = (b[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (h, l) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// 直前が箇条書きの続きなら、字下げはコードではない。
fn in_list_context(buf: &str) -> bool {
    buf.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|l| {
            let t = l.trim_start();
            t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") || t.starts_with('>')
        })
        .unwrap_or(false)
}

/// `![説明](場所)` だけが置かれた行を見つける。
/// 行の中に混ざっている絵は、そのまま本文として描く（そちらは文の一部だからである）。
fn lone_media(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    let rest = t.strip_prefix("![")?;
    let close = rest.find("](")?;
    let alt = rest[..close].to_string();
    let tail = &rest[close + 2..];
    let end = tail.rfind(')')?;
    if tail[end + 1..].trim().len() > 0 {
        return None;
    }
    let link = tail[..end].split_whitespace().next().unwrap_or("").to_string();
    if link.is_empty() {
        return None;
    }
    Some((alt, link))
}

pub fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let mut d = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
            d.feed(bytes, true);
            d.guess(None, chardetng::Utf8Detection::Allow)
                .decode(bytes)
                .0
                .into_owned()
        }
    }
}

/// 行頭のフェンス（``` や ~~~）で囲まれたところを取り出し、その前後を
/// 普通のマークダウンとして残す。Mermaid の図と、色を付けるコードを別に扱うため。
/// 字下げされたフェンス（箇条書きの中など）は、そのままマークダウンへ渡す。
fn split_blocks(text: &str, base: &Path) -> Vec<Block> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut lines = text.lines().peekable();
    let mut line_no = 0usize;

    while let Some(line) = lines.next() {
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let fence = !indented && (line.starts_with("```") || line.starts_with("~~~"));

        // 1行に絵だけが置かれていたら、自分で描くために取り出す
        if !fence && !indented {
            if let Some((alt, link)) = lone_media(line) {
                let remote = link.starts_with("http://")
                    || link.starts_with("https://")
                    || link.starts_with("data:");
                if let (false, Some(kind)) = (remote, media_kind(&link)) {
                    if !buf.trim().is_empty() {
                        out.push(Block::Markdown(std::mem::take(&mut buf)));
                    }
                    buf.clear();
                    out.push(Block::Media {
                        alt,
                        path: resolve(base, &link),
                        link,
                        kind,
                    });
                    line_no += 1;
                    continue;
                }
            }
        }

        // 引用は自分で受け持つ。中に置いた箇条書きや見出しも縦棒の内側へ入れる。
        if !fence && line.trim_start().starts_with('>') {
            let mut block = String::new();
            let mut cur = Some(line);
            loop {
                let Some(l) = cur else { break };
                let t = l.trim_start();
                match t.strip_prefix('>') {
                    Some(rest) => {
                        block.push_str(rest.strip_prefix(' ').unwrap_or(rest));
                        block.push('\n');
                    }
                    None => break,
                }
                match lines.peek() {
                    Some(n) if n.trim_start().starts_with('>') => {
                        cur = lines.next();
                        line_no += 1;
                    }
                    _ => break,
                }
            }
            if !buf.trim().is_empty() {
                out.push(Block::Markdown(std::mem::take(&mut buf)));
            }
            buf.clear();
            out.push(Block::Quote {
                depth: 0,
                inner: split_blocks(&block, base),
            });
            line_no += 1;
            continue;
        }

        // 4つの空白で字下げしたコードも、フェンスのコードと同じ見た目にする
        if !fence && (line.starts_with("    ") || line.starts_with('\t')) && !in_list_context(&buf) {
            let mut src = String::new();
            let mut cur = Some(line);
            loop {
                let Some(l) = cur else { break };
                let body = l.strip_prefix("    ").or_else(|| l.strip_prefix('\t'));
                match body {
                    Some(b) => {
                        src.push_str(b);
                        src.push('\n');
                    }
                    None if l.trim().is_empty() => src.push('\n'),
                    None => break,
                }
                match lines.peek() {
                    Some(n)
                        if n.starts_with("    ") || n.starts_with('\t') || n.trim().is_empty() =>
                    {
                        cur = lines.next();
                        line_no += 1;
                    }
                    _ => break,
                }
            }
            if !src.trim().is_empty() {
                if !buf.trim().is_empty() {
                    out.push(Block::Markdown(std::mem::take(&mut buf)));
                }
                buf.clear();
                out.push(Block::Code {
                    lang: String::new(),
                    src: src.trim_end().to_string(),
                });
                line_no += 1;
                continue;
            }
        }

        // 箇条書きは自分で受け持つ。丸や番号の縦位置を本文と揃えるためである。
        if !fence {
            let t = line.trim_start();
            let deep = line.len() - t.len() < 8;
            if deep && crate::mdlist::is_item(line) {
                let start_line = line_no;
                let mut block = String::from(line);
                block.push('\n');
                while let Some(next) = lines.peek() {
                    let nt = next.trim_start();
                    let ndeep = next.len() - nt.len() < 8;
                    if ndeep && crate::mdlist::is_item(next) {
                        block.push_str(next);
                        block.push('\n');
                        lines.next();
                        line_no += 1;
                    } else {
                        break;
                    }
                }
                if let Some(list) = crate::mdlist::parse(&block, start_line) {
                    if !buf.trim().is_empty() {
                        out.push(Block::Markdown(std::mem::take(&mut buf)));
                    }
                    buf.clear();
                    out.push(Block::List(list));
                    line_no += 1;
                    continue;
                }
            }
        }

        // 表は自分で受け持つ。本文の幅からはみ出すときに、ここだけ横へ流すためである。
        if !fence && !indented && line.trim_start().starts_with('|') {
            let mut rows = String::from(line);
            rows.push('\n');
            while let Some(next) = lines.peek() {
                if next.trim_start().starts_with('|') {
                    rows.push_str(next);
                    rows.push('\n');
                    lines.next();
                } else {
                    break;
                }
            }
            if !buf.trim().is_empty() {
                out.push(Block::Markdown(std::mem::take(&mut buf)));
            }
            buf.clear();
            out.push(Block::Table(rows));
            line_no += 1;
            continue;
        }

        if !fence {
            buf.push_str(line);
            buf.push('\n');
            line_no += 1;
            continue;
        }

        let marker: String = line.chars().take_while(|c| *c == '`' || *c == '~').collect();
        let info = line[marker.len()..].trim().to_string();
        let lang = info
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();

        let mut src = String::new();
        line_no += 1;
        for l in lines.by_ref() {
            line_no += 1;
            if l.trim_end().starts_with(&marker) && !l.starts_with(' ') {
                break;
            }
            src.push_str(l);
            src.push('\n');
        }

        if !buf.trim().is_empty() {
            out.push(Block::Markdown(std::mem::take(&mut buf)));
        }
        buf.clear();

        if lang == "mermaid" {
            out.push(Block::Mermaid { src });
        } else {
            out.push(Block::Code { lang, src });
        }
    }

    if !buf.trim().is_empty() {
        out.push(Block::Markdown(buf));
    }
    if out.is_empty() {
        out.push(Block::Markdown(String::new()));
    }
    out
}
