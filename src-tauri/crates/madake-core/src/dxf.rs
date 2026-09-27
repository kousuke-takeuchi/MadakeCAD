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

use crate::frame_template::{Align, BindContext, FrameTemplate};
use crate::geometry::Point;
use crate::io::ExportReport;
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
/// 図形 ([`Entity::Shape`]) = 電気的な意味を持たない作図要素。
pub const LAYER_SHAPES: &str = "SHAPES";
/// 表 ([`Entity::WireLegend`]) など、図面の注釈。
pub const LAYER_ANNOTATION: &str = "ANNOTATION";
/// 盤レイアウト (盤外形・DINレール・ダクト・フットプリント・凡例表)。
pub const LAYER_PANEL: &str = "PANEL";
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

/// MadakeCADの回転角 (用紙座標の時計回り) → DXFの群50 (Y反転後の反時計回り)。
///
/// 書き出しはY座標を反転する (`y_dxf = 用紙高さ - y`)。鏡像にした座標系では回転の向きが
/// 逆になるので、角度の符号を反転しないと90度・270度のシンボルが180度ずれて置かれ、
/// ピンが配線から離れる。弧の開始・終了角を入れ替えるのと同じ理屈 ([`Primitive::Arc`]の分岐)。
fn dxf_angle(rotation: u16) -> f64 {
    (360.0 - f64::from(rotation % 360)) % 360.0
}

