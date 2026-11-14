//! ストロークフォント (線で描く字)。同梱は KST32B 3.0 (坂 直純 氏、フリーウェア: 使用・配付・改変・再配付自由、無保証)。
//! 形式 CSF/1 の解析と、文字 (Unicode) をキーにした字形の表を持つ。
//! 字形の座標は 30x32 の格子で、y は上向き。半角の字は左の15列を使う。

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

/// 同梱の KST32B (原本のまま。`src-tauri/resources/stroke_fonts/kst32b/`)。
const KST32B: &[u8] = include_bytes!("../../../resources/stroke_fonts/kst32b/KST32B.TXT");

/// 1つの字形。
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    /// 線の並び。各線は格子点 (x, y) の折れ線。
    pub strokes: Vec<Vec<(u8, u8)>>,
    /// 全角 (送り30) か半角 (送り15) か。
    pub wide: bool,
}

/// 解析済みのストロークフォント。
#[derive(Debug)]
pub struct StrokeFont {
    pub id: &'static str,
    pub name: &'static str,
    pub glyphs: HashMap<char, Glyph>,
    /// 半角の大文字が立つ行と、届く行 (`H`の字形から測る)。
    pub baseline: u8,
    pub cap_top: u8,
    /// フォントファイルの字の行数 (番地の数)。
    pub address_count: usize,
}

/// CSF/1 の定義1つを線の並びへ。ペンは (0, 0) から始まり、移動は新しい線を始める。
/// 点が1つだけの線は捨てる。空白 (0x20) と予約の値 (0x27, 0x5C, 0x5D) で終わる。
pub fn parse_csf1(def: &[u8]) -> Vec<Vec<(u8, u8)>> {
    fn flush(strokes: &mut Vec<Vec<(u8, u8)>>, cur: &mut Vec<(u8, u8)>, start: (u8, u8)) {
        if cur.len() > 1 {
            strokes.push(std::mem::take(cur));
        }
        *cur = vec![start];
    }
    let (mut x, mut y, mut next_x) = (0u8, 0u8, None::<u8>);
    let mut strokes = Vec::new();
    let mut cur: Vec<(u8, u8)> = vec![(0, 0)];
    for &b in def {
        match b {
            // x へ移動
            0x21..=0x26 | 0x28..=0x3F => {
                x = if b <= 0x26 { b - 0x21 } else { b - 0x28 + 6 };
                flush(&mut strokes, &mut cur, (x, y));
            }
            // x へ線を引く
            0x40..=0x5B | 0x5E..=0x5F => {
                x = if b <= 0x5B { b - 0x40 } else { b - 0x5E + 28 };
                cur.push((x, y));
            }
            // 次の y 命令で使う x
            0x60..=0x7D => next_x = Some(b - 0x60),
            // y へ移動
            0x7E | 0xA1..=0xBF => {
                y = if b == 0x7E { 0 } else { b - 0xA0 };
                if let Some(nx) = next_x.take() {
                    x = nx;
                }
                flush(&mut strokes, &mut cur, (x, y));
            }
            // y へ線を引く
            0xC0..=0xDF => {
                y = b - 0xC0;
                if let Some(nx) = next_x.take() {
                    x = nx;
                }
                cur.push((x, y));
            }
            _ => break,
        }
    }
    flush(&mut strokes, &mut cur, (0, 0));
    strokes
}

/// フォントファイル全体を番地 → 線の並びの表へ。`*`で始まる行と短い行は飛ばす。
pub fn parse_kst(data: &[u8]) -> BTreeMap<u16, Vec<Vec<(u8, u8)>>> {
    let mut table = BTreeMap::new();
    for raw in data.split(|&b| b == b'\n') {
        let line = raw.strip_suffix(b"\r").unwrap_or(raw);
        if line.len() < 6 || line[0] == b'*' {
            continue;
        }
        let Some(addr) = std::str::from_utf8(&line[..4]).ok().and_then(|h| u16::from_str_radix(h, 16).ok()) else {
            continue;
        };
        table.insert(addr, parse_csf1(&line[5..]));
    }
    table
}

