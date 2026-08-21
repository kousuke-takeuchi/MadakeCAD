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

/// インポート結果の要約。UI/CLIでユーザーへ報告する。
#[derive(Debug, Default, serde::Serialize, schemars::JsonSchema)]
pub struct ImportReport {
    pub symbols: usize,
    pub wires: usize,
    pub junctions: usize,
    pub labels: usize,
    pub texts: usize,
    /// 未対応lib_idでスキップしたシンボル (例: "Device:Q_NPN_BCE x2")。
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
}

/// KiCadのlib_id → 本ライブラリのsymbol_id。動的シンボル(コネクタ・端子台)はNone側で処理。
fn map_lib_id(lib_id: &str) -> Option<&'static str> {
    let name = lib_id.split(':').nth(1).unwrap_or(lib_id);
    match name {
        "R" | "R_Small" | "R_US" | "R_Small_US" => Some("resistor"),
        "C" | "C_Small" | "C_Polarized" | "C_Polarized_Small" => Some("capacitor"),
        "D" | "D_Small" => Some("diode"),
        "LED" | "LED_Small" => Some("led"),
        "Fuse" | "Fuse_Small" | "Polyfuse" | "Polyfuse_Small" => Some("fuse"),
        "Lamp" => Some("lamp"),
        "Battery" | "Battery_Cell" => Some("battery"),
        "SW_SPST" | "SW_DIP_x01" => Some("switch_spst"),
        "SW_Push" => Some("pushbutton_no"),
        _ if lib_id.starts_with("Motor:") => Some("motor"),
        _ if lib_id.starts_with("Relay:") => Some("relay_coil"),
        _ => None,
    }
}

