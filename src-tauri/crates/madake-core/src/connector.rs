//! ブロックのピンのコネクタの絵 (USB・3.5mm・DC・端子台・JST)。
//!
//! ピン表のピンが `connector` (どの形のコネクタか) を持つと、外形の辺にそのコネクタの小さな絵を描く。
//! 絵は既存のプリミティブ (線・円・弧・長方形) だけで作り、ピンが外形と交わる点 E を原点にした
//! ローカル座標 (u = 外向き、v = 辺に沿う向き) で決める。`connector` は絵だけで、接点・ネットは変えない。
//! TS側 `src/canvas/connector.ts` と同じ数・同じ順 (`tests/golden/connector-blocks.json` で突き合わせる)。

use serde::Deserialize;

use crate::block::BlockPin;
use crate::geometry::Point;
use crate::symbol::{PinDir, Primitive};

/// 絵の本体が外形の内側に入る深さ (mm)。
pub const CONNECTOR_IN_MM: f64 = 1.5;
/// 絵の本体が外形の外側へ出る長さ (mm)。
pub const CONNECTOR_OUT_MM: f64 = 2.5;
/// 本体の辺に沿う長さの半分 (mm)。本体は4mm。
pub const CONNECTOR_HALF_SPAN_MM: f64 = 2.0;
/// コネクタのあるピンの、外形から接続点までの長さ (mm)。
pub const CONNECTOR_LEAD_MM: f64 = 5.0;
/// 本体の外端からピン番号までのすき間 (mm)。
pub const CONNECTOR_NUM_GAP_MM: f64 = 0.25;
/// 上下辺のピン番号を、リードからこれだけ右へずらして書く (mm)。
pub const CONNECTOR_NUM_SIDE_MM: f64 = 0.5;
/// 1本だけのまとまりの名前を、ピン名の後ろにこれだけ空けて書く (mm)。
pub const CONNECTOR_LABEL_GAP_MM: f64 = 1.0;
/// 上下辺のまとまりの名前の中心の、外形からの深さ (mm)。
pub const CONNECTOR_LABEL_TOP_MM: f64 = 7.0;
/// まとまりの名前を書く上下辺が外形の内側で使う高さ (mm)。
pub const CONNECTOR_LABEL_INSET_MM: f64 = 10.0;

/// コネクタの形 (13種、決め打ち)。`_f` = メス (機器側の口)、`_m` = オス (プラグ)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    UsbAF,
    UsbAM,
    UsbCF,
    UsbCM,
    MicroBF,
    MicroBM,
    Jack35F,
    Jack35M,
    DcF,
    DcM,
    Screw,
    JstPhF,
    JstXhF,
}

impl ConnectorKind {
    /// 全種類 (仕様の表の順。エディタの選択肢もこの順)。
    pub const ALL: [ConnectorKind; 13] = [
        ConnectorKind::UsbAF,
        ConnectorKind::UsbAM,
        ConnectorKind::UsbCF,
        ConnectorKind::UsbCM,
        ConnectorKind::MicroBF,
        ConnectorKind::MicroBM,
        ConnectorKind::Jack35F,
        ConnectorKind::Jack35M,
        ConnectorKind::DcF,
        ConnectorKind::DcM,
        ConnectorKind::Screw,
        ConnectorKind::JstPhF,
        ConnectorKind::JstXhF,
    ];

