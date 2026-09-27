//! DXF入出力 (AutoCAD Electrical / EPLAN との受け渡し)。
//!
//! ACADEの`.dwg`とEPLANの`.elk`/`.zw1`は非公開のバイナリ形式なので、両製品が読み書きできる
//! 中間形式のDXF (ASCII、AutoCAD 2000形式 = AC1015) で受け渡す。
//!
//! 書き出しの流儀 (ACADEの慣習に寄せる):
//! - 配線は`WIRES`レイヤの`LINE` (ACADEが配線として認識する形)。線番は`WIRENO`レイヤの`TEXT`
//! - シンボルは`MDK_<symbol_id>`ブロックの`INSERT`+属性 (`TAG1`=参照記号、`CAT`=型番、
//!   `DESC1`/`RATING1`=説明・定格、`TERMnn`=ピン番号)。ジャンクションは`WDDOT`ブロック
//! - ネットラベルは`LABELS`、注記は`MISC`、ハーネス境界は`HARNESS`レイヤ (破線の閉多角形)、
//!   用紙枠は`FRAME`レイヤ
//! - 座標: DXFはY上向きなので `y_dxf = 用紙高さ - y`。単位はmm (`$INSUNITS`=4)
//! - 非ASCII文字は `\U+XXXX` で書く (AutoCAD 2000形式の可搬なUnicode表記)
//!
//! 読み込みは自前で書いたDXFのほか、ACADE/EPLANが書き出した一般のDXFも受け付ける:
//! 配線レイヤ (名前に`WIRE`を含む、または指定) の線分→配線、`INSERT`→シンボル (対応する
//! ブロック名だけ。未知のブロックはスキップ報告)、`TEXT`/`MTEXT`→注記または線番。

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::geometry::Point;
use crate::kicad::ImportReport;
use crate::model::*;
use crate::symbol::{Primitive, SymbolDef};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DxfError {
    #[error("DXFの構文エラー (行 {0}): {1}")]
    Syntax(usize, String),
    #[error("DXFファイルではありません (ENTITIESセクションがありません)")]
    NotDxf,
}

/// レイヤ名 (書き出し側の規約。読み込みはこれ以外も受ける)。
pub const LAYER_WIRES: &str = "WIRES";
pub const LAYER_WIRENO: &str = "WIRENO";
pub const LAYER_SYMS: &str = "SYMS";
pub const LAYER_TAGS: &str = "TAGS";
pub const LAYER_DESC: &str = "DESC";
pub const LAYER_LABELS: &str = "LABELS";
pub const LAYER_MISC: &str = "MISC";
pub const LAYER_HARNESS: &str = "HARNESS";
pub const LAYER_FRAME: &str = "FRAME";
/// シンボルブロック名の接頭辞 (`MDK_relay_coil`)。
pub const BLOCK_PREFIX: &str = "MDK_";
/// ジャンクションのブロック名 (ACADEの結線ドットと同名)。
pub const BLOCK_DOT: &str = "WDDOT";
const DOT_RADIUS: f64 = 0.6;
const TEXT_HEIGHT: f64 = 2.5;
/// 線番テキストをこの距離 (mm) 以内の配線へ割り当てる。
const WIRENO_SNAP_MM: f64 = 5.0;

// ---------------------------------------------------------------------------
// 共通: 数値・文字列
// ---------------------------------------------------------------------------

fn num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// 非ASCII文字を `\U+XXXX` にする (AutoCAD 2000形式のテキストの可搬表記)。
pub fn encode_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let cp = c as u32;
        if cp < 0x80 {
            out.push(c);
        } else if cp <= 0xFFFF {
            let _ = write!(out, "\\U+{cp:04X}");
        } else {
            // BMP外はサロゲートペアで書く (AutoCADの解釈に合わせる)
            let mut buf = [0u16; 2];
            for u in c.encode_utf16(&mut buf) {
                let _ = write!(out, "\\U+{u:04X}");
            }
        }
    }
    out
}

/// `\U+XXXX` / `\P` (改行) を戻す。MTEXTの書式コード (`{`, `}`, `\A1;` 等) は取り除く。
pub fn decode_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut units: Vec<u16> = Vec::new();
    let mut chars = s.chars().peekable();
    let flush = |units: &mut Vec<u16>, out: &mut String| {
        if !units.is_empty() {
            out.extend(char::decode_utf16(units.drain(..)).map(|r| r.unwrap_or('\u{FFFD}')));
        }
    };
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek().copied() {
                Some('U') | Some('u') => {
                    let rest: String = chars.clone().take(6).collect();
                    if rest.len() == 6 && &rest[1..2] == "+" {
                        if let Ok(u) = u16::from_str_radix(&rest[2..6], 16) {
                            units.push(u);
                            for _ in 0..6 {
                                chars.next();
                            }
                            continue;
                        }
                    }
                    flush(&mut units, &mut out);
                    out.push('\\');
                }
                Some('P') => {
                    flush(&mut units, &mut out);
                    chars.next();
                    out.push('\n');
                }
                Some('\\') => {
                    flush(&mut units, &mut out);
                    chars.next();
                    out.push('\\');
                }
                Some('{') | Some('}') => {
                    flush(&mut units, &mut out);
                    out.push(chars.next().unwrap());
                }
                Some(code) if code.is_ascii_alphabetic() => {
                    // \A1; \fArial|b0; \H2.5; などの書式コードは`;`まで読み飛ばす
                    flush(&mut units, &mut out);
                    let mut peek = chars.clone();
                    let mut consumed = 0;
                    let mut ended = false;
                    for ch in peek.by_ref() {
                        consumed += 1;
                        if ch == ';' {
                            ended = true;
                            break;
                        }
                        if consumed > 40 {
                            break;
                        }
                    }
                    if ended {
                        for _ in 0..consumed {
                            chars.next();
                        }
                    } else {
                        chars.next();
                    }
                }
                _ => {
                    flush(&mut units, &mut out);
                    out.push('\\');
                }
            }
        } else if c == '{' || c == '}' {
            flush(&mut units, &mut out);
        } else {
            flush(&mut units, &mut out);
            out.push(c);
        }
    }
    flush(&mut units, &mut out);
    out
}

// ---------------------------------------------------------------------------
// 書き出し
// ---------------------------------------------------------------------------

struct Writer {
    out: String,
    next_handle: u64,
}

impl Writer {
    fn new() -> Self {
        Self {
            out: String::new(),
            next_handle: 0x100,
        }
    }
    fn pair(&mut self, code: i32, value: impl std::fmt::Display) {
        let _ = writeln!(self.out, "{code:>3}\n{value}");
    }
    fn handle(&mut self) -> String {
        let h = format!("{:X}", self.next_handle);
        self.next_handle += 1;
        h
    }
    /// エンティティ共通ヘッダ。戻り値=そのハンドル。
    fn entity(&mut self, kind: &str, owner: &str, layer: &str) -> String {
        let h = self.handle();
        self.pair(0, kind);
        self.pair(5, &h);
        self.pair(330, owner);
        self.pair(100, "AcDbEntity");
        self.pair(8, layer);
        h
    }
    fn line(&mut self, owner: &str, layer: &str, a: (f64, f64), b: (f64, f64)) {
        self.entity("LINE", owner, layer);
        self.pair(100, "AcDbLine");
        self.pair(10, num(a.0));
        self.pair(20, num(a.1));
        self.pair(30, 0);
        self.pair(11, num(b.0));
        self.pair(21, num(b.1));
        self.pair(31, 0);
    }
    fn polyline(
        &mut self,
        owner: &str,
        layer: &str,
        pts: &[(f64, f64)],
        closed: bool,
        dashed: bool,
    ) {
        self.entity("LWPOLYLINE", owner, layer);
        if dashed {
            self.pair(6, "DASHED");
        }
        self.pair(100, "AcDbPolyline");
        self.pair(90, pts.len());
        self.pair(70, if closed { 1 } else { 0 });
        self.pair(43, 0);
        for (x, y) in pts {
            self.pair(10, num(*x));
            self.pair(20, num(*y));
        }
    }
    fn circle(&mut self, owner: &str, layer: &str, c: (f64, f64), r: f64) {
        self.entity("CIRCLE", owner, layer);
        self.pair(100, "AcDbCircle");
        self.pair(10, num(c.0));
        self.pair(20, num(c.1));
        self.pair(30, 0);
        self.pair(40, num(r));
    }
    fn arc(&mut self, owner: &str, layer: &str, c: (f64, f64), r: f64, start: f64, end: f64) {
        self.entity("ARC", owner, layer);
        self.pair(100, "AcDbCircle");
        self.pair(10, num(c.0));
        self.pair(20, num(c.1));
        self.pair(30, 0);
        self.pair(40, num(r));
        self.pair(100, "AcDbArc");
        self.pair(50, num(start));
        self.pair(51, num(end));
    }
    fn solid(&mut self, owner: &str, layer: &str, p: [(f64, f64); 4]) {
        self.entity("SOLID", owner, layer);
        self.pair(100, "AcDbTrace");
        // SOLIDの頂点順は 1-2-4-3 (蝶ネクタイにならないように)
        for (i, (x, y)) in [p[0], p[1], p[3], p[2]].iter().enumerate() {
            self.pair(10 + i as i32, num(*x));
            self.pair(20 + i as i32, num(*y));
            self.pair(30 + i as i32, 0);
        }
    }
    /// TEXT。`halign`: 0=左 1=中央 2=右、`valign`: 0=ベースライン 1=下 2=中央 3=上。
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        owner: &str,
        layer: &str,
        at: (f64, f64),
        height: f64,
        rot: f64,
        halign: i32,
        valign: i32,
        text: &str,
    ) {
        self.entity("TEXT", owner, layer);
        self.pair(100, "AcDbText");
        self.pair(10, num(at.0));
        self.pair(20, num(at.1));
        self.pair(30, 0);
        self.pair(40, num(height));
        self.pair(1, encode_text(text));
        self.pair(50, num(rot));
        self.pair(72, halign);
        self.pair(11, num(at.0));
        self.pair(21, num(at.1));
        self.pair(31, 0);
        self.pair(100, "AcDbText");
        self.pair(73, valign);
    }
    #[allow(clippy::too_many_arguments)]
    fn attdef(
        &mut self,
        owner: &str,
        layer: &str,
        at: (f64, f64),
        height: f64,
        tag: &str,
        default: &str,
        invisible: bool,
    ) {
        self.entity("ATTDEF", owner, layer);
        self.pair(100, "AcDbText");
        self.pair(10, num(at.0));
        self.pair(20, num(at.1));
        self.pair(30, 0);
        self.pair(40, num(height));
        self.pair(1, encode_text(default));
        self.pair(72, 1);
        self.pair(11, num(at.0));
        self.pair(21, num(at.1));
        self.pair(31, 0);
        self.pair(100, "AcDbAttributeDefinition");
        self.pair(3, tag);
        self.pair(2, tag);
        self.pair(70, if invisible { 1 } else { 0 });
        self.pair(74, 2);
    }
    #[allow(clippy::too_many_arguments)]
    fn attrib(
        &mut self,
        owner: &str,
        layer: &str,
        at: (f64, f64),
        height: f64,
        rot: f64,
        tag: &str,
        value: &str,
        invisible: bool,
    ) {
        self.entity("ATTRIB", owner, layer);
        self.pair(100, "AcDbText");
        self.pair(10, num(at.0));
        self.pair(20, num(at.1));
        self.pair(30, 0);
        self.pair(40, num(height));
        self.pair(1, encode_text(value));
        self.pair(50, num(rot));
        self.pair(72, 1);
        self.pair(11, num(at.0));
        self.pair(21, num(at.1));
        self.pair(31, 0);
        self.pair(100, "AcDbAttribute");
        self.pair(2, tag);
        self.pair(70, if invisible { 1 } else { 0 });
        self.pair(74, 2);
    }
}

