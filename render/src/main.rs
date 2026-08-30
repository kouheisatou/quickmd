// 配布用のビルドでは黒い窓を出さない。標準入出力は親が渡す管を使うので、
// 窓を持たなくても本体とのやり取りはそのまま動く。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! QuickMD の図と数式を作る係。
//!
//! 本体（ネイティブ）は WebView を持たないので、Mermaid の図と数式だけを
//! この小さなプロセスへ任せる。標準入力で依頼を受け、SVG を標準出力へ返す。
//! 本体の起動には関わらないので、立ち上がりの速さは損なわれない。
//!
//! 依頼（1行に1件）
//!   {"id":1,"kind":"mermaid","src":"flowchart LR\n A-->B","dark":true}
//!   {"id":2,"kind":"math","src":"E = mc^2","display":true}
//! 返事
//!   {"id":1,"ok":true,"svg":"<svg …>"}
//!   {"id":1,"ok":false,"error":"…"}

use std::io::{BufRead, Write};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

const PAGE_TEMPLATE: &str = include_str!("page.html");
const PLAYER_TEMPLATE: &str = include_str!("player.html");
const MERMAID_JS: &str = include_str!("../vendor/mermaid.min.js");
const MATHJAX_JS: &str = include_str!("../vendor/tex-svg.js");

/// ページと2つのライブラリを1枚にまとめる。外のファイルを読みに行かせない。
fn page() -> String {
    PAGE_TEMPLATE
        .replace(
            "<script src=\"vendor:tex-svg.js\"></script>",
            &format!("<script>{MATHJAX_JS}</script>"),
        )
        .replace(
            "<script src=\"vendor:mermaid.min.js\"></script>",
            &format!("<script>{MERMAID_JS}</script>"),
        )
}

enum Msg {
    /// 本体からの依頼をそのまま渡す
    Request(String),
    /// WebView から返ってきた結果
    Reply(String),
    Eof,
}

fn main() -> wry::Result<()> {
    // `--play <ファイル>` のときは、再生のためのウィンドウを出す役に回る。
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--play") {
        if let Some(path) = args.get(i + 1) {
            return play(std::path::PathBuf::from(path));
        }
    }
    render_loop()
}

/// 動画を1本だけ再生するウィンドウ。OS が持つ再生の仕組みをそのまま使う。
fn play(path: std::path::PathBuf) -> wry::Result<()> {
    use tao::event::WindowEvent;

    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "動画".into());

    let event_loop = EventLoopBuilder::<Msg>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title(format!("QuickMD — {name}"))
        .with_inner_size(tao::dpi::LogicalSize::new(880.0, 520.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(320.0, 200.0))
        .build(&event_loop)
        .expect("窓を作れませんでした");

    let url = format!(
        "qmdfile://localhost/{}",
        percent_encode_path(&path.to_string_lossy())
    );
    let html = PLAYER_TEMPLATE.replace("{{SRC}}", &escape_attr(&url));

    let _webview = WebViewBuilder::new()
        .with_html(html)
        .with_custom_protocol("qmdfile".into(), serve_file)
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;")
}

fn percent_encode_path(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for b in p.as_bytes() {
        match b {
            b'/' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            b if b.is_ascii_alphanumeric() => out.push(*b as char),
            b => out.push_str(&format!("%{b:02X}")),
        }
    }
    out.trim_start_matches('/').to_string()
}

