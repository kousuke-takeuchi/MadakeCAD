//! 機能シートの下書き: 通り道の線 (トラック)。
//!
//! 隣り合う2つの列のあいだの縦の帯 (通り道) を通るネットのうち、端が全て列kの右辺か列k+1の左辺にあるものを、
//! 格子の経路探索より先に決まった手順で引く。端が同じ高さの2つだけなら1本の横線、それ以外は通り道の中の縦の幹
//! (トラック) 1本と、各端から幹への横の枝。トラックの割り当てと、引いた線の検査はここで行う。
//! 部品の座標は自分で計算せず、配置の段 (`draft.rs`) が決めた端の位置を受け取る。
use super::*;
use crate::route::RouteGrid;
use std::cell::RefCell;

/// 通り道の外へ伸びる枝の、抽象的な座標の端 (どの実際の座標よりも遠い)。
const FAR: f64 = 1.0e4;
/// 隣り合うトラックの間隔 (mm)。
pub(crate) const TRACK_PITCH_MM: f64 = 2.5;
/// 列の張り出しからトラックまでの最小の間隔 (mm)。
pub(crate) const TRACK_MARGIN_MM: f64 = 5.0;
/// 同じトラックを分け合うネットの幹の範囲に足す余白 (mm) (両側)。
pub(crate) const SPAN_MARGIN_MM: f64 = 5.0;
/// 端の短い線に触れる区間を検査から外す長さ (mm): 枝は端から出るので、端で触れるのは当たり前。
const TERMINAL_SKIP_MM: f64 = 1.25;
const EPS: f64 = 1e-6;
/// 札を幹の端につけるとき、幹を最も上(下)の端から伸ばす長さの上限 (mm)。5mm、だめなら10mm。
pub(crate) const TAG_EXTENSION_MM: f64 = 10.0;

/// 通り道で見た端1つ: 高さと、通り道の左側 (列kの右辺) にあるか。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ChanTerm {
    pub y: f64,
    pub left: bool,
}

/// トラックを割り当てるネット1本。`key` は [`DraftModel::nets`] の添字。
#[derive(Debug, Clone)]
pub(crate) struct ChanNet {
    pub key: usize,
    pub name: String,
    pub terms: Vec<ChanTerm>,
    /// 札を幹の端につけるネット: 端が同じ高さの2つだけでも、札のために幹が要る (トラックを使う)。
    pub tagged: bool,
}

/// 割り当てた結果1つ: `track` が `None` なら、トラックを使わない1本の横線。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placed {
    pub key: usize,
    pub track: Option<usize>,
}

/// 1つの通り道の割り当て。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Assignment {
    /// 必要なトラックの数。
    pub tracks: usize,
    /// 割り当てたネット (処理した順 = ネット名の自然順)。
    pub placed: Vec<Placed>,
    /// どの位置でも他の線とぶつかり、割り当てられなかったネット (格子の経路探索へ回す)。
    pub unplaced: Vec<usize>,
}

/// 線分どうしの関係。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rel {
    /// 離れている。
    Apart,
    /// 両方の途中で直角に交わる (許される)。
    Cross,
    /// 触れる・重なる・端や頂点で会う (別のネットなら短絡)。
    Short,
}

fn bounds(s: (Point, Point)) -> (f64, f64, f64, f64) {
    (s.0.x.min(s.1.x), s.0.x.max(s.1.x), s.0.y.min(s.1.y), s.0.y.max(s.1.y))
}

/// 縦か横の2つの線分の関係。直角に、どちらの端でもない点で交わるときだけ `Cross`。
pub(crate) fn seg_rel(a: (Point, Point), b: (Point, Point)) -> Rel {
    let ((ax0, ax1, ay0, ay1), (bx0, bx1, by0, by1)) = (bounds(a), bounds(b));
    if ax1 < bx0 - EPS || bx1 < ax0 - EPS || ay1 < by0 - EPS || by1 < ay0 - EPS {
        return Rel::Apart;
    }
    let (a_h, a_v) = ((ay1 - ay0).abs() < EPS, (ax1 - ax0).abs() < EPS);
    let (b_h, b_v) = ((by1 - by0).abs() < EPS, (bx1 - bx0).abs() < EPS);
    let perpendicular = (a_h && !a_v && b_v && !b_h) || (a_v && !a_h && b_h && !b_v);
    if !perpendicular {
        return Rel::Short;
    }
    let (h, v) = if a_h { ((ax0, ax1, ay0), (bx0, by0, by1)) } else { ((bx0, bx1, by0), (ax0, ay0, ay1)) };
    let (hx0, hx1, hy) = h;
    let (vx, vy0, vy1) = v;
    let at_end = (vx - hx0).abs() < EPS || (vx - hx1).abs() < EPS || (hy - vy0).abs() < EPS || (hy - vy1).abs() < EPS;
    if at_end { Rel::Short } else { Rel::Cross }
}

/// 点が線分の上にあるか。
fn point_on(p: Point, s: (Point, Point)) -> bool {
    let (x0, x1, y0, y1) = bounds(s);
    p.x >= x0 - EPS && p.x <= x1 + EPS && p.y >= y0 - EPS && p.y <= y1 + EPS
}

