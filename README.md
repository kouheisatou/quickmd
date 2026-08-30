# QuickMD

マークダウンと CSV を、**読むためだけに**開くアプリです。編集はしません。

Windows のメモ帳と競える速さで立ち上がることを狙って作りました。WebView を使わず、
Rust から GPU へ直に描いています。

```
起動してから本文が画面に出るまで  220 ms
（Intel Mac mini・配布用ビルド・8回の中央値）
```

## できること

| | |
|---|---|
| マークダウン | 見出し・箇条書き・表・引用・脚注・タスクリスト・打ち消し |
| 数式 | `$…$` と `$$…$$` を組版して描く |
| Mermaid | フローチャート・順序図などを図にする |
| 画像 | PNG・JPEG・GIF・WebP・BMP・SVG。右クリックで保存できる |
| 動画 | ひとコマをサムネにして出す。押すと別のウィンドウで再生する |
| 音声 | 札で示し、既定のアプリへ渡す |
| CSV・TSV | 表として開く。UTF-8 と Shift_JIS を自動で見分ける |
| コード | 等幅で表示し、右上のボタンで写し取れる |
| 読みかけの位置 | ファイルごとに覚え、開き直すと続きから |
| リンク | マークダウンと CSV は新しいウィンドウ、フォルダはその場所、それ以外は既定のアプリ |

編集はこのアプリでは行いません。`E` を押すと、設定で決めたアプリがそのファイルを開きます。

## 速さのための作り

起動を遅くする仕事は、起動時に一切しません。

- **WebView を使わない。** ここが最も大きく、実測で 300 ms 近い差になりました。
- **数式・Mermaid・動画のひとコマは、別のプロセスに任せる。** それらが本文に出てきて、しかも
  画面に入ったときだけ `quickmd-render` を起こします。ふつうの文書では一度も動きません。
- **読みかけの位置を覚える。** 同じファイルを開き直すと続きから読めます。

## 使い方

```
quickmd path/to/file.md
```

| キー | 動き |
|---|---|
| `E` | 編集するアプリで開く |
| `O` | ファイルのある場所を開く |
| `R` | 読み直す |
| `Ctrl` / `⌘` + `,` | 設定を開く |
| `Ctrl` / `⌘` + `N` | 新しいウィンドウ |
| `Ctrl` / `⌘` + `O` | 開く |
| `Ctrl` / `⌘` + `W` | 閉じる |
| `Q` / `Esc` | 閉じる |

macOS では画面いちばん上のメニューバーに、それ以外ではウィンドウの中の帯に、
同じ項目が並びます。

## ビルド

Rust が要ります（1.90 以降）。

```bash
cargo build --release --manifest-path native/Cargo.toml
cargo build --release --manifest-path render/Cargo.toml
```

`quickmd-render` は、図・数式・動画のときだけ呼ばれる補助のプログラムです。
本体と同じ場所に置いてください。無くても本文とコードと表は読めます。

起動時間は次のように測れます。

```bash
python3 tools/bench.py examples/basic.md -n 10 --release
```

Windows でビルドするには、Rust の MSVC ツールチェーンと Visual Studio の Build Tools、
そして WebView2 ランタイム（Windows 11 には最初から入っています）が要ります。
WebView2 は `quickmd-render` だけが使います。

## 構成

| 場所 | 中身 |
|---|---|
| `native/` | 本体。ウィンドウ・描画・設定 |
| `render/` | 図・数式・動画のひとコマを作る係。標準入出力で本体とやり取りする |
| `examples/` | 表示を確かめるための見本。`full.md` に書き方をひととおり並べている |
| `tools/bench.py` | 起動時間を測る |
| `.github/workflows/` | Windows・macOS・Linux 向けのビルド |

## ライセンス

MIT

## 同梱しているもの

`render/vendor/` に次の2つを置いています。数式と図が本文に出てきたときだけ読み込みます。

| | ライセンス |
|---|---|
| [MathJax](https://github.com/mathjax/MathJax) | Apache License 2.0 |
| [Mermaid](https://github.com/mermaid-js/mermaid) | MIT |