/// `Conn_01x03` / `Screw_Terminal_01x08` 等からピン数を読む。
fn pin_count_of(name: &str) -> Option<usize> {
    let idx = name.rfind("01x")?;
    let digits: String = name[idx + 3..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let n: usize = digits.parse().ok()?;
    (1..=crate::symbol::DYNAMIC_PIN_MAX).contains(&n).then_some(n)
}

/// (at x y angle) を読む。
fn read_at(e: &SExpr) -> Option<(f64, f64, f64)> {
    let at = e.child("at")?;
    Some((at.arg_num(0)?, at.arg_num(1)?, at.arg_num(2).unwrap_or(0.0)))
}

/// `.kicad_sch`の中身をProjectへ変換する。
pub fn import_kicad_sch(
    input: &str,
    project_name: &str,
) -> Result<(crate::model::Project, ImportReport), KicadError> {
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    let root = parse_sexpr(input)?;
    if root.name() != Some("kicad_sch") {
        return Err(KicadError::NotSchematic);
    }
    let mut report = ImportReport::default();

    // 用紙: (paper "A3") または (paper "A4" portrait)
    let (size, orientation) = match root.child("paper") {
        Some(paper) => {
            let size = match paper.arg_str(0) {
                Some("A4") => PaperSize::A4,
                Some("A3") => PaperSize::A3,
                Some("A2") => PaperSize::A2,
                Some("A1") => PaperSize::A1,
                Some("A0") => PaperSize::A0,
                other => {
                    report.warnings.push(format!(
                        "未対応の用紙サイズ {:?} のためA3にしました",
                        other.unwrap_or("?")
                    ));
                    PaperSize::A3
                }
            };
            let portrait = matches!(paper.arg_str(1), Some("portrait"));
            (
                size,
                if portrait { Orientation::Portrait } else { Orientation::Landscape },
            )
        }
        None => (PaperSize::A3, Orientation::Landscape),
    };
    let mut sheet = Sheet::new(project_name, size, orientation);

    if let Some(tb) = root.child("title_block") {
        let get = |name: &str| tb.child(name).and_then(|c| c.arg_str(0)).unwrap_or("").to_string();
        sheet.title_block.title = get("title");
        sheet.title_block.date = get("date");
        sheet.title_block.rev = get("rev");
        sheet.title_block.company = get("company");
    }

    let push = |sheet: &mut Sheet, e: Entity| {
        sheet.entities.insert(e.id(), e);
    };

    // ワイヤ
    for wire in root.children("wire") {
        let Some(pts) = wire.child("pts") else { continue };
        let points: Vec<Point> = pts
            .children("xy")
            .filter_map(|xy| Some(Point::new(xy.arg_num(0)?, xy.arg_num(1)?)))
            .collect();
        if points.len() < 2 {
            continue;
        }
        push(
            &mut sheet,
            Entity::Wire(Wire {
                id: Uuid::new_v4(),
                points,
                color: "black".into(),
                sq: 0.0,
                length_m: None,
                part_no: None,
                net: None,
            }),
        );
        report.wires += 1;
    }
    // ジャンクション
    for j in root.children("junction") {
        let Some((x, y, _)) = read_at(j) else { continue };
        push(
            &mut sheet,
            Entity::Junction(Junction {
                id: Uuid::new_v4(),
                at: Point::new(x, y),
            }),
        );
        report.junctions += 1;
    }
    // ラベル (local/global/hierarchical) → NetLabel
    for kind in ["label", "global_label", "hierarchical_label"] {
        for l in root.children(kind) {
            let (Some(name), Some((x, y, angle))) = (l.arg_str(0), read_at(l)) else { continue };
            push(
                &mut sheet,
                Entity::NetLabel(NetLabel {
                    id: Uuid::new_v4(),
                    at: Point::new(x, y),
                    name: name.to_string(),
                    rotation: (angle.rem_euclid(360.0) as u16) % 360,
                }),
            );
            report.labels += 1;
        }
    }
    // テキスト
    for t in root.children("text") {
        let (Some(text), Some((x, y, angle))) = (t.arg_str(0), read_at(t)) else { continue };
        let height = t
            .child("effects")
            .and_then(|e| e.child("font"))
            .and_then(|f| f.child("size"))
            .and_then(|s| s.arg_num(0))
            .unwrap_or(2.5);
        push(
            &mut sheet,
            Entity::Text(TextEntity {
                id: Uuid::new_v4(),
                at: Point::new(x, y),
                text: text.to_string(),
                height,
                rotation: (angle.rem_euclid(360.0) as u16) % 360,
            }),
        );
        report.texts += 1;
    }
    // シンボル
    let mut skipped: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for sym in root.children("symbol") {
        let Some(lib_id) = sym.child("lib_id").and_then(|l| l.arg_str(0)) else { continue };
        let Some((x, y, angle)) = read_at(sym) else { continue };
        let prop = |name: &str| -> String {
            sym.children("property")
                .find(|p| p.arg_str(0) == Some(name))
                .and_then(|p| p.arg_str(1))
                .unwrap_or("")
                .to_string()
        };
        // 電源シンボル: GND系はNetLabel "GND"、その他はValue名のNetLabel (KiCadの電位定義に相当)
        if lib_id.starts_with("power:") {
            let name = if lib_id["power:".len()..].starts_with("GND") {
                "GND".to_string()
            } else {
                let v = prop("Value");
                if v.is_empty() { lib_id["power:".len()..].to_string() } else { v }
            };
            push(
                &mut sheet,
                Entity::NetLabel(NetLabel {
                    id: Uuid::new_v4(),
                    at: Point::new(x, y),
                    name,
                    rotation: 0,
                }),
            );
            report.labels += 1;
            continue;
        }
        // lib_id → symbol_id (静的マッピング → コネクタ/端子台の動的ID)
        let symbol_id: Option<String> = match map_lib_id(lib_id) {
            Some(id) => Some(id.to_string()),
            None => {
                let name = lib_id.split(':').nth(1).unwrap_or(lib_id);
                pin_count_of(name).map(|n| {
                    if name.contains("Screw_Terminal") || name.contains("TerminalBlock") {
                        format!("terminal_block_{n}p")
                    } else {
                        format!("connector_{n}p")
                    }
                })
            }
        };
        let Some(symbol_id) = symbol_id else {
            *skipped.entry(lib_id.to_string()).or_insert(0) += 1;
            continue;
        };
        let rotation = angle.rem_euclid(360.0) as u16;
        let rotation = if rotation % 90 == 0 {
            rotation % 360
        } else {
            report
                .warnings
                .push(format!("{lib_id}: 回転角 {angle} を0度に丸めました"));
            0
        };
        push(
            &mut sheet,
            Entity::Symbol(SymbolInstance {
                id: Uuid::new_v4(),
                symbol_id,
                at: Point::new(x, y),
                rotation,
                mirror: sym.child("mirror").is_some(),
                reference: prop("Reference"),
                value: prop("Value"),
                attrs: Default::default(),
            }),
        );
        report.symbols += 1;
    }
    for (lib_id, count) in skipped {
        report.skipped.push(format!("{lib_id} x{count}"));
    }
    if !report.skipped.is_empty() {
        report.warnings.push(
            "未対応シンボルはスキップしました。手動で置き直してください".into(),
        );
    }
    report.warnings.push(
        "KiCadと本ライブラリでピン形状が異なるため、接続は検証(ERC)で確認してください".into(),
    );

    let mut project = Project::new(project_name);
    project.sheets = vec![sheet];
    Ok((project, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    const FIXTURE: &str = r##"(kicad_sch (version 20250114) (generator "eeschema")
  (paper "A3")
  (title_block (title "テスト回路") (date "2026-08-21") (rev "B") (company "サンプル社"))
  (lib_symbols (symbol "Device:R" (pin_numbers hide)))
  (wire (pts (xy 100 50) (xy 150 50)) (stroke (width 0)) (uuid "w1"))
  (wire (pts (xy 150 50) (xy 150 80)))
  (junction (at 150 50) (diameter 0) (uuid "j1"))
  (label "24V" (at 120 50 0) (effects (font (size 1.27 1.27))))
  (global_label "0V" (at 150 80 0))
  (text "注記です" (at 60 90 0) (effects (font (size 2 2))))
  (symbol (lib_id "Device:R") (at 100 50 90) (mirror x)
    (property "Reference" "R1" (at 0 0 0))
    (property "Value" "10k" (at 0 0 0)))
  (symbol (lib_id "Connector_Generic:Conn_01x03") (at 200 60 0)
    (property "Reference" "J3" (at 0 0 0))
    (property "Value" "CN" (at 0 0 0)))
  (symbol (lib_id "power:GND") (at 150 90 0)
    (property "Reference" "#PWR01" (at 0 0 0))
    (property "Value" "GND" (at 0 0 0)))
  (symbol (lib_id "power:+24V") (at 100 30 0)
    (property "Reference" "#PWR02" (at 0 0 0))
    (property "Value" "+24V" (at 0 0 0)))
  (symbol (lib_id "Device:Q_NPN_BCE") (at 60 60 0)
    (property "Reference" "Q1" (at 0 0 0))
    (property "Value" "2N2222" (at 0 0 0)))
)"##;

    fn entities_of<'a>(sheet: &'a Sheet) -> Vec<&'a Entity> {
        sheet.entities.values().collect()
    }

    #[test]
    fn imports_paper_title_block_and_geometry() {
        let (project, report) = import_kicad_sch(FIXTURE, "テスト").unwrap();
        assert_eq!(project.name, "テスト");
        let sheet = &project.sheets[0];
        assert_eq!(sheet.size, PaperSize::A3);
        assert_eq!(sheet.title_block.title, "テスト回路");
        assert_eq!(sheet.title_block.rev, "B");
        assert_eq!(sheet.title_block.company, "サンプル社");
        let es = entities_of(sheet);
        assert_eq!(es.iter().filter(|e| matches!(e, Entity::Wire(_))).count(), 2);
        assert_eq!(report.wires, 2);
        assert_eq!(
            es.iter().filter(|e| matches!(e, Entity::Junction(_))).count(),
            1
        );
        // label + global_label + power:GND + power:+24V → NetLabel 4件
        let labels: Vec<&str> = es
            .iter()
            .filter_map(|e| match e {
                Entity::NetLabel(l) => Some(l.name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(labels.len(), 4, "{labels:?}");
        assert!(labels.contains(&"24V") && labels.contains(&"0V"));
        assert!(labels.contains(&"GND") && labels.contains(&"+24V"));
        assert_eq!(
            es.iter().filter(|e| matches!(e, Entity::Text(_))).count(),
            1
        );
    }

    #[test]
    fn maps_symbols_and_reports_skipped() {
        let (project, report) = import_kicad_sch(FIXTURE, "t").unwrap();
        let sheet = &project.sheets[0];
        let syms: Vec<&SymbolInstance> = sheet
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Symbol(s) => Some(s),
                _ => None,
            })
            .collect();
        // R1 (resistor, 90度回転+ミラー) と J3 (connector_3p)。電源シンボルはNetLabel化
        assert_eq!(syms.len(), 2, "{syms:?}");
        let r1 = syms.iter().find(|s| s.reference == "R1").unwrap();
        assert_eq!(r1.symbol_id, "resistor");
        assert_eq!(r1.value, "10k");
        assert_eq!(r1.rotation, 90);
        assert!(r1.mirror);
        assert!((r1.at.x - 100.0).abs() < 1e-9 && (r1.at.y - 50.0).abs() < 1e-9);
        let j3 = syms.iter().find(|s| s.reference == "J3").unwrap();
        assert_eq!(j3.symbol_id, "connector_3p");
        // 未対応 Device:Q_NPN_BCE はスキップ報告
        assert_eq!(report.symbols, 2);
        assert!(
            report.skipped.iter().any(|s| s.contains("Device:Q_NPN_BCE")),
            "{:?}",
            report.skipped
        );
    }

    #[test]
    fn rejects_non_schematic() {
        assert_eq!(
            import_kicad_sch("(kicad_pcb (version 1))", "t").unwrap_err(),
            KicadError::NotSchematic
        );
    }

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