    /// JSON での名前 (`usb_a_f` など)。
    pub fn name(self) -> &'static str {
        match self {
            ConnectorKind::UsbAF => "usb_a_f",
            ConnectorKind::UsbAM => "usb_a_m",
            ConnectorKind::UsbCF => "usb_c_f",
            ConnectorKind::UsbCM => "usb_c_m",
            ConnectorKind::MicroBF => "micro_b_f",
            ConnectorKind::MicroBM => "micro_b_m",
            ConnectorKind::Jack35F => "jack35_f",
            ConnectorKind::Jack35M => "jack35_m",
            ConnectorKind::DcF => "dc_f",
            ConnectorKind::DcM => "dc_m",
            ConnectorKind::Screw => "screw",
            ConnectorKind::JstPhF => "jst_ph_f",
            ConnectorKind::JstXhF => "jst_xh_f",
        }
    }

    /// 並んだ数本のピンで1つの本体を描く種類 (端子台・JST)。`connector_ref` はこれだけが持てる。
    pub fn is_grouped(self) -> bool {
        matches!(self, ConnectorKind::Screw | ConnectorKind::JstPhF | ConnectorKind::JstXhF)
    }

    /// JST のシリーズ名 (`PH` / `XH`)。他は `None`。
    pub fn series(self) -> Option<&'static str> {
        match self {
            ConnectorKind::JstPhF => Some("PH"),
            ConnectorKind::JstXhF => Some("XH"),
            _ => None,
        }
    }
}

/// 1辺の中の、1つの絵で描くピンのまとまり (`start..end` は辺のピン表の添字)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorGroup {
    pub kind: ConnectorKind,
    /// 前後の空白を落とした `connector_ref`。無ければ `None`。
    pub reference: Option<String>,
    pub start: usize,
    pub end: usize,
}

/// 1辺のピンをまとまりに分ける。まとめる種類のピンは、すぐ前のピンと種類も `connector_ref` も
/// 同じなら同じまとまりに入る (無い同士も同じ)。まとめない種類は1本で1つ。`connector` の無いピンは入らない。
pub fn connector_groups(side: &[BlockPin]) -> Vec<ConnectorGroup> {
    let mut groups: Vec<ConnectorGroup> = Vec::new();
    for (i, pin) in side.iter().enumerate() {
        let Some(kind) = pin.connector else { continue };
        let reference = pin.connector_ref.as_deref().map(|r| r.trim().to_string());
        if kind.is_grouped() {
            if let Some(last) = groups.last_mut() {
                if last.end == i && last.kind == kind && last.reference == reference {
                    last.end = i + 1;
                    continue;
                }
            }
        }
        groups.push(ConnectorGroup { kind, reference, start: i, end: i + 1 });
    }
    groups
}

/// まとまりの名前: JST は `<ref> PH` / `<ref> XH` (ref が無ければ `PH` / `XH`)、端子台は `<ref>` (無ければ書かない)。
pub fn group_label(group: &ConnectorGroup) -> Option<String> {
    match (group.kind.series(), &group.reference) {
        (Some(series), Some(r)) => Some(format!("{r} {series}")),
        (Some(series), None) => Some(series.to_string()),
        (None, Some(r)) if group.kind.is_grouped() => Some(r.clone()),
        _ => None,
    }
}

/// 辺ごとのローカル座標 (右辺の座標を時計回りに回したもの)。
struct Frame {
    e: Point,
    u: Point,
    v: Point,
    theta: u16,
}

fn frame(dir: PinDir, e: Point) -> Frame {
    let (u, v, theta) = match dir {
        PinDir::Right => ((1.0, 0.0), (0.0, 1.0), 0),
        PinDir::Down => ((0.0, 1.0), (-1.0, 0.0), 90),
        PinDir::Left => ((-1.0, 0.0), (0.0, -1.0), 180),
        PinDir::Up => ((0.0, -1.0), (1.0, 0.0), 270),
    };
    Frame { e, u: Point::new(u.0, u.1), v: Point::new(v.0, v.1), theta }
}

impl Frame {
    fn at(&self, u: f64, v: f64) -> Point {
        Point::new(
            self.e.x + u * self.u.x + v * self.v.x,
            self.e.y + u * self.u.y + v * self.v.y,
        )
    }

    fn line(&self, pts: &[(f64, f64)]) -> Primitive {
        Primitive::Line { pts: pts.iter().map(|&(u, v)| self.at(u, v)).collect() }
    }

