//! PLC I/O (M4仕様 §3): モジュール定義・I/O割付表・I/O図面の自動生成・I/Oレポート。
//!
//! - **モジュール定義** [`PlcModuleSpec`]: 点数・入出力の種別・メーカ別のアドレス体系。
//!   部品DBの`plc_module`列 (JSON) に入っていて、図面には動的シンボル
//!   `plc_di_{n}p` / `plc_do_{n}p` ([`crate::symbol`]) として置かれる
//! - **割付表** [`crate::model::PlcAssignment`]: アドレス・信号名・コメントを人が決めて
//!   プロジェクトに保存する。編集は`set_plc_assignments`コマンド (undo可)
//! - **接続先・線番**: 図面の結線から読み取って表示するだけ ([`plc_points`])。
//!   図面を直せば表もレポートも変わる (双方向同期の「図面→表」の向き)
//! - **I/O図面の生成** [`generate_plc_sheet`]: 生成設定からラダーページを作り、
//!   [`Engine::execute_batch`]で入れるので**undo一発**でページごと消える

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::command::{Command, EditOrigin, Engine, Patch};
use crate::geometry::Point;
use crate::model::{
    Entity, EntityId, Junction, NetLabel, Orientation, PaperSize, PlcAssignment, Project, Sheet,
    SheetId, SymbolInstance, TextEntity, Wire,
};
use crate::netlist::{on_wire, pin_positions};
use crate::reports::{csv_row, endpoint_label};
use crate::symbol::{resolve_symbol, sheet_symbol_defs, PLC_POINT_MAX, PLC_POINT_PITCH_MM};
use crate::{CoreError, Result};

/// I/O割付表の編集できる列 (CSV入出力の列見出し)。接続先・線番は図面から導くので入らない。
pub const PLC_ASSIGNMENT_COLUMNS: [&str; 3] = ["アドレス", "信号名", "コメント"];

/// I/Oレポート (モジュール1つぶん) の列見出し。
pub const PLC_IO_COLUMNS: [&str; 5] = ["アドレス", "信号名", "接続先", "線番", "コメント"];

/// I/Oレポート (プロジェクト全体) の列見出し。[`PLC_IO_COLUMNS`]の先頭にモジュール列を足したもの。
pub const PLC_IO_PROJECT_COLUMNS: [&str; 6] = [
    "モジュール",
    "アドレス",
    "信号名",
    "接続先",
    "線番",
    "コメント",
];

/// 入力モジュールのシンボルidの接頭辞。
pub const PLC_DI_PREFIX: &str = "plc_di_";
/// 出力モジュールのシンボルidの接頭辞。
pub const PLC_DO_PREFIX: &str = "plc_do_";

/// I/Oの向き。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "UPPERCASE")]
pub enum PlcIoKind {
    /// 入力 (押釦・センサ → PLC)。
    Di,
    /// 出力 (PLC → 表示灯・リレー)。
    Do,
}

impl PlcIoKind {
    /// シンボルidの接頭辞 (`plc_di_` / `plc_do_`)。
    pub fn symbol_prefix(self) -> &'static str {
        match self {
            PlcIoKind::Di => PLC_DI_PREFIX,
            PlcIoKind::Do => PLC_DO_PREFIX,
        }
    }
}

/// メーカ別のアドレス体系。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum PlcAddressStyle {
    /// 三菱 (MELSEC FX/iQ-F系): `X0 X1 … X7 X10` と**8進**で数える。
    Mitsubishi,
    /// Siemens (SIMATIC): `%I0.0 … %I0.7 %I1.0` と「バイト.ビット」(1バイト8点)。
    Siemens,
    /// Allen-Bradley (SLC/MicroLogix系): `I:0/0 … I:0/15 I:1/0` と「ワード/ビット」(1ワード16点)。
    Ab,
}

impl PlcAddressStyle {
    /// この体系での0起点の点番号 → アドレスの後半部分。
    fn suffix(self, index: usize) -> String {
        match self {
            PlcAddressStyle::Mitsubishi => format!("{index:o}"),
            PlcAddressStyle::Siemens => format!("{}.{}", index / 8, index % 8),
            PlcAddressStyle::Ab => format!("{}/{}", index / 16, index % 16),
        }
    }
}

/// PLCモジュール1機種の定義 (部品DBの`plc_module`列のJSON)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PlcModuleSpec {
    /// I/O点数 (1〜[`PLC_POINT_MAX`])。
    pub points: usize,
    /// 入出力の種別。
    pub kind: PlcIoKind,
    /// アドレスの接頭辞 (例 "X" / "Y" / "%I" / "%Q" / "I:" / "O:")。
    #[serde(default)]
    pub address_prefix: String,
    /// アドレスの数え方。
    pub address_style: PlcAddressStyle,
}

impl PlcModuleSpec {
    /// 部品DBの`plc_module`列 (JSON) から読む。空欄・壊れたJSON・点数が範囲外ならNone。
    pub fn parse(json: &str) -> Option<Self> {
        let spec: PlcModuleSpec = serde_json::from_str(json.trim()).ok()?;
        (1..=PLC_POINT_MAX).contains(&spec.points).then_some(spec)
    }

    /// 部品DBの`plc_module`列へ入れるJSON。
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// この機種を図面に置くときの動的シンボルid (例 "plc_di_16p")。
    pub fn symbol_id(&self) -> String {
        format!("{}{}p", self.kind.symbol_prefix(), self.points)
    }

    /// 0起点の点番号に対応するアドレス (例 `X10`)。
    pub fn address_at(&self, index: usize) -> String {
        format!("{}{}", self.address_prefix, self.address_style.suffix(index))
    }

    /// `start`点目から始めて点数ぶんのアドレスを並べる (自動採番)。
    pub fn addresses(&self, start: usize) -> Vec<String> {
        (0..self.points).map(|i| self.address_at(start + i)).collect()
    }
}

