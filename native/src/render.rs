//! 図と数式を作る係（別のプロセス）とのやり取り。
//!
//! 本体は WebView を持たない。Mermaid の図と数式が本文に出てきて、しかも
//! それが画面に入ったときに初めてこの係を起こす。だから起動の速さには関わらない。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

pub enum Art {
    /// 頼んで待っている
    Waiting,
    /// できた絵
    Ready(egui::TextureHandle),
    Failed(String),
}


#[derive(Default)]
struct Shared {
    done: HashMap<String, Result<serde_json::Value, String>>,
}

pub struct Renderer {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    shared: Arc<Mutex<Shared>>,
    asked: HashMap<String, bool>,
    loaded: HashMap<String, egui::TextureHandle>,
    dead: bool,
}

impl Default for Renderer {
    fn default() -> Self {
        Self {
            child: None,
            stdin: None,
            shared: Arc::new(Mutex::new(Shared::default())),
            asked: HashMap::new(),
            loaded: HashMap::new(),
            dead: false,
        }
    }
}

/// 同じ中身なら同じ名前になるようにする。二度作らないため。
/// 数値ではなく文字にするのは、JSON をまたぐと大きな整数の桁が落ちるからである。
pub fn key(kind: &str, src: &str, dark: bool, display: bool) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    kind.hash(&mut h);
    src.hash(&mut h);
    dark.hash(&mut h);
    display.hash(&mut h);
    format!("{:016x}", h.finish())
}

pub fn helper_path() -> Option<std::path::PathBuf> {
    let name = if cfg!(target_os = "windows") {
        "quickmd-render.exe"
    } else {
        "quickmd-render"
    };
    // 配布したときは本体の隣に置く。開発中は render/target の中を見る。
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(name));
            candidates.push(dir.join("..").join("..").join("..").join("render").join("target").join("release").join(name));
            candidates.push(dir.join("..").join("..").join("..").join("render").join("target").join("debug").join(name));
        }
    }
    candidates.into_iter().find(|p| p.exists())
}

impl Renderer {
    fn ensure_started(&mut self, ctx: &egui::Context) -> bool {
        if self.dead {
            return false;
        }
        if self.child.is_some() {
            return true;
        }
        let Some(exe) = helper_path() else {
            self.dead = true;
            return false;
        };
        let mut child = match Command::new(exe)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => {
                self.dead = true;
                return false;
            }
        };

        self.stdin = child.stdin.take();
        let stdout = child.stdout.take();
        self.child = Some(child);

