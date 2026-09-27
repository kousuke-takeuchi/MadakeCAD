# MadakeCAD M5-3 (経路同期・配置同期) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M5仕様のM5-3。①ネットリストからFreeCADへ経路スタブを生成(MadakeCAD→FreeCAD)、②ネット行の選択でFreeCADの3Dビューをハイライト(可視化同期)、③FreeCADの配置をMadakeCADへ書き戻す(`MechLink.placement`。将来の2D盤レイアウトシートのデータ)。

**共通ルール:** 書き込みはCommand経由(`set_mech_link`)。`format_version` 3→4。tests-as-spec+gen_spec。ドキュメントEN+JA同期。同期は明示操作のみ。

**Spec:** `docs/internal/specs/m5-freecad.md` M5-3(本日起草)

## 設計決定(2026-09-27)

- 2D盤レイアウトシート(M4 §8)は未実装かつPencilデザインが必要なため、M5-3では**データと操作**(配置の保存・表示)まで。盤シートは`placement`を初期配置として読む前提で設計を残す
- 経路スタブは`Part::Feature`の2点ポリゴン(Draft WBに依存しない)。`madake_id`=配線idを付けるので、M5-2の計測・書き戻しがそのまま使える
- ネット内の割り当ては「対応付け済み部品を名前順に鎖でつなぎ、区間を`wire_ids`へ順に割り当てる」ヒューリスティック(仕様に明記)。冪等性は既存の`madake_id`で判定
- `MechPlacement`は`Option`+`serde(default)`で追加(旧ファイル互換)。回転はZ軸まわりのdegのみ(盤面配置に十分。3D姿勢の完全同期は対象外)

### Task 1: コア

- [x] Step 1 (red): `model.rs`/`io.rs`テスト: `placement`付き`MechLink`のJSON往復、形式3ファイル(placement無し)が`placement: None`で開く
- [x] Step 2 (green): `MechPlacement`、`MechLink.placement`、FORMAT_VERSION 4

### Task 2: アドオン(純Python+パネル)

- [x] Step 1 (red): `tests/test_link_routes.py`: 経路スタブ(両端対応付け済みのみ / 鎖の割り当て / 余りの扱い / 既存スタブはスキップ / 配置座標の引き渡し)、ネットのハイライト対象(部品+経路オブジェクト、重複なし)、配置→`set_mech_link`コマンド(mm・deg丸め)
- [x] Step 2 (green): `routes.py`、`panel.py`(Wiresタブ「Create route stubs」、Netlist行選択→`FreeCADGui.Selection`、Partsタブ「Sync placements」)

### Task 3: MadakeCAD側UI+ドキュメント

- [x] `ipc.ts`型、プロパティの「3D対応付け」行に配置表示、m5仕様(済)、data-model(形式4)、11-mechanical-integration(EN+JA)、roadmap(M5完了)、feature-inventory、アドオンREADME、gen_spec見出し、仕様書再生成

## 実施記録 (2026-09-27)

- Rust 39スイート、vitest、アドオン29テスト(routes 7件追加)、`gen_spec --check`最新
- **実FreeCADでの目視確認はユーザー確認事項**: 経路スタブの生成と再実行の冪等性、ネット行選択での3Dビュー選択、Sync placements→MadakeCADプロパティの配置表示と保存
- `.pen`ボードへの反映(プロパティの配置表示)は次回のPencil作業時

## 受け入れ基準

- 対応付け済み部品どうしを結ぶ配線ごとに経路スタブが1本でき、再実行で増えない
- ネット行の選択でFreeCADの該当オブジェクトが選ばれる
- 「Sync placements」後、MadakeCADのプロパティに配置が出て、保存ファイルに残る