/// 番地 → (文字, 全角か)。ASCII と半角カナは半角、ISO 8859-15 は 0x1A20〜0x1A7F に置かれた半角、
/// JIS X 0208 の区点 (0x2121〜0x7E7E) は全角。対応する文字が無ければ None。
pub fn address_to_char(addr: u16) -> Option<(char, bool)> {
    let one = |s: &str| {
        let mut it = s.chars();
        match (it.next(), it.next()) {
            (Some(c), None) => Some(c),
            _ => None,
        }
    };
    match addr {
        0x0021..=0x007E => Some((addr as u8 as char, false)),
        0x00A1..=0x00DF => char::from_u32(0xFF61 + (addr as u32 - 0xA1)).map(|c| (c, false)),
        0x1A20..=0x1A7F => {
            let b = [(addr - 0x1A20 + 0xA0) as u8];
            let (s, _) = encoding_rs::ISO_8859_15.decode_without_bom_handling(&b);
            one(&s).filter(|c| *c != '\u{FFFD}').map(|c| (c, false))
        }
        0x2121..=0x7E7E => {
            let (hi, lo) = ((addr >> 8) as u8, (addr & 0xFF) as u8);
            if !(0x21..=0x7E).contains(&lo) {
                return None;
            }
            let bytes = [hi | 0x80, lo | 0x80];
            match encoding_rs::EUC_JP.decode_without_bom_handling_and_without_replacement(&bytes) {
                Some(s) => one(&s).map(|c| (c, true)),
                // 3区の空き (英数字の間) には、このフォントは同じ位置のASCII記号の全角の字を置いている
                None if hi == 0x23 => char::from_u32(0xFF01 + (lo as u32 - 0x21)).map(|c| (c, true)),
                None => None,
            }
        }
        _ => None,
    }
}

fn build(id: &'static str, name: &'static str, data: &[u8]) -> StrokeFont {
    let table = parse_kst(data);
    let mut glyphs = HashMap::new();
    for (addr, strokes) in &table {
        if let Some((c, wide)) = address_to_char(*addr) {
            glyphs.entry(c).or_insert_with(|| Glyph { strokes: strokes.clone(), wide });
        }
    }
    glyphs.entry(' ').or_insert(Glyph { strokes: Vec::new(), wide: false });
    glyphs.entry('\u{3000}').or_insert(Glyph { strokes: Vec::new(), wide: true });
    let ys: Vec<u8> = glyphs.get(&'H').map(|g| g.strokes.iter().flatten().map(|p| p.1).collect()).unwrap_or_default();
    StrokeFont {
        id,
        name,
        glyphs,
        baseline: ys.iter().copied().min().unwrap_or(0),
        cap_top: ys.iter().copied().max().unwrap_or(31),
        address_count: table.len(),
    }
}

fn kst32b() -> &'static StrokeFont {
    static FONT: OnceLock<StrokeFont> = OnceLock::new();
    FONT.get_or_init(|| build("kst32b", "KST32B 3.0", KST32B))
}

/// 半角の大文字の高さ ÷ フォントサイズ (図枠の`text_size_is: cap`と同じ比)。
pub const CAP_HEIGHT_RATIO: f64 = 0.72;
/// 半角・全角の字の送り (格子の単位)。
pub const HALF_ADVANCE: f64 = 15.0;
pub const FULL_ADVANCE: f64 = 30.0;
/// 線の太さ ÷ フォントサイズ、線の太さの下限 (mm)、太字の倍率。
const STROKE_RATIO: f64 = 0.07;
const STROKE_MIN: f64 = 0.13;
const BOLD_FACTOR: f64 = 1.5;

/// 配置した文字列。座標はベースラインの書き出し位置が原点、y は下向き (用紙と同じ)。
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeText {
    pub polylines: Vec<Vec<(f64, f64)>>,
    /// 文字列の幅 (最後の字の後ろの字間は含めない)。
    pub width: f64,
}

/// 文字サイズに応じた線の太さ (mm)。
pub fn stroke_width(size: f64, bold: bool) -> f64 {
    (STROKE_RATIO * size).max(STROKE_MIN) * if bold { BOLD_FACTOR } else { 1.0 }
}