/// DXFの群50の角度 → MadakeCADの回転角 ([`dxf_angle`]の逆。符号を戻すだけ)。
fn paper_angle(dxf_deg: f64) -> f64 {
    (-dxf_deg).rem_euclid(360.0)
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
    /// エンティティ共通ヘッダ + 線種 (破線なら`DASHED`の群6を足す)。
    fn entity_styled(&mut self, kind: &str, owner: &str, layer: &str, dashed: bool) -> String {
        let h = self.entity(kind, owner, layer);
        if dashed {
            self.pair(6, "DASHED");
        }
        h
    }
    fn line(&mut self, owner: &str, layer: &str, a: (f64, f64), b: (f64, f64)) {
        self.line_styled(owner, layer, a, b, false);
    }
    fn line_styled(
        &mut self,
        owner: &str,
        layer: &str,
        a: (f64, f64),
        b: (f64, f64),
        dashed: bool,
    ) {
        self.entity_styled("LINE", owner, layer, dashed);
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
        self.entity_styled("LWPOLYLINE", owner, layer, dashed);
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
        self.circle_styled(owner, layer, c, r, false);
    }
    fn circle_styled(&mut self, owner: &str, layer: &str, c: (f64, f64), r: f64, dashed: bool) {
        self.entity_styled("CIRCLE", owner, layer, dashed);
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
    (LAYER_SHAPES, 7, "CONTINUOUS"),
    (LAYER_ANNOTATION, 7, "CONTINUOUS"),
    (LAYER_PANEL, 8, "CONTINUOUS"),
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

/// 矩形を閉じた`LWPOLYLINE`で書く (左上・右下の用紙座標を渡す。Y反転はここでする)。
fn write_rect(w: &mut Writer, ms: &str, layer: &str, min: Point, max: Point, ph: f64, dashed: bool) {
    let fy = |y: f64| ph - y;
    w.polyline(
        ms,
        layer,
        &[
            (min.x, fy(min.y)),
            (max.x, fy(min.y)),
            (max.x, fy(max.y)),
            (min.x, fy(max.y)),
        ],
        true,
        dashed,
    );
}

/// 図枠 (外枠・ゾーン目盛・表題欄・ロゴ枠) を`FRAME`レイヤへ書く。
///
/// SVG/PDFと同じ図枠テンプレート ([`crate::frame_template`]) の寸法とバインド解決を使うので、
/// DXFを受け取った相手にも図面番号・改訂・会社名とゾーンの住所が伝わる。改訂欄は出さない
/// (表題欄のバインド`rev.latest`に最新改訂が入る)。
fn write_frame(w: &mut Writer, ms: &str, sheet: &Sheet, t: &FrameTemplate, ctx: &BindContext) {
    let (pw, ph) = sheet.paper_mm();
    let fy = |y: f64| ph - y;
    let m = t.frame_margin;
    let (x0, y0, x1, y1) = (m, m, pw - m, ph - m);
    write_rect(
        w,
        ms,
        LAYER_FRAME,
        Point::new(x0, y0),
        Point::new(x1, y1),
        ph,
        false,
    );
    // ゾーン目盛 (図面の住所)。分割数はシート、文字種はテンプレートが決める
    let (cols, rows) = (sheet.zone_cols.max(1), sheet.zone_rows.max(1));
    let zw = (x1 - x0) / f64::from(cols);
    let zh = (y1 - y0) / f64::from(rows);
    for i in 0..cols {
        let cx = x0 + zw * (f64::from(i) + 0.5);
        let label = t.zones.col_labels.label(i);
        w.text(ms, LAYER_FRAME, (cx, fy(y0 - 3.0)), 3.0, 0.0, 1, 0, &label);
        w.text(ms, LAYER_FRAME, (cx, fy(y1 + 6.0)), 3.0, 0.0, 1, 0, &label);
        if i > 0 {
            let tx = x0 + zw * f64::from(i);
            w.line(ms, LAYER_FRAME, (tx, fy(y0 - 2.0)), (tx, fy(y0)));
            w.line(ms, LAYER_FRAME, (tx, fy(y1)), (tx, fy(y1 + 2.0)));
        }
    }
    for i in 0..rows {
        let cy = y0 + zh * (f64::from(i) + 0.5) + 1.0;
        let label = t.zones.row_labels.label(i);
        w.text(ms, LAYER_FRAME, (x0 - 3.0, fy(cy)), 3.0, 0.0, 1, 0, &label);
        w.text(ms, LAYER_FRAME, (x1 + 3.0, fy(cy)), 3.0, 0.0, 1, 0, &label);
    }
    // 表題欄 (枠・罫線・セル)。セルはバインドを解決してから書く
    let def = &t.title_block;
    let (tx, ty) = t.title_block_origin(pw, ph);
    write_rect(
        w,
        ms,
        LAYER_FRAME,
        Point::new(tx, ty),
        Point::new(tx + def.w, ty + def.h),
        ph,
        false,
    );
    for g in &def.grid_lines {
        w.line(
            ms,
            LAYER_FRAME,
            (tx + g.x1, fy(ty + g.y1)),
            (tx + g.x2, fy(ty + g.y2)),
        );
    }
    for cell in &def.cells {
        let value = cell.resolve(ctx);
        if value.is_empty() {
            continue;
        }
        let halign = match cell.align {
            Align::Start => 0,
            Align::Middle => 1,
            Align::End => 2,
        };
        w.text(
            ms,
            LAYER_FRAME,
            (tx + cell.x, fy(ty + cell.y)),
            cell.size,
            0.0,
            halign,
            0,
            &value,
        );
    }
    if let Some(logo) = &t.logo {
        write_rect(
            w,
            ms,
            LAYER_FRAME,
            Point::new(logo.x, logo.y),
            Point::new(logo.x + logo.w, logo.y + logo.h),
            ph,
            false,
        );
    }
}

/// 図形 (G10) を`SHAPES`レイヤへ書く。破線は`DASHED`線種で表す。
/// 塗り ([`Shape::fill`]) は出さない (DXFのHATCHは読み手の対応差が大きい)。
fn write_shapes(w: &mut Writer, ms: &str, sheet: &Sheet, ph: f64, report: &mut ExportReport) {
    let fy = |y: f64| ph - y;
    let mut broken = 0;
    for shape in crate::shape::shapes(sheet) {
        if crate::shape::validate(shape).is_err() {
            broken += 1;
            continue;
        }
        match shape.shape {
            ShapeKind::Rect => {
                let Some((min, max)) = crate::shape::rect_corners(shape) else {
                    continue;
                };
                write_rect(w, ms, LAYER_SHAPES, min, max, ph, shape.dashed);
            }
            ShapeKind::Line => {
                let (a, b) = (shape.points[0], shape.points[1]);
                w.line_styled(
                    ms,
                    LAYER_SHAPES,
                    (a.x, fy(a.y)),
                    (b.x, fy(b.y)),
                    shape.dashed,
                );
            }
            ShapeKind::Circle => {
                let Some((center, r)) = crate::shape::circle(shape) else {
                    continue;
                };
                w.circle_styled(ms, LAYER_SHAPES, (center.x, fy(center.y)), r, shape.dashed);
            }
            ShapeKind::Polyline => {
                let pts: Vec<(f64, f64)> = shape.points.iter().map(|p| (p.x, fy(p.y))).collect();
                w.polyline(ms, LAYER_SHAPES, &pts, false, shape.dashed);
            }
        }
        report.count("shape");
    }
    report.skip("shape", broken, "頂点が足りず図面にも描けない図形");
}

/// 表 (凡例) の枠・罫線・文字を書く。行の中身は呼び出し元が導出したもの
/// (SVGと同じ`legend_rows*`) を渡す。見出しの下だけ枠と同じ太さの罫線で区切る。
#[allow(clippy::too_many_arguments)]
fn write_table(
    w: &mut Writer,
    ms: &str,
    layer: &str,
    min: Point,
    max: Point,
    headers: &[&str],
    col_w: &[f64],
    rows: &[Vec<String>],
    row_h: f64,
    font: f64,
    ph: f64,
) {
    let fy = |y: f64| ph - y;
    write_rect(w, ms, layer, min, max, ph, false);
    let mut x = min.x;
    for cw in col_w.iter().take(col_w.len().saturating_sub(1)) {
        x += cw;
        w.line(ms, layer, (x, fy(min.y)), (x, fy(max.y)));
    }
    let cells = std::iter::once(headers.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .chain(rows.iter().cloned());
    for (r, row) in cells.enumerate() {
        let top = min.y + row_h * r as f64;
        if r > 0 {
            w.line(ms, layer, (min.x, fy(top)), (max.x, fy(top)));
        }
        let base = top + row_h / 2.0 + font / 2.0;
        let mut cx = min.x;
        for (i, cell) in row.iter().enumerate() {
            if !cell.is_empty() {
                w.text(
                    ms,
                    layer,
                    (cx + crate::svg::CELL_PAD, fy(base)),
                    font,
                    0.0,
                    0,
                    0,
                    cell,
                );
            }
            cx += col_w.get(i).copied().unwrap_or(0.0);
        }
    }
}

/// 電線品番凡例ブロック (G9) を`ANNOTATION`レイヤへ書く。行はSVGと同じく
/// そのシートの配線から導出する ([`crate::wire_legend::legend_rows_with`])。
fn write_wire_legends(
    w: &mut Writer,
    ms: &str,
    sheet: &Sheet,
    wire_parts: &[WirePart],
    ph: f64,
    report: &mut ExportReport,
) {
    let rows = crate::wire_legend::legend_rows_with(sheet, wire_parts);
    for legend in crate::wire_legend::legends(sheet) {
        let (min, max) = crate::wire_legend::legend_rect_with(sheet, wire_parts, legend);
        write_table(
            w,
            ms,
            LAYER_ANNOTATION,
            min,
            max,
            &crate::wire_legend::LEGEND_COLUMNS,
            &crate::wire_legend::LEGEND_COL_W,
            &rows,
            crate::wire_legend::LEGEND_ROW_H,
            crate::wire_legend::LEGEND_FONT,
            ph,
        );
        report.count("wire_legend");
    }
}

/// 盤レイアウト (盤外形・ダクト・DINレール・フットプリント・凡例表) を`PANEL`レイヤへ書く。
/// DXFは製造図面を渡す形式なので、盤レイアウトシートもそのまま出す (SVGと同じ寸法)。
fn write_panel(w: &mut Writer, ms: &str, sheet: &Sheet, ph: f64, report: &mut ExportReport) {
    let fy = |y: f64| ph - y;
    for e in sheet.entities.values() {
        let Entity::PanelOutline(o) = e else { continue };
        let (min, max) = crate::panel::outline_rect(o);
        write_rect(w, ms, LAYER_PANEL, min, max, ph, false);
        if let Some((pmin, pmax)) = crate::panel::plate_rect(o) {
            write_rect(w, ms, LAYER_PANEL, pmin, pmax, ph, false);
        }
        let name = o.name.trim();
        if !name.is_empty() {
            w.text(
                ms,
                LAYER_PANEL,
                (min.x + crate::svg::CELL_PAD, fy(min.y - 1.5)),
                crate::svg::TITLE_FONT,
                0.0,
                0,
                0,
                name,
            );
        }
        report.count("panel_outline");
    }
    for duct in crate::panel::ducts(sheet) {
        let min = duct.at;
        let max = Point::new(duct.at.x + duct.w, duct.at.y + duct.h);
        write_rect(w, ms, LAYER_PANEL, min, max, ph, false);
        for (a, b) in crate::panel::duct_hatch(duct.at, duct.w, duct.h) {
            w.line(ms, LAYER_PANEL, (a.x, fy(a.y)), (b.x, fy(b.y)));
        }
        report.count("duct");
    }
    for rail in crate::panel::rails(sheet) {
        let (min, max) = crate::panel::rail_rect(rail);
        write_rect(w, ms, LAYER_PANEL, min, max, ph, false);
        // 取付線 (部品が乗る中心線)
        w.line(
            ms,
            LAYER_PANEL,
            (min.x, fy(rail.y)),
            (max.x, fy(rail.y)),
        );
        report.count("din_rail");
    }
    for fp in crate::panel::footprints(sheet) {
        let (min, max) = crate::panel::footprint_rect(fp);
        write_rect(w, ms, LAYER_PANEL, min, max, ph, false);
        let (cx, cy) = ((min.x + max.x) / 2.0, (min.y + max.y) / 2.0);
        let reference = fp.reference.trim();
        if !reference.is_empty() {
            w.text(
                ms,
                LAYER_PANEL,
                (cx, fy(cy)),
                crate::svg::LABEL_FONT,
                0.0,
                1,
                0,
                reference,
            );
        }
        let part_no = fp.part_no.trim();
        if !part_no.is_empty() {
            w.text(
                ms,
                LAYER_PANEL,
                (cx, fy(cy + 3.0)),
                2.0,
                0.0,
                1,
                0,
                part_no,
            );
        }
        // バルーン (アイテム番号)。採番済みのフットプリントだけに付く
        if let Some(b) = crate::panel::balloon(fp) {
            w.line(
                ms,
                LAYER_PANEL,
                (b.leader_from.x, fy(b.leader_from.y)),
                (b.leader_to.x, fy(b.leader_to.y)),
            );
            w.circle(
                ms,
                LAYER_PANEL,
                (b.center.x, fy(b.center.y)),
                crate::panel::BALLOON_R,
            );
            w.text(
                ms,
                LAYER_PANEL,
                (b.center.x, fy(b.center.y + crate::panel::BALLOON_FONT / 2.0)),
                crate::panel::BALLOON_FONT,
                0.0,
                1,
                0,
                &b.item_no.to_string(),
            );
        }
        report.count("footprint");
    }
    let legend_rows = crate::panel::legend_rows(sheet);
    for legend in crate::panel::legends(sheet) {
        let (min, max) = crate::panel::legend_rect(sheet, legend);
        write_table(
            w,
            ms,
            LAYER_PANEL,
            min,
            max,
            &crate::panel::LEGEND_COLUMNS,
            &crate::panel::LEGEND_COL_W,
            &legend_rows,
            crate::panel::LEGEND_ROW_H,
            crate::panel::LEGEND_FONT,
            ph,
        );
        report.count("panel_legend");
    }
}

/// シートをDXF (AutoCAD 2000形式のASCII) の文字列と書き出し内訳にする。図枠は既定の`jis-a3`。
///
/// `wire_parts`はプロジェクトの電線品番表 ([`Project::wire_parts`])。電線品番凡例ブロックの
/// 品番欄はここから引くので、**プロジェクトを持っている呼び出し元は
/// [`project_sheet_to_dxf`]を使うこと** (図枠もシート番号もそちらが正しく入る)。
pub fn sheet_to_dxf(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    wire_parts: &[WirePart],
) -> (String, ExportReport) {
    sheet_to_dxf_with_frame(
        sheet,
        symbols,
        crate::frame_template::default_template(),
        &BindContext::for_sheet(sheet),
        wire_parts,
    )
}

/// プロジェクトの中の1シートをDXFへ書き出す。図枠はプロジェクトの図枠テンプレート、
/// 表題欄のバインドはシート番号・総数入りで解決する。シートが無ければNone。
pub fn project_sheet_to_dxf(
    project: &Project,
    sheet_id: SheetId,
    symbols: &[SymbolDef],
) -> Option<(String, ExportReport)> {
    let sheet = project.sheet(sheet_id)?;
    let frame = crate::frame_template::for_project(project);
    Some(sheet_to_dxf_with_frame(
        sheet,
        symbols,
        &frame,
        &BindContext::for_project(project, sheet),
        &project.wire_parts,
    ))
}

/// [`sheet_to_dxf`]の本体。`frame`/`ctx`は図枠テンプレートと表題欄バインドの文脈。
pub fn sheet_to_dxf_with_frame(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    frame: &FrameTemplate,
    ctx: &BindContext,
    wire_parts: &[WirePart],
) -> (String, ExportReport) {
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
    let mut report = ExportReport::default();
    // 用紙の外周 (読み込み時に外形=用紙になるので座標と用紙サイズが往復で保たれる)
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
    // 図枠 (外枠・ゾーン目盛・表題欄) → 盤レイアウト → 図形 の順に、SVGと同じ背面から描く。
    // DXFは汎用の2D図面形式なので、電気要素以外 (図形・凡例表・盤レイアウト) も出す。
    // 出した件数と出せなかったものは`report`で呼び出し元へ返す (黙って落とさない)。
    write_frame(&mut w, ms, sheet, frame, ctx);
    write_panel(&mut w, ms, sheet, paper_h, &mut report);
    write_shapes(&mut w, ms, sheet, paper_h, &mut report);
    write_wire_legends(&mut w, ms, sheet, wire_parts, paper_h, &mut report);
    // 以降は種別ごとに1周ずつ回して描く。
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
            report.count("harness");
        }
    }
    // 配線 (LINEに分割)
    for e in sheet.entities.values() {
        if let Entity::Wire(wire) = e {
            for seg in wire.points.windows(2) {
                w.line(ms, LAYER_WIRES, pt(seg[0]), pt(seg[1]));
            }
            report.count("wire");
        }
    }
    // ジャンクション
    for e in sheet.entities.values() {
        if let Entity::Junction(j) = e {
            report.count("junction");
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
                dxf_angle(l.rotation),
                0,
                0,
                &l.name,
            );
            report.count("net_label");
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
                    dxf_angle(t.rotation),
                    0,
                    0,
                    line,
                );
            }
            report.count("text");
        }
    }
    // シンボル
    let mut unresolved: BTreeMap<&str, usize> = BTreeMap::new();
    for e in sheet.entities.values() {
        let Entity::Symbol(inst) = e else { continue };
        let Some(def) = defs.get(inst.symbol_id.as_str()) else {
            // 解決できないシンボル定義は描けない。SVGでは出るので黙って落とさず報告する
            *unresolved.entry(inst.symbol_id.as_str()).or_insert(0) += 1;
            continue;
        };
        let insert = w.entity("INSERT", ms, LAYER_SYMS);
        w.pair(100, "AcDbBlockReference");
        w.pair(66, 1);
        w.pair(2, format!("{BLOCK_PREFIX}{}", inst.symbol_id));
        let (x, y) = pt(inst.at);
        w.pair(10, num(x));
        w.pair(20, num(y));
        w.pair(30, 0);
        w.pair(41, if inst.mirror { -1 } else { 1 });
        w.pair(42, 1);
        w.pair(43, 1);
        w.pair(50, num(dxf_angle(inst.rotation)));
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
        report.count("symbol");
    }
    for (id, count) in unresolved {
        report.skip(
            "symbol",
            count,
            &format!("シンボル定義 {id} が見つからないので図形を書けない"),
        );
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
    (w.out, report)
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

/// `TEXT`の基準点 (単位変換前、DXF座標)。
///
/// **揃え指定 (群72=水平、群73=垂直) が0以外のときは群11/21が基準点**で、群10/20は
/// 読み捨てるのがDXFの決まり。ACADEの中央揃えの線番や右寄せの文字はこれを読まないと
/// 見当違いの場所に着き、外形の集計まで狂う。`MTEXT`は群11/21がX軸方向ベクトルなので
/// この規則の対象外 (基準点は常に群10/20)。
fn text_point(rec: &Record) -> Option<(f64, f64)> {
    let justified = rec.num(72).unwrap_or(0.0) != 0.0 || rec.num(73).unwrap_or(0.0) != 0.0;
    if rec.kind == "TEXT" && justified {
        if let (Some(x), Some(y)) = (rec.num(11), rec.num(21)) {
            return Some((x, y));
        }
    }
    Some((rec.num(10)?, rec.num(20)?))
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
    for r in recs.iter() {
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

    // 用紙の範囲: ヘッダの図面限界 ($LIMMIN/$LIMMAX) → 図面範囲 ($EXTMIN/$EXTMAX) →
    // どちらも無ければエンティティの外形。ヘッダを見ないと、内容が200×100mmしか無い
    // ACADEのA3図面がA4になって隅へ寄り、書き戻したDXFが客先の用紙と合わなくなる。
    let header_rect = |min_var: &str, max_var: &str| -> Option<(f64, f64, f64, f64)> {
        let (x0, y0) = (header_var(min_var, 10)?, header_var(min_var, 20)?);
        let (x1, y1) = (header_var(max_var, 10)?, header_var(max_var, 20)?);
        (x1 > x0 && y1 > y0).then_some((x0 * scale, y0 * scale, x1 * scale, y1 * scale))
    };
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    if let Some(rect) = header_rect("$LIMMIN", "$LIMMAX").or_else(|| header_rect("$EXTMIN", "$EXTMAX"))
    {
        (minx, miny, maxx, maxy) = rect;
    } else {
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
                "TEXT" => {
                    if let Some((x, y)) = text_point(r) {
                        visit(x * scale, y * scale);
                    }
                }
                "INSERT" | "MTEXT" | "CIRCLE" | "ARC" => {
                    if let (Some(x), Some(y)) = (r.num(10), r.num(20)) {
                        visit(x * scale, y * scale);
                    }
                }
                _ => {}
            }
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
    // 全ての線分を配線として読む救済は、**レイヤを指定されていないときだけ**。
    // 指定したレイヤに1本も無いからといって全部を配線にすると、絞り込みの指定が
    // 「最も広く読む」に反転してしまう (図枠・表題欄・シンボルの線まで配線になり、
    // ネットリスト・ERC・電線リスト・部品表が狂う)。
    let fallback_all_lines = explicit.is_empty() && !any_wire_layer;
    if !any_wire_layer {
        let mut layers: Vec<String> = entities
            .iter()
            .filter(|r| line_kinds.contains(&r.kind.as_str()))
            .map(|r| r.layer())
            .collect();
        layers.sort();
        layers.dedup();
        if !layers.is_empty() {
            // レイヤ名は機械可読でも返す (UI/CLIが「このレイヤで読み直す」を出せる)
            report.layers = layers.clone();
            if fallback_all_lines {
                report.warnings.push(format!(
                    "配線レイヤが見つからないため全ての線分を配線として読みました (レイヤ: {}). 配線のレイヤを wire_layers で指定すると絞れます",
                    layers.join(", ")
                ));
            } else {
                report.warnings.push(format!(
                    "指定した配線レイヤ ({}) に線分が1本もないため配線を読み込みませんでした。このDXFで線分があるレイヤ: {}",
                    explicit.join(", "),
                    layers.join(", ")
                ));
            }
        }
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
                // 小さい円を結線ドットとみなすのは**本物の配線レイヤの上だけ**。
                // 全線分救済 (`fallback_all_lines`) の下でこれを許すと、ACADE/EPLANの
                // ピン円・端子ドットまでジャンクションになり、ネットが勝手に繋がる
                if is_wire_layer(&layer) && r_mm <= 1.0 {
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
                let Some((x, y)) = text_point(r) else {
                    i += 1;
                    continue;
                };
                let at = conv(x, y);
                let text = r.text();
                let rotation = paper_angle(r.num(50).unwrap_or(0.0)).round() as u16 % 360;
                let rotation = if rotation.is_multiple_of(90) {
                    rotation
                } else {
                    0
                };
                if text.trim().is_empty() {
                    // 空文字は無視
                } else if up == LAYER_FRAME {
                    // 図枠の文字 (ゾーン目盛・表題欄) は図面要素ではない。図枠は
                    // シートの図枠テンプレートと表題欄が持つので、注記として拾わない
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
                            attrs: Default::default(),
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
                            anchor: crate::model::TextAnchor::Start,
                            bold: false,
                            color: None,
                            leader_to: None,
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
                    // 群50は反時計回り、MadakeCADの回転は用紙上の時計回りなので符号を戻す
                    let in_file = r.num(50).unwrap_or(0.0).rem_euclid(360.0);
                    let angle = paper_angle(in_file);
                    let rounded = angle.round() as i64;
                    let rotation = if rounded % 90 == 0 {
                        (rounded % 360) as u16
                    } else {
                        report
                            .warnings
                            .push(format!("{name}: 回転角 {} を0度に丸めました", num(in_file)));
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
                        anchor: crate::model::TextAnchor::Start,
                        bold: false,
                        color: None,
                        leader_to: None,
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
                        anchor: crate::model::TextAnchor::Start,
                        bold: false,
                        color: None,
                        leader_to: None,
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
            attrs: Default::default(),
        }));
        push(Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(20.0, 90.0),
            text: "注記A".into(),
            height: 3.0,
            rotation: 0,
            anchor: crate::model::TextAnchor::Start,
            bold: false,
            color: None,
            leader_to: None,
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
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet), &[]).0;
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
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet), &[]).0;
        let lines: Vec<Record> = entity_records(&dxf)
            .into_iter()
            .filter(|r| r.kind == "LINE" && r.layer() == LAYER_WIRES)
            .collect();
        assert_eq!(lines.len(), 2);
        let first = lines.iter().find(|l| l.num(10) == Some(10.0)).unwrap();
        assert_eq!(first.num(20), Some(297.0 - 10.0));
        assert_eq!(first.num(11), Some(40.0));
    }

    /// Each symbol becomes an INSERT of a block named MDK_<symbol_id> (defined once) with a negative X scale for mirroring and TAG1/CAT/DESC1/RATING1/TERMnn attributes.
    /// シンボルはMDK_<symbol_id>ブロック(1回だけ定義)のINSERTになり、ミラー(X倍率-1)・TAG1/CAT/DESC1/RATING1/TERMnnの属性を持つ。
    #[test]
    fn symbols_become_block_inserts_with_attributes() {
        let sheet = fixture();
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet), &[]).0;
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
        // 回転角は生の値ではなく、ピンが着地する座標で確かめる
        // (`rotated_symbol_pins_meet_their_wires_in_the_exported_dxf`)
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
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet), &[]).0;
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
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet), &[]).0;
        assert!(!dxf.contains("注記A"), "生のマルチバイトを書かない");
    }

    /// Exporting a sheet to DXF and importing it back keeps the paper, symbols (id, position, rotation, mirror, reference, value, attributes), wires, junctions, wire numbers, labels, notes and harness names.
    /// シートをDXFへ書き出して読み戻すと、用紙・シンボル(種類・位置・回転・ミラー・参照記号・型番・属性)・配線・ジャンクション・線番・ラベル・注記・ハーネス名が保たれる。
    #[test]
    fn export_then_import_round_trips_the_sheet() {
        let sheet = fixture();
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet), &[]).0;
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

    /// When the named wire layers hold no line at all, no wire is imported and the warning lists the layers the file really has (the explicit narrowing never widens into "every line is a wire").
    /// 指定した配線レイヤに線分が1本も無いときは配線を1本も読まず、警告がそのDXFに実際にあるレイヤ名を挙げる(明示した絞り込みが「全ての線分を配線」に反転しない)。
    #[test]
    fn explicit_wire_layers_that_match_nothing_import_no_wire() {
        let options = DxfImportOptions {
            wire_layers: vec!["WIRES_TYPO".into()],
        };
        let (project, report) = import_dxf(ACADE_LIKE, "acade", &options).unwrap();
        assert_eq!(report.wires, 0);
        assert!(
            !project.sheets[0]
                .entities
                .values()
                .any(|e| matches!(e, Entity::Wire(_))),
            "配線が1本も作られない"
        );
        let warning = report
            .warnings
            .iter()
            .find(|w| w.contains("WIRES_TYPO"))
            .unwrap_or_else(|| panic!("{:?}", report.warnings));
        assert!(warning.contains("WIRES") && warning.contains("_MULTI_WIRE_1"), "{warning}");
        assert!(!warning.contains("全ての線分を配線"), "{warning}");
        assert_eq!(
            report.layers,
            vec!["MISC".to_string(), "WIRES".to_string(), "_MULTI_WIRE_1".to_string()],
            "読み直し用にレイヤ名を機械可読で返す"
        );
    }

    /// A small circle becomes a junction only on a wire layer: under the "every line is a wire" fallback the pin circles of a foreign drawing stay skipped instead of silently merging nets.
    /// 小さい円がジャンクションになるのは配線レイヤの上だけ。全線分を配線として読む救済の下では、他社図面のピン円はスキップされ、ネットが勝手に繋がらない。
    #[test]
    fn small_circles_become_junctions_only_on_wire_layers() {
        let body = |layer: &str| {
            format!("  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n  8\n{layer}\n 10\n0\n 20\n0\n 11\n50\n 21\n0\n\
  0\nCIRCLE\n  8\n{layer}\n 10\n25\n 20\n0\n 40\n0.6\n  0\nENDSEC\n  0\nEOF\n")
        };
        let (_, fallback) = import_dxf(&body("SYMS"), "t", &DxfImportOptions::default()).unwrap();
        assert_eq!((fallback.wires, fallback.junctions), (1, 0));
        assert!(
            fallback.skipped.iter().any(|s| s.starts_with("CIRCLE (レイヤ SYMS)")),
            "{:?}",
            fallback.skipped
        );
        let (_, on_wires) = import_dxf(&body("WIRES"), "t", &DxfImportOptions::default()).unwrap();
        assert_eq!((on_wires.wires, on_wires.junctions), (1, 1));
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

    /// The paper size and origin come from the header drawing limits ($LIMMIN/$LIMMAX, else $EXTMIN/$EXTMAX) when the file has them, so an A3 drawing whose content only covers a corner still imports as A3 at the same coordinates.
    /// ヘッダの図面限界 ($LIMMIN/$LIMMAX、無ければ$EXTMIN/$EXTMAX) がある図面は用紙サイズと原点をそこから決めるので、内容が隅にしか無いA3図面もA3のまま同じ座標で読める。
    #[test]
    fn header_limits_decide_the_paper_and_origin() {
        let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$INSUNITS\n 70\n4\n  9\n$LIMMIN\n 10\n0\n 20\n0\n  9\n$LIMMAX\n 10\n420\n 20\n297\n  0\nENDSEC\n\
  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n  8\nWIRES\n 10\n10\n 20\n287\n 11\n60\n 21\n287\n  0\nENDSEC\n  0\nEOF\n";
        let (project, _) = import_dxf(dxf, "t", &DxfImportOptions::default()).unwrap();
        let sheet = &project.sheets[0];
        assert_eq!(
            (sheet.size, sheet.orientation),
            (PaperSize::A3, Orientation::Landscape),
            "内容は50×0mmでもヘッダのA3が効く"
        );
        let Entity::Wire(w) = find(sheet, |e| matches!(e, Entity::Wire(_))) else {
            unreachable!()
        };
        // 用紙左上が原点 (y' = 297 - 287 = 10)
        assert!(
            (w.points[0].x - 10.0).abs() < 0.01 && (w.points[0].y - 10.0).abs() < 0.01,
            "{:?}",
            w.points
        );
    }

    /// A justified TEXT (group 72 or 73 non-zero) is placed at its alignment point in groups 11/21, not at the ignored group 10/20.
    /// 揃え指定のあるTEXT(群72か73が0以外)は、読み捨てられる群10/20ではなく群11/21の揃え基準点に置かれる。
    #[test]
    fn justified_text_is_placed_at_its_alignment_point() {
        let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$LIMMIN\n 10\n0\n 20\n0\n  9\n$LIMMAX\n 10\n420\n 20\n297\n  0\nENDSEC\n\
  0\nSECTION\n  2\nENTITIES\n\
  0\nTEXT\n  8\nMISC\n 10\n0\n 20\n0\n 40\n2.5\n  1\nCENTER\n 72\n1\n 11\n100\n 21\n200\n 73\n2\n\
  0\nTEXT\n  8\nMISC\n 10\n30\n 20\n250\n 40\n2.5\n  1\nLEFT\n 72\n0\n 11\n0\n 21\n0\n\
  0\nENDSEC\n  0\nEOF\n";
        let (project, report) = import_dxf(dxf, "t", &DxfImportOptions::default()).unwrap();
        assert_eq!(report.texts, 2);
        let sheet = &project.sheets[0];
        let at = |text: &str| {
            let Entity::Text(t) = find(sheet, |e| matches!(e, Entity::Text(t) if t.text == text))
            else {
                unreachable!()
            };
            t.at
        };
        let centered = at("CENTER");
        assert!(
            (centered.x - 100.0).abs() < 0.01 && (centered.y - 97.0).abs() < 0.01,
            "{centered:?}"
        );
        // 揃え無し (72/73が0) は今までどおり群10/20
        let left = at("LEFT");
        assert!((left.x - 30.0).abs() < 0.01 && (left.y - 47.0).abs() < 0.01, "{left:?}");
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

    /// The frame goes to the FRAME layer: the border, the zone ticks with their letters and numbers, and the title block cells with their binds resolved (drawing number, title, revision).
    /// 図枠はFRAMEレイヤに出る: 外枠・ゾーン目盛の英字と数字・バインドを解決した表題欄のセル(図番・品名・改訂)。
    #[test]
    fn the_frame_and_title_block_go_to_the_frame_layer() {
        let mut sheet = fixture();
        sheet.title_block.drawing_no = "E-1001".into();
        sheet.title_block.title = "制御盤".into();
        sheet.title_block.rev = "B".into();
        let dxf = sheet_to_dxf(&sheet, &defs(&sheet), &[]).0;
        let ents = entity_records(&dxf);
        let texts: Vec<String> = ents
            .iter()
            .filter(|r| r.kind == "TEXT" && r.layer() == LAYER_FRAME)
            .map(|r| r.text())
            .collect();
        assert!(texts.contains(&"E-1001  Rev B".to_string()), "{texts:?}");
        assert!(texts.contains(&"制御盤".to_string()), "{texts:?}");
        assert!(texts.contains(&"図番".to_string()), "{texts:?}");
        // ゾーン目盛 (A3の既定は4列×6行、列=数字・行=英字)。上下・左右の2か所に出る
        assert_eq!(texts.iter().filter(|t| *t == "1").count(), 2, "{texts:?}");
        assert_eq!(texts.iter().filter(|t| *t == "F").count(), 2, "{texts:?}");
        // 外枠 (マージン10mm) と表題欄 (120×32mm、右下) の枠
        let frames: Vec<&Record> = ents
            .iter()
            .filter(|r| r.kind == "LWPOLYLINE" && r.layer() == LAYER_FRAME)
            .collect();
        assert!(
            frames.iter().any(|r| r.num(10) == Some(10.0) && is_closed(r)),
            "外枠が無い"
        );
        assert!(
            frames.iter().any(|r| r.num(10) == Some(420.0 - 10.0 - 120.0)),
            "表題欄の枠が無い"
        );
    }

    /// Shapes, the wire part legend and the panel layout entities are exported too, each on its own layer (SHAPES / ANNOTATION / PANEL), and the export report counts what was written per kind.
    /// 図形・電線品番凡例・盤レイアウトのエンティティもそれぞれのレイヤ(SHAPES / ANNOTATION / PANEL)へ書き出され、書き出し結果の内訳が種別ごとの件数を返す。
    #[test]
    fn shapes_legends_and_panel_layout_are_exported_on_their_own_layers() {
        let mut sheet = Sheet::new("Panel", PaperSize::A3, Orientation::Landscape);
        let mut push = |e: Entity| {
            sheet.entities.insert(e.id(), e);
        };
        push(Entity::Shape(crate::model::Shape {
            id: Uuid::new_v4(),
            shape: ShapeKind::Rect,
            points: vec![Point::new(10.0, 10.0), Point::new(30.0, 20.0)],
            stroke: crate::shape::DEFAULT_SHAPE_COLOR.into(),
            stroke_width_mm: 0.25,
            fill: None,
            dashed: true,
        }));
        push(Entity::Shape(crate::model::Shape {
            id: Uuid::new_v4(),
            shape: ShapeKind::Circle,
            points: vec![Point::new(50.0, 50.0), Point::new(55.0, 50.0)],
            stroke: crate::shape::DEFAULT_SHAPE_COLOR.into(),
            stroke_width_mm: 0.25,
            fill: None,
            dashed: false,
        }));
        push(Entity::WireLegend(WireLegend {
            id: Uuid::new_v4(),
            at: Point::new(200.0, 200.0),
        }));
        push(Entity::PanelOutline(PanelOutline {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 100.0),
            w: 200.0,
            h: 150.0,
            name: "制御盤".into(),
        }));
        push(Entity::DinRail(DinRail {
            id: Uuid::new_v4(),
            y: 150.0,
            x1: 110.0,
            x2: 290.0,
        }));
        push(Entity::Duct(Duct {
            id: Uuid::new_v4(),
            at: Point::new(110.0, 200.0),
            w: 180.0,
            h: 30.0,
        }));
        push(Entity::Footprint(Footprint {
            id: Uuid::new_v4(),
            at: Point::new(120.0, 135.0),
            w: 30.0,
            h: 30.0,
            rotation: 0,
            reference: "K1".into(),
            part_no: "MY2N".into(),
            item_no: Some(1),
        }));
        push(Entity::PanelLegend(PanelLegend {
            id: Uuid::new_v4(),
            at: Point::new(20.0, 240.0),
        }));
        let (dxf, report) = sheet_to_dxf(&sheet, &defs(&sheet), &[]);
        let ents = entity_records(&dxf);
        let on = |layer: &str| -> Vec<&Record> {
            ents.iter().filter(|r| r.layer() == layer).collect()
        };
        // 図形: 破線の矩形 (LWPOLYLINE + DASHED線種) と円
        let shapes = on(LAYER_SHAPES);
        let rect = shapes
            .iter()
            .find(|r| r.kind == "LWPOLYLINE")
            .unwrap_or_else(|| panic!("{shapes:?}"));
        assert_eq!(rect.get(6), Some("DASHED"));
        assert!(is_closed(rect));
        assert_eq!(rect.num(20), Some(297.0 - 10.0), "Y反転して書く");
        let circle = shapes.iter().find(|r| r.kind == "CIRCLE").unwrap();
        assert_eq!((circle.num(10), circle.num(40)), (Some(50.0), Some(5.0)));
        // 電線品番凡例: 表の枠 + 見出しのTEXT
        let annotation = on(LAYER_ANNOTATION);
        assert!(annotation.iter().any(|r| r.kind == "LWPOLYLINE"));
        let headers: Vec<String> = annotation
            .iter()
            .filter(|r| r.kind == "TEXT")
            .map(|r| r.text())
            .collect();
        assert!(headers.contains(&"線色".to_string()), "{headers:?}");
        // 盤レイアウト: 外形・レール・ダクト・フットプリント・凡例表が1レイヤに乗る
        let panel = on(LAYER_PANEL);
        let panel_texts: Vec<String> = panel
            .iter()
            .filter(|r| r.kind == "TEXT")
            .map(|r| r.text())
            .collect();
        for expected in ["制御盤", "K1", "MY2N", "1", "参照記号"] {
            assert!(
                panel_texts.contains(&expected.to_string()),
                "{expected}: {panel_texts:?}"
            );
        }
        assert!(
            panel.iter().filter(|r| r.kind == "LWPOLYLINE").count() >= 6,
            "盤外形・取付板・レール・ダクト・フットプリント・凡例表の枠"
        );
        assert_eq!(
            report.counts,
            [
                ("din_rail".to_string(), 1),
                ("duct".to_string(), 1),
                ("footprint".to_string(), 1),
                ("panel_legend".to_string(), 1),
                ("panel_outline".to_string(), 1),
                ("shape".to_string(), 2),
                ("wire_legend".to_string(), 1),
            ]
            .into_iter()
            .collect::<BTreeMap<String, usize>>()
        );
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    }

    /// A symbol whose definition cannot be resolved is reported in the export report instead of disappearing without a word.
    /// シンボル定義が見つからないシンボルは、黙って消えるのではなく書き出し結果の内訳で報告される。
    #[test]
    fn an_unresolvable_symbol_is_reported_as_skipped() {
        let mut sheet = Sheet::new("S", PaperSize::A3, Orientation::Landscape);
        let id = Uuid::new_v4();
        sheet.entities.insert(
            id,
            Entity::Symbol(SymbolInstance {
                id,
                symbol_id: "no_such_symbol".into(),
                at: Point::new(50.0, 50.0),
                rotation: 0,
                mirror: false,
                reference: "X1".into(),
                value: String::new(),
                attrs: Default::default(),
            }),
        );
        let (dxf, report) = sheet_to_dxf(&sheet, &defs(&sheet), &[]);
        assert!(!dxf.contains("no_such_symbol"));
        assert_eq!(report.counts.get("symbol"), None);
        assert_eq!(
            report.skipped,
            vec!["symbol x1: シンボル定義 no_such_symbol が見つからないので図形を書けない".to_string()]
        );
    }

    /// A rotated or mirrored symbol's pins land on exactly the paper positions the model gives them, so the exported pin still meets the wire drawn to it, and reading the file back gives the same rotation and mirror.
    /// 回転・ミラーしたシンボルのピンはモデルどおりの用紙座標に着地するので、書き出したDXFでもピンがそのピンへ引いた配線の端と一致し、読み戻すと同じ回転角・ミラーになる。
    #[test]
    fn rotated_symbol_pins_meet_their_wires_in_the_exported_dxf() {
        let def = crate::symbol::resolve_symbol("relay_coil").unwrap();
        for rotation in [0u16, 90, 180, 270] {
            for mirror in [false, true] {
                let mut sheet = Sheet::new("S", PaperSize::A3, Orientation::Landscape);
                let inst = SymbolInstance {
                    id: Uuid::new_v4(),
                    symbol_id: "relay_coil".into(),
                    at: Point::new(100.0, 60.0),
                    rotation,
                    mirror,
                    reference: "K1".into(),
                    value: String::new(),
                    attrs: Default::default(),
                };
                // 1本目のピンへ配線を引く (ピンと配線の端が合うことを確かめる相手)
                let pin = crate::netlist::pin_positions(&inst, &def)[0].1;
                let wire = Wire {
                    id: Uuid::new_v4(),
                    points: vec![Point::new(pin.x - 10.0, pin.y), pin],
                    color: "black".into(),
                    sq: 0.0,
                    length_m: None,
                    length_source: Default::default(),
                    part_no: None,
                    net: None,
                };
                sheet.entities.insert(wire.id, Entity::Wire(wire));
                sheet.entities.insert(inst.id, Entity::Symbol(inst));
                let dxf = sheet_to_dxf(&sheet, &defs(&sheet), &[]).0;
                let (_, paper_h) = sheet.paper_mm();
                let recs = records(&parse_pairs(&dxf).unwrap());
                let ents = entity_records(&dxf);
                let ins = ents
                    .iter()
                    .find(|r| r.kind == "INSERT" && r.get(2) == Some("MDK_relay_coil"))
                    .unwrap();
                // ブロック内の1本目のピン (TERM01の属性定義) のローカル座標に
                // INSERTの変換 (倍率→回転→平行移動) をかけてDXF座標を出す
                let attdef = recs
                    .iter()
                    .find(|r| r.kind == "ATTDEF" && r.get(2) == Some("TERM01"))
                    .unwrap();
                let theta = ins.num(50).unwrap().to_radians();
                let (lx, ly) = (
                    attdef.num(10).unwrap() * ins.num(41).unwrap(),
                    attdef.num(20).unwrap(),
                );
                let x = ins.num(10).unwrap() + lx * theta.cos() - ly * theta.sin();
                let y = ins.num(20).unwrap() + lx * theta.sin() + ly * theta.cos();
                let got = Point::new(x, paper_h - y);
                assert!(
                    (got.x - pin.x).abs() < 0.01 && (got.y - pin.y).abs() < 0.01,
                    "rotation {rotation} mirror {mirror}: {got:?} ≠ {pin:?}"
                );
                // 配線は真の用紙座標で書かれるので、その端とも一致する
                let line = ents
                    .iter()
                    .find(|r| r.kind == "LINE" && r.layer() == LAYER_WIRES)
                    .unwrap();
                assert!(
                    (line.num(11).unwrap() - x).abs() < 0.01
                        && (line.num(21).unwrap() - y).abs() < 0.01,
                    "rotation {rotation} mirror {mirror}: 配線の端とピンがずれている"
                );
                let (project, _) = import_dxf(&dxf, "t", &DxfImportOptions::default()).unwrap();
                let Entity::Symbol(back) =
                    find(&project.sheets[0], |e| matches!(e, Entity::Symbol(_)))
                else {
                    unreachable!()
                };
                assert_eq!((back.rotation, back.mirror), (rotation, mirror));
            }
        }
    }
}