/// 図面に置かれているPLCモジュール1つの概要 (割付表エディタの選択肢に使う)。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PlcModuleInfo {
    pub entity_id: EntityId,
    pub sheet_id: SheetId,
    pub sheet_name: String,
    /// 参照記号 (例 "PLC1")。割付表の`module_ref`と対応する。
    pub reference: String,
    /// 型番・値。
    pub value: String,
    /// I/O点数。
    pub points: usize,
    pub kind: PlcIoKind,
}

/// 割付表の1行に図面から読み取った接続先・線番を足したもの (エディタのグリッド1行)。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PlcPoint {
    /// 1起点の点番号 (モジュールシンボルのピン番号と同じ)。
    pub point: usize,
    pub address: String,
    pub signal_name: String,
    pub comment: String,
    /// 図面から読んだ接続先 (`"参照記号:ピン番号"` またはネットラベル名)。未結線なら空。
    pub target: String,
    /// 図面から読んだ線番。未採番・未結線なら空。
    pub wire_no: String,
}

impl PlcPoint {
    /// [`PLC_IO_COLUMNS`]と同じ並びのセル行。
    pub fn cells(&self) -> Vec<String> {
        vec![
            self.address.clone(),
            self.signal_name.clone(),
            self.target.clone(),
            self.wire_no.clone(),
            self.comment.clone(),
        ]
    }
}

/// `symbol_id`がPLCモジュールのシンボルか。
pub fn is_plc_module(symbol_id: &str) -> bool {
    symbol_id.starts_with(PLC_DI_PREFIX) || symbol_id.starts_with(PLC_DO_PREFIX)
}

/// シンボルidから入出力の種別を読む。PLCモジュールでなければNone。
pub fn kind_of_symbol(symbol_id: &str) -> Option<PlcIoKind> {
    if symbol_id.starts_with(PLC_DI_PREFIX) {
        Some(PlcIoKind::Di)
    } else if symbol_id.starts_with(PLC_DO_PREFIX) {
        Some(PlcIoKind::Do)
    } else {
        None
    }
}

/// 図面に置かれている全PLCモジュール (シート順→参照記号順)。
pub fn plc_modules(project: &Project) -> Vec<PlcModuleInfo> {
    let mut out = Vec::new();
    for sheet in &project.sheets {
        let mut on_sheet: Vec<&SymbolInstance> = sheet
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Symbol(s) if is_plc_module(&s.symbol_id) => Some(s),
                _ => None,
            })
            .collect();
        on_sheet.sort_by(|a, b| (&a.reference, a.id).cmp(&(&b.reference, b.id)));
        for s in on_sheet {
            let Some(kind) = kind_of_symbol(&s.symbol_id) else {
                continue;
            };
            let points = resolve_symbol(&s.symbol_id).map(|d| d.pins.len()).unwrap_or(0);
            out.push(PlcModuleInfo {
                entity_id: s.id,
                sheet_id: sheet.id,
                sheet_name: sheet.name.clone(),
                reference: s.reference.clone(),
                value: s.value.clone(),
                points,
                kind,
            });
        }
    }
    out
}

/// 割付表に載っているモジュールの参照記号 (表の並び順・重複なし)。
pub fn assigned_module_refs(project: &Project) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in &project.plc_assignments {
        if !out.contains(&a.module_ref) {
            out.push(a.module_ref.clone());
        }
    }
    out
}

/// 1つのモジュールの割付表 (図面から読んだ接続先・線番つき)。
///
/// **行は割付表の順**で、n行目がモジュールの点n番に対応する。図面にそのモジュールが
/// 置かれていなければ接続先・線番は空欄のままになる。
pub fn plc_points(project: &Project, module_ref: &str) -> Vec<PlcPoint> {
    let placed = project.sheets.iter().find_map(|sheet| {
        sheet.entities.values().find_map(|e| match e {
            Entity::Symbol(s)
                if is_plc_module(&s.symbol_id) && s.reference == module_ref =>
            {
                Some((sheet, s))
            }
            _ => None,
        })
    });
    project
        .plc_assignments
        .iter()
        .filter(|a| a.module_ref == module_ref)
        .enumerate()
        .map(|(i, a)| {
            let point = i + 1;
            let (target, wire_no) = placed
                .map(|(sheet, inst)| point_connection(sheet, inst, &point.to_string()))
                .unwrap_or_default();
            PlcPoint {
                point,
                address: a.address.clone(),
                signal_name: a.signal_name.clone(),
                comment: a.comment.clone(),
                target,
                wire_no,
            }
        })
        .collect()
}

/// モジュールの1点に繋がっている電線から (接続先, 線番) を読む。
///
/// 接続先は電線の**反対側**の端の表記 ([`endpoint_label`]) で、複数あれば ", " で並べる。
fn point_connection(sheet: &Sheet, inst: &SymbolInstance, pin_no: &str) -> (String, String) {
    let Some(def) = resolve_symbol(&inst.symbol_id) else {
        return (String::new(), String::new());
    };
    let Some(at) = pin_positions(inst, &def)
        .into_iter()
        .find(|(no, _)| no == pin_no)
        .map(|(_, p)| p)
    else {
        return (String::new(), String::new());
    };
    let defs = sheet_symbol_defs(sheet);
    let mut targets: Vec<String> = Vec::new();
    let mut numbers: Vec<String> = Vec::new();
    for entity in sheet.entities.values() {
        let Entity::Wire(w) = entity else { continue };
        if !on_wire(w, &at) {
            continue;
        }
        if let Some(no) = w.net.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            if !numbers.contains(&no.to_string()) {
                numbers.push(no.to_string());
            }
        }
        for end in [w.points.first(), w.points.last()].into_iter().flatten() {
            if end.distance_to(&at) < crate::netlist::CONNECT_EPS {
                continue;
            }
            let label = endpoint_label(sheet, &defs, end);
            if !label.is_empty() && !targets.contains(&label) {
                targets.push(label);
            }
        }
    }
    targets.sort();
    numbers.sort();
    (targets.join(", "), numbers.join("/"))
}