/// 端が同じ高さの両側に1つずつだけ: トラックを使わない1本の横線。
fn is_straight(terms: &[ChanTerm]) -> bool {
    terms.len() == 2 && terms[0].left != terms[1].left && (terms[0].y - terms[1].y).abs() < EPS
}

fn span_of(terms: &[ChanTerm]) -> (f64, f64) {
    let lo = terms.iter().map(|t| t.y).fold(f64::MAX, f64::min);
    let hi = terms.iter().map(|t| t.y).fold(f64::MIN, f64::max);
    (lo, hi)
}

/// ネットの線分 (抽象的な座標: 左の端は -FAR、右の端は +FAR から、トラック `x` まで)。
fn abstract_segments(terms: &[ChanTerm], x: f64, straight: bool) -> Vec<(Point, Point)> {
    if straight {
        let y = terms[0].y;
        return vec![(Point::new(-FAR, y), Point::new(FAR, y))];
    }
    let (lo, hi) = span_of(terms);
    let mut out = vec![(Point::new(x, lo), Point::new(x, hi))];
    for t in terms {
        out.push(if t.left { (Point::new(-FAR, t.y), Point::new(x, t.y)) } else { (Point::new(x, t.y), Point::new(FAR, t.y)) });
    }
    out
}

/// 1つの通り道のネットにトラックを割り当てる。
///
/// ネット名の自然順に1本ずつ、全ての位置 (今あるトラックに相乗りする位置と、どのトラックのあいだにも新しいトラックを
/// 開く位置) のうち、すでに置いた線との交差がいちばん少ない位置へ置く。同数なら、新しいトラックを開かず相乗りする位置、それも同じなら左の位置。触れる・重なる位置 (短絡) は選べない。
/// 相乗りできるのは、幹の範囲を両側に5mm広げたものが重ならないときだけ。どの位置も選べないネットは `unplaced`。
pub(crate) fn assign_tracks(nets: &[ChanNet]) -> Assignment {
    assign_tracks_with(nets, &[])
}

/// [`assign_tracks`] の、この通り道を横いっぱいに通り抜ける他の通り道のネットの枝 (高さ `fixed_h` の横線) も、動かない線として
/// 数える版: 幹がその枝と交差する数に入り、枝の高さが幹の端や他の枝と同じになる位置 (触れる・重なる位置) は選べない。
pub(crate) fn assign_tracks_with(nets: &[ChanNet], fixed_h: &[f64]) -> Assignment {
    struct Track {
        members: Vec<usize>,
    }
    struct Done<'a> {
        key: usize,
        straight: bool,
        span: (f64, f64),
        terms: &'a [ChanTerm],
    }
    let mut order: Vec<&ChanNet> = nets.iter().collect();
    order.sort_by(|a, b| natural_cmp(&a.name, &b.name).then(a.key.cmp(&b.key)));

    let mut tracks: Vec<Track> = Vec::new();
    let mut done: Vec<Done> = Vec::new();
    let mut out = Assignment::default();
    for net in order {
        let straight = !net.tagged && is_straight(&net.terms);
        let span = span_of(&net.terms);
        // 札を幹の端につけるネットは、幹が上下へ最大 TAG_EXTENSION_MM 伸びるので、相乗りの判定にその分を足す
        let span = if net.tagged { (span.0 - TAG_EXTENSION_MM, span.1 + TAG_EXTENSION_MM) } else { span };
        // 抽象的な位置は整数の並び: i番目のトラックは 2i+1、i番目のトラックの左に開く新しいトラックは 2i。
        // 新しいトラックを開くたびに付け直すので、何本開いても位置が近づきすぎない (半分ずつにすると丸めで重なる)
        let mut segs_of_done: Vec<Vec<(Point, Point)>> = done
            .iter()
            .map(|d| {
                let x = if d.straight { 0.0 } else { (2 * tracks.iter().position(|t| t.members.contains(&d.key)).expect("a placed net has a track") + 1) as f64 };
                abstract_segments(d.terms, x, d.straight)
            })
            .collect();
        segs_of_done.extend(fixed_h.iter().map(|&y| vec![(Point::new(-FAR, y), Point::new(FAR, y))]));
        // 候補の位置 (左から右)。(抽象的な位置, 相乗りする既存のトラックの添字, 新しいトラックを差し込む添字)
        let mut candidates: Vec<(f64, Option<usize>, usize)> = Vec::new();
        for i in 0..=tracks.len() {
            candidates.push(((2 * i) as f64, None, i));
            if i < tracks.len() {
                candidates.push(((2 * i + 1) as f64, Some(i), i));
            }
        }
        if straight {
            candidates = vec![(0.0, None, 0)];
        }
        let mut best: Option<(usize, bool, usize)> = None; // (交差, 新しいトラックを開くか, 候補の添字)
        for (ci, &(x, share, _)) in candidates.iter().enumerate() {
            if let Some(i) = share {
                let clash = tracks[i].members.iter().any(|m| {
                    let other = done.iter().find(|d| d.key == *m).expect("a member is done");
                    other.span.0 - SPAN_MARGIN_MM < span.1 + SPAN_MARGIN_MM && span.0 - SPAN_MARGIN_MM < other.span.1 + SPAN_MARGIN_MM
                });
                if clash {
                    continue;
                }
            }
            let mine = abstract_segments(&net.terms, x, straight);
            let (mut crossings, mut short) = (0usize, false);
            'pairs: for d in &segs_of_done {
                for a in &mine {
                    for b in d {
                        match seg_rel(*a, *b) {
                            Rel::Short => {
                                short = true;
                                break 'pairs;
                            }
                            Rel::Cross => crossings += 1,
                            Rel::Apart => {}
                        }
                    }
                }
            }
            if short {
                continue;
            }
            // 交差が同数なら、新しいトラックを開かない (相乗りする) 方を先に、それも同じなら左の位置
            let key = (crossings, share.is_none(), ci);
            if best.map_or(true, |b| key < b) {
                best = Some(key);
            }
        }
        let Some((_, _, ci)) = best else {
            out.unplaced.push(net.key);
            continue;
        };
        let (_, share, at) = candidates[ci];
        if !straight {
            match share {
                Some(i) => tracks[i].members.push(net.key),
                None => tracks.insert(at, Track { members: vec![net.key] }),
            }
        }
        done.push(Done { key: net.key, straight, span, terms: &net.terms });
    }
    out.tracks = tracks.len();
    for d in &done {
        let track = if d.straight { None } else { tracks.iter().position(|t| t.members.contains(&d.key)) };
        out.placed.push(Placed { key: d.key, track });
    }
    out
}