/// レイヤ定義 (名前, 色番号, 線種)。
const LAYERS: &[(&str, u8, &str)] = &[
    ("0", 7, "CONTINUOUS"),
    (LAYER_FRAME, 8, "CONTINUOUS"),
    (LAYER_SYMS, 7, "CONTINUOUS"),
    (LAYER_TAGS, 5, "CONTINUOUS"),
    (LAYER_DESC, 3, "CONTINUOUS"),
    (LAYER_WIRES, 1, "CONTINUOUS"),
    (LAYER_WIRENO, 4, "CONTINUOUS"),
    (LAYER_LABELS, 6, "CONTINUOUS"),
    (LAYER_MISC, 7, "CONTINUOUS"),
    (LAYER_HARNESS, 3, "DASHED"),
];

/// シンボルブロックの図形 (ローカル座標、Y反転済み) を書く。
fn write_symbol_block_body(w: &mut Writer, owner: &str, def: &SymbolDef) {
    let flip = |p: Point| (p.x, -p.y);
    for prim in &def.primitives {
        match prim {
            Primitive::Line { pts } => {
                let pts: Vec<(f64, f64)> = pts.iter().map(|p| flip(*p)).collect();
                if pts.len() == 2 {
                    w.line(owner, LAYER_SYMS, pts[0], pts[1]);
                } else if pts.len() > 2 {
                    w.polyline(owner, LAYER_SYMS, &pts, false, false);
                }
            }
            Primitive::Circle { center, r, filled } => {
                w.circle(owner, LAYER_SYMS, flip(*center), *r);
                if *filled {
                    // 塗り円: 小さい正方形のSOLIDで近似 (ハッチは読み手の対応差が大きい)
                    let (cx, cy) = flip(*center);
                    let d = r * std::f64::consts::FRAC_1_SQRT_2;
                    w.solid(
                        owner,
                        LAYER_SYMS,
                        [
                            (cx - d, cy + d),
                            (cx + d, cy + d),
                            (cx + d, cy - d),
                            (cx - d, cy - d),
                        ],
                    );
                }
            }
            Primitive::Arc {
                center,
                r,
                start_deg,
                end_deg,
            } => {
                // 用紙座標の時計回り (start→end) は、Y反転後は反時計回り (-end→-start)
                let (s, e) = ((-end_deg).rem_euclid(360.0), (-start_deg).rem_euclid(360.0));
                w.arc(owner, LAYER_SYMS, flip(*center), *r, s, e);
            }
            Primitive::Rect { p1, p2, filled } => {
                let pts = [
                    flip(*p1),
                    flip(Point::new(p2.x, p1.y)),
                    flip(*p2),
                    flip(Point::new(p1.x, p2.y)),
                ];
                w.polyline(owner, LAYER_SYMS, &pts, true, false);
                if *filled {
                    w.solid(owner, LAYER_SYMS, pts);
                }
            }
            Primitive::Text { at, text, height } => {
                w.text(owner, LAYER_SYMS, flip(*at), *height, 0.0, 1, 2, text);
            }
        }
    }
    // 属性定義: 参照記号と型番はシンボル上端の上、説明・定格は下端の下 (SVGと同じ配置)
    let (min, max) = crate::symbol::local_bounds(def);
    let cx = (min.x + max.x) / 2.0;
    w.attdef(
        owner,
        LAYER_TAGS,
        (cx, -(min.y - crate::svg::REF_LABEL_DY)),
        TEXT_HEIGHT,
        "TAG1",
        &def.ref_prefix,
        false,
    );
    w.attdef(
        owner,
        LAYER_DESC,
        (cx, -(min.y - crate::svg::VALUE_LABEL_DY)),
        TEXT_HEIGHT,
        "CAT",
        "",
        false,
    );
    w.attdef(
        owner,
        LAYER_DESC,
        (cx, -(max.y + crate::svg::VALUE_LABEL_DY + 2.0)),
        TEXT_HEIGHT,
        "DESC1",
        "",
        false,
    );
    w.attdef(
        owner,
        LAYER_DESC,
        (cx, -(max.y + crate::svg::REF_LABEL_DY + 2.0)),
        TEXT_HEIGHT,
        "RATING1",
        "",
        false,
    );
    for (i, pin) in def.pins.iter().enumerate() {
        w.attdef(
            owner,
            LAYER_SYMS,
            flip(pin.at),
            1.5,
            &format!("TERM{:02}", i + 1),
            &pin.number,
            true,
        );
    }
}

/// シンボルインスタンスの属性値 (タグ, 値, 非表示) を書き出し順に並べる。
fn instance_attributes(inst: &SymbolInstance, def: &SymbolDef) -> Vec<(String, String, bool)> {
    let mut out = vec![
        ("TAG1".to_string(), inst.reference.clone(), false),
        ("CAT".to_string(), inst.value.clone(), false),
        (
            "DESC1".to_string(),
            inst.attrs
                .get(crate::symbol::SLOT_DESC)
                .cloned()
                .unwrap_or_default(),
            false,
        ),
        (
            "RATING1".to_string(),
            inst.attrs
                .get(crate::symbol::SLOT_RATING)
                .cloned()
                .unwrap_or_default(),
            false,
        ),
    ];
    for (k, v) in &inst.attrs {
        if k == crate::symbol::SLOT_DESC || k == crate::symbol::SLOT_RATING {
            continue;
        }
        out.push((k.to_uppercase(), v.clone(), true));
    }
    for (i, pin) in def.pins.iter().enumerate() {
        out.push((format!("TERM{:02}", i + 1), pin.number.clone(), true));
    }
    out
}