/// I/Oレポートの行 ([`PLC_IO_PROJECT_COLUMNS`]と同じ並び)。図面シート化・CSVから使う。
pub fn plc_io_rows(project: &Project) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for module_ref in assigned_module_refs(project) {
        for point in plc_points(project, &module_ref) {
            let mut row = vec![module_ref.clone()];
            row.extend(point.cells());
            rows.push(row);
        }
    }
    rows
}

/// I/OレポートのCSV。見出しは [`PLC_IO_PROJECT_COLUMNS`]。
pub fn plc_io_csv(project: &Project) -> String {
    let mut out = PLC_IO_PROJECT_COLUMNS.join(",");
    out.push('\n');
    for row in plc_io_rows(project) {
        out.push_str(&csv_row(&row));
        out.push('\n');
    }
    out
}

/// 1つのモジュールの割付表CSV (アドレス・信号名・コメントの3列)。
///
/// 接続先・線番は図面が正なので**入れない**。この形のまま
/// [`parse_assignments_csv`] で読み戻せる (Excelで信号名を一括入力する実務フロー)。
pub fn assignments_csv(project: &Project, module_ref: &str) -> String {
    let mut out = PLC_ASSIGNMENT_COLUMNS.join(",");
    out.push('\n');
    for a in project
        .plc_assignments
        .iter()
        .filter(|a| a.module_ref == module_ref)
    {
        out.push_str(&csv_row(&[
            a.address.clone(),
            a.signal_name.clone(),
            a.comment.clone(),
        ]));
        out.push('\n');
    }
    out
}

/// CSV1行をセルへ分ける (引用符で囲んだセル・セル内のカンマ・`""`によるエスケープに対応)。
fn split_csv_line(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted => {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            }
            '"' if cur.is_empty() && !quoted => quoted = true,
            ',' if !quoted => cells.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    cells.push(cur);
    cells
}

/// 割付表のCSVを読む。列は [`PLC_ASSIGNMENT_COLUMNS`] の3列で、見出し行はあってもなくてもよい。
///
/// 3列に足りない行・多すぎる行があればエラーにして**1行も採用しない**
/// (半端に取り込まれた表を残さない)。空行は読み飛ばす。
pub fn parse_assignments_csv(text: &str, module_ref: &str) -> Result<Vec<PlcAssignment>> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let cells = split_csv_line(line);
        if i == 0 && cells.first().map(|c| c.trim()) == Some(PLC_ASSIGNMENT_COLUMNS[0]) {
            continue; // 見出し行
        }
        if cells.len() != PLC_ASSIGNMENT_COLUMNS.len() {
            return Err(CoreError::InvalidCommand(format!(
                "{}行目: 列数が{}です ({}の3列で書いてください)",
                i + 1,
                cells.len(),
                PLC_ASSIGNMENT_COLUMNS.join("・")
            )));
        }
        out.push(PlcAssignment {
            id: Uuid::new_v4(),
            module_ref: module_ref.to_string(),
            address: cells[0].trim().to_string(),
            signal_name: cells[1].trim().to_string(),
            comment: cells[2].trim().to_string(),
        });
    }
    Ok(out)
}

/// あるモジュールの割付を`rows`で**置き換えた**割付表全体を組み立てる。
///
/// 他のモジュールの行は順番も内容もそのまま。置き換え先は元の位置 (そのモジュールの
/// 最初の行があった場所)、元が無ければ末尾。
pub fn replaced_assignments(
    project: &Project,
    module_ref: &str,
    rows: Vec<PlcAssignment>,
) -> Vec<PlcAssignment> {
    let mut out: Vec<PlcAssignment> = Vec::new();
    let mut inserted = false;
    for a in &project.plc_assignments {
        if a.module_ref == module_ref {
            if !inserted {
                out.extend(rows.iter().cloned());
                inserted = true;
            }
            continue;
        }
        out.push(a.clone());
    }
    if !inserted {
        out.extend(rows);
    }
    out
}

/// 割付表のCSVを取り込む (対象モジュールの行だけ置き換え)。**Commandを通す**ので
/// undo一発で元の表に戻る。CSVが読めなければ図面は一切変わらない。
pub fn import_assignments_csv(
    engine: &mut Engine,
    module_ref: &str,
    csv: &str,
    origin: EditOrigin,
) -> Result<Patch> {
    let rows = parse_assignments_csv(csv, module_ref)?;
    let assignments = replaced_assignments(engine.project(), module_ref, rows);
    engine.execute_batch(vec![Command::SetPlcAssignments { assignments }], origin)
}

/// ラダーの形式 (生成設定)。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum LadderStyle {
    /// 縦バス+横ラング (既定)。
    #[default]
    VerticalBus,
    /// 横バス+縦ラング。**未実装** (設定だけ用意してある)。
    HorizontalBus,
}

/// モジュールの配置方針 (生成設定)。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ModulePlacement {
    /// モジュールごとに新しいラダー (既定)。
    #[default]
    NewLadder,
    /// 収まる場合のみ前のモジュールと同居させる。**未実装**。
    ShareIfFits,
    /// 前のモジュールと同居させ、必要ならページを分割する。**未実装**。
    ShareOrSplit,
}

/// I/O図面の生成設定 (デザイン「生成設定ダイアログ」準拠)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PlcSheetOptions {
    /// ラング間隔 (mm)。2.5mmグリッドの正の倍数。
    pub rung_spacing_mm: f64,
    /// 先頭で空けるラング位置の数 (図枠の上や注記のためのスペース)。
    pub start_skip: usize,
    pub ladder_style: LadderStyle,
    pub placement: ModulePlacement,
}

impl Default for PlcSheetOptions {
    fn default() -> Self {
        Self {
            rung_spacing_mm: DEFAULT_RUNG_SPACING_MM,
            start_skip: 0,
            ladder_style: LadderStyle::default(),
            placement: ModulePlacement::default(),
        }
    }
}

