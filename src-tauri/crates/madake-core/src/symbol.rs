use serde::{Deserialize, Serialize};

use crate::geometry::Point;

/// シンボル定義。ローカル座標はmm、原点=配置基準点、ピンは2.5mmグリッド上に置く。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SymbolDef {
    /// ライブラリキー(例: "relay_coil")。
    pub id: String,
    pub name: String,
    pub name_ja: String,
    pub category: String,
    /// 参照記号の接頭辞(例: "R", "K", "J")。
    pub ref_prefix: String,
    /// 部品挿入ダイアログの検索語(英語+日本語+略称)。名称に無い呼び方で探せるようにする。
    #[serde(default)]
    pub keywords: Vec<String>,
    pub primitives: Vec<Primitive>,
    pub pins: Vec<PinDef>,
    /// 属性スロット(M4 §7)。配置時に実値(参照記号・型番・説明・定格)が流し込まれる位置。
    #[serde(default)]
    pub text_slots: Vec<TextSlot>,
}

/// 属性スロットのキー: 参照記号(TAG1相当)。
pub const SLOT_TAG: &str = "TAG";
/// 属性スロットのキー: 型番・値。
pub const SLOT_PART: &str = "PART";
/// 属性スロットのキー: 説明。
pub const SLOT_DESC: &str = "DESC";
/// 属性スロットのキー: 定格。
pub const SLOT_RATING: &str = "RATING";

/// 部品挿入ダイアログでのカテゴリ表示順。同カテゴリの記号は連続して並べる。
pub const CATEGORY_ORDER: &[&str] = &[
    "power",
    "protection",
    "switch",
    "relay",
    "semiconductor",
    "passive",
    "output",
    "instrument",
    "plc",
    "connector",
];

