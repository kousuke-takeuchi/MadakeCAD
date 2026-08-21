# MadakeCAD フェーズ2終盤 (KiCadインポート) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** KiCad 8/9の回路図(`.kicad_sch`、S式)を本モデルへ変換して開けるようにする(spec §3.3)。既存KiCadプロジェクトからの移行の起点を提供する。

**Architecture:** パーサと変換はmadake-core(`kicad.rs`)の純関数。読み込みは既存の`load_project`と同様に「新しいProjectでエンジンを置き換える」経路(Commandではない)。

## 設計決定(v1の割り切り)

- **S式パーサは自前実装**(トークナイザ+再帰下降、依存追加なし。`.kicad_sch`はクォート文字列・数値・シンボルのみで文法が小さい)
- **ジオメトリはそのまま**: KiCadの座標はmm・Y下向きで本モデルと同じ。ワイヤ(2点)・ジャンクション・ラベル(local/global→NetLabel)・テキスト・用紙サイズ(paper)・表題欄(title_block)を変換
- **シンボルはlib_idマッピング表**で対応付け(Device:R→resistor、Device:Fuse→fuse、Connector_Generic:Conn_01xNN→connector_{N}p、Screw_Terminal_01xNN→terminal_block_{N}p等)。Reference/Valueプロパティを反映
- **既知の制限(v1)**: KiCadと本ライブラリでシンボルのピン形状・オフセットが異なるため、**配置位置は保たれるがピンとワイヤ端の一致は崩れ得る**(ERCの未接続/宙ぶらりん警告で洗い出して手直しする運用)。未対応lib_idのシンボルはスキップし、`ImportReport`(変換数・スキップ一覧・警告)で報告する
- **露出**: `import_kicad(path) -> (Project, ImportReport)`(core)。Link API `POST /api/v1/import/kicad {path}`(エンジン置き換え+patch)、MCPツール`import_kicad`、CLI `madake open`は`.kicad_sch`も受け付ける。UIの「開く」ダイアログもフィルタに追加

### Task 1: S式パーサ (madake-core/kicad.rs)

- [x] Step 1 (red): テスト: アトム/文字列(エスケープ)/数値/入れ子リストのパース、不正入力のエラー
- [x] Step 2 (green): 実装、コミット

### Task 2: .kicad_sch → Project変換

- [x] Step 1 (red): テスト: 最小の.kicad_schフィクスチャ(paper/title_block/wire/junction/label/symbol R+Conn_01x03)→ Sheet内容・マッピング・Reference/Value・ImportReport(未対応lib_idのスキップ報告)
- [x] Step 2 (green): 実装、コミット

### Task 3: 露出(Link API/MCP/CLI/UI)

- [x] Step 1 (red): link_api統合テスト: POST /import/kicad がプロジェクトを置き換えreportを返す
- [x] Step 2 (green): 実装+CLI open拡張+UIの開くフィルタ、実機検証(サンプル.kicad_schを開いて表示)、ドキュメント更新、コミット

## 進捗

- 2026-08-21: プラン作成。Task 1〜3完了。cargo test全ワークスペース+vitest 103件+vue-tscグリーン。
  実機検証済み: サンプル.kicad_schを`madake open`で読み込み(シンボル4/配線4/ラベル2、Q_NPNスキップ報告)、
  ブラウザで表示確認、ERCが未接続を想定通り警告(F1はピン位置一致で接続再現)。フェーズ2完了
