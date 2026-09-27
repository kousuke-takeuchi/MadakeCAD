# MadakeCAD M5-1 (FreeCADアドオンWB「MadakeCAD Link」骨格) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M5仕様のM5-1。FreeCAD 1.0+のワークベンチとして、起動中のMadakeCADにLink APIで接続し、プロジェクト概要とネットリストをパネルに表示し、図面編集にライブ追従する骨格を作る。M5-2(部品対応付け・電線長書き戻し)の土台になる。

**共通ルール:** MadakeCAD側の変更なし(既存Link APIのみ)。書き込みは`POST /api/v1/commands`だけ(Commandエンジン経由)。tests-as-spec+gen_spec(Python版の規約を追加)。ドキュメントEN+JA同期。

**Spec:** `docs/internal/specs/m5-freecad.md` M5-1、マスタースペック §7

## 設計決定(2026-09-27)

- **配置**: リポジトリ直下`freecad-addon/`(仕様どおり)。FreeCADの`Mod`へコピー/シンボリックリンクで導入。`package.xml`を同梱しAddon Manager登録(M6)に備える
- **層の分離**: FreeCAD/Qtに依存しない純Python(`client.py`=標準ライブラリだけのREST+SSEクライアント、`model.py`=概要・ネットリスト行、`events.py`=再読込判定、`settings.py`=ポート)と、FreeCAD内でしか動かない`panel.py`(QDockWidget+QThreadでSSE購読)・`commands.py`・`InitGui.py`を分ける。CIにFreeCADは無いので**テスト可能な範囲を最大化**する
- **ライブ追従**: `GET /api/v1/events`のpatchを`PatchFollower`でrevision順に受け、`refresh_needed`(表示中シートの要素変更・シート構成変化・プロジェクト置換)のときだけ再読込。それ以外はrevision表示だけ更新
- **テスト規約(Python)**: `def test_…`直後のdocstring2行(英/日)。`scripts/gen_spec.py`に`PY_SOURCES`と`parse_py`を追加し、`docs/13-specification.md`に「FreeCAD add-on」節を生成。CIに`freecad`ジョブ(`python3 -m unittest discover -s freecad-addon/tests`)
- UI文言は英語(FreeCAD側の流儀。翻訳はAddon Manager登録時に検討)。Pencil対象外(仕様どおり)

### Task 1: 純Python(client / model / events / settings)+テスト

- [x] Step 1 (red): `tests/test_link_client.py`(base_url / health・project / netlist・partsのクエリ / commands POST / 接続拒否・HTTPエラー・不正JSONのLinkError / SSEのpatch受信 / parse_sse)、`tests/test_link_model.py`(概要・件数・空スナップショット / ネットリスト行・ピン表記)、`tests/test_link_events.py`(revision順・再読込判定・ポート正規化)。フェイクLink API(`_fake_server.py`、`http.server`)
- [x] Step 2 (green): `madakecad_link/client.py`・`model.py`・`events.py`・`settings.py`

### Task 2: ワークベンチ(FreeCAD側)

- [x] `InitGui.py`(`MadakeCADLinkWorkbench`、ツールバー/メニュー)、`Init.py`、`package.xml`、`resources/madakecad_link.svg`、`madakecad_link/commands.py`(`MadakeCAD_ShowPanel` / `MadakeCAD_Refresh`)、`madakecad_link/panel.py`(ポート・Connect・Follow live・状態行・シート選択・ネットリスト表。`EventThread`がSSEを読みGUIスレッドへpatchを渡す)。`python3 -m py_compile`で構文確認

### Task 3: 仕様書生成・CI・ドキュメント

- [x] `scripts/gen_spec.py`(Python規約)、`.github/workflows/ci.yml`(`freecad`ジョブ)、`CLAUDE.md`(構成・コマンド・テスト規約)、`freecad-addon/README(.ja).md`、`docs/11-mechanical-integration(.ja).md`、`docs/12-roadmap(.ja).md`(M5-1完了)、`docs/internal/specs/m5-freecad.md`、`docs/internal/feature-inventory.md`、`README(.ja).md`、`docs/13`再生成

## 実施記録 (2026-09-27)

- 純Python 15テストgreen(`python3 -m unittest discover -s freecad-addon/tests`)。`gen_spec.py --check`最新
- **実FreeCADでの目視確認はユーザー確認事項**(このコンテナにFreeCADは無い): ワークベンチの表示、Connect→概要・ネットリスト表、MadakeCAD側で配線を足したときの自動再読込、Follow live OFF/ON、MadakeCAD終了時の状態行

## 受け入れ基準

- FreeCADのパネルから起動中MadakeCADの図面概要とネットリストが見える
- MadakeCADで図面を編集すると、表示中シートのネットリストが自動で更新される
- MadakeCADが起動していないときは、原因の分かる文面(ポート番号つき)が状態行に出る