/// 属性テキストの流し込み位置(シンボル外形の外側)。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TextSlot {
    /// [`SLOT_TAG`] などのキー。
    pub key: String,
    pub at: Point,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Primitive {
    /// 折れ線。
    Line { pts: Vec<Point> },
    Circle { center: Point, r: f64, filled: bool },
    Arc {
        center: Point,
        r: f64,
        start_deg: f64,
        end_deg: f64,
    },
    Rect { p1: Point, p2: Point, filled: bool },
    Text { at: Point, text: String, height: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PinDef {
    pub number: String,
    #[serde(default)]
    pub name: String,
    /// 接続点(ローカル座標)。
    pub at: Point,
    /// 接続方向(シンボルの外側へ電線が出る向き)。配線の自動接続・引き出しに使う。
    #[serde(default)]
    pub dir: PinDir,
}

/// ピンの接続方向(回転0度のときの向き)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PinDir {
    Up,
    Down,
    Left,
    Right,
}

impl Default for PinDir {
    fn default() -> Self {
        PinDir::Right
    }
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn line(pts: &[(f64, f64)]) -> Primitive {
    Primitive::Line {
        pts: pts.iter().map(|&(x, y)| p(x, y)).collect(),
    }
}

fn rect(x1: f64, y1: f64, x2: f64, y2: f64, filled: bool) -> Primitive {
    Primitive::Rect { p1: p(x1, y1), p2: p(x2, y2), filled }
}

fn circle(cx: f64, cy: f64, r: f64, filled: bool) -> Primitive {
    Primitive::Circle { center: p(cx, cy), r, filled }
}

/// 弧。角度は用紙座標系(Y下向き)基準で、start→endへ時計回りに描く。
fn arc(cx: f64, cy: f64, r: f64, start_deg: f64, end_deg: f64) -> Primitive {
    Primitive::Arc { center: p(cx, cy), r, start_deg, end_deg }
}

fn text(x: f64, y: f64, t: &str, height: f64) -> Primitive {
    Primitive::Text { at: p(x, y), text: t.into(), height }
}

fn pin(number: &str, name: &str, x: f64, y: f64, dir: PinDir) -> PinDef {
    PinDef {
        number: number.into(),
        name: name.into(),
        at: p(x, y),
        dir,
    }
}

/// シンボルのローカル外形(図形+ピンを含む最小矩形)。属性スロットの位置決めに使う。
pub fn local_bounds(def: &SymbolDef) -> (Point, Point) {
    bounds_of(&def.primitives, &def.pins)
}

fn bounds_of(primitives: &[Primitive], pins: &[PinDef]) -> (Point, Point) {
    let (mut min, mut max) = (p(f64::MAX, f64::MAX), p(f64::MIN, f64::MIN));
    let mut visit = |q: Point| {
        min.x = min.x.min(q.x);
        min.y = min.y.min(q.y);
        max.x = max.x.max(q.x);
        max.y = max.y.max(q.y);
    };
    for prim in primitives {
        match prim {
            Primitive::Line { pts } => pts.iter().copied().for_each(&mut visit),
            Primitive::Circle { center, r, .. } | Primitive::Arc { center, r, .. } => {
                visit(p(center.x - r, center.y - r));
                visit(p(center.x + r, center.y + r));
            }
            Primitive::Rect { p1, p2, .. } => {
                visit(*p1);
                visit(*p2);
            }
            Primitive::Text { at, .. } => visit(*at),
        }
    }
    for pin in pins {
        visit(pin.at);
    }
    if min.x > max.x {
        return (p(0.0, 0.0), p(0.0, 0.0));
    }
    (min, max)
}

/// 標準の属性スロット4種。参照記号・型番は外形の上、説明・定格は下に置く
/// (位置はSVG/キャンバスの注記と同じ規則。シンボルエディタで個別に動かせる)。
fn standard_slots(primitives: &[Primitive], pins: &[PinDef]) -> Vec<TextSlot> {
    use crate::svg::{LABEL_FONT, REF_LABEL_DY, VALUE_LABEL_DY};
    let (min, max) = bounds_of(primitives, pins);
    let slot = |key: &str, y: f64| TextSlot { key: key.into(), at: p(0.0, y), height: LABEL_FONT };
    vec![
        slot(SLOT_TAG, min.y - REF_LABEL_DY),
        slot(SLOT_PART, min.y - VALUE_LABEL_DY),
        slot(SLOT_DESC, max.y + VALUE_LABEL_DY + 2.0),
        slot(SLOT_RATING, max.y + REF_LABEL_DY + 2.0),
    ]
}

/// シンボル定義を組み立てる(属性スロットは外形から自動で決める)。
#[allow(clippy::too_many_arguments)]
fn sym(
    id: &str,
    name: &str,
    name_ja: &str,
    category: &str,
    ref_prefix: &str,
    keywords: &[&str],
    primitives: Vec<Primitive>,
    pins: Vec<PinDef>,
) -> SymbolDef {
    let text_slots = standard_slots(&primitives, &pins);
    SymbolDef {
        id: id.into(),
        name: name.into(),
        name_ja: name_ja.into(),
        category: category.into(),
        ref_prefix: ref_prefix.into(),
        keywords: keywords.iter().map(|k| (*k).to_string()).collect(),
        primitives,
        pins,
        text_slots,
    }
}

/// 水平2端子のリード(左右)。
fn leads_lr(inner: f64) -> Vec<Primitive> {
    vec![line(&[(-7.5, 0.0), (-inner, 0.0)]), line(&[(inner, 0.0), (7.5, 0.0)])]
}

/// 水平2端子のピン(左=1・右=2)。
fn pins_lr(n1: &str, name1: &str, n2: &str, name2: &str) -> Vec<PinDef> {
    vec![
        pin(n1, name1, -7.5, 0.0, PinDir::Left),
        pin(n2, name2, 7.5, 0.0, PinDir::Right),
    ]
}

/// a接点(メーク接点)の図形。開いた状態で描く(JIS: 非励磁・非操作)。
fn make_contact() -> Vec<Primitive> {
    vec![
        line(&[(-7.5, 0.0), (-2.5, 0.0)]),
        line(&[(2.5, 0.0), (7.5, 0.0)]),
        line(&[(-2.5, 0.0), (2.5, -3.5)]),
    ]
}

/// b接点(ブレーク接点)の図形。可動接点が固定接点に載っていることを横切り線で示す。
fn break_contact() -> Vec<Primitive> {
    let mut v = make_contact();
    v.push(line(&[(2.5, 0.0), (2.5, -4.5)]));
    v
}

/// 押しボタン・リミットスイッチの操作ロッド(可動接点の中点から上へ)。
fn actuator_rod() -> Primitive {
    line(&[(0.0, -1.75), (0.0, -5.0)])
}

/// 縦向き多極機器の極位置(5mmピッチ・中央揃え)。
fn pole_x(i: usize, n: usize) -> f64 {
    (i as f64 - (n - 1) as f64 / 2.0) * 5.0
}

/// 縦向きの1極(上リード=固定接点まで、下リード=支点まで、開いた可動接点)。
fn vertical_pole(x: f64) -> Vec<Primitive> {
    vec![
        line(&[(x, -7.5), (x, -2.5)]),
        line(&[(x, 7.5), (x, 2.5)]),
        line(&[(x, 2.5), (x + 2.0, -2.0)]),
    ]
}

/// 極をつなぐ連動線(破線。短い線分の並びで表す)。
fn dashed_link(n: usize) -> Vec<Primitive> {
    let (x0, x1) = (pole_x(0, n), pole_x(n - 1, n));
    let mut out = Vec::new();
    let mut x = x0;
    while x < x1 {
        out.push(line(&[(x, 0.0), ((x + 1.0).min(x1), 0.0)]));
        x += 2.0;
    }
    out
}

/// 縦向き多極機器(遮断器・断路器・電磁接触器)の図形とピン。端子は極ごとに1/2, 3/4, …。
fn multipole(n: usize, mark: impl Fn(f64) -> Vec<Primitive>) -> (Vec<Primitive>, Vec<PinDef>) {
    let mut primitives = Vec::new();
    let mut pins = Vec::new();
    for i in 0..n {
        let x = pole_x(i, n);
        primitives.extend(vertical_pole(x));
        primitives.extend(mark(x));
        pins.push(pin(&(2 * i + 1).to_string(), "", x, -7.5, PinDir::Up));
        pins.push(pin(&(2 * i + 2).to_string(), "", x, 7.5, PinDir::Down));
    }
    if n >= 2 {
        primitives.extend(dashed_link(n));
    }
    (primitives, pins)
}

/// 円の中に文字を書く記号(電動機・計器)。
fn circle_with_text(r: f64, marks: &[(f64, &str, f64)]) -> Vec<Primitive> {
    let mut v = vec![circle(0.0, 0.0, r, false)];
    for &(y, t, h) in marks {
        v.push(text(0.0, y, t, h));
    }
    v
}

/// 導通判定で「同時につながる」ピン番号のまとまり。
/// 端子が1/2, 3/4, … と極対で並ぶ多極機器は極ごとに分かれ、相どうしは短絡しない。
pub fn conducting_pin_groups(def: &SymbolDef) -> Vec<Vec<&str>> {
    let mut numbers: Vec<&str> = Vec::new();
    for pin in &def.pins {
        if !numbers.contains(&pin.number.as_str()) {
            numbers.push(pin.number.as_str());
        }
    }
    let one_point_each = numbers.len() == def.pins.len();
    let parsed: Option<Vec<usize>> = numbers.iter().map(|n| n.parse::<usize>().ok()).collect();
    if let (true, Some(ns)) = (one_point_each, parsed) {
        let consecutive = ns.len() >= 4
            && ns.len() % 2 == 0
            && ns.iter().enumerate().all(|(i, &n)| n == i + 1);
        if consecutive {
            return numbers.chunks(2).map(|c| c.to_vec()).collect();
        }
    }
    vec![numbers]
}

/// 同梱のJIS C 0617 / IEC 60617系シンボルセット。
///
/// [`CATEGORY_ORDER`]の順に、同じカテゴリの記号を連続して並べる(部品挿入ダイアログの表示順)。
/// 形は一般に確立した図記号のみを収録し、社内様式に依存する記号はユーザーライブラリに委ねる。
pub fn builtin_symbols() -> Vec<SymbolDef> {
    let mut out = Vec::new();

    // ---- power: 電源・変換・接地 ----
    out.push(sym(
        "battery",
        "DC source / Battery",
        "直流電源",
        "power",
        "BT",
        &["battery", "dc", "source", "直流電源", "電池", "バッテリ"],
        vec![
            line(&[(-7.5, 0.0), (-1.0, 0.0)]),
            line(&[(1.0, 0.0), (7.5, 0.0)]),
            line(&[(-1.0, -4.0), (-1.0, 4.0)]),
            line(&[(1.0, -2.0), (1.0, 2.0)]),
            text(-3.0, -5.5, "+", 3.0),
        ],
        pins_lr("1", "+", "2", "-"),
    ));
    // 交流電源: 円の中に正弦波 (IEC 60617 06-01-02)
    let sine: Vec<(f64, f64)> = (0..=12)
        .map(|i| {
            let x = -3.0 + i as f64 * 0.5;
            (x, -2.0 * (std::f64::consts::PI * (x + 3.0) / 3.0).sin())
        })
        .collect();
    let mut ac = vec![circle(0.0, 0.0, 5.0, false), line(&sine)];
    ac.extend(leads_lr(5.0));
    out.push(sym(
        "ac_source",
        "AC source",
        "交流電源",
        "power",
        "G",
        &["ac", "source", "supply", "交流電源", "電源"],
        ac,
        pins_lr("1", "L", "2", "N"),
    ));
    // 変圧器 (2巻線): 鉄心 (縦2本) を挟んで一次・二次の巻線を半円3山で描く
    let mut tr = Vec::new();
    for (x, bulge_left) in [(-2.5, true), (2.5, false)] {
        for i in 0..3 {
            let y = -2.5 + i as f64 * 2.5;
            let (a0, a1) = if bulge_left { (90.0, 270.0) } else { (270.0, 450.0) };
            tr.push(arc(x, y, 1.25, a0, a1));
        }
        tr.push(line(&[(x, -3.75), (x, -7.5)]));
        tr.push(line(&[(x, 3.75), (x, 7.5)]));
    }
    tr.push(line(&[(-0.75, -5.0), (-0.75, 5.0)]));
    tr.push(line(&[(0.75, -5.0), (0.75, 5.0)]));
    out.push(sym(
        "transformer",
        "Transformer (2 windings)",
        "変圧器(2巻線)",
        "power",
        "T",
        &["transformer", "trafo", "変圧器", "トランス"],
        tr,
        vec![
            pin("1", "P1", -2.5, -7.5, PinDir::Up),
            pin("2", "P2", -2.5, 7.5, PinDir::Down),
            pin("3", "S1", 2.5, -7.5, PinDir::Up),
            pin("4", "S2", 2.5, 7.5, PinDir::Down),
        ],
    ));
    // 整流器 (ブリッジ): 菱形の中にダイオード。交流側=左右、直流側=上(+)・下(-)
    out.push(sym(
        "rectifier_bridge",
        "Bridge rectifier",
        "整流器(ブリッジ)",
        "power",
        "D",
        &["rectifier", "bridge", "diode", "整流器", "ブリッジ", "ダイオードブリッジ"],
        vec![
            line(&[(-7.5, 0.0), (0.0, -7.5), (7.5, 0.0), (0.0, 7.5), (-7.5, 0.0)]),
            line(&[(-2.5, 2.5), (2.5, 2.5), (0.0, -1.5), (-2.5, 2.5)]),
            line(&[(-2.5, -1.5), (2.5, -1.5)]),
        ],
        vec![
            pin("1", "~", -7.5, 0.0, PinDir::Left),
            pin("2", "~", 7.5, 0.0, PinDir::Right),
            pin("3", "+", 0.0, -7.5, PinDir::Up),
            pin("4", "-", 0.0, 7.5, PinDir::Down),
        ],
    ));
    let earth = || {
        vec![
            line(&[(0.0, 0.0), (0.0, 2.5)]),
            line(&[(-4.0, 2.5), (4.0, 2.5)]),
            line(&[(-2.5, 4.0), (2.5, 4.0)]),
            line(&[(-1.0, 5.5), (1.0, 5.5)]),
        ]
    };
    out.push(sym(
        "ground",
        "Ground",
        "接地",
        "power",
        "GND",
        &["ground", "earth", "接地", "アース", "コモン"],
        earth(),
        vec![pin("1", "", 0.0, 0.0, PinDir::Up)],
    ));
    // 保護接地 (PE): 接地記号を円で囲む (IEC 60417-5019)
    let mut pe = earth();
    pe.push(circle(0.0, 4.0, 4.5, false));
    out.push(sym(
        "earth_protective",
        "Protective earth (PE)",
        "保護接地",
        "power",
        "PE",
        &["protective", "earth", "pe", "保護接地", "接地", "アース"],
        pe,
        vec![pin("1", "", 0.0, 0.0, PinDir::Up)],
    ));
    // フレーム接地 (機能接地): 横棒+3本の斜線 (IEC 60417-5020)
    let mut fg = vec![line(&[(0.0, 0.0), (0.0, 2.5)]), line(&[(-3.0, 2.5), (3.0, 2.5)])];
    for x0 in [-1.5, 0.75, 3.0] {
        fg.push(line(&[(x0, 2.5), (x0 - 1.5, 5.0)]));
    }
    out.push(sym(
        "frame_ground",
        "Frame / functional earth",
        "フレーム接地(機能接地)",
        "power",
        "FG",
        &["frame", "chassis", "functional", "earth", "フレーム接地", "機能接地", "筐体"],
        fg,
        vec![pin("1", "", 0.0, 0.0, PinDir::Up)],
    ));

    // ---- protection: 保護機器 ----
    out.push(sym(
        "fuse",
        "Fuse",
        "ヒューズ",
        "protection",
        "F",
        &["fuse", "ヒューズ", "保護"],
        vec![rect(-5.0, -2.0, 5.0, 2.0, false), line(&[(-7.5, 0.0), (7.5, 0.0)])],
        pins_lr("1", "", "2", ""),
    ));
    // 遮断器: 固定接点に×印 (IEC 60617 07-13-08)
    let breaker_mark = |x: f64| {
        vec![
            line(&[(x - 1.0, -3.5), (x + 1.0, -1.5)]),
            line(&[(x - 1.0, -1.5), (x + 1.0, -3.5)]),
        ]
    };
    // 断路器: 固定接点に可動接点と直交する短棒 (IEC 60617 07-13-05)
    let disconnector_mark = |x: f64| vec![line(&[(x - 1.5, -2.5), (x + 1.5, -2.5)])];
    for (n, id, name, name_ja) in [
        (1usize, "breaker_1p", "Circuit breaker 1P (MCB)", "配線用遮断器(1極)"),
        (2, "breaker_2p", "Circuit breaker 2P (MCB)", "配線用遮断器(2極)"),
        (3, "breaker_3p", "Circuit breaker 3P (MCB/MCCB)", "配線用遮断器(3極)"),
    ] {
        let (primitives, pins) = multipole(n, breaker_mark);
        out.push(sym(
            id,
            name,
            name_ja,
            "protection",
            "CB",
            &["breaker", "mcb", "mccb", "nfb", "遮断器", "配線用遮断器", "ブレーカ"],
            primitives,
            pins,
        ));
    }
    for (n, id, name, name_ja) in [
        (1usize, "disconnector_1p", "Disconnector 1P", "断路器(1極)"),
        (3, "disconnector_3p", "Disconnector 3P", "断路器(3極)"),
    ] {
        let (primitives, pins) = multipole(n, disconnector_mark);
        out.push(sym(
            id,
            name,
            name_ja,
            "protection",
            "DS",
            &["disconnector", "isolator", "断路器", "アイソレータ", "開閉器"],
            primitives,
            pins,
        ));
    }

    // ---- switch: 操作用スイッチ ----
    let mut spst = make_contact();
    spst.push(circle(-2.5, 0.0, 0.5, false));
    spst.push(circle(2.5, 0.0, 0.5, false));
    out.push(sym(
        "switch_spst",
        "Switch (SPST)",
        "スイッチ",
        "switch",
        "SW",
        &["switch", "spst", "スイッチ", "単極単投"],
        spst,
        pins_lr("1", "", "2", ""),
    ));
    // 切替スイッチ (c接点): 共通端子+固定接点2つ。非操作ではb接点側に載る
    let changeover = |blade: Primitive| {
        vec![
            line(&[(-7.5, 0.0), (-2.5, 0.0)]),
            line(&[(2.5, -2.5), (7.5, -2.5)]),
            line(&[(2.5, 2.5), (7.5, 2.5)]),
            blade,
        ]
    };
    let co_pins = |common: &str, no: &str, nc: &str| {
        vec![
            pin(common, "COM", -7.5, 0.0, PinDir::Left),
            pin(no, "NO", 7.5, -2.5, PinDir::Right),
            pin(nc, "NC", 7.5, 2.5, PinDir::Right),
        ]
    };
    out.push(sym(
        "switch_spdt",
        "Changeover switch (SPDT)",
        "切替スイッチ(c接点)",
        "switch",
        "SW",
        &["switch", "spdt", "changeover", "切替スイッチ", "c接点", "単極双投"],
        changeover(line(&[(-2.5, 0.0), (2.5, 2.5)])),
        co_pins("1", "2", "3"),
    ));
    out.push(sym(
        "switch_3pos",
        "Three-position switch (centre off)",
        "3位置切替スイッチ(中立オフ)",
        "switch",
        "SW",
        &["switch", "3 position", "centre off", "3位置", "中立", "切替スイッチ"],
        changeover(line(&[(-2.5, 0.0), (2.0, 0.0)])),
        vec![
            pin("1", "COM", -7.5, 0.0, PinDir::Left),
            pin("2", "", 7.5, -2.5, PinDir::Right),
            pin("3", "", 7.5, 2.5, PinDir::Right),
        ],
    ));
    out.push(sym(
        "pushbutton_no",
        "Pushbutton (NO)",
        "押しボタン(a接点)",
        "switch",
        "PB",
        &["pushbutton", "button", "no", "押しボタン", "a接点", "ボタン"],
        vec![
            line(&[(-7.5, 0.0), (-2.5, 0.0)]),
            line(&[(2.5, 0.0), (7.5, 0.0)]),
            line(&[(-2.5, -2.5), (2.5, -2.5)]),
            line(&[(0.0, -2.5), (0.0, -5.0)]),
            line(&[(-2.0, -5.0), (2.0, -5.0)]),
            circle(-2.5, 0.0, 0.5, false),
            circle(2.5, 0.0, 0.5, false),
        ],
        pins_lr("1", "", "2", ""),
    ));
    let mut pb_nc = break_contact();
    pb_nc.push(actuator_rod());
    pb_nc.push(line(&[(-2.0, -5.0), (2.0, -5.0)]));
    out.push(sym(
        "pushbutton_nc",
        "Pushbutton (NC)",
        "押しボタン(b接点)",
        "switch",
        "PB",
        &["pushbutton", "button", "nc", "押しボタン", "b接点", "ボタン"],
        pb_nc,
        pins_lr("1", "", "2", ""),
    ));
    // 非常停止: b接点+きのこ形の頭部
    let mut estop = break_contact();
    estop.push(actuator_rod());
    estop.push(line(&[(-2.5, -5.0), (2.5, -5.0)]));
    estop.push(arc(0.0, -5.0, 2.5, 180.0, 360.0));
    out.push(sym(
        "emergency_stop",
        "Emergency stop (mushroom head)",
        "非常停止(きのこ形)",
        "switch",
        "PB",
        &["emergency", "stop", "estop", "mushroom", "非常停止", "きのこ", "b接点"],
        estop,
        pins_lr("1", "", "2", ""),
    ));
    // リミットスイッチ: 操作ロッドの先端に塗りつぶし四角 (位置スイッチの操作子)
    for (id, name, name_ja, base) in [
        ("limit_switch_no", "Limit switch (NO)", "リミットスイッチ(a接点)", make_contact()),
        ("limit_switch_nc", "Limit switch (NC)", "リミットスイッチ(b接点)", break_contact()),
    ] {
        let mut prims = base;
        prims.push(actuator_rod());
        prims.push(rect(-1.0, -7.0, 1.0, -5.0, true));
        out.push(sym(
            id,
            name,
            name_ja,
            "switch",
            "LS",
            &["limit switch", "position switch", "リミットスイッチ", "位置スイッチ", "LS"],
            prims,
            pins_lr("1", "", "2", ""),
        ));
    }

    // ---- relay: リレー・電磁接触器 ----
    out.push(sym(
        "relay_coil",
        "Relay coil",
        "リレーコイル",
        "relay",
        "K",
        &["relay", "coil", "contactor", "リレー", "コイル", "電磁接触器"],
        {
            let mut v = vec![rect(-5.0, -3.0, 5.0, 3.0, false)];
            v.extend(leads_lr(5.0));
            v
        },
        pins_lr("A1", "", "A2", ""),
    ));
    out.push(sym(
        "relay_contact_no",
        "Relay contact (NO)",
        "リレー接点(a接点)",
        "relay",
        "K",
        &["relay", "contact", "no", "make", "リレー接点", "a接点", "メーク接点"],
        make_contact(),
        pins_lr("1", "", "2", ""),
    ));
    out.push(sym(
        "relay_contact_nc",
        "Relay contact (NC)",
        "リレー接点(b接点)",
        "relay",
        "K",
        &["relay", "contact", "nc", "break", "リレー接点", "b接点", "ブレーク接点"],
        break_contact(),
        pins_lr("1", "", "2", ""),
    ));
    out.push(sym(
        "relay_contact_co",
        "Relay contact (changeover)",
        "リレー接点(c接点)",
        "relay",
        "K",
        &["relay", "contact", "changeover", "co", "リレー接点", "c接点", "切替接点"],
        changeover(line(&[(-2.5, 0.0), (2.5, 2.5)])),
        co_pins("11", "14", "12"),
    ));
    // 電磁接触器の主接点 (3極): 固定接点のコンタクタ半円+破線の連動線
    let (contactor, contactor_pins) = multipole(3, |x| vec![arc(x, -2.5, 1.25, 0.0, 180.0)]);
    out.push(sym(
        "contactor_3p",
        "Contactor main contacts 3P",
        "電磁接触器 主接点(3極)",
        "relay",
        "K",
        &["contactor", "magnet", "mc", "電磁接触器", "マグネットスイッチ", "主接点"],
        contactor,
        contactor_pins,
    ));

    // ---- semiconductor: 半導体 ----
    let diode_body = || {
        let mut v = vec![
            line(&[(-7.5, 0.0), (-2.5, 0.0)]),
            line(&[(2.5, 0.0), (7.5, 0.0)]),
            line(&[(-2.5, -3.0), (-2.5, 3.0), (2.5, 0.0), (-2.5, -3.0)]),
        ];
        v.push(line(&[(2.5, -3.0), (2.5, 3.0)]));
        v
    };
    out.push(sym(
        "diode",
        "Diode",
        "ダイオード",
        "semiconductor",
        "D",
        &["diode", "ダイオード", "整流"],
        diode_body(),
        pins_lr("1", "A", "2", "K"),
    ));
    let mut zener = diode_body();
    zener.pop();
    zener.push(line(&[(1.5, -3.0), (2.5, -3.0), (2.5, 3.0), (3.5, 3.0)]));
    out.push(sym(
        "zener_diode",
        "Zener diode",
        "ツェナーダイオード",
        "semiconductor",
        "D",
        &["zener", "diode", "ツェナー", "定電圧ダイオード"],
        zener,
        pins_lr("1", "A", "2", "K"),
    ));
    out.push(sym(
        "led",
        "LED",
        "発光ダイオード",
        "semiconductor",
        "LED",
        &["led", "light emitting diode", "発光ダイオード", "表示"],
        vec![
            line(&[(-7.5, 0.0), (-2.5, 0.0)]),
            line(&[(2.5, 0.0), (7.5, 0.0)]),
            line(&[(-2.5, -3.0), (-2.5, 3.0), (2.5, 0.0), (-2.5, -3.0)]),
            line(&[(2.5, -3.0), (2.5, 3.0)]),
            line(&[(0.0, -3.5), (2.0, -5.5)]),
            line(&[(1.0, -5.5), (2.0, -5.5), (2.0, -4.5)]),
            line(&[(2.5, -3.5), (4.5, -5.5)]),
            line(&[(3.5, -5.5), (4.5, -5.5), (4.5, -4.5)]),
        ],
        pins_lr("1", "A", "2", "K"),
    ));

    // ---- passive: 受動部品 ----
    let resistor_body = || {
        let mut v = vec![rect(-5.0, -2.0, 5.0, 2.0, false)];
        v.extend(leads_lr(5.0));
        v
    };
    out.push(sym(
        "resistor",
        "Resistor",
        "抵抗器",
        "passive",
        "R",
        &["resistor", "抵抗器", "抵抗"],
        resistor_body(),
        pins_lr("1", "", "2", ""),
    ));
    let mut vr = resistor_body();
    vr.push(line(&[(-6.0, 4.0), (6.0, -4.0)]));
    vr.push(line(&[(4.4, -1.9), (6.0, -4.0), (3.4, -3.4)]));
    out.push(sym(
        "resistor_variable",
        "Variable resistor",
        "可変抵抗器",
        "passive",
        "VR",
        &["variable resistor", "rheostat", "可変抵抗器", "ボリューム"],
        vr,
        pins_lr("1", "", "2", ""),
    ));
    let mut varistor = resistor_body();
    varistor.push(line(&[(-5.0, 3.0), (5.0, -3.0)]));
    varistor.push(text(-6.0, 4.3, "U", 2.5));
    out.push(sym(
        "varistor",
        "Varistor (VDR)",
        "バリスタ",
        "passive",
        "RV",
        &["varistor", "vdr", "surge", "バリスタ", "サージ", "電圧依存抵抗"],
        varistor,
        pins_lr("1", "", "2", ""),
    ));
    out.push(sym(
        "capacitor",
        "Capacitor",
        "コンデンサ",
        "passive",
        "C",
        &["capacitor", "コンデンサ", "キャパシタ"],
        vec![
            line(&[(-7.5, 0.0), (-1.0, 0.0)]),
            line(&[(1.0, 0.0), (7.5, 0.0)]),
            line(&[(-1.0, -4.0), (-1.0, 4.0)]),
            line(&[(1.0, -4.0), (1.0, 4.0)]),
        ],
        pins_lr("1", "", "2", ""),
    ));
    out.push(sym(
        "capacitor_polarized",
        "Capacitor (polarized)",
        "有極性コンデンサ",
        "passive",
        "C",
        &["capacitor", "polarized", "electrolytic", "有極性", "電解コンデンサ"],
        vec![
            line(&[(-7.5, 0.0), (-1.0, 0.0)]),
            line(&[(2.0, 0.0), (7.5, 0.0)]),
            line(&[(-1.0, -4.0), (-1.0, 4.0)]),
            rect(1.0, -4.0, 2.0, 4.0, true),
            text(-3.0, -5.5, "+", 3.0),
        ],
        pins_lr("1", "+", "2", "-"),
    ));
    let mut inductor = vec![];
    for i in 0..4 {
        inductor.push(arc(-3.75 + i as f64 * 2.5, 0.0, 1.25, 180.0, 360.0));
    }
    inductor.extend(leads_lr(5.0));
    out.push(sym(
        "inductor",
        "Inductor / reactor",
        "インダクタ(リアクトル)",
        "passive",
        "L",
        &["inductor", "coil", "reactor", "choke", "インダクタ", "リアクトル", "チョーク"],
        inductor,
        pins_lr("1", "", "2", ""),
    ));

    // ---- output: 負荷・報知 ----
    out.push(sym(
        "lamp",
        "Lamp",
        "ランプ",
        "output",
        "L",
        &["lamp", "indicator", "pilot", "ランプ", "表示灯", "パイロットランプ"],
        vec![
            circle(0.0, 0.0, 4.0, false),
            line(&[(-2.83, -2.83), (2.83, 2.83)]),
            line(&[(-2.83, 2.83), (2.83, -2.83)]),
            line(&[(-7.5, 0.0), (-4.0, 0.0)]),
            line(&[(4.0, 0.0), (7.5, 0.0)]),
        ],
        pins_lr("1", "", "2", ""),
    ));
    out.push(sym(
        "motor",
        "Motor",
        "モータ",
        "output",
        "M",
        &["motor", "モータ", "電動機"],
        {
            let mut v = vec![circle(0.0, 0.0, 5.0, false), text(0.0, 0.0, "M", 4.0)];
            v.extend(leads_lr(5.0));
            v
        },
        pins_lr("1", "", "2", ""),
    ));
    // 電動機の相種別: 円の中に「M」と 3~ / 1~ / 直流記号。端子は円の上から出す
    let motor_leads = |xs: &[f64]| -> Vec<Primitive> {
        xs.iter()
            .map(|&x| {
                let y = -(36.0f64 - x * x).sqrt();
                line(&[(x, y), (x, -10.0)])
            })
            .collect()
    };
    let mut m3 = circle_with_text(6.0, &[(-1.5, "M", 4.0), (3.0, "3~", 2.5)]);
    m3.extend(motor_leads(&[-2.5, 0.0, 2.5]));
    out.push(sym(
        "motor_3ph",
        "Motor (3-phase)",
        "三相電動機",
        "output",
        "M",
        &["motor", "three phase", "induction", "三相", "電動機", "モータ"],
        m3,
        vec![
            pin("U", "", -2.5, -10.0, PinDir::Up),
            pin("V", "", 0.0, -10.0, PinDir::Up),
            pin("W", "", 2.5, -10.0, PinDir::Up),
        ],
    ));
    let mut m1 = circle_with_text(6.0, &[(-1.5, "M", 4.0), (3.0, "1~", 2.5)]);
    m1.extend(motor_leads(&[-2.5, 2.5]));
    out.push(sym(
        "motor_1ph",
        "Motor (single-phase)",
        "単相電動機",
        "output",
        "M",
        &["motor", "single phase", "単相", "電動機", "モータ"],
        m1,
        vec![
            pin("1", "U1", -2.5, -10.0, PinDir::Up),
            pin("2", "U2", 2.5, -10.0, PinDir::Up),
        ],
    ));
    let mut mdc = circle_with_text(6.0, &[(-1.5, "M", 4.0)]);
    mdc.push(line(&[(-1.5, 3.0), (1.5, 3.0)]));
    mdc.push(line(&[(-1.5, 4.2), (-0.5, 4.2)]));
    mdc.push(line(&[(0.5, 4.2), (1.5, 4.2)]));
    mdc.extend(motor_leads(&[-2.5, 2.5]));
    out.push(sym(
        "motor_dc",
        "Motor (DC)",
        "直流電動機",
        "output",
        "M",
        &["motor", "dc", "直流", "電動機", "モータ"],
        mdc,
        vec![
            pin("1", "A1", -2.5, -10.0, PinDir::Up),
            pin("2", "A2", 2.5, -10.0, PinDir::Up),
        ],
    ));
    let mut bell = vec![arc(0.0, 0.0, 5.0, 180.0, 360.0), line(&[(-5.0, 0.0), (5.0, 0.0)])];
    bell.extend(leads_lr(5.0));
    out.push(sym(
        "bell",
        "Bell",
        "ベル",
        "output",
        "BL",
        &["bell", "alarm", "ベル", "電鈴", "報知"],
        bell,
        pins_lr("1", "", "2", ""),
    ));

    // ---- instrument: 計測器 ----
    for (id, name, name_ja, prefix, mark, keywords) in [
        (
            "voltmeter",
            "Voltmeter",
            "電圧計",
            "VM",
            "V",
            &["voltmeter", "meter", "電圧計", "計器"][..],
        ),
        (
            "ammeter",
            "Ammeter",
            "電流計",
            "AM",
            "A",
            &["ammeter", "meter", "電流計", "計器"][..],
        ),
    ] {
        let mut prims = circle_with_text(5.0, &[(0.0, mark, 4.0)]);
        prims.extend(leads_lr(5.0));
        out.push(sym(id, name, name_ja, "instrument", prefix, keywords, prims, pins_lr("1", "", "2", "")));
    }
    // 変流器 (CT): 一次導体が鉄心を貫き、二次はS1/S2
    out.push(sym(
        "current_transformer",
        "Current transformer (CT)",
        "変流器(CT)",
        "instrument",
        "CT",
        &["current transformer", "ct", "変流器", "計器用変成器"],
        vec![
            line(&[(0.0, -7.5), (0.0, 7.5)]),
            circle(0.0, 0.0, 4.0, false),
            line(&[(3.12, -2.5), (7.5, -2.5)]),
            line(&[(3.12, 2.5), (7.5, 2.5)]),
        ],
        vec![
            pin("P1", "", 0.0, -7.5, PinDir::Up),
            pin("P2", "", 0.0, 7.5, PinDir::Down),
            pin("S1", "", 7.5, -2.5, PinDir::Right),
            pin("S2", "", 7.5, 2.5, PinDir::Right),
        ],
    ));

    // ---- connector: 端子・接続器 ----
    out.push(sym(
        "terminal",
        "Terminal",
        "端子",
        "connector",
        "T",
        &["terminal", "端子", "接続点"],
        vec![circle(0.0, 0.0, 1.2, false), line(&[(1.2, 0.0), (5.0, 0.0)])],
        vec![pin("1", "", 5.0, 0.0, PinDir::Right)],
    ));
    out.push(sym(
        "connector_plug",
        "Connector plug (male)",
        "差込接続器 プラグ(雄)",
        "connector",
        "J",
        &["plug", "male", "connector", "プラグ", "雄", "コネクタ"],
        vec![
            line(&[(-7.5, 0.0), (-2.5, 0.0)]),
            line(&[(-2.5, -2.5), (2.5, 0.0), (-2.5, 2.5)]),
        ],
        vec![pin("1", "", -7.5, 0.0, PinDir::Left)],
    ));
    out.push(sym(
        "connector_socket",
        "Connector socket (female)",
        "差込接続器 ソケット(雌)",
        "connector",
        "J",
        &["socket", "female", "connector", "ソケット", "雌", "コネクタ"],
        vec![line(&[(2.5, 0.0), (7.5, 0.0)]), arc(0.0, 0.0, 2.5, 270.0, 450.0)],
        vec![pin("1", "", 7.5, 0.0, PinDir::Right)],
    ));

    out
}

/// 動的シンボルの最大極数。
pub const DYNAMIC_PIN_MAX: usize = 50;

/// PLCモジュール(`plc_di_{n}p` / `plc_do_{n}p`)の最大点数。
pub const PLC_POINT_MAX: usize = 64;
/// PLCモジュールのI/O点の縦ピッチ (mm)。2.5mmグリッドの倍数。
pub const PLC_POINT_PITCH_MM: f64 = 5.0;

/// `plc_di_{n}p` / `plc_do_{n}p` 形式のIDからPLC I/Oモジュールのシンボルを生成する。
///
/// 縦長の箱の**左側にI/O点ぶんの接続点**を [`PLC_POINT_PITCH_MM`] ピッチで並べる
/// (入力は外部の機器から信号が入ってくる側、出力も同じ側にまとめて配線しやすくする)。
/// 箱の中には点番号 (1〜n) と種別 (DI/DO) を書く。参照記号の接頭辞は `PLC`。
fn plc_module_symbol(id: &str) -> Option<SymbolDef> {
    let (rest, input) = match (id.strip_prefix("plc_di_"), id.strip_prefix("plc_do_")) {
        (Some(rest), _) => (rest, true),
        (_, Some(rest)) => (rest, false),
        _ => return None,
    };
    let n: usize = rest.strip_suffix('p')?.parse().ok()?;
    if !(1..=PLC_POINT_MAX).contains(&n) {
        return None;
    }
    let pitch = PLC_POINT_PITCH_MM;
    let offset = |i: usize| (i as f64 - (n - 1) as f64 / 2.0) * pitch;
    let (top, bottom) = (offset(0) - pitch, offset(n - 1) + pitch);
    let mut primitives = vec![
        Primitive::Rect {
            p1: p(-5.0, top),
            p2: p(5.0, bottom),
            filled: false,
        },
        text(0.0, top + 3.5, if input { "DI" } else { "DO" }, 3.0),
    ];
    let mut pins = Vec::with_capacity(n);
    for i in 0..n {
        let y = offset(i);
        primitives.push(text(-2.5, y + 1.0, &(i + 1).to_string(), 2.0));
        primitives.push(line(&[(-7.5, y), (-5.0, y)]));
        pins.push(pin(&(i + 1).to_string(), "", -7.5, y, PinDir::Left));
    }
    let (name, name_ja) = if input {
        (format!("PLC input module {n} points"), format!("PLC入力モジュール({n}点)"))
    } else {
        (format!("PLC output module {n} points"), format!("PLC出力モジュール({n}点)"))
    };
    let keywords: Vec<&str> = if input {
        vec!["plc", "input", "di", "module", "PLC", "入力", "モジュール", "シーケンサ"]
    } else {
        vec!["plc", "output", "do", "module", "PLC", "出力", "モジュール", "シーケンサ"]
    };
    Some(sym(id, &name, &name_ja, "plc", "PLC", &keywords, primitives, pins))
}

/// `connector_{n}p` / `terminal_block_{n}p` / `plc_di_{n}p` / `plc_do_{n}p` 形式のIDから
/// ピン数可変シンボルを生成する。
/// 端子は縦並び・ピッチ5mm・中央揃え(オフセットは常に2.5mmグリッド倍数)。
pub fn dynamic_symbol(id: &str) -> Option<SymbolDef> {
    let parse = |rest: &str| -> Option<usize> {
        let n: usize = rest.strip_suffix('p')?.parse().ok()?;
        (1..=DYNAMIC_PIN_MAX).contains(&n).then_some(n)
    };
    let offset = |i: usize, n: usize| (i as f64 - (n - 1) as f64 / 2.0) * 5.0;

    if let Some(n) = id.strip_prefix("connector_").and_then(parse) {
        let mut primitives = vec![Primitive::Rect {
            p1: p(-4.0, offset(0, n) - 2.5),
            p2: p(4.0, offset(n - 1, n) + 2.5),
            filled: false,
        }];
        let mut pins = Vec::with_capacity(n);
        for i in 0..n {
            let y = offset(i, n);
            primitives.push(Primitive::Text {
                at: p(-2.0, y),
                text: (i + 1).to_string(),
                height: 2.0,
            });
            primitives.push(line(&[(4.0, y), (7.5, y)]));
            pins.push(pin(&(i + 1).to_string(), "", 7.5, y, PinDir::Right));
        }
        return Some(sym(
            id,
            &format!("Connector {n}P"),
            &format!("コネクタ({n}極)"),
            "connector",
            "J",
            &["connector", "plug", "コネクタ", "接続器"],
            primitives,
            pins,
        ));
    }
    if let Some(n) = id.strip_prefix("terminal_block_").and_then(parse) {
        let mut primitives = vec![Primitive::Rect {
            p1: p(-2.5, offset(0, n) - 2.5),
            p2: p(2.5, offset(n - 1, n) + 2.5),
            filled: false,
        }];
        let mut pins = Vec::with_capacity(n * 2);
        for i in 0..n {
            let y = offset(i, n);
            let no = (i + 1).to_string();
            primitives.push(Primitive::Circle {
                center: p(0.0, y),
                r: 1.8,
                filled: false,
            });
            primitives.push(Primitive::Text {
                at: p(0.0, y - 1.0),
                text: no.clone(),
                height: 2.0,
            });
            primitives.push(line(&[(-2.5, y), (-1.8, y)]));
            primitives.push(line(&[(1.8, y), (2.5, y)]));
            // 貫通端子: 左右2接続点に同一ピン番号(ネットリストで内部短絡)
            pins.push(pin(&no, "", -2.5, y, PinDir::Left));
            pins.push(pin(&no, "", 2.5, y, PinDir::Right));
        }
        return Some(sym(
            id,
            &format!("Terminal block {n}P"),
            &format!("端子台({n}極)"),
            "connector",
            "TB",
            &["terminal block", "tb", "端子台", "端子"],
            primitives,
            pins,
        ));
    }
    plc_module_symbol(id)
}

/// symbol_idから定義を解決する。静的ライブラリ優先、なければ動的生成。
pub fn resolve_symbol(id: &str) -> Option<SymbolDef> {
    builtin_symbols()
        .into_iter()
        .find(|s| s.id == id)
        .or_else(|| dynamic_symbol(id))
}

/// シートの描画・ネットリストに必要な全シンボル定義(静的+使用中の動的)を集める。
pub fn sheet_symbol_defs(sheet: &crate::model::Sheet) -> Vec<SymbolDef> {
    let mut defs = builtin_symbols();
    let mut seen: std::collections::BTreeSet<&str> =
        defs.iter().map(|d| d.id.as_str()).collect();
    let mut ids: Vec<&str> = Vec::new();
    for e in sheet.entities.values() {
        if let crate::model::Entity::Symbol(s) = e {
            if !seen.contains(s.symbol_id.as_str()) {
                seen.insert(s.symbol_id.as_str());
                ids.push(s.symbol_id.as_str());
            }
        }
    }
    defs.extend(ids.into_iter().filter_map(dynamic_symbol));
    defs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every bundled symbol has a unique id and at least one pin.
    /// 同梱シンボルはすべて一意のidを持ち、最低1つのピンを持つ。
    #[test]
    fn builtin_symbols_have_unique_ids_and_pins() {
        let syms = builtin_symbols();
        assert!(syms.len() >= 10);
        let mut ids: Vec<_> = syms.iter().map(|s| s.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), syms.len(), "duplicate symbol ids");
        for s in &syms {
            assert!(!s.pins.is_empty(), "symbol {} has no pins", s.id);
        }
    }

    /// A parametric terminal block (e.g. 8 poles) has one left and one right connection point per terminal, all on the 2.5 mm grid and vertically centered.
    /// ピン数可変の端子台(例: 8極)は端子ごとに左右1点ずつの接続点を持ち、すべて2.5mmグリッド上・上下中央揃えになる。
    #[test]
    fn dynamic_terminal_block_has_through_pins_on_grid() {
        let def = resolve_symbol("terminal_block_8p").expect("dynamic terminal block");
        assert_eq!(def.ref_prefix, "TB");
        // 8端子 x 左右2接続点 = 16ピン、各番号がちょうど2回
        assert_eq!(def.pins.len(), 16);
        for i in 1..=8 {
            let same: Vec<_> = def
                .pins
                .iter()
                .filter(|p| p.number == i.to_string())
                .collect();
            assert_eq!(same.len(), 2, "terminal {i}");
            // 左右対称
            assert!((same[0].at.x + same[1].at.x).abs() < 1e-9);
            assert!((same[0].at.y - same[1].at.y).abs() < 1e-9);
        }
        // 全ピン2.5mmグリッド上、中央揃え(y合計=0)
        let mut ysum: f64 = 0.0;
        for p in &def.pins {
            assert!((p.at.x / 2.5 - (p.at.x / 2.5).round()).abs() < 1e-9, "{:?}", p.at);
            assert!((p.at.y / 2.5 - (p.at.y / 2.5).round()).abs() < 1e-9, "{:?}", p.at);
            ysum += p.at.y;
        }
        assert!(ysum.abs() < 1e-9);
    }

    /// connector_2p generated dynamically has exactly the same pin coordinates as the old static definition, so existing drawings are unaffected.
    /// 動的生成のconnector_2pは旧静的定義と完全に同じピン座標を持ち、既存図面に影響しない。
    #[test]
    fn dynamic_connector_2p_matches_legacy_static_def() {
        // 旧静的connector_2pと同一のピン座標(既存図面の互換性)
        let def = resolve_symbol("connector_2p").expect("dynamic connector");
        assert_eq!(def.ref_prefix, "J");
        let pins: Vec<_> = def.pins.iter().map(|p| (p.number.as_str(), p.at.x, p.at.y)).collect();
        assert_eq!(pins, vec![("1", 7.5, -2.5), ("2", 7.5, 2.5)]);
    }

    /// resolve_symbol finds built-in ids, and rejects malformed or out-of-range dynamic ids (0 poles, 51 poles, missing count).
    /// resolve_symbolは同梱idを見つけ、不正・範囲外の動的ID(0極・51極・数値なし)は拒否する。
    #[test]
    fn resolve_symbol_rejects_invalid_ids_and_finds_builtins() {
        assert!(resolve_symbol("resistor").is_some());
        assert!(resolve_symbol("connector_0p").is_none());
        assert!(resolve_symbol("connector_51p").is_none());
        assert!(resolve_symbol("connector_p").is_none());
        assert!(resolve_symbol("terminal_block_xp").is_none());
        assert!(resolve_symbol("unknown").is_none());
    }

    /// sheet_symbol_defs returns the built-in library plus definitions for every dynamic symbol actually used on the sheet.
    /// sheet_symbol_defsは同梱ライブラリに加え、シートで実際に使われている動的シンボルの定義を返す。
    #[test]
    fn sheet_symbol_defs_includes_dynamic_ids_in_use() {
        use crate::model::*;
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let s = Entity::Symbol(SymbolInstance {
            id: uuid::Uuid::new_v4(),
            symbol_id: "terminal_block_3p".into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: String::new(),
            attrs: Default::default(),
        });
        sheet.entities.insert(s.id(), s);
        let defs = sheet_symbol_defs(&sheet);
        assert!(defs.iter().any(|d| d.id == "resistor"), "builtin含む");
        assert!(defs.iter().any(|d| d.id == "terminal_block_3p"), "使用中の動的ID含む");
    }

    /// A PLC input module symbol (e.g. 16 points) is a tall box with one connection point per I/O point on its left side, all on the 2.5 mm grid.
    /// PLC入力モジュールのシンボル (例: 16点) は縦長の箱で、左側に点数ぶんの接続点が2.5mmグリッド上に並ぶ。
    #[test]
    fn a_plc_input_module_has_one_connection_point_per_io_point() {
        let def = resolve_symbol("plc_di_16p").expect("PLC入力モジュール");
        assert_eq!(def.ref_prefix, "PLC");
        assert_eq!(def.category, "plc");
        assert_eq!(def.pins.len(), 16);
        // ピン番号は点番号 (1〜16)、全て左向き・左端に揃う
        let numbers: Vec<String> = def.pins.iter().map(|p| p.number.clone()).collect();
        assert_eq!(numbers[0], "1");
        assert_eq!(numbers[15], "16");
        assert!(def.pins.iter().all(|p| p.dir == PinDir::Left));
        assert!(def.pins.windows(2).all(|w| w[0].at.x == w[1].at.x), "左端に一列");
        // 点ピッチは一定で、全ピンが2.5mmグリッド上・上下中央揃え
        let pitch = def.pins[1].at.y - def.pins[0].at.y;
        assert!((pitch - PLC_POINT_PITCH_MM).abs() < 1e-9, "点ピッチ: {pitch}");
        let mut ysum = 0.0;
        for p in &def.pins {
            assert!((p.at.x / 2.5 - (p.at.x / 2.5).round()).abs() < 1e-9, "{:?}", p.at);
            assert!((p.at.y / 2.5 - (p.at.y / 2.5).round()).abs() < 1e-9, "{:?}", p.at);
            ysum += p.at.y;
        }
        assert!(ysum.abs() < 1e-9, "上下中央揃え");
    }

    /// PLC modules come in an input and an output flavour, and both carry the PLC reference prefix.
    /// PLCモジュールには入力用と出力用があり、どちらも参照記号の接頭辞はPLCになる。
    #[test]
    fn plc_modules_come_in_input_and_output_flavours() {
        let di = resolve_symbol("plc_di_8p").expect("入力8点");
        let d_o = resolve_symbol("plc_do_8p").expect("出力8点");
        assert_eq!(di.pins.len(), 8);
        assert_eq!(d_o.pins.len(), 8);
        assert_eq!(d_o.ref_prefix, "PLC");
        assert_ne!(di.name, d_o.name, "名称で入出力が区別できる");
        assert!(d_o.name.contains("output") || d_o.name_ja.contains("出力"));
    }

    /// PLC module symbols exist from 1 to 64 points; anything outside that range is not a symbol.
    /// PLCモジュールのシンボルは1〜64点まで作れ、その外の点数はシンボルとして存在しない。
    #[test]
    fn plc_module_point_counts_are_limited_to_one_through_sixty_four() {
        assert!(resolve_symbol("plc_di_1p").is_some());
        assert!(resolve_symbol("plc_di_64p").is_some());
        assert!(resolve_symbol("plc_di_0p").is_none());
        assert!(resolve_symbol("plc_di_65p").is_none());
        assert!(resolve_symbol("plc_do_xp").is_none());
    }

    /// The relay coil symbol carries the JIS coil terminal names A1 and A2 and the reference prefix K.
    /// リレーコイルのシンボルは、JISのコイル端子記号A1・A2と参照記号の接頭辞Kを持つ。
    #[test]
    fn relay_coil_has_a1_a2_terminals() {
        let def = resolve_symbol("relay_coil").expect("relay coil");
        assert_eq!(def.ref_prefix, "K");
        assert_eq!(def.category, "relay");
        let numbers: Vec<&str> = def.pins.iter().map(|p| p.number.as_str()).collect();
        assert_eq!(numbers, vec!["A1", "A2"]);
    }

    /// Both relay contact types (make and break) exist, share the coil's reference prefix, and have their two connection points on the 2.5 mm grid.
    /// リレー接点はa接点・b接点の2種類があり、コイルと同じ参照記号の接頭辞を持ち、2つの接続点が2.5mmグリッド上にある。
    #[test]
    fn relay_contacts_come_in_make_and_break_types() {
        let no = resolve_symbol("relay_contact_no").expect("a接点");
        let nc = resolve_symbol("relay_contact_nc").expect("b接点");
        for def in [&no, &nc] {
            assert_eq!(def.ref_prefix, "K");
            assert_eq!(def.category, "relay");
            assert_eq!(def.pins.len(), 2, "{}", def.id);
            for pin in &def.pins {
                assert!((pin.at.x.abs() - 7.5).abs() < 1e-9, "{} {:?}", def.id, pin.at);
                assert!((pin.at.y / 2.5 - (pin.at.y / 2.5).round()).abs() < 1e-9);
            }
        }
        // b接点は接点を横切る線が1本多く、a接点と図形が違う
        assert!(nc.primitives.len() > no.primitives.len(), "b接点はa接点と区別できる図形");
    }

    /// Symbol definitions serialize to JSON and back without loss.
    /// シンボル定義はJSONに往復変換しても失われない。
    #[test]
    fn symbol_json_roundtrip() {
        let syms = builtin_symbols();
        let json = serde_json::to_string(&syms).unwrap();
        let back: Vec<SymbolDef> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), syms.len());
    }

    // ---- M4フェーズ3: ライブラリ拡充 (JIS C 0617 / IEC 60617) ----

    fn def(id: &str) -> SymbolDef {
        resolve_symbol(id).unwrap_or_else(|| panic!("シンボル {id} が無い"))
    }

    /// ピン番号→座標の一覧 (番号順)。
    fn pin_map(def: &SymbolDef) -> Vec<(String, f64, f64)> {
        let mut v: Vec<(String, f64, f64)> = def
            .pins
            .iter()
            .map(|p| (p.number.clone(), p.at.x, p.at.y))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.total_cmp(&b.2)));
        v
    }

    fn dir_of(def: &SymbolDef, number: &str) -> PinDir {
        def.pins.iter().find(|p| p.number == number).expect("pin").dir
    }

    /// The bundled library covers the JIS C 0617 main symbols at a 50-symbol scale, with unique ids.
    /// 同梱ライブラリはJIS C 0617の主要記号を50種規模で網羅し、idはすべて一意である。
    #[test]
    fn library_covers_the_main_jis_symbols() {
        let syms = builtin_symbols();
        assert!(syms.len() >= 45, "同梱記号は45種以上 (実際 {})", syms.len());
        let mut ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        ids.sort_unstable();
        let unique = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), unique, "idの重複");
    }

    /// Every pin sits on the 2.5 mm grid, has a unique number per connection point, and points away from the body of the symbol.
    /// すべてのピンは2.5mmグリッド上にあり、接続点ごとに番号が決まっていて、シンボル本体の外側を向いている。
    #[test]
    fn every_pin_is_on_the_grid_and_points_outwards() {
        for s in builtin_symbols() {
            let (min, max) = local_bounds(&s);
            let cx = (min.x + max.x) / 2.0;
            let cy = (min.y + max.y) / 2.0;
            for pin in &s.pins {
                assert!(!pin.number.is_empty(), "{}: 空のピン番号", s.id);
                for v in [pin.at.x, pin.at.y] {
                    assert!(
                        (v / 2.5 - (v / 2.5).round()).abs() < 1e-9,
                        "{}: ピン{}がグリッド外 {:?}",
                        s.id,
                        pin.number,
                        pin.at
                    );
                }
                let (dx, dy) = (pin.at.x - cx, pin.at.y - cy);
                let expect = if dx.abs() >= dy.abs() {
                    if dx >= 0.0 { PinDir::Right } else { PinDir::Left }
                } else if dy >= 0.0 {
                    PinDir::Down
                } else {
                    PinDir::Up
                };
                assert_eq!(pin.dir, expect, "{}: ピン{}の方向", s.id, pin.number);
            }
        }
    }

    /// Every symbol carries the four attribute slots (reference, part number, description, rating) placed clear of its outline.
    /// すべてのシンボルは4つの属性スロット(参照記号・型番・説明・定格)を持ち、外形に重ならない位置に置かれる。
    #[test]
    fn every_symbol_exposes_the_standard_attribute_slots() {
        for s in builtin_symbols() {
            let keys: Vec<&str> = s.text_slots.iter().map(|t| t.key.as_str()).collect();
            assert_eq!(keys, vec![SLOT_TAG, SLOT_PART, SLOT_DESC, SLOT_RATING], "{}", s.id);
            let (min, max) = local_bounds(&s);
            for slot in &s.text_slots {
                assert!(slot.height > 0.0, "{}: スロット{}の文字高", s.id, slot.key);
                let above = slot.at.y < min.y;
                let below = slot.at.y > max.y;
                assert!(above || below, "{}: スロット{}が外形に重なる", s.id, slot.key);
            }
        }
    }

    /// Symbols are listed grouped by category in the display order used by the insert dialog, and every symbol is searchable by English and Japanese keywords.
    /// シンボルは部品挿入ダイアログの表示順にカテゴリごとまとまって並び、英語・日本語のキーワードで検索できる。
    #[test]
    fn symbols_are_grouped_by_category_in_display_order() {
        let syms = builtin_symbols();
        let mut seen: Vec<&str> = Vec::new();
        for s in &syms {
            assert!(
                CATEGORY_ORDER.contains(&s.category.as_str()),
                "{}: 未知のカテゴリ {}",
                s.id,
                s.category
            );
            if seen.last() != Some(&s.category.as_str()) {
                assert!(
                    !seen.contains(&s.category.as_str()),
                    "{}: カテゴリ {} が離れて現れる",
                    s.id,
                    s.category
                );
                seen.push(s.category.as_str());
            }
        }
        let order: Vec<&str> = CATEGORY_ORDER
            .iter()
            .copied()
            .filter(|c| seen.contains(c))
            .collect();
        assert_eq!(seen, order, "カテゴリの並び順");
        for s in &syms {
            assert!(!s.keywords.is_empty(), "{}: 検索キーワードが無い", s.id);
            assert!(
                s.keywords.iter().any(|k| k.is_ascii()),
                "{}: 英語キーワードが無い",
                s.id
            );
            assert!(
                s.keywords.iter().any(|k| !k.is_ascii()),
                "{}: 日本語キーワードが無い",
                s.id
            );
        }
    }

    /// The symbols shipped in the first version keep their id, category, reference prefix, pin numbers and pin positions, so drawings made before the library grew still render identically.
    /// 初版から同梱している記号はid・カテゴリ・参照記号の接頭辞・ピン番号・ピン位置が変わらないため、ライブラリ拡充前に描いた図面もそのまま同じに描画される。
    #[test]
    fn legacy_symbols_keep_their_definition() {
        let lr = |n1: &str, n2: &str| {
            vec![(n1.to_string(), -7.5, 0.0), (n2.to_string(), 7.5, 0.0)]
        };
        let legacy: Vec<(&str, &str, &str, usize, Vec<(String, f64, f64)>)> = vec![
            ("resistor", "passive", "R", 3, lr("1", "2")),
            ("fuse", "protection", "F", 2, lr("1", "2")),
            ("capacitor", "passive", "C", 4, lr("1", "2")),
            ("diode", "semiconductor", "D", 4, lr("1", "2")),
            ("led", "semiconductor", "LED", 8, lr("1", "2")),
            ("switch_spst", "switch", "SW", 5, lr("1", "2")),
            ("pushbutton_no", "switch", "PB", 7, lr("1", "2")),
            ("relay_coil", "relay", "K", 3, lr("A1", "A2")),
            ("relay_contact_no", "relay", "K", 3, lr("1", "2")),
            ("relay_contact_nc", "relay", "K", 4, lr("1", "2")),
            ("lamp", "output", "L", 5, lr("1", "2")),
            ("motor", "output", "M", 4, lr("1", "2")),
            ("battery", "power", "BT", 5, lr("1", "2")),
            ("ground", "power", "GND", 4, vec![("1".into(), 0.0, 0.0)]),
            ("terminal", "connector", "T", 2, vec![("1".into(), 5.0, 0.0)]),
        ];
        for (id, category, prefix, prims, pins) in legacy {
            let d = def(id);
            assert_eq!(d.category, category, "{id}");
            assert_eq!(d.ref_prefix, prefix, "{id}");
            assert_eq!(d.primitives.len(), prims, "{id}: 図形の数");
            assert_eq!(pin_map(&d), pins, "{id}: ピン");
        }
    }

    /// The three-pole contactor draws one make contact per pole with the contactor半円, terminals 1/2, 3/4, 5/6 and a dashed mechanical link.
    /// 電磁接触器(3極)は極ごとにコンタクタの半円付きa接点を描き、端子は1/2・3/4・5/6、極をつなぐ連動線は破線で表す。
    #[test]
    fn contactor_3p_has_three_ganged_main_contacts() {
        let d = def("contactor_3p");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("relay", "K"));
        assert_eq!(
            pin_map(&d),
            vec![
                ("1".into(), -5.0, -7.5),
                ("2".into(), -5.0, 7.5),
                ("3".into(), 0.0, -7.5),
                ("4".into(), 0.0, 7.5),
                ("5".into(), 5.0, -7.5),
                ("6".into(), 5.0, 7.5),
            ]
        );
        assert_eq!(dir_of(&d, "1"), PinDir::Up);
        assert_eq!(dir_of(&d, "2"), PinDir::Down);
        // 極ごとに1つ、固定接点のコンタクタ半円
        let arcs = d.primitives.iter().filter(|p| matches!(p, Primitive::Arc { .. })).count();
        assert_eq!(arcs, 3, "コンタクタ半円は極ごとに1つ");
    }

    /// The single-pole circuit breaker (MCB) is a make contact whose fixed contact carries the breaker cross, with terminals 1 and 2.
    /// 単極の配線用遮断器(MCB)は固定接点に遮断器の×印を付けたa接点で、端子は1と2である。
    #[test]
    fn circuit_breaker_1p_marks_the_fixed_contact_with_a_cross() {
        let d = def("breaker_1p");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("protection", "CB"));
        assert_eq!(pin_map(&d), vec![("1".into(), 0.0, -7.5), ("2".into(), 0.0, 7.5)]);
        assert_eq!(dir_of(&d, "1"), PinDir::Up);
    }

    /// The two- and three-pole circuit breakers repeat the pole at a 5 mm pitch and number the terminals 1/2, 3/4 (and 5/6).
    /// 2極・3極の配線用遮断器は極を5mmピッチで並べ、端子を1/2・3/4(・5/6)と振る。
    #[test]
    fn circuit_breaker_multipole_numbers_terminals_by_pole() {
        let d2 = def("breaker_2p");
        assert_eq!(
            pin_map(&d2),
            vec![
                ("1".into(), -2.5, -7.5),
                ("2".into(), -2.5, 7.5),
                ("3".into(), 2.5, -7.5),
                ("4".into(), 2.5, 7.5),
            ]
        );
        let d3 = def("breaker_3p");
        assert_eq!(d3.pins.len(), 6);
        assert_eq!(pin_map(&d3)[4], ("5".into(), 5.0, -7.5));
    }

    /// The disconnector marks its fixed contact with a short bar at right angles to the blade instead of the breaker cross.
    /// 断路器は固定接点を、遮断器の×印ではなく可動接点に直交する短い棒で表す。
    #[test]
    fn disconnector_marks_the_fixed_contact_with_a_bar() {
        let d = def("disconnector_1p");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("protection", "DS"));
        assert_eq!(pin_map(&d), vec![("1".into(), 0.0, -7.5), ("2".into(), 0.0, 7.5)]);
        // ×印(2本)を持つ遮断器より図形が少ない = 短棒1本で表している
        assert!(d.primitives.len() < def("breaker_1p").primitives.len());
    }

    /// The three-pole disconnector gangs three blades with terminals 1/2, 3/4, 5/6.
    /// 3極の断路器は3つの可動接点を連動させ、端子は1/2・3/4・5/6になる。
    #[test]
    fn disconnector_3p_gangs_three_blades() {
        let d = def("disconnector_3p");
        assert_eq!(d.ref_prefix, "DS");
        assert_eq!(d.pins.len(), 6);
        assert_eq!(pin_map(&d)[5], ("6".into(), 5.0, 7.5));
    }

    /// The break-contact pushbutton keeps the two connection points of the make type and adds the button actuator.
    /// b接点の押しボタンは、a接点と同じ2つの接続点を持ち、押しボタンの操作子が付く。
    #[test]
    fn pushbutton_nc_is_a_break_contact_with_a_button_actuator() {
        let d = def("pushbutton_nc");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("switch", "PB"));
        assert_eq!(pin_map(&d), vec![("1".into(), -7.5, 0.0), ("2".into(), 7.5, 0.0)]);
        assert!(d.primitives.len() > def("relay_contact_nc").primitives.len(), "操作子の分だけ図形が多い");
    }

    /// The emergency stop is a break contact with the mushroom head actuator, so it opens the circuit when hit.
    /// 非常停止は、きのこ形の頭部を持つb接点で、叩くと回路が開く。
    #[test]
    fn emergency_stop_is_a_break_contact_with_a_mushroom_head() {
        let d = def("emergency_stop");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("switch", "PB"));
        assert_eq!(pin_map(&d), vec![("1".into(), -7.5, 0.0), ("2".into(), 7.5, 0.0)]);
        assert!(
            d.primitives.iter().any(|p| matches!(p, Primitive::Arc { .. })),
            "きのこ形の頭部は半円で描く"
        );
    }

    /// The changeover switch has one common terminal and two fixed contacts, with the blade resting on the break side while it is not operated.
    /// 切替スイッチは共通端子1つと固定接点2つを持ち、操作していない状態では可動接点がb接点側に載っている。
    #[test]
    fn switch_spdt_has_a_common_and_two_fixed_contacts() {
        let d = def("switch_spdt");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("switch", "SW"));
        assert_eq!(
            pin_map(&d),
            vec![
                ("1".into(), -7.5, 0.0),
                ("2".into(), 7.5, -2.5),
                ("3".into(), 7.5, 2.5),
            ]
        );
        assert_eq!(dir_of(&d, "1"), PinDir::Left);
        assert_eq!(dir_of(&d, "2"), PinDir::Right);
    }

    /// The three-position switch shows the blade in the neutral centre position, touching neither fixed contact.
    /// 3位置切替スイッチは可動接点が中立位置にあり、どちらの固定接点にも接触していない状態で描く。
    #[test]
    fn switch_3pos_shows_the_blade_in_neutral() {
        let d = def("switch_3pos");
        assert_eq!(d.pins.len(), 3);
        assert_eq!(d.ref_prefix, "SW");
        // 中立の可動接点は水平 (y=0のまま終わる)
        let neutral = d.primitives.iter().any(|p| match p {
            Primitive::Line { pts } => {
                pts.len() == 2 && pts[0].y == 0.0 && pts[1].y == 0.0 && pts[1].x > 0.0 && pts[1].x < 2.5
            }
            _ => false,
        });
        assert!(neutral, "中立位置の可動接点");
    }

    /// The limit switch comes as a make and a break type, both driven by the position-switch actuator (a filled square on the rod).
    /// リミットスイッチはa接点・b接点の2種類があり、どちらも位置スイッチの操作子(ロッド先端の塗りつぶし四角)で動く。
    #[test]
    fn limit_switch_comes_in_make_and_break_types() {
        for id in ["limit_switch_no", "limit_switch_nc"] {
            let d = def(id);
            assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("switch", "LS"), "{id}");
            assert_eq!(pin_map(&d), vec![("1".into(), -7.5, 0.0), ("2".into(), 7.5, 0.0)], "{id}");
            assert!(
                d.primitives
                    .iter()
                    .any(|p| matches!(p, Primitive::Rect { filled: true, .. })),
                "{id}: 操作子の塗り四角"
            );
        }
        assert!(
            def("limit_switch_nc").primitives.len() > def("limit_switch_no").primitives.len(),
            "b接点はa接点と区別できる図形"
        );
    }

    /// The relay changeover contact uses the IEC terminal numbers 11 (common), 12 (break) and 14 (make).
    /// リレーの切替接点(c接点)は、IECの端子番号11(共通)・12(b接点)・14(a接点)を使う。
    #[test]
    fn relay_contact_co_uses_iec_terminal_numbers() {
        let d = def("relay_contact_co");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("relay", "K"));
        assert_eq!(
            pin_map(&d),
            vec![
                ("11".into(), -7.5, 0.0),
                ("12".into(), 7.5, 2.5),
                ("14".into(), 7.5, -2.5),
            ]
        );
    }

    /// The two-winding transformer draws both windings as arcs on either side of the core, with primary terminals 1/2 and secondary 3/4.
    /// 変圧器(2巻線)は鉄心を挟んで両側に巻線を半円で描き、一次側が端子1/2、二次側が端子3/4になる。
    #[test]
    fn transformer_has_two_windings_around_a_core() {
        let d = def("transformer");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("power", "T"));
        assert_eq!(
            pin_map(&d),
            vec![
                ("1".into(), -2.5, -7.5),
                ("2".into(), -2.5, 7.5),
                ("3".into(), 2.5, -7.5),
                ("4".into(), 2.5, 7.5),
            ]
        );
        let arcs = d.primitives.iter().filter(|p| matches!(p, Primitive::Arc { .. })).count();
        assert!(arcs >= 6, "巻線は片側3山ずつ (実際 {arcs})");
    }

    /// The AC source is a circle with a sine wave inside and two connection points.
    /// 交流電源は円の中に正弦波を描いた記号で、接続点は2つである。
    #[test]
    fn ac_source_is_a_circle_with_a_sine_wave() {
        let d = def("ac_source");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("power", "G"));
        assert_eq!(pin_map(&d), vec![("1".into(), -7.5, 0.0), ("2".into(), 7.5, 0.0)]);
        assert!(d.primitives.iter().any(|p| matches!(p, Primitive::Circle { .. })));
    }

    /// The bridge rectifier is a diamond with a diode inside, the AC terminals on the left and right and the DC terminals on top (+) and bottom (-).
    /// 整流器(ブリッジ)は菱形の中にダイオードを描いた記号で、交流側が左右、直流側が上(+)と下(-)の端子になる。
    #[test]
    fn rectifier_bridge_has_ac_and_dc_terminals() {
        let d = def("rectifier_bridge");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("power", "D"));
        assert_eq!(
            pin_map(&d),
            vec![
                ("1".into(), -7.5, 0.0),
                ("2".into(), 7.5, 0.0),
                ("3".into(), 0.0, -7.5),
                ("4".into(), 0.0, 7.5),
            ]
        );
        let names: Vec<&str> = d.pins.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"+") && names.contains(&"-"), "直流側の極性を名前で示す");
    }

    /// Protective earth encloses the earth symbol in a circle, while functional (frame) earth uses the chassis symbol; both have a single connection point.
    /// 保護接地は接地記号を円で囲み、機能接地(フレーム接地)はシャーシ記号で表す。どちらも接続点は1つ。
    #[test]
    fn earth_symbols_distinguish_protective_and_frame_earth() {
        let pe = def("earth_protective");
        assert_eq!((pe.category.as_str(), pe.ref_prefix.as_str()), ("power", "PE"));
        assert_eq!(pin_map(&pe), vec![("1".into(), 0.0, 0.0)]);
        assert!(pe.primitives.iter().any(|p| matches!(p, Primitive::Circle { .. })), "囲みの円");
        let fg = def("frame_ground");
        assert_eq!((fg.category.as_str(), fg.ref_prefix.as_str()), ("power", "FG"));
        assert_eq!(pin_map(&fg), vec![("1".into(), 0.0, 0.0)]);
        assert!(!fg.primitives.iter().any(|p| matches!(p, Primitive::Circle { .. })));
    }

    /// The zener diode keeps the diode outline and bends both ends of the cathode bar.
    /// ツェナーダイオードはダイオードの形を保ちつつ、陰極バーの両端を折り曲げた形で描く。
    #[test]
    fn zener_diode_bends_the_cathode_bar() {
        let d = def("zener_diode");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("semiconductor", "D"));
        assert_eq!(pin_map(&d), vec![("1".into(), -7.5, 0.0), ("2".into(), 7.5, 0.0)]);
        let names: Vec<&str> = d.pins.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["A", "K"], "陽極・陰極");
    }

    /// The polarized capacitor draws one plate solid and marks the positive terminal.
    /// 有極性コンデンサは片方の極板を塗りつぶし、プラス側の端子を「+」で示す。
    #[test]
    fn capacitor_polarized_marks_the_positive_plate() {
        let d = def("capacitor_polarized");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("passive", "C"));
        assert_eq!(pin_map(&d), vec![("1".into(), -7.5, 0.0), ("2".into(), 7.5, 0.0)]);
        assert!(d.primitives.iter().any(|p| matches!(p, Primitive::Rect { filled: true, .. })));
    }

    /// The inductor is drawn as a row of half circles on the conductor.
    /// インダクタ(コイル)は導線の上に並んだ半円で描く。
    #[test]
    fn inductor_is_a_row_of_half_circles() {
        let d = def("inductor");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("passive", "L"));
        assert_eq!(pin_map(&d), vec![("1".into(), -7.5, 0.0), ("2".into(), 7.5, 0.0)]);
        let arcs = d.primitives.iter().filter(|p| matches!(p, Primitive::Arc { .. })).count();
        assert_eq!(arcs, 4);
    }

    /// The variable resistor adds an arrow across the resistor body.
    /// 可変抵抗器は抵抗器の外形を斜めに貫く矢印を加えた形で描く。
    #[test]
    fn resistor_variable_adds_an_arrow_across_the_body() {
        let d = def("resistor_variable");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("passive", "VR"));
        assert_eq!(pin_map(&d), vec![("1".into(), -7.5, 0.0), ("2".into(), 7.5, 0.0)]);
        assert!(d.primitives.len() > def("resistor").primitives.len());
    }

    /// The varistor is a resistor crossed by an oblique line and labelled U, marking it as voltage dependent.
    /// バリスタは抵抗器を斜線が貫き「U」を添えた形で、電圧に依存する抵抗であることを示す。
    #[test]
    fn varistor_is_a_voltage_dependent_resistor() {
        let d = def("varistor");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("passive", "RV"));
        assert_eq!(pin_map(&d), vec![("1".into(), -7.5, 0.0), ("2".into(), 7.5, 0.0)]);
        assert!(
            d.primitives
                .iter()
                .any(|p| matches!(p, Primitive::Text { text, .. } if text == "U")),
            "電圧依存を示すU"
        );
    }

    /// The three-phase motor has the three phase terminals U, V and W leaving the top of the circle.
    /// 三相電動機は円の上側にU・V・Wの3つの相端子を持つ。
    #[test]
    fn motor_3ph_has_u_v_w_terminals() {
        let d = def("motor_3ph");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("output", "M"));
        assert_eq!(
            pin_map(&d),
            vec![
                ("U".into(), -2.5, -10.0),
                ("V".into(), 0.0, -10.0),
                ("W".into(), 2.5, -10.0),
            ]
        );
        assert_eq!(dir_of(&d, "U"), PinDir::Up);
        assert!(
            d.primitives
                .iter()
                .any(|p| matches!(p, Primitive::Text { text, .. } if text == "3~")),
            "三相を示す3~"
        );
    }

    /// The single-phase and DC motors share the motor circle and are told apart by the 1~ and DC marks inside.
    /// 単相電動機と直流電動機は電動機の円を共有し、中に書く「1~」と直流記号で区別する。
    #[test]
    fn motor_1ph_and_dc_are_told_apart_by_the_mark_inside() {
        let ac = def("motor_1ph");
        let dc = def("motor_dc");
        for d in [&ac, &dc] {
            assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("output", "M"), "{}", d.id);
            assert_eq!(d.pins.len(), 2, "{}", d.id);
        }
        assert!(ac
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Text { text, .. } if text == "1~")));
        assert!(!dc
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Text { text, .. } if text == "1~")));
    }

    /// The bell is a dome (half circle on its base line) with two connection points.
    /// ベルは底辺の上に半円を載せたドーム形で、接続点は2つである。
    #[test]
    fn bell_is_a_dome_with_two_terminals() {
        let d = def("bell");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("output", "BL"));
        assert_eq!(pin_map(&d), vec![("1".into(), -7.5, 0.0), ("2".into(), 7.5, 0.0)]);
        assert!(d.primitives.iter().any(|p| matches!(p, Primitive::Arc { .. })));
    }

    /// The voltmeter and ammeter are circles marked V and A, wired in the circuit like any two-terminal instrument.
    /// 電圧計・電流計は「V」「A」を書いた円で、2端子の計器として回路に入れる。
    #[test]
    fn voltmeter_and_ammeter_are_circles_marked_v_and_a() {
        for (id, mark, prefix) in [("voltmeter", "V", "VM"), ("ammeter", "A", "AM")] {
            let d = def(id);
            assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("instrument", prefix), "{id}");
            assert_eq!(pin_map(&d), vec![("1".into(), -7.5, 0.0), ("2".into(), 7.5, 0.0)], "{id}");
            assert!(
                d.primitives
                    .iter()
                    .any(|p| matches!(p, Primitive::Text { text, .. } if text == mark)),
                "{id}: 計器の文字"
            );
        }
    }

    /// The current transformer has the primary conductor passing through the core and two secondary terminals S1/S2.
    /// 変流器は一次導体が鉄心を貫き、二次側にS1・S2の2端子を持つ。
    #[test]
    fn current_transformer_has_primary_through_and_secondary_terminals() {
        let d = def("current_transformer");
        assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("instrument", "CT"));
        assert_eq!(
            pin_map(&d),
            vec![
                ("P1".into(), 0.0, -7.5),
                ("P2".into(), 0.0, 7.5),
                ("S1".into(), 7.5, -2.5),
                ("S2".into(), 7.5, 2.5),
            ]
        );
        assert_eq!(dir_of(&d, "S1"), PinDir::Right);
    }

    /// The plug and the socket of a connector pair face each other: the plug is a wedge, the socket the cup that receives it.
    /// 差込接続器のプラグとソケットは向かい合う形で、プラグはくさび形、ソケットはそれを受ける半円形になる。
    #[test]
    fn connector_plug_and_socket_face_each_other() {
        let plug = def("connector_plug");
        let socket = def("connector_socket");
        for d in [&plug, &socket] {
            assert_eq!((d.category.as_str(), d.ref_prefix.as_str()), ("connector", "J"), "{}", d.id);
            assert_eq!(d.pins.len(), 1, "{}", d.id);
        }
        assert_eq!(pin_map(&plug), vec![("1".into(), -7.5, 0.0)]);
        assert_eq!(pin_map(&socket), vec![("1".into(), 7.5, 0.0)]);
        assert_eq!(dir_of(&plug, "1"), PinDir::Left);
        assert_eq!(dir_of(&socket, "1"), PinDir::Right);
    }

    /// A multi-pole device conducts pole by pole (terminals 1-2, 3-4, 5-6), so the phases are never treated as connected to each other.
    /// 多極機器は極ごと(端子1-2・3-4・5-6)に導通するため、相どうしがつながっている扱いにはならない。
    #[test]
    fn multipole_devices_conduct_pole_by_pole() {
        let breaker = def("breaker_3p");
        assert_eq!(
            conducting_pin_groups(&breaker),
            vec![vec!["1", "2"], vec!["3", "4"], vec!["5", "6"]]
        );
        // 2端子の機器は従来どおり1グループ
        let spst = def("switch_spst");
        assert_eq!(conducting_pin_groups(&spst), vec![vec!["1", "2"]]);
        // 番号が連番でない切替接点は1グループのまま (共通端子で全体がつながる)
        let co = def("relay_contact_co");
        assert_eq!(conducting_pin_groups(&co).len(), 1);
    }
}
