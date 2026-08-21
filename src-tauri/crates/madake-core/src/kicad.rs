//! KiCadインポート (spec §3.3)。`.kicad_sch`(S式)を本モデルへ変換する。
//!
//! v1の割り切り: ジオメトリ(座標mm・Y下向き)はそのまま、シンボルはlib_idマッピング表で
//! 対応付ける。KiCadと本ライブラリでピン形状が異なるため接続は崩れ得る(ERCで洗い出す運用)。

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum KicadError {
    #[error("S式の構文エラー (位置 {0}): {1}")]
    Syntax(usize, String),
    #[error("kicad_schではありません")]
    NotSchematic,
}

/// S式ノード。
#[derive(Debug, Clone, PartialEq)]
pub enum SExpr {
    /// 裸のアトム (例: kicad_sch, xy)。
    Sym(String),
    /// クォート文字列。
    Str(String),
    /// 数値。
    Num(f64),
    List(Vec<SExpr>),
}

impl SExpr {
    /// リストの先頭シンボル名 (例: `(wire ...)` → "wire")。
    pub fn name(&self) -> Option<&str> {
        match self {
            SExpr::List(items) => match items.first() {
                Some(SExpr::Sym(s)) => Some(s),
                _ => None,
            },
            _ => None,
        }
    }

    /// 子のうち `(name ...)` 形式のリストを全て返す。
    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a SExpr> + 'a {
        let items: &[SExpr] = match self {
            SExpr::List(items) => &items[1..],
            _ => &[],
        };
        items.iter().filter(move |e| e.name() == Some(name))
    }

    /// 子のうち最初の `(name ...)` を返す。
    pub fn child<'a>(&'a self, name: &'a str) -> Option<&'a SExpr> {
        self.children(name).next()
    }

    /// リストのn番目(先頭シンボルを除く)のアトムを文字列として返す。
    pub fn arg_str(&self, n: usize) -> Option<&str> {
        match self {
            SExpr::List(items) => match items.get(n + 1) {
                Some(SExpr::Str(s)) | Some(SExpr::Sym(s)) => Some(s),
                _ => None,
            },
            _ => None,
        }
    }

    /// リストのn番目(先頭シンボルを除く)を数値として返す。
    pub fn arg_num(&self, n: usize) -> Option<f64> {
        match self {
            SExpr::List(items) => match items.get(n + 1) {
                Some(SExpr::Num(v)) => Some(*v),
                _ => None,
            },
            _ => None,
        }
    }
}

/// S式1フォームをパースする (入力全体を消費すること)。
pub fn parse_sexpr(input: &str) -> Result<SExpr, KicadError> {
    let bytes = input.as_bytes();
    let mut pos = 0usize;
    let expr = parse_at(bytes, &mut pos)?;
    skip_ws(bytes, &mut pos);
    if pos != bytes.len() {
        return Err(KicadError::Syntax(pos, "末尾に余分な入力があります".into()));
    }
    Ok(expr)
}

fn skip_ws(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && (bytes[*pos] as char).is_ascii_whitespace() {
        *pos += 1;
    }
}

fn parse_at(bytes: &[u8], pos: &mut usize) -> Result<SExpr, KicadError> {
    skip_ws(bytes, pos);
    match bytes.get(*pos) {
        None => Err(KicadError::Syntax(*pos, "入力が途中で終わっています".into())),
        Some(b'(') => {
            *pos += 1;
            let mut items = Vec::new();
            loop {
                skip_ws(bytes, pos);
                match bytes.get(*pos) {
                    None => {
                        return Err(KicadError::Syntax(*pos, "')' がありません".into()));
                    }
                    Some(b')') => {
                        *pos += 1;
                        return Ok(SExpr::List(items));
                    }
                    _ => items.push(parse_at(bytes, pos)?),
                }
            }
        }
        Some(b')') => Err(KicadError::Syntax(*pos, "対応しない ')'".into())),
        Some(b'"') => {
            *pos += 1;
            let mut out = String::new();
            loop {
                match bytes.get(*pos) {
                    None => {
                        return Err(KicadError::Syntax(*pos, "文字列が閉じていません".into()));
                    }
                    Some(b'"') => {
                        *pos += 1;
                        return Ok(SExpr::Str(out));
                    }
                    Some(b'\\') => {
                        *pos += 1;
                        match bytes.get(*pos) {
                            Some(b'n') => out.push('\n'),
                            Some(b't') => out.push('\t'),
                            Some(&c) => out.push(c as char),
                            None => {
                                return Err(KicadError::Syntax(
                                    *pos,
                                    "文字列が閉じていません".into(),
                                ));
                            }
                        }
                        *pos += 1;
                    }
                    Some(_) => {
                        // UTF-8マルチバイトをそのまま写す
                        let start = *pos;
                        let s = &bytes[start..];
                        let ch_len = utf8_len(s[0]);
                        let chunk = std::str::from_utf8(&s[..ch_len.min(s.len())])
                            .map_err(|_| KicadError::Syntax(*pos, "不正なUTF-8".into()))?;
                        out.push_str(chunk);
                        *pos += ch_len;
                    }
                }
            }
        }
        Some(_) => {
            let start = *pos;
            while *pos < bytes.len() {
                let c = bytes[*pos];
                if (c as char).is_ascii_whitespace() || c == b'(' || c == b')' || c == b'"' {
                    break;
                }
                *pos += 1;
            }
            let token = std::str::from_utf8(&bytes[start..*pos])
                .map_err(|_| KicadError::Syntax(start, "不正なUTF-8".into()))?;
            match token.parse::<f64>() {
                Ok(v) => Ok(SExpr::Num(v)),
                Err(_) => Ok(SExpr::Sym(token.to_string())),
            }
        }
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_atoms_strings_numbers_and_nesting() {
        let e = parse_sexpr(r#"(kicad_sch (version 20250114) (paper "A4") (at 12.7 -25.4 90))"#)
            .unwrap();
        assert_eq!(e.name(), Some("kicad_sch"));
        assert_eq!(e.child("version").unwrap().arg_num(0), Some(20250114.0));
        assert_eq!(e.child("paper").unwrap().arg_str(0), Some("A4"));
        let at = e.child("at").unwrap();
        assert_eq!(at.arg_num(0), Some(12.7));
        assert_eq!(at.arg_num(1), Some(-25.4));
        assert_eq!(at.arg_num(2), Some(90.0));
    }

    #[test]
    fn parses_escaped_strings_and_multibyte() {
        let e = parse_sexpr(r#"(title_block (title "動力\"系統\"図") (comment 1 "改訂\nA"))"#)
            .unwrap();
        assert_eq!(
            e.child("title").unwrap().arg_str(0),
            Some("動力\"系統\"図")
        );
        assert_eq!(e.child("comment").unwrap().arg_str(1), Some("改訂\nA"));
    }

    #[test]
    fn children_iterates_all_matches() {
        let e = parse_sexpr("(root (wire (a)) (junction) (wire (b)))").unwrap();
        assert_eq!(e.children("wire").count(), 2);
        assert_eq!(e.children("junction").count(), 1);
        assert_eq!(e.children("label").count(), 0);
    }

    #[test]
    fn syntax_errors_are_reported() {
        assert!(matches!(parse_sexpr("(a (b)"), Err(KicadError::Syntax(_, _))));
        assert!(matches!(parse_sexpr("(a)) "), Err(KicadError::Syntax(_, _))));
        assert!(matches!(parse_sexpr(r#"(a "x"#), Err(KicadError::Syntax(_, _))));
    }
}