/// 通り道の実際の端1つ。
#[derive(Debug, Clone, Copy)]
pub(crate) struct ChanTermAt {
    pub at: Point,
    pub left: bool,
}

/// 通り道の1ネットぶんの配線: 線の点列と、3本以上の端が会う点。
#[derive(Debug, Clone, Default)]
pub(crate) struct ChanRoute {
    pub wires: Vec<Vec<Point>>,
    pub junctions: Vec<Point>,
    /// 札を幹の端へ動かしたネットなら、その札 (基準点は幹の端) と旗の外形 (後から書かれる行き先の分まで)。
    pub tag: Option<(NetLabel, Rect)>,
}

impl ChanRoute {
    /// 動かない点として経路探索に渡す点: 線の端 (端の位置・幹の端・枝の付け根) と接点。他のネットの経路が、
    /// 端や接点を直角に横切るように通り抜けて短絡しないようにする。
    pub(crate) fn spots(&self) -> Vec<Point> {
        let mut out: Vec<Point> = self.wires.iter().flat_map(|w| [w[0], w[w.len() - 1]]).chain(self.junctions.iter().copied()).collect();
        out.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
        out.dedup_by(|a, b| a.distance_to(b) < EPS);
        out
    }

    pub(crate) fn segments(&self) -> Vec<(Point, Point)> {
        self.wires.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect()
    }
}

/// 端と、トラックの位置 `x` (横線なら `None`) から、実際の座標の配線を作る。端から幹まで横に枝を出し、
/// 幹は最も上の端から最も下の端まで。
pub(crate) fn route_geometry(terms: &[ChanTermAt], x: Option<f64>) -> ChanRoute {
    route_geometry_ext(terms, x, None)
}

/// [`route_geometry`] の、幹を札のために伸ばせる版: `ext` = (上へ伸ばすか, 伸ばす長さ mm)。伸ばした幹の端が札の位置で、
/// 幹の端の高さ (最も上か最も下の端の高さ) では幹が通り抜けるので、枝が1本でもあれば3本が会って接点になる。
pub(crate) fn route_geometry_ext(terms: &[ChanTermAt], x: Option<f64>, ext: Option<(bool, f64)>) -> ChanRoute {
    let Some(x) = x else {
        let mut ends: Vec<Point> = terms.iter().map(|t| t.at).collect();
        ends.sort_by(|a, b| a.x.total_cmp(&b.x));
        return ChanRoute { wires: vec![ends], junctions: Vec::new(), tag: None };
    };
    let lo = terms.iter().map(|t| t.at.y).fold(f64::MAX, f64::min);
    let hi = terms.iter().map(|t| t.at.y).fold(f64::MIN, f64::max);
    let (top, bottom) = match ext {
        Some((true, e)) => (lo - e, hi),
        Some((false, e)) => (lo, hi + e),
        None => (lo, hi),
    };
    let mut wires = vec![vec![Point::new(x, top), Point::new(x, bottom)]];
    for t in terms {
        wires.push(vec![t.at, Point::new(x, t.at.y)]);
    }
    let mut ys: Vec<f64> = terms.iter().map(|t| t.at.y).collect();
    ys.sort_by(|a, b| a.total_cmp(b));
    ys.dedup_by(|a, b| (*a - *b).abs() < EPS);
    let mut junctions = Vec::new();
    for y in ys {
        let drops = terms.iter().filter(|t| (t.at.y - y).abs() < EPS).count();
        let trunk = if (y - top).abs() < EPS || (y - bottom).abs() < EPS { 1 } else { 2 };
        if drops + trunk >= 3 {
            junctions.push(Point::new(x, y));
        }
    }
    ChanRoute { wires, junctions, tag: None }
}