    /// 2隅を写してから (小さい方, 大きい方) に並べ直す。
    fn rect(&self, (u1, v1): (f64, f64), (u2, v2): (f64, f64), filled: bool) -> Primitive {
        let (a, b) = (self.at(u1, v1), self.at(u2, v2));
        Primitive::Rect {
            p1: Point::new(a.x.min(b.x), a.y.min(b.y)),
            p2: Point::new(a.x.max(b.x), a.y.max(b.y)),
            filled,
        }
    }

    fn circle(&self, (u, v): (f64, f64), r: f64, filled: bool) -> Primitive {
        Primitive::Circle { center: self.at(u, v), r, filled }
    }

    /// 角度は辺の回転ぶん足す (開始は0〜360へ、振れ幅はそのまま。時計回りは時計回りのまま)。
    fn arc(&self, (u, v): (f64, f64), r: f64, start_deg: f64, end_deg: f64) -> Primitive {
        let (start_deg, end_deg) =
            crate::netlist::rotate_arc_angles(start_deg, end_deg, self.theta, false);
        Primitive::Arc { center: self.at(u, v), r, start_deg, end_deg }
    }
}

/// 1つのまとまり (または1本のピン) の絵。`edges` はそのピンたちが外形と交わる点 (辺の並び順)。
/// リード・番号・名前は含まない。
pub fn connector_primitives(kind: ConnectorKind, dir: PinDir, edges: &[Point]) -> Vec<Primitive> {
    let f = frame(dir, edges[0]);
    let vs: Vec<f64> = edges
        .iter()
        .map(|p| (p.x - f.e.x) * f.v.x + (p.y - f.e.y) * f.v.y)
        .collect();
    let v0 = vs.iter().copied().fold(f64::INFINITY, f64::min);
    let v1 = vs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (i, o, s) = (-CONNECTOR_IN_MM, CONNECTOR_OUT_MM, CONNECTOR_HALF_SPAN_MM);
    // メス = 辺にまたがる白い口、オス = 内側の枠と外へ出る黒い差し込み
    let female = || f.rect((i, -s), (o, s), false);
    let housing = || f.rect((i, -s), (0.0, s), false);
    match kind {
        ConnectorKind::UsbAF => vec![female(), f.rect((-0.8, -0.9), (1.8, -0.1), true)],
        ConnectorKind::UsbAM => vec![housing(), f.rect((0.0, -1.2), (o, 1.2), true)],
        ConnectorKind::UsbCF => vec![
            female(),
            f.line(&[(-0.2, -0.8), (1.2, -0.8)]),
            f.arc((1.2, 0.0), 0.8, -90.0, 90.0),
            f.line(&[(1.2, 0.8), (-0.2, 0.8)]),
            f.arc((-0.2, 0.0), 0.8, 90.0, 270.0),
            f.line(&[(0.0, 0.0), (1.0, 0.0)]),
        ],
        ConnectorKind::UsbCM => vec![
            housing(),
            f.rect((0.0, -0.8), (1.7, 0.8), true),
            f.circle((1.7, 0.0), 0.8, true),
        ],
        ConnectorKind::MicroBF => vec![
            f.line(&[(i, -s), (0.0, -s), (o, -1.4), (o, 1.4), (0.0, s), (i, s), (i, -s)]),
            f.line(&[(-0.6, 0.0), (1.6, 0.0)]),
        ],
        ConnectorKind::MicroBM => vec![housing(), f.rect((0.0, -0.6), (o, 0.6), true)],
        ConnectorKind::Jack35F => vec![
            female(),
            f.circle((0.5, 0.0), 1.0, false),
            f.circle((0.5, 0.0), 0.35, true),
        ],
        ConnectorKind::Jack35M => vec![
            housing(),
            f.rect((0.0, -1.0), (0.8, 1.0), true),
            f.rect((0.8, -0.4), (o, 0.4), true),
        ],
        ConnectorKind::DcF => vec![
            female(),
            f.circle((0.5, 0.0), 1.5, false),
            f.circle((0.5, 0.0), 0.6, true),
        ],
        ConnectorKind::DcM => vec![
            housing(),
            f.rect((0.0, -1.4), (o, -0.5), true),
            f.rect((0.0, 0.5), (o, 1.4), true),
        ],
        ConnectorKind::Screw => {
            let mut out = vec![f.rect((i, v0 - s), (o, v1 + s), false)];
            for &v in &vs {
                out.push(f.circle((0.5, v), 1.2, false));
                out.push(f.line(&[(1.3, v - 0.8), (-0.3, v + 0.8)]));
            }
            out
        }
        ConnectorKind::JstPhF | ConnectorKind::JstXhF => {
            let mut out = vec![
                f.rect((i, v0 - s), (o, v1 + s), false),
                f.line(&[(1.5, v0 - s), (1.5, v1 + s)]),
            ];
            for &v in &vs {
                out.push(f.rect((-0.6, v - 0.6), (0.6, v + 0.6), false));
            }
            out
        }
    }
}

