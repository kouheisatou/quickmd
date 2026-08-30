# 埋め込みの確認

画像・動画・コードが、それぞれどう出るかを確かめるためのファイルです。

## 画像

1行に画像だけを置いた場合です。右クリックで保存できます。

![青い帯の入った見本](./sample.png)

小さめの画像も並べます。

![二枚目の見本](./sample2.png)

文の途中に置いた画像 ![印](./sample2.png) は、そのまま文の一部として出ます。

## 動画

![動きの見本](./sample.mp4)

## 見つからないファイル

![ここには無いはずの絵](./no_such_image.png)

## コード

言語を指定した場合です。

```rust
fn main() {
    // 速さは、削った分だけ手に入る
    let files: Vec<&str> = vec!["a.md", "b.csv"];
    for f in &files {
        println!("開く: {f}");
    }
}
```

```python
def 起動時間(回数: int) -> float:
    """何度か測って、真ん中の値を返す。"""
    values = [measure() for _ in range(回数)]
    return sorted(values)[len(values) // 2]
```

言語を指定しない場合です。

```
これは説明のための枠です。
色は付けず、そのままの文字で出します。
```

長い行を含む場合は、枠の中だけが横に流れます。

```bash
cargo build --release --target x86_64-pc-windows-msvc --manifest-path native/Cargo.toml --features "" --verbose
```