        if let Some(stdout) = stdout {
            let shared = self.shared.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                        continue;
                    };
                    let Some(id) = v.get("id").and_then(|x| x.as_str()).map(str::to_owned) else {
                        continue;
                    };
                    let entry = if v.get("ok").and_then(|x| x.as_bool()) == Some(true) {
                        Ok(v.clone())
                    } else {
                        Err(v
                            .get("error")
                            .and_then(|x| x.as_str())
                            .unwrap_or("作れませんでした")
                            .to_string())
                    };
                    shared.lock().unwrap().done.insert(id, entry);
                    ctx.request_repaint();
                }
            });
        }
        true
    }

    /// 図や数式を頼む。すでに頼んであれば、その状態を返す。
    pub fn ask(
        &mut self,
        ctx: &egui::Context,
        kind: &str,
        src: &str,
        dark: bool,
        display: bool,
        em: f32,
    ) -> Art {
        let id = key(kind, src, dark, display);

        if let Some(tex) = self.loaded.get(&id) {
            return Art::Ready(tex.clone());
        }
        if let Some(result) = self.shared.lock().unwrap().done.remove(&id) {
            return match result {
                Ok(v) => match rasterize(
                    v.get("svg").and_then(|x| x.as_str()).unwrap_or(""),
                    ctx.pixels_per_point(),
                ) {
                    Ok(image) => {
                        let tex = ctx.load_texture(
                            format!("qmd-{id}"),
                            image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.loaded.insert(id, tex.clone());
                        Art::Ready(tex)
                    }
                    Err(e) => {
                        self.asked.insert(id.clone(), false);
                        Art::Failed(e)
                    }
                },
                Err(e) => {
                    self.asked.insert(id.clone(), false);
                    Art::Failed(e)
                }
            };
        }
        if self.asked.get(&id) == Some(&true) {
            return Art::Waiting;
        }
        if !self.ensure_started(ctx) {
            return Art::Failed("図を作る係を起こせませんでした".into());
        }

        let req = serde_json::json!({
            "id": id,
            "kind": kind,
            "src": src,
            "dark": dark,
            "display": display,
            "ex": em * 0.47,
        });
        if let Some(stdin) = self.stdin.as_mut() {
            if writeln!(stdin, "{req}").is_ok() && stdin.flush().is_ok() {
                self.asked.insert(id.clone(), true);
                return Art::Waiting;
            }
        }
        self.dead = true;
        Art::Failed("図を作る係へ渡せませんでした".into())
    }

    /// 動画の1コマを頼む。画面に入ったときだけ呼ぶ。
    pub fn ask_thumb(&mut self, ctx: &egui::Context, path: &str) -> Art {
        let id = key("thumb", path, false, false);

        if let Some(tex) = self.loaded.get(&id) {
            return Art::Ready(tex.clone());
        }
        if let Some(result) = self.shared.lock().unwrap().done.remove(&id) {
            return match result {
                Ok(v) => {
                    let b64 = v.get("png").and_then(|x| x.as_str()).unwrap_or("");
                    match decode_png(b64) {
                        Ok(image) => {
                            let tex = ctx.load_texture(
                                format!("qmd-{id}"),
                                image,
                                egui::TextureOptions::LINEAR,
                            );
                            self.loaded.insert(id, tex.clone());
                            Art::Ready(tex)
                        }
                        Err(e) => {
                            self.asked.insert(id, false);
                            Art::Failed(e)
                        }
                    }
                }
                Err(e) => {
                    self.asked.insert(id, false);
                    Art::Failed(e)
                }
            };
        }
        if self.asked.get(&id) == Some(&true) {
            return Art::Waiting;
        }
        if !self.ensure_started(ctx) {
            return Art::Failed("動画の係を起こせませんでした".into());
        }
        let req = serde_json::json!({ "id": id, "kind": "thumb", "src": path, "at": 0.3 });
        if let Some(stdin) = self.stdin.as_mut() {
            if writeln!(stdin, "{req}").is_ok() && stdin.flush().is_ok() {
                self.asked.insert(id, true);
                return Art::Waiting;
            }
        }
        self.dead = true;
        Art::Failed("動画の係へ渡せませんでした".into())
    }

    /// できあがっているかだけを見る（頼まない）。
    pub fn peek(&self, kind: &str, src: &str, dark: bool, display: bool) -> Option<egui::TextureHandle> {
        self.loaded.get(&key(kind, src, dark, display)).cloned()
    }

    /// 設定やテーマが変わったら、作り直させる。
    pub fn forget_all(&mut self) {
        self.asked.clear();
        self.loaded.clear();
        self.shared.lock().unwrap().done.clear();
    }
}

/// 受け取った PNG を絵に変える。
fn decode_png(b64: &str) -> Result<egui::ColorImage, String> {
    let bytes = base64_decode(b64).ok_or("受け取った絵を読めませんでした")?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        img.as_raw(),
    ))
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut rev = [255u8; 256];
    for (i, c) in TABLE.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for c in s.bytes() {
        if c == b'=' || c == b'\n' || c == b'\r' {
            continue;
        }
        let v = rev[c as usize];
        if v == 255 {
            return None;
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// SVG を絵に変える。フォントを読ませないと、図の中の文字が消える。
fn rasterize(svg: &str, ppp: f32) -> Result<egui::ColorImage, String> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    opt.font_family = if cfg!(target_os = "windows") {
        "Yu Gothic UI".to_string()
    } else if cfg!(target_os = "macos") {
        "Hiragino Sans".to_string()
    } else {
        "Noto Sans CJK JP".to_string()
    };

    let tree = resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| e.to_string())?;
    let size = tree.size();
    let scale = ppp.clamp(1.0, 3.0);
    let w = ((size.width() * scale).ceil() as u32).max(1);
    let h = ((size.height() * scale).ceil() as u32).max(1);
    if w > 8192 || h > 8192 {
        return Err("図が大きすぎます".into());
    }

    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).ok_or("絵の置き場を作れません")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        pixmap.data(),
    ))
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.stdin.take();
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
        }
    }
}
