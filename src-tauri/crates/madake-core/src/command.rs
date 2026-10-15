use serde::{Deserialize, Serialize};

use crate::model::*;
use crate::wire_no::{plan_sheet, reserve_sheet, Numberer, RenumberMode};
use crate::{CoreError, Result};

/// シリアライズ可能な編集コマンド。UI(Tauri IPC)とAI(MCP)の両方がこれを発行する。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    AddSheet {
        name: String,
        size: PaperSize,
        orientation: Orientation,
        /// シート種別 (省略時は回路図)。盤レイアウトシートは`"panel"`。
        #[serde(default)]
        kind: SheetKind,
    },
    RemoveSheet {
        sheet_id: SheetId,
    },
    /// 位置(index)を指定して**組み立て済みのシートを丸ごと挿入する**。
    /// 削除の逆操作(復元)であると同時に、生成した図面ページ(PLC I/O図面など)を
    /// 1コマンドで入れる口でもある(エンティティ込みで入るのでundo一発で消える)。
    RestoreSheet {
        index: usize,
        sheet: Box<Sheet>,
    },
    RenameSheet {
        sheet_id: SheetId,
        name: String,
    },
    SetTitleBlock {
        sheet_id: SheetId,
        title_block: TitleBlock,
    },
    SetRevisions {
        sheet_id: SheetId,
        revisions: Vec<Revision>,
    },
    /// シートの注記文字高 (mm) の設定 (G20)。配線注記・線番・ネットタグの大きさが変わる。
    /// `height_mm`がnullなら既定 ([`crate::model::DEFAULT_ANNOTATION_TEXT_MM`]) に戻す。
    /// 範囲は [`crate::model::MIN_ANNOTATION_TEXT_MM`]〜[`crate::model::MAX_ANNOTATION_TEXT_MM`]。
    SetSheetAnnotationText {
        sheet_id: SheetId,
        #[serde(default)]
        height_mm: Option<f64>,
    },
    /// シートの用紙サイズ・向きの変更 (G15)。既存エンティティの座標はそのまま
    /// (動かない・消えない)。ゾーン分割 (`zone_cols`/`zone_rows`) も**変えない**
    /// (シートが正。図枠テンプレートの差し替えと同じで、クロスリファレンスの
    /// ゾーン住所を用紙変更で書き換えないため)。逆コマンドはサイズと向きだけを元へ戻す。
    /// 新しい用紙の外に出たエンティティはERC警告 (`erc.outside_paper`) で報告する
    /// (このコマンドでは検出しない。検証は読み取り専用で行う)。
    SetSheetPaper {
        sheet_id: SheetId,
        size: PaperSize,
        orientation: Orientation,
    },
    AddEntity {
        sheet_id: SheetId,
        entity: Entity,
    },
    /// 多芯ケーブルを置く (M4 §5)。定義線 (`cable.points`) が横切った配線を、始点に近い順に
    /// 芯 "1", "2", … として割り当てる (芯の色=配線の線色)。`cable.cores`は無視して作り直す。
    /// `core_count`が0なら横切った本数 (最低1) にする。逆コマンドは削除。
    AddCable {
        sheet_id: SheetId,
        cable: crate::model::Cable,
    },
    /// idで既存エンティティを完全置換。
    UpdateEntity {
        sheet_id: SheetId,
        entity: Entity,
    },
    DeleteEntities {
        sheet_id: SheetId,
        ids: Vec<EntityId>,
    },
    MoveEntities {
        sheet_id: SheetId,
        ids: Vec<EntityId>,
        dx: f64,
        dy: f64,
    },
    SetWireParts {
        wire_parts: Vec<WirePart>,
    },
    /// PLC I/O割付表の置換 (プロジェクト単位)。逆コマンドは置換前のリスト。
    SetPlcAssignments {
        assignments: Vec<PlcAssignment>,
    },
    /// PLC I/O図面の生成設定をプロジェクトに保存する (`None`で保存を消す)。生成ダイアログの
    /// 「この設定をプロジェクトに保存」。逆コマンドは保存前の設定。
    SetPlcSheetOptions {
        #[serde(default)]
        options: Option<crate::plc::PlcSheetOptions>,
    },
    /// 図枠テンプレートの差し替え (プロジェクト単位。M4仕様 §9)。全シートの図枠・
    /// 表題欄・改訂欄が一括で変わる。nullで既定 (`jis-a3`) へ戻す。
    /// 逆コマンドは差し替え前のid。
    SetFrameTemplate {
        #[serde(default)]
        frame_template: Option<String>,
    },
    /// 線番のネット単位自動採番。sheet_id省略時はプロジェクトの全シートが対象。
    RenumberWires {
        #[serde(default)]
        sheet_id: Option<SheetId>,
        mode: RenumberMode,
        #[serde(default = "crate::wire_no::default_start")]
        start: u32,
    },
    /// 線番の直接指定 (個別編集・自動採番の逆コマンド)。numberがnullなら線番を消す。
    SetWireNumbers {
        sheet_id: SheetId,
        numbers: Vec<WireNumber>,
    },
    /// 盤レイアウトシートのアイテム番号 (バルーン) の自動採番 (M4仕様 §8)。
    /// そのシートのフットプリントへ読み順 ([`crate::panel::item_order`]) で 1..N を振る。
    /// 逆コマンドは採番前の番号 (`SetPanelItemNumbers`)。
    RenumberPanelItems {
        sheet_id: SheetId,
    },
    /// アイテム番号の直接指定 (自動採番の逆コマンド・個別編集)。
    /// item_noがnullなら番号を消す (バルーンも消える)。
    SetPanelItemNumbers {
        sheet_id: SheetId,
        numbers: Vec<PanelItemNumber>,
    },
    /// 端子台1台の端子番号の自動採番 (spec §1.1)。ピン順に開始番号・増分・接頭辞で
    /// 振り直す (`attrs["terminal_numbers"]`)。多段端子台は段記号を保って極だけ採番する
    /// (`11a`,`11b`,…)。同じコマンドで`attrs["jumpers"]`・`attrs["pin_groups"]`の
    /// 端子番号も新しい番号へ書き換える。逆コマンドは採番前の番号 (`SetTerminalNumbers`)。
    RenumberTerminals {
        sheet_id: SheetId,
        entity_id: EntityId,
        #[serde(default = "crate::terminal_chart::default_terminal_start")]
        start: u32,
        #[serde(default = "crate::terminal_chart::default_terminal_step")]
        step: u32,
        #[serde(default)]
        prefix: String,
    },
    /// 端子台1台の端子番号の直接指定 (自動採番の逆コマンド・個別編集)。
    /// `numbers`はピン順 ([`crate::terminal_chart::terminal_numbers_of`]と同じ並び) のラベル。
    /// 個数がピン数と合わない・重複がある・空要素があれば図面を変えずにエラーにする。
    /// `None`ならシンボル定義の既定 (`1`,`2`,…) へ戻す。`RenumberTerminals`と同様、
    /// `attrs["jumpers"]`・`attrs["pin_groups"]`の端子番号も同じコマンドで書き換える。
    SetTerminalNumbers {
        sheet_id: SheetId,
        entity_id: EntityId,
        #[serde(default)]
        numbers: Option<Vec<String>>,
    },
    /// 端子台1台の段数を変える (spec §1.1「多段端子の生成・解除」、Task 3)。
    /// `symbol_id`を`terminal_block_{n}p` <-> `terminal_block_{n}p_{levels}l`へ書き換える
    /// ([`crate::terminal_chart::multilevel_id`]。極数は今のidから読み取る)。同じコマンドで
    /// `attrs["terminal_numbers"]`・`attrs["jumpers"]`・`attrs["pin_groups"]`の段記号も
    /// 新しい段数へ追従させる (段を増やすときは極ごとに段a相当のラベルを土台に段記号を
    /// 付け足す。段を減らすときは残る段のラベルだけにする)。
    ///
    /// `levels`が1〜3の範囲外、`entity_id`が端子台でない、または段を減らして**消える段に
    /// 結線・ジャンパ・ピングループ・付属品の指定が残っている**場合は、図面を一切変えずに
    /// エラーを返す。逆コマンドは変更前の段数へ戻す同じコマンド。
    SetTerminalLevels {
        sheet_id: SheetId,
        entity_id: EntityId,
        levels: usize,
    },
    /// 端子台1台に予備端子を挿入する (spec §1.1「予備端子の挿入」、Task 4)。物理的には
    /// シンボルの**末尾**に`count`極を足す (多段なら1極につき段数ぶんの端子。
    /// [`crate::terminal_chart::insert_spare_terminals`])。**番号だけ**が`after`
    /// (表示ラベル。`None`なら末尾) の直後に挿入されたように振られる。既存の極は`at`の
    /// 平行移動でシート上の座標に留まるので ([`crate::symbol::pole_origin_shift`])、
    /// 極数が増えても結線は外れない。位置は**チャートに出る順**で数えるので、挿入済みの
    /// 端子台へもう1つ挿入しても見た目どおりの位置に入る。
    ///
    /// `count`が0、極数の上限[`crate::symbol::DYNAMIC_PIN_MAX`]を超える、`entity_id`が
    /// 端子台でない、`after`が今の端子番号のどれとも一致しない、番号を決められない、または
    /// 出来上がる番号が重複する場合は図面を一切変えずにエラーを返す。逆コマンドは挿入前の
    /// インスタンスへ戻す`UpdateEntity` (symbol_id・terminal_numbers・atが1回のundoで
    /// そろって戻る)。
    InsertSpareTerminals {
        sheet_id: SheetId,
        entity_id: EntityId,
        #[serde(default)]
        after: Option<String>,
        count: usize,
        /// 番号の扱い (デザイン`eTFE5`の「番号の扱い」ラジオ)。`true`=詰める
        /// (`after`より後ろの端子の番号を繰り下げて番号を連続させる)、`false`=空ける
        /// (既存の番号は変えず、予備が空き番号を取る。空きが無ければ拒否)。
        /// 省略時は`true` (繰り下げ。このコマンドが最初から持っていた挙動)。
        #[serde(default = "crate::terminal_chart::default_renumber_following")]
        renumber_following: bool,
    },
    /// プロジェクト名・番号と、全シート共通の表題欄の供給元となる項目の一括設定 (G16)。
    /// 各欄は省略 (キーごと書かない) すれば変更しない。`name`/`no`はプロジェクト単位、
    /// `company`/`drawing_no`/`title`は**全シートの表題欄**へ同じ値を書く
    /// (1コマンドでundo一発)。表題欄はシートごとに異なる値を持てるので、逆コマンドは
    /// シートごとの元の値を個別に復元する。
    SetProjectMeta {
        #[serde(default)]
        name: Option<String>,
        /// プロジェクト番号 ([`crate::model::Project::no`]) は**3状態**:
        /// キーを書かなければ変更しない / `null`なら未設定へ戻す (番号を消す) /
        /// 文字列ならその値にする。逆コマンドも同じ3状態で元の状態を正確に復元する。
        #[serde(
            default,
            deserialize_with = "double_option",
            skip_serializing_if = "Option::is_none"
        )]
        #[schemars(with = "Option<String>")]
        no: Option<Option<String>>,
        #[serde(default)]
        company: Option<String>,
        #[serde(default)]
        drawing_no: Option<String>,
        #[serde(default)]
        title: Option<String>,
    },
    /// FreeCADオブジェクトとの対応付けを登録・更新する (entity_idがキー。M5-2)。
    SetMechLink {
        link: MechLink,
    },
    /// FreeCADオブジェクトとの対応付けを外す。
    RemoveMechLink {
        entity_id: EntityId,
    },
    /// 配線の長さをまとめて書き換える (FreeCADの経路計測の書き戻し。出所も一緒に記録)。
    /// 逆コマンドは書き換え前の長さと出所。
    SetWireLengths {
        sheet_id: SheetId,
        lengths: Vec<WireLength>,
    },
}

/// 1本の配線の長さと出所 (`set_wire_lengths` コマンドの要素)。length_mがnullなら長さを消す。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WireLength {
    pub wire_id: EntityId,
    #[serde(default)]
    pub length_m: Option<f64>,
    #[serde(default)]
    pub source: LengthSource,
}

/// 「キーが無い」と「`null`」を区別して読む (`Option<Option<T>>`)。serdeの既定では
/// どちらも`None`になってしまうので、**変更しない**と**未設定へ戻す**を区別する欄に使う。
fn double_option<'de, T, D>(de: D) -> std::result::Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(de).map(Some)
}

/// エンジンからフロントエンドへ通知する差分。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Patch {
    /// 単調増加のドキュメント世代番号。
    pub revision: u64,
    pub ops: Vec<PatchOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PatchOp {
    /// プロジェクト全体の置換(新規作成・ファイル読込時)。
    ProjectReplaced { project: Project },
    SheetAdded { index: usize, sheet: Sheet },
    SheetRemoved { sheet_id: SheetId },
    /// シートのメタ情報(名前・表題欄・改訂欄)更新。entitiesは含まない。
    SheetMetaUpdated { sheet: Sheet },
    EntityUpserted { sheet_id: SheetId, entity: Entity },
    EntityRemoved { sheet_id: SheetId, id: EntityId },
    WirePartsReplaced { wire_parts: Vec<WirePart> },
    /// PLC I/O割付表が置き換わった。
    PlcAssignmentsReplaced { assignments: Vec<PlcAssignment> },
    /// 図枠テンプレートが差し替わった (全シートの図枠を描き直す)。
    FrameTemplateChanged { frame_template: Option<String> },
    /// PLC I/O図面の生成設定 (プロジェクトに保存したもの) が変わった。
    PlcSheetOptionsChanged { options: Option<crate::plc::PlcSheetOptions> },
    /// プロジェクト名・番号が変わった (G16)。表題欄側の変更は各シートの
    /// [`PatchOp::SheetMetaUpdated`]で別に届く。
    ProjectMetaChanged { name: String, no: Option<String> },
    /// FreeCAD対応付けの一覧が置き換わった (1件の登録・解除でも全体を送る)。
    MechLinksReplaced { mech_links: Vec<MechLink> },
}

/// 編集の由来(誰の操作か)。undo履歴の各エントリに記録する。
///
/// ターン巻き戻し([`Engine::revert_range`])が「エージェントの編集だけ」を戻し、
/// 間に挟まったユーザーの手編集を保持するために使う。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EditOrigin {
    /// UI操作(Tauri IPC)。既定
    #[default]
    User,
    /// 内蔵AIエージェントがターン実行中に行った編集
    Agent,
    /// 外部MCPクライアント・Link API・madake CLIからの編集
    Mcp,
}

/// [`Engine::revert_range`]の結果。
#[derive(Debug, Clone)]
pub struct Reverted {
    /// 逆適用した履歴エントリ数(= 巻き戻した編集コマンド数)
    pub commands: usize,
    /// 巻き戻しで生じた差分
    pub patch: Patch,
}

struct HistoryEntry {
    /// 適用したコマンド列(通常は1件。巻き戻しのように1操作=複数コマンドの場合もある)
    forward: Vec<Command>,
    inverse: Vec<Command>,
    origin: EditOrigin,
}

/// ドキュメントの単一の真実。全編集はexecute()を通る。
pub struct Engine {
    project: Project,
    revision: u64,
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
}