/// 配線が避けるもの。
pub(crate) struct Obstacles<'a> {
    /// 動かない線 (箱の短い線・棒・印の引き出し): 端や頂点で触れる・重なるのは短絡。途中を直角に横切るのは許す。
    pub fixed_lines: &'a [Vec<Point>],
    /// 動かない接点の位置: 線が通ってはいけない。
    pub fixed_spots: &'a [Point],
    /// 札の枝 (札を下げた端から札の位置まで): どのネットのものも、触れる・横切るのも許さない。
    pub branches: &'a [(Point, Point)],
    /// 線分が、箱・文字・札の旗・印・図枠・表題欄・ピンの位置を、経路探索と同じすき間を空けて避けているか。
    pub free: &'a dyn Fn(Point, Point) -> bool,
}

/// 端 (`terms`) で始まる線分の、端に近い短い区間を除いたもの。端で触れるのは自分の短い線なので検査しない。
fn without_terminal_ends(s: (Point, Point), terms: &[ChanTermAt]) -> (Point, Point) {
    let (mut a, mut b) = s;
    let len = a.distance_to(&b);
    if len < 2.0 * TERMINAL_SKIP_MM + EPS {
        return (a, b);
    }
    let (ux, uy) = ((b.x - a.x) / len, (b.y - a.y) / len);
    if terms.iter().any(|t| t.at.distance_to(&a) < EPS) {
        a = Point::new(a.x + ux * TERMINAL_SKIP_MM, a.y + uy * TERMINAL_SKIP_MM);
    }
    if terms.iter().any(|t| t.at.distance_to(&b) < EPS) {
        b = Point::new(b.x - ux * TERMINAL_SKIP_MM, b.y - uy * TERMINAL_SKIP_MM);
    }
    (a, b)
}

/// 配線が使えない理由: 最初に当たったもの。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Block {
    /// 通れない場所 (箱・文字・札の旗・印・図枠・表題欄・ピン) を通る線分。
    Free(Point, Point),
    /// 札の枝に触れる線分。
    Branch(Point, Point),
    /// 動かない線 (`Obstacles::fixed_lines` の添字) の端や頂点に触れる線分。
    Line(Point, Point, usize),
    /// 動かない接点を通る線分。
    Spot(Point, Point),
    /// すでに使うと決めた配線 (`accepted` の添字) に触れる・重なる線分。重なりなら `true`。
    Accepted(Point, Point, usize, bool),
}

/// ネットの配線が使えないなら、最初に当たったもの (使えるなら `None`)。
pub(crate) fn first_block(route: &ChanRoute, terms: &[ChanTermAt], obs: &Obstacles, accepted: &[&ChanRoute]) -> Option<Block> {
    for seg in route.segments() {
        if !(obs.free)(seg.0, seg.1) {
            return Some(Block::Free(seg.0, seg.1));
        }
        let probe = without_terminal_ends(seg, terms);
        if obs.branches.iter().any(|b| seg_rel(seg, *b) != Rel::Apart) {
            return Some(Block::Branch(seg.0, seg.1));
        }
        for (i, line) in obs.fixed_lines.iter().enumerate() {
            if line.windows(2).any(|f| seg_rel(probe, (f[0], f[1])) == Rel::Short) {
                return Some(Block::Line(seg.0, seg.1, i));
            }
        }
        if obs.fixed_spots.iter().any(|&p| point_on(p, probe)) {
            return Some(Block::Spot(seg.0, seg.1));
        }
        for (i, other) in accepted.iter().enumerate() {
            if let Some(o) = other.segments().iter().find(|o| seg_rel(seg, **o) == Rel::Short) {
                let parallel = ((seg.0.y - seg.1.y).abs() < EPS) == ((o.0.y - o.1.y).abs() < EPS);
                return Some(Block::Accepted(seg.0, seg.1, i, parallel));
            }
        }
    }
    None
}

/// ネットの配線が使えないなら、その理由 (使えるなら `None`)。
pub(crate) fn invalid_reason(route: &ChanRoute, terms: &[ChanTermAt], obs: &Obstacles, accepted: &[&ChanRoute]) -> Option<String> {
    first_block(route, terms, obs, accepted).map(|b| format!("{b:?}"))
}

/// ネットの配線が使えるか: 避けるものに触れず、すでに使うと決めた別のネットの配線と短絡しない。
pub(crate) fn net_is_valid(route: &ChanRoute, terms: &[ChanTermAt], obs: &Obstacles, accepted: &[&ChanRoute]) -> bool {
    first_block(route, terms, obs, accepted).is_none()
}

/// 1つの通り道の、レイアウトが決めたもの。
#[derive(Debug, Clone, Default)]
pub(crate) struct ChannelLayout {
    /// 通り道の左右の端 (列kの右の張り出しと、列k+1の左の張り出し)。
    pub left: f64,
    pub right: f64,
    pub tracks: usize,
    pub placed: Vec<Placed>,
    /// この通り道に選ばれたが、どの位置も他の線とぶつかり置けなかったネット。
    pub unplaced: Vec<usize>,
}