/// 既定のラング間隔 (mm)。
pub const DEFAULT_RUNG_SPACING_MM: f64 = 10.0;
/// 生成したラダーの1本目のラングのY (mm。先頭スキップはここから下へ数える)。
pub const LADDER_TOP_MM: f64 = 40.0;
/// 縦バスのX (mm)。
pub const LADDER_BUS_X_MM: f64 = 40.0;
/// モジュールの接続点が並ぶX (mm)。
pub const LADDER_POINT_X_MM: f64 = 250.0;
/// ラング用の作図グリッド (mm)。
const GRID_MM: f64 = 2.5;
/// 縦バスに付ける電源のネット名 (直流24V制御電源)。
pub const LADDER_RAIL_NET: &str = "P24";

/// PLC I/O図面 (ラダーページ) を1枚生成してプロジェクトへ足す。
///
/// - **1点=1ラング**。左の縦バスから各点へ横ラングを引き、モジュールの点ピッチと
///   ラング間隔が違っても線が重ならないよう、点ごとに違う列で縦に振り分ける
/// - 割付表に足りない行は自動採番したアドレスで補う (信号名・コメントは表が正なので触らない)
/// - 生成物 (シート+割付表の追記) は**1回の編集**なのでundo一発で元に戻る
///
/// v1が対応するのは [`LadderStyle::VerticalBus`] + [`ModulePlacement::NewLadder`] だけで、
/// 他の設定は「未実装」のエラーになる (図面は変わらない)。
pub fn generate_plc_sheet(
    engine: &mut Engine,
    module_ref: &str,
    spec: &PlcModuleSpec,
    options: &PlcSheetOptions,
    origin: EditOrigin,
) -> Result<Patch> {
    let module_ref = module_ref.trim();
    if module_ref.is_empty() {
        return Err(CoreError::InvalidCommand(
            "モジュールの参照記号 (例 PLC1) を指定してください".into(),
        ));
    }
    if options.ladder_style != LadderStyle::VerticalBus {
        return Err(CoreError::InvalidCommand(
            "横バス+縦ラングのラダー形式は未実装です (縦バス+横ラングを選んでください)".into(),
        ));
    }
    if options.placement != ModulePlacement::NewLadder {
        return Err(CoreError::InvalidCommand(
            "モジュールを同居させる配置方針は未実装です (モジュールごとに新ラダーを選んでください)"
                .into(),
        ));
    }
    let spacing = options.rung_spacing_mm;
    if !(spacing > 0.0 && (spacing / GRID_MM - (spacing / GRID_MM).round()).abs() < 1e-9) {
        return Err(CoreError::InvalidCommand(format!(
            "ラング間隔は{GRID_MM}mmグリッドの正の倍数にしてください (指定: {spacing})"
        )));
    }
    if !(1..=PLC_POINT_MAX).contains(&spec.points) {
        return Err(CoreError::InvalidCommand(format!(
            "PLCモジュールの点数は1〜{PLC_POINT_MAX}です (指定: {})",
            spec.points
        )));
    }

    // 割付表: 足りない行を自動採番で補う (既にある信号名・コメントはそのまま)
    let existing: Vec<PlcAssignment> = engine
        .project()
        .plc_assignments
        .iter()
        .filter(|a| a.module_ref == module_ref)
        .cloned()
        .collect();
    let mut rows = existing.clone();
    for i in rows.len()..spec.points {
        rows.push(PlcAssignment {
            id: Uuid::new_v4(),
            module_ref: module_ref.to_string(),
            address: spec.address_at(i),
            signal_name: String::new(),
            comment: String::new(),
        });
    }
    let mut commands = Vec::new();
    if rows.len() != existing.len() {
        commands.push(Command::SetPlcAssignments {
            assignments: replaced_assignments(engine.project(), module_ref, rows.clone()),
        });
    }
    let sheet = build_ladder_sheet(module_ref, spec, options, &rows);
    commands.push(Command::RestoreSheet {
        index: engine.project().sheets.len(),
        sheet: Box::new(sheet),
    });
    engine.execute_batch(commands, origin)
}