/// 英数字を全角の字で描くときの文字 (空白は全角の空白)。
fn full_width(c: char) -> char {
    match c {
        ' ' => '\u{3000}',
        '!'..='~' => char::from_u32(c as u32 - 0x21 + 0xFF01).unwrap_or(c),
        _ => c,
    }
}

/// キャンバスへ送る字形1つ (線は x, y の並びに平らにする)。
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct GlyphData {
    pub wide: bool,
    pub strokes: Vec<Vec<u8>>,
}

/// キャンバスへ送るフォント (文字 → 字形)。配置の規則は [`StrokeFont::layout`] と同じものをフロントエンドが持つ。
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct StrokeFontData {
    pub id: String,
    pub baseline: u8,
    pub cap_top: u8,
    pub glyphs: BTreeMap<String, GlyphData>,
}

impl StrokeFont {
    /// 文字列を配置する。フォントに無い字が1つでもあればNone (呼び出し側はシステムの書体で描く)。
    /// `spacing`は字の送りに掛ける倍率 (字の間だけを広げる)。
    pub fn layout(&self, text: &str, size: f64, latin: crate::model::Latin, spacing: f64) -> Option<StrokeText> {
        let unit = CAP_HEIGHT_RATIO * size / f64::from(self.cap_top.saturating_sub(self.baseline).max(1));
        let base = f64::from(self.baseline);
        let (mut pen, mut last) = (0.0, 0.0);
        let mut polylines = Vec::new();
        for c in text.chars() {
            let glyph = match latin {
                crate::model::Latin::Full => self.glyphs.get(&full_width(c)).or_else(|| self.glyphs.get(&c)),
                crate::model::Latin::Half => self.glyphs.get(&c),
            }?;
            for stroke in &glyph.strokes {
                polylines.push(
                    stroke
                        .iter()
                        .map(|&(x, y)| (pen + f64::from(x) * unit, -(f64::from(y) - base) * unit))
                        .collect(),
                );
            }
            last = if glyph.wide { FULL_ADVANCE } else { HALF_ADVANCE } * unit;
            pen += last * spacing;
        }
        let width = if text.is_empty() { 0.0 } else { pen - last * (spacing - 1.0) };
        Some(StrokeText { polylines, width })
    }

    /// キャンバスへ送る形にする。
    pub fn data(&self) -> StrokeFontData {
        StrokeFontData {
            id: self.id.to_string(),
            baseline: self.baseline,
            cap_top: self.cap_top,
            glyphs: self
                .glyphs
                .iter()
                .map(|(c, g)| {
                    let strokes = g.strokes.iter().map(|s| s.iter().flat_map(|&(x, y)| [x, y]).collect()).collect();
                    (c.to_string(), GlyphData { wide: g.wide, strokes })
                })
                .collect(),
        }
    }
}

/// 同梱のストロークフォントの一覧。
pub fn fonts() -> Vec<&'static StrokeFont> {
    vec![kst32b()]
}