impl ChannelLayout {
    /// i番目のトラックのx。通り道の中ほどに、2.5mm間隔で並べる (両側の張り出しから5mm以上)。
    pub(crate) fn track_x(&self, i: usize) -> f64 {
        let used = TRACK_PITCH_MM * self.tracks.saturating_sub(1) as f64;
        let slack = (self.right - self.left) - 2.0 * TRACK_MARGIN_MM - used;
        self.left + TRACK_MARGIN_MM + grid_down(slack / 2.0).max(0.0) + TRACK_PITCH_MM * i as f64
    }
}

/// 通り道を通れるネット1本ぶんの1つの通り道の候補: どの通り道か、端 (位置と、幹から見た側)。
#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub channel: usize,
    pub net: usize,
    pub name: String,
    pub terms: Vec<ChanTermAt>,
    /// 札を幹の端につけられるネットか (出ていくネットで組が2つ以上)。
    pub tagged: bool,
    /// 枝が横いっぱいに通り抜ける他の通り道: (通り道の添字, 枝の高さ)。
    pub through: Vec<(usize, f64)>,
}

/// 列の番号 → 機器のある列の並びの中の位置。
pub(crate) fn column_positions(model: &DraftModel) -> BTreeMap<usize, usize> {
    let mut c: Vec<usize> = model.units.iter().map(|u| u.column).collect();
    c.sort_unstable();
    c.dedup();
    c.into_iter().enumerate().map(|(k, c)| (c, k)).collect()
}

/// 通り道を通れるネットの候補 (ネット × 通り道)。端は2つ以上 (札の位置の端は数えない)。通り道cの候補になるのは、端が全て
/// 列c以前の機器の右辺か、列c+1以降の機器の左辺にあるとき。枝は端の高さを横に通り、間の通り道を横切り、間の列の機器の
/// すき間を通る (通れるかは配線の検査が決める)。札を下げた組の端は、札の枝を持たないものとして数える。
pub(crate) fn candidates(model: &DraftModel, terminals: &[Terminal]) -> Vec<Candidate> {
    let pos = column_positions(model);
    let ncols = pos.len();
    let mut by_net: BTreeMap<usize, Vec<&Terminal>> = BTreeMap::new();
    for t in terminals {
        by_net.entry(t.net).or_default().push(t);
    }
    let mut out = Vec::new();
    for (net, all) in by_net {
        let tagged = all.iter().any(|t| t.is_tag);
        let terms: Vec<&Terminal> = all.into_iter().filter(|t| !t.is_tag).collect();
        if terms.len() < 2 || ncols < 2 {
            continue;
        }
        // (列の位置, 右辺か)
        let mut ends = Vec::new();
        let mut ok = true;
        for t in &terms {
            let k = pos[&model.units[t.u].column];
            match t.dir {
                Dir::E => ends.push((k, true, t.at)),
                Dir::W => ends.push((k, false, t.at)),
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        for c in 0..ncols - 1 {
            // 右辺 (E) は列 <= c、左辺 (W) は列 >= c+1
            if !ends.iter().all(|&(k, east, _)| if east { k <= c } else { k >= c + 1 }) {
                continue;
            }
            let mut through = Vec::new();
            for &(k, east, at) in &ends {
                if east {
                    through.extend((k..c).map(|j| (j, at.y)));
                } else {
                    through.extend((c + 1..k).map(|j| (j, at.y)));
                }
            }
            out.push(Candidate {
                channel: c,
                net,
                name: model.nets[net].name.clone(),
                terms: ends.iter().map(|&(_, east, at)| ChanTermAt { at, left: east }).collect(),
                tagged,
                through,
            });
        }
    }
    out
}

/// ネットごとに通り道を1つ選ぶ (`excluded` の (ネット, 通り道) は選ばない): 枝の長さの合計 (端から、通り道の中ほどの幹までの
/// 横の距離の和。`trunk_x` が通り道の幹のxの見積り) が短い順、同じなら通り道の番号が小さい順で最初のもの。
pub(crate) fn choose_channels(cands: &[Candidate], excluded: &BTreeSet<(usize, usize)>, trunk_x: &dyn Fn(usize) -> f64) -> BTreeMap<usize, usize> {
    let mut best: BTreeMap<usize, (f64, usize)> = BTreeMap::new();
    for c in cands.iter().filter(|c| !excluded.contains(&(c.net, c.channel))) {
        let x = trunk_x(c.channel);
        let len: f64 = c.terms.iter().map(|t| (t.at.x - x).abs()).sum();
        let key = ((len * 1000.0).round(), c.channel);
        let better = best.get(&c.net).map_or(true, |&(l, ch)| (key.0, key.1) < (l, ch));
        if better {
            best.insert(c.net, key);
        }
    }
    best.into_iter().map(|(net, (_, ch))| (net, ch)).collect()
}

/// 選んだ通り道ごとに割り当てる (通り道の数だけの割り当て)。他の通り道を選んだネットの、この通り道を通り抜ける枝は動かない線として数える。
pub(crate) fn assign_all(cands: &[Candidate], chosen: &BTreeMap<usize, usize>, channels: usize) -> Vec<Assignment> {
    (0..channels)
        .map(|k| {
            let mine: Vec<&Candidate> = cands.iter().filter(|c| chosen.get(&c.net) == Some(&c.channel) && c.channel == k).collect();
            let nets: Vec<ChanNet> = mine
                .iter()
                .map(|c| ChanNet {
                    key: c.net,
                    name: c.name.clone(),
                    terms: c.terms.iter().map(|t| ChanTerm { y: t.at.y, left: t.left }).collect(),
                    tagged: c.tagged,
                })
                .collect();
            let mut fixed: Vec<f64> = cands
                .iter()
                .filter(|c| chosen.get(&c.net) == Some(&c.channel) && c.channel != k)
                .flat_map(|c| c.through.iter().filter(|(j, _)| *j == k).map(|(_, y)| *y))
                .collect();
            fixed.sort_by(|a, b| a.total_cmp(b));
            fixed.dedup_by(|a, b| (*a - *b).abs() < EPS);
            assign_tracks_with(&nets, &fixed)
        })
        .collect()
}

/// 旗 (の外形) が使えるか: 通れない場所 (箱・文字・図枠・表題欄・立入禁止) と、動かない線・接点・札の枝・すでに使うと決めた
/// 配線・動かした他の札に、すき間 `gap` 以内で近づかず、自分の枝にも触れず、通り道の中 (`lane`) に収まる。
struct FlagCheck<'a> {
    grid: &'a RouteGrid,
    gap: f64,
    lane: (f64, f64),
    /// 動かした札の旗と、そのネットの名前。
    others: &'a RefCell<Vec<(Rect, String)>>,
}

/// 配線が当たったものの名前を付けるための、シートの中身: 箱・文字・札の旗・印・図枠・表題欄の矩形 (名前つき。この順に調べる) と、
/// 動かない線の持ち主の名前。
pub(crate) struct Scene<'a> {
    pub named: &'a [(String, Rect)],
    pub line_owner: &'a dyn Fn(&[Point]) -> String,
}