/// シートをDXF (AutoCAD 2000形式のASCII) の文字列にする。
pub fn sheet_to_dxf(sheet: &Sheet, symbols: &[SymbolDef]) -> String {
    let (paper_w, paper_h) = sheet.paper_mm();
    let fy = |y: f64| paper_h - y;
    let pt = |p: Point| (p.x, fy(p.y));
    let defs: BTreeMap<&str, &SymbolDef> = symbols.iter().map(|d| (d.id.as_str(), d)).collect();
    let mut used: BTreeMap<&str, &SymbolDef> = BTreeMap::new();
    let mut has_junction = false;
    for e in sheet.entities.values() {
        match e {
            Entity::Symbol(inst) => {
                if let Some(def) = defs.get(inst.symbol_id.as_str()) {
                    used.insert(def.id.as_str(), def);
                }
            }
            Entity::Junction(_) => has_junction = true,
            _ => {}
        }
    }

    let mut w = Writer::new();
    // ---- HEADER ----
    w.pair(0, "SECTION");
    w.pair(2, "HEADER");
    w.pair(9, "$ACADVER");
    w.pair(1, "AC1015");
    w.pair(9, "$DWGCODEPAGE");
    w.pair(3, "ANSI_1252");
    w.pair(9, "$INSUNITS");
    w.pair(70, 4);
    w.pair(9, "$MEASUREMENT");
    w.pair(70, 1);
    w.pair(9, "$EXTMIN");
    w.pair(10, 0);
    w.pair(20, 0);
    w.pair(30, 0);
    w.pair(9, "$EXTMAX");
    w.pair(10, num(paper_w));
    w.pair(20, num(paper_h));
    w.pair(30, 0);
    w.pair(9, "$LIMMIN");
    w.pair(10, 0);
    w.pair(20, 0);
    w.pair(9, "$LIMMAX");
    w.pair(10, num(paper_w));
    w.pair(20, num(paper_h));
    w.pair(9, "$HANDSEED");
    w.pair(5, "FFFFF");
    w.pair(0, "ENDSEC");
    // ---- CLASSES ----
    w.pair(0, "SECTION");
    w.pair(2, "CLASSES");
    w.pair(0, "ENDSEC");
    // ---- TABLES ----
    w.pair(0, "SECTION");
    w.pair(2, "TABLES");
    let table = |w: &mut Writer, name: &str, handle: &str, count: usize| {
        w.pair(0, "TABLE");
        w.pair(2, name);
        w.pair(5, handle);
        w.pair(330, "0");
        w.pair(100, "AcDbSymbolTable");
        w.pair(70, count);
    };
    table(&mut w, "VPORT", "8", 0);
    w.pair(0, "ENDTAB");
    table(&mut w, "LTYPE", "5", 2);
    for (name, desc, pattern) in [
        ("CONTINUOUS", "Solid line", vec![]),
        ("DASHED", "Dashed __ __ __", vec![1.5, -0.5]),
    ] {
        let h = w.handle();
        w.pair(0, "LTYPE");
        w.pair(5, h);
        w.pair(330, "5");
        w.pair(100, "AcDbSymbolTableRecord");
        w.pair(100, "AcDbLinetypeTableRecord");
        w.pair(2, name);
        w.pair(70, 0);
        w.pair(3, desc);
        w.pair(72, 65);
        w.pair(73, pattern.len());
        w.pair(40, num(pattern.iter().map(|d: &f64| d.abs()).sum()));
        for d in pattern {
            w.pair(49, num(d));
            w.pair(74, 0);
        }
    }
    w.pair(0, "ENDTAB");
    table(&mut w, "LAYER", "2", LAYERS.len());
    for (name, color, ltype) in LAYERS {
        let h = w.handle();
        w.pair(0, "LAYER");
        w.pair(5, h);
        w.pair(330, "2");
        w.pair(100, "AcDbSymbolTableRecord");
        w.pair(100, "AcDbLayerTableRecord");
        w.pair(2, name);
        w.pair(70, 0);
        w.pair(62, color);
        w.pair(6, ltype);
        w.pair(370, -3);
        w.pair(390, "F");
    }
    w.pair(0, "ENDTAB");
    table(&mut w, "STYLE", "3", 1);
    {
        let h = w.handle();
        w.pair(0, "STYLE");
        w.pair(5, h);
        w.pair(330, "3");
        w.pair(100, "AcDbSymbolTableRecord");
        w.pair(100, "AcDbTextStyleTableRecord");
        w.pair(2, "STANDARD");
        w.pair(70, 0);
        w.pair(40, 0);
        w.pair(41, 1);
        w.pair(50, 0);
        w.pair(71, 0);
        w.pair(42, num(TEXT_HEIGHT));
        w.pair(3, "txt");
        w.pair(4, "");
    }
    w.pair(0, "ENDTAB");
    table(&mut w, "VIEW", "6", 0);
    w.pair(0, "ENDTAB");
    table(&mut w, "UCS", "7", 0);
    w.pair(0, "ENDTAB");
    table(&mut w, "APPID", "9", 1);
    {
        let h = w.handle();
        w.pair(0, "APPID");
        w.pair(5, h);
        w.pair(330, "9");
        w.pair(100, "AcDbSymbolTableRecord");
        w.pair(100, "AcDbRegAppTableRecord");
        w.pair(2, "ACAD");
        w.pair(70, 0);
    }
    w.pair(0, "ENDTAB");
    w.pair(0, "TABLE");
    w.pair(2, "DIMSTYLE");
    w.pair(5, "A");
    w.pair(330, "0");
    w.pair(100, "AcDbSymbolTable");
    w.pair(70, 0);
    w.pair(100, "AcDbDimStyleTable");
    w.pair(71, 0);
    w.pair(0, "ENDTAB");
    // ブロックレコード: モデル空間・ペーパー空間・シンボル・ドット
    let mut block_names: Vec<String> = used
        .keys()
        .map(|id| format!("{BLOCK_PREFIX}{id}"))
        .collect();
    if has_junction {
        block_names.push(BLOCK_DOT.to_string());
    }
    table(&mut w, "BLOCK_RECORD", "1", 2 + block_names.len());
    let mut record_handles: BTreeMap<String, String> = BTreeMap::new();
    for name in ["*MODEL_SPACE", "*PAPER_SPACE"]
        .iter()
        .map(|s| s.to_string())
        .chain(block_names.iter().cloned())
    {
        let h = w.handle();
        w.pair(0, "BLOCK_RECORD");
        w.pair(5, &h);
        w.pair(330, "1");
        w.pair(100, "AcDbSymbolTableRecord");
        w.pair(100, "AcDbBlockTableRecord");
        w.pair(2, &name);
        record_handles.insert(name, h);
    }
    w.pair(0, "ENDTAB");
    w.pair(0, "ENDSEC");
    // ---- BLOCKS ----
    w.pair(0, "SECTION");
    w.pair(2, "BLOCKS");
    let model_space = record_handles["*MODEL_SPACE"].clone();
    for name in ["*MODEL_SPACE", "*PAPER_SPACE"] {
        let owner = record_handles[name].clone();
        w.entity("BLOCK", &owner, "0");
        w.pair(100, "AcDbBlockBegin");
        w.pair(2, name);
        w.pair(70, 0);
        w.pair(10, 0);
        w.pair(20, 0);
        w.pair(30, 0);
        w.pair(3, name);
        w.pair(1, "");
        w.entity("ENDBLK", &owner, "0");
        w.pair(100, "AcDbBlockEnd");
    }
    for (id, def) in &used {
        let name = format!("{BLOCK_PREFIX}{id}");
        let owner = record_handles[&name].clone();
        w.entity("BLOCK", &owner, "0");
        w.pair(100, "AcDbBlockBegin");
        w.pair(2, &name);
        w.pair(70, 2);
        w.pair(10, 0);
        w.pair(20, 0);
        w.pair(30, 0);
        w.pair(3, &name);
        w.pair(1, encode_text(&def.name));
        write_symbol_block_body(&mut w, &owner, def);
        w.entity("ENDBLK", &owner, "0");
        w.pair(100, "AcDbBlockEnd");
    }
    if has_junction {
        let owner = record_handles[BLOCK_DOT].clone();
        w.entity("BLOCK", &owner, "0");
        w.pair(100, "AcDbBlockBegin");
        w.pair(2, BLOCK_DOT);
        w.pair(70, 0);
        w.pair(10, 0);
        w.pair(20, 0);
        w.pair(30, 0);
        w.pair(3, BLOCK_DOT);
        w.pair(1, "wire dot");
        w.circle(&owner, LAYER_WIRES, (0.0, 0.0), DOT_RADIUS);
        w.entity("ENDBLK", &owner, "0");
        w.pair(100, "AcDbBlockEnd");
    }
    w.pair(0, "ENDSEC");
    // ---- ENTITIES ----
    w.pair(0, "SECTION");
    w.pair(2, "ENTITIES");
    let ms = model_space.as_str();
    // 用紙枠
    w.polyline(
        ms,
        LAYER_FRAME,
        &[
            (0.0, 0.0),
            (paper_w, 0.0),
            (paper_w, paper_h),
            (0.0, paper_h),
        ],
        true,
        false,
    );
    // ハーネス (背面)
    for e in sheet.entities.values() {
        if let Entity::Harness(h) = e {
            if h.points.len() >= 2 {
                let pts: Vec<(f64, f64)> = h.points.iter().map(|p| pt(*p)).collect();
                w.polyline(ms, LAYER_HARNESS, &pts, true, true);
            }
            if !h.name.is_empty() {
                let (minx, miny) = h
                    .points
                    .iter()
                    .fold((f64::MAX, f64::MAX), |(x, y), p| (x.min(p.x), y.min(p.y)));
                w.text(
                    ms,
                    LAYER_HARNESS,
                    (minx, fy(miny - 1.0)),
                    TEXT_HEIGHT,
                    0.0,
                    0,
                    0,
                    &h.name,
                );
            }
        }
    }
    // 配線 (LINEに分割)
    for e in sheet.entities.values() {
        if let Entity::Wire(wire) = e {
            for seg in wire.points.windows(2) {
                w.line(ms, LAYER_WIRES, pt(seg[0]), pt(seg[1]));
            }
        }
    }
    // ジャンクション
    for e in sheet.entities.values() {
        if let Entity::Junction(j) = e {
            w.entity("INSERT", ms, LAYER_WIRES);
            w.pair(100, "AcDbBlockReference");
            w.pair(2, BLOCK_DOT);
            let (x, y) = pt(j.at);
            w.pair(10, num(x));
            w.pair(20, num(y));
            w.pair(30, 0);
        }
    }
    // 線番
    for net in crate::netlist::extract_netlist(sheet, symbols) {
        let Some(no) = net
            .wire_no
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some((a, b)) = crate::svg::longest_segment(sheet, &net.wire_ids) else {
            continue;
        };
        let mid = crate::svg::midpoint(&a, &b);
        if (b.x - a.x).abs() >= (b.y - a.y).abs() {
            w.text(
                ms,
                LAYER_WIRENO,
                (mid.x, fy(mid.y - crate::svg::WIRE_NO_GAP)),
                crate::svg::WIRE_NO_FONT,
                0.0,
                1,
                0,
                no,
            );
        } else {
            w.text(
                ms,
                LAYER_WIRENO,
                (mid.x - crate::svg::WIRE_NO_GAP, fy(mid.y)),
                crate::svg::WIRE_NO_FONT,
                0.0,
                2,
                2,
                no,
            );
        }
    }
    // ネットラベル
    for e in sheet.entities.values() {
        if let Entity::NetLabel(l) = e {
            w.text(
                ms,
                LAYER_LABELS,
                pt(l.at),
                TEXT_HEIGHT,
                f64::from(l.rotation),
                0,
                0,
                &l.name,
            );
        }
    }
    // 注記 (行ごとにTEXT)
    for e in sheet.entities.values() {
        if let Entity::Text(t) = e {
            for (i, line) in t.text.lines().enumerate() {
                let y = t.at.y + i as f64 * t.height * 1.5;
                w.text(
                    ms,
                    LAYER_MISC,
                    (t.at.x, fy(y)),
                    t.height,
                    f64::from(t.rotation),
                    0,
                    0,
                    line,
                );
            }
        }
    }
    // シンボル
    for e in sheet.entities.values() {
        let Entity::Symbol(inst) = e else { continue };
        let Some(def) = defs.get(inst.symbol_id.as_str()) else {
            continue;
        };
        let insert = w.entity("INSERT", ms, LAYER_SYMS);
        w.pair(66, 1);
        w.pair(100, "AcDbBlockReference");
        w.pair(2, format!("{BLOCK_PREFIX}{}", inst.symbol_id));
        let (x, y) = pt(inst.at);
        w.pair(10, num(x));
        w.pair(20, num(y));
        w.pair(30, 0);
        w.pair(41, if inst.mirror { -1 } else { 1 });
        w.pair(42, 1);
        w.pair(43, 1);
        w.pair(50, inst.rotation);
        let top = crate::svg::symbol_top_y(inst, def);
        let (min, max) = crate::svg::symbol_bounds(inst, def);
        let cx = (min.x + max.x) / 2.0;
        for (i, (tag, value, invisible)) in instance_attributes(inst, def).into_iter().enumerate() {
            let (layer, at) = match tag.as_str() {
                "TAG1" => (LAYER_TAGS, (cx, fy(top - crate::svg::REF_LABEL_DY))),
                "CAT" => (LAYER_DESC, (cx, fy(top - crate::svg::VALUE_LABEL_DY))),
                "DESC1" => (
                    LAYER_DESC,
                    (cx, fy(max.y + crate::svg::VALUE_LABEL_DY + 2.0)),
                ),
                "RATING1" => (LAYER_DESC, (cx, fy(max.y + crate::svg::REF_LABEL_DY + 2.0))),
                _ => (LAYER_SYMS, (x, y - i as f64)),
            };
            w.attrib(
                &insert,
                layer,
                at,
                TEXT_HEIGHT,
                0.0,
                &tag,
                &value,
                invisible,
            );
        }
        w.entity("SEQEND", &insert, LAYER_SYMS);
    }
    w.pair(0, "ENDSEC");
    // ---- OBJECTS ----
    w.pair(0, "SECTION");
    w.pair(2, "OBJECTS");
    w.pair(0, "DICTIONARY");
    w.pair(5, "C");
    w.pair(330, "0");
    w.pair(100, "AcDbDictionary");
    w.pair(281, 1);
    w.pair(3, "ACAD_GROUP");
    w.pair(350, "D");
    w.pair(0, "DICTIONARY");
    w.pair(5, "D");
    w.pair(330, "C");
    w.pair(100, "AcDbDictionary");
    w.pair(281, 1);
    w.pair(0, "ENDSEC");
    w.pair(0, "EOF");
    w.out
}