/// idで同梱のストロークフォントを引く。
pub fn font(id: &str) -> Option<&'static StrokeFont> {
    fonts().into_iter().find(|f| f.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Latin;

    /// Each kind of CSF/1 command moves or draws the pen as the format describes, and a space ends the definition.
    /// CSF/1の各命令は形式のとおりにペンを動かすか線を引き、空白で定義が終わる。
    #[test]
    fn csf1_commands_move_and_draw_the_pen() {
        // move x5, move y9 (0xA9) → start (5,9); next x11, draw y25 → (11,25)
        assert_eq!(parse_csf1(b"&\xA9k\xD9"), vec![vec![(5, 9), (11, 25)]]);
        // draw x10 then draw x28 along y9
        assert_eq!(parse_csf1(b"&\xA9J^"), vec![vec![(5, 9), (10, 9), (28, 9)]]);
        // move x6 (0x28) at y0; move y0 (0x7E) restarts; draw y31 (0xDF)
        assert_eq!(parse_csf1(b"(~\xDF"), vec![vec![(6, 0), (6, 31)]]);
        // a lone move makes no stroke; the terminator stops parsing
        assert_eq!(parse_csf1(b"&\xA9J \xC0"), vec![vec![(5, 9), (10, 9)]]);
        assert_eq!(parse_csf1(b"!"), Vec::<Vec<(u8, u8)>>::new());
    }

    /// Font records are read by their hexadecimal address, comment lines are skipped, and the bundled font has 7514 records.
    /// フォントの行は16進の番地で読み、注釈の行は飛ばす。同梱フォントの行は7514件。
    #[test]
    fn kst_records_are_read_by_address() {
        let table = parse_kst(b"**** comment\n0041 &\xA9k\xD9 \n* x\n0042 (~\xDF\r\n");
        assert_eq!(table.len(), 2);
        assert_eq!(table[&0x41], vec![vec![(5, 9), (11, 25)]]);
        assert_eq!(table[&0x42], vec![vec![(6, 0), (6, 31)]]);
        assert_eq!(font("kst32b").unwrap().address_count, 7514);
    }

    /// Addresses map to characters: ASCII and half-width kana are half width, ISO 8859-15 is shifted into its own block, JIS X 0208 cells are full width, and the free cells of row 3 hold the full-width forms of the ASCII symbols at the same position.
    /// 番地は文字に対応する。ASCIIと半角カナは半角、ISO 8859-15は専用の領域、JIS X 0208の区点は全角。3区の空きは同じ位置のASCII記号の全角の字。
    #[test]
    fn addresses_map_to_unicode() {
        assert_eq!(address_to_char(0x0041), Some(('A', false)));
        assert_eq!(address_to_char(0x00B1), Some(('ｱ', false)));
        assert_eq!(address_to_char(0x1A24), Some(('€', false)));
        assert_eq!(address_to_char(0x3021), Some(('亜', true)));
        assert_eq!(address_to_char(0x5021), Some(('弌', true)));
        assert_eq!(address_to_char(0x2341), Some(('Ａ', true)));
        assert_eq!(address_to_char(0x0000), None);
        assert_eq!(address_to_char(0x2321), Some(('！', true)));
        assert_eq!(address_to_char(0x2F21), None);
    }

    /// The bundled font is registered as kst32b, covers JIS level 1 and level 2 kanji, kana and Latin letters, and has a space; capitals stand on row 3 and reach row 28.
    /// 同梱フォントはkst32bとして登録され、第1・第2水準の漢字、かな、英字と空白を持つ。大文字は3行目に立ち28行目に届く。
    #[test]
    fn the_bundled_font_is_registered() {
        assert_eq!(fonts().iter().map(|f| f.id).collect::<Vec<_>>(), vec!["kst32b"]);
        assert!(font("missing").is_none());
        let f = font("kst32b").unwrap();
        for c in ['A', 'z', '0', 'ｱ', 'あ', 'ア', '亜', '弌', 'Ａ', ' ', '\u{3000}'] {
            assert!(f.glyphs.contains_key(&c), "{c}");
        }
        // 7514行のうち、JIS X 0208に無い区点 (3区の空きを除く) の約350字は文字に対応しない
        assert!(f.glyphs.len() >= 7200, "{}", f.glyphs.len());
        assert!(!f.glyphs[&'A'].wide && f.glyphs[&'亜'].wide);
        assert_eq!((f.baseline, f.cap_top), (3, 28));
    }

    /// A half-width capital is 0.72 of the font size high and advances 0.432 of it; a full-width character advances twice as far.
    /// 半角の大文字の高さはフォントサイズの0.72倍、送りは0.432倍。全角の字は2倍送る。
    #[test]
    fn layout_scales_glyphs_to_the_font_size() {
        let f = font("kst32b").unwrap();
        let t = f.layout("H", 10.0, Latin::Half, 1.0).unwrap();
        let ys: Vec<f64> = t.polylines.iter().flatten().map(|p| p.1).collect();
        let (lo, hi) = ys.iter().fold((f64::MAX, f64::MIN), |(a, b), y| (a.min(*y), b.max(*y)));
        assert!((lo + 7.2).abs() < 1e-9 && hi.abs() < 1e-9, "{lo} {hi}");
        assert!((t.width - 4.32).abs() < 1e-9);
        assert!((f.layout("亜", 10.0, Latin::Half, 1.0).unwrap().width - 8.64).abs() < 1e-9);
        assert!((f.layout("AB", 10.0, Latin::Half, 1.0).unwrap().width - 8.64).abs() < 1e-9);
    }

    /// With full-width Latin letters ASCII is drawn with the font's full-width forms; spacing widens the gaps between letters but not after the last one.
    /// 英数字を全角にするとASCIIはフォントの全角の字で描かれる。字間は文字の間だけを広げ、最後の字の後ろは広げない。
    #[test]
    fn full_width_latin_and_spacing() {
        let f = font("kst32b").unwrap();
        assert!((f.layout("AB", 10.0, Latin::Full, 1.0).unwrap().width - 17.28).abs() < 1e-9);
        assert!((f.layout("A B", 10.0, Latin::Full, 1.0).unwrap().width - 25.92).abs() < 1e-9);
        assert!((f.layout("AB", 10.0, Latin::Half, 1.5).unwrap().width - (4.32 * 1.5 + 4.32)).abs() < 1e-9);
        let full = f.layout("A", 10.0, Latin::Full, 1.0).unwrap();
        let half = f.layout("A", 10.0, Latin::Half, 1.0).unwrap();
        assert_ne!(full.polylines, half.polylines);
    }

    /// A character whose full-width form the font lacks is drawn with its half-width glyph instead of failing the string.
    /// 全角の字形が無い文字は、文字列ごと諦めずに半角の字形で描く。
    #[test]
    fn full_width_falls_back_to_half_width() {
        let mut f = build("t", "t", b"0041 &\xA9k\xD9\n");
        assert!(!f.glyphs.contains_key(&'Ａ'));
        f.baseline = 0;
        f.cap_top = 25;
        let t = f.layout("A", 10.0, Latin::Full, 1.0).unwrap();
        assert!((t.width - 4.32).abs() < 1e-9);
    }

    /// A string with any character the font does not have is not laid out at all; an empty string and spaces are laid out without strokes.
    /// フォントに無い字を1つでも含む文字列は配置しない。空の文字列と空白は線なしで配置する。
    #[test]
    fn missing_characters_and_empty_strings() {
        let f = font("kst32b").unwrap();
        assert!(f.layout("A😀", 10.0, Latin::Half, 1.0).is_none());
        let e = f.layout("", 10.0, Latin::Half, 1.0).unwrap();
        assert!(e.polylines.is_empty() && e.width == 0.0);
        let s = f.layout("  ", 10.0, Latin::Half, 1.0).unwrap();
        assert!(s.polylines.is_empty() && (s.width - 8.64).abs() < 1e-9);
    }

    /// The stroke is 0.07 of the font size thick but never thinner than 0.13 mm, and bold is half as thick again.
    /// 線の太さはフォントサイズの0.07倍で、0.13mmより細くならない。太字はその1.5倍。
    #[test]
    fn stroke_width_follows_the_size() {
        assert!((stroke_width(10.0, false) - 0.7).abs() < 1e-9);
        assert!((stroke_width(1.0, false) - 0.13).abs() < 1e-9);
        assert!((stroke_width(10.0, true) - 1.05).abs() < 1e-9);
    }

    /// The glyph data sent to the canvas holds every glyph keyed by its character, with the strokes flattened to x, y pairs.
    /// キャンバスへ送る字形データは、全ての字形を文字をキーに持ち、線は x, y の並びに平らにしてある。
    #[test]
    fn glyph_data_for_the_canvas() {
        let f = font("kst32b").unwrap();
        let d = f.data();
        assert_eq!((d.id.as_str(), d.baseline, d.cap_top, d.glyphs.len()), ("kst32b", f.baseline, f.cap_top, f.glyphs.len()));
        let a = &d.glyphs["A"];
        assert_eq!(a.strokes.len(), f.glyphs[&'A'].strokes.len());
        assert_eq!(a.strokes[0].len(), f.glyphs[&'A'].strokes[0].len() * 2);
        assert!(!a.wide && d.glyphs["亜"].wide);
    }
}