impl Scene<'_> {
    /// 矩形 `r` (すき間 `gap` を空けて) に重なる最初のものの名前。
    pub(crate) fn name_rect(&self, r: &Rect, gap: f64) -> String {
        self.named
            .iter()
            .find(|(_, n)| n.min.x < r.max.x + gap && r.min.x < n.max.x + gap && n.min.y < r.max.y + gap && r.min.y < n.max.y + gap)
            .map_or_else(|| "a blocked grid point".to_string(), |(name, _)| name.clone())
    }
}

/// 札の位置1つを試した結果。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TagTry {
    pub position: String,
    /// 使えなかった理由 (何が邪魔か)。使えたら `None`。
    pub rejected: Option<String>,
}

/// 札を幹の端へ動かす試みの記録: 置いた位置 (`None` なら置けなかった) と、試した位置と理由。
#[derive(Debug, Clone, Default)]
pub(crate) struct TagLog {
    pub placed: Option<String>,
    pub tried: Vec<TagTry>,
}

/// 通り道の配線の結果。
#[derive(Debug, Clone, Default)]
pub(crate) struct ChannelReport {
    /// ネットの添字 → 配線。
    pub routes: BTreeMap<usize, ChanRoute>,
    /// 外す (ネットの添字, 通り道の添字) と、その理由。
    pub retry: Vec<(usize, usize, String)>,
    /// 札を幹へ動かそうとしたネットの記録。
    pub tags: BTreeMap<usize, TagLog>,
}

/// 線分の呼び名: 横なら枝 (`drop at y=…`)、縦なら幹 (`trunk at x=…`)。
fn seg_label(a: Point, b: Point) -> String {
    if (a.y - b.y).abs() < EPS { format!("drop at y={:.1}", a.y) } else { format!("trunk at x={:.1}", a.x) }
}

/// 位置の呼び名: `top+5 right` など。
fn position_name(top: bool, ext: f64, right: bool) -> String {
    format!("{}+{} {}", if top { "top" } else { "bottom" }, ext, if right { "right" } else { "left" })
}

/// 使えない理由の短い文: 何が邪魔か。`names` は受け入れた配線のネット名 (`accepted` と同じ順)。
fn block_text(block: &Block, scene: &Scene, obs: &Obstacles, names: &[&str], gap: f64) -> String {
    match block {
        Block::Free(a, b) => {
            let r = Rect::from_corners(*a, *b);
            format!("{} hits {}", seg_label(*a, *b), scene.name_rect(&r, gap))
        }
        Block::Branch(a, b) => format!("{} crosses the tag branch of another net", seg_label(*a, *b)),
        Block::Line(a, b, i) => format!("{} meets {}", seg_label(*a, *b), (scene.line_owner)(&obs.fixed_lines[*i])),
        Block::Spot(a, b) => format!("{} passes a junction", seg_label(*a, *b)),
        Block::Accepted(a, b, i, parallel) => {
            let n = names.get(*i).copied().unwrap_or("?");
            if *parallel {
                format!("{} is collinear with wire of net {n}", seg_label(*a, *b))
            } else {
                format!("{} touches wire of net {n}", seg_label(*a, *b))
            }
        }
    }
}

