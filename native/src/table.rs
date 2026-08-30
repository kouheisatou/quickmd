//! CSV と TSV を表として描く。日本語のファイルは Shift_JIS のことが多いので、文字コードを見分ける。

/// 表示する行の上限。これを超えると描くのに時間がかかり、速さの狙いから外れる。
const MAX_ROWS: usize = 50_000;

pub struct Table {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub cols: usize,
    pub encoding: String,
    pub delimiter: char,
    pub truncated: bool,
}

impl Table {
    pub fn parse(bytes: &[u8], ext: &str, forced: &str) -> Self {
        let (text, encoding) = decode(bytes, forced);
        let delimiter = detect_delimiter(&text, ext);

        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter as u8)
            .flexible(true)
            .has_headers(false)
            .from_reader(text.as_bytes());

        let mut records: Vec<Vec<String>> = Vec::new();
        let mut truncated = false;
        for result in reader.records() {
            if records.len() >= MAX_ROWS {
                truncated = true;
                break;
            }
            if let Ok(r) = result {
                records.push(r.iter().map(|s| s.to_string()).collect());
            }
        }

        let cols = records.iter().map(|r| r.len()).max().unwrap_or(0);
        let header = if records.is_empty() {
            Vec::new()
        } else {
            records.remove(0)
        };

        Self {
            header,
            rows: records,
            cols,
            encoding,
            delimiter,
            truncated,
        }
    }

    pub fn summary(&self) -> String {
        let d = match self.delimiter {
            '\t' => "タブ",
            ';' => "セミコロン",
            '|' => "縦棒",
            _ => "カンマ",
        };
        format!(
            "{} 行 × {} 列／{}／区切り {}",
            self.rows.len() + 1,
            self.cols,
            self.encoding,
            d
        )
    }
}

fn decode(bytes: &[u8], forced: &str) -> (String, String) {
    if !forced.eq_ignore_ascii_case("auto") {
        if let Some(enc) = encoding_rs::Encoding::for_label(forced.as_bytes()) {
            return (enc.decode(bytes).0.into_owned(), enc.name().to_string());
        }
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (
            encoding_rs::UTF_8.decode(bytes).0.into_owned(),
            "UTF-8 (BOM)".to_string(),
        );
    }
    if std::str::from_utf8(bytes).is_ok() {
        return (
            encoding_rs::UTF_8.decode(bytes).0.into_owned(),
            "UTF-8".to_string(),
        );
    }
    let mut d = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
    d.feed(bytes, true);
    let enc = d.guess(None, chardetng::Utf8Detection::Allow);
    (enc.decode(bytes).0.into_owned(), enc.name().to_string())
}

fn detect_delimiter(text: &str, ext: &str) -> char {
    if ext.eq_ignore_ascii_case("tsv") {
        return '\t';
    }
    let head: String = text.lines().take(5).collect::<Vec<_>>().join("\n");
    let mut best = (',', 0usize);
    for c in [',', '\t', ';', '|'] {
        let n = head.matches(c).count();
        if n > best.1 {
            best = (c, n);
        }
    }
    best.0
}

pub fn is_numeric(s: &str) -> bool {
    let t = s.trim().replace(',', "");
    if t.is_empty() {
        return false;
    }
    let t = t.trim_start_matches(['+', '-']).trim_end_matches('%');
    !t.is_empty() && t.parse::<f64>().is_ok()
}