impl Engine {
    pub fn new(project: Project) -> Self {
        Self {
            project,
            revision: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// プロジェクトを丸ごと置換(新規作成・読込)。履歴はクリアされる。
    pub fn replace_project(&mut self, project: Project) -> Patch {
        self.project = project;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.revision += 1;
        Patch {
            revision: self.revision,
            ops: vec![PatchOp::ProjectReplaced {
                project: self.project.clone(),
            }],
        }
    }

    /// コマンドを実行し、undo履歴に積む(ユーザー操作として記録する)。
    pub fn execute(&mut self, cmd: Command) -> Result<Patch> {
        self.execute_as(cmd, EditOrigin::User)
    }

    /// 由来を明示してコマンドを実行する。
    ///
    /// 由来は履歴エントリに残り、[`Self::revert_range`]が対象を絞るのに使う。
    /// 呼び出し経路ごとに: UI(Tauri IPC)=[`EditOrigin::User`]、エージェントの
    /// ターン実行中=[`EditOrigin::Agent`]、外部MCP/Link API/CLI=[`EditOrigin::Mcp`]。
    ///
    /// **何も変わらなかった編集は履歴に積まない**(同じ値の書き戻し・同じ番号の再採番
    /// など。patchもrevisionも動かない)。空の段を積むとundoが1回空振りし、redo履歴まで
    /// 消えてしまうため。
    pub fn execute_as(&mut self, cmd: Command, origin: EditOrigin) -> Result<Patch> {
        let (ops, inverse) = self.apply(&cmd)?;
        if ops.is_empty() && inverse.is_empty() {
            return Ok(Patch {
                revision: self.revision,
                ops,
            });
        }
        self.undo_stack.push(HistoryEntry {
            forward: vec![cmd],
            inverse,
            origin,
        });
        self.redo_stack.clear();
        self.revision += 1;
        Ok(Patch {
            revision: self.revision,
            ops,
        })
    }

    /// コマンド列を**まとめて1回の編集**として実行する(履歴エントリは1件)。
    ///
    /// テンプレートの適用のように「1操作で図面へ一式を入れる」編集に使う。
    /// [`Self::execute`]をN回呼ぶとundoもN回必要になるが、これなら**undo一発**で
    /// 全体が元へ戻る。
    ///
    /// 途中のコマンドが失敗したら適用前の図面へ戻して`Err`(部分適用しない・
    /// 履歴も積まない)。空のコマンド列は何もしない(revisionも進めない)。
    pub fn execute_batch(&mut self, cmds: Vec<Command>, origin: EditOrigin) -> Result<Patch> {
        if cmds.is_empty() {
            return Ok(Patch {
                revision: self.revision,
                ops: Vec::new(),
            });
        }
        let (ops, inverse) = self.apply_all(&cmds)?;
        self.undo_stack.push(HistoryEntry {
            forward: cmds,
            inverse,
            origin,
        });
        self.redo_stack.clear();
        self.revision += 1;
        Ok(Patch {
            revision: self.revision,
            ops,
        })
    }

    /// undo履歴に積まれている編集の由来を古い順に返す(検査・テスト用)。
    pub fn history_origins(&self) -> Vec<EditOrigin> {
        self.undo_stack.iter().map(|e| e.origin).collect()
    }

    /// undo深さの区間`[start_depth, end_depth)`にある`origin`由来の編集だけを
    /// 逆Commandとして適用し、巻き戻す。
    ///
    /// 履歴末尾からn回undoする方式と違い、**区間に挟まった他の由来の編集(ユーザーの
    /// 手編集)は保持する**。AIエージェントのターン単位の巻き戻しが使う。
    ///
    /// 規則:
    /// - 逆適用は新しい編集として通常の履歴に積まれる(由来は[`EditOrigin::User`]。
    ///   「元に戻す」はユーザー操作のため)。したがって**巻き戻し自体をundoで取り消せる**
    ///   一方、redo履歴は通常の編集と同じく破棄される
    /// - 逆適用が現在の図面と衝突した場合(対象が手で削除されている等)は
    ///   [`CoreError::RevertConflict`]を返し、**何も変更しない**(部分適用しない)
    /// - 区間に対象の編集が1件も無ければ`Ok(None)`(図面もrevisionも変わらない)
    pub fn revert_range(
        &mut self,
        start_depth: usize,
        end_depth: usize,
        origin: EditOrigin,
    ) -> Result<Option<Reverted>> {
        let end = end_depth.min(self.undo_stack.len());
        let start = start_depth.min(end);
        // 新しい編集から順に戻す(逆コマンドはLIFOで適用しないと前提が崩れる)
        let mut commands = 0usize;
        let mut inverses: Vec<Command> = Vec::new();
        for entry in self.undo_stack[start..end].iter().rev() {
            if entry.origin != origin {
                continue;
            }
            commands += 1;
            inverses.extend(entry.inverse.iter().cloned());
        }
        if inverses.is_empty() {
            return Ok(None);
        }
        let (ops, inverse) = self
            .apply_all(&inverses)
            .map_err(|e| CoreError::RevertConflict(e.to_string()))?;
        self.undo_stack.push(HistoryEntry {
            forward: inverses,
            inverse,
            origin: EditOrigin::User,
        });
        self.redo_stack.clear();
        self.revision += 1;
        Ok(Some(Reverted {
            commands,
            patch: Patch {
                revision: self.revision,
                ops,
            },
        }))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// undo履歴に積まれている編集の数。
    ///
    /// `revision`はundo/redoでも進むため「ある区間で何コマンド積まれたか」を測るのに
    /// 使えない。区間の前後でこの深さを比べれば、増分がそのまま巻き戻しに必要な
    /// undo回数になる(AIエージェントのターン単位undoが利用する)。
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> Result<Option<Patch>> {
        let Some(entry) = self.undo_stack.pop() else {
            return Ok(None);
        };
        let mut ops = Vec::new();
        for inv in &entry.inverse {
            let (mut o, _) = self.apply(inv)?;
            ops.append(&mut o);
        }
        self.redo_stack.push(entry);
        self.revision += 1;
        Ok(Some(Patch {
            revision: self.revision,
            ops,
        }))
    }

    pub fn redo(&mut self) -> Result<Option<Patch>> {
        let Some(entry) = self.redo_stack.pop() else {
            return Ok(None);
        };
        let (ops, inverse) = self.apply_all(&entry.forward)?;
        self.undo_stack.push(HistoryEntry {
            forward: entry.forward,
            inverse,
            origin: entry.origin,
        });
        self.revision += 1;
        Ok(Some(Patch {
            revision: self.revision,
            ops,
        }))
    }

    /// コマンド列を順に適用する。**途中で失敗したら適用前の状態へ戻して`Err`**
    /// (中途半端に適用された図面を残さない)。
    ///
    /// 戻り値は (生成patch, 逆コマンド列)。逆コマンドは適用と逆順に並べてあるので、
    /// そのまま順に適用すれば元へ戻る。
    fn apply_all(&mut self, cmds: &[Command]) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        let snapshot = self.project.clone();
        let mut ops = Vec::new();
        let mut groups: Vec<Vec<Command>> = Vec::new();
        for cmd in cmds {
            match self.apply(cmd) {
                Ok((mut o, inv)) => {
                    ops.append(&mut o);
                    groups.push(inv);
                }
                Err(e) => {
                    self.project = snapshot;
                    return Err(e);
                }
            }
        }
        groups.reverse();
        Ok((ops, groups.into_iter().flatten().collect()))
    }

    /// コマンドを適用し、(生成patch, 逆コマンド列) を返す。履歴には触れない。
    fn apply(&mut self, cmd: &Command) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        match cmd {
            Command::AddSheet {
                name,
                size,
                orientation,
                kind,
            } => {
                let sheet = Sheet::new_kind(name, *size, *orientation, *kind);
                let inverse = vec![Command::RemoveSheet { sheet_id: sheet.id }];
                let index = self.project.sheets.len();
                self.project.sheets.push(sheet.clone());
                Ok((vec![PatchOp::SheetAdded { index, sheet }], inverse))
            }
            Command::RemoveSheet { sheet_id } => {
                let index = self
                    .project
                    .sheets
                    .iter()
                    .position(|s| s.id == *sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let sheet = self.project.sheets.remove(index);
                let inverse = vec![Command::RestoreSheet {
                    index,
                    sheet: Box::new(sheet),
                }];
                Ok((vec![PatchOp::SheetRemoved { sheet_id: *sheet_id }], inverse))
            }
            Command::RestoreSheet { index, sheet } => {
                let idx = (*index).min(self.project.sheets.len());
                self.project.sheets.insert(idx, (**sheet).clone());
                let inverse = vec![Command::RemoveSheet { sheet_id: sheet.id }];
                Ok((
                    vec![PatchOp::SheetAdded {
                        index: idx,
                        sheet: (**sheet).clone(),
                    }],
                    inverse,
                ))
            }
            Command::RenameSheet { sheet_id, name } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let old = sheet.name.clone();
                sheet.name = name.clone();
                let meta = sheet_meta(sheet);
                Ok((
                    vec![PatchOp::SheetMetaUpdated { sheet: meta }],
                    vec![Command::RenameSheet {
                        sheet_id: *sheet_id,
                        name: old,
                    }],
                ))
            }
            Command::SetTitleBlock {
                sheet_id,
                title_block,
            } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let old = sheet.title_block.clone();
                sheet.title_block = title_block.clone();
                let meta = sheet_meta(sheet);
                Ok((
                    vec![PatchOp::SheetMetaUpdated { sheet: meta }],
                    vec![Command::SetTitleBlock {
                        sheet_id: *sheet_id,
                        title_block: old,
                    }],
                ))
            }
            Command::SetRevisions {
                sheet_id,
                revisions,
            } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let old = sheet.revisions.clone();
                sheet.revisions = revisions.clone();
                let meta = sheet_meta(sheet);
                Ok((
                    vec![PatchOp::SheetMetaUpdated { sheet: meta }],
                    vec![Command::SetRevisions {
                        sheet_id: *sheet_id,
                        revisions: old,
                    }],
                ))
            }
            Command::SetSheetAnnotationText { sheet_id, height_mm } => {
                if let Some(h) = height_mm {
                    if !h.is_finite()
                        || *h < MIN_ANNOTATION_TEXT_MM
                        || *h > MAX_ANNOTATION_TEXT_MM
                    {
                        return Err(CoreError::InvalidCommand(format!(
                            "annotation text height out of range: {h} (expected {MIN_ANNOTATION_TEXT_MM}..{MAX_ANNOTATION_TEXT_MM} mm)"
                        )));
                    }
                }
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let old = sheet.annotation_text_mm;
                sheet.annotation_text_mm = *height_mm;
                let meta = sheet_meta(sheet);
                Ok((
                    vec![PatchOp::SheetMetaUpdated { sheet: meta }],
                    vec![Command::SetSheetAnnotationText {
                        sheet_id: *sheet_id,
                        height_mm: old,
                    }],
                ))
            }
            Command::SetSheetPaper {
                sheet_id,
                size,
                orientation,
            } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let old_size = sheet.size;
                let old_orientation = sheet.orientation;
                sheet.size = *size;
                sheet.orientation = *orientation;
                // ゾーン分割はシートが正のまま触らない (図枠テンプレートの差し替えと同じ扱い。
                // クロスリファレンスの指すゾーン住所が用紙変更で書き換わらないように。docs/04)
                let meta = sheet_meta(sheet);
                Ok((
                    vec![PatchOp::SheetMetaUpdated { sheet: meta }],
                    vec![Command::SetSheetPaper {
                        sheet_id: *sheet_id,
                        size: old_size,
                        orientation: old_orientation,
                    }],
                ))
            }
            Command::AddEntity { sheet_id, entity } => {
                validate_entity(
                    entity,
                    self.project
                        .sheet(*sheet_id)
                        .ok_or(CoreError::SheetNotFound(*sheet_id))?,
                )?;
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let id = entity.id();
                if sheet.entities.contains_key(&id) {
                    return Err(CoreError::InvalidCommand(format!(
                        "entity already exists: {id}"
                    )));
                }
                sheet.entities.insert(id, entity.clone());
                Ok((
                    vec![PatchOp::EntityUpserted {
                        sheet_id: *sheet_id,
                        entity: entity.clone(),
                    }],
                    vec![Command::DeleteEntities {
                        sheet_id: *sheet_id,
                        ids: vec![id],
                    }],
                ))
            }
            Command::AddCable { sheet_id, cable } => {
                let sheet = self
                    .project
                    .sheet(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let mut cable = cable.clone();
                let [a, b] = cable.points.as_slice() else {
                    return Err(CoreError::InvalidCommand(
                        "a cable definition line needs exactly two points".into(),
                    ));
                };
                let crossed = crate::cable::crossed_wires(sheet, *a, *b);
                cable.cores = crossed
                    .into_iter()
                    .enumerate()
                    .map(|(i, (wire, color))| crate::model::CableCore {
                        core: (i + 1).to_string(),
                        color,
                        wire: Some(wire),
                    })
                    .collect();
                if cable.core_count == 0 {
                    cable.core_count = cable.cores.len().max(1);
                }
                self.apply(&Command::AddEntity {
                    sheet_id: *sheet_id,
                    entity: Entity::Cable(cable),
                })
            }
            Command::UpdateEntity { sheet_id, entity } => {
                validate_entity(
                    entity,
                    self.project
                        .sheet(*sheet_id)
                        .ok_or(CoreError::SheetNotFound(*sheet_id))?,
                )?;
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let id = entity.id();
                // 先に存在確認する: insertしてからエラーにすると、失敗したはずの
                // 更新でエンティティがシートへ紛れ込む
                if !sheet.entities.contains_key(&id) {
                    return Err(CoreError::EntityNotFound(id));
                }
                let old = sheet
                    .entities
                    .insert(id, entity.clone())
                    .expect("直前に存在確認済み");
                Ok((
                    vec![PatchOp::EntityUpserted {
                        sheet_id: *sheet_id,
                        entity: entity.clone(),
                    }],
                    vec![Command::UpdateEntity {
                        sheet_id: *sheet_id,
                        entity: old,
                    }],
                ))
            }
            Command::DeleteEntities { sheet_id, ids } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let mut ops = Vec::new();
                let mut inverse = Vec::new();
                for id in ids {
                    if let Some(old) = sheet.entities.remove(id) {
                        ops.push(PatchOp::EntityRemoved {
                            sheet_id: *sheet_id,
                            id: *id,
                        });
                        inverse.push(Command::AddEntity {
                            sheet_id: *sheet_id,
                            entity: old,
                        });
                    }
                }
                // 復元順は削除の逆順でなくても問題ない(idは独立)が、直感的に逆順にする。
                inverse.reverse();
                Ok((ops, inverse))
            }
            Command::MoveEntities {
                sheet_id,
                ids,
                dx,
                dy,
            } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let mut ops = Vec::new();
                let mut moved = Vec::new();
                for id in ids {
                    if let Some(e) = sheet.entities.get_mut(id) {
                        e.translate(*dx, *dy);
                        moved.push(*id);
                        ops.push(PatchOp::EntityUpserted {
                            sheet_id: *sheet_id,
                            entity: e.clone(),
                        });
                    }
                }
                Ok((
                    ops,
                    vec![Command::MoveEntities {
                        sheet_id: *sheet_id,
                        ids: moved,
                        dx: -dx,
                        dy: -dy,
                    }],
                ))
            }
            Command::SetWireParts { wire_parts } => {
                let old = self.project.wire_parts.clone();
                self.project.wire_parts = wire_parts.clone();
                Ok((
                    vec![PatchOp::WirePartsReplaced {
                        wire_parts: wire_parts.clone(),
                    }],
                    vec![Command::SetWireParts { wire_parts: old }],
                ))
            }
            Command::SetPlcAssignments { assignments } => {
                let old = self.project.plc_assignments.clone();
                self.project.plc_assignments = assignments.clone();
                Ok((
                    vec![PatchOp::PlcAssignmentsReplaced {
                        assignments: assignments.clone(),
                    }],
                    vec![Command::SetPlcAssignments { assignments: old }],
                ))
            }
            Command::SetPlcSheetOptions { options } => {
                let old = self.project.plc_generate.clone();
                self.project.plc_generate = options.clone();
                Ok((
                    vec![PatchOp::PlcSheetOptionsChanged { options: options.clone() }],
                    vec![Command::SetPlcSheetOptions { options: old }],
                ))
            }
            Command::SetFrameTemplate { frame_template } => {
                let old = self.project.frame_template.clone();
                self.project.frame_template = frame_template.clone();
                Ok((
                    vec![PatchOp::FrameTemplateChanged {
                        frame_template: frame_template.clone(),
                    }],
                    vec![Command::SetFrameTemplate {
                        frame_template: old,
                    }],
                ))
            }
            Command::RenumberWires {
                sheet_id,
                mode,
                start,
            } => {
                let targets: Vec<SheetId> = match sheet_id {
                    Some(id) => {
                        if self.project.sheet(*id).is_none() {
                            return Err(CoreError::SheetNotFound(*id));
                        }
                        vec![*id]
                    }
                    None => self.project.sheets.iter().map(|s| s.id).collect(),
                };
                // 1周目: 対象シート全体で保持される名前・線番を予約 (図面全体で番号が重複しない)
                let mut numberer = Numberer::new(*start);
                for id in &targets {
                    let sheet = self.project.sheet(*id).expect("checked above");
                    reserve_sheet(sheet, *mode, &mut numberer);
                }
                // 2周目: シート順に採番して書き込む
                let mut ops = Vec::new();
                let mut inverse = Vec::new();
                for id in &targets {
                    let sheet = self.project.sheet(*id).expect("checked above");
                    let plan = plan_sheet(sheet, *mode, &mut numberer);
                    let numbers: Vec<WireNumber> = plan
                        .into_iter()
                        .map(|(wire_id, number)| WireNumber {
                            wire_id,
                            number: Some(number),
                        })
                        .collect();
                    let (mut o, mut inv) = self.apply_wire_numbers(*id, &numbers)?;
                    ops.append(&mut o);
                    inverse.append(&mut inv);
                }
                Ok((ops, inverse))
            }
            Command::SetWireNumbers { sheet_id, numbers } => {
                self.apply_wire_numbers(*sheet_id, numbers)
            }
            Command::RenumberPanelItems { sheet_id } => {
                let sheet = self
                    .project
                    .sheet(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let numbers: Vec<PanelItemNumber> = crate::panel::renumber_plan(sheet)
                    .into_iter()
                    .map(|(footprint_id, item_no)| PanelItemNumber {
                        footprint_id,
                        item_no,
                    })
                    .collect();
                self.apply_panel_item_numbers(*sheet_id, &numbers)
            }
            Command::SetPanelItemNumbers { sheet_id, numbers } => {
                self.apply_panel_item_numbers(*sheet_id, numbers)
            }
            Command::RenumberTerminals { sheet_id, entity_id, start, step, prefix } => {
                let sheet = self.project.sheet(*sheet_id).ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let Some(Entity::Symbol(inst)) = sheet.entities.get(entity_id) else {
                    return Err(CoreError::EntityNotFound(*entity_id));
                };
                let def = crate::symbol::resolve_symbol(&inst.symbol_id)
                    .ok_or_else(|| CoreError::SymbolNotFound(inst.symbol_id.clone()))?;
                require_terminal_block(&inst.symbol_id)?;
                let raw = crate::terminal_chart::terminal_numbers_of(&def);
                let plan = crate::terminal_chart::terminal_renumber_plan(&raw, *start, *step, prefix);
                self.apply_terminal_numbers(*sheet_id, *entity_id, Some(plan))
            }
            Command::SetTerminalNumbers { sheet_id, entity_id, numbers } => {
                self.apply_terminal_numbers(*sheet_id, *entity_id, numbers.clone())
            }
            Command::SetTerminalLevels { sheet_id, entity_id, levels } => {
                self.apply_terminal_levels(*sheet_id, *entity_id, *levels)
            }
            Command::InsertSpareTerminals { sheet_id, entity_id, after, count, renumber_following } => self
                .apply_insert_spare_terminals(
                    *sheet_id,
                    *entity_id,
                    after.as_deref(),
                    *count,
                    *renumber_following,
                ),
            Command::SetProjectMeta {
                name,
                no,
                company,
                drawing_no,
                title,
            } => self.apply_project_meta(name, no, company, drawing_no, title),
            Command::SetMechLink { link } => {
                let previous = self
                    .project
                    .mech_links
                    .iter()
                    .position(|l| l.entity_id == link.entity_id)
                    .map(|i| self.project.mech_links.remove(i));
                self.project.mech_links.push(link.clone());
                let inverse = match previous {
                    Some(old) => Command::SetMechLink { link: old },
                    None => Command::RemoveMechLink {
                        entity_id: link.entity_id,
                    },
                };
                Ok((
                    vec![PatchOp::MechLinksReplaced {
                        mech_links: self.project.mech_links.clone(),
                    }],
                    vec![inverse],
                ))
            }
            Command::RemoveMechLink { entity_id } => {
                let index = self
                    .project
                    .mech_links
                    .iter()
                    .position(|l| l.entity_id == *entity_id)
                    .ok_or(CoreError::EntityNotFound(*entity_id))?;
                let old = self.project.mech_links.remove(index);
                Ok((
                    vec![PatchOp::MechLinksReplaced {
                        mech_links: self.project.mech_links.clone(),
                    }],
                    vec![Command::SetMechLink { link: old }],
                ))
            }
            Command::SetWireLengths { sheet_id, lengths } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                // 先に**全ての**wire_idを検査する。1件でも配線として見つからなければ、
                // 何も書き換えずに`Err`を返す(全か無か)。書き換えたあとで失敗すると、
                // その編集は履歴に積まれないのでundoで戻せない図面が残ってしまう。
                for WireLength { wire_id, .. } in lengths {
                    if !matches!(sheet.entities.get(wire_id), Some(Entity::Wire(_))) {
                        return Err(CoreError::EntityNotFound(*wire_id));
                    }
                }
                let mut ops = Vec::new();
                let mut old = Vec::new();
                for WireLength { wire_id, length_m, source } in lengths {
                    // 上の検査で配線であることを確認済み。
                    let Some(Entity::Wire(w)) = sheet.entities.get_mut(wire_id) else {
                        continue;
                    };
                    if w.length_m == *length_m && w.length_source == *source {
                        continue;
                    }
                    old.push(WireLength {
                        wire_id: *wire_id,
                        length_m: w.length_m,
                        source: w.length_source,
                    });
                    w.length_m = *length_m;
                    w.length_source = *source;
                    ops.push(PatchOp::EntityUpserted {
                        sheet_id: *sheet_id,
                        entity: Entity::Wire(w.clone()),
                    });
                }
                // 1本も変わらなかったら逆コマンドも空にする(無駄なundoの段を作らない)。
                let inverse = if old.is_empty() {
                    Vec::new()
                } else {
                    vec![Command::SetWireLengths {
                        sheet_id: *sheet_id,
                        lengths: old,
                    }]
                };
                Ok((ops, inverse))
            }
        }
    }

    /// [`Command::SetProjectMeta`]の適用: プロジェクト単位の欄(名前・番号)は1つの
    /// [`PatchOp::ProjectMetaChanged`]で、表題欄の欄は**シートごとに**
    /// [`PatchOp::SheetMetaUpdated`]で届く。逆コマンドも同じ形に分かれるので、
    /// undoはプロジェクト単位の値とシートごとの表題欄をそれぞれ正しい元の値へ戻す。
    fn apply_project_meta(
        &mut self,
        name: &Option<String>,
        no: &Option<Option<String>>,
        company: &Option<String>,
        drawing_no: &Option<String>,
        title: &Option<String>,
    ) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        let mut ops = Vec::new();
        let mut inverse = Vec::new();

        let mut inv_name = None;
        let mut inv_no = None;
        if let Some(n) = name {
            inv_name = Some(self.project.name.clone());
            self.project.name = n.clone();
        }
        if let Some(next) = no {
            // 「未設定」も1つの状態なので、そのまま逆コマンドへ持たせる
            // (undoで空文字にすり替わらない・nullで番号を消せる)
            inv_no = Some(self.project.no.clone());
            self.project.no = next.clone();
        }
        if inv_name.is_some() || inv_no.is_some() {
            ops.push(PatchOp::ProjectMetaChanged {
                name: self.project.name.clone(),
                no: self.project.no.clone(),
            });
            inverse.push(Command::SetProjectMeta {
                name: inv_name,
                no: inv_no,
                company: None,
                drawing_no: None,
                title: None,
            });
        }

        if company.is_some() || drawing_no.is_some() || title.is_some() {
            for sheet in &mut self.project.sheets {
                let old_tb = sheet.title_block.clone();
                let mut changed = false;
                if let Some(c) = company {
                    sheet.title_block.company = c.clone();
                    changed = true;
                }
                if let Some(d) = drawing_no {
                    sheet.title_block.drawing_no = d.clone();
                    changed = true;
                }
                if let Some(t) = title {
                    sheet.title_block.title = t.clone();
                    changed = true;
                }
                if changed {
                    ops.push(PatchOp::SheetMetaUpdated { sheet: sheet_meta(sheet) });
                    inverse.push(Command::SetTitleBlock {
                        sheet_id: sheet.id,
                        title_block: old_tb,
                    });
                }
            }
        }

        Ok((ops, inverse))
    }

    /// 指定フットプリントのアイテム番号を書き換える。値が変わるものだけをpatch・
    /// 逆コマンドに含める (同じ番号を振り直しても無駄なundoの段を作らない)。
    fn apply_panel_item_numbers(
        &mut self,
        sheet_id: SheetId,
        numbers: &[PanelItemNumber],
    ) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        let sheet = self
            .project
            .sheet_mut(sheet_id)
            .ok_or(CoreError::SheetNotFound(sheet_id))?;
        let mut ops = Vec::new();
        let mut old = Vec::new();
        for PanelItemNumber {
            footprint_id,
            item_no,
        } in numbers
        {
            let Some(Entity::Footprint(fp)) = sheet.entities.get_mut(footprint_id) else {
                continue;
            };
            if fp.item_no == *item_no {
                continue;
            }
            old.push(PanelItemNumber {
                footprint_id: *footprint_id,
                item_no: fp.item_no,
            });
            fp.item_no = *item_no;
            ops.push(PatchOp::EntityUpserted {
                sheet_id,
                entity: Entity::Footprint(fp.clone()),
            });
        }
        let inverse = if old.is_empty() {
            Vec::new()
        } else {
            vec![Command::SetPanelItemNumbers {
                sheet_id,
                numbers: old,
            }]
        };
        Ok((ops, inverse))
    }

    /// 端子台1台の端子番号 (`attrs["terminal_numbers"]`) を書き換える。同時に
    /// `attrs["jumpers"]`・`attrs["pin_groups"]`の端子番号も新しい表示ラベルへ追従させる
    /// (`RenumberTerminals`/`SetTerminalNumbers`の共通実装)。
    ///
    /// `numbers`がSomeなら個数・重複・空要素を検証し、1つでも満たさなければ図面を変えずに
    /// エラーを返す (部分適用しない)。検証・計算をすべて終えてから書き込むので、
    /// エラー時は`self.project`に一切触れない。
    fn apply_terminal_numbers(
        &mut self,
        sheet_id: SheetId,
        entity_id: EntityId,
        numbers: Option<Vec<String>>,
    ) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        let sheet = self.project.sheet(sheet_id).ok_or(CoreError::SheetNotFound(sheet_id))?;
        let Some(Entity::Symbol(inst)) = sheet.entities.get(&entity_id) else {
            return Err(CoreError::EntityNotFound(entity_id));
        };
        let def = crate::symbol::resolve_symbol(&inst.symbol_id)
            .ok_or_else(|| CoreError::SymbolNotFound(inst.symbol_id.clone()))?;
        require_terminal_block(&inst.symbol_id)?;
        let raw = crate::terminal_chart::terminal_numbers_of(&def);
        let old_display = crate::terminal_chart::terminal_numbers_of_instance(&def, &inst.attrs);
        // 「上書きが効いていたか」= キーがあり値が妥当だったか ([`terminal_numbers_override`])。
        // 展開後の値が既定と同じかどうか (`old_display != raw`) では判定しない: キーはあるが
        // 既定と同じ値を明示的に書いていた場合と、キーはあるが無効な値でフォールバックして
        // いた場合の両方を「上書きなし」と誤判定し、undoでそのキーを消してしまう (Task 3 ルーリング3a)
        let had_override = crate::terminal_chart::terminal_numbers_override(&def, &inst.attrs).is_some();

        let new_display = match &numbers {
            Some(v) => {
                if !crate::terminal_chart::valid_terminal_numbers(v, raw.len()) {
                    return Err(CoreError::InvalidCommand(format!(
                        "端子番号は{}個・重複なし・空欄なしで指定してください (要素に「,」は使えません)",
                        raw.len()
                    )));
                }
                crate::terminal_chart::trimmed_terminal_numbers(v)
            }
            None => raw.clone(),
        };

        if new_display == old_display {
            return Ok((Vec::new(), Vec::new()));
        }

        let rename: std::collections::BTreeMap<String, String> =
            old_display.iter().cloned().zip(new_display.iter().cloned()).collect();
        let new_jumpers = inst
            .attrs
            .get("jumpers")
            .map(|spec| crate::terminal_chart::rewrite_jumper_labels(spec, &rename));
        let new_pin_groups = inst
            .attrs
            .get(crate::pin_groups::PIN_GROUPS_ATTR)
            .map(|spec| rewrite_pin_group_ranges(spec, &rename));

        // ここまでは読み取りだけ。検証・計算が済んだので、ここから先だけ書き込む
        let sheet = self.project.sheet_mut(sheet_id).expect("直前に存在を確認済み");
        let Some(Entity::Symbol(inst)) = sheet.entities.get_mut(&entity_id) else {
            unreachable!("直前に存在を確認済み");
        };
        match &numbers {
            Some(_) => {
                inst.attrs.insert(
                    crate::terminal_chart::TERMINAL_NUMBERS_ATTR.to_string(),
                    new_display.join(","),
                );
            }
            None => {
                inst.attrs.remove(crate::terminal_chart::TERMINAL_NUMBERS_ATTR);
            }
        }
        if let Some(nj) = new_jumpers {
            if nj.is_empty() {
                inst.attrs.remove("jumpers");
            } else {
                inst.attrs.insert("jumpers".to_string(), nj);
            }
        }
        if let Some(npg) = new_pin_groups {
            if npg.is_empty() {
                inst.attrs.remove(crate::pin_groups::PIN_GROUPS_ATTR);
            } else {
                inst.attrs.insert(crate::pin_groups::PIN_GROUPS_ATTR.to_string(), npg);
            }
        }

        let ops = vec![PatchOp::EntityUpserted { sheet_id, entity: Entity::Symbol(inst.clone()) }];
        let inverse = vec![Command::SetTerminalNumbers {
            sheet_id,
            entity_id,
            numbers: if had_override { Some(old_display) } else { None },
        }];
        Ok((ops, inverse))
    }

    /// 端子台1台の段数 (`symbol_id`の`terminal_block_{n}p` <-> `terminal_block_{n}p_{levels}l`)
    /// を変える (Task 3 多段端子の生成・解除)。`RenumberTerminals`/`SetTerminalNumbers`と同様、
    /// 検証・計算をすべて終えてから書き込むので、エラー時は`self.project`に一切触れない。
    ///
    /// **段aのピンは用紙上の同じ座標に留まる**: 段の列は原点を中心に左右対称に並ぶので
    /// (フェーズ6のジオメトリ。[`crate::symbol::TERMINAL_LEVEL_PITCH`])、段数が変わると
    /// 段aの列の中心が動いてしまう。そのぶんだけインスタンスの`at`を逆向きに平行移動して
    /// ([`crate::symbol::level_origin_shift`] を回転・反転を通した向きで足す)、段aの端子に
    /// 繋がっていた配線が黙って別の段の端子へ繋ぎ変わったり宙に浮いたりしないようにする。
    /// 逆コマンドは同じ計算を逆向きに行うので、`symbol_id`と`at`が1回のundoでそろって戻る。
    ///
    /// 段を増やすときは常に成功する (極ごとに段aのラベルを土台に段記号を付け足すだけ)。
    /// 段を減らして**消える段に参照が残っていれば**図面を一切変えずにエラーを返す:
    /// 消えるピンの座標に何か載っていれば (配線・ネットラベル・ジャンクション・他シンボルの
    /// ピン。[`crate::netlist::point_is_attached`]) 結線ありと見なし、その段の端子番号を指す
    /// ジャンパ・ピングループ・(将来実装の)付属品指定も拒否する。`mates_with`はコネクタ一族
    /// だけが対象で端子台は最初から対象外 ([`crate::mate::is_connector_instance`]) なので、
    /// ここでは確認しない。
    fn apply_terminal_levels(
        &mut self,
        sheet_id: SheetId,
        entity_id: EntityId,
        levels: usize,
    ) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        let sheet = self.project.sheet(sheet_id).ok_or(CoreError::SheetNotFound(sheet_id))?;
        let Some(Entity::Symbol(inst)) = sheet.entities.get(&entity_id) else {
            return Err(CoreError::EntityNotFound(entity_id));
        };
        let new_id = crate::terminal_chart::multilevel_id(&inst.symbol_id, levels)
            .map_err(CoreError::InvalidCommand)?;
        if new_id == inst.symbol_id {
            // 今と同じ段数を指定したら何もしない (無駄なundoの段を作らない)
            return Ok((Vec::new(), Vec::new()));
        }
        let old_def = crate::symbol::resolve_symbol(&inst.symbol_id)
            .ok_or_else(|| CoreError::SymbolNotFound(inst.symbol_id.clone()))?;
        crate::symbol::resolve_symbol(&new_id).ok_or_else(|| CoreError::SymbolNotFound(new_id.clone()))?;

        let (poles, old_levels) = crate::terminal_chart::poles_and_levels_of(&inst.symbol_id);
        let old_display = crate::terminal_chart::terminal_numbers_of_instance(&old_def, &inst.attrs);
        let had_override = crate::terminal_chart::terminal_numbers_override(&old_def, &inst.attrs).is_some();
        let new_display =
            crate::terminal_chart::relabel_for_levels(&old_display, poles, old_levels, levels);
        // 書き込む前に必ず検証する (Task 3再レビューの持ち越し): relabel_for_levelsが土台を
        // 取り違えて2つの極が同じラベルに潰れるような壊れた入力でも、無効な番号列を黙って
        // 書き込まず図面を変えずに拒否する (SetTerminalNumbers/InsertSpareTerminalsと同じ門)
        if !crate::terminal_chart::valid_terminal_numbers(&new_display, poles * levels) {
            return Err(CoreError::InvalidCommand(
                "段数を変えると端子番号が重複します (先に端子番号の上書きを直してください)".to_string(),
            ));
        }

        // 極→段の同じ位置 (段a・段bなど、両方に残る段) だけの旧ラベル→新ラベル対応。
        // ジャンパ・ピングループの書き換えに使う (rewrite_jumper_labels/rewrite_pin_group_ranges
        // は知らないラベルの項目をそのまま残す規則なので、消える段のラベルは含めない=
        // 下の検証を通っていれば触れないし、通っていなければそもそもここへ来ない)
        let kept = old_levels.min(levels);
        let mut rename: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        for p in 0..poles {
            for j in 0..kept {
                let old_label = old_display[p * old_levels + j].clone();
                let new_label = new_display[p * levels + j].clone();
                if old_label != new_label {
                    rename.insert(old_label, new_label);
                }
            }
        }

        if levels < old_levels {
            // 消える段の端子番号 (旧表示ラベル) の集合
            let vanishing: std::collections::BTreeSet<String> = (0..poles)
                .flat_map(|p| (levels..old_levels).map(move |j| p * old_levels + j))
                .map(|idx| old_display[idx].clone())
                .collect();

            // 結線の判定は**図面から消えるピンの座標**を見る。チャートの`spare`は配線だけを
            // 数えるので、ネットラベル・ジャンクション・他シンボルのピンだけが載っている端子を
            // 予備と見てしまう。チャートは取れないほうが異常なので、黙って素通りさせず落とす
            // (この guard が段を減らすときの唯一の結線の門なので、抜けを作らない)
            let chart = crate::terminal_chart::terminal_chart(sheet, entity_id).ok_or_else(|| {
                CoreError::InvalidCommand(
                    "端子台チャートを組み立てられないため段数を変更できません".to_string(),
                )
            })?;
            let defs = crate::symbol::sheet_symbol_defs(sheet);
            let labels = crate::terminal_chart::TerminalLabels::of(&old_def, &inst.attrs);
            let pin_at = crate::netlist::pin_positions(inst, &old_def);
            for label in &vanishing {
                let row = chart.rows.iter().find(|r| &r.terminal == label);
                let attached = pin_at.iter().any(|(no, pos)| {
                    labels.display_of(no) == *label
                        && crate::netlist::point_is_attached(sheet, &defs, pos, Some(entity_id))
                });
                if !attached && !row.is_some_and(|r| !r.spare) {
                    continue;
                }
                let target = row
                    .map(|r| {
                        [r.internal.as_str(), r.external.as_str()]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" / ")
                    })
                    .unwrap_or_default();
                let where_ = if target.is_empty() {
                    String::new()
                } else {
                    format!(" ({target})")
                };
                return Err(CoreError::InvalidCommand(format!(
                    "端子{label}{where_}に結線が残っているため段を減らせません (先にその結線を外してください)"
                )));
            }

            if let Some(spec) = inst.attrs.get("jumpers") {
                let raw_terminals = crate::terminal_chart::terminal_numbers_of(&old_def);
                let jumpers = crate::terminal_chart::parse_jumpers(spec, &old_display, &raw_terminals);
                if let Some((a, b)) =
                    jumpers.pairs.iter().find(|(a, b)| vanishing.contains(a) || vanishing.contains(b))
                {
                    return Err(CoreError::InvalidCommand(format!(
                        "ジャンパ{a}-{b}が消える段を指しているため段を減らせません (先にそのジャンパを外してください)"
                    )));
                }
            }

            if let Some(spec) = inst.attrs.get(crate::pin_groups::PIN_GROUPS_ATTR) {
                let pg_order = crate::pin_groups::chart_order_pin_numbers(&old_def);
                let labels = crate::terminal_chart::TerminalLabels::of(&old_def, &inst.attrs);
                let display_order: Vec<String> = pg_order.iter().map(|n| labels.display_of(n)).collect();
                let parsed = crate::pin_groups::parse_pin_groups(spec, &display_order);
                let index_of = |label: &str| display_order.iter().position(|p| p == label);
                if let Some(group) = parsed.groups.iter().find(|g| match (index_of(&g.first), index_of(&g.last)) {
                    (Some(a), Some(b)) => {
                        let (lo, hi) = (a.min(b), a.max(b));
                        display_order[lo..=hi].iter().any(|label| vanishing.contains(label))
                    }
                    _ => vanishing.contains(&g.first) || vanishing.contains(&g.last),
                }) {
                    return Err(CoreError::InvalidCommand(format!(
                        "ピングループ{}が消える段の端子を含むため段を減らせません (先にそのピングループを外してください)",
                        group.range_text
                    )));
                }
            }

            // 付属品 (spec §1.1・Task 5で実装予定)。属性キーはまだ書き込み口が無いが、
            // 直接attrsへ入れられた指定 (MCP/Link API経由) が消える段を指していないかは
            // 先に確認しておく (Task 5が実装したときに同じ検証を素通りできるように)
            if let Some(spec) = inst.attrs.get("accessories") {
                let refers_vanishing = spec.split(',').any(|item| {
                    let label = item.split(':').next().unwrap_or("").trim();
                    label != "end" && vanishing.contains(label)
                });
                if refers_vanishing {
                    return Err(CoreError::InvalidCommand(
                        "付属品指定が消える段の端子を指しているため段を減らせません (先にその指定を外してください)"
                            .to_string(),
                    ));
                }
            }
        }

        // ここまでは読み取りだけ。検証が済んだので、ここから先だけ書き込む
        let new_jumpers = inst.attrs.get("jumpers").map(|spec| crate::terminal_chart::rewrite_jumper_labels(spec, &rename));
        let new_pin_groups =
            inst.attrs.get(crate::pin_groups::PIN_GROUPS_ATTR).map(|spec| rewrite_pin_group_ranges(spec, &rename));
        let new_accessories = inst
            .attrs
            .get("accessories")
            .map(|spec| crate::terminal_chart::rewrite_accessory_labels(spec, &rename));
        // 段aのピンを用紙上の同じ座標に留めるための平行移動 (回転・反転を通した向きで)
        let shift = crate::netlist::rotate_local(
            crate::geometry::Point::new(crate::symbol::level_origin_shift(old_levels, levels), 0.0),
            inst.rotation,
            inst.mirror,
        );

        let sheet = self.project.sheet_mut(sheet_id).expect("直前に存在を確認済み");
        let Some(Entity::Symbol(inst)) = sheet.entities.get_mut(&entity_id) else {
            unreachable!("直前に存在を確認済み");
        };
        inst.symbol_id = new_id;
        inst.at = crate::geometry::Point::new(inst.at.x + shift.x, inst.at.y + shift.y);
        // 値が無効な`terminal_numbers`キー (個数不一致など) はそのまま残す: `had_override`は
        // 「キーがあり、かつ値が妥当」なので、ここで消してしまうと逆コマンドが復元できない
        // (undoで元のバイト列へ戻らない)。キーが無ければ書かないだけで足りる
        if had_override {
            inst.attrs.insert(
                crate::terminal_chart::TERMINAL_NUMBERS_ATTR.to_string(),
                new_display.join(","),
            );
        }
        if let Some(nj) = new_jumpers {
            if nj.is_empty() {
                inst.attrs.remove("jumpers");
            } else {
                inst.attrs.insert("jumpers".to_string(), nj);
            }
        }
        if let Some(npg) = new_pin_groups {
            if npg.is_empty() {
                inst.attrs.remove(crate::pin_groups::PIN_GROUPS_ATTR);
            } else {
                inst.attrs.insert(crate::pin_groups::PIN_GROUPS_ATTR.to_string(), npg);
            }
        }
        if let Some(na) = new_accessories {
            if na.is_empty() {
                inst.attrs.remove("accessories");
            } else {
                inst.attrs.insert("accessories".to_string(), na);
            }
        }

        let ops = vec![PatchOp::EntityUpserted { sheet_id, entity: Entity::Symbol(inst.clone()) }];
        let inverse = vec![Command::SetTerminalLevels { sheet_id, entity_id, levels: old_levels }];
        Ok((ops, inverse))
    }

    /// [`Command::InsertSpareTerminals`]の実装 (Task 4)。検証・計算
    /// ([`crate::terminal_chart::insert_spare_terminals`]) をすべて終えてから書き込むので、
    /// エラー時は`self.project`に一切触れない。
    ///
    /// 「詰める」指定では`after`より後ろの極の番号が繰り下がる (表示ラベルが変わる) ので、
    /// `RenumberTerminals`/`SetTerminalLevels`と同じく`attrs["jumpers"]`・ピングループ・
    /// 付属品指定もその変わったラベルへ追従させる (古いラベルのまま残すと、テキストは同じなのに
    /// 指す端子が変わってしまう)。「空ける」指定では既存のラベルが1つも変わらないので、
    /// 追従の対応表は空になり指定はそのまま残る。新しく増える予備極は追従の対象にならない
    /// (旧ラベルが無いため)。
    ///
    /// 逆コマンドは挿入前のインスタンスへ丸ごと戻す`UpdateEntity`にする
    /// (`SetTerminalLevels`のような対称なコマンドを新たに作らなくても、symbol_id・
    /// terminal_numbers・jumpers・atが1回のundoでそろって戻る。`UpdateEntity`自身がその
    /// 逆コマンドを「置換前のエンティティへのUpdateEntity」にする既存の仕組みと同じ形)。
    fn apply_insert_spare_terminals(
        &mut self,
        sheet_id: SheetId,
        entity_id: EntityId,
        after: Option<&str>,
        count: usize,
        renumber_following: bool,
    ) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        let sheet = self.project.sheet(sheet_id).ok_or(CoreError::SheetNotFound(sheet_id))?;
        let Some(Entity::Symbol(inst)) = sheet.entities.get(&entity_id) else {
            return Err(CoreError::EntityNotFound(entity_id));
        };
        let (new_id, new_display) =
            crate::terminal_chart::insert_spare_terminals(inst, after, count, renumber_following)
                .map_err(CoreError::InvalidCommand)?;
        let new_def = crate::symbol::resolve_symbol(&new_id)
            .ok_or_else(|| CoreError::SymbolNotFound(new_id.clone()))?;
        let new_raw = crate::terminal_chart::terminal_numbers_of(&new_def);
        let old_def = crate::symbol::resolve_symbol(&inst.symbol_id)
            .ok_or_else(|| CoreError::SymbolNotFound(inst.symbol_id.clone()))?;
        let old_display = crate::terminal_chart::terminal_numbers_of_instance(&old_def, &inst.attrs);
        let (poles, _) = crate::terminal_chart::poles_and_levels_of(&inst.symbol_id);
        let new_poles = poles + count;
        let shift = crate::netlist::rotate_local(
            crate::geometry::Point::new(0.0, crate::symbol::pole_origin_shift(poles, new_poles)),
            inst.rotation,
            inst.mirror,
        );

        // 既存の極 (先頭poles*levels個) だけの旧ラベル→新ラベル対応。新しく増える予備極には
        // 旧ラベルが無いので含めない (rewrite_*_labels は知らないラベルの項目をそのまま
        // 残す規則なので、対応が無くても素通しされるだけで壊れない)
        let mut rename: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        for (old_label, new_label) in old_display.iter().zip(new_display.iter()) {
            if old_label != new_label {
                rename.insert(old_label.clone(), new_label.clone());
            }
        }
        let new_jumpers =
            inst.attrs.get("jumpers").map(|spec| crate::terminal_chart::rewrite_jumper_labels(spec, &rename));
        let new_pin_groups = inst
            .attrs
            .get(crate::pin_groups::PIN_GROUPS_ATTR)
            .map(|spec| rewrite_pin_group_ranges(spec, &rename));
        let new_accessories = inst
            .attrs
            .get("accessories")
            .map(|spec| crate::terminal_chart::rewrite_accessory_labels(spec, &rename));
        let old = inst.clone();

        let sheet = self.project.sheet_mut(sheet_id).expect("直前に存在を確認済み");
        let Some(Entity::Symbol(inst)) = sheet.entities.get_mut(&entity_id) else {
            unreachable!("直前に存在を確認済み");
        };
        inst.symbol_id = new_id;
        inst.at = crate::geometry::Point::new(inst.at.x + shift.x, inst.at.y + shift.y);
        if new_display == new_raw {
            inst.attrs.remove(crate::terminal_chart::TERMINAL_NUMBERS_ATTR);
        } else {
            inst.attrs.insert(
                crate::terminal_chart::TERMINAL_NUMBERS_ATTR.to_string(),
                new_display.join(","),
            );
        }
        if let Some(nj) = new_jumpers {
            if nj.is_empty() {
                inst.attrs.remove("jumpers");
            } else {
                inst.attrs.insert("jumpers".to_string(), nj);
            }
        }
        if let Some(npg) = new_pin_groups {
            if npg.is_empty() {
                inst.attrs.remove(crate::pin_groups::PIN_GROUPS_ATTR);
            } else {
                inst.attrs.insert(crate::pin_groups::PIN_GROUPS_ATTR.to_string(), npg);
            }
        }
        if let Some(na) = new_accessories {
            if na.is_empty() {
                inst.attrs.remove("accessories");
            } else {
                inst.attrs.insert("accessories".to_string(), na);
            }
        }

        let ops = vec![PatchOp::EntityUpserted { sheet_id, entity: Entity::Symbol(inst.clone()) }];
        let inverse = vec![Command::UpdateEntity { sheet_id, entity: Entity::Symbol(old) }];
        Ok((ops, inverse))
    }

    /// 指定Wireの線番を書き換える。値が変わるものだけをpatch・逆コマンドに含める。
    fn apply_wire_numbers(
        &mut self,
        sheet_id: SheetId,
        numbers: &[WireNumber],
    ) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        let sheet = self
            .project
            .sheet_mut(sheet_id)
            .ok_or(CoreError::SheetNotFound(sheet_id))?;
        let mut ops = Vec::new();
        let mut old = Vec::new();
        for WireNumber { wire_id, number } in numbers {
            let Some(Entity::Wire(w)) = sheet.entities.get_mut(wire_id) else {
                continue;
            };
            if w.net == *number {
                continue;
            }
            old.push(WireNumber {
                wire_id: *wire_id,
                number: w.net.clone(),
            });
            w.net = number.clone();
            ops.push(PatchOp::EntityUpserted {
                sheet_id,
                entity: Entity::Wire(w.clone()),
            });
        }
        let inverse = if old.is_empty() {
            Vec::new()
        } else {
            vec![Command::SetWireNumbers {
                sheet_id,
                numbers: old,
            }]
        };
        Ok((ops, inverse))
    }
}