fn near_rect(r: &Rect, a: Point, b: Point, gap: f64) -> bool {
    a.x.max(b.x) > r.min.x - gap && a.x.min(b.x) < r.max.x + gap && a.y.max(b.y) > r.min.y - gap && a.y.min(b.y) < r.max.y + gap
}

/// 旗が使えないなら、最初に当たったものの名前 (使えるなら `None`)。
fn flag_block(flag: &Rect, route: &ChanRoute, obs: &Obstacles, accepted: &[(&str, &ChanRoute)], check: &FlagCheck, scene: &Scene) -> Option<String> {
    if flag.min.x < check.lane.0 - EPS || flag.max.x > check.lane.1 + EPS {
        return Some("column extent".to_string());
    }
    if !check.grid.rect_free(flag) {
        return Some(scene.name_rect(flag, check.gap));
    }
    let g = check.gap - EPS;
    let touches = |a: Point, b: Point| near_rect(flag, a, b, g);
    if let Some(l) = obs.fixed_lines.iter().find(|l| l.windows(2).any(|s| touches(s[0], s[1]))) {
        return Some((scene.line_owner)(l));
    }
    if obs.fixed_spots.iter().any(|&p| touches(p, p)) {
        return Some("a junction".to_string());
    }
    if obs.branches.iter().any(|b| touches(b.0, b.1)) {
        return Some("the tag branch of another net".to_string());
    }
    if let Some((n, _)) = accepted.iter().find(|(_, r)| r.segments().iter().any(|s| touches(s.0, s.1))) {
        return Some(format!("wire of net {n}"));
    }
    // 自分の枝には触れない (幹は、旗の平らな辺で基準点につながるので除く)
    if route.wires.iter().skip(1).any(|w| w.windows(2).any(|s| touches(s[0], s[1]))) {
        return Some("a drop of its own net".to_string());
    }
    check
        .others
        .borrow()
        .iter()
        .find(|(o, _)| o.min.x < flag.max.x + g && flag.min.x < o.max.x + g && o.min.y < flag.max.y + g && flag.min.y < o.max.y + g)
        .map(|(_, n)| format!("flag of net {n}"))
}

/// 札のネットラベル: 旗は幹の端から右 (`right`) か左へ伸び、先端は幹から遠い側を向く (`tag_flow` = `out`)。
pub(crate) fn trunk_tag(name: &str, at: Point, right: bool, dest: Option<&String>) -> NetLabel {
    let mut attrs = BTreeMap::new();
    attrs.insert(crate::net_tag::TAG_ATTR.to_string(), if right { "left" } else { "right" }.to_string());
    attrs.insert(crate::net_tag::FLOW_ATTR.to_string(), "out".to_string());
    if let Some(dest) = dest {
        attrs.insert(crate::net_tag::DEST_ATTR.to_string(), dest.clone());
    }
    NetLabel { id: uuid::Uuid::new_v4(), at, name: name.to_string(), rotation: 0, attrs }
}