/// テスト用: プリミティブが占める範囲 (最小, 最大)。円・弧は中心±半径。
#[cfg(test)]
pub(crate) fn primitive_extent(p: &Primitive) -> (Point, Point) {
    let pts = match p {
        Primitive::Line { pts } => pts.clone(),
        Primitive::Rect { p1, p2, .. } => vec![*p1, *p2],
        Primitive::Circle { center, r, .. } | Primitive::Arc { center, r, .. } => vec![
            Point::new(center.x - r, center.y - r),
            Point::new(center.x + r, center.y + r),
        ],
        Primitive::Text { at, .. } => vec![*at],
    };
    let lo = pts.iter().fold(Point::new(f64::INFINITY, f64::INFINITY), |a, p| Point::new(a.x.min(p.x), a.y.min(p.y)));
    let hi = pts.iter().fold(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| Point::new(a.x.max(p.x), a.y.max(p.y)));
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockPins;

    fn left_side(json: &str) -> Vec<BlockPin> {
        let pins: BlockPins = serde_json::from_str(json).expect("ピン表");
        pins.left
    }

    fn spans(json: &str) -> Vec<(usize, usize)> {
        connector_groups(&left_side(json)).iter().map(|g| (g.start, g.end)).collect()
    }

    fn label(json: &str) -> Option<String> {
        group_label(&connector_groups(&left_side(json))[0])
    }

    /// Every connector kind is read from its snake_case name (usb_a_f … jst_xh_f); only screw terminals and JST sockets are grouped, and only JST has a series.
    /// コネクタの種類はどれも snake_case の名前 (usb_a_f … jst_xh_f) で読め、まとめて描くのは端子台と JST だけで、シリーズ名を持つのは JST だけ。
    #[test]
    fn every_connector_kind_has_its_json_name() {
        let names: Vec<&str> = ConnectorKind::ALL.iter().map(|k| k.name()).collect();
        assert_eq!(
            names,
            [
                "usb_a_f", "usb_a_m", "usb_c_f", "usb_c_m", "micro_b_f", "micro_b_m", "jack35_f",
                "jack35_m", "dc_f", "dc_m", "screw", "jst_ph_f", "jst_xh_f"
            ]
        );
        for kind in ConnectorKind::ALL {
            let read: ConnectorKind = serde_json::from_str(&format!("\"{}\"", kind.name())).expect("読める");
            assert_eq!(read, kind);
        }
        let grouped: Vec<&str> = ConnectorKind::ALL.iter().filter(|k| k.is_grouped()).map(|k| k.name()).collect();
        assert_eq!(grouped, ["screw", "jst_ph_f", "jst_xh_f"]);
        assert_eq!(ConnectorKind::JstPhF.series(), Some("PH"));
        assert_eq!(ConnectorKind::JstXhF.series(), Some("XH"));
        assert_eq!(ConnectorKind::Screw.series(), None);
    }

    /// Consecutive screw or JST pins with the same kind and the same name (or both unnamed) form one connector; a different name, a different kind or a pin in between starts a new one, and USB-like kinds are always one pin each.
    /// 同じ種類で同じ名前 (どちらも名前なしを含む) の端子台・JST のピンが続けば1つのコネクタになり、名前・種類が違うか間に別のピンがあれば別のコネクタになる。USB などはいつも1本で1つ。
    #[test]
    fn consecutive_pins_with_the_same_kind_and_name_form_one_connector() {
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"screw","connector_ref":"TB1"},{"no":"2","connector":"screw","connector_ref":" TB1 "}]}"#),
            [(0, 2)]
        );
        assert_eq!(spans(r#"{"left":[{"no":"1","connector":"screw"},{"no":"2","connector":"screw"}]}"#), [(0, 2)]);
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"screw","connector_ref":"TB1"},{"no":"2","connector":"screw","connector_ref":"TB2"}]}"#),
            [(0, 1), (1, 2)]
        );
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"jst_ph_f"},{"no":"2","connector":"jst_xh_f"}]}"#),
            [(0, 1), (1, 2)]
        );
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"screw"},{"no":"2"},{"no":"3","connector":"screw"}]}"#),
            [(0, 1), (2, 3)]
        );
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"usb_a_f"},{"no":"2","connector":"usb_a_f"}]}"#),
            [(0, 1), (1, 2)]
        );
    }

    /// A JST connector is labelled with its name and series ("J3 PH", or "XH" alone), a screw terminal block with its name, and an unnamed screw block or a USB socket gets no label.
    /// JST のコネクタには名前とシリーズ (「J3 PH」、名前が無ければ「XH」だけ)、端子台には名前を書き、名前の無い端子台と USB の口には何も書かない。
    #[test]
    fn group_labels_name_the_connector_and_its_series() {
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"jst_ph_f","connector_ref":"J3"}]}"#).as_deref(), Some("J3 PH"));
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"jst_xh_f"}]}"#).as_deref(), Some("XH"));
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"screw","connector_ref":"TB1"}]}"#).as_deref(), Some("TB1"));
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"screw"}]}"#), None);
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"usb_a_f"}]}"#), None);
    }

    /// 数を短い文字にする (-0 は 0 にそろえる)。
    fn n(v: f64) -> String {
        format!("{}", v + 0.0)
    }

    /// プリミティブを1行の文字にする (期待値を手で書けるように)。
    fn show(p: &Primitive) -> String {
        match p {
            Primitive::Line { pts } => {
                let pts: Vec<String> = pts.iter().map(|q| format!("{},{}", n(q.x), n(q.y))).collect();
                format!("line {}", pts.join(" "))
            }
            Primitive::Rect { p1, p2, filled } => {
                format!("rect {},{} {},{}{}", n(p1.x), n(p1.y), n(p2.x), n(p2.y), if *filled { " filled" } else { "" })
            }
            Primitive::Circle { center, r, filled } => {
                format!("circle {},{} r{}{}", n(center.x), n(center.y), n(*r), if *filled { " filled" } else { "" })
            }
            Primitive::Arc { center, r, start_deg, end_deg } => {
                format!("arc {},{} r{} {}..{}", n(center.x), n(center.y), n(*r), n(*start_deg), n(*end_deg))
            }
            Primitive::Text { at, text, .. } => format!("text {text} {},{}", n(at.x), n(at.y)),
        }
    }

    /// 左辺の、E = (0, 0) の1本 (まとめる種類は E と (0, 5) の2本) の絵。
    fn left_picture(kind: ConnectorKind) -> Vec<String> {
        let edges = if kind.is_grouped() {
            vec![Point::new(0.0, 0.0), Point::new(0.0, 5.0)]
        } else {
            vec![Point::new(0.0, 0.0)]
        };
        connector_primitives(kind, PinDir::Left, &edges).iter().map(show).collect()
    }

    /// On the left side every connector picture is exactly the drawing of the spec: sockets are an open body across the edge, plugs a housing inside with a black insert outside, and screw and JST bodies span their pins.
    /// 左辺では、どのコネクタの絵も仕様の図そのものになる: メスは辺にまたがる白い口、オスは内側の枠と外へ出る黒い差し込み、端子台と JST の本体はピンをまたいで1つ。
    #[test]
    fn every_picture_matches_the_spec_on_the_left_side() {
        use ConnectorKind::*;
        let open = "rect -2.5,-2 1.5,2";
        let housing = "rect 0,-2 1.5,2";
        assert_eq!(left_picture(UsbAF), [open, "rect -1.8,0.1 0.8,0.9 filled"]);
        assert_eq!(left_picture(UsbAM), [housing, "rect -2.5,-1.2 0,1.2 filled"]);
        assert_eq!(
            left_picture(UsbCF),
            [open, "line 0.2,0.8 -1.2,0.8", "arc -1.2,0 r0.8 90..270", "line -1.2,-0.8 0.2,-0.8", "arc 0.2,0 r0.8 270..450", "line 0,0 -1,0"]
        );
        assert_eq!(left_picture(UsbCM), [housing, "rect -1.7,-0.8 0,0.8 filled", "circle -1.7,0 r0.8 filled"]);
        assert_eq!(
            left_picture(MicroBF),
            ["line 1.5,2 0,2 -2.5,1.4 -2.5,-1.4 0,-2 1.5,-2 1.5,2", "line 0.6,0 -1.6,0"]
        );
        assert_eq!(left_picture(MicroBM), [housing, "rect -2.5,-0.6 0,0.6 filled"]);
        assert_eq!(left_picture(Jack35F), [open, "circle -0.5,0 r1", "circle -0.5,0 r0.35 filled"]);
        assert_eq!(left_picture(Jack35M), [housing, "rect -0.8,-1 0,1 filled", "rect -2.5,-0.4 -0.8,0.4 filled"]);
        assert_eq!(left_picture(DcF), [open, "circle -0.5,0 r1.5", "circle -0.5,0 r0.6 filled"]);
        assert_eq!(left_picture(DcM), [housing, "rect -2.5,0.5 0,1.4 filled", "rect -2.5,-1.4 0,-0.5 filled"]);
        assert_eq!(
            left_picture(Screw),
            ["rect -2.5,-2 1.5,7", "circle -0.5,0 r1.2", "line -1.3,0.8 0.3,-0.8", "circle -0.5,5 r1.2", "line -1.3,5.8 0.3,4.2"]
        );
        let jst = ["rect -2.5,-2 1.5,7", "line -1.5,7 -1.5,-2", "rect -0.6,-0.6 0.6,0.6", "rect -0.6,4.4 0.6,5.6"];
        assert_eq!(left_picture(JstPhF), jst);
        assert_eq!(left_picture(JstXhF), jst, "PH と XH は名前だけが違う");
    }

    /// The picture frame of each side is the right-side frame turned clockwise: a local point lands where the side faces, and an arc keeps its clockwise sweep.
    /// 辺ごとの絵の座標は右辺の座標を時計回りに回したもので、ローカルの点はその辺の向きに写り、弧は時計回りの振れ幅を保つ。
    #[test]
    fn side_frames_are_turns_of_the_right_side_frame() {
        let e = Point::new(10.0, 20.0);
        let cases = [
            (PinDir::Right, (11.0, 20.5), (270.0, 450.0)),
            (PinDir::Down, (9.5, 21.0), (0.0, 180.0)),
            (PinDir::Left, (9.0, 19.5), (90.0, 270.0)),
            (PinDir::Up, (10.5, 19.0), (180.0, 360.0)),
        ];
        for (dir, point, angles) in cases {
            let f = frame(dir, e);
            let at = f.at(1.0, 0.5);
            assert_eq!((at.x, at.y), point, "{dir:?}");
            let Primitive::Arc { start_deg, end_deg, .. } = f.arc((0.0, 0.0), 1.0, -90.0, 90.0) else { unreachable!() };
            assert_eq!((start_deg, end_deg), angles, "{dir:?}");
        }
    }

    /// No picture ever leaves its envelope (1.5 mm inside to 2.5 mm outside the edge, 2 mm beyond its outer pins along the edge), on any side.
    /// どの辺でも、絵は決まった枠 (辺の内側1.5mmから外側2.5mm、辺に沿って両端のピンから2mm) の外へ出ない。
    #[test]
    fn every_picture_stays_inside_its_envelope() {
        for dir in [PinDir::Left, PinDir::Right, PinDir::Up, PinDir::Down] {
            for kind in ConnectorKind::ALL {
                let e = Point::new(30.0, 40.0);
                let f = frame(dir, e);
                let edges = if kind.is_grouped() { vec![e, f.at(0.0, 5.0)] } else { vec![e] };
                let v1 = if kind.is_grouped() { 5.0 } else { 0.0 };
                let (a, b) = (f.at(-CONNECTOR_IN_MM, -2.0), f.at(CONNECTOR_OUT_MM, v1 + 2.0));
                let (lo, hi) = (Point::new(a.x.min(b.x), a.y.min(b.y)), Point::new(a.x.max(b.x), a.y.max(b.y)));
                for prim in connector_primitives(kind, dir, &edges) {
                    let (a, b) = primitive_extent(&prim);
                    let inside = a.x >= lo.x - 1e-9 && a.y >= lo.y - 1e-9 && b.x <= hi.x + 1e-9 && b.y <= hi.y + 1e-9;
                    assert!(inside, "{} {dir:?}: {} が枠の外", kind.name(), show(&prim));
                }
            }
        }
    }

    /// A USB-C socket's opening stays a closed rounded slot on every side and at every rotation and mirror of the block: each arc ends where a straight edge of the opening ends and bulges outward.
    /// USB-C メスの口は、どの辺でも、ブロックをどう回転・ミラーしても閉じた角丸の穴のまま: 弧の端は口の直線の端に合い、弧は外へふくらむ。
    #[test]
    fn the_usb_c_opening_stays_closed_at_every_rotation_and_mirror() {
        use crate::netlist::{rotate_arc_angles, rotate_local};
        for dir in [PinDir::Left, PinDir::Right, PinDir::Up, PinDir::Down] {
            let e = Point::new(7.5, 2.5);
            let prims = connector_primitives(ConnectorKind::UsbCF, dir, &[e]);
            let slot_centre = frame(dir, e).at(0.5, 0.0);
            for rotation in [0u16, 90, 180, 270] {
                for mirror in [false, true] {
                    let line_ends: Vec<Point> = prims
                        .iter()
                        .filter_map(|p| match p {
                            Primitive::Line { pts } => Some(pts.clone()),
                            _ => None,
                        })
                        .flatten()
                        .map(|q| rotate_local(q, rotation, mirror))
                        .collect();
                    let centre = rotate_local(slot_centre, rotation, mirror);
                    for p in &prims {
                        let Primitive::Arc { center, r, start_deg, end_deg } = p else { continue };
                        let c = rotate_local(*center, rotation, mirror);
                        let (a0, a1) = rotate_arc_angles(*start_deg, *end_deg, rotation, mirror);
                        let on_arc = |a: f64| Point::new(c.x + r * a.to_radians().cos(), c.y + r * a.to_radians().sin());
                        for end in [on_arc(a0), on_arc(a1)] {
                            assert!(
                                line_ends.iter().any(|q| (q.x - end.x).abs() < 1e-9 && (q.y - end.y).abs() < 1e-9),
                                "{dir:?} {rotation} {mirror}: 弧の端 {end:?} が口の直線の端に合わない"
                            );
                        }
                        let mid = on_arc((a0 + a1) / 2.0);
                        let bulge = ((mid.x - centre.x).powi(2) + (mid.y - centre.y).powi(2)).sqrt();
                        assert!(bulge > 1.0, "{dir:?} {rotation} {mirror}: 弧が内側へふくらむ ({bulge})");
                    }
                }
            }
        }
    }
}