fn render_loop() -> wry::Result<()> {
    let event_loop = EventLoopBuilder::<Msg>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // 標準入力は別の流れで読み、イベントとして送り込む
    {
        let proxy = proxy.clone();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                match line {
                    Ok(l) if !l.trim().is_empty() => {
                        let _ = proxy.send_event(Msg::Request(l));
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            let _ = proxy.send_event(Msg::Eof);
        });
    }

    // 画面には出さない。図を作るためだけの窓である。
    let window = WindowBuilder::new()
        .with_visible(false)
        .with_title("quickmd-render")
        .with_inner_size(tao::dpi::LogicalSize::new(1200.0, 900.0))
        .build(&event_loop)
        .expect("窓を作れませんでした");

    let ipc_proxy = proxy.clone();
    let webview = WebViewBuilder::new()
        .with_html(page())
        .with_custom_protocol("qmdfile".into(), serve_file)
        .with_ipc_handler(move |req| {
            let _ = ipc_proxy.send_event(Msg::Reply(req.body().to_string()));
        })
        .build(&window)?;

    let mut ready = false;
    let mut queue: Vec<String> = Vec::new();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {}
            Event::UserEvent(Msg::Request(line)) => {
                if ready {
                    send_to_page(&webview, &line);
                } else {
                    queue.push(line);
                }
            }
            Event::UserEvent(Msg::Reply(body)) => {
                if body == "\"ready\"" || body == "ready" {
                    ready = true;
                    for line in queue.drain(..) {
                        send_to_page(&webview, &line);
                    }
                } else {
                    let mut out = std::io::stdout();
                    let _ = writeln!(out, "{body}");
                    let _ = out.flush();
                }
            }
            Event::UserEvent(Msg::Eof) => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}

fn send_to_page(webview: &wry::WebView, line: &str) {
    let js = format!(
        "window.__render({})",
        serde_json::to_string(line).unwrap_or_else(|_| "\"\"".into())
    );
    let _ = webview.evaluate_script(&js);
}

/// 手元のファイルを WebView へ渡す口。
///
/// `with_html` で作ったページからは `file://` を読めない（別の出所として弾かれる）。
/// そこで `qmdfile://` という名前でこちらが読み、中身を返す。
/// 動画は途中から読む要求（Range）が来るので、そこも受ける。
fn serve_file(
    _id: wry::WebViewId,
    request: wry::http::Request<Vec<u8>>,
) -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
    use std::borrow::Cow;
    use wry::http::{header, Response, StatusCode};

    let deny = |code: StatusCode| {
        Response::builder()
            .status(code)
            .body(Cow::Owned(Vec::new()))
            .unwrap()
    };

    let uri = request.uri().to_string();
    let path = match decode_path(&uri) {
        Some(p) => p,
        None => return deny(StatusCode::BAD_REQUEST),
    };

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return deny(StatusCode::NOT_FOUND),
    };
    let mime = mime_of(&path);
    let len = bytes.len();

    // 途中から読む要求への返事。動画の再生と早送りに要る。
    if let Some(range) = request.headers().get(header::RANGE).and_then(|v| v.to_str().ok()) {
        if let Some((start, end)) = parse_range(range, len) {
            let part = bytes[start..=end].to_vec();
            return Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, mime)
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CONTENT_LENGTH, part.len().to_string())
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {start}-{end}/{len}"),
                )
                .body(Cow::Owned(part))
                .unwrap();
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, len.to_string())
        .body(Cow::Owned(bytes))
        .unwrap()
}

/// `qmdfile://localhost/%2FUsers%2F…` の形から、元の場所を取り出す。
fn decode_path(uri: &str) -> Option<std::path::PathBuf> {
    let rest = uri.split_once("://")?.1;
    let after_host = match rest.find('/') {
        Some(i) => &rest[i + 1..],
        None => rest,
    };
    let raw = after_host.split(['?', '#']).next().unwrap_or(after_host);
    let decoded = percent_decode(raw);
    if decoded.is_empty() {
        return None;
    }
    let p = std::path::PathBuf::from(if decoded.starts_with('/') {
        decoded
    } else {
        format!("/{decoded}")
    });
    p.is_file().then_some(p)
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

fn parse_range(value: &str, len: usize) -> Option<(usize, usize)> {
    let spec = value.strip_prefix("bytes=")?;
    let (a, b) = spec.split_once('-')?;
    let start: usize = if a.is_empty() { 0 } else { a.parse().ok()? };
    let end: usize = if b.is_empty() {
        len.saturating_sub(1)
    } else {
        b.parse().ok()?
    };
    if start > end || start >= len {
        return None;
    }
    Some((start, end.min(len - 1)))
}

fn mime_of(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "mp3" => "audio/mpeg",
        "m4a" | "aac" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
