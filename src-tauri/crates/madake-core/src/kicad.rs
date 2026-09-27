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
                length_source: Default::default(),
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
        // lib_id → symbol_id (自前エクスポートの"MadakeCAD:<id>" → 静的マッピング → コネクタ/端子台の動的ID)
        let own_id = lib_id
            .strip_prefix(EXPORT_LIB)
            .filter(|id| crate::symbol::resolve_symbol(id).is_some())
            .map(str::to_string);
        let symbol_id: Option<String> = match own_id.as_deref().or_else(|| map_lib_id(lib_id)) {
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


// ---------------------------------------------------------------------------
// エクスポート (.kicad_sch)
// ---------------------------------------------------------------------------

use crate::model::{Entity, Orientation, PaperSize, Sheet};
use crate::symbol::SymbolDef;

/// エクスポート時のライブラリ名接頭辞。`MadakeCAD:relay_coil` のようにsymbol_idをそのまま使う。
/// インポーターはこの接頭辞を見て同じsymbol_idへ戻す (往復で図形が変わらない)。
pub const EXPORT_LIB: &str = "MadakeCAD:";

/// KiCadのファイル形式バージョン (KiCad 9)。
const KICAD_VERSION: &str = "20250114";

/// S式の文字列リテラル (引用符・バックスラッシュ・改行をエスケープ)。
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 数値 (末尾の0を落とした最大4桁小数)。
fn num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

fn xy(p: crate::geometry::Point) -> String {
    format!("(xy {} {})", num(p.x), num(p.y))
}

/// ライブラリシンボルの座標系 (KiCadはY上向き) へ: yを反転する。
fn lib_pt(p: crate::geometry::Point) -> String {
    format!("{} {}", num(p.x), num(-p.y))
}

fn stroke(width: f64, dashed: bool) -> String {
    format!(
        "(stroke (width {}) (type {}))",
        num(width),
        if dashed { "dash" } else { "default" }
    )
}

/// ピンの向き: KiCadの角度はピン線が接続点から本体へ伸びる向き (配線が出る向きの逆)。
fn pin_angle(dir: crate::symbol::PinDir) -> u16 {
    use crate::symbol::PinDir;
    match dir {
        PinDir::Right => 180,
        PinDir::Left => 0,
        PinDir::Up => 270,
        PinDir::Down => 90,
    }
}

/// ライブラリシンボル定義 (図形+ピン) を書く。
fn write_lib_symbol(out: &mut String, def: &SymbolDef) {
    use crate::symbol::Primitive;
    let name = format!("{EXPORT_LIB}{}", def.id);
    out.push_str(&format!(
        "    (symbol {} (pin_numbers hide) (pin_names hide) (exclude_from_sim no) (in_bom yes) (on_board yes)\n",
        esc(&name)
    ));
    out.push_str(&format!(
        "      (property \"Reference\" {} (at 0 0 0) (effects (font (size 1.27 1.27))))\n",
        esc(&def.ref_prefix)
    ));
    out.push_str(&format!(
        "      (property \"Value\" {} (at 0 0 0) (effects (font (size 1.27 1.27))))\n",
        esc(&def.name)
    ));
    // 図形ユニット
    out.push_str(&format!("      (symbol {}\n", esc(&format!("{}_0_1", def.id))));
    for prim in &def.primitives {
        match prim {
            Primitive::Line { pts } => {
                let pts: Vec<String> = pts.iter().map(|p| format!("(xy {})", lib_pt(*p))).collect();
                out.push_str(&format!(
                    "        (polyline (pts {}) {} (fill (type none)))\n",
                    pts.join(" "),
                    stroke(0.254, false)
                ));
            }
            Primitive::Circle { center, r, filled } => out.push_str(&format!(
                "        (circle (center {}) (radius {}) {} (fill (type {})))\n",
                lib_pt(*center),
                num(*r),
                stroke(0.254, false),
                if *filled { "outline" } else { "none" }
            )),
            Primitive::Arc { center, r, start_deg, end_deg } => {
                // 用紙座標(Y下向き)で始点・中点・終点を取り、Y反転してKiCadの3点弧にする
                let mid_deg = start_deg + (end_deg - start_deg) / 2.0;
                let at = |deg: f64| {
                    let (s, c) = deg.to_radians().sin_cos();
                    crate::geometry::Point::new(center.x + r * c, center.y + r * s)
                };
                out.push_str(&format!(
                    "        (arc (start {}) (mid {}) (end {}) {} (fill (type none)))\n",
                    lib_pt(at(*start_deg)),
                    lib_pt(at(mid_deg)),
                    lib_pt(at(*end_deg)),
                    stroke(0.254, false)
                ));
            }
            Primitive::Rect { p1, p2, filled } => out.push_str(&format!(
                "        (rectangle (start {}) (end {}) {} (fill (type {})))\n",
                lib_pt(*p1),
                lib_pt(*p2),
                stroke(0.254, false),
                if *filled { "outline" } else { "none" }
            )),
            Primitive::Text { at, text, height } => out.push_str(&format!(
                "        (text {} (at {} 0) (effects (font (size {} {}))))\n",
                esc(text),
                lib_pt(*at),
                num(*height),
                num(*height)
            )),
        }
    }
    out.push_str("      )\n");
    // ピンユニット (長さ0: 接続点=ピン位置)
    out.push_str(&format!("      (symbol {}\n", esc(&format!("{}_1_1", def.id))));
    for pin in &def.pins {
        out.push_str(&format!(
            "        (pin passive line (at {} {}) (length 0) (name {} (effects (font (size 1.27 1.27)))) (number {} (effects (font (size 1.27 1.27)))))\n",
            lib_pt(pin.at),
            pin_angle(pin.dir),
            esc(&pin.name),
            esc(&pin.number)
        ));
    }
    out.push_str("      )\n");
    out.push_str("    )\n");
}

/// シートをKiCad回路図 (`.kicad_sch`、KiCad 9形式) の文字列にする。
///
/// - 用紙・表題欄・配線 (ポリラインは2点ずつのwireに分割)・ジャンクション・ネットラベル・注記を書く
/// - シンボルは `MadakeCAD:<symbol_id>` として図形とピンをファイル内の`lib_symbols`へ埋め込む
///   (KiCad側にライブラリが無くてもそのまま開ける)。参照記号・型番はReference/Valueプロパティ
/// - 線番はKiCadに無いので、そのネットの最長セグメント上の**ラベル**にする (ネット名として残る)
/// - ハーネス境界は破線の多角形と名前の注記にする (KiCadでは図形扱い)
/// - 回転・ミラーはインポートと同じ規則で書くので、往復で配置が変わらない
pub fn export_kicad_sch(sheet: &Sheet, symbols: &[SymbolDef]) -> String {
    use std::collections::BTreeMap;

    let mut out = String::new();
    out.push_str(&format!(
        "(kicad_sch (version {KICAD_VERSION}) (generator \"madakecad\") (generator_version \"{}\")\n",
        env!("CARGO_PKG_VERSION")
    ));
    out.push_str(&format!("  (uuid {})\n", esc(&sheet.id.to_string())));
    let size = match sheet.size {
        PaperSize::A4 => "A4",
        PaperSize::A3 => "A3",
        PaperSize::A2 => "A2",
        PaperSize::A1 => "A1",
        PaperSize::A0 => "A0",
    };
    match sheet.orientation {
        Orientation::Landscape => out.push_str(&format!("  (paper {})\n", esc(size))),
        Orientation::Portrait => out.push_str(&format!("  (paper {} portrait)\n", esc(size))),
    }
    let tb = &sheet.title_block;
    out.push_str("  (title_block\n");
    out.push_str(&format!("    (title {})\n", esc(&tb.title)));
    out.push_str(&format!("    (date {})\n", esc(&tb.date)));
    out.push_str(&format!("    (rev {})\n", esc(&crate::svg::effective_rev(sheet))));
    out.push_str(&format!("    (company {})\n", esc(&tb.company)));
    out.push_str("  )\n");

    // 使われているシンボル定義だけを埋め込む (id順で決定的)
    let defs: BTreeMap<&str, &SymbolDef> = symbols.iter().map(|d| (d.id.as_str(), d)).collect();
    let mut used: BTreeMap<&str, &SymbolDef> = BTreeMap::new();
    for e in sheet.entities.values() {
        if let Entity::Symbol(inst) = e {
            if let Some(def) = defs.get(inst.symbol_id.as_str()) {
                used.insert(def.id.as_str(), def);
            }
        }
    }
    out.push_str("  (lib_symbols\n");
    for def in used.values() {
        write_lib_symbol(&mut out, def);
    }
    out.push_str("  )\n");

    // ジャンクション
    for e in sheet.entities.values() {
        if let Entity::Junction(j) = e {
            out.push_str(&format!(
                "  (junction (at {} {}) (diameter 0) (color 0 0 0 0) (uuid {}))\n",
                num(j.at.x),
                num(j.at.y),
                esc(&j.id.to_string())
            ));
        }
    }
    // 配線 (2点ずつ)
    for e in sheet.entities.values() {
        if let Entity::Wire(w) = e {
            for (i, seg) in w.points.windows(2).enumerate() {
                out.push_str(&format!(
                    "  (wire (pts {} {}) {} (uuid {}))\n",
                    xy(seg[0]),
                    xy(seg[1]),
                    stroke(0.0, false),
                    esc(&format!("{}-{i}", w.id))
                ));
            }
        }
    }
    // ハーネス境界: 破線の閉多角形+名前
    for e in sheet.entities.values() {
        if let Entity::Harness(h) = e {
            if h.points.len() >= 2 {
                let mut pts: Vec<String> = h.points.iter().map(|p| xy(*p)).collect();
                pts.push(xy(h.points[0]));
                out.push_str(&format!(
                    "  (polyline (pts {}) {} (uuid {}))\n",
                    pts.join(" "),
                    stroke(0.2, true),
                    esc(&h.id.to_string())
                ));
            }
            if !h.name.is_empty() {
                let (minx, miny) = h
                    .points
                    .iter()
                    .fold((f64::MAX, f64::MAX), |(x, y), p| (x.min(p.x), y.min(p.y)));
                out.push_str(&format!(
                    "  (text {} (at {} {} 0) (effects (font (size 2.5 2.5)) (justify left bottom)) (uuid {}))\n",
                    esc(&h.name),
                    num(minx),
                    num(miny - 1.0),
                    esc(&format!("{}-name", h.id))
                ));
            }
        }
    }
    // ネットラベル
    for e in sheet.entities.values() {
        if let Entity::NetLabel(l) = e {
            out.push_str(&format!(
                "  (label {} (at {} {} {}) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid {}))\n",
                esc(&l.name),
                num(l.at.x),
                num(l.at.y),
                l.rotation,
                esc(&l.id.to_string())
            ));
        }
    }
    // 線番 → ラベル (ラベルの無いネットのみ。最長セグメントの中点)
    for net in crate::netlist::extract_netlist(sheet, symbols) {
        if net.label.is_some() {
            continue;
        }
        let Some(no) = net.wire_no.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some((a, b)) = crate::svg::longest_segment(sheet, &net.wire_ids) else { continue };
        let mid = crate::svg::midpoint(&a, &b);
        out.push_str(&format!(
            "  (label {} (at {} {} 0) (effects (font (size 1.27 1.27)) (justify left bottom)))\n",
            esc(no),
            num(mid.x),
            num(mid.y)
        ));
    }
    // 注記
    for e in sheet.entities.values() {
        if let Entity::Text(t) = e {
            out.push_str(&format!(
                "  (text {} (at {} {} {}) (effects (font (size {} {})) (justify left bottom)) (uuid {}))\n",
                esc(&t.text),
                num(t.at.x),
                num(t.at.y),
                t.rotation,
                num(t.height),
                num(t.height),
                esc(&t.id.to_string())
            ));
        }
    }
    // シンボル
    let project_name = sheet.name.as_str();
    for e in sheet.entities.values() {
        let Entity::Symbol(inst) = e else { continue };
        let Some(def) = defs.get(inst.symbol_id.as_str()) else { continue };
        out.push_str(&format!(
            "  (symbol (lib_id {}) (at {} {} {})",
            esc(&format!("{EXPORT_LIB}{}", inst.symbol_id)),
            num(inst.at.x),
            num(inst.at.y),
            inst.rotation
        ));
        if inst.mirror {
            out.push_str(" (mirror y)");
        }
        out.push_str(&format!(
            " (unit 1) (exclude_from_sim no) (in_bom yes) (on_board yes) (dnp no) (uuid {})\n",
            esc(&inst.id.to_string())
        ));
        let top = crate::svg::symbol_top_y(inst, def);
        out.push_str(&format!(
            "    (property \"Reference\" {} (at {} {} 0) (effects (font (size 1.27 1.27))))\n",
            esc(&inst.reference),
            num(inst.at.x),
            num(top - 5.0)
        ));
        out.push_str(&format!(
            "    (property \"Value\" {} (at {} {} 0) (effects (font (size 1.27 1.27))))\n",
            esc(&inst.value),
            num(inst.at.x),
            num(top - 2.5)
        ));
        for (k, v) in &inst.attrs {
            out.push_str(&format!(
                "    (property {} {} (at {} {} 0) (effects (font (size 1.27 1.27)) hide))\n",
                esc(k),
                esc(v),
                num(inst.at.x),
                num(inst.at.y)
            ));
        }
        for pin in &def.pins {
            out.push_str(&format!(
                "    (pin {} (uuid {}))\n",
                esc(&pin.number),
                esc(&format!("{}-{}", inst.id, pin.number))
            ));
        }
        out.push_str(&format!(
            "    (instances (project {} (path {} (reference {}) (unit 1))))\n",
            esc(project_name),
            esc(&format!("/{}", sheet.id)),
            esc(&inst.reference)
        ));
        out.push_str("  )\n");
    }
    out.push_str(")\n");
    out
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

    /// KiCad import converts paper size, title block, wires, junctions, labels (power symbols become net labels) and text.
    /// KiCadインポートは用紙サイズ・表題欄・配線・ジャンクション・ラベル(電源シンボルはネットラベル化)・テキストを変換する。
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

    /// Known lib_ids map to our symbols (Device:R -> resistor, Conn_01x03 -> connector_3p) keeping designator/value/rotation/mirror; unknown symbols are skipped and itemized in the report.
    /// 既知のlib_idは本ライブラリへ対応付けられ(Device:R→抵抗、Conn_01x03→connector_3p)、参照記号・値・回転・ミラーが保たれる。未知のシンボルはスキップされレポートに列挙される。
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

    /// A file that is not a kicad_sch document is rejected with a clear error.
    /// kicad_sch文書でないファイルは明確なエラーで拒否される。
    #[test]
    fn rejects_non_schematic() {
        assert_eq!(
            import_kicad_sch("(kicad_pcb (version 1))", "t").unwrap_err(),
            KicadError::NotSchematic
        );
    }

    /// The S-expression parser reads atoms, quoted strings, numbers and nested lists with typed accessors.
    /// S式パーサはアトム・クォート文字列・数値・入れ子リストを読み、型付きアクセサで取り出せる。
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

    /// Escaped quotes/newlines and multibyte (Japanese) text inside strings parse correctly.
    /// 文字列内のエスケープ(引用符・改行)と日本語などのマルチバイト文字を正しく解釈する。
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

    /// children(name) iterates every child list with the given head symbol.
    /// children(name)は指定した先頭シンボルを持つ子リストをすべて列挙する。
    #[test]
    fn children_iterates_all_matches() {
        let e = parse_sexpr("(root (wire (a)) (junction) (wire (b)))").unwrap();
        assert_eq!(e.children("wire").count(), 2);
        assert_eq!(e.children("junction").count(), 1);
        assert_eq!(e.children("label").count(), 0);
    }

    /// Unbalanced parentheses and unterminated strings are reported as syntax errors with a position.
    /// 括弧の不整合や閉じていない文字列は、位置付きの構文エラーとして報告される。
    #[test]
    fn syntax_errors_are_reported() {
        assert!(matches!(parse_sexpr("(a (b)"), Err(KicadError::Syntax(_, _))));
        assert!(matches!(parse_sexpr("(a)) "), Err(KicadError::Syntax(_, _))));
        assert!(matches!(parse_sexpr(r#"(a "x"#), Err(KicadError::Syntax(_, _))));
    }

    // ---- エクスポート ----

    fn export_fixture() -> Sheet {
        use crate::geometry::Point;
        use uuid::Uuid;
        let mut sheet = Sheet::new("Sheet1", PaperSize::A4, Orientation::Portrait);
        sheet.title_block.title = "動力\"系統\"図".into();
        sheet.title_block.company = "サンプル社".into();
        sheet.title_block.date = "2026-09-27".into();
        let mut push = |e: Entity| {
            sheet.entities.insert(e.id(), e);
        };
        push(Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "relay_coil".into(),
            at: Point::new(100.0, 50.0),
            rotation: 90,
            mirror: true,
            reference: "K1".into(),
            value: "MY2N".into(),
            attrs: [("DESC".to_string(), "主回路".to_string())].into(),
        }));
        push(Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "terminal_block_3p".into(),
            at: Point::new(50.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: String::new(),
            attrs: Default::default(),
        }));
        push(Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(10.0, 10.0), Point::new(40.0, 10.0), Point::new(40.0, 30.0)],
            color: "red".into(),
            sq: 0.75,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: Some("101".into()),
        }));
        push(Entity::Junction(Junction { id: Uuid::new_v4(), at: Point::new(40.0, 10.0) }));
        push(Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(60.0, 80.0),
            name: "24V".into(),
            rotation: 0,
        }));
        push(Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(20.0, 90.0),
            text: "注記 \"A\"\n2行目".into(),
            height: 3.0,
            rotation: 0,
        }));
        push(Entity::Harness(Harness {
            id: Uuid::new_v4(),
            points: vec![
                Point::new(5.0, 5.0),
                Point::new(45.0, 5.0),
                Point::new(45.0, 35.0),
                Point::new(5.0, 35.0),
            ],
            name: "W1".into(),
            note: String::new(),
        }));
        sheet
    }

    fn defs(sheet: &Sheet) -> Vec<SymbolDef> {
        crate::symbol::sheet_symbol_defs(sheet)
    }

    /// Exporting a sheet to .kicad_sch and importing it back keeps paper, title block, symbols (id, reference, value, rotation, mirror), wires, junctions, labels and text.
    /// シートを.kicad_schへ書き出して読み戻すと、用紙・表題欄・シンボル(種類・参照記号・型番・回転・ミラー)・配線・ジャンクション・ラベル・注記が保たれる。
    #[test]
    fn export_then_import_round_trips_the_sheet() {
        let sheet = export_fixture();
        let text = export_kicad_sch(&sheet, &defs(&sheet));
        let (project, report) = import_kicad_sch(&text, "往復").unwrap();
        let back = &project.sheets[0];
        assert_eq!(back.size, PaperSize::A4);
        assert_eq!(back.orientation, Orientation::Portrait);
        assert_eq!(back.title_block.title, "動力\"系統\"図");
        assert_eq!(back.title_block.company, "サンプル社");
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        assert_eq!(report.symbols, 2);
        let k1 = back
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Symbol(s) if s.reference == "K1" => Some(s),
                _ => None,
            })
            .expect("K1");
        assert_eq!(k1.symbol_id, "relay_coil");
        assert_eq!(k1.value, "MY2N");
        assert_eq!(k1.rotation, 90);
        assert!(k1.mirror);
        assert_eq!((k1.at.x, k1.at.y), (100.0, 50.0));
        let tb1 = back
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Symbol(s) if s.reference == "TB1" => Some(s),
                _ => None,
            })
            .expect("TB1");
        assert_eq!(tb1.symbol_id, "terminal_block_3p");
        assert_eq!(report.junctions, 1);
        let texts: Vec<&str> = back
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"注記 \"A\"\n2行目"), "{texts:?}");
    }

    /// A wire drawn as a polyline is exported as one two-point KiCad wire per segment.
    /// 折れ線で描いた配線は、セグメントごとに2点のKiCad wireとして書き出される。
    #[test]
    fn polyline_wires_are_split_into_segments() {
        let sheet = export_fixture();
        let text = export_kicad_sch(&sheet, &defs(&sheet));
        let root = parse_sexpr(&text).unwrap();
        let wires: Vec<_> = root.children("wire").collect();
        assert_eq!(wires.len(), 2);
        for w in &wires {
            assert_eq!(w.child("pts").unwrap().children("xy").count(), 2);
        }
    }

    /// Every symbol used on the sheet is embedded once in lib_symbols with its graphics and pins, so KiCad opens the file without MadakeCAD libraries.
    /// シートで使うシンボルは図形とピンごとにlib_symbolsへ1回だけ埋め込まれ、KiCad側にライブラリが無くても開ける。
    #[test]
    fn used_symbols_are_embedded_in_lib_symbols() {
        let sheet = export_fixture();
        let text = export_kicad_sch(&sheet, &defs(&sheet));
        let root = parse_sexpr(&text).unwrap();
        let libs = root.child("lib_symbols").unwrap();
        let names: Vec<&str> = libs.children("symbol").filter_map(|s| s.arg_str(0)).collect();
        assert_eq!(names, vec!["MadakeCAD:relay_coil", "MadakeCAD:terminal_block_3p"]);
        let tb = libs
            .children("symbol")
            .find(|s| s.arg_str(0) == Some("MadakeCAD:terminal_block_3p"))
            .unwrap();
        let pins = tb
            .children("symbol")
            .flat_map(|unit| unit.children("pin").collect::<Vec<_>>())
            .count();
        assert_eq!(pins, 6, "端子台3極は左右貫通で6ピン");
    }

    /// A wire number becomes a KiCad label on the numbered net, so the net keeps its name in KiCad and on re-import.
    /// 線番はそのネット上のKiCadラベルになり、KiCadでも再インポート後もネット名として残る。
    #[test]
    fn wire_numbers_become_labels() {
        let sheet = export_fixture();
        let text = export_kicad_sch(&sheet, &defs(&sheet));
        let root = parse_sexpr(&text).unwrap();
        let labels: Vec<&str> = root.children("label").filter_map(|l| l.arg_str(0)).collect();
        assert!(labels.contains(&"101"), "{labels:?}");
        assert!(labels.contains(&"24V"), "{labels:?}");
        let (project, _) = import_kicad_sch(&text, "往復").unwrap();
        let nets = crate::netlist::extract_netlist(&project.sheets[0], &[]);
        assert!(nets.iter().any(|n| n.name == "101"), "{:?}", nets.iter().map(|n| &n.name).collect::<Vec<_>>());
    }

    /// A harness boundary is exported as a dashed closed polyline plus a text with its name.
    /// ハーネス境界は破線の閉じた多角形と、その名前の注記として書き出される。
    #[test]
    fn harness_becomes_dashed_polyline_with_name() {
        let sheet = export_fixture();
        let text = export_kicad_sch(&sheet, &defs(&sheet));
        let root = parse_sexpr(&text).unwrap();
        let poly = root.children("polyline").next().expect("polyline");
        assert_eq!(poly.child("pts").unwrap().children("xy").count(), 5, "閉じるため始点を再掲");
        assert_eq!(
            poly.child("stroke").unwrap().child("type").unwrap().arg_str(0),
            Some("dash")
        );
        assert!(root.children("text").any(|t| t.arg_str(0) == Some("W1")));
    }

    /// Quotes, backslashes and newlines in text are escaped so the exported file parses and reads back unchanged.
    /// 注記内の引用符・バックスラッシュ・改行はエスケープされ、書き出したファイルは正しく解釈されて同じ文字列に戻る。
    #[test]
    fn strings_are_escaped_and_read_back() {
        assert_eq!(esc("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        let e = parse_sexpr(&format!("(text {})", esc("a\"b\\c\nd"))).unwrap();
        assert_eq!(e.arg_str(0), Some("a\"b\\c\nd"));
    }

    /// Importing a schematic exported by MadakeCAD resolves "MadakeCAD:<id>" library ids directly, including parametric terminal blocks and connectors.
    /// MadakeCADが書き出した回路図の"MadakeCAD:<id>"ライブラリidは、端子台・コネクタの動的シンボルも含めそのまま同じシンボルに戻る。
    #[test]
    fn madakecad_lib_ids_resolve_directly_on_import() {
        let text = r##"(kicad_sch (version 20250114) (generator "madakecad") (paper "A3")
  (symbol (lib_id "MadakeCAD:connector_12p") (at 10 10 0) (property "Reference" "J1" (at 0 0 0)) (property "Value" "" (at 0 0 0)))
  (symbol (lib_id "MadakeCAD:no_such_symbol") (at 20 10 0) (property "Reference" "X1" (at 0 0 0)) (property "Value" "" (at 0 0 0)))
)"##;
        let (project, report) = import_kicad_sch(text, "t").unwrap();
        let ids: Vec<String> = project.sheets[0]
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Symbol(s) => Some(s.symbol_id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["connector_12p"]);
        assert_eq!(report.skipped, vec!["MadakeCAD:no_such_symbol x1"]);
    }
}
