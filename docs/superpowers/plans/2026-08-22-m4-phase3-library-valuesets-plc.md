# MadakeCAD M4フェーズ3 (シンボルライブラリ拡充+マクロ値セット+PLC I/O) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M4優先順の残りから3項目: ①同梱シンボルライブラリ拡充(JIS C 0617主要記号を50種規模へ、仕様§7の一部) ②マクロ値セット(プレースホルダ、仕様§2の残り) ③PLC I/O(仕様§3)。シンボルエディタGUI・盤レイアウト・図枠テンプレートの実装はフェーズ4。

**共通ルール:** tests-as-spec+gen_spec。i18n。Command絶対原則。ドキュメントEN+JA同期。デザインは確定済み(.pen「M4デザイン - PLC I/O」等)。

## 設計決定

- **ライブラリ拡充**: JIS C 0617/IEC 60617の主要記号を追加(目標: コンタクタ主接点・サーマルリレー・MCB/MCCB・遮断器・変圧器・ブザー/ベル・表示灯色種・切替スイッチ・リミットスイッチ・近接/光電センサ・ソレノイド・モータ種・電源種・接地種・ヒューズ種等で計50種規模)。全て2.5mmグリッドピン+方向付き接続点+属性スロット。**形状に自信の無い記号は追加しない**(一般に確立した形のみ。参考にした記号名を仕様文へ)。部品挿入ダイアログのカテゴリ整理も同時に
- **値セット**: マクロ形式へ `placeholders: [{key, label{en,ja}, targets:[{entity_ref, field(value|attrs.X), …}]}]` と `value_sets: [{id, label, values:{key: value}}]` を追加。挿入時に値セット選択→対象フィールドへ一括適用(execute_batch内)。UI=マクロ挿入プレビューの値セットドロップダウン(デザイン確定済み)+保存ダイアログでのプレースホルダ指定(v1=既存attrs/valueから選ぶ簡易UI)
- **PLC I/O**: デザイン確定済み(割付表エディタ+生成設定+モジュールライブラリ+ラダーページ)どおり:
  - 部品DB v4: `plc_module`(点数・種別DI/DO・アドレス体系プレフィックス)。サンプル3メーカ
  - 動的シンボル `plc_di_{n}p`/`plc_do_{n}p`
  - I/O割付表(プロジェクト保存: `Project.plc_assignments`、format_version注意)+CSV入出力
  - 図面生成: 生成設定(ラダー形式・ラング間隔・配置方針3種)→execute_batchでI/O図面シートを生成(1ターンundo)
  - 双方向同期: 図面の結線→割付表の接続先・線番列(読み取り導出)。I/Oレポート(帳票機構re利用)

### Task 1: シンボルライブラリ拡充

- [ ] Step 1 (red): Rustテスト: 新規各記号のピン(グリッド上・方向・番号)/属性スロット/カテゴリ分類/既存図面の後方互換(既存シンボルidの定義不変)
- [ ] Step 2 (green): 記号追加(まとまりごとに小コミット)+部品挿入ダイアログのカテゴリ整理+i18n名。SVGサンプル出力で目視確認→gen_spec→コミット

### Task 2: マクロ値セット

- [ ] Step 1 (red): Rustテスト: placeholders/value_setsの形式・検証/挿入時の一括適用(value・attrs)/値セット無し・不正参照の扱い/undo一発維持。TS: 挿入プレビューの値セット選択→apply引数/保存ダイアログのプレースホルダ指定
- [ ] Step 2 (green): 実装(macros.rs拡張+UI)。実機確認(値セット付きマクロ→挿入で定格一括設定)→コミット

### Task 3: PLC I/O(コア)

- [ ] Step 1 (red): Rustテスト: 部品DB v4移行/動的シンボルplc_di_{n}p/割付表モデル(format_version)+CSV入出力/接続先・線番の導出/生成設定→ラダーページ生成(配置方針3種・ページ分割)/undo一発/I/Oレポート
- [ ] Step 2 (green): 実装。gen_spec→コミット

### Task 4: PLC I/O(UI)

- [ ] Step 1 (red): TS: 割付表エディタ(編集→Command/CSV読込/生成設定の組み立て)
- [ ] Step 2 (green): デザインどおり実装(割付表エディタ+生成設定ダイアログ、リボン配線)。実機確認(16点DI割付→図面生成→結線変更が表へ反映→undo)→コミット

### Task 5: 仕上げ

- [ ] 受け入れ: 新記号での作図・BOM・SVGが既存同様に動く/値セット付きモータマクロで定格一括設定/CSVから16点DI→図面生成→レポート一致。docs(03/07他EN+JA)・feature-inventory・roadmap更新。全テストgreen+後始末

## 受け入れ基準

- 新記号50種規模が配置・配線・BOM・SVG/PDFで既存記号と同等に機能
- 値セット選択で関連フィールドが一括設定され、挿入はundo一発のまま
- CSV→割付表→I/O図面生成→結線変更→割付表/レポート反映の往復が成立