// ---------------------------------------------------------------------------
// 読み込み
// ---------------------------------------------------------------------------

/// 読み込みオプション。
#[derive(Debug, Default, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct DxfImportOptions {
    /// 配線とみなすレイヤ名 (大文字小文字不問)。省略時は名前に`WIRE`を含むレイヤ
    /// (線番の`WIRENO`は除く)。1つも無ければ全ての線分を配線として読む。
    #[serde(default)]
    pub wire_layers: Vec<String>,
}

/// グループコードと値の並び。
type Pairs = Vec<(i32, String)>;

/// DXFを (コード, 値) の並びへ分解する。
pub fn parse_pairs(input: &str) -> Result<Pairs, DxfError> {
    let mut pairs = Vec::new();
    let mut lines = input.lines().enumerate();
    while let Some((i, code_line)) = lines.next() {
        let code_str = code_line.trim();
        if code_str.is_empty() {
            continue;
        }
        let code: i32 = code_str.parse().map_err(|_| {
            DxfError::Syntax(i + 1, format!("グループコードではありません: {code_str:?}"))
        })?;
        let Some((_, value)) = lines.next() else {
            return Err(DxfError::Syntax(i + 1, "値の行がありません".into()));
        };
        pairs.push((code, value.trim_end_matches('\r').to_string()));
    }
    Ok(pairs)
}

/// 1エンティティ (`0`で始まり次の`0`の手前まで)。
#[derive(Debug, Clone, Default)]
struct Record {
    kind: String,
    pairs: Pairs,
}

impl Record {
    fn get(&self, code: i32) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, v)| v.as_str())
    }
    fn num(&self, code: i32) -> Option<f64> {
        self.get(code).and_then(|v| v.trim().parse().ok())
    }
    fn all(&self, code: i32) -> impl Iterator<Item = &str> {
        self.pairs
            .iter()
            .filter(move |(c, _)| *c == code)
            .map(|(_, v)| v.as_str())
    }
    fn layer(&self) -> String {
        self.get(8).unwrap_or("0").to_string()
    }
    fn text(&self) -> String {
        // MTEXTは3(続き)+1(末尾)。TEXTは1のみ
        let mut s: String = self.all(3).collect();
        s.push_str(self.get(1).unwrap_or(""));
        decode_text(&s)
    }
}

/// レコード列に切る。先頭要素のkindは`SECTION`/`BLOCK`などのマーカも含む。
fn records(pairs: &[(i32, String)]) -> Vec<Record> {
    let mut out: Vec<Record> = Vec::new();
    for (code, value) in pairs {
        if *code == 0 {
            out.push(Record {
                kind: value.trim().to_uppercase(),
                pairs: Vec::new(),
            });
        } else if let Some(last) = out.last_mut() {
            last.pairs.push((*code, value.clone()));
        }
    }
    out
}

/// 線分の頂点 (単位変換前、DXF座標)。
fn polyline_points(rec: &Record, following: &[Record]) -> Vec<(f64, f64)> {
    match rec.kind.as_str() {
        "LINE" => {
            let (Some(x1), Some(y1), Some(x2), Some(y2)) =
                (rec.num(10), rec.num(20), rec.num(11), rec.num(21))
            else {
                return Vec::new();
            };
            vec![(x1, y1), (x2, y2)]
        }
        "LWPOLYLINE" => {
            let xs: Vec<f64> = rec.all(10).filter_map(|v| v.trim().parse().ok()).collect();
            let ys: Vec<f64> = rec.all(20).filter_map(|v| v.trim().parse().ok()).collect();
            xs.into_iter().zip(ys).collect()
        }
        "POLYLINE" => following
            .iter()
            .take_while(|r| r.kind != "SEQEND")
            .filter(|r| r.kind == "VERTEX")
            .filter_map(|r| Some((r.num(10)?, r.num(20)?)))
            .collect(),
        _ => Vec::new(),
    }
}

fn is_closed(rec: &Record) -> bool {
    rec.num(70).map(|f| (f as i64) & 1 == 1).unwrap_or(false)
}

/// 点と線分の距離。
fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    p.distance_to(&Point::new(a.x + t * dx, a.y + t * dy))
}

/// ブロック名 → symbol_id (`MDK_<id>` のみ。ACADE/EPLANの独自ブロック名は対応表が無いので未対応)。
fn symbol_id_of_block(name: &str) -> Option<String> {
    let id = name.strip_prefix(BLOCK_PREFIX)?;
    crate::symbol::resolve_symbol(id).map(|_| id.to_string())
}