/// `attrs["pin_groups"]` (`"範囲:名前,…"`) の範囲の両端 (端子番号) を`rename`
/// (旧表示ラベル→新表示ラベル) で書き換える。名前(キャプション)には触れない。
///
/// **書き換えるのは認識できた端子番号のトークンだけ**で、区切り記号・空白・キャプションは
/// 1文字も変えない ([`crate::terminal_chart::rename_terminal_tokens`])。`rename`に無い
/// 端子番号が入った項・`:`が無い読めない項は**丸ごとそのまま残す**ので、ERCが指摘している
/// 指定が採番で消えることはなく、逆コマンドで`attrs`は元のバイト列へ戻る。
fn rewrite_pin_group_ranges(spec: &str, rename: &std::collections::BTreeMap<String, String>) -> String {
    spec.split(',')
        .map(|item| {
            let Some(colon) = item.find(':') else {
                return item.to_string();
            };
            let (range, rest) = item.split_at(colon);
            match crate::terminal_chart::rename_terminal_tokens(range, rename) {
                Some(renamed) => format!("{renamed}{rest}"),
                None => item.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// 端子番号を振れるのは端子台シンボルだけ (`RenumberTerminals`/`SetTerminalNumbers`)。
///
/// `attrs["terminal_numbers"]` はチャート・接続図・図面のピン番号テキストを差し替えるので、
/// リレーやランプへ書けてしまうと、そのシンボルのピン番号が図面上で別の文字に化ける。
/// 端子台チャート・ジャンパ・端子台チェックが揃って`is_terminal_block`で門を作っているので、
/// 書き込み口も同じ門を通す (MCP/Link API経由の指定も同じ)。
fn require_terminal_block(symbol_id: &str) -> Result<()> {
    if crate::terminal_chart::is_terminal_block(symbol_id) {
        return Ok(());
    }
    Err(CoreError::InvalidCommand(format!(
        "端子番号を振れるのは端子台シンボルだけです ({symbol_id})"
    )))
}

/// 図面へ入れる前のエンティティ検証。
///
/// 描けないもの・意味が壊れているものは**黙って受け取らず**、
/// [`CoreError::InvalidCommand`]で断る(サイレント失敗の禁止)。今のところ対象は
/// ブロックシンボル ([`crate::block`]) のピン表・外形寸法と、注記の色トークン。
/// 図面へ入れられない`symbol_id`を拒む (G17)。
///
/// **描けないシンボルを黙って受け取らない**: 知らないidや極数が範囲外のidを200 OKで
/// 受け取ると、エンティティは保存されるのに図面には何も描かれず、原因も分からない。
/// 解決先は配置と同じ [`crate::symbol::resolve_symbol`] (同梱→極数可変→ユーザーライブラリ)。
fn validate_symbol_id(id: &str) -> Result<()> {
    if crate::symbol::resolve_symbol(id).is_some() {
        return Ok(());
    }
    let reason = match crate::symbol::dynamic_family(id) {
        Some((prefix, max)) => format!(
            "極数が範囲外です (`{prefix}{{1〜{max}}}p` の形で指定してください)"
        ),
        None => "そのようなシンボルはライブラリにありません".to_string(),
    };
    Err(CoreError::InvalidCommand(format!(
        "unknown symbol_id: {id:?} ({reason})"
    )))
}

fn validate_entity(entity: &Entity, sheet: &Sheet) -> Result<()> {
    match entity {
        Entity::Cable(c) => {
            if c.points.len() != 2 {
                return Err(CoreError::InvalidCommand(
                    "a cable definition line needs exactly two points".into(),
                ));
            }
            if c.core_count == 0 {
                return Err(CoreError::InvalidCommand("a cable needs at least one core".into()));
            }
            for core in &c.cores {
                if let Some(w) = core.wire {
                    if !matches!(sheet.entities.get(&w), Some(Entity::Wire(_))) {
                        return Err(CoreError::InvalidCommand(format!(
                            "cable core {} points at a wire that is not on this sheet: {w}",
                            core.core
                        )));
                    }
                }
            }
        }
        Entity::Symbol(s) if s.symbol_id == crate::block::BLOCK_SYMBOL_ID => {
            crate::block::parse_block_spec(&s.attrs).map_err(CoreError::InvalidCommand)?;
        }
        Entity::Symbol(s) => validate_symbol_id(&s.symbol_id)?,
        Entity::Text(t) => {
            // 注記色は決められたトークン名だけ。生の色コードを黙って受け取ると、
            // 紙では読めない色の注記が図面に残ってしまう
            if let Some(color) = &t.color {
                if !crate::text_style::is_text_color_token(color) {
                    let known: Vec<&str> = crate::text_style::text_color_tokens().collect();
                    return Err(CoreError::InvalidCommand(format!(
                        "unknown text color token: {color:?} (expected one of {})",
                        known.join(", ")
                    )));
                }
            }
            // 引き出し線の対象 (G20): 同じシート上に実在するエンティティだけを指せる。
            // 存在しないIDを黙って受け取ると、注記は保存されるのに引き出し線が
            // 永遠に描かれず、原因も分からない (シンボルidの検証と同じ理由)
            if let Some(target) = t.leader_to {
                if target == t.id {
                    return Err(CoreError::InvalidCommand(
                        "leader_to cannot point to the note itself".to_string(),
                    ));
                }
                if !sheet.entities.contains_key(&target) {
                    return Err(CoreError::InvalidCommand(format!(
                        "unknown leader_to target: {target} (no such entity on this sheet)"
                    )));
                }
            }
        }
        // 図形 (G10): 頂点の数・色トークン・線幅を確かめる。生の色コードや
        // 印刷できない線幅を黙って受け取ると、紙に出せない図形が図面に残ってしまう
        Entity::Shape(s) => crate::shape::validate(s).map_err(CoreError::InvalidCommand)?,
        _ => {}
    }
    Ok(())
}

/// entitiesを除いたシートのコピー(メタ更新patch用)。
fn sheet_meta(sheet: &Sheet) -> Sheet {
    let mut meta = sheet.clone();
    meta.entities = Default::default();
    meta
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use uuid::Uuid;

    fn test_engine() -> (Engine, SheetId) {
        let project = Project::new("test");
        let sheet_id = project.sheets[0].id;
        (Engine::new(project), sheet_id)
    }

    fn sample_wire() -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(10.0, 10.0), Point::new(50.0, 10.0)],
            color: "red".into(),
            sq: 0.75,
            length_m: Some(0.4),
            length_source: Default::default(),
            part_no: None,
            net: None,
        })
    }

    /// 色トークンだけを変えた注記 (他の欄は同じ)。
    fn styled_note(color: Option<&str>) -> Entity {
        Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(20.0, 20.0),
            text: "注意".into(),
            height: 2.5,
            rotation: 0,
            anchor: TextAnchor::Start,
            color: color.map(|c| c.to_string()),
            bold: false,
            leader_to: None,
        })
    }

    /// シンボルを1個置くコマンド (idと属性だけを変える)。
    fn place(symbol_id: &str) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "X1".into(),
            value: String::new(),
            attrs: Default::default(),
        })
    }

    /// A symbol whose id is not in the library is refused instead of being stored as an invisible entity, and the sheet is left untouched.
    /// ライブラリに無いシンボルidは、描かれない幽霊エンティティとして保存されるのではなく拒否され、図面は何も変わらない。
    #[test]
    fn an_unknown_symbol_id_is_refused_and_changes_nothing() {
        let (mut engine, sheet_id) = test_engine();
        let err = engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: place("no_such_symbol"),
            })
            .expect_err("知らないシンボルidは拒否される");
        assert!(
            matches!(&err, CoreError::InvalidCommand(m)
                if m.contains("no_such_symbol") && m.contains("ライブラリ")),
            "{err:?}"
        );
        assert!(engine.project().sheet(sheet_id).unwrap().entities.is_empty());
        assert_eq!(engine.undo_depth(), 0, "履歴も積まれない");
    }

    /// A parametric symbol asked for outside its pole range (0 poles, or more than the limit) is refused with a message that says the range.
    /// 極数可変シンボルを範囲外の極数 (0極や上限超え) で頼むと、使える範囲を書いたメッセージで拒否される。
    #[test]
    fn a_pole_count_outside_the_range_is_refused() {
        for id in [
            "connector_0p",
            "connector_51p",
            "terminal_block_0p",
            "plc_di_65p",
            "plc_do_0p",
        ] {
            let (mut engine, sheet_id) = test_engine();
            let err = engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: place(id),
                })
                .expect_err("範囲外の極数は拒否される");
            assert!(
                matches!(&err, CoreError::InvalidCommand(m) if m.contains("極数が範囲外")),
                "{id}: {err:?}"
            );
            assert!(engine.project().sheet(sheet_id).unwrap().entities.is_empty());
        }
    }

    /// Every ordinary placement still goes through: bundled symbols, parametric symbols inside their range, and a block defined by its attributes.
    /// ふつうの配置はこれまでどおり通る: 同梱シンボル・範囲内の極数可変シンボル・属性で形が決まるブロック。
    #[test]
    fn valid_symbol_ids_are_still_accepted() {
        let (mut engine, sheet_id) = test_engine();
        for id in [
            "resistor",
            "relay_coil",
            "connector_3p",
            "terminal_block_8p",
            "plc_di_16p",
            "plc_do_64p",
        ] {
            engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: place(id),
                })
                .unwrap_or_else(|e| panic!("{id} が置けない: {e:?}"));
        }
        let block = crate::block::test_block(
            "CTRL1",
            "MainController",
            100.0,
            50.0,
            r#"{"left":[{"no":"IN1","name":"入力1"}]}"#,
        );
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: block,
            })
            .expect("ブロックも置ける");
        assert_eq!(engine.project().sheet(sheet_id).unwrap().entities.len(), 7);
    }

    /// Changing an existing symbol to an unknown id is refused too, so a symbol on the sheet can always be drawn.
    /// 既にある部品を知らないシンボルidへ変更するのも拒否されるので、図面の部品は常に描ける。
    #[test]
    fn changing_a_symbol_to_an_unknown_id_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let entity = place("resistor");
        let id = entity.id();
        engine
            .execute(Command::AddEntity { sheet_id, entity })
            .expect("抵抗は置ける");
        let Entity::Symbol(mut s) =
            engine.project().sheet(sheet_id).unwrap().entities[&id].clone()
        else {
            unreachable!()
        };
        s.symbol_id = "resistor_typo".into();
        let err = engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: Entity::Symbol(s),
            })
            .expect_err("知らないidへの変更は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        let Entity::Symbol(kept) = &engine.project().sheet(sheet_id).unwrap().entities[&id] else {
            unreachable!()
        };
        assert_eq!(kept.symbol_id, "resistor", "元の部品のまま");
    }

    /// A block whose pin table is broken (duplicate numbers) is refused when added, and the sheet is left untouched.
    /// ピン表が壊れている(番号が重複した)ブロックは追加時に拒否され、図面は何も変わらない。
    #[test]
    fn a_block_with_a_broken_pin_table_is_refused_and_changes_nothing() {
        let (mut engine, sheet_id) = test_engine();
        let bad = crate::block::test_block(
            "CTRL1",
            "MainController",
            100.0,
            50.0,
            r#"{"left":[{"no":"IN1"}],"right":[{"no":"IN1"}]}"#,
        );
        let err = engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: bad,
            })
            .expect_err("重複ピン番号は拒否される");
        assert!(
            matches!(&err, CoreError::InvalidCommand(m) if m.contains("duplicate")),
            "{err:?}"
        );
        assert!(
            engine.project().sheet(sheet_id).unwrap().entities.is_empty(),
            "拒否されたのに図面へ入っている"
        );
        assert_eq!(engine.undo_depth(), 0, "undo履歴に積まれている");
    }

    /// Editing a good block into a broken one is refused too, so a block on the sheet always has a drawable pin table.
    /// 正しいブロックを壊れた内容へ編集するのも拒否されるので、図面上のブロックは常に描けるピン表を持つ。
    #[test]
    fn editing_a_block_into_a_broken_one_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let good = crate::block::test_block(
            "CTRL1",
            "MainController",
            100.0,
            50.0,
            r#"{"left":[{"no":"IN1"}]}"#,
        );
        let id = good.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: good,
            })
            .expect("正しいブロックは追加できる");
        let mut broken = match engine.project().sheet(sheet_id).unwrap().entities[&id].clone() {
            Entity::Symbol(s) => s,
            _ => unreachable!(),
        };
        broken
            .attrs
            .insert("block_pins".into(), "not json".into());
        let err = engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: Entity::Symbol(broken),
            })
            .expect_err("壊れた属性への更新は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        let Entity::Symbol(kept) = &engine.project().sheet(sheet_id).unwrap().entities[&id] else {
            unreachable!()
        };
        assert_eq!(
            kept.attrs.get("block_pins").map(String::as_str),
            Some(r#"{"left":[{"no":"IN1"}]}"#),
            "拒否されたのに書き換わっている"
        );
    }

    /// A note whose colour is a name the drawing does not know (a typo, or a raw colour code) is refused instead of being stored and silently drawn in black.
    /// 図面が知らない色名 (打ち間違いや生の色コード) を付けた注記は、黙って黒で描かれるのではなく拒否される。
    #[test]
    fn a_note_with_an_unknown_colour_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        for bad in ["#ff0000", "red", "赤", "", "Accent"] {
            let err = engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: styled_note(Some(bad)),
                })
                .expect_err("知らない色は拒否される");
            assert!(matches!(err, CoreError::InvalidCommand(_)), "{bad}: {err:?}");
        }
        assert!(
            engine.project().sheet(sheet_id).unwrap().entities.is_empty(),
            "拒否された注記はシートに残らない"
        );
    }

    /// A note that uses one of the drawing's colour tokens (or no colour at all) is accepted.
    /// 図面の色トークンを使った注記 (色を指定しない注記も) は受け付けられる。
    #[test]
    fn a_note_with_a_known_colour_token_is_accepted() {
        let (mut engine, sheet_id) = test_engine();
        for good in [None, Some("default"), Some("accent"), Some("warning")] {
            engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: styled_note(good),
                })
                .unwrap_or_else(|e| panic!("{good:?} は受け付ける: {e:?}"));
        }
        assert_eq!(engine.project().sheet(sheet_id).unwrap().entities.len(), 4);
    }

    /// Changing an existing note to an unknown colour is refused too, and the note keeps the colour it had.
    /// 既にある注記を知らない色へ変える更新も拒否され、注記は元の色のままになる。
    #[test]
    fn recolouring_a_note_to_an_unknown_colour_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let good = styled_note(Some("accent"));
        let id = good.id();
        engine.execute(Command::AddEntity { sheet_id, entity: good }).unwrap();
        let Entity::Text(mut broken) =
            engine.project().sheet(sheet_id).unwrap().entities[&id].clone()
        else {
            unreachable!()
        };
        broken.color = Some("#ff0000".into());
        let err = engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: Entity::Text(broken),
            })
            .expect_err("生の色コードへの更新は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        let Entity::Text(kept) = &engine.project().sheet(sheet_id).unwrap().entities[&id] else {
            unreachable!()
        };
        assert_eq!(kept.color.as_deref(), Some("accent"), "拒否されたのに書き換わっている");
    }

    /// A note whose leader points at an entity id that does not exist on the sheet is refused instead of being stored with a leader line that never draws.
    /// シート上に存在しないIDへ引き出し線を向けた注記は、いつまでも描かれない引き出し線を持ったまま保存されるのではなく拒否される。
    #[test]
    fn a_note_with_an_unknown_leader_target_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let mut note = TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(20.0, 20.0),
            text: "対象なし".into(),
            height: 2.5,
            rotation: 0,
            anchor: TextAnchor::Start,
            color: None,
            bold: false,
            leader_to: Some(Uuid::new_v4()),
        };
        let err = engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: Entity::Text(note.clone()),
            })
            .expect_err("存在しない対象は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert!(engine.project().sheet(sheet_id).unwrap().entities.is_empty());

        // 自分自身を指すのも拒否される (長さ0の引き出し線に意味は無い)
        note.leader_to = Some(note.id);
        let err = engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: Entity::Text(note),
            })
            .expect_err("自分自身を指す引き出し線は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
    }

    /// A note can set its leader to another entity already on the sheet, and clearing the leader (setting it back to None) is undoable like any other update.
    /// 注記はシート上の他のエンティティへ引き出し先を設定でき、引き出し先を外す更新 (Noneへ戻す) も他の更新と同じくundoできる。
    #[test]
    fn a_note_can_target_an_existing_entity_and_the_leader_can_be_cleared() {
        let (mut engine, sheet_id) = test_engine();
        let target = sample_wire();
        let target_id = target.id();
        engine.execute(Command::AddEntity { sheet_id, entity: target }).unwrap();

        let note = Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(20.0, 20.0),
            text: "対象あり".into(),
            height: 2.5,
            rotation: 0,
            anchor: TextAnchor::Start,
            color: None,
            bold: false,
            leader_to: Some(target_id),
        });
        let note_id = note.id();
        engine.execute(Command::AddEntity { sheet_id, entity: note }).unwrap();
        let Entity::Text(t) = &engine.project().sheet(sheet_id).unwrap().entities[&note_id] else {
            unreachable!()
        };
        assert_eq!(t.leader_to, Some(target_id));

        let mut cleared = t.clone();
        cleared.leader_to = None;
        engine
            .execute(Command::UpdateEntity { sheet_id, entity: Entity::Text(cleared) })
            .unwrap();
        assert_eq!(
            engine.undo().unwrap().unwrap().ops.len(),
            1,
            "更新のundoも1回で戻る"
        );
        let Entity::Text(restored) = &engine.project().sheet(sheet_id).unwrap().entities[&note_id]
        else {
            unreachable!()
        };
        assert_eq!(restored.leader_to, Some(target_id), "undoで引き出し先が復元される");
    }

    /// Adding an entity can be undone (the entity disappears) and redone (it comes back).
    /// エンティティの追加はundoで消え、redoで復活する。
    #[test]
    fn add_undo_redo_entity() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();

        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: wire,
            })
            .unwrap();
        assert!(engine.project().sheets[0].entities.contains_key(&id));

        let patch = engine.undo().unwrap().unwrap();
        assert!(!engine.project().sheets[0].entities.contains_key(&id));
        assert!(matches!(patch.ops[0], PatchOp::EntityRemoved { .. }));

        engine.redo().unwrap().unwrap();
        assert!(engine.project().sheets[0].entities.contains_key(&id));
    }

    /// Moving entities shifts their coordinates; undo restores the original position exactly.
    /// 移動でエンティティの座標が動き、undoで元の位置に正確に戻る。
    #[test]
    fn move_and_undo_restores_position() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: wire,
            })
            .unwrap();
        engine
            .execute(Command::MoveEntities {
                sheet_id,
                ids: vec![id],
                dx: 5.0,
                dy: 2.5,
            })
            .unwrap();
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!(w.points[0].x, 15.0);
        engine.undo().unwrap().unwrap();
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!(w.points[0].x, 10.0);
    }

    /// Deleting several entities at once is a single undo step that restores all of them.
    /// 複数エンティティの一括削除は1回のundoで全て復元される。
    #[test]
    fn delete_multiple_and_undo() {
        let (mut engine, sheet_id) = test_engine();
        let a = sample_wire();
        let b = sample_wire();
        let (ida, idb) = (a.id(), b.id());
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: a,
            })
            .unwrap();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: b,
            })
            .unwrap();
        engine
            .execute(Command::DeleteEntities {
                sheet_id,
                ids: vec![ida, idb],
            })
            .unwrap();
        assert!(engine.project().sheets[0].entities.is_empty());
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].entities.len(), 2);
        assert!(engine.project().sheets[0].entities.contains_key(&ida));
        assert!(engine.project().sheets[0].entities.contains_key(&idb));
    }

    /// Sheets can be added and removed; undoing a removal restores the sheet with its original id and position.
    /// シートは追加・削除でき、削除のundoは元のid・位置のままシートを復元する。
    #[test]
    fn sheet_add_remove_undo() {
        let (mut engine, _) = test_engine();
        engine
            .execute(Command::AddSheet {
                name: "TB2".into(),
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
                kind: SheetKind::Schematic,
            })
            .unwrap();
        assert_eq!(engine.project().sheets.len(), 2);
        let new_id = engine.project().sheets[1].id;
        engine
            .execute(Command::RemoveSheet { sheet_id: new_id })
            .unwrap();
        assert_eq!(engine.project().sheets.len(), 1);
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets.len(), 2);
        assert_eq!(engine.project().sheets[1].id, new_id);
    }

    /// Adding a sheet without naming a kind gives an ordinary schematic sheet, and a panel-layout sheet is added by asking for the panel kind (undo removes it either way).
    /// 種別を指定しないシート追加は通常の回路図シートになり、盤レイアウトシートは種別を指定して追加する(どちらもundoで消える)。
    #[test]
    fn add_sheet_defaults_to_schematic_and_can_add_a_panel_sheet() {
        let (mut engine, _) = test_engine();
        engine
            .execute(Command::AddSheet {
                name: "回路".into(),
                size: PaperSize::A3,
                orientation: Orientation::Landscape,
                kind: SheetKind::default(),
            })
            .unwrap();
        assert_eq!(engine.project().sheets[1].kind, SheetKind::Schematic);
        engine
            .execute(Command::AddSheet {
                name: "盤レイアウト".into(),
                size: PaperSize::A3,
                orientation: Orientation::Landscape,
                kind: SheetKind::Panel,
            })
            .unwrap();
        assert_eq!(engine.project().sheets[2].kind, SheetKind::Panel);
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets.len(), 2);
    }

    /// A sheet-add command written before panel sheets existed (no kind field) still works and adds a schematic sheet.
    /// 盤シートが無かった頃に書かれたシート追加コマンド(種別の欄が無い)もそのまま動き、回路図シートを追加する。
    #[test]
    fn a_sheet_add_command_without_a_kind_still_adds_a_schematic_sheet() {
        let cmd: Command = serde_json::from_str(
            r#"{"type":"add_sheet","name":"S2","size":"A3","orientation":"Landscape"}"#,
        )
        .expect("旧いコマンドJSONが読める");
        let Command::AddSheet { kind, .. } = cmd else {
            panic!("シート追加コマンドであること")
        };
        assert_eq!(kind, SheetKind::Schematic);
    }

    fn sample_panel_entities() -> Vec<Entity> {
        vec![
            Entity::PanelOutline(PanelOutline {
                id: Uuid::new_v4(),
                at: Point::new(40.0, 40.0),
                w: 200.0,
                h: 150.0,
                name: "制御盤".into(),
            }),
            Entity::DinRail(DinRail {
                id: Uuid::new_v4(),
                y: 100.0,
                x1: 60.0,
                x2: 220.0,
            }),
            Entity::Duct(Duct {
                id: Uuid::new_v4(),
                at: Point::new(60.0, 60.0),
                w: 160.0,
                h: 20.0,
            }),
            Entity::Footprint(Footprint {
                id: Uuid::new_v4(),
                at: Point::new(80.0, 82.5),
                w: 25.0,
                h: 65.0,
                rotation: 0,
                reference: "K1".into(),
                part_no: "MDK-RLY-24V".into(),
                item_no: None,
            }),
        ]
    }

    /// The four panel entities (panel outline, DIN rail, duct, footprint) are added, moved and deleted with the ordinary entity commands, and every step can be undone.
    /// 盤エンティティ4種(盤外形・DINレール・ダクト・フットプリント)は通常のエンティティ用コマンドで追加・移動・削除でき、どの操作もundoで戻せる。
    #[test]
    fn panel_entities_are_added_moved_and_deleted_with_undo() {
        let (mut engine, sheet_id) = test_engine();
        let entities = sample_panel_entities();
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id()).collect();
        for entity in entities {
            engine
                .execute(Command::AddEntity { sheet_id, entity })
                .unwrap();
        }
        assert_eq!(engine.project().sheets[0].entities.len(), 4);

        engine
            .execute(Command::MoveEntities {
                sheet_id,
                ids: ids.clone(),
                dx: 10.0,
                dy: 5.0,
            })
            .unwrap();
        let Entity::DinRail(rail) = &engine.project().sheets[0].entities[&ids[1]] else {
            panic!("DINレールであること")
        };
        assert_eq!((rail.x1, rail.y), (70.0, 105.0));

        engine
            .execute(Command::DeleteEntities {
                sheet_id,
                ids: ids.clone(),
            })
            .unwrap();
        assert!(engine.project().sheets[0].entities.is_empty());
        engine.undo().unwrap().unwrap(); // 削除を戻す
        engine.undo().unwrap().unwrap(); // 移動を戻す
        let Entity::DinRail(rail) = &engine.project().sheets[0].entities[&ids[1]] else {
            panic!("DINレールであること")
        };
        assert_eq!((rail.x1, rail.y), (60.0, 100.0));
    }

    /// Editing a footprint (its reference, part number or rotation) replaces it wholesale, and undo brings back the previous version.
    /// フットプリントの編集(参照記号・型番・回転)は丸ごと置換で、undoで編集前に戻る。
    #[test]
    fn a_footprint_can_be_edited_and_undone() {
        let (mut engine, sheet_id) = test_engine();
        let entity = Entity::Footprint(Footprint {
            id: Uuid::new_v4(),
            at: Point::new(80.0, 80.0),
            w: 25.0,
            h: 65.0,
            rotation: 0,
            reference: "K1".into(),
            part_no: "MDK-RLY-24V".into(),
            item_no: None,
        });
        let id = entity.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: entity.clone(),
            })
            .unwrap();
        let mut edited = entity.clone();
        if let Entity::Footprint(f) = &mut edited {
            f.reference = "K2".into();
            f.rotation = 90;
        }
        engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: edited,
            })
            .unwrap();
        let Entity::Footprint(f) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!((f.reference.as_str(), f.rotation), ("K2", 90));
        engine.undo().unwrap().unwrap();
        let Entity::Footprint(f) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!((f.reference.as_str(), f.rotation), ("K1", 0));
    }

    /// Commands serialize to JSON and back without loss, so any client can send them over the wire.
    /// CommandはJSONに往復変換でき、どのクライアントからも送信できる。
    #[test]
    fn command_json_roundtrip() {
        let (_, sheet_id) = test_engine();
        let cmd = Command::AddEntity {
            sheet_id,
            entity: sample_wire(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: Command = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Command::AddEntity { .. }));
    }

    /// UpdateEntity replaces an entity wholesale; undo brings back the previous version.
    /// UpdateEntityはエンティティを丸ごと置換し、undoで置換前の状態に戻る。
    #[test]
    fn update_entity_undo_restores_previous_version() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();
        engine.execute(Command::AddEntity { sheet_id, entity: wire.clone() }).unwrap();
        let mut updated = wire.clone();
        if let Entity::Wire(w) = &mut updated { w.color = "blue".into(); }
        engine.execute(Command::UpdateEntity { sheet_id, entity: updated }).unwrap();
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&id] else { panic!() };
        assert_eq!(w.color, "blue");
        engine.undo().unwrap().unwrap();
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&id] else { panic!() };
        assert_eq!(w.color, "red");
    }

    /// SetTitleBlock updates the sheet's title block; undo restores the previous fields.
    /// SetTitleBlockはシートの表題欄を更新し、undoで以前の内容に戻る。
    #[test]
    fn set_title_block_is_undoable() {
        let (mut engine, sheet_id) = test_engine();
        let mut tb = TitleBlock::default();
        tb.title = "動力系統図".into();
        engine.execute(Command::SetTitleBlock { sheet_id, title_block: tb }).unwrap();
        assert_eq!(engine.project().sheets[0].title_block.title, "動力系統図");
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].title_block.title, "");
    }

    /// SetRevisions replaces the revision-table rows; undo restores the previous list.
    /// SetRevisionsは改訂欄の行を置き換え、undoで以前のリストに戻る。
    #[test]
    fn set_revisions_is_undoable() {
        let (mut engine, sheet_id) = test_engine();
        let rev = Revision { mark: "A".into(), date: "2026-08-21".into(), description: "初版".into(), by: "K.T".into() };
        engine.execute(Command::SetRevisions { sheet_id, revisions: vec![rev] }).unwrap();
        assert_eq!(engine.project().sheets[0].revisions.len(), 1);
        engine.undo().unwrap().unwrap();
        assert!(engine.project().sheets[0].revisions.is_empty());
    }

    /// The annotation text height of a sheet is set through a command, and undo puts the previous height back.
    /// シートの注記文字高はコマンドで設定し、undoで以前の高さに戻る。
    #[test]
    fn set_sheet_annotation_text_is_undoable() {
        let (mut engine, sheet_id) = test_engine();
        engine
            .execute(Command::SetSheetAnnotationText {
                sheet_id,
                height_mm: Some(1.4),
            })
            .unwrap();
        assert_eq!(engine.project().sheets[0].annotation_text_mm, Some(1.4));
        assert!((engine.project().sheets[0].annotation_text_mm() - 1.4).abs() < 1e-9);
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].annotation_text_mm, None, "設定前へ戻る");
    }

    /// Clearing the annotation text height with null puts the sheet back to the default 2.0mm.
    /// 注記文字高をnullで消すと、シートは既定の2.0mmに戻る。
    #[test]
    fn clearing_the_annotation_text_height_restores_the_default() {
        let (mut engine, sheet_id) = test_engine();
        for height in [Some(3.0), None] {
            engine
                .execute(Command::SetSheetAnnotationText {
                    sheet_id,
                    height_mm: height,
                })
                .unwrap();
        }
        assert_eq!(engine.project().sheets[0].annotation_text_mm, None);
        assert!((engine.project().sheets[0].annotation_text_mm() - 2.0).abs() < 1e-9);
    }

    /// An annotation text height outside 1.0-3.5mm is rejected with a reason, and the sheet is left untouched.
    /// 1.0〜3.5mmの範囲を外れた注記文字高は理由付きで拒否され、シートは書き換わらない。
    #[test]
    fn an_out_of_range_annotation_text_height_is_rejected() {
        let (mut engine, sheet_id) = test_engine();
        for bad in [0.5, 5.0, f64::NAN] {
            let err = engine
                .execute(Command::SetSheetAnnotationText {
                    sheet_id,
                    height_mm: Some(bad),
                })
                .unwrap_err();
            assert!(err.to_string().contains("annotation text height"), "{err}");
        }
        assert_eq!(engine.project().sheets[0].annotation_text_mm, None, "書き換わらない");
    }

    /// The annotation text height command is plain JSON, so AI agents and the CLI can send it.
    /// 注記文字高のコマンドは素のJSONで表現でき、AIやCLIから送れる。
    #[test]
    fn set_sheet_annotation_text_is_plain_json() {
        let (mut engine, sheet_id) = test_engine();
        let json = format!(
            r#"{{"type":"set_sheet_annotation_text","sheet_id":"{sheet_id}","height_mm":1.4}}"#
        );
        let cmd: Command = serde_json::from_str(&json).unwrap();
        engine.execute(cmd).unwrap();
        assert_eq!(engine.project().sheets[0].annotation_text_mm, Some(1.4));
    }

    /// SetPlcSheetOptions saves the PLC I/O drawing settings with the project, and undo puts back what was saved before.
    /// SetPlcSheetOptionsはPLC I/O図面の生成設定をプロジェクトに保存し、undoで保存前の状態に戻る。
    #[test]
    fn set_plc_sheet_options_saves_the_settings_and_undo_restores_them() {
        let mut engine = Engine::new(Project::new("t"));
        let options = crate::plc::PlcSheetOptions {
            ladder_style: crate::plc::LadderStyle::HorizontalBus,
            ..Default::default()
        };
        let patch = engine
            .execute(Command::SetPlcSheetOptions { options: Some(options.clone()) })
            .unwrap();
        assert!(matches!(patch.ops[0], PatchOp::PlcSheetOptionsChanged { .. }));
        assert_eq!(engine.project().plc_generate, Some(options));
        engine.undo().unwrap();
        assert_eq!(engine.project().plc_generate, None);
    }

    /// SetPlcAssignments replaces the whole PLC I/O assignment table of the project; undo restores the previous table.
    /// SetPlcAssignmentsはプロジェクトのPLC I/O割付表を丸ごと置き換え、undoで以前の表に戻る。
    #[test]
    fn set_plc_assignments_is_undoable() {
        let (mut engine, _) = test_engine();
        let row = PlcAssignment {
            id: Uuid::new_v4(),
            module_ref: "PLC1".into(),
            address: "X0".into(),
            signal_name: "起動押釦".into(),
            comment: "PB1".into(),
        };
        let patch = engine
            .execute(Command::SetPlcAssignments {
                assignments: vec![row],
            })
            .unwrap();
        assert!(matches!(
            patch.ops[0],
            PatchOp::PlcAssignmentsReplaced { .. }
        ));
        assert_eq!(engine.project().plc_assignments.len(), 1);
        assert_eq!(engine.project().plc_assignments[0].signal_name, "起動押釦");
        engine.undo().unwrap().unwrap();
        assert!(engine.project().plc_assignments.is_empty());
        engine.redo().unwrap().unwrap();
        assert_eq!(engine.project().plc_assignments.len(), 1);
    }

    /// SetFrameTemplate switches the frame template of the whole project in one edit, and undo puts the previous one back (undoing the very first switch returns to the default frame).
    /// SetFrameTemplateはプロジェクト全体の図枠テンプレートを1回の編集で差し替え、undoで元の図枠へ戻る(最初の差し替えをundoすると未指定=既定の図枠へ戻る)。
    #[test]
    fn set_frame_template_is_undoable() {
        let (mut engine, _) = test_engine();
        assert_eq!(engine.project().frame_template, None);
        let patch = engine
            .execute(Command::SetFrameTemplate {
                frame_template: Some("jis-a4-portrait".into()),
            })
            .unwrap();
        assert!(matches!(patch.ops[0], PatchOp::FrameTemplateChanged { .. }));
        assert_eq!(
            engine.project().frame_template.as_deref(),
            Some("jis-a4-portrait")
        );
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().frame_template, None, "未指定 (既定の図枠) へ戻る");
        engine.redo().unwrap().unwrap();
        assert_eq!(
            engine.project().frame_template.as_deref(),
            Some("jis-a4-portrait")
        );
    }

    /// SetFrameTemplate accepts an id nobody knows: the drawing keeps printing with the default frame, so a project made elsewhere still opens.
    /// SetFrameTemplateは知らないidも受け付ける(図面は既定の図枠で描かれ続けるので、他所で作ったプロジェクトも開ける)。
    #[test]
    fn set_frame_template_accepts_an_unknown_id() {
        let (mut engine, _) = test_engine();
        engine
            .execute(Command::SetFrameTemplate {
                frame_template: Some("our-own-frame".into()),
            })
            .expect("未知のidでも失敗しない");
        assert_eq!(
            crate::frame_template::for_project(engine.project()).id,
            crate::frame_template::DEFAULT_FRAME_TEMPLATE_ID
        );
    }

    fn sample_harness(name: &str) -> Entity {
        Entity::Harness(Harness {
            id: Uuid::new_v4(),
            points: crate::harness::rect_points(Point::new(50.0, 50.0), Point::new(150.0, 100.0)),
            name: name.into(),
            note: String::new(),
        })
    }

    /// A harness boundary is added, renamed and deleted with the ordinary entity commands, and every step can be undone.
    /// ハーネス境界は通常のエンティティ用コマンドで追加・改名・削除でき、どの操作もundoで戻せる。
    #[test]
    fn harness_add_rename_delete_are_undoable() {
        let (mut engine, sheet_id) = test_engine();
        let harness = sample_harness("W1");
        let id = harness.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: harness.clone(),
            })
            .unwrap();
        assert!(engine.project().sheets[0].entities.contains_key(&id));

        let mut renamed = harness.clone();
        if let Entity::Harness(h) = &mut renamed {
            h.name = "W2".into();
        }
        engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: renamed,
            })
            .unwrap();
        let Entity::Harness(h) = &engine.project().sheets[0].entities[&id] else {
            panic!("ハーネスであること")
        };
        assert_eq!(h.name, "W2");

        engine
            .execute(Command::DeleteEntities {
                sheet_id,
                ids: vec![id],
            })
            .unwrap();
        assert!(!engine.project().sheets[0].entities.contains_key(&id));

        engine.undo().unwrap().unwrap(); // 削除を戻す
        engine.undo().unwrap().unwrap(); // 改名を戻す
        let Entity::Harness(h) = &engine.project().sheets[0].entities[&id] else {
            panic!("ハーネスであること")
        };
        assert_eq!(h.name, "W1");
        engine.undo().unwrap().unwrap(); // 追加を戻す
        assert!(engine.project().sheets[0].entities.is_empty());
    }

    /// Moving a harness boundary shifts all of its corners, and undo puts them back exactly.
    /// ハーネス境界を移動すると4隅すべてが動き、undoで元の位置に正確に戻る。
    #[test]
    fn harness_move_and_undo_restores_every_corner() {
        let (mut engine, sheet_id) = test_engine();
        let harness = sample_harness("W1");
        let id = harness.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: harness,
            })
            .unwrap();
        engine
            .execute(Command::MoveEntities {
                sheet_id,
                ids: vec![id],
                dx: 10.0,
                dy: -5.0,
            })
            .unwrap();
        let Entity::Harness(h) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!(h.points[0], Point::new(60.0, 45.0));
        assert_eq!(h.points[2], Point::new(160.0, 95.0));
        engine.undo().unwrap().unwrap();
        let Entity::Harness(h) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!(h.points[0], Point::new(50.0, 50.0));
    }

    /// A harness boundary survives a JSON round trip with the kind tag "harness", so any client can send it.
    /// ハーネス境界は kind="harness" としてJSONに往復変換でき、どのクライアントからも送れる。
    #[test]
    fn harness_command_json_roundtrip_uses_the_harness_kind() {
        let (_, sheet_id) = test_engine();
        let cmd = Command::AddEntity {
            sheet_id,
            entity: sample_harness("W1"),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"kind\":\"harness\""), "{json}");
        let back: Command = serde_json::from_str(&json).unwrap();
        let Command::AddEntity {
            entity: Entity::Harness(h),
            ..
        } = back
        else {
            panic!("ハーネスの追加コマンドに戻ること")
        };
        assert_eq!(h.name, "W1");
        assert_eq!(h.points.len(), 4);
    }

    /// 図形1つ (G10)。既定の見た目 (既定色・細線・塗りなし・実線) で作る。
    fn sample_shape(kind: ShapeKind, points: &[(f64, f64)]) -> Entity {
        Entity::Shape(crate::shape::new_shape(
            kind,
            points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        ))
    }

    /// All four kinds of shape are added, edited and deleted with the ordinary entity commands, and every step can be undone.
    /// 4種類すべての図形が通常のエンティティ用コマンドで追加・編集・削除でき、どの操作もundoで戻せる。
    #[test]
    fn shape_add_edit_delete_are_undoable_for_every_kind() {
        for (kind, points) in [
            (ShapeKind::Rect, &[(50.0, 50.0), (150.0, 100.0)][..]),
            (ShapeKind::Line, &[(50.0, 50.0), (150.0, 50.0)][..]),
            (ShapeKind::Circle, &[(100.0, 50.0), (110.0, 50.0)][..]),
            (
                ShapeKind::Polyline,
                &[(50.0, 50.0), (100.0, 50.0), (100.0, 90.0)][..],
            ),
        ] {
            let (mut engine, sheet_id) = test_engine();
            let entity = sample_shape(kind, points);
            let id = entity.id();
            engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: entity.clone(),
                })
                .unwrap_or_else(|e| panic!("{kind:?}: {e}"));
            assert!(engine.project().sheets[0].entities.contains_key(&id), "{kind:?}");

            let mut styled = entity.clone();
            if let Entity::Shape(s) = &mut styled {
                s.stroke = "accent".into();
                s.stroke_width_mm = 0.5;
                s.dashed = true;
                s.fill = Some("muted".into());
            }
            engine
                .execute(Command::UpdateEntity {
                    sheet_id,
                    entity: styled,
                })
                .unwrap();
            let Entity::Shape(s) = &engine.project().sheets[0].entities[&id] else {
                panic!("図形であること")
            };
            assert_eq!(s.stroke, "accent");
            assert!(s.dashed);

            engine
                .execute(Command::DeleteEntities {
                    sheet_id,
                    ids: vec![id],
                })
                .unwrap();
            assert!(!engine.project().sheets[0].entities.contains_key(&id));

            engine.undo().unwrap().unwrap(); // 削除を戻す
            engine.undo().unwrap().unwrap(); // 見た目の変更を戻す
            let Entity::Shape(s) = &engine.project().sheets[0].entities[&id] else {
                panic!("図形であること")
            };
            assert_eq!(s.stroke, crate::shape::DEFAULT_SHAPE_COLOR, "{kind:?}");
            assert!(!s.dashed);
            engine.undo().unwrap().unwrap(); // 追加を戻す
            assert!(engine.project().sheets[0].entities.is_empty(), "{kind:?}");
        }
    }

    /// Moving a shape shifts every one of its points, and undo puts them back exactly.
    /// 図形を移動すると全ての頂点が動き、undoで元の位置に正確に戻る。
    #[test]
    fn shape_move_and_undo_restores_every_point() {
        let (mut engine, sheet_id) = test_engine();
        let entity = sample_shape(ShapeKind::Polyline, &[(50.0, 50.0), (100.0, 50.0), (100.0, 90.0)]);
        let id = entity.id();
        engine.execute(Command::AddEntity { sheet_id, entity }).unwrap();
        engine
            .execute(Command::MoveEntities {
                sheet_id,
                ids: vec![id],
                dx: 10.0,
                dy: -5.0,
            })
            .unwrap();
        let Entity::Shape(s) = &engine.project().sheets[0].entities[&id] else { panic!() };
        assert_eq!(s.points[0], Point::new(60.0, 45.0));
        assert_eq!(s.points[2], Point::new(110.0, 85.0));
        engine.undo().unwrap().unwrap();
        let Entity::Shape(s) = &engine.project().sheets[0].entities[&id] else { panic!() };
        assert_eq!(s.points[0], Point::new(50.0, 50.0));
        assert_eq!(s.points[2], Point::new(100.0, 90.0));
    }

    /// A shape with a raw colour code instead of a colour token is refused, and the drawing is left untouched.
    /// 色トークンではなく生の色コードを入れた図形は拒否され、図面は何も変わらない。
    #[test]
    fn a_shape_with_a_raw_colour_code_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let mut entity = sample_shape(ShapeKind::Rect, &[(0.0, 0.0), (10.0, 10.0)]);
        if let Entity::Shape(s) = &mut entity {
            s.stroke = "#ff0000".into();
        }
        let err = engine
            .execute(Command::AddEntity { sheet_id, entity })
            .expect_err("生の色コードは拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err}");
        assert!(engine.project().sheets[0].entities.is_empty(), "図面は変わらない");
    }

    /// A shape whose stroke is too thin to print or thick enough to swamp the grid is refused.
    /// 印刷できないほど細い線幅・グリッドを潰すほど太い線幅の図形は拒否される。
    #[test]
    fn a_shape_with_an_unprintable_stroke_width_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        for width in [0.0, 0.05, 2.5] {
            let mut entity = sample_shape(ShapeKind::Line, &[(0.0, 0.0), (10.0, 0.0)]);
            if let Entity::Shape(s) = &mut entity {
                s.stroke_width_mm = width;
            }
            let err = engine
                .execute(Command::AddEntity { sheet_id, entity })
                .expect_err("範囲外の線幅は拒否される");
            assert!(matches!(err, CoreError::InvalidCommand(_)), "{width}: {err}");
        }
        assert!(engine.project().sheets[0].entities.is_empty());
    }

    /// A rectangle sent with the wrong number of points is refused, so a half-drawn shape never reaches the drawing.
    /// 頂点の数が合っていない矩形は拒否されるので、描きかけの図形が図面へ入ることはない。
    #[test]
    fn a_shape_with_the_wrong_number_of_points_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let entity = sample_shape(ShapeKind::Rect, &[(0.0, 0.0)]);
        assert!(engine.execute(Command::AddEntity { sheet_id, entity }).is_err());
        assert!(engine.project().sheets[0].entities.is_empty());
    }

    /// A shape survives a JSON round trip with the kind tag "shape", so any client can send it.
    /// 図形は kind="shape" としてJSONに往復変換でき、どのクライアントからも送れる。
    #[test]
    fn shape_command_json_roundtrip_uses_the_shape_kind() {
        let (_, sheet_id) = test_engine();
        let cmd = Command::AddEntity {
            sheet_id,
            entity: sample_shape(ShapeKind::Rect, &[(0.0, 0.0), (10.0, 10.0)]),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"kind\":\"shape\""), "{json}");
        let back: Command = serde_json::from_str(&json).unwrap();
        let Command::AddEntity {
            entity: Entity::Shape(s),
            ..
        } = back
        else {
            panic!("図形の追加コマンドに戻ること")
        };
        assert_eq!(s.shape, ShapeKind::Rect);
        assert_eq!(s.points.len(), 2);
    }

    /// Every execute/undo/redo increases the document revision, so clients can discard stale patches.
    /// execute/undo/redoのたびにドキュメントrevisionが増加し、クライアントは古いpatchを破棄できる。
    #[test]
    fn revision_increases_monotonically() {
        let (mut engine, sheet_id) = test_engine();
        let r0 = engine.revision();
        let p1 = engine.execute(Command::AddEntity { sheet_id, entity: sample_wire() }).unwrap();
        let p2 = engine.undo().unwrap().unwrap();
        let p3 = engine.redo().unwrap().unwrap();
        assert!(r0 < p1.revision && p1.revision < p2.revision && p2.revision < p3.revision);
    }

    /// Commands targeting a non-existent sheet fail with an error and change nothing.
    /// 存在しないシートへのCommandはエラーになり、何も変更されない。
    #[test]
    fn unknown_sheet_is_rejected() {
        let (mut engine, _) = test_engine();
        let missing = Uuid::new_v4();
        let err = engine.execute(Command::AddEntity { sheet_id: missing, entity: sample_wire() });
        assert!(err.is_err());
        assert_eq!(engine.project().sheets[0].entities.len(), 0);
    }

    /// Undo with an empty history returns None instead of an error.
    /// 履歴が空のときのundoはエラーではなくNoneを返す。
    #[test]
    fn undo_on_empty_history_returns_none() {
        let (mut engine, _) = test_engine();
        assert!(engine.undo().unwrap().is_none());
        assert!(engine.redo().unwrap().is_none());
    }

    /// A new edit after undo clears the redo history (standard editor behavior).
    /// undo後に新しい編集をするとredo履歴は消える(一般的なエディタと同じ挙動)。
    #[test]
    fn new_edit_after_undo_clears_redo() {
        let (mut engine, sheet_id) = test_engine();
        engine.execute(Command::AddEntity { sheet_id, entity: sample_wire() }).unwrap();
        engine.undo().unwrap().unwrap();
        engine.execute(Command::AddEntity { sheet_id, entity: sample_wire() }).unwrap();
        assert!(engine.redo().unwrap().is_none());
    }

    /// Updating an entity that does not exist fails and does not sneak the new entity into the sheet.
    /// 存在しないエンティティの更新は失敗し、その要素がシートへ紛れ込むこともない。
    #[test]
    fn updating_a_missing_entity_fails_without_inserting_it() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();
        assert!(engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: wire
            })
            .is_err());
        assert!(!engine.project().sheets[0].entities.contains_key(&id));
    }

    /// Every history entry records who made the edit; plain execute() counts as a user edit.
    /// 履歴の各エントリは編集の由来(誰の編集か)を記録し、通常のexecute()はユーザー編集として扱う。
    #[test]
    fn execute_as_records_the_edit_origin() {
        let (mut engine, sheet_id) = test_engine();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: sample_wire(),
            })
            .unwrap();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: sample_wire(),
                },
                EditOrigin::Agent,
            )
            .unwrap();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: sample_wire(),
                },
                EditOrigin::Mcp,
            )
            .unwrap();
        assert_eq!(
            engine.history_origins(),
            vec![EditOrigin::User, EditOrigin::Agent, EditOrigin::Mcp]
        );
        // redoで積み直しても由来は変わらない
        engine.undo().unwrap().unwrap();
        engine.redo().unwrap().unwrap();
        assert_eq!(
            engine.history_origins(),
            vec![EditOrigin::User, EditOrigin::Agent, EditOrigin::Mcp]
        );
    }

    /// Reverting a range rolls back only the agent's edits in it and keeps the user's own edits, even when they were interleaved.
    /// 区間の巻き戻しはその中のエージェント編集だけを戻し、間に挟まったユーザー編集はそのまま残す。
    #[test]
    fn revert_range_rolls_back_agent_edits_and_keeps_user_edits() {
        let (mut engine, sheet_id) = test_engine();
        let agent_a = sample_wire();
        let user = sample_wire();
        let agent_b = sample_wire();
        let (agent_a_id, user_id, agent_b_id) = (agent_a.id(), user.id(), agent_b.id());

        let start = engine.undo_depth();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: agent_a,
                },
                EditOrigin::Agent,
            )
            .unwrap();
        // ターンの途中でユーザーが手で1本引いた
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: user,
            })
            .unwrap();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: agent_b,
                },
                EditOrigin::Agent,
            )
            .unwrap();
        let end = engine.undo_depth();

        let reverted = engine
            .revert_range(start, end, EditOrigin::Agent)
            .unwrap()
            .expect("エージェント編集が2件戻る");
        assert_eq!(reverted.commands, 2);
        let entities = &engine.project().sheets[0].entities;
        assert!(!entities.contains_key(&agent_a_id), "エージェントの編集は戻る");
        assert!(!entities.contains_key(&agent_b_id));
        assert!(entities.contains_key(&user_id), "ユーザーの手編集は残る");
    }

    /// A revert is a normal edit in the history, so undoing it brings the agent's work back.
    /// 巻き戻しも通常の編集として履歴に乗るため、undoすればエージェントの編集が戻ってくる。
    #[test]
    fn revert_range_is_itself_undoable() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();
        let start = engine.undo_depth();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: wire,
                },
                EditOrigin::Agent,
            )
            .unwrap();
        let end = engine.undo_depth();

        engine.revert_range(start, end, EditOrigin::Agent).unwrap();
        assert!(!engine.project().sheets[0].entities.contains_key(&id));
        assert_eq!(engine.undo_depth(), end + 1, "巻き戻しも1件の履歴になる");
        assert_eq!(
            engine.history_origins().last(),
            Some(&EditOrigin::User),
            "巻き戻しはユーザー操作として積まれる"
        );

        engine.undo().unwrap().unwrap();
        assert!(
            engine.project().sheets[0].entities.contains_key(&id),
            "巻き戻しの取り消しでエージェントの編集が戻る"
        );
        engine.redo().unwrap().unwrap();
        assert!(!engine.project().sheets[0].entities.contains_key(&id));
    }

    /// If the agent's edit cannot be undone against the current drawing (the user deleted the target), the whole revert is refused and nothing changes.
    /// エージェントの編集を現在の図面へ逆適用できない場合(対象をユーザーが消した等)、巻き戻し全体を拒否し何も変更しない。
    #[test]
    fn revert_range_refuses_conflicting_reverts_without_partial_changes() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: wire.clone(),
            })
            .unwrap();

        let start = engine.undo_depth();
        // エージェントが色を変え、さらに別の配線を追加した
        let mut updated = wire.clone();
        if let Entity::Wire(w) = &mut updated {
            w.color = "blue".into();
        }
        engine
            .execute_as(
                Command::UpdateEntity {
                    sheet_id,
                    entity: updated,
                },
                EditOrigin::Agent,
            )
            .unwrap();
        let other = sample_wire();
        let other_id = other.id();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: other,
                },
                EditOrigin::Agent,
            )
            .unwrap();
        let end = engine.undo_depth();

        // ユーザーが更新対象を手で削除 → 色の巻き戻し先が存在しない
        engine
            .execute(Command::DeleteEntities {
                sheet_id,
                ids: vec![id],
            })
            .unwrap();
        let depth_before = engine.undo_depth();

        let err = engine
            .revert_range(start, end, EditOrigin::Agent)
            .unwrap_err();
        assert!(
            matches!(err, CoreError::RevertConflict(_)),
            "衝突は専用エラー: {err}"
        );
        assert!(
            engine.project().sheets[0].entities.contains_key(&other_id),
            "部分適用しない(戻せた分も戻さない)"
        );
        assert_eq!(engine.undo_depth(), depth_before, "履歴も増えない");
    }

    /// Commands run as one batch become a single history entry, so one undo removes all of them at once (and one redo brings them all back).
    /// バッチとしてまとめて実行したコマンド列は履歴1件になり、undo一発で全部消え、redo一発で全部戻る。
    #[test]
    fn execute_batch_is_undone_in_one_step() {
        let (mut engine, sheet_id) = test_engine();
        let cmds = vec![
            Command::AddEntity {
                sheet_id,
                entity: sample_wire(),
            },
            Command::AddEntity {
                sheet_id,
                entity: sample_wire(),
            },
            Command::AddEntity {
                sheet_id,
                entity: sample_wire(),
            },
        ];
        engine.execute_batch(cmds, EditOrigin::User).unwrap();
        assert_eq!(engine.project().sheets[0].entities.len(), 3);
        assert_eq!(engine.undo_depth(), 1, "3コマンドでも履歴は1件");

        engine.undo().unwrap().unwrap();
        assert!(
            engine.project().sheets[0].entities.is_empty(),
            "undo一発で全部戻る"
        );
        engine.redo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].entities.len(), 3);
    }

    /// If any command in a batch fails, the whole batch is refused: the drawing is unchanged and nothing lands in the history.
    /// バッチ内の1つでも失敗したらバッチ全体を拒否し、図面は変わらず履歴にも残らない。
    #[test]
    fn execute_batch_refuses_everything_when_one_command_fails() {
        let (mut engine, sheet_id) = test_engine();
        let missing = Uuid::new_v4();
        let err = engine.execute_batch(
            vec![
                Command::AddEntity {
                    sheet_id,
                    entity: sample_wire(),
                },
                Command::AddEntity {
                    sheet_id: missing,
                    entity: sample_wire(),
                },
            ],
            EditOrigin::User,
        );
        assert!(err.is_err());
        assert!(engine.project().sheets[0].entities.is_empty(), "部分適用しない");
        assert_eq!(engine.undo_depth(), 0, "履歴も増えない");
    }

    /// An empty batch changes nothing at all: no history entry and no new document revision.
    /// 空のバッチは何も変えない(履歴も増えず、ドキュメントrevisionも進まない)。
    #[test]
    fn empty_batch_changes_nothing() {
        let (mut engine, _) = test_engine();
        let revision = engine.revision();
        let patch = engine.execute_batch(Vec::new(), EditOrigin::User).unwrap();
        assert!(patch.ops.is_empty());
        assert_eq!(engine.revision(), revision);
        assert_eq!(engine.undo_depth(), 0);
    }

    /// Reverting a range with no edits of that origin reports "nothing to do" instead of touching the drawing.
    /// その由来の編集が1件も無い区間の巻き戻しは、図面に触れず「戻すものが無い」と報告する。
    #[test]
    fn revert_range_without_matching_edits_changes_nothing() {
        let (mut engine, sheet_id) = test_engine();
        let start = engine.undo_depth();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: sample_wire(),
            })
            .unwrap();
        let end = engine.undo_depth();
        let revision = engine.revision();
        assert!(engine
            .revert_range(start, end, EditOrigin::Agent)
            .unwrap()
            .is_none());
        assert_eq!(engine.project().sheets[0].entities.len(), 1);
        assert_eq!(engine.revision(), revision, "revisionも進まない");
    }

    // --- 盤レイアウトのアイテム番号 (バルーン) ---------------------------------

    /// 盤シート1枚を足し、そこへフットプリントを置いたエンジンを作る。
    fn panel_engine(places: &[(&str, f64, f64)]) -> (Engine, SheetId, Vec<EntityId>) {
        let mut engine = Engine::new(Project::new("panel"));
        engine
            .execute(Command::AddSheet {
                name: "盤".into(),
                size: PaperSize::A3,
                orientation: Orientation::Landscape,
                kind: SheetKind::Panel,
            })
            .unwrap();
        let sheet_id = engine.project().sheets.last().unwrap().id;
        let mut ids = Vec::new();
        for (reference, x, y) in places {
            let id = Uuid::new_v4();
            ids.push(id);
            engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: Entity::Footprint(Footprint {
                        id,
                        at: Point::new(*x, *y),
                        w: 20.0,
                        h: 40.0,
                        rotation: 0,
                        reference: (*reference).into(),
                        part_no: String::new(),
                        item_no: None,
                    }),
                })
                .unwrap();
        }
        (engine, sheet_id, ids)
    }

    fn item_no_of(engine: &Engine, sheet_id: SheetId, id: EntityId) -> Option<u32> {
        match engine.project().sheet(sheet_id).unwrap().entities.get(&id) {
            Some(Entity::Footprint(fp)) => fp.item_no,
            other => panic!("フットプリントではない: {other:?}"),
        }
    }

    /// Numbering the panel gives every footprint on that sheet an item number in reading order.
    /// 盤の採番は、そのシートのフットプリント全てに読み順のアイテム番号を振る。
    #[test]
    fn renumbering_a_panel_numbers_every_footprint() {
        let (mut engine, sheet_id, ids) =
            panel_engine(&[("K2", 200.0, 80.0), ("K1", 100.0, 80.0)]);
        engine
            .execute(Command::RenumberPanelItems { sheet_id })
            .unwrap();
        assert_eq!(item_no_of(&engine, sheet_id, ids[1]), Some(1), "左のK1が1番");
        assert_eq!(item_no_of(&engine, sheet_id, ids[0]), Some(2));
    }

    /// Undoing the numbering puts the previous item numbers back, including parts that had no number at all.
    /// 採番のundoは元のアイテム番号を戻す(番号が無かった部品は番号の無い状態へ戻る)。
    #[test]
    fn undoing_the_numbering_restores_the_previous_numbers() {
        let (mut engine, sheet_id, ids) =
            panel_engine(&[("K2", 200.0, 80.0), ("K1", 100.0, 80.0)]);
        engine
            .execute(Command::SetPanelItemNumbers {
                sheet_id,
                numbers: vec![PanelItemNumber {
                    footprint_id: ids[0],
                    item_no: Some(9),
                }],
            })
            .unwrap();
        engine
            .execute(Command::RenumberPanelItems { sheet_id })
            .unwrap();
        assert_eq!(item_no_of(&engine, sheet_id, ids[0]), Some(2));
        engine.undo().unwrap();
        assert_eq!(item_no_of(&engine, sheet_id, ids[0]), Some(9), "元の番号へ戻る");
        assert_eq!(item_no_of(&engine, sheet_id, ids[1]), None, "無かった番号は無いまま");
    }

    /// Numbering the same panel a second time changes nothing on the drawing: the numbers are already the ones the reading order asks for.
    /// 同じ盤をもう一度採番しても図面は何も変わらない(すでに読み順どおりの番号が入っているため)。
    #[test]
    fn numbering_an_already_numbered_panel_changes_nothing() {
        let (mut engine, sheet_id, ids) = panel_engine(&[("K1", 100.0, 80.0)]);
        engine
            .execute(Command::RenumberPanelItems { sheet_id })
            .unwrap();
        let patch = engine
            .execute(Command::RenumberPanelItems { sheet_id })
            .unwrap();
        assert!(patch.ops.is_empty(), "図面は1つも変わらない: {:?}", patch.ops);
        assert_eq!(item_no_of(&engine, sheet_id, ids[0]), Some(1));
    }

    /// The numbering command is plain JSON, so agents and the CLI can send it.
    /// 採番コマンドは素のJSONで表現できるので、AIやCLIから送れる。
    #[test]
    fn the_panel_numbering_command_is_plain_json() {
        let (mut engine, sheet_id, _) = panel_engine(&[("K1", 100.0, 80.0)]);
        let json = format!(r#"{{"type":"renumber_panel_items","sheet_id":"{sheet_id}"}}"#);
        let command: Command = serde_json::from_str(&json).unwrap();
        engine.execute(command).unwrap();
    }

    /// Numbering a sheet that does not exist is refused instead of silently doing nothing.
    /// 存在しないシートの採番は、黙って何もせずに断られる。
    #[test]
    fn numbering_an_unknown_sheet_is_refused() {
        let (mut engine, _, _) = panel_engine(&[("K1", 100.0, 80.0)]);
        let missing = Uuid::new_v4();
        assert!(engine
            .execute(Command::RenumberPanelItems { sheet_id: missing })
            .is_err());
    }

    /// 端子台を1台置いたエンジン (attrsは呼び出し側で足す)。戻り値は (engine, sheet_id, entity_id)。
    fn terminal_engine(symbol_id: &str) -> (Engine, SheetId, EntityId) {
        let (mut engine, sheet_id) = test_engine();
        let entity = place(symbol_id);
        let id = entity.id();
        engine.execute(Command::AddEntity { sheet_id, entity }).unwrap();
        (engine, sheet_id, id)
    }

    fn terminal_numbers_attr(engine: &Engine, sheet_id: SheetId, id: EntityId) -> Option<String> {
        match engine.project().sheet(sheet_id).unwrap().entities.get(&id) {
            Some(Entity::Symbol(s)) => s.attrs.get("terminal_numbers").cloned(),
            other => panic!("シンボルではない: {other:?}"),
        }
    }

    fn jumpers_attr(engine: &Engine, sheet_id: SheetId, id: EntityId) -> Option<String> {
        match engine.project().sheet(sheet_id).unwrap().entities.get(&id) {
            Some(Entity::Symbol(s)) => s.attrs.get("jumpers").cloned(),
            other => panic!("シンボルではない: {other:?}"),
        }
    }

    fn pin_groups_attr(engine: &Engine, sheet_id: SheetId, id: EntityId) -> Option<String> {
        match engine.project().sheet(sheet_id).unwrap().entities.get(&id) {
            Some(Entity::Symbol(s)) => s.attrs.get("pin_groups").cloned(),
            other => panic!("シンボルではない: {other:?}"),
        }
    }

    /// Renumbering an 8-pole terminal block from a given start writes every pin's new label in pin order.
    /// 8極の端子台を開始番号から採番すると、全ピンの新しい番号がピン順に書き込まれる。
    #[test]
    fn renumbering_writes_every_terminal_number_from_the_start_value() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        assert_eq!(
            terminal_numbers_attr(&engine, sheet_id, id).as_deref(),
            Some("11,12,13,14,15,16,17,18")
        );
    }

    /// On a multi-level block, renumbering keeps each pole's level letters and only changes the pole number, so every level of one pole shares the same new number.
    /// 多段端子台の採番は各極の段記号を保ったまま極番号だけを変えるので、同じ極の全段が同じ新番号を共有する。
    #[test]
    fn renumbering_a_multilevel_block_keeps_the_level_letters() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p_2l");
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        assert_eq!(
            terminal_numbers_attr(&engine, sheet_id, id).as_deref(),
            Some("11a,11b,12a,12b,13a,13b,14a,14b")
        );
    }

    /// A prefix and a step greater than one both apply: the step advances the pole number and the prefix is written before every label.
    /// 接頭辞と1より大きい増分はどちらも効く: 増分が極番号を進め、接頭辞は全ラベルの前に付く。
    #[test]
    fn renumbering_applies_the_step_and_prefix() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 1, step: 2, prefix: "P".into() })
            .unwrap();
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id).as_deref(), Some("P1,P3,P5,P7"));
    }

    /// Renumbering rewrites the block's jumper spec to the new labels in the same command, so a jumper between two poles still names the terminals it actually connects.
    /// 採番は同じコマンド内でジャンパの指定も新しい番号へ書き換えるので、ジャンパは実際に繋ぐ端子を指したままになる。
    #[test]
    fn renumbering_rewrites_the_jumpers_to_the_new_labels() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        engine.execute(set_attr_command(&engine, sheet_id, id, "jumpers", "1-2")).unwrap();
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        assert_eq!(jumpers_attr(&engine, sheet_id, id).as_deref(), Some("11-12"));
    }

    /// Renumbering also rewrites a pin-group range's endpoints to the new labels, since the range names terminals by number.
    /// 採番はピングループの範囲の両端も新しい番号へ書き換える。範囲は端子番号で指しているため。
    #[test]
    fn renumbering_rewrites_pin_group_ranges_too() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        engine.execute(set_attr_command(&engine, sheet_id, id, "pin_groups", "1-4:電源")).unwrap();
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        assert_eq!(pin_groups_attr(&engine, sheet_id, id).as_deref(), Some("11-14:電源"));
    }

    /// その端子台の表示ラベル対応表 ([`crate::terminal_chart::TerminalLabels`]) を作る。
    fn labels_of(engine: &Engine, sheet_id: SheetId, id: EntityId) -> crate::terminal_chart::TerminalLabels {
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!("シンボルではない");
        };
        let def = crate::symbol::resolve_symbol(&s.symbol_id).unwrap();
        crate::terminal_chart::TerminalLabels::of(&def, &s.attrs)
    }

    /// その端子台のシンボル定義。
    fn def_of(engine: &Engine, sheet_id: SheetId, id: EntityId) -> crate::symbol::SymbolDef {
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!("シンボルではない");
        };
        crate::symbol::resolve_symbol(&s.symbol_id).unwrap()
    }

    /// After a renumbering, the pin-group bracket is still drawn on the drawing and the design check stays clean: the stored range is matched against the block's displayed terminal numbers.
    /// 採番したあとでもピングループの括弧は図面に描かれ続け、図面検証も無指摘のままになる: 保存された範囲はその端子台の表示番号と突き合わせられるため。
    #[test]
    fn a_pin_group_bracket_still_draws_after_renumbering_the_terminals() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        engine.execute(set_attr_command(&engine, sheet_id, id, "pin_groups", "1-4:電源")).unwrap();
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        let (def, labels) = (def_of(&engine, sheet_id, id), labels_of(&engine, sheet_id, id));
        let spec = pin_groups_attr(&engine, sheet_id, id).unwrap();
        assert_eq!(
            crate::pin_groups::pin_group_brackets(&def, &spec, labels.display_map()).len(),
            1,
            "採番後も括弧が1本描かれる (spec={spec})"
        );
        assert!(
            crate::pin_groups::pin_group_issues(&def, &spec, labels.display_map()).is_empty(),
            "採番後に「存在しないピン」を誤報しない"
        );
        let sheet = engine.project().sheet(sheet_id).unwrap();
        let diags = crate::verify::verify_sheet(sheet, &crate::symbol::sheet_symbol_defs(sheet));
        assert!(
            !diags.iter().any(|d| d.code == crate::pin_groups::PIN_GROUP_UNKNOWN_PIN),
            "採番だけでERCが増えない: {diags:?}"
        );
    }

    /// On a multi-level block, a pin-group range that names one level's terminals still draws its bracket after a renumbering, because the level letters are kept.
    /// 多段端子台で1段ぶんの端子を指すピングループの範囲も、段記号が保たれるので採番後に括弧を描き続ける。
    #[test]
    fn a_multilevel_pin_group_bracket_survives_a_renumbering() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p_2l");
        engine.execute(set_attr_command(&engine, sheet_id, id, "pin_groups", "1a-4a:電源")).unwrap();
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        let spec = pin_groups_attr(&engine, sheet_id, id).unwrap();
        assert_eq!(spec, "11a-14a:電源");
        let (def, labels) = (def_of(&engine, sheet_id, id), labels_of(&engine, sheet_id, id));
        assert_eq!(
            crate::pin_groups::pin_group_brackets(&def, &spec, labels.display_map()).len(),
            1,
            "多段でも採番後に括弧が描かれる"
        );
    }

    /// A jumper item the editor cannot read (or that names a terminal the block does not have) is carried through a renumbering unchanged, so the design check keeps pointing at it and undo brings the attribute back byte for byte here — this holds whenever the old and new label sets do not overlap (see `undoing_a_renumbering_of_an_unreadable_item_is_not_byte_identical_when_old_and_new_labels_overlap` for the documented exception).
    /// 読めないジャンパの項目 (や端子台に無い端子を指す項目) は採番をそのまま通り抜けるので、図面検証はそれを指し続け、undoで属性は1バイトも変わらず戻る (ここでは旧番号と新番号のラベル集合が重ならないため。重なる場合の例外は`undoing_a_renumbering_of_an_unreadable_item_is_not_byte_identical_when_old_and_new_labels_overlap`を参照)。
    #[test]
    fn a_broken_jumper_item_survives_a_renumbering_and_comes_back_unchanged_on_undo() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        let original = "1-2, abc ,1-99";
        engine.execute(set_attr_command(&engine, sheet_id, id, "jumpers", original)).unwrap();
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        assert_eq!(
            jumpers_attr(&engine, sheet_id, id).as_deref(),
            Some("11-12, abc ,1-99"),
            "読める項目だけが追従し、読めない項目はそのまま残る"
        );
        let diags = crate::terminal_chart::check_terminal_block(engine.project().sheet(sheet_id).unwrap(), id);
        assert!(
            diags.iter().any(|d| d.message.contains("abc")),
            "採番後も読めない項目を指摘し続ける: {diags:?}"
        );
        assert!(
            diags.iter().any(|d| d.code == crate::terminal_chart::JUMPER_UNKNOWN_TERMINAL),
            "存在しない端子の指摘も残る: {diags:?}"
        );
        engine.undo().unwrap();
        assert_eq!(
            jumpers_attr(&engine, sheet_id, id).as_deref(),
            Some(original),
            "undoで元のバイト列へ戻る"
        );
    }

    /// The undo of a renumbering does NOT always restore an unreadable jumper item byte for byte: when the old and new label sets overlap, a token the forward rewrite could not resolve as an old label (because it never existed under the old numbering) can become resolvable as a new label on the reverse pass, so the reverse rewrite touches an item the forward rewrite left alone. This is a documented corner case (docs/05-wire-management), not a bug this task fixes.
    /// 採番のundoは、読めないジャンパの項目を必ずしも1バイトも変えずに戻すとは限らない: 旧番号と新番号のラベル集合が重なると、順方向では旧番号として解決できなかったトークン(旧番号には存在しなかったため)が、逆方向では新番号として解決できてしまうことがあり、順方向では触れなかった項目に逆方向だけが触れてしまう。これはドキュメント化済みの境界事例 (docs/05-wire-management) であり、このタスクで直す不具合ではない。
    #[test]
    fn undoing_a_renumbering_of_an_unreadable_item_is_not_byte_identical_when_old_and_new_labels_overlap() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        // 「5」は旧番号(1..4)には存在しないが、開始番号2の採番後の新番号(2..5)には存在するようになる
        // (旧集合{1,2,3,4}と新集合{2,3,4,5}が重なる)
        engine.execute(set_attr_command(&engine, sheet_id, id, "jumpers", "5-2")).unwrap();
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 2, step: 1, prefix: String::new() })
            .unwrap();
        assert_eq!(
            jumpers_attr(&engine, sheet_id, id).as_deref(),
            Some("5-2"),
            "採番時点では「5」を旧番号として解決できないので、項目はそのまま残る"
        );
        engine.undo().unwrap();
        assert_ne!(
            jumpers_attr(&engine, sheet_id, id).as_deref(),
            Some("5-2"),
            "undoでは「5」が新番号として解決できてしまうため、1バイトも変わらずには戻らない (境界事例)"
        );
    }

    /// Undoing a renumbering restores a pin-group spec byte for byte, spacing included: the rewrite only replaces the terminal-number tokens it recognises.
    /// 採番のundoはピングループの指定を空白ごと1バイトも変えずに戻す: 書き換えは認識できた端子番号のトークンだけを置き換えるため。
    #[test]
    fn undoing_a_renumbering_restores_the_pin_group_spec_byte_for_byte() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        let original = "1 - 4 : 電源 ,こわれた項目,5-8:信号";
        engine.execute(set_attr_command(&engine, sheet_id, id, "pin_groups", original)).unwrap();
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        assert_eq!(
            pin_groups_attr(&engine, sheet_id, id).as_deref(),
            Some("11 - 14 : 電源 ,こわれた項目,15-18:信号")
        );
        engine.undo().unwrap();
        assert_eq!(pin_groups_attr(&engine, sheet_id, id).as_deref(), Some(original));
    }

    /// A chain of jumpers over three terminals follows a renumbering with a step of two: both endpoints of every link get the new label and the links stay a chain.
    /// 3端子にまたがるジャンパの連鎖は増分2の採番にも追従する: 各リンクの両端が新しい番号になり、連鎖のままになる。
    #[test]
    fn a_jumper_chain_follows_a_renumbering_with_a_step() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        engine.execute(set_attr_command(&engine, sheet_id, id, "jumpers", "1-2,2-3")).unwrap();
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 2, prefix: String::new() })
            .unwrap();
        assert_eq!(jumpers_attr(&engine, sheet_id, id).as_deref(), Some("11-13,13-15"));
        let chart = crate::terminal_chart::terminal_chart(engine.project().sheet(sheet_id).unwrap(), id).unwrap();
        assert_eq!(
            chart.jumpers,
            vec![("11".to_string(), "13".to_string()), ("13".to_string(), "15".to_string())],
            "採番後もチャートが連鎖として読む"
        );
        assert!(chart.jumper_issues.is_empty(), "{:?}", chart.jumper_issues);
    }

    /// Renumbering a symbol that is not a terminal block is refused: the terminal-number override would rewrite that symbol's pin-number texts on the drawing.
    /// 端子台ではないシンボルの採番は拒否される: 端子番号の上書きはそのシンボルのピン番号テキストを図面上で書き換えてしまうため。
    #[test]
    fn renumbering_a_symbol_that_is_not_a_terminal_block_is_refused() {
        let (mut engine, sheet_id, id) = terminal_engine("relay_coil");
        let err = engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .expect_err("端子台以外は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None);
        assert!(engine
            .execute(Command::SetTerminalNumbers {
                sheet_id,
                entity_id: id,
                numbers: Some(vec!["X".into(), "Y".into()]),
            })
            .is_err());
    }

    /// Terminal numbers are stored with the surrounding spaces removed, so what the command validated is exactly what the chart reads back.
    /// 端子番号は前後の空白を落として保存されるので、コマンドが検証した値とチャートが読み戻す値が必ず一致する。
    #[test]
    fn terminal_numbers_are_stored_without_their_surrounding_spaces() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        engine
            .execute(Command::SetTerminalNumbers {
                sheet_id,
                entity_id: id,
                numbers: Some(vec![" 11".into(), "12 ".into(), " 13 ".into(), "14".into()]),
            })
            .unwrap();
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id).as_deref(), Some("11,12,13,14"));
        let chart = crate::terminal_chart::terminal_chart(engine.project().sheet(sheet_id).unwrap(), id).unwrap();
        assert_eq!(
            chart.rows.iter().map(|r| r.terminal.as_str()).collect::<Vec<_>>(),
            vec!["11", "12", "13", "14"]
        );
    }

    /// Two labels that differ only in their surrounding spaces count as a duplicate and are refused: they would collapse to the same terminal number in storage.
    /// 前後の空白しか違わない2つのラベルは重複として拒否される: 保存されると同じ端子番号になってしまうため。
    #[test]
    fn terminal_numbers_that_are_duplicates_once_trimmed_are_refused() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        let err = engine
            .execute(Command::SetTerminalNumbers {
                sheet_id,
                entity_id: id,
                numbers: Some(vec!["1 ".into(), "1".into(), "2".into(), "3".into()]),
            })
            .expect_err("trim後の重複は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None);
    }

    /// A terminal number containing a comma is refused: the comma is the separator of the stored list, so the value would read back as a different number of terminals.
    /// カンマを含む端子番号は拒否される: カンマは保存形の区切り記号なので、読み戻すと端子の個数そのものが変わってしまうため。
    #[test]
    fn a_terminal_number_containing_a_comma_is_refused() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        let err = engine
            .execute(Command::SetTerminalNumbers {
                sheet_id,
                entity_id: id,
                numbers: Some(vec!["1,2".into(), "3".into(), "4".into(), "5".into()]),
            })
            .expect_err("カンマ入りは拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None);
    }

    /// `key`属性を`value`へ書き換える`update_entity`コマンドを作る (シンボルでなければpanic)。
    fn set_attr_command(engine: &Engine, sheet_id: SheetId, id: EntityId, key: &str, value: &str) -> Command {
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!("シンボルではない");
        };
        let mut attrs = s.attrs.clone();
        attrs.insert(key.to_string(), value.to_string());
        Command::UpdateEntity {
            sheet_id,
            entity: Entity::Symbol(SymbolInstance { attrs, ..s.clone() }),
        }
    }

    /// Undoing a renumbering restores the previous terminal numbers exactly: a prior direct override comes back, and renumbering from the symbol's defaults undoes to no override at all.
    /// 採番のundoは直前の端子番号を正確に戻す: 直前に直接指定していればそれが戻り、既定値から採番していれば上書き無しの状態へ戻る。
    #[test]
    fn undoing_a_renumbering_restores_the_previous_numbers() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        engine
            .execute(Command::SetTerminalNumbers {
                sheet_id,
                entity_id: id,
                numbers: Some(vec!["A".into(), "B".into(), "C".into(), "D".into()]),
            })
            .unwrap();
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id).as_deref(), Some("11,12,13,14"));
        engine.undo().unwrap();
        assert_eq!(
            terminal_numbers_attr(&engine, sheet_id, id).as_deref(),
            Some("A,B,C,D"),
            "直前の直接指定へ戻る"
        );
        engine.undo().unwrap();
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None, "上書き無し (既定) へ戻る");
    }

    /// Undoing a renumbering restores an explicit override even when that override's value happened to equal the symbol's own default labels: the key was present, so it comes back, instead of being treated as "no override" and dropped (Task 3 ルーリング3a: had_overrideの判定はキーの有無と妥当性で決め、展開結果が既定と同じかどうかでは決めない).
    /// 明示的な上書きの値がたまたま既定のラベルと同じでも、採番のundoはそのキーを復元する: キーがあった事実は「上書き無し」として消されない (Task 3 ルーリング3a)。
    #[test]
    fn undoing_a_renumbering_restores_an_override_that_matches_the_default_value() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        // SetTerminalNumbersは「今の表示と同じ値」を渡すと無変更として無視する (このキーを持つ
        // 状態そのものは作れない) ので、直接update_entityでキーを書く (MCP/Link API経由の
        // 直接編集と同じ経路。ルーリング3aが問題にしているのはまさにこの経路で作られる状態)
        engine.execute(set_attr_command(&engine, sheet_id, id, "terminal_numbers", "1,2,3,4")).unwrap();
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id).as_deref(), Some("1,2,3,4"));
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        engine.undo().unwrap();
        assert_eq!(
            terminal_numbers_attr(&engine, sheet_id, id).as_deref(),
            Some("1,2,3,4"),
            "既定と同じ値の明示的な上書きでも、キーごと復元される"
        );
    }

    /// Setting a number list whose length does not match the pin count is refused and leaves the drawing untouched.
    /// ピン数と個数が合わない番号列の指定は拒否され、図面は変わらない。
    #[test]
    fn a_terminal_number_count_mismatch_is_refused_without_changing_the_drawing() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        let err = engine
            .execute(Command::SetTerminalNumbers {
                sheet_id,
                entity_id: id,
                numbers: Some(vec!["1".into(), "2".into(), "3".into()]),
            })
            .expect_err("4個のピンに3個は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None);
        assert_eq!(engine.undo_depth(), 1, "配置コマンドだけが履歴に残る (採番の失敗は積まれない)");
    }

    /// A duplicate label in the number list is refused without changing the drawing.
    /// 番号列に重複したラベルがあると、図面を変えずに拒否される。
    #[test]
    fn duplicate_terminal_numbers_are_refused() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        let err = engine
            .execute(Command::SetTerminalNumbers {
                sheet_id,
                entity_id: id,
                numbers: Some(vec!["1".into(), "1".into(), "2".into(), "3".into()]),
            })
            .expect_err("重複は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None);
    }

    /// An empty label in the number list is refused without changing the drawing.
    /// 番号列に空要素があると、図面を変えずに拒否される。
    #[test]
    fn an_empty_terminal_number_is_refused() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        let err = engine
            .execute(Command::SetTerminalNumbers {
                sheet_id,
                entity_id: id,
                numbers: Some(vec!["1".into(), "".into(), "3".into(), "4".into()]),
            })
            .expect_err("空要素は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None);
    }

    /// Setting the numbers to `None` after a direct override returns the block to the symbol's default numbers (the `terminal_numbers` attribute itself disappears).
    /// 直接指定のあとに番号を`None`にすると、シンボルの既定番号へ戻る (`terminal_numbers`属性自体が消える)。
    #[test]
    fn setting_numbers_to_none_returns_to_the_defaults() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        engine
            .execute(Command::SetTerminalNumbers {
                sheet_id,
                entity_id: id,
                numbers: Some(vec!["A".into(), "B".into(), "C".into(), "D".into()]),
            })
            .unwrap();
        engine
            .execute(Command::SetTerminalNumbers { sheet_id, entity_id: id, numbers: None })
            .unwrap();
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None);
    }

    /// Renumbering an entity id that does not exist on the sheet is refused instead of silently doing nothing.
    /// シートに無いentity idの採番は、黙って何もせずに断られる。
    #[test]
    fn renumbering_an_unknown_entity_is_refused() {
        let (mut engine, sheet_id, _) = terminal_engine("terminal_block_4p");
        let missing = Uuid::new_v4();
        assert!(engine
            .execute(Command::RenumberTerminals {
                sheet_id,
                entity_id: missing,
                start: 1,
                step: 1,
                prefix: String::new(),
            })
            .is_err());
    }

    /// The renumbering command is plain JSON with sensible defaults for the start and step, so agents and the CLI can send it with just an entity id.
    /// 採番コマンドは既定の開始番号・増分を持つ素のJSONで表現できるので、AIやCLIからentity idだけでも送れる。
    #[test]
    fn the_terminal_renumbering_command_is_plain_json() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        let json = format!(r#"{{"type":"renumber_terminals","sheet_id":"{sheet_id}","entity_id":"{id}"}}"#);
        let command: Command = serde_json::from_str(&json).unwrap();
        engine.execute(command).unwrap();
        // 開始番号1・増分1は4極の端子台の既定 ("1,2,3,4") と同じ結果になるので、
        // 図面は変わらない (属性は書き込まれない。採番済みの盤を再採番しても変わらないのと同じ)
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None);
    }

    // --- SetTerminalLevels (Task 3: 多段端子の生成・解除) ---

    /// a-b間にワイヤを1本引く (entity idを返す)。
    fn add_wire(sheet_id: SheetId, engine: &mut Engine, a: (f64, f64), b: (f64, f64)) -> EntityId {
        let w = Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
            color: String::new(),
            sq: 0.0,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: None,
        };
        let id = w.id;
        engine.execute(Command::AddEntity { sheet_id, entity: Entity::Wire(w) }).unwrap();
        id
    }

    /// 端子台を回転・反転を指定して1台置く (段数変更で座標がどう動くかを見るテスト用)。
    fn terminal_engine_turned(
        symbol_id: &str,
        rotation: u16,
        mirror: bool,
    ) -> (Engine, SheetId, EntityId) {
        let (mut engine, sheet_id) = test_engine();
        let mut entity = place(symbol_id);
        if let Entity::Symbol(s) = &mut entity {
            s.rotation = rotation;
            s.mirror = mirror;
        }
        let id = entity.id();
        engine.execute(Command::AddEntity { sheet_id, entity }).unwrap();
        (engine, sheet_id, id)
    }

    /// 端子番号`label`の接続点 (貫通端子なので左右2つ) の**用紙上絶対座標**を並べて返す。
    fn terminal_pin_points(
        engine: &Engine,
        sheet_id: SheetId,
        id: EntityId,
        label: &str,
    ) -> Vec<Point> {
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id)
        else {
            panic!("シンボルではない");
        };
        let def = crate::symbol::resolve_symbol(&s.symbol_id).expect("シンボル定義");
        let labels = crate::terminal_chart::TerminalLabels::of(&def, &s.attrs);
        let mut points: Vec<Point> = crate::netlist::pin_positions(s, &def)
            .into_iter()
            .filter(|(no, _)| labels.display_of(no) == label)
            .map(|(_, p)| p)
            .collect();
        points.sort_by(|a, b| (a.x, a.y).partial_cmp(&(b.x, b.y)).expect("NaNは無い"));
        points
    }

    /// エンティティ`wire`が属するネットに載っているピン (`(entity_id, ピン番号)`)。
    fn net_pins_of_wire(engine: &Engine, sheet_id: SheetId, wire: EntityId) -> Vec<(EntityId, String)> {
        let sheet = engine.project().sheet(sheet_id).unwrap();
        let nets = crate::netlist::extract_netlist(sheet, &crate::symbol::sheet_symbol_defs(sheet));
        nets.iter()
            .find(|n| n.wire_ids.contains(&wire))
            .map(|n| n.pins.iter().map(|p| (p.entity_id, p.pin.clone())).collect())
            .unwrap_or_default()
    }

    /// Turning a single-level 4-pole terminal block into 2 levels rewrites its symbol_id and leaves the terminal_numbers attribute unset (no explicit override existed).
    /// 4極の単段端子台を2段化すると、symbol_idが書き換わり、terminal_numbers属性は未設定のまま (明示的な上書きが無かったため)。
    #[test]
    fn setting_levels_turns_a_single_level_block_into_two_levels() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 }).unwrap();
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!()
        };
        assert_eq!(s.symbol_id, "terminal_block_4p_2l");
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None);
    }

    /// Undoing a level increase (or setting levels back to 1 directly) restores the plain single-level symbol_id, never a "_1l" alias.
    /// 段数を増やしたあとの解除 (直接levels=1を指定しても) は、単段のsymbol_idそのものに戻る (`_1l`という別名にはならない)。
    #[test]
    fn setting_levels_back_to_one_restores_the_plain_single_level_id() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 }).unwrap();
        engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 1 }).unwrap();
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!()
        };
        assert_eq!(s.symbol_id, "terminal_block_4p");
    }

    /// Increasing levels expands an explicit terminal_numbers override into one lettered label per level, pole-major, exactly as the plan's example describes ("11,12,…" -> "11a,11b,12a,12b,…").
    /// 段を増やすと、明示的なterminal_numbers上書きが極→段の順で段記号付きのラベルへ展開される (計画の例と同じ "11,12,…" → "11a,11b,12a,12b,…")。
    #[test]
    fn increasing_levels_expands_an_explicit_terminal_numbers_override() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 }).unwrap();
        assert_eq!(
            terminal_numbers_attr(&engine, sheet_id, id).as_deref(),
            Some("11a,11b,12a,12b,13a,13b,14a,14b")
        );
    }

    /// A jumper written without a level letter on a single-level block moves to level a when the block gains a second level, so the existing connection is not silently dropped.
    /// 段記号の無い単段のジャンパ指定は、2段化すると段aへ移る (既存の結線指定が黙って消えない)。
    #[test]
    fn increasing_levels_moves_a_levelless_jumper_to_level_a() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        engine.execute(set_attr_command(&engine, sheet_id, id, "jumpers", "1-2")).unwrap();
        engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 }).unwrap();
        assert_eq!(jumpers_attr(&engine, sheet_id, id).as_deref(), Some("1a-2a"));
    }

    /// Decreasing levels is refused, and the drawing is left completely untouched, when a wire is still connected to a pin of the level that would vanish.
    /// 消える段のピンに配線が繋がったままだと、段を減らす操作は拒否され、図面は一切変わらない。
    #[test]
    fn decreasing_levels_is_refused_when_a_wire_is_connected_to_a_vanishing_level() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p_2l");
        // 極1・段bの外部側接続点 (107.5, 42.5) から配線を1本引く
        add_wire(sheet_id, &mut engine, (107.5, 42.5), (120.0, 42.5));
        let before = format!("{:?}", engine.project().sheet(sheet_id).unwrap().entities.get(&id));
        let err = engine
            .execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 1 })
            .expect_err("段bに結線が残っているので拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        let after = format!("{:?}", engine.project().sheet(sheet_id).unwrap().entities.get(&id));
        assert_eq!(after, before, "図面は一切変わらない");
    }

    /// Decreasing levels is refused when a jumper still names a terminal of the level that would vanish.
    /// 消える段の端子を指すジャンパが残っていると、段を減らす操作は拒否される。
    #[test]
    fn decreasing_levels_is_refused_when_a_jumper_names_a_vanishing_level() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p_2l");
        engine.execute(set_attr_command(&engine, sheet_id, id, "jumpers", "1a-1b")).unwrap();
        let err = engine
            .execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 1 })
            .expect_err("段bを指すジャンパが残っているので拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert_eq!(jumpers_attr(&engine, sheet_id, id).as_deref(), Some("1a-1b"), "図面は一切変わらない");
    }

    /// Decreasing levels is refused when a pin-group range spans terminals of the level that would vanish.
    /// 消える段の端子を範囲に含むピングループが残っていると、段を減らす操作は拒否される。
    #[test]
    fn decreasing_levels_is_refused_when_a_pin_group_spans_a_vanishing_level() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p_2l");
        engine.execute(set_attr_command(&engine, sheet_id, id, "pin_groups", "1b-4b:信号")).unwrap();
        let err = engine
            .execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 1 })
            .expect_err("段bを範囲に含むピングループが残っているので拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert_eq!(pin_groups_attr(&engine, sheet_id, id).as_deref(), Some("1b-4b:信号"), "図面は一切変わらない");
    }

    /// Decreasing levels is refused when an accessory spec names a terminal of the level that would vanish (spec §1.1 accessories format, checked ahead of its own command landing in a later task).
    /// 消える段の端子を指す付属品指定が残っていると、段を減らす操作は拒否される (spec §1.1 付属品の書式。専用コマンドが入る前から検証する)。
    #[test]
    fn decreasing_levels_is_refused_when_an_accessory_names_a_vanishing_level() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p_2l");
        engine.execute(set_attr_command(&engine, sheet_id, id, "accessories", "1b:end_bracket:TB-EB")).unwrap();
        let err = engine
            .execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 1 })
            .expect_err("段bを指す付属品指定が残っているので拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
    }

    /// When nothing references the vanishing level, decreasing levels succeeds and both the symbol_id and the terminal-number override drop that level's labels.
    /// 消える段に何も参照が残っていなければ、段を減らす操作は成功し、symbol_idと端子番号の上書きの両方から消える段のラベルが落ちる。
    #[test]
    fn decreasing_levels_succeeds_and_drops_the_vanishing_levels_labels() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p_2l");
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        assert_eq!(
            terminal_numbers_attr(&engine, sheet_id, id).as_deref(),
            Some("11a,11b,12a,12b,13a,13b,14a,14b")
        );
        engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 1 }).unwrap();
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!()
        };
        assert_eq!(s.symbol_id, "terminal_block_4p");
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id).as_deref(), Some("11,12,13,14"));
    }

    /// Changing the level count keeps level a's terminals at exactly the same coordinates on the sheet, whatever the block's rotation or mirroring, so nothing that was drawn onto those terminals has to move.
    /// 段数を変えても段aの端子の用紙上の座標は1mmも動かない (端子台の回転・反転にかかわらず) ので、その端子に描かれていたものは動かさずに済む。
    #[test]
    fn a_level_change_keeps_level_a_terminals_at_the_same_sheet_coordinates() {
        for (rotation, mirror) in [(0u16, false), (90, false), (0, true), (90, true)] {
            // 単段 (端子番号 "1") → 2段 (段aは "1a") → 単段
            let (mut engine, sheet_id, id) = terminal_engine_turned("terminal_block_4p", rotation, mirror);
            let before = terminal_pin_points(&engine, sheet_id, id, "1");
            engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 }).unwrap();
            assert_eq!(
                terminal_pin_points(&engine, sheet_id, id, "1a"),
                before,
                "rot={rotation} mirror={mirror}: 1→2段で段aの端子が動いた"
            );
            engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 1 }).unwrap();
            assert_eq!(
                terminal_pin_points(&engine, sheet_id, id, "1"),
                before,
                "rot={rotation} mirror={mirror}: 2→1段で端子が元の座標に戻らない"
            );
            // 3段への一足飛びでも同じ (段aは常に元の位置)
            engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 3 }).unwrap();
            assert_eq!(
                terminal_pin_points(&engine, sheet_id, id, "1a"),
                before,
                "rot={rotation} mirror={mirror}: 1→3段で段aの端子が動いた"
            );
        }
    }

    /// A wire drawn to terminal 1 of a single-level block is still electrically on the same terminal (now called 1a) after a second level is added, and it is not re-mapped to the new level's terminal.
    /// 単段の端子台の端子1に引いてあった配線は、2段化したあとも電気的に同じ端子 (名前は`1a`になる) に繋がったままで、新しくできた段の端子へ繋ぎ変わったりしない。
    #[test]
    fn a_wire_on_terminal_one_stays_on_level_a_after_adding_a_level() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        // 端子1の内部側 (左) 接続点 (97.5, 42.5) へ1本引く
        let wire = add_wire(sheet_id, &mut engine, (85.0, 42.5), (97.5, 42.5));
        engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 }).unwrap();
        let pins = net_pins_of_wire(&engine, sheet_id, wire);
        assert!(
            pins.contains(&(id, "1a".to_string())),
            "2段化後も配線は段aの端子に繋がったまま: {pins:?}"
        );
        assert!(
            !pins.iter().any(|(_, p)| p == "1b"),
            "配線が新しくできた段bの端子へ繋ぎ変わっていない: {pins:?}"
        );
        let sheet = engine.project().sheet(sheet_id).unwrap();
        let chart = crate::terminal_chart::terminal_chart(sheet, id).expect("chart");
        let row = chart.rows.iter().find(|r| r.terminal == "1a").expect("row 1a");
        assert!(!row.spare, "端子1aは予備にならない");
    }

    /// Decreasing levels is refused when the only thing sitting on a vanishing level's pin is a net label (no wire at all), the same coordinate-coincidence rule the netlist uses.
    /// 消える段のピンに載っているのがネットラベルだけ (配線は1本も無い) のときも、ネットリストと同じ座標一致の規則で段を減らす操作は拒否される。
    #[test]
    fn decreasing_levels_is_refused_when_a_net_label_sits_on_a_vanishing_pin() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p_2l");
        let label = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(107.5, 42.5),
            name: "N-P1".into(),
            rotation: 0,
            attrs: Default::default(),
        });
        engine.execute(Command::AddEntity { sheet_id, entity: label }).unwrap();
        let before = format!("{:?}", engine.project().sheet(sheet_id).unwrap().entities.get(&id));
        let err = engine
            .execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 1 })
            .expect_err("段bのピンにネットラベルが載っているので拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        let after = format!("{:?}", engine.project().sheet(sheet_id).unwrap().entities.get(&id));
        assert_eq!(after, before, "図面は一切変わらない");
    }

    /// Going from 3 levels to 2 is refused while a jumper still names a level-c terminal, and allowed once it is gone - then only level c's labels are dropped and the remaining jumper is kept.
    /// 3段から2段へ減らす操作は、段cの端子を指すジャンパが残っている間は拒否され、外せば通る (そのとき落ちるのは段cのラベルだけで、残ったジャンパはそのまま)。
    #[test]
    fn decreasing_three_levels_to_two_is_refused_only_while_level_c_is_referenced() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p_3l");
        engine.execute(set_attr_command(&engine, sheet_id, id, "jumpers", "1a-2a,1b-1c")).unwrap();
        let err = engine
            .execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 })
            .expect_err("段cを指すジャンパが残っているので拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert_eq!(jumpers_attr(&engine, sheet_id, id).as_deref(), Some("1a-2a,1b-1c"));

        engine.execute(set_attr_command(&engine, sheet_id, id, "jumpers", "1a-2a")).unwrap();
        engine
            .execute(Command::RenumberTerminals { sheet_id, entity_id: id, start: 11, step: 1, prefix: String::new() })
            .unwrap();
        engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 }).unwrap();
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!()
        };
        assert_eq!(s.symbol_id, "terminal_block_4p_2l");
        assert_eq!(
            terminal_numbers_attr(&engine, sheet_id, id).as_deref(),
            Some("11a,11b,12a,12b,13a,13b,14a,14b"),
            "段cのラベルだけが落ちる"
        );
        assert_eq!(jumpers_attr(&engine, sheet_id, id).as_deref(), Some("11a-12a"), "残ったジャンパは採番に追従したまま");
    }

    /// Increasing levels rewrites the terminal label of an accessory spec the same way it rewrites jumpers, so an accessory pinned to terminal 1 follows to 1a; the kind and part number are left byte for byte.
    /// 段を増やすと、付属品指定の端子番号もジャンパと同じように書き換わる (端子1に付けた付属品は`1a`へ追従する)。種別と型番は1バイトも変わらない。
    #[test]
    fn increasing_levels_rewrites_accessory_terminal_labels() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        engine
            .execute(set_attr_command(
                &engine,
                sheet_id,
                id,
                "accessories",
                "1:end_bracket:TB-EB,end:end_bracket:TB-EB",
            ))
            .unwrap();
        engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 }).unwrap();
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!()
        };
        assert_eq!(
            s.attrs.get("accessories").map(String::as_str),
            Some("1a:end_bracket:TB-EB,end:end_bracket:TB-EB"),
            "末尾指定の`end`は書き換えない"
        );
    }

    /// A terminal_numbers value that is present but invalid (its item count does not match the pin count) survives a level change untouched and comes back byte for byte after undo; the command never deletes a key it cannot restore.
    /// `terminal_numbers`キーがあるのに値が無効 (個数がピン数と合わない) なとき、その値は段数変更でも書き換わらず、undoで1バイトも変わらず戻る (コマンドは自分で戻せないキーを消さない)。
    #[test]
    fn a_present_but_invalid_terminal_numbers_value_survives_a_level_change() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        engine.execute(set_attr_command(&engine, sheet_id, id, "terminal_numbers", "1,2")).unwrap();
        engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 }).unwrap();
        assert_eq!(
            terminal_numbers_attr(&engine, sheet_id, id).as_deref(),
            Some("1,2"),
            "無効な値でも消さず・書き換えず残す"
        );
        engine.undo().unwrap();
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id).as_deref(), Some("1,2"));
    }

    /// A level change whose relabelled bases would collide (one pole's level-a label ends in level a's own letter and strips to the same base as another pole's level-a label that never had a letter to strip) is refused before anything is written, instead of silently storing a `terminal_numbers` value with duplicate labels.
    /// 段数変更で土台の取り違えにより重複が起きる指定 (一方の極の段aラベルは段aの記号で終わって剥がされ、もう一方の極の段aラベルはもともと記号を持たず剥がされない、その結果同じ土台になる指定) は、何も書き込む前に拒否される (重複したラベルを持つterminal_numbersを黙って保存しない)。
    #[test]
    fn a_level_change_that_would_produce_duplicate_labels_is_refused_without_writing_anything() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_2p_2l");
        // 極1の段aは"11a"(段aの記号で終わるので土台"11"に剥がれる)、極2の段aはもともと"11"
        // (記号を持たないので剥がれない、土台はやはり"11") -> 土台が衝突する
        engine
            .execute(set_attr_command(&engine, sheet_id, id, "terminal_numbers", "11a,11b,11,XX"))
            .unwrap();
        let before = terminal_numbers_attr(&engine, sheet_id, id);
        assert!(engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 3 }).is_err());
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), before, "図面は一切変わらない");
    }

    /// Undoing a level change restores the previous symbol_id and the instance position together, in one undo step, so the terminals land back on their original coordinates.
    /// 段数変更のundoは、変更前のsymbol_idと配置座標を**1回のundoでそろって**戻すので、端子は元の座標へ帰る。
    #[test]
    fn undoing_a_level_change_restores_the_symbol_id_and_the_position_together() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        let before_at = match engine.project().sheet(sheet_id).unwrap().entities.get(&id) {
            Some(Entity::Symbol(s)) => s.at,
            other => panic!("シンボルではない: {other:?}"),
        };
        let before_pins = terminal_pin_points(&engine, sheet_id, id, "1");
        let depth = engine.undo_depth();
        engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 }).unwrap();
        assert_eq!(engine.undo_depth(), depth + 1, "段数変更はundo1段だけ積む");
        engine.undo().unwrap();
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!()
        };
        assert_eq!(s.symbol_id, "terminal_block_4p");
        assert_eq!(s.at, before_at, "配置座標も一緒に戻る");
        assert_eq!(terminal_pin_points(&engine, sheet_id, id, "1"), before_pins);
    }

    /// Asking for a level count outside 1-3 is refused without touching the drawing.
    /// 1〜3の範囲外の段数指定は、図面を変えずに拒否される。
    #[test]
    fn setting_levels_outside_one_to_three_is_refused() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        assert!(engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 0 }).is_err());
        assert!(engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 4 }).is_err());
    }

    /// Changing the level count of a symbol that is not a terminal block is refused, the same gate RenumberTerminals/SetTerminalNumbers already use.
    /// 端子台以外のシンボルの段数変更は拒否される (RenumberTerminals/SetTerminalNumbersと同じ門)。
    #[test]
    fn setting_levels_on_a_symbol_that_is_not_a_terminal_block_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let entity = place("resistor");
        let id = entity.id();
        engine.execute(Command::AddEntity { sheet_id, entity }).unwrap();
        assert!(engine.execute(Command::SetTerminalLevels { sheet_id, entity_id: id, levels: 2 }).is_err());
    }

    /// A symbol_id spelled with the "_1l" one-level alias is refused when placed on the drawing: a single-level terminal block has exactly one valid spelling of its symbol_id.
    /// `_1l`という段数1の別名で綴ったsymbol_idは配置時に拒否される: 単段端子台のsymbol_idの綴りは1通りだけが有効。
    #[test]
    fn a_one_level_alias_symbol_id_is_refused_when_placed() {
        let (mut engine, sheet_id) = test_engine();
        let err = engine
            .execute(Command::AddEntity { sheet_id, entity: place("terminal_block_8p_1l") })
            .expect_err("`_1l`別名は未知のシンボルとして拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
    }

    // --- InsertSpareTerminals (Task 4: 予備端子の挿入) ---

    /// Growing a terminal block's pole count (as inserting a spare does) never moves an existing pin's sheet coordinates — the instance is shifted the opposite way to compensate — so no wire that was attached to it comes loose.
    /// 端子台の極数を増やしても (予備の挿入がそうする)、既存ピンの用紙上の座標は動かない — インスタンスが逆向きにずれて打ち消すため — ので、繋がっていた配線は外れない。
    #[test]
    fn inserting_a_spare_terminal_keeps_every_existing_pins_coordinates() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        let before: Vec<Vec<Point>> =
            (1..=8).map(|n| terminal_pin_points(&engine, sheet_id, id, &n.to_string())).collect();
        engine
            .execute(Command::InsertSpareTerminals { sheet_id, entity_id: id, after: None, count: 1, renumber_following: true })
            .unwrap();
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!()
        };
        assert_eq!(s.symbol_id, "terminal_block_9p");
        for (n, points) in (1..=8).zip(before) {
            assert_eq!(
                terminal_pin_points(&engine, sheet_id, id, &n.to_string()),
                points,
                "端子{n}の座標が動いた"
            );
        }
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None, "末尾挿入は既定の番号列と同じ");
    }

    /// Inserting a spare after a chosen terminal writes the renumbered label sequence and rewrites the jumper spec that used the labels which moved, so the jumper still names the same physical pair of terminals.
    /// 指定した端子の後ろに予備を挿入すると、繰り下がった番号列が書き込まれ、動いたラベルを使っていたジャンパ指定も書き換わるので、ジャンパは同じ物理的な端子の組を指したままになる。
    #[test]
    fn inserting_a_spare_after_a_terminal_renumbers_labels_and_follows_the_jumper() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        engine.execute(set_attr_command(&engine, sheet_id, id, "jumpers", "4-5")).unwrap();
        engine
            .execute(Command::InsertSpareTerminals {
                sheet_id,
                entity_id: id,
                after: Some("3".to_string()),
                count: 1,
                renumber_following: true,
            })
            .unwrap();
        assert_eq!(
            terminal_numbers_attr(&engine, sheet_id, id).as_deref(),
            Some("1,2,3,5,6,7,8,9,4")
        );
        assert_eq!(jumpers_attr(&engine, sheet_id, id).as_deref(), Some("5-6"), "4→5・5→6に追従");
    }

    /// A count of zero is refused without touching the drawing.
    /// 0個の指定は、図面を変えずに拒否される。
    #[test]
    fn inserting_zero_spares_is_refused_without_touching_the_drawing() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        assert!(engine
            .execute(Command::InsertSpareTerminals { sheet_id, entity_id: id, after: None, count: 0, renumber_following: true })
            .is_err());
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!()
        };
        assert_eq!(s.symbol_id, "terminal_block_8p");
    }

    /// Naming a terminal the block does not have as the insertion point is refused.
    /// 端子台に無い端子番号を挿入位置に指定すると拒否される。
    #[test]
    fn inserting_after_an_unknown_terminal_is_refused() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        assert!(engine
            .execute(Command::InsertSpareTerminals {
                sheet_id,
                entity_id: id,
                after: Some("99".to_string()),
                count: 1,
                renumber_following: true,
            })
            .is_err());
    }

    /// Inserting past the dynamic symbol family's pole cap is refused.
    /// 動的シンボルの極数上限を超える挿入は拒否される。
    #[test]
    fn inserting_past_the_pole_cap_is_refused() {
        let (mut engine, sheet_id, id) =
            terminal_engine(&format!("terminal_block_{}p", crate::symbol::DYNAMIC_PIN_MAX));
        assert!(engine
            .execute(Command::InsertSpareTerminals { sheet_id, entity_id: id, after: None, count: 1, renumber_following: true })
            .is_err());
    }

    /// Inserting a spare into a symbol that is not a terminal block is refused, the same gate the other terminal commands already use.
    /// 端子台以外のシンボルへの予備挿入は拒否される (他の端子台系コマンドと同じ門)。
    #[test]
    fn inserting_a_spare_on_a_symbol_that_is_not_a_terminal_block_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let entity = place("resistor");
        let id = entity.id();
        engine.execute(Command::AddEntity { sheet_id, entity }).unwrap();
        assert!(engine
            .execute(Command::InsertSpareTerminals { sheet_id, entity_id: id, after: None, count: 1, renumber_following: true })
            .is_err());
    }

    /// Undoing an inserted spare restores the symbol_id, the terminal numbers and the position together in one step.
    /// 挿入した予備をundoすると、symbol_id・端子番号・位置がまとめて1回で元に戻る。
    #[test]
    fn undoing_an_inserted_spare_restores_symbol_id_numbers_and_position_together() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        let before_at = match engine.project().sheet(sheet_id).unwrap().entities.get(&id) {
            Some(Entity::Symbol(s)) => s.at,
            other => panic!("シンボルではない: {other:?}"),
        };
        let before_pins = terminal_pin_points(&engine, sheet_id, id, "1");
        let depth = engine.undo_depth();
        engine
            .execute(Command::InsertSpareTerminals {
                sheet_id,
                entity_id: id,
                after: Some("3".to_string()),
                count: 1,
                renumber_following: true,
            })
            .unwrap();
        assert_eq!(engine.undo_depth(), depth + 1, "予備挿入はundo1段だけ積む");
        engine.undo().unwrap();
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!()
        };
        assert_eq!(s.symbol_id, "terminal_block_8p");
        assert_eq!(s.at, before_at, "配置座標も一緒に戻る");
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None);
        assert_eq!(terminal_pin_points(&engine, sheet_id, id, "1"), before_pins);
    }

    /// A multi-level block's inserted spare pole adds one terminal per level ("9a","9b" for a 2-level block).
    /// 多段端子台への予備挿入は、1極につき段数ぶんの端子が増える (2段なら"9a","9b")。
    #[test]
    fn inserting_a_spare_on_a_multilevel_block_adds_one_terminal_per_level() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p_2l");
        engine
            .execute(Command::InsertSpareTerminals { sheet_id, entity_id: id, after: None, count: 1, renumber_following: true })
            .unwrap();
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!()
        };
        assert_eq!(s.symbol_id, "terminal_block_5p_2l");
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None, "末尾挿入は既定と同じ並び");
    }

    /// The insertion command's JSON with `after` omitted (the minimal form MCP/the Link API/the CLI can send) is accepted as an append at the end.
    /// `after`を省略した挿入コマンドのJSON (MCP/Link API/CLIが送れる最小形) は、末尾挿入として通る。
    #[test]
    fn the_insert_spare_terminals_command_accepts_after_omitted() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        let json = format!(r#"{{"type":"insert_spare_terminals","sheet_id":"{sheet_id}","entity_id":"{id}","count":1}}"#);
        let command: Command = serde_json::from_str(&json).unwrap();
        engine.execute(command).unwrap();
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id) else {
            panic!()
        };
        assert_eq!(s.symbol_id, "terminal_block_9p", "afterを省略すると末尾挿入");
    }

    /// Inserting a second spare into a block that already has one works, both at the end and after any terminal, because the command counts the insertion point in the order the chart shows rather than the physical pin order.
    /// 予備を入れた端子台へもう1つ予備を挿入する操作は、末尾でもどの端子の後ろでも通る。挿入位置を物理的なピン順ではなくチャートに出る順で数えるため。
    #[test]
    fn inserting_a_second_spare_into_the_same_block_succeeds() {
        for after in [None, Some("4".to_string()), Some("5".to_string()), Some("9".to_string())] {
            let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
            engine
                .execute(Command::InsertSpareTerminals {
                    sheet_id,
                    entity_id: id,
                    after: Some("3".to_string()),
                    count: 1,
                    renumber_following: true,
                })
                .expect("1回目");
            engine
                .execute(Command::InsertSpareTerminals {
                    sheet_id,
                    entity_id: id,
                    after: after.clone(),
                    count: 1,
                    renumber_following: true,
                })
                .unwrap_or_else(|e| panic!("after={after:?} の2回目が拒否された: {e}"));
            let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id)
            else {
                panic!()
            };
            assert_eq!(s.symbol_id, "terminal_block_10p", "after={after:?}");
            let chart = crate::terminal_chart::terminal_chart(
                engine.project().sheet(sheet_id).unwrap(),
                id,
            )
            .expect("chart");
            let shown: Vec<&str> = chart.rows.iter().map(|r| r.terminal.as_str()).collect();
            assert_eq!(
                shown,
                ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"],
                "after={after:?}: チャートは1〜10の連番で出る"
            );
        }
    }

    /// Inserting a spare into a block renumbered with a prefix keeps the prefix on every label and does not panic (the prefixed labels are not readable as plain pole numbers, which used to overflow the numbering arithmetic).
    /// 接頭辞を付けて採番した端子台への予備挿入は、どのラベルも接頭辞を保ったまま通り、パニックしない (接頭辞付きのラベルは素の極番号として読めず、以前は番号の計算が桁溢れしていた)。
    #[test]
    fn inserting_a_spare_into_a_prefixed_block_keeps_the_prefix() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_8p");
        engine
            .execute(Command::RenumberTerminals {
                sheet_id,
                entity_id: id,
                start: 1,
                step: 1,
                prefix: "A".to_string(),
            })
            .unwrap();
        engine
            .execute(Command::InsertSpareTerminals {
                sheet_id,
                entity_id: id,
                after: Some("A3".to_string()),
                count: 1,
                renumber_following: true,
            })
            .expect("接頭辞付きでも挿入できる");
        assert_eq!(
            terminal_numbers_attr(&engine, sheet_id, id).as_deref(),
            Some("A1,A2,A3,A5,A6,A7,A8,A9,A4")
        );
    }

    /// The "leave the spare with a vacant number" mode writes the spare into the vacant number and leaves every following terminal's number (and so every jumper naming them) untouched.
    /// 「予備を空き番号のまま残す」扱いは、予備を空き番号に入れ、後ろの端子の番号 (とそれを指すジャンパ) を一切変えない。
    #[test]
    fn inserting_a_spare_without_renumbering_leaves_the_following_numbers_alone() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        engine
            .execute(Command::SetTerminalNumbers {
                sheet_id,
                entity_id: id,
                numbers: Some(vec!["1".into(), "2".into(), "3".into(), "5".into()]),
            })
            .unwrap();
        engine.execute(set_attr_command(&engine, sheet_id, id, "jumpers", "3-5")).unwrap();
        engine
            .execute(Command::InsertSpareTerminals {
                sheet_id,
                entity_id: id,
                after: Some("3".to_string()),
                count: 1,
                renumber_following: false,
            })
            .expect("空き番号4へ挿入できる");
        assert_eq!(
            terminal_numbers_attr(&engine, sheet_id, id).as_deref(),
            Some("1,2,3,5,4"),
            "5は繰り下がらず、空いていた4が予備になる"
        );
        assert_eq!(jumpers_attr(&engine, sheet_id, id).as_deref(), Some("3-5"), "ジャンパも変わらない");
    }

    /// In the "leave the numbers alone" mode an insertion whose spare number is already taken is refused without touching the drawing.
    /// 「空き番号のまま残す」扱いで予備の番号が既に使われている挿入は、図面を変えずに拒否される。
    #[test]
    fn inserting_a_spare_without_renumbering_is_refused_when_the_number_is_taken() {
        let (mut engine, sheet_id, id) = terminal_engine("terminal_block_4p");
        assert!(engine
            .execute(Command::InsertSpareTerminals {
                sheet_id,
                entity_id: id,
                after: Some("1".to_string()),
                count: 1,
                renumber_following: false,
            })
            .is_err());
        let Some(Entity::Symbol(s)) = engine.project().sheet(sheet_id).unwrap().entities.get(&id)
        else {
            panic!()
        };
        assert_eq!(s.symbol_id, "terminal_block_4p", "図面は変わらない");
        assert_eq!(terminal_numbers_attr(&engine, sheet_id, id), None);
    }

    /// Inserting a spare keeps every existing terminal at exactly the same coordinates on the sheet whatever the block's rotation or mirroring, so nothing drawn onto those terminals has to move.
    /// 予備を挿入しても既存の端子の用紙上の座標は1mmも動かない (端子台の回転・反転にかかわらず) ので、その端子に描かれていたものは動かさずに済む。
    #[test]
    fn inserting_a_spare_keeps_pin_coordinates_at_every_rotation_and_mirroring() {
        for (rotation, mirror) in [(0u16, false), (90, false), (0, true), (90, true)] {
            let (mut engine, sheet_id, id) =
                terminal_engine_turned("terminal_block_8p", rotation, mirror);
            let before: Vec<Vec<Point>> =
                (1..=8).map(|n| terminal_pin_points(&engine, sheet_id, id, &n.to_string())).collect();
            engine
                .execute(Command::InsertSpareTerminals {
                    sheet_id,
                    entity_id: id,
                    after: Some("3".to_string()),
                    count: 1,
                    renumber_following: true,
                })
                .unwrap();
            // 3の後ろに挿入したので4以降の番号は繰り下がっている (端子nは挿入後 n<=3 ならn、そうでなければn+1)
            for (n, points) in (1..=8).zip(before) {
                let label = if n <= 3 { n } else { n + 1 };
                assert_eq!(
                    terminal_pin_points(&engine, sheet_id, id, &label.to_string()),
                    points,
                    "rot={rotation} mirror={mirror}: 端子{n} (挿入後{label}) の座標が動いた"
                );
            }
        }
    }

    /// Changing a sheet's paper size and orientation (G15) keeps every existing entity at its original coordinates; nothing is moved or deleted.
    /// シートの用紙サイズ・向きを変えても (G15)、既存のエンティティは元の座標のまま動かず、消えもしない。
    #[test]
    fn changing_sheet_paper_keeps_entity_coordinates() {
        let (mut engine, sheet_id) = test_engine();
        let entity = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "R1".into(),
            value: "10k".into(),
            attrs: Default::default(),
        });
        let id = entity.id();
        engine.execute(Command::AddEntity { sheet_id, entity }).unwrap();
        engine
            .execute(Command::SetSheetPaper {
                sheet_id,
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
            })
            .unwrap();
        let sheet = &engine.project().sheets[0];
        assert_eq!(sheet.size, PaperSize::A4);
        assert_eq!(sheet.orientation, Orientation::Portrait);
        let Entity::Symbol(s) = &sheet.entities[&id] else {
            panic!("シンボルであること")
        };
        assert_eq!(s.at, Point::new(100.0, 50.0), "座標は変わらない");
    }

    /// Undoing a paper size change restores both the previous size and orientation.
    /// 用紙サイズの変更をundoすると、サイズと向きの両方が元へ戻る。
    #[test]
    fn undoing_a_paper_change_restores_size_and_orientation() {
        let (mut engine, sheet_id) = test_engine();
        assert_eq!(engine.project().sheets[0].size, PaperSize::A3);
        assert_eq!(engine.project().sheets[0].orientation, Orientation::Landscape);
        engine
            .execute(Command::SetSheetPaper {
                sheet_id,
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
            })
            .unwrap();
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].size, PaperSize::A3);
        assert_eq!(engine.project().sheets[0].orientation, Orientation::Landscape);
    }

    /// Changing the paper leaves the sheet's own zone division alone: a sheet divided into its own number of zones keeps that division.
    /// 用紙を変えてもシート独自のゾーン分割はそのまま: 独自の分割数を持つシートはその分割を保つ。
    #[test]
    fn changing_paper_keeps_the_sheets_own_zone_division() {
        let mut project = Project::new("t");
        project.sheets[0].zone_cols = 8;
        project.sheets[0].zone_rows = 5;
        let sheet_id = project.sheets[0].id;
        let mut engine = Engine::new(project);
        engine
            .execute(Command::SetSheetPaper {
                sheet_id,
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
            })
            .unwrap();
        assert_eq!(engine.project().sheets[0].zone_cols, 8);
        assert_eq!(engine.project().sheets[0].zone_rows, 5);
    }

    /// Undoing a paper change puts the sheet back exactly as it was, so a cross-reference zone address computed on it points at the same zone as before.
    /// 用紙変更をundoするとシートは元どおりになるので、そこで求めたクロスリファレンスのゾーン住所も変更前と同じゾーンを指す。
    #[test]
    fn undoing_a_paper_change_restores_the_zone_addresses() {
        let mut project = Project::new("t");
        project.sheets[0].zone_cols = 8;
        project.sheets[0].zone_rows = 5;
        let sheet_id = project.sheets[0].id;
        let mut engine = Engine::new(project);
        let at = Point::new(200.0, 120.0);
        let before = crate::xref::zone_at(&engine.project().sheets[0], at);
        engine
            .execute(Command::SetSheetPaper {
                sheet_id,
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
            })
            .unwrap();
        engine.undo().unwrap().unwrap();
        assert_eq!(crate::xref::zone_at(&engine.project().sheets[0], at), before);
    }

    /// A paper size change on a sheet that does not exist is refused.
    /// 存在しないシートの用紙変更は断られる。
    #[test]
    fn changing_paper_on_an_unknown_sheet_is_refused() {
        let (mut engine, _sheet_id) = test_engine();
        let missing = Uuid::new_v4();
        assert!(engine
            .execute(Command::SetSheetPaper {
                sheet_id: missing,
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
            })
            .is_err());
    }

    /// `set_project_meta` sets the project name and number in one command, leaving every sheet's title block untouched when those fields are omitted.
    /// `set_project_meta`はプロジェクト名と番号を1コマンドで設定し、その欄を省略すれば全シートの表題欄には触れない。
    #[test]
    fn set_project_meta_sets_name_and_number() {
        let (mut engine, sheet_id) = test_engine();
        let before_company = engine.project().sheets[0].title_block.company.clone();
        engine
            .execute(Command::SetProjectMeta {
                name: Some("まだけ制御盤".into()),
                no: Some(Some("MDK-2026-001".into())),
                company: None,
                drawing_no: None,
                title: None,
            })
            .unwrap();
        assert_eq!(engine.project().name, "まだけ制御盤");
        assert_eq!(engine.project().no.as_deref(), Some("MDK-2026-001"));
        assert_eq!(
            engine.project().sheets[0].title_block.company,
            before_company,
            "表題欄は変わらない"
        );
        let _ = sheet_id;
    }

    /// `set_project_meta` writes the title-block fields (company, drawing number, title) to every sheet in the project with one undoable command.
    /// `set_project_meta`は表題欄の欄(会社名・図番・表題)をプロジェクトの全シートへ1つのundo可能なコマンドで書き込む。
    #[test]
    fn set_project_meta_writes_title_block_to_every_sheet() {
        let mut project = Project::new("t");
        project.sheets.push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let mut engine = Engine::new(project);
        engine
            .execute(Command::SetProjectMeta {
                name: None,
                no: None,
                company: Some("まだけ電機".into()),
                drawing_no: Some("MDK-100".into()),
                title: Some("主回路".into()),
            })
            .unwrap();
        for sheet in &engine.project().sheets {
            assert_eq!(sheet.title_block.company, "まだけ電機");
            assert_eq!(sheet.title_block.drawing_no, "MDK-100");
            assert_eq!(sheet.title_block.title, "主回路");
        }
    }

    /// Undoing `set_project_meta` restores each sheet's own previous title block, even when the sheets had different values beforehand (not a single shared value).
    /// `set_project_meta`のundoは、各シートがそれぞれ異なる元の値を持っていた場合でも、シートごとの元の表題欄を正しく復元する(共通の1値には戻さない)。
    #[test]
    fn undoing_set_project_meta_restores_each_sheets_own_title_block() {
        let mut project = Project::new("t");
        project.sheets[0].title_block.drawing_no = "OLD-1".into();
        project.sheets.push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        project.sheets[1].title_block.drawing_no = "OLD-2".into();
        let mut engine = Engine::new(project);
        engine
            .execute(Command::SetProjectMeta {
                name: None,
                no: None,
                company: None,
                drawing_no: Some("NEW".into()),
                title: None,
            })
            .unwrap();
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].title_block.drawing_no, "OLD-1");
        assert_eq!(engine.project().sheets[1].title_block.drawing_no, "OLD-2");
    }

    /// The project number command is plain JSON, so agents, the CLI and the Link API can all send it.
    /// プロジェクト番号のコマンドは素のJSONで表現できるので、AI・CLI・Link APIのどこからでも送れる。
    #[test]
    fn set_project_meta_command_is_plain_json() {
        let (mut engine, _sheet_id) = test_engine();
        let json = r#"{"type":"set_project_meta","no":"MDK-2026-001"}"#;
        let command: Command = serde_json::from_str(json).unwrap();
        engine.execute(command).unwrap();
        assert_eq!(engine.project().no.as_deref(), Some("MDK-2026-001"));
    }

    /// Undoing the very first project number puts the project back to having no number at all, not to an empty one.
    /// 番号が無かったプロジェクトに初めて番号を付けてundoすると、空の番号ではなく「番号なし」に戻る。
    #[test]
    fn undoing_the_first_project_number_restores_the_unset_state() {
        let (mut engine, _sheet_id) = test_engine();
        assert_eq!(engine.project().no, None, "はじめは番号なし");
        engine
            .execute(Command::SetProjectMeta {
                name: None,
                no: Some(Some("MDK-2026-001".into())),
                company: None,
                drawing_no: None,
                title: None,
            })
            .unwrap();
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().no, None, "空文字ではなく未設定へ戻る");
    }

    /// Sending the project number as null clears it back to unset, which is how a number set by mistake is removed.
    /// プロジェクト番号にnullを送ると未設定へ戻る(間違って付けた番号を消す手段)。
    #[test]
    fn a_null_project_number_clears_it() {
        let (mut engine, _sheet_id) = test_engine();
        engine
            .execute(serde_json::from_str(r#"{"type":"set_project_meta","no":"MDK-2026-001"}"#).unwrap())
            .unwrap();
        engine
            .execute(serde_json::from_str(r#"{"type":"set_project_meta","no":null}"#).unwrap())
            .unwrap();
        assert_eq!(engine.project().no, None);
    }

    /// Undoing a cleared project number brings the previous number back exactly as it was.
    /// 消したプロジェクト番号をundoすると、元の番号がそのまま戻る。
    #[test]
    fn undoing_a_cleared_project_number_brings_it_back() {
        let (mut engine, _sheet_id) = test_engine();
        engine
            .execute(Command::SetProjectMeta {
                name: None,
                no: Some(Some("MDK-2026-001".into())),
                company: None,
                drawing_no: None,
                title: None,
            })
            .unwrap();
        engine
            .execute(Command::SetProjectMeta {
                name: None,
                no: Some(None),
                company: None,
                drawing_no: None,
                title: None,
            })
            .unwrap();
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().no.as_deref(), Some("MDK-2026-001"));
    }

    /// Leaving the project number out of the command leaves an existing number untouched, even while other fields change.
    /// コマンドからプロジェクト番号の欄を省くと、ほかの欄を変えても既にある番号はそのまま残る。
    #[test]
    fn omitting_the_project_number_leaves_it_untouched() {
        let (mut engine, _sheet_id) = test_engine();
        engine
            .execute(serde_json::from_str(r#"{"type":"set_project_meta","no":"MDK-2026-001"}"#).unwrap())
            .unwrap();
        engine
            .execute(serde_json::from_str(r#"{"type":"set_project_meta","name":"まだけ制御盤"}"#).unwrap())
            .unwrap();
        assert_eq!(engine.project().name, "まだけ制御盤");
        assert_eq!(engine.project().no.as_deref(), Some("MDK-2026-001"));
    }

    // --- FreeCAD連携 (対応付け・電線長の書き戻し) -------------------------------

    fn mech_link(entity_id: Uuid, object_name: &str) -> MechLink {
        MechLink {
            entity_id,
            fcstd_path: "/work/panel.FCStd".into(),
            object_name: object_name.into(),
            synced_at: "2026-09-27T10:00:00Z".into(),
            placement: None,
        }
    }

    /// Registering a FreeCAD link stores it under the entity id, registering again replaces it, removing it takes it away, and each step undoes back to the previous list.
    /// FreeCAD対応付けを登録するとentity idをキーに保存され、再登録で置き換わり、解除で消える。各操作はundoで直前の一覧に戻る。
    #[test]
    fn mech_links_are_upserted_removed_and_undone() {
        let (mut engine, _) = test_engine();
        let id = Uuid::new_v4();
        let patch = engine.execute(Command::SetMechLink { link: mech_link(id, "Relay001") }).unwrap();
        assert!(matches!(&patch.ops[0], PatchOp::MechLinksReplaced { mech_links } if mech_links.len() == 1));
        engine.execute(Command::SetMechLink { link: mech_link(id, "Relay002") }).unwrap();
        assert_eq!(engine.project().mech_links.len(), 1);
        assert_eq!(engine.project().mech_links[0].object_name, "Relay002");
        engine.execute(Command::RemoveMechLink { entity_id: id }).unwrap();
        assert!(engine.project().mech_links.is_empty());
        engine.undo().unwrap();
        assert_eq!(engine.project().mech_links[0].object_name, "Relay002");
        engine.undo().unwrap();
        assert_eq!(engine.project().mech_links[0].object_name, "Relay001");
        engine.undo().unwrap();
        assert!(engine.project().mech_links.is_empty());
        assert!(matches!(
            engine.execute(Command::RemoveMechLink { entity_id: id }),
            Err(CoreError::EntityNotFound(_))
        ));
    }

    /// Writing measured wire lengths back sets each wire's length and marks it as measured by FreeCAD in one undo step; a write-back that changes nothing costs no undo step at all.
    /// 計測した電線長の書き戻しは、各配線の長さを設定して出所を「FreeCAD計測」にする1回の編集になる。何も変わらない書き戻しはundoの段を作らない。
    #[test]
    fn wire_lengths_write_back_with_their_source() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let wire_id = wire.id();
        engine.execute(Command::AddEntity { sheet_id, entity: wire }).unwrap();
        let patch = engine
            .execute(Command::SetWireLengths {
                sheet_id,
                lengths: vec![WireLength { wire_id, length_m: Some(1.25), source: LengthSource::Freecad }],
            })
            .unwrap();
        assert_eq!(patch.ops.len(), 1);
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&wire_id] else { panic!() };
        assert_eq!((w.length_m, w.length_source), (Some(1.25), LengthSource::Freecad));
        // 同じ値をもう一度: 変更なし (patchは空・revisionもundo履歴も動かない)
        let before = engine.revision();
        let history = engine.history_origins().len();
        let patch = engine
            .execute(Command::SetWireLengths {
                sheet_id,
                lengths: vec![WireLength { wire_id, length_m: Some(1.25), source: LengthSource::Freecad }],
            })
            .unwrap();
        assert!(patch.ops.is_empty());
        assert_eq!((engine.revision(), engine.history_origins().len()), (before, history));
        // 空振りの段が無いので、undo1回で手入力の値へ戻る
        engine.undo().unwrap();
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&wire_id] else { panic!() };
        assert_eq!((w.length_m, w.length_source), (Some(0.4), LengthSource::Manual));
    }

    /// A write-back naming a wire that is not on the sheet changes nothing: the other wires of the same batch keep their lengths and no undo step is created.
    /// シートに無い配線が混ざった書き戻しは何も変えない。同じ一括分の他の配線も元のままで、undoの段もできない。
    #[test]
    fn wire_lengths_write_back_is_all_or_nothing() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let wire_id = wire.id();
        engine.execute(Command::AddEntity { sheet_id, entity: wire }).unwrap();
        let revision = engine.revision();
        let history = engine.history_origins().len();
        assert!(matches!(
            engine.execute(Command::SetWireLengths {
                sheet_id,
                lengths: vec![
                    WireLength { wire_id, length_m: Some(9.9), source: LengthSource::Freecad },
                    WireLength { wire_id: Uuid::new_v4(), length_m: Some(1.0), source: LengthSource::Freecad },
                ],
            }),
            Err(CoreError::EntityNotFound(_))
        ));
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&wire_id] else { panic!() };
        assert_eq!((w.length_m, w.length_source), (Some(0.4), LengthSource::Manual));
        assert_eq!((engine.revision(), engine.history_origins().len()), (revision, history));
    }
}
