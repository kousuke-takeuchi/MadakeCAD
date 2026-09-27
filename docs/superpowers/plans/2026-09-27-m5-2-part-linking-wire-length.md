# MadakeCAD M5-2 (部品対応付け+電線長書き戻し) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M5仕様のM5-2。FreeCADアドオンから部品の3Dモデルを挿入して`madake_id`で対応付け(MadakeCAD側は`Project.mech_links`)、FreeCADで計測した経路長を配線の`length_m`へ書き戻す(出所フラグで手入力の暗黙上書きを防ぐ)。

**共通ルール:** 全編集はCommand経由(`set_mech_link` / `remove_mech_link` / `set_wire_lengths`を追加)。`format_version` 2→3とマイグレーション(既定値)。tests-as-spec+gen_spec。ドキュメントEN+JA同期。同期は明示操作のみ。

**Spec:** `docs/internal/specs/m5-freecad.md` M5-2、マスタースペック §7.3

## 設計決定(2026-09-27)

- `Wire.length_source: LengthSource {Manual(既定), Freecad}`を`#[serde(default)]`で追加(仕様どおりWire側に出所を持つ。旧ファイルは既定値で開ける)。Rustの全Wire構造体リテラルに`length_source: Default::default()`を補う
- `MechLink`はentity_id・fcstd_path・object_name・synced_at(ISO 8601文字列。FreeCAD側が書く)。1件の登録・解除でもPatchは一覧全体(`mech_links_replaced`)で送る(フロントのミラーが単純)
- `set_wire_lengths`は書き戻し専用の一括コマンド(`update_entity`を配線本数分送るより1回の編集=undo一発で扱いやすく、出所も同時に記録できる)。値が変わらない配線は飛ばし、存在しない配線はエラー
- 手入力の上書き警告はMadakeCAD側UI(プロパティパネル)で行う: 長さ欄に「FreeCAD計測」バッジ、変更時に出所をmanualへ戻してログへ警告(エンジンは拒否しない。ユーザーの判断を尊重)
- アドオンの挿入対象: STEP/IGES/BREP(`Part.read`)とFCStd(`mergeProject`)。それ以外は案内のみ。経路はDraft Wire等の形状を持つ任意のオブジェクトで、`Shape.Length`(mm)→m(1mm単位)
- MCP・CLIは`/commands`・`execute_commands`で新コマンドをそのまま使える(専用口は作らない)

### Task 1: コア(モデル+Command)

- [x] Step 1 (red): `command.rs`テスト(対応付けの登録・置換・解除・undo / 長さ書き戻しの値と出所・変更なしスキップ・未知配線エラー・undo)、`io.rs`テスト(形式2ファイル→対応付け空・出所manual)
- [x] Step 2 (green): `model.rs`(`MechLink`・`LengthSource`・`Project.mech_links`・`Wire.length_source`・FORMAT_VERSION 3)、`command.rs`(3コマンド+`WireLength`+`PatchOp::MechLinksReplaced`)。全クレートgreen

### Task 2: MadakeCAD側UI

- [x] `ipc.ts`型、`document.ts`の`mech_links_replaced`(+テスト)、`propertyCommands.ts::wireUpdateCommand`(長さ変更で出所manual・計測値上書きフラグ、+テスト4件)、`PropertiesPanel.vue`(「FreeCAD計測」バッジ・「3D対応付け」行・上書き警告ログ)、i18n `mech.*`

### Task 3: アドオン

- [x] `linking.py`(部品行・配線行・対応付けコマンド・mm→m換算・経路の突き合わせ・書き戻しコマンド、+テスト7件)、`panel.py`(Parts/Wiresタブ、Insert 3D model / Link selected object / Unlink / Link selected route / Measure routes → write back lengths)

### Task 4: ドキュメント

- [x] m5仕様(M5-2完了・未決事項の表示方法を決定)、data-model(形式3・MechLink・length_source)、11-mechanical-integration(EN+JA)、roadmap、feature-inventory、アドオンREADME(EN+JA)、gen_spec見出し、仕様書再生成

## 実施記録 (2026-09-27)

- Rust 39スイートgreen、vitest 525件、アドオン22テスト、`gen_spec --check`最新
- **実FreeCADでの目視確認はユーザー確認事項**: 3Dモデル挿入(STEP)と`madake_id`付与、Link selected object、Draft Wireの対応付けと「Measure routes → write back lengths」→MadakeCADの電線リスト・プロパティのバッジ反映、手入力上書き時の警告
- `.pen`ボード(バッジ・3D対応付け行)への反映は次回のPencil作業時

## 受け入れ基準

- FreeCADで引いた経路長がMadakeCADの電線リスト・電圧降下検証に反映される(`length_m`に入るため既存の帳票・検証がそのまま使う)
- 往復してもmadake_idで再同期できる(対応付けは両側に保存: FreeCAD側`madake_id`、MadakeCAD側`mech_links`)
- 計測値を手で上書きしたときに警告が出て、出所が手入力に戻る