/// DXFの中身をProjectへ変換する。
pub fn import_dxf(
    input: &str,
    project_name: &str,
    options: &DxfImportOptions,
) -> Result<(Project, ImportReport), DxfError> {
    use uuid::Uuid;

    let pairs = parse_pairs(input)?;
    let recs = records(&pairs);
    // セクション分割
    let mut section = String::new();
    let mut header: Pairs = Vec::new();
    let mut blocks: BTreeMap<String, Vec<Record>> = BTreeMap::new();
    let mut entities: Vec<Record> = Vec::new();
    let mut current_block: Option<String> = None;
    let mut seen_entities = false;
    for (i, r) in recs.iter().enumerate() {
        match r.kind.as_str() {
            "SECTION" => {
                section = r.get(2).unwrap_or("").trim().to_uppercase();
                if section == "HEADER" {
                    header = r.pairs.clone();
                }
                if section == "ENTITIES" {
                    seen_entities = true;
                }
                continue;
            }
            "ENDSEC" => {
                section.clear();
                continue;
            }
            "EOF" => break,
            _ => {}
        }
        match section.as_str() {
            "BLOCKS" => match r.kind.as_str() {
                "BLOCK" => {
                    let name = r.get(2).unwrap_or("").trim().to_string();
                    current_block = Some(name.clone());
                    blocks.entry(name).or_default();
                }
                "ENDBLK" => current_block = None,
                _ => {
                    if let Some(name) = &current_block {
                        blocks.get_mut(name).unwrap().push(r.clone());
                    }
                }
            },
            "ENTITIES" => entities.push(r.clone()),
            _ => {}
        }
        let _ = i;
    }
    if !seen_entities {
        return Err(DxfError::NotDxf);
    }
    let mut report = ImportReport::default();

    // 単位: $INSUNITS 1=inch, 4=mm, 0/その他=無単位(mm扱い)
    let header_var = |name: &str, code: i32| -> Option<f64> {
        let idx = header
            .iter()
            .position(|(c, v)| *c == 9 && v.trim() == name)?;
        header[idx + 1..]
            .iter()
            .take_while(|(c, _)| *c != 9)
            .find(|(c, _)| *c == code)
            .and_then(|(_, v)| v.trim().parse().ok())
    };
    let scale = match header_var("$INSUNITS", 70).map(|v| v as i64) {
        Some(1) => {
            report
                .warnings
                .push("単位がインチのためmmへ換算しました (×25.4)".into());
            25.4
        }
        _ => 1.0,
    };

    // 外形 (エンティティ全体) → 用紙サイズと座標変換
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut visit = |x: f64, y: f64| {
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
    };
    for (i, r) in entities.iter().enumerate() {
        match r.kind.as_str() {
            "LINE" | "LWPOLYLINE" | "POLYLINE" => {
                for (x, y) in polyline_points(r, &entities[i + 1..]) {
                    visit(x * scale, y * scale);
                }
            }
            "INSERT" | "TEXT" | "MTEXT" | "CIRCLE" | "ARC" => {
                if let (Some(x), Some(y)) = (r.num(10), r.num(20)) {
                    visit(x * scale, y * scale);
                }
            }
            _ => {}
        }
    }
    if minx > maxx {
        // 何も無い図面
        (minx, miny, maxx, maxy) = (0.0, 0.0, 0.0, 0.0);
    }
    let (bw, bh) = (maxx - minx, maxy - miny);
    let portrait = bh > bw;
    let mut chosen: Option<PaperSize> = None;
    for size in [
        PaperSize::A4,
        PaperSize::A3,
        PaperSize::A2,
        PaperSize::A1,
        PaperSize::A0,
    ] {
        let (w, h) = size.dimensions_mm();
        let (w, h) = if portrait { (h, w) } else { (w, h) };
        if bw <= w + 0.01 && bh <= h + 0.01 {
            chosen = Some(size);
            break;
        }
    }
    let size = chosen.unwrap_or_else(|| {
        report.warnings.push(format!(
            "図面の外形 {}×{} mm がA0を超えるためA0に収めずそのまま読みました",
            num(bw),
            num(bh)
        ));
        PaperSize::A0
    });
    let orientation = if portrait {
        Orientation::Portrait
    } else {
        Orientation::Landscape
    };
    let mut sheet = Sheet::new(project_name, size, orientation);
    // 用紙左上を原点に: x' = x - minx, y' = maxy - y
    let conv = |x: f64, y: f64| Point::new(x * scale - minx, maxy - y * scale);

    // 配線レイヤ
    let explicit: Vec<String> = options
        .wire_layers
        .iter()
        .map(|l| l.trim().to_uppercase())
        .collect();
    let is_wire_layer = |layer: &str| -> bool {
        let up = layer.to_uppercase();
        if !explicit.is_empty() {
            return explicit.contains(&up);
        }
        up.contains("WIRE")
            && !up.contains("WIRENO")
            && !up.contains("WIRE_NO")
            && !up.contains("WIRENUM")
    };
    let line_kinds = ["LINE", "LWPOLYLINE", "POLYLINE"];
    let any_wire_layer = entities
        .iter()
        .any(|r| line_kinds.contains(&r.kind.as_str()) && is_wire_layer(&r.layer()));
    let fallback_all_lines = !any_wire_layer;
    if fallback_all_lines
        && entities
            .iter()
            .any(|r| line_kinds.contains(&r.kind.as_str()))
    {
        let mut layers: Vec<String> = entities
            .iter()
            .filter(|r| line_kinds.contains(&r.kind.as_str()))
            .map(|r| r.layer())
            .collect();
        layers.sort();
        layers.dedup();
        report.warnings.push(format!(
            "配線レイヤが見つからないため全ての線分を配線として読みました (レイヤ: {}). 配線のレイヤを wire_layers で指定すると絞れます",
            layers.join(", ")
        ));
    }

    let push = |sheet: &mut Sheet, e: Entity| {
        sheet.entities.insert(e.id(), e);
    };
    let mut skipped: BTreeMap<String, usize> = BTreeMap::new();
    let mut wire_no_texts: Vec<(Point, String)> = Vec::new();
    let mut harness_names: Vec<(Point, String)> = Vec::new();
    let mut i = 0;
    while i < entities.len() {
        let r = &entities[i];
        let layer = r.layer();
        let up = layer.to_uppercase();
        match r.kind.as_str() {
            "LINE" | "LWPOLYLINE" | "POLYLINE" => {
                let raw = polyline_points(r, &entities[i + 1..]);
                let mut points: Vec<Point> = raw.iter().map(|(x, y)| conv(*x, *y)).collect();
                if up == LAYER_FRAME {
                    // 用紙枠は図面要素ではない
                } else if up == LAYER_HARNESS && points.len() >= 3 {
                    if points.len() > 1
                        && points
                            .first()
                            .map(|p| p.distance_to(points.last().unwrap()) < 0.01)
                            .unwrap_or(false)
                    {
                        points.pop();
                    }
                    push(
                        &mut sheet,
                        Entity::Harness(Harness {
                            id: Uuid::new_v4(),
                            points,
                            name: String::new(),
                            note: String::new(),
                        }),
                    );
                } else if (fallback_all_lines || is_wire_layer(&layer))
                    && points.len() >= 2
                    && !is_closed(r)
                {
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
                } else {
                    *skipped
                        .entry(format!("{} (レイヤ {layer})", r.kind))
                        .or_insert(0) += 1;
                }
            }
            "CIRCLE" => {
                let r_mm = r.num(40).unwrap_or(0.0) * scale;
                if (fallback_all_lines || is_wire_layer(&layer)) && r_mm <= 1.0 {
                    if let (Some(x), Some(y)) = (r.num(10), r.num(20)) {
                        push(
                            &mut sheet,
                            Entity::Junction(Junction {
                                id: Uuid::new_v4(),
                                at: conv(x, y),
                            }),
                        );
                        report.junctions += 1;
                    }
                } else {
                    *skipped
                        .entry(format!("CIRCLE (レイヤ {layer})"))
                        .or_insert(0) += 1;
                }
            }
            "TEXT" | "MTEXT" => {
                let (Some(x), Some(y)) = (r.num(10), r.num(20)) else {
                    i += 1;
                    continue;
                };
                let at = conv(x, y);
                let text = r.text();
                let rotation = r.num(50).unwrap_or(0.0).rem_euclid(360.0).round() as u16 % 360;
                let rotation = if rotation.is_multiple_of(90) {
                    rotation
                } else {
                    0
                };
                if text.trim().is_empty() {
                    // 空文字は無視
                } else if up.contains("WIRENO") || up.contains("WIRE_NO") || up.contains("WIRENUM")
                {
                    wire_no_texts.push((at, text.trim().to_string()));
                } else if up == LAYER_LABELS {
                    push(
                        &mut sheet,
                        Entity::NetLabel(NetLabel {
                            id: Uuid::new_v4(),
                            at,
                            name: text.trim().to_string(),
                            rotation,
                        }),
                    );
                    report.labels += 1;
                } else if up == LAYER_HARNESS {
                    harness_names.push((at, text.trim().to_string()));
                } else {
                    let height = r
                        .num(40)
                        .map(|h| h * scale)
                        .filter(|h| *h > 0.0)
                        .unwrap_or(TEXT_HEIGHT);
                    push(
                        &mut sheet,
                        Entity::Text(TextEntity {
                            id: Uuid::new_v4(),
                            at,
                            text,
                            height,
                            rotation,
                        }),
                    );
                    report.texts += 1;
                }
            }
            "INSERT" => {
                let name = r.get(2).unwrap_or("").trim().to_string();
                let (Some(x), Some(y)) = (r.num(10), r.num(20)) else {
                    i += 1;
                    continue;
                };
                let at = conv(x, y);
                // 続くATTRIB (SEQENDまで)
                let mut attribs: BTreeMap<String, String> = BTreeMap::new();
                let mut j = i + 1;
                while j < entities.len() && entities[j].kind == "ATTRIB" {
                    let tag = entities[j].get(2).unwrap_or("").trim().to_uppercase();
                    attribs.insert(tag, entities[j].text().trim().to_string());
                    j += 1;
                }
                if j < entities.len() && entities[j].kind == "SEQEND" {
                    j += 1;
                }
                if name.eq_ignore_ascii_case(BLOCK_DOT) {
                    push(
                        &mut sheet,
                        Entity::Junction(Junction {
                            id: Uuid::new_v4(),
                            at,
                        }),
                    );
                    report.junctions += 1;
                } else if let Some(symbol_id) = symbol_id_of_block(&name) {
                    let angle = r.num(50).unwrap_or(0.0).rem_euclid(360.0);
                    let rounded = angle.round() as i64;
                    let rotation = if rounded % 90 == 0 {
                        (rounded % 360) as u16
                    } else {
                        report
                            .warnings
                            .push(format!("{name}: 回転角 {} を0度に丸めました", num(angle)));
                        0
                    };
                    let mirror = r.num(41).map(|sx| sx < 0.0).unwrap_or(false);
                    let mut attrs = BTreeMap::new();
                    let mut reference = String::new();
                    let mut value = String::new();
                    for (tag, v) in &attribs {
                        match tag.as_str() {
                            "TAG1" | "TAG" => reference = v.clone(),
                            "CAT" => value = v.clone(),
                            "DESC1" => {
                                if !v.is_empty() {
                                    attrs.insert(crate::symbol::SLOT_DESC.to_string(), v.clone());
                                }
                            }
                            "RATING1" => {
                                if !v.is_empty() {
                                    attrs.insert(crate::symbol::SLOT_RATING.to_string(), v.clone());
                                }
                            }
                            t if t.starts_with("TERM") => {}
                            t => {
                                if !v.is_empty() {
                                    attrs.insert(t.to_string(), v.clone());
                                }
                            }
                        }
                    }
                    push(
                        &mut sheet,
                        Entity::Symbol(SymbolInstance {
                            id: Uuid::new_v4(),
                            symbol_id,
                            at,
                            rotation,
                            mirror,
                            reference,
                            value,
                            attrs,
                        }),
                    );
                    report.symbols += 1;
                } else {
                    let tag = attribs
                        .get("TAG1")
                        .or_else(|| attribs.get("TAG"))
                        .cloned()
                        .unwrap_or_default();
                    let key = if tag.is_empty() {
                        name.clone()
                    } else {
                        format!("{name} ({tag})")
                    };
                    *skipped.entry(key).or_insert(0) += 1;
                }
                i = j;
                continue;
            }
            "ATTRIB" | "SEQEND" | "VERTEX" => {}
            other => {
                *skipped
                    .entry(format!("{other} (レイヤ {layer})"))
                    .or_insert(0) += 1;
            }
        }
        i += 1;
    }
    // 線番テキスト → 最寄りの配線
    for (at, no) in wire_no_texts {
        let mut best: Option<(f64, EntityId)> = None;
        for e in sheet.entities.values() {
            if let Entity::Wire(w) = e {
                for seg in w.points.windows(2) {
                    let d = segment_distance(at, seg[0], seg[1]);
                    if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                        best = Some((d, w.id));
                    }
                }
            }
        }
        match best {
            Some((d, id)) if d <= WIRENO_SNAP_MM => {
                if let Some(Entity::Wire(w)) = sheet.entities.get_mut(&id) {
                    w.net = Some(no);
                }
            }
            _ => {
                report
                    .warnings
                    .push(format!("線番 {no:?} の近くに配線が無いため注記にしました"));
                push(
                    &mut sheet,
                    Entity::Text(TextEntity {
                        id: Uuid::new_v4(),
                        at,
                        text: no,
                        height: TEXT_HEIGHT,
                        rotation: 0,
                    }),
                );
                report.texts += 1;
            }
        }
    }
    // ハーネス名 → 左上角に最も近い囲み
    for (at, name) in harness_names {
        let mut best: Option<(f64, EntityId)> = None;
        for e in sheet.entities.values() {
            if let Entity::Harness(h) = e {
                let (minx, miny) = h
                    .points
                    .iter()
                    .fold((f64::MAX, f64::MAX), |(x, y), p| (x.min(p.x), y.min(p.y)));
                let d = at.distance_to(&Point::new(minx, miny));
                if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                    best = Some((d, h.id));
                }
            }
        }
        match best {
            Some((d, id)) if d <= WIRENO_SNAP_MM => {
                if let Some(Entity::Harness(h)) = sheet.entities.get_mut(&id) {
                    h.name = name;
                }
            }
            _ => {
                push(
                    &mut sheet,
                    Entity::Text(TextEntity {
                        id: Uuid::new_v4(),
                        at,
                        text: name,
                        height: TEXT_HEIGHT,
                        rotation: 0,
                    }),
                );
                report.texts += 1;
            }
        }
    }
    for (name, count) in skipped {
        report.skipped.push(format!("{name} x{count}"));
    }
    if !report.skipped.is_empty() {
        report.warnings.push(
            "対応していないブロック・図形はスキップしました。部品は手動で置き直してください".into(),
        );
    }
    let _ = &blocks;
    let mut project = Project::new(project_name);
    project.sheets = vec![sheet];
    Ok((project, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn fixture() -> Sheet {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
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
            attrs: [
                ("DESC".to_string(), "主回路".to_string()),
                ("RATING".to_string(), "DC24V".to_string()),
            ]
            .into(),
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
            points: vec![
                Point::new(10.0, 10.0),
                Point::new(40.0, 10.0),
                Point::new(40.0, 30.0),
            ],
            color: "red".into(),
            sq: 0.75,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: Some("101".into()),
        }));
        push(Entity::Junction(Junction {
            id: Uuid::new_v4(),
            at: Point::new(40.0, 10.0),
        }));
        push(Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(60.0, 80.0),
            name: "24V".into(),
            rotation: 0,
        }));
        push(Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(20.0, 90.0),
            text: "注記A".into(),
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

    fn entity_records(dxf: &str) -> Vec<Record> {
        let recs = records(&parse_pairs(dxf).unwrap());
        let start = recs
            .iter()
            .position(|r| r.kind == "SECTION" && r.get(2) == Some("ENTITIES"))
            .unwrap();
        recs[start + 1..]
            .iter()
            .take_while(|r| r.kind != "ENDSEC")
            .cloned()
            .collect()
    }

    fn find(sheet: &Sheet, f: impl Fn(&Entity) -> bool) -> &Entity {
        sheet.entities.values().find(|e| f(e)).expect("entity")
    }

    /// The exported DXF is an AutoCAD 2000 (AC1015) file in millimetres with HEADER, TABLES, BLOCKS, ENTITIES and OBJECTS sections and unique entity handles.
    /// 書き出したDXFはmm単位のAutoCAD 2000形式(AC1015)で、HEADER/TABLES/BLOCKS/ENTITIES/OBJECTSの各セクションと重複しないハンドルを持つ。
    #[test]
    fn export_writes_a_well_formed_ac1015_file() {
        let sheet = fixture();
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet));
        let pairs = parse_pairs(&dxf).unwrap();
        assert!(pairs
            .windows(2)
            .any(|w| w[0] == (9, "$ACADVER".into()) && w[1] == (1, "AC1015".into())));
        assert!(pairs
            .windows(2)
            .any(|w| w[0] == (9, "$INSUNITS".into()) && w[1] == (70, "4".into())));
        for sec in ["HEADER", "TABLES", "BLOCKS", "ENTITIES", "OBJECTS"] {
            assert!(
                pairs
                    .windows(2)
                    .any(|w| w[0] == (0, "SECTION".into()) && w[1] == (2, sec.into())),
                "{sec}"
            );
        }
        assert_eq!(pairs.last().unwrap(), &(0, "EOF".into()));
        let handles: Vec<&str> = pairs
            .iter()
            .filter(|(c, _)| *c == 5)
            .map(|(_, v)| v.as_str())
            .collect();
        let mut dedup = handles.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(handles.len(), dedup.len(), "ハンドルの重複");
    }

    /// Wires are written as one LINE per segment on the WIRES layer with the Y axis flipped to DXF's upward convention.
    /// 配線はセグメントごとに1本のLINEとしてWIRESレイヤに書かれ、Y座標はDXFの上向き座標へ反転される。
    #[test]
    fn wires_become_lines_on_the_wires_layer_with_y_flipped() {
        let sheet = fixture();
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet));
        let lines: Vec<Record> = entity_records(&dxf)
            .into_iter()
            .filter(|r| r.kind == "LINE" && r.layer() == LAYER_WIRES)
            .collect();
        assert_eq!(lines.len(), 2);
        let first = lines.iter().find(|l| l.num(10) == Some(10.0)).unwrap();
        assert_eq!(first.num(20), Some(297.0 - 10.0));
        assert_eq!(first.num(11), Some(40.0));
    }

    /// Each symbol becomes an INSERT of a block named MDK_<symbol_id> (defined once) with rotation, a negative X scale for mirroring, and TAG1/CAT/DESC1/RATING1/TERMnn attributes.
    /// シンボルはMDK_<symbol_id>ブロック(1回だけ定義)のINSERTになり、回転・ミラー(X倍率-1)・TAG1/CAT/DESC1/RATING1/TERMnnの属性を持つ。
    #[test]
    fn symbols_become_block_inserts_with_attributes() {
        let sheet = fixture();
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet));
        let recs = records(&parse_pairs(&dxf).unwrap());
        let block_names: Vec<&str> = recs
            .iter()
            .filter(|r| r.kind == "BLOCK")
            .filter_map(|r| r.get(2))
            .collect();
        assert!(block_names.contains(&"MDK_relay_coil"), "{block_names:?}");
        assert!(block_names.contains(&"MDK_terminal_block_3p"));
        assert!(block_names.contains(&"WDDOT"));
        assert_eq!(
            block_names
                .iter()
                .filter(|n| **n == "MDK_relay_coil")
                .count(),
            1
        );
        let ents = entity_records(&dxf);
        let idx = ents
            .iter()
            .position(|r| r.kind == "INSERT" && r.get(2) == Some("MDK_relay_coil"))
            .unwrap();
        let ins = &ents[idx];
        assert_eq!(ins.num(50), Some(90.0));
        assert_eq!(ins.num(41), Some(-1.0));
        assert_eq!(ins.get(66), Some("1"));
        let attribs: Vec<(String, String)> = ents[idx + 1..]
            .iter()
            .take_while(|r| r.kind == "ATTRIB")
            .map(|r| (r.get(2).unwrap().to_string(), r.text()))
            .collect();
        assert!(
            attribs.contains(&("TAG1".into(), "K1".into())),
            "{attribs:?}"
        );
        assert!(attribs.contains(&("CAT".into(), "MY2N".into())));
        assert!(attribs.contains(&("DESC1".into(), "主回路".into())));
        assert!(attribs.contains(&("RATING1".into(), "DC24V".into())));
        let first_pin = crate::symbol::resolve_symbol("relay_coil").unwrap().pins[0]
            .number
            .clone();
        assert!(
            attribs
                .iter()
                .any(|(t, v)| t == "TERM01" && *v == first_pin),
            "{attribs:?}"
        );
        assert_eq!(ents[idx + 1 + attribs.len()].kind, "SEQEND");
    }

    /// Wire numbers go to the WIRENO layer, net labels to LABELS, notes to MISC, and a harness becomes a closed dashed polyline on HARNESS with its name.
    /// 線番はWIRENOレイヤ、ネットラベルはLABELS、注記はMISCに書かれ、ハーネス境界はHARNESSレイヤの閉じた破線多角形と名前になる。
    #[test]
    fn texts_and_harness_go_to_their_layers() {
        let sheet = fixture();
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet));
        let ents = entity_records(&dxf);
        let text_on = |layer: &str| -> Vec<String> {
            ents.iter()
                .filter(|r| r.kind == "TEXT" && r.layer() == layer)
                .map(|r| r.text())
                .collect()
        };
        assert_eq!(text_on(LAYER_WIRENO), vec!["101"]);
        assert_eq!(text_on(LAYER_LABELS), vec!["24V"]);
        assert_eq!(text_on(LAYER_MISC), vec!["注記A"]);
        assert_eq!(text_on(LAYER_HARNESS), vec!["W1"]);
        let harness = ents
            .iter()
            .find(|r| r.kind == "LWPOLYLINE" && r.layer() == LAYER_HARNESS)
            .unwrap();
        assert!(is_closed(harness));
        assert_eq!(harness.get(6), Some("DASHED"));
        assert_eq!(harness.all(10).count(), 4);
    }

    /// Non-ASCII text is written as \U+XXXX escapes and decoded back; MTEXT paragraph breaks and formatting codes are handled on read.
    /// 非ASCII文字は\U+XXXXで書かれ読み込み時に戻る。MTEXTの段落区切り(\P)や書式コードも読み込みで処理する。
    #[test]
    fn unicode_is_escaped_and_decoded() {
        assert_eq!(encode_text("A注記"), "A\\U+6CE8\\U+8A18");
        assert_eq!(decode_text("A\\U+6CE8\\U+8A18"), "A注記");
        assert_eq!(
            decode_text("{\\fMS Gothic|b0;1行目\\P2行目}"),
            "1行目\n2行目"
        );
        assert_eq!(decode_text("\\U+D83D\\U+DE00"), "😀");
        let sheet = fixture();
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet));
        assert!(!dxf.contains("注記A"), "生のマルチバイトを書かない");
    }

    /// Exporting a sheet to DXF and importing it back keeps the paper, symbols (id, position, rotation, mirror, reference, value, attributes), wires, junctions, wire numbers, labels, notes and harness names.
    /// シートをDXFへ書き出して読み戻すと、用紙・シンボル(種類・位置・回転・ミラー・参照記号・型番・属性)・配線・ジャンクション・線番・ラベル・注記・ハーネス名が保たれる。
    #[test]
    fn export_then_import_round_trips_the_sheet() {
        let sheet = fixture();
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet));
        let (project, report) = import_dxf(&dxf, "往復", &DxfImportOptions::default()).unwrap();
        let back = &project.sheets[0];
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        assert_eq!(
            (back.size, back.orientation),
            (PaperSize::A3, Orientation::Landscape)
        );
        assert_eq!(report.symbols, 2);
        assert_eq!(report.wires, 2);
        assert_eq!(report.junctions, 1);
        assert_eq!(report.labels, 1);
        assert_eq!(report.texts, 1);
        let Entity::Symbol(k1) = find(
            back,
            |e| matches!(e, Entity::Symbol(s) if s.reference == "K1"),
        ) else {
            unreachable!()
        };
        assert_eq!(k1.symbol_id, "relay_coil");
        assert!(
            (k1.at.x - 100.0).abs() < 0.01 && (k1.at.y - 50.0).abs() < 0.01,
            "{:?}",
            k1.at
        );
        assert_eq!(k1.rotation, 90);
        assert!(k1.mirror);
        assert_eq!(k1.value, "MY2N");
        assert_eq!(k1.attrs.get("DESC").map(String::as_str), Some("主回路"));
        assert_eq!(k1.attrs.get("RATING").map(String::as_str), Some("DC24V"));
        let Entity::Symbol(tb1) = find(
            back,
            |e| matches!(e, Entity::Symbol(s) if s.reference == "TB1"),
        ) else {
            unreachable!()
        };
        assert_eq!(tb1.symbol_id, "terminal_block_3p");
        assert!(!tb1.mirror);
        let numbered: Vec<&str> = back
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Wire(w) => w.net.as_deref(),
                _ => None,
            })
            .collect();
        assert_eq!(numbered, vec!["101"]);
        let Entity::NetLabel(label) = find(back, |e| matches!(e, Entity::NetLabel(_))) else {
            unreachable!()
        };
        assert_eq!(label.name, "24V");
        assert!((label.at.x - 60.0).abs() < 0.01 && (label.at.y - 80.0).abs() < 0.01);
        let Entity::Text(text) = find(back, |e| matches!(e, Entity::Text(_))) else {
            unreachable!()
        };
        assert_eq!((text.text.as_str(), text.height), ("注記A", 3.0));
        let Entity::Harness(h) = find(back, |e| matches!(e, Entity::Harness(_))) else {
            unreachable!()
        };
        assert_eq!(h.name, "W1");
        assert_eq!(h.points.len(), 4);
    }

    const ACADE_LIKE: &str = "  0\nSECTION\n  2\nHEADER\n  9\n$INSUNITS\n 70\n4\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n\
  0\nLINE\n  8\nWIRES\n 10\n0\n 20\n0\n 11\n100\n 21\n0\n\
  0\nLINE\n  8\n_MULTI_WIRE_1\n 10\n100\n 20\n0\n 11\n100\n 21\n50\n\
  0\nLINE\n  8\nMISC\n 10\n0\n 20\n80\n 11\n10\n 21\n80\n\
  0\nTEXT\n  8\nWIRENO\n 10\n50\n 20\n2\n 40\n2.5\n  1\n205\n\
  0\nTEXT\n  8\nMISC\n 10\n5\n 20\n90\n 40\n3\n  1\nNOTE\\U+3042\n\
  0\nINSERT\n  8\nSYMS\n 66\n1\n  2\nHCR1\n 10\n50\n 20\n50\n 50\n0\n\
  0\nATTRIB\n  8\nTAGS\n 10\n50\n 20\n60\n 40\n2.5\n  1\nCR1\n  2\nTAG1\n\
  0\nSEQEND\n\
  0\nINSERT\n  8\nSYMS\n  2\nHCR1\n 10\n80\n 20\n50\n\
  0\nENDSEC\n  0\nEOF\n";

    /// Importing an AutoCAD Electrical style DXF reads lines on wire layers (WIRES, _MULTI_WIRE_*) as wires, assigns WIRENO texts to the nearest wire, keeps other lines and unknown blocks as skipped items, and decodes text.
    /// AutoCAD Electrical流のDXFを読むと、配線レイヤ(WIRES・_MULTI_WIRE_*)の線分が配線になり、WIRENOの文字は最寄りの配線の線番になり、他の線分と未知のブロックはスキップ項目として報告され、文字はデコードされる。
    #[test]
    fn imports_acade_style_wires_and_reports_unknown_blocks() {
        let (project, report) =
            import_dxf(ACADE_LIKE, "acade", &DxfImportOptions::default()).unwrap();
        let sheet = &project.sheets[0];
        assert_eq!(report.wires, 2);
        let numbered: Vec<&str> = sheet
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Wire(w) => w.net.as_deref(),
                _ => None,
            })
            .collect();
        assert_eq!(numbered, vec!["205"]);
        assert!(
            report.skipped.contains(&"HCR1 (CR1) x1".to_string()),
            "{:?}",
            report.skipped
        );
        assert!(
            report.skipped.contains(&"HCR1 x1".to_string()),
            "{:?}",
            report.skipped
        );
        assert!(
            report
                .skipped
                .contains(&"LINE (レイヤ MISC) x1".to_string()),
            "{:?}",
            report.skipped
        );
        let Entity::Text(t) = find(sheet, |e| matches!(e, Entity::Text(_))) else {
            unreachable!()
        };
        assert_eq!(t.text, "NOTEあ");
        assert_eq!(sheet.size, PaperSize::A4, "100×90mmはA4に収まる");
    }

    /// When the import options name the wire layers explicitly, only those layers become wires.
    /// 読み込みオプションで配線レイヤを明示すると、そのレイヤだけが配線になる。
    #[test]
    fn explicit_wire_layers_limit_what_becomes_a_wire() {
        let options = DxfImportOptions {
            wire_layers: vec!["misc".into()],
        };
        let (project, report) = import_dxf(ACADE_LIKE, "acade", &options).unwrap();
        assert_eq!(report.wires, 1);
        let Entity::Wire(w) = find(&project.sheets[0], |e| matches!(e, Entity::Wire(_))) else {
            unreachable!()
        };
        assert!((w.points[0].x - 0.0).abs() < 0.01);
        assert_eq!(
            report
                .skipped
                .iter()
                .filter(|s| s.starts_with("LINE (レイヤ WIRES)"))
                .count(),
            1
        );
    }

    /// A DXF without any wire layer reads every line as a wire and says so in a warning; a WIRENO text with no wire nearby becomes a note.
    /// 配線レイヤの無いDXFは全ての線分を配線として読み、その旨を警告する。近くに配線の無い線番の文字は注記になる。
    #[test]
    fn without_wire_layers_all_lines_become_wires() {
        let dxf = "  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n  8\nLAYER1\n 10\n0\n 20\n0\n 11\n50\n 21\n0\n\
  0\nLWPOLYLINE\n  8\nL2\n 90\n3\n 70\n0\n 10\n0\n 20\n10\n 10\n20\n 20\n10\n 10\n20\n 20\n30\n\
  0\nTEXT\n  8\nWIRENO\n 10\n100\n 20\n100\n 40\n2.5\n  1\n999\n  0\nENDSEC\n  0\nEOF\n";
        let (project, report) = import_dxf(dxf, "t", &DxfImportOptions::default()).unwrap();
        assert_eq!(report.wires, 2);
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("全ての線分を配線")),
            "{:?}",
            report.warnings
        );
        assert!(
            report.warnings.iter().any(|w| w.contains("999")),
            "{:?}",
            report.warnings
        );
        assert_eq!(report.texts, 1);
        let Entity::Wire(poly) = find(
            &project.sheets[0],
            |e| matches!(e, Entity::Wire(w) if w.points.len() == 3),
        ) else {
            unreachable!()
        };
        assert_eq!(poly.points.len(), 3);
    }

    /// A DXF in inches is converted to millimetres, and the paper size is chosen as the smallest ISO A size that fits the drawing extents.
    /// インチ単位のDXFはmmへ換算され、用紙サイズは図面の外形が収まる最小のA判になる。
    #[test]
    fn inch_files_are_scaled_and_paper_fits_extents() {
        let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$INSUNITS\n 70\n1\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n\
  0\nLINE\n  8\nWIRES\n 10\n0\n 20\n0\n 11\n20\n 21\n0\n\
  0\nLINE\n  8\nWIRES\n 10\n0\n 20\n0\n 11\n0\n 21\n12\n  0\nENDSEC\n  0\nEOF\n";
        let (project, report) = import_dxf(dxf, "t", &DxfImportOptions::default()).unwrap();
        let sheet = &project.sheets[0];
        assert_eq!(
            sheet.size,
            PaperSize::A2,
            "20in×12in = 508×305mm → A2 (594×420)"
        );
        assert!(report.warnings.iter().any(|w| w.contains("インチ")));
        let Entity::Wire(w) = find(
            sheet,
            |e| matches!(e, Entity::Wire(w) if w.points[1].x > 100.0),
        ) else {
            unreachable!()
        };
        assert!((w.points[1].x - 508.0).abs() < 0.01, "{:?}", w.points);
        // y: DXFの上向きを用紙の下向きへ (上端が0)
        assert!((w.points[0].y - 304.8).abs() < 0.01, "{:?}", w.points);
    }

    /// A block insert rotated by an angle that is not a multiple of 90 degrees is placed at 0 degrees with a warning.
    /// 90度の倍数でない角度で置かれたブロックは0度で配置され、警告が出る。
    #[test]
    fn non_right_angle_rotation_is_rounded_with_a_warning() {
        let dxf = "  0\nSECTION\n  2\nENTITIES\n  0\nINSERT\n  8\nSYMS\n  2\nMDK_resistor\n 10\n10\n 20\n10\n 50\n45\n  0\nENDSEC\n  0\nEOF\n";
        let (project, report) = import_dxf(dxf, "t", &DxfImportOptions::default()).unwrap();
        let Entity::Symbol(s) = find(&project.sheets[0], |e| matches!(e, Entity::Symbol(_))) else {
            unreachable!()
        };
        assert_eq!(s.rotation, 0);
        assert!(
            report.warnings.iter().any(|w| w.contains("45")),
            "{:?}",
            report.warnings
        );
    }

    /// A file without an ENTITIES section is rejected as not a DXF, and a group code that is not a number is a syntax error with its line number.
    /// ENTITIESセクションの無いファイルはDXFではないとして拒否され、数値でないグループコードは行番号付きの構文エラーになる。
    #[test]
    fn rejects_non_dxf_and_reports_syntax_errors() {
        assert_eq!(
            import_dxf("hello\n", "t", &DxfImportOptions::default()).unwrap_err(),
            DxfError::Syntax(1, "グループコードではありません: \"hello\"".into())
        );
        assert_eq!(
            import_dxf(
                "  0\nSECTION\n  2\nHEADER\n  0\nENDSEC\n  0\nEOF\n",
                "t",
                &DxfImportOptions::default()
            )
            .unwrap_err(),
            DxfError::NotDxf
        );
        assert!(matches!(parse_pairs("  0\n"), Err(DxfError::Syntax(1, _))));
    }

    /// Symbol block graphics are written in local coordinates with Y flipped; arcs keep their sweep because the clockwise paper-space direction becomes counter-clockwise in DXF.
    /// シンボルブロックの図形はY反転したローカル座標で書かれ、弧は用紙座標の時計回りがDXFでは反時計回りになる分だけ角度を入れ替えて向きを保つ。
    #[test]
    fn block_graphics_are_y_flipped_and_arcs_keep_direction() {
        let mut w = Writer::new();
        let def = SymbolDef {
            id: "t".into(),
            name: "t".into(),
            name_ja: "t".into(),
            category: "x".into(),
            ref_prefix: "T".into(),
            keywords: vec![],
            primitives: vec![
                Primitive::Line {
                    pts: vec![Point::new(0.0, -5.0), Point::new(0.0, 5.0)],
                },
                Primitive::Arc {
                    center: Point::new(0.0, 0.0),
                    r: 2.0,
                    start_deg: 270.0,
                    end_deg: 450.0,
                },
            ],
            pins: vec![],
            text_slots: vec![],
        };
        write_symbol_block_body(&mut w, "1", &def);
        let recs = records(&parse_pairs(&w.out).unwrap());
        let line = recs.iter().find(|r| r.kind == "LINE").unwrap();
        assert_eq!((line.num(20), line.num(21)), (Some(5.0), Some(-5.0)));
        let arc = recs.iter().find(|r| r.kind == "ARC").unwrap();
        // 用紙座標で270°→450°(上→右→下の時計回り) は、DXFでは -450→-270 = 270°→90° の反時計回り
        assert_eq!((arc.num(50), arc.num(51)), (Some(270.0), Some(90.0)));
    }
}