/// 割り当てに従って全ての通り道のネットを引き、使える配線だけを返す (ネットの添字 → 配線)。
/// 避けるものに触れる・短絡するネット、札の置き場所が無いネットは含めない。`retry` にその (ネット, 通り道) と理由を入れる
/// (次の配置では別の通り道を選ぶ。他に通り道が無ければ、そのネットは通り道に置かず、格子の経路探索が引く)。
/// 通り道をまたぐ枝があるので、受け入れた配線は全ての通り道で共有して短絡を調べる。
pub(crate) fn route_channels(
    model: &DraftModel,
    layout: &DraftLayout,
    grid: &RouteGrid,
    fixed_lines: &[Vec<Point>],
    fixed_spots: &[Point],
    scene: &Scene,
) -> ChannelReport {
    let cands = candidates(model, &layout.terminals);
    let branches: Vec<(usize, Point, Point)> = layout.terminals.iter().filter_map(|t| t.tag_at.map(|tag| (t.net, t.at, tag))).collect();
    let moved: RefCell<Vec<(Rect, String)>> = RefCell::new(Vec::new());
    let gap = grid.opts.clearance_mm;
    let free = |a: Point, b: Point| {
        grid.segment_free(a, b) && !moved.borrow().iter().any(|(f, _)| near_rect(f, a, b, gap - EPS))
    };
    let mut report = ChannelReport::default();
    // 受け入れた配線 (ネットの添字, 配線)。通り道をまたぐ枝があるので、全ての通り道で共有して短絡を調べる
    let mut accepted: Vec<(usize, ChanRoute)> = Vec::new();
    // 札を幹の端へ動かすネット: (通り道, 候補)。他の全てのネットの配線を受け入れたあとで、札の位置を決める
    let mut pending: Vec<(usize, &Candidate)> = Vec::new();
    let others_of = |net: usize| -> Vec<(Point, Point)> { branches.iter().filter(|b| b.0 != net).map(|b| (b.1, b.2)).collect() };
    // 動かした旗が邪魔をしたときの名前も付ける
    let describe = |block: &Block, obs: &Obstacles, names: &[&str]| -> String {
        if let Block::Free(a, b) = block {
            if let Some((_, n)) = moved.borrow().iter().find(|(f, _)| near_rect(f, *a, *b, gap - EPS)) {
                return format!("{} hits flag of net {n}", seg_label(*a, *b));
            }
        }
        block_text(block, scene, obs, names, gap)
    };
    for (k, ch) in layout.channels.iter().enumerate() {
        // 置けなかったネットは、他の通り道の候補があるときだけ、その通り道を外す (他の線が外れれば、次の配置で置けるかもしれない)
        for &net in &ch.unplaced {
            if cands.iter().any(|c| c.net == net && c.channel != k && !layout.excluded.contains(&(net, c.channel))) {
                report.retry.push((net, k, "no free track".to_string()));
            }
        }
        for placed in &ch.placed {
            let Some(c) = cands.iter().find(|c| c.channel == k && c.net == placed.key) else { continue };
            let x = placed.track.map(|i| ch.track_x(i));
            let refs: Vec<&ChanRoute> = accepted.iter().map(|(_, r)| r).collect();
            let names: Vec<&str> = accepted.iter().map(|(n, _)| model.nets[*n].name.as_str()).collect();
            // 札の枝は、そのネット自身のものを除く (札を幹へ動かすなら枝は無くなる。動かせなければ格子の経路探索が引く)
            let others = others_of(c.net);
            let obstacles = Obstacles { fixed_lines, fixed_spots, branches: &others, free: &free };
            let route = route_geometry(&c.terms, x);
            if let Some(block) = first_block(&route, &c.terms, &obstacles, &refs) {
                report.retry.push((c.net, k, describe(&block, &obstacles, &names)));
                continue;
            }
            if c.tagged {
                pending.push((k, c));
            }
            accepted.push((c.net, route));
        }
    }
    // 札を幹の端へ: 上に5mm 右・左、下に5mm 右・左、次に10mm で同じ順。最初に全てが通る位置。他のネットの配線は先に決めてあるので、
    // 札は他のネットの配線を押しのけない。置き場所が無ければ、そのネットは通り道に置かない (札はピンの枝のまま、格子の経路探索が引く)
    for (k, c) in pending {
        let ch = &layout.channels[k];
        let Some(x) = ch.placed.iter().find(|p| p.key == c.net).and_then(|p| p.track).map(|i| ch.track_x(i)) else {
            accepted.retain(|(n, _)| *n != c.net);
            report.retry.push((c.net, k, "no trunk for the tag".to_string()));
            continue;
        };
        let refs: Vec<&ChanRoute> = accepted.iter().filter(|(n, _)| *n != c.net).map(|(_, r)| r).collect();
        let names: Vec<&str> = accepted.iter().filter(|(n, _)| *n != c.net).map(|(n, _)| model.nets[*n].name.as_str()).collect();
        let named: Vec<(&str, &ChanRoute)> = names.iter().copied().zip(refs.iter().copied()).collect();
        let others = others_of(c.net);
        let obstacles = Obstacles { fixed_lines, fixed_spots, branches: &others, free: &free };
        let check = FlagCheck { grid, gap, lane: (ch.left, ch.right), others: &moved };
        let dest = model.nets[c.net].dest.as_ref();
        let mut log = TagLog::default();
        let mut found: Option<ChanRoute> = None;
        'positions: for ext in [5.0, TAG_EXTENSION_MM] {
            for top in [true, false] {
                for right in [true, false] {
                    let position = position_name(top, ext, right);
                    let mut route = route_geometry_ext(&c.terms, Some(x), Some((top, ext)));
                    if let Some(block) = first_block(&route, &c.terms, &obstacles, &refs) {
                        log.tried.push(TagTry { position, rejected: Some(describe(&block, &obstacles, &names)) });
                        continue;
                    }
                    let at = if top { route.wires[0][0] } else { route.wires[0][1] };
                    let label = trunk_tag(&c.name, at, right, dest);
                    let Some(flag) = flag_rect(&label, layout.label_font, layout.sheet.xref_style, layout.reserve.text(&label.name)) else {
                        log.tried.push(TagTry { position, rejected: Some("no flag shape".to_string()) });
                        continue;
                    };
                    if let Some(why) = flag_block(&flag, &route, &obstacles, &named, &check, scene) {
                        log.tried.push(TagTry { position, rejected: Some(why) });
                        continue;
                    }
                    log.tried.push(TagTry { position: position.clone(), rejected: None });
                    log.placed = Some(position);
                    route.tag = Some((label, flag));
                    found = Some(route);
                    break 'positions;
                }
            }
        }
        report.tags.insert(c.net, log);
        accepted.retain(|(n, _)| *n != c.net);
        match found {
            Some(route) => {
                if let Some((_, flag)) = &route.tag {
                    moved.borrow_mut().push((*flag, c.name.clone()));
                }
                accepted.push((c.net, route));
            }
            None => report.retry.push((c.net, k, "no tag position is free".to_string())),
        }
    }
    for (net, route) in accepted {
        report.routes.insert(net, route);
    }
    report
}
