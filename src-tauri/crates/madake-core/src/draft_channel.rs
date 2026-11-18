//! 機能シートの下書き: 通り道の線 (トラック)。
//!
//! 隣り合う2つの列のあいだの縦の帯 (通り道) を通るネットのうち、端が全て列kの右辺か列k+1の左辺にあるものを、
//! 格子の経路探索より先に決まった手順で引く。端が同じ高さの2つだけなら1本の横線、それ以外は通り道の中の縦の幹
//! (トラック) 1本と、各端から幹への横の枝。トラックの割り当てと、引いた線の検査はここで行う。
//! 部品の座標は自分で計算せず、配置の段 (`draft.rs`) が決めた端の位置を受け取る。
use super::*;
use crate::route::RouteGrid;

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
fn abstract_segments(terms: &[ChanTerm], x: f64) -> Vec<(Point, Point)> {
    if is_straight(terms) {
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
    struct Track {
        pos: f64,
        members: Vec<usize>,
    }
    struct Done {
        key: usize,
        straight: bool,
        span: (f64, f64),
        segs: Vec<(Point, Point)>,
    }
    let mut order: Vec<&ChanNet> = nets.iter().collect();
    order.sort_by(|a, b| natural_cmp(&a.name, &b.name).then(a.key.cmp(&b.key)));

    let mut tracks: Vec<Track> = Vec::new();
    let mut done: Vec<Done> = Vec::new();
    let mut out = Assignment::default();
    for net in order {
        let straight = is_straight(&net.terms);
        let span = span_of(&net.terms);
        // 候補の位置 (左から右)。(トラックの位置, 相乗りする既存のトラックの添字)
        let mut candidates: Vec<(f64, Option<usize>)> = Vec::new();
        for i in 0..=tracks.len() {
            let pos = match (i.checked_sub(1).map(|j| tracks[j].pos), tracks.get(i).map(|t| t.pos)) {
                (None, None) => 0.0,
                (None, Some(next)) => next - 1.0,
                (Some(prev), None) => prev + 1.0,
                (Some(prev), Some(next)) => (prev + next) / 2.0,
            };
            candidates.push((pos, None));
            if i < tracks.len() {
                candidates.push((tracks[i].pos, Some(i)));
            }
        }
        if straight {
            candidates = vec![(0.0, None)];
        }
        let mut best: Option<(usize, bool, usize)> = None; // (交差, 新しいトラックを開くか, 候補の添字)
        for (ci, &(x, share)) in candidates.iter().enumerate() {
            if let Some(i) = share {
                let clash = tracks[i].members.iter().any(|m| {
                    let other = done.iter().find(|d| d.key == *m).expect("a member is done");
                    other.span.0 - SPAN_MARGIN_MM < span.1 + SPAN_MARGIN_MM && span.0 - SPAN_MARGIN_MM < other.span.1 + SPAN_MARGIN_MM
                });
                if clash {
                    continue;
                }
            }
            let mine = abstract_segments(&net.terms, x);
            let (mut crossings, mut short) = (0usize, false);
            'pairs: for d in &done {
                for a in &mine {
                    for b in &d.segs {
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
        let (x, share) = candidates[ci];
        if !straight {
            match share {
                Some(i) => tracks[i].members.push(net.key),
                None => {
                    let at = tracks.iter().position(|t| t.pos > x).unwrap_or(tracks.len());
                    tracks.insert(at, Track { pos: x, members: vec![net.key] });
                }
            }
        }
        done.push(Done { key: net.key, straight, span, segs: abstract_segments(&net.terms, x) });
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
    let Some(x) = x else {
        let mut ends: Vec<Point> = terms.iter().map(|t| t.at).collect();
        ends.sort_by(|a, b| a.x.total_cmp(&b.x));
        return ChanRoute { wires: vec![ends], junctions: Vec::new() };
    };
    let lo = terms.iter().map(|t| t.at.y).fold(f64::MAX, f64::min);
    let hi = terms.iter().map(|t| t.at.y).fold(f64::MIN, f64::max);
    let mut wires = vec![vec![Point::new(x, lo), Point::new(x, hi)]];
    for t in terms {
        wires.push(vec![t.at, Point::new(x, t.at.y)]);
    }
    let mut ys: Vec<f64> = terms.iter().map(|t| t.at.y).collect();
    ys.sort_by(|a, b| a.total_cmp(b));
    ys.dedup_by(|a, b| (*a - *b).abs() < EPS);
    let mut junctions = Vec::new();
    for y in ys {
        let drops = terms.iter().filter(|t| (t.at.y - y).abs() < EPS).count();
        let trunk = if (y - lo).abs() < EPS || (y - hi).abs() < EPS { 1 } else { 2 };
        if drops + trunk >= 3 {
            junctions.push(Point::new(x, y));
        }
    }
    ChanRoute { wires, junctions }
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

/// ネットの配線が使えないなら、その理由 (使えるなら `None`)。
pub(crate) fn invalid_reason(route: &ChanRoute, terms: &[ChanTermAt], obs: &Obstacles, accepted: &[&ChanRoute]) -> Option<String> {
    for seg in route.segments() {
        if !(obs.free)(seg.0, seg.1) {
            return Some(format!("blocked {seg:?}"));
        }
        let probe = without_terminal_ends(seg, terms);
        if obs.branches.iter().any(|b| seg_rel(seg, *b) != Rel::Apart) {
            return Some(format!("tag branch {seg:?}"));
        }
        for line in obs.fixed_lines {
            if line.windows(2).any(|f| seg_rel(probe, (f[0], f[1])) == Rel::Short) {
                return Some(format!("fixed line {seg:?} vs {line:?}"));
            }
        }
        if obs.fixed_spots.iter().any(|&p| point_on(p, probe)) {
            return Some(format!("fixed spot {seg:?}"));
        }
        for other in accepted {
            if other.segments().iter().any(|o| seg_rel(seg, *o) == Rel::Short) {
                return Some(format!("short with an accepted net {seg:?}"));
            }
        }
    }
    None
}

/// ネットの配線が使えるか: 避けるものに触れず、すでに使うと決めた別のネットの配線と短絡しない。
pub(crate) fn net_is_valid(route: &ChanRoute, terms: &[ChanTermAt], obs: &Obstacles, accepted: &[&ChanRoute]) -> bool {
    invalid_reason(route, terms, obs, accepted).is_none()
}

/// 1つの通り道の、レイアウトが決めたもの。
#[derive(Debug, Clone, Default)]
pub(crate) struct ChannelLayout {
    /// 通り道の左右の端 (列kの右の張り出しと、列k+1の左の張り出し)。
    pub left: f64,
    pub right: f64,
    pub tracks: usize,
    pub placed: Vec<Placed>,
}

impl ChannelLayout {
    /// i番目のトラックのx。通り道の中ほどに、2.5mm間隔で並べる (両側の張り出しから5mm以上)。
    pub(crate) fn track_x(&self, i: usize) -> f64 {
        let used = TRACK_PITCH_MM * self.tracks.saturating_sub(1) as f64;
        let slack = (self.right - self.left) - 2.0 * TRACK_MARGIN_MM - used;
        self.left + TRACK_MARGIN_MM + grid_down(slack / 2.0).max(0.0) + TRACK_PITCH_MM * i as f64
    }
}

/// 通り道を通れるネット1本: どの通り道か、端 (位置と側)。
#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub channel: usize,
    pub net: usize,
    pub name: String,
    pub terms: Vec<ChanTermAt>,
}

/// 列の番号 → 機器のある列の並びの中の位置。
pub(crate) fn column_positions(model: &DraftModel) -> BTreeMap<usize, usize> {
    let mut c: Vec<usize> = model.units.iter().map(|u| u.column).collect();
    c.sort_unstable();
    c.dedup();
    c.into_iter().enumerate().map(|(k, c)| (c, k)).collect()
}

/// 通り道を通れるネット: 端が全て、ある列の右辺か、その右隣の列の左辺にある。端は2つ以上。札の枝を持つネット (札を下げた
/// 組が配線もあるネット) は、枝と旗が枝の行にあり、枝が端から幹へ向かう向きと重なるので通れない。
pub(crate) fn candidates(model: &DraftModel, terminals: &[Terminal]) -> Vec<Candidate> {
    let pos = column_positions(model);
    let ncols = pos.len();
    let mut by_net: BTreeMap<usize, Vec<&Terminal>> = BTreeMap::new();
    for t in terminals {
        by_net.entry(t.net).or_default().push(t);
    }
    let mut out = Vec::new();
    for (net, terms) in by_net {
        if terms.len() < 2 || terms.iter().any(|t| t.tag_at.is_some() || t.is_tag) {
            continue;
        }
        let mut channel: Option<usize> = None;
        let mut at = Vec::new();
        let mut ok = true;
        for t in &terms {
            let k = pos[&model.units[t.u].column];
            let (c, left) = match t.dir {
                Dir::E if k + 1 < ncols => (k, true),
                Dir::W if k >= 1 => (k - 1, false),
                _ => {
                    ok = false;
                    break;
                }
            };
            if channel.is_some_and(|prev| prev != c) {
                ok = false;
                break;
            }
            channel = Some(c);
            at.push(ChanTermAt { at: t.at, left });
        }
        if let (true, Some(channel)) = (ok, channel) {
            out.push(Candidate { channel, net, name: model.nets[net].name.clone(), terms: at });
        }
    }
    out
}

/// 候補を通り道ごとに割り当てる (通り道の数だけの割り当て)。
pub(crate) fn assign_all(cands: &[Candidate], channels: usize) -> Vec<Assignment> {
    (0..channels)
        .map(|k| {
            let nets: Vec<ChanNet> = cands
                .iter()
                .filter(|c| c.channel == k)
                .map(|c| ChanNet {
                    key: c.net,
                    name: c.name.clone(),
                    terms: c.terms.iter().map(|t| ChanTerm { y: t.at.y, left: t.left }).collect(),
                })
                .collect();
            assign_tracks(&nets)
        })
        .collect()
}

/// 割り当てに従って全ての通り道のネットを引き、使える配線だけを返す (ネットの添字 → 配線)。
/// 使えないネット (避けるものに触れる・短絡する) は含めない。それは格子の経路探索が引く。
pub(crate) fn route_channels(
    model: &DraftModel,
    layout: &DraftLayout,
    grid: &RouteGrid,
    fixed_lines: &[Vec<Point>],
    fixed_spots: &[Point],
) -> BTreeMap<usize, ChanRoute> {
    let cands = candidates(model, &layout.terminals);
    let branches: Vec<(Point, Point)> = layout.terminals.iter().filter_map(|t| t.tag_at.map(|tag| (t.at, tag))).collect();
    let free = |a: Point, b: Point| grid.segment_free(a, b);
    let obstacles = Obstacles { fixed_lines, fixed_spots, branches: &branches, free: &free };
    let mut out: BTreeMap<usize, ChanRoute> = BTreeMap::new();
    for (k, ch) in layout.channels.iter().enumerate() {
        let mut accepted: Vec<ChanRoute> = Vec::new();
        for placed in &ch.placed {
            let Some(c) = cands.iter().find(|c| c.channel == k && c.net == placed.key) else { continue };
            let route = route_geometry(&c.terms, placed.track.map(|i| ch.track_x(i)));
            let refs: Vec<&ChanRoute> = accepted.iter().collect();
            let why = invalid_reason(&route, &c.terms, &obstacles, &refs);
            if why.is_none() {
                accepted.push(route.clone());
                out.insert(c.net, route);
            }
        }
    }
    out
}