/// ラダーページ1枚を組み立てる (図面には触れない純関数)。
fn build_ladder_sheet(
    module_ref: &str,
    spec: &PlcModuleSpec,
    options: &PlcSheetOptions,
    rows: &[PlcAssignment],
) -> Sheet {
    let n = spec.points;
    let spacing = options.rung_spacing_mm;
    let top = LADDER_TOP_MM + options.start_skip as f64 * spacing;
    let rung_y = |i: usize| top + i as f64 * spacing;
    let point_y = |i: usize| top + i as f64 * PLC_POINT_PITCH_MM;

    let mut sheet = Sheet::new(
        &format!("PLC {module_ref}"),
        PaperSize::A3,
        Orientation::Landscape,
    );
    sheet.title_block.title = format!("PLC I/O {module_ref}");
    let mut add = |entity: Entity| {
        sheet.entities.insert(entity.id(), entity);
    };

    // モジュールのシンボル (点は左側・中央揃えなので、点1が1本目のラングに来るよう置く)
    add(Entity::Symbol(SymbolInstance {
        id: Uuid::new_v4(),
        symbol_id: spec.symbol_id(),
        at: Point::new(
            LADDER_POINT_X_MM + 7.5,
            point_y(0) + (n - 1) as f64 * PLC_POINT_PITCH_MM / 2.0,
        ),
        rotation: 0,
        mirror: false,
        reference: module_ref.to_string(),
        value: String::new(),
        attrs: Default::default(),
    }));

    // 縦バス: 1本目のラングの5mm上から最後のラングまで。上端にはネットラベルを付ける
    let bus_top = rung_y(0) - 5.0;
    add(Entity::Wire(Wire {
        id: Uuid::new_v4(),
        points: vec![
            Point::new(LADDER_BUS_X_MM, bus_top),
            Point::new(LADDER_BUS_X_MM, rung_y(n - 1)),
        ],
        color: String::new(),
        sq: 0.0,
        length_m: None,
        part_no: None,
        net: None,
    }));
    add(Entity::NetLabel(NetLabel {
        id: Uuid::new_v4(),
        at: Point::new(LADDER_BUS_X_MM, bus_top),
        name: LADDER_RAIL_NET.into(),
        rotation: 0,
    }));

    for (i, row) in rows.iter().enumerate().take(n) {
        let (ry, py) = (rung_y(i), point_y(i));
        // 点ごとに違う列で縦へ振り分ける (ラング間隔と点ピッチが違っても線が重ならない)
        let drop_x = LADDER_POINT_X_MM - 5.0 - (n - 1 - i) as f64 * GRID_MM;
        let points = if (py - ry).abs() < 1e-9 {
            vec![
                Point::new(LADDER_BUS_X_MM, ry),
                Point::new(LADDER_POINT_X_MM, ry),
            ]
        } else {
            vec![
                Point::new(LADDER_BUS_X_MM, ry),
                Point::new(drop_x, ry),
                Point::new(drop_x, py),
                Point::new(LADDER_POINT_X_MM, py),
            ]
        };
        add(Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points,
            color: String::new(),
            sq: 0.0,
            length_m: None,
            part_no: None,
            net: None,
        }));
        // バスとの接続点 (ドット)
        add(Entity::Junction(Junction {
            id: Uuid::new_v4(),
            at: Point::new(LADDER_BUS_X_MM, ry),
        }));
        // ラングの見出し: アドレス+信号名
        let label = [row.address.as_str(), row.signal_name.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if !label.is_empty() {
            add(Entity::Text(TextEntity {
                id: Uuid::new_v4(),
                at: Point::new(LADDER_BUS_X_MM + 5.0, ry - 2.0),
                text: label,
                height: 2.5,
                rotation: 0,
            }));
        }
    }
    sheet
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{EditOrigin, Engine};
    use crate::geometry::Point;
    use crate::model::*;
    use crate::netlist::pin_positions;
    use crate::symbol::resolve_symbol;
    use uuid::Uuid;

    fn spec(points: usize, kind: PlcIoKind, prefix: &str, style: PlcAddressStyle) -> PlcModuleSpec {
        PlcModuleSpec {
            points,
            kind,
            address_prefix: prefix.into(),
            address_style: style,
        }
    }

    fn di16() -> PlcModuleSpec {
        spec(16, PlcIoKind::Di, "X", PlcAddressStyle::Mitsubishi)
    }

    /// PLCモジュールを1つ置いただけのプロジェクト (参照記号PLC1)。
    fn project_with_module(points: usize) -> (Project, EntityId) {
        let mut project = Project::new("plc");
        let sid = project.sheets[0].id;
        let inst = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: format!("plc_di_{points}p"),
            at: Point::new(200.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "PLC1".into(),
            value: "FX5-16EX".into(),
            attrs: Default::default(),
        };
        let id = inst.id;
        let sheet = project.sheet_mut(sid).unwrap();
        sheet.entities.insert(id, Entity::Symbol(inst));
        (project, id)
    }

    fn assignment(module_ref: &str, address: &str, signal: &str, comment: &str) -> PlcAssignment {
        PlcAssignment {
            id: Uuid::new_v4(),
            module_ref: module_ref.into(),
            address: address.into(),
            signal_name: signal.into(),
            comment: comment.into(),
        }
    }

    /// Mitsubishi-style addresses count up in octal, so the eighth point of an X module is X10 rather than X8.
    /// 三菱式のアドレスは8進で数えるので、Xモジュールの8点目はX8ではなくX10になる。
    #[test]
    fn mitsubishi_addresses_count_up_in_octal() {
        let m = di16();
        let addresses = m.addresses(0);
        assert_eq!(addresses.len(), 16);
        assert_eq!(&addresses[0..9], ["X0", "X1", "X2", "X3", "X4", "X5", "X6", "X7", "X10"]);
        assert_eq!(addresses[15], "X17");
    }

    /// Siemens-style addresses are written as byte.bit with eight bits per byte, so the ninth point is %I1.0.
    /// Siemens式のアドレスは「バイト.ビット」で1バイト8点なので、9点目は%I1.0になる。
    #[test]
    fn siemens_addresses_are_written_as_byte_and_bit() {
        let m = spec(16, PlcIoKind::Di, "%I", PlcAddressStyle::Siemens);
        let addresses = m.addresses(0);
        assert_eq!(addresses[0], "%I0.0");
        assert_eq!(addresses[7], "%I0.7");
        assert_eq!(addresses[8], "%I1.0");
    }

    /// Allen-Bradley-style addresses are written as word/bit with sixteen bits per word, so the seventeenth point is I:1/0.
    /// Allen-Bradley式のアドレスは「ワード/ビット」で1ワード16点なので、17点目はI:1/0になる。
    #[test]
    fn allen_bradley_addresses_are_written_as_word_and_bit() {
        let m = spec(32, PlcIoKind::Di, "I:", PlcAddressStyle::Ab);
        let addresses = m.addresses(0);
        assert_eq!(addresses[0], "I:0/0");
        assert_eq!(addresses[15], "I:0/15");
        assert_eq!(addresses[16], "I:1/0");
    }

    /// Auto-numbering can start from a point offset, so a second module continues where the first one ended.
    /// アドレスの自動採番は開始点をずらせるので、2枚目のモジュールは1枚目の続きから振れる。
    #[test]
    fn auto_addresses_can_start_from_a_point_offset() {
        let m = spec(4, PlcIoKind::Do, "Y", PlcAddressStyle::Mitsubishi);
        assert_eq!(m.addresses(16), ["Y20", "Y21", "Y22", "Y23"]);
    }

    /// A module definition knows which dynamic symbol to place: input modules use plc_di_{n}p and output modules plc_do_{n}p.
    /// モジュール定義は置くべき動的シンボルを知っている (入力=plc_di_{n}p、出力=plc_do_{n}p)。
    #[test]
    fn a_module_definition_names_its_drawing_symbol() {
        assert_eq!(di16().symbol_id(), "plc_di_16p");
        assert_eq!(
            spec(8, PlcIoKind::Do, "Y", PlcAddressStyle::Mitsubishi).symbol_id(),
            "plc_do_8p"
        );
    }

    /// A module definition survives the round trip through the parts-database JSON column.
    /// モジュール定義は部品DBのJSON列を往復しても内容が変わらない。
    #[test]
    fn a_module_definition_round_trips_through_json() {
        let m = spec(8, PlcIoKind::Do, "%Q", PlcAddressStyle::Siemens);
        let back = PlcModuleSpec::parse(&m.to_json()).expect("読み直せる");
        assert_eq!(back, m);
        assert!(PlcModuleSpec::parse("").is_none(), "空欄はモジュールではない");
        assert!(PlcModuleSpec::parse("{ 壊れた").is_none());
    }

    /// The assignment table is written to CSV as address, signal name and comment, and reading it back gives the same rows.
    /// 割付表はアドレス・信号名・コメントの3列でCSVになり、読み直すと同じ行に戻る。
    #[test]
    fn the_assignment_table_round_trips_through_csv() {
        let mut project = Project::new("t");
        project.plc_assignments = vec![
            assignment("PLC1", "X0", "起動押釦", "PB1"),
            assignment("PLC1", "X1", "停止押釦, 非常停止", ""),
        ];
        let csv = assignments_csv(&project, "PLC1");
        assert_eq!(csv.lines().next().unwrap(), "アドレス,信号名,コメント");
        let rows = parse_assignments_csv(&csv, "PLC1").expect("読める");
        let seen: Vec<(String, String, String)> = rows
            .iter()
            .map(|r| (r.address.clone(), r.signal_name.clone(), r.comment.clone()))
            .collect();
        assert_eq!(
            seen,
            vec![
                ("X0".into(), "起動押釦".into(), "PB1".into()),
                ("X1".into(), "停止押釦, 非常停止".into(), String::new()),
            ]
        );
        assert!(rows.iter().all(|r| r.module_ref == "PLC1"));
    }

    /// A CSV without a header row is read as data, so a spreadsheet exported without titles still loads.
    /// 見出し行の無いCSVもデータとして読めるので、見出しを付けずに書き出した表もそのまま取り込める。
    #[test]
    fn a_csv_without_a_header_row_is_still_read() {
        let rows = parse_assignments_csv("X0,起動,\nX1,停止,\n", "PLC1").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].address, "X0");
    }

    /// Importing a CSV replaces only the target module's rows and leaves other modules untouched; one undo puts the old table back.
    /// CSVの取り込みは対象モジュールの行だけを置き換え、他のモジュールの割付はそのまま残る。undo一発で元の表に戻る。
    #[test]
    fn importing_a_csv_replaces_only_the_target_module() {
        let mut project = Project::new("t");
        project.plc_assignments = vec![
            assignment("PLC1", "X0", "旧", ""),
            assignment("PLC2", "Y0", "他のモジュール", ""),
        ];
        let mut engine = Engine::new(project);
        import_assignments_csv(&mut engine, "PLC1", "X0,新,\nX1,追加,\n", EditOrigin::User).unwrap();
        let table = &engine.project().plc_assignments;
        assert_eq!(table.len(), 3, "PLC1が2行+PLC2が1行: {table:?}");
        assert!(table.iter().any(|a| a.module_ref == "PLC2" && a.signal_name == "他のモジュール"));
        assert!(table.iter().any(|a| a.module_ref == "PLC1" && a.signal_name == "新"));
        assert!(!table.iter().any(|a| a.signal_name == "旧"), "古い行は消える");
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().plc_assignments.len(), 2, "undo一発で戻る");
    }

    /// A CSV row without the three columns is refused, and the drawing keeps its old assignment table.
    /// 3列に足りない行があるCSVは拒否され、図面の割付表は元のまま変わらない。
    #[test]
    fn a_malformed_csv_row_is_refused_without_changing_the_table() {
        let mut project = Project::new("t");
        project.plc_assignments = vec![assignment("PLC1", "X0", "元", "")];
        let mut engine = Engine::new(project);
        assert!(parse_assignments_csv("X0\n", "PLC1").is_err());
        assert!(import_assignments_csv(&mut engine, "PLC1", "X0\n", EditOrigin::User).is_err());
        assert_eq!(engine.project().plc_assignments[0].signal_name, "元");
    }

    /// Each point of the assignment table picks up the connected device and the wire number from the drawing.
    /// 割付表の各点は、図面の結線から接続先の機器と線番を拾ってくる。
    #[test]
    fn each_point_picks_up_its_target_and_wire_number_from_the_drawing() {
        let (mut project, _) = project_with_module(4);
        let sid = project.sheets[0].id;
        project.plc_assignments = vec![
            assignment("PLC1", "X0", "起動押釦", "常時開"),
            assignment("PLC1", "X1", "停止押釦", ""),
        ];
        // 1点目のピン位置へ抵抗R1のピン2から電線を引く (線番101)
        let Entity::Symbol(inst) = project.sheets[0]
            .entities
            .values()
            .find(|e| matches!(e, Entity::Symbol(s) if s.reference == "PLC1"))
            .unwrap()
            .clone()
        else {
            panic!()
        };
        let def = resolve_symbol(&inst.symbol_id).unwrap();
        let pin1 = pin_positions(&inst, &def)
            .into_iter()
            .find(|(no, _)| no == "1")
            .unwrap()
            .1;
        let r1 = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(pin1.x - 20.0, pin1.y),
            rotation: 0,
            mirror: false,
            reference: "R1".into(),
            value: String::new(),
            attrs: Default::default(),
        });
        let wire = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(pin1.x - 12.5, pin1.y), pin1],
            color: "black".into(),
            sq: 0.3,
            length_m: None,
            part_no: None,
            net: Some("101".into()),
        });
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [r1, wire] {
            sheet.entities.insert(e.id(), e);
        }
        let points = plc_points(&project, "PLC1");
        assert_eq!(points.len(), 2, "割付表の行数ぶん");
        assert_eq!(points[0].address, "X0");
        assert_eq!(points[0].signal_name, "起動押釦");
        assert_eq!(points[0].comment, "常時開");
        assert_eq!(points[0].target, "R1:2", "結線した相手が接続先になる");
        assert_eq!(points[0].wire_no, "101");
        assert_eq!(points[1].target, "", "未結線の点は接続先が空欄");
        assert_eq!(points[1].wire_no, "");
    }

    /// The modules placed on the drawing are listed with their point count and I/O kind, so the editor can offer them for selection.
    /// 図面に置かれたPLCモジュールは点数と入出力の種別つきで一覧でき、エディタの選択肢にできる。
    #[test]
    fn placed_modules_are_listed_with_their_point_count_and_kind() {
        let (project, entity_id) = project_with_module(8);
        let modules = plc_modules(&project);
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].entity_id, entity_id);
        assert_eq!(modules[0].reference, "PLC1");
        assert_eq!(modules[0].points, 8);
        assert_eq!(modules[0].kind, PlcIoKind::Di);
        assert_eq!(modules[0].sheet_id, project.sheets[0].id);
    }

    /// Generating an I/O drawing adds one new sheet whose ladder has one rung per point of the module.
    /// I/O図面の生成はシートを1枚増やし、そのラダーにはモジュールの点数ぶんのラングが並ぶ。
    #[test]
    fn generating_an_io_drawing_makes_one_rung_per_point() {
        let (project, _) = project_with_module(4);
        let mut engine = Engine::new(project);
        let sheets_before = engine.project().sheets.len();
        generate_plc_sheet(
            &mut engine,
            "PLC1",
            &spec(4, PlcIoKind::Di, "X", PlcAddressStyle::Mitsubishi),
            &PlcSheetOptions::default(),
            EditOrigin::User,
        )
        .unwrap();
        assert_eq!(engine.project().sheets.len(), sheets_before + 1);
        let sheet = engine.project().sheets.last().unwrap();
        assert_eq!(rung_ys(sheet).len(), 4, "1点=1ラング");
        // モジュールのシンボルも1つ置かれる
        assert_eq!(
            sheet
                .entities
                .values()
                .filter(|e| matches!(e, Entity::Symbol(s) if s.reference == "PLC1"))
                .count(),
            1
        );
    }

    /// Rungs sit at the requested vertical spacing, and the requested number of leading rung positions is left empty.
    /// ラングは指定したラング間隔で並び、先頭の指定本数ぶんの位置は空けられる。
    #[test]
    fn rungs_follow_the_requested_spacing_and_leading_skip() {
        let (project, _) = project_with_module(3);
        let mut engine = Engine::new(project);
        let options = PlcSheetOptions {
            rung_spacing_mm: 12.5,
            start_skip: 2,
            ..Default::default()
        };
        generate_plc_sheet(
            &mut engine,
            "PLC1",
            &spec(3, PlcIoKind::Di, "X", PlcAddressStyle::Mitsubishi),
            &options,
            EditOrigin::User,
        )
        .unwrap();
        let sheet = engine.project().sheets.last().unwrap();
        let ys = rung_ys(sheet);
        assert_eq!(ys.len(), 3);
        assert!((ys[1] - ys[0] - 12.5).abs() < 1e-9, "ラング間隔: {ys:?}");
        assert!((ys[2] - ys[1] - 12.5).abs() < 1e-9);
        // 先頭スキップ2本ぶん下から始まる
        let top = LADDER_TOP_MM + 2.0 * 12.5;
        assert!((ys[0] - top).abs() < 1e-9, "先頭スキップ: {ys:?}");
    }

    /// The whole generated page is a single edit, so one undo removes the sheet and everything on it.
    /// 生成したページ全体が1回の編集なので、undo一発でシートごと消える。
    #[test]
    fn a_generated_io_page_is_undone_in_one_step() {
        let (project, _) = project_with_module(8);
        let mut engine = Engine::new(project);
        let before = engine.project().sheets.len();
        generate_plc_sheet(
            &mut engine,
            "PLC1",
            &di16(),
            &PlcSheetOptions::default(),
            EditOrigin::User,
        )
        .unwrap();
        assert_eq!(engine.undo_depth(), 1, "コマンドが何本でも履歴は1件");
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets.len(), before, "undo一発で元通り");
    }

    /// Generating twice adds a second sheet instead of overwriting the first one.
    /// 2回生成すると1枚目を上書きせず、2枚目のシートが増える。
    #[test]
    fn generating_twice_adds_a_second_sheet() {
        let (project, _) = project_with_module(4);
        let mut engine = Engine::new(project);
        let module = spec(4, PlcIoKind::Di, "X", PlcAddressStyle::Mitsubishi);
        for _ in 0..2 {
            generate_plc_sheet(
                &mut engine,
                "PLC1",
                &module,
                &PlcSheetOptions::default(),
                EditOrigin::User,
            )
            .unwrap();
        }
        assert_eq!(engine.project().sheets.len(), 3, "元の1枚+生成2枚");
    }

    /// A generated I/O page has no ERC complaints: every point is wired and every wire end lands on something.
    /// 生成したI/Oページには ERC の指摘が出ない (全ての点が結線され、電線の端はどこかに繋がっている)。
    #[test]
    fn a_generated_io_page_passes_the_electrical_rule_check() {
        let mut engine = Engine::new(Project::new("t"));
        generate_plc_sheet(
            &mut engine,
            "PLC1",
            &spec(6, PlcIoKind::Di, "X", PlcAddressStyle::Mitsubishi),
            &PlcSheetOptions::default(),
            EditOrigin::User,
        )
        .unwrap();
        let sheet = engine.project().sheets.last().unwrap();
        let diags = crate::verify::verify_sheet(sheet, &crate::symbol::sheet_symbol_defs(sheet));
        let erc: Vec<_> = diags.iter().filter(|d| d.code.starts_with("erc.")).collect();
        assert!(erc.is_empty(), "ERC指摘なし: {erc:?}");
    }

    /// Generating for a module with no assignments fills the table with auto-numbered addresses in the same single edit.
    /// 割付表が空のモジュールを生成すると、同じ1回の編集の中で自動採番したアドレスが表に入る。
    #[test]
    fn generating_fills_an_empty_assignment_table_with_auto_addresses() {
        let (project, _) = project_with_module(4);
        let mut engine = Engine::new(project);
        generate_plc_sheet(
            &mut engine,
            "PLC1",
            &spec(4, PlcIoKind::Di, "X", PlcAddressStyle::Mitsubishi),
            &PlcSheetOptions::default(),
            EditOrigin::User,
        )
        .unwrap();
        let addresses: Vec<&str> = engine
            .project()
            .plc_assignments
            .iter()
            .map(|a| a.address.as_str())
            .collect();
        assert_eq!(addresses, ["X0", "X1", "X2", "X3"]);
        engine.undo().unwrap().unwrap();
        assert!(engine.project().plc_assignments.is_empty(), "undoで表も戻る");
    }

    /// Existing signal names are kept when the page is generated: the assignment table stays the master of names and comments.
    /// 生成しても既存の信号名は保たれる (信号名・コメントは割付表が正)。
    #[test]
    fn generating_keeps_the_signal_names_already_in_the_table() {
        let (mut project, _) = project_with_module(2);
        project.plc_assignments = vec![assignment("PLC1", "X0", "起動押釦", "")];
        let mut engine = Engine::new(project);
        generate_plc_sheet(
            &mut engine,
            "PLC1",
            &spec(2, PlcIoKind::Di, "X", PlcAddressStyle::Mitsubishi),
            &PlcSheetOptions::default(),
            EditOrigin::User,
        )
        .unwrap();
        let sheet = engine.project().sheets.last().unwrap();
        let texts: Vec<String> = sheet
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        assert!(
            texts.iter().any(|t| t.contains("起動押釦") && t.contains("X0")),
            "ラングに信号名とアドレスが載る: {texts:?}"
        );
    }

    /// The two module-placement policies that are not implemented yet are refused with a clear error, and the drawing is left untouched.
    /// まだ実装していない2つのモジュール配置方針ははっきりしたエラーで断られ、図面には何も起きない。
    #[test]
    fn the_unimplemented_placement_policies_are_refused() {
        let (project, _) = project_with_module(4);
        let mut engine = Engine::new(project);
        let module = spec(4, PlcIoKind::Di, "X", PlcAddressStyle::Mitsubishi);
        for placement in [ModulePlacement::ShareIfFits, ModulePlacement::ShareOrSplit] {
            let options = PlcSheetOptions {
                placement,
                ..Default::default()
            };
            let err = generate_plc_sheet(&mut engine, "PLC1", &module, &options, EditOrigin::User)
                .unwrap_err()
                .to_string();
            assert!(err.contains("未実装"), "理由が書かれている: {err}");
        }
        assert_eq!(engine.project().sheets.len(), 1, "図面は変わらない");
    }

    /// The horizontal-bus ladder style is not implemented yet and is refused the same way.
    /// 横バス+縦ラングのラダー形式もまだ実装しておらず、同じように断られる。
    #[test]
    fn the_horizontal_bus_ladder_style_is_refused() {
        let (project, _) = project_with_module(4);
        let mut engine = Engine::new(project);
        let options = PlcSheetOptions {
            ladder_style: LadderStyle::HorizontalBus,
            ..Default::default()
        };
        let err = generate_plc_sheet(
            &mut engine,
            "PLC1",
            &spec(4, PlcIoKind::Di, "X", PlcAddressStyle::Mitsubishi),
            &options,
            EditOrigin::User,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("未実装"), "{err}");
    }

    /// A rung spacing that is not a positive multiple of the 2.5 mm grid is refused, so generated pages stay on grid.
    /// 2.5mmグリッドの正の倍数でないラング間隔は断られるので、生成したページは必ずグリッドに乗る。
    #[test]
    fn an_off_grid_rung_spacing_is_refused() {
        let (project, _) = project_with_module(4);
        let mut engine = Engine::new(project);
        let module = spec(4, PlcIoKind::Di, "X", PlcAddressStyle::Mitsubishi);
        for spacing in [0.0, -5.0, 7.0] {
            let options = PlcSheetOptions {
                rung_spacing_mm: spacing,
                ..Default::default()
            };
            assert!(
                generate_plc_sheet(&mut engine, "PLC1", &module, &options, EditOrigin::User)
                    .is_err(),
                "ラング間隔 {spacing} は拒否される"
            );
        }
    }

    /// The I/O report lists every point as address, signal name, target, wire number and comment, with the module reference in front for the whole project.
    /// I/Oレポートは各点をアドレス・信号名・接続先・線番・コメントで並べ、プロジェクト全体版では先頭にモジュールの参照記号が付く。
    #[test]
    fn the_io_report_lists_address_signal_target_wire_number_and_comment() {
        assert_eq!(
            PLC_IO_COLUMNS,
            ["アドレス", "信号名", "接続先", "線番", "コメント"]
        );
        assert_eq!(PLC_IO_PROJECT_COLUMNS[0], "モジュール");
        let mut project = Project::new("t");
        project.plc_assignments = vec![assignment("PLC1", "X0", "起動押釦", "PB1")];
        let rows = plc_io_rows(&project);
        assert_eq!(
            rows,
            vec![vec![
                "PLC1".to_string(),
                "X0".into(),
                "起動押釦".into(),
                String::new(),
                String::new(),
                "PB1".into(),
            ]]
        );
    }

    /// ラダーの横ラング (左バスから点へ伸びる電線) のY座標を上から順に返す。
    fn rung_ys(sheet: &Sheet) -> Vec<f64> {
        let mut ys: Vec<f64> = sheet
            .entities
            .values()
            .filter_map(|e| match e {
                // 縦バス (始点と終点のxが同じ) 以外の電線がラング
                Entity::Wire(w) if w.points[0].x != w.points[w.points.len() - 1].x => {
                    Some(w.points[0].y)
                }
                _ => None,
            })
            .collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ys
    }
}
