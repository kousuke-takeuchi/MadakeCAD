# MadakeCAD M4フェーズ4 (図枠テンプレート+シンボルエディタ+2D盤レイアウト) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M4仕様の残り3本柱を実装する。①図枠・表題欄テンプレート(§9)、②シンボルエディタGUI(§7)、③2D盤レイアウト(§8)。デザインは全て確定済み(.pen「M4デザイン - 図枠テンプレート」「M4デザイン - シンボルエディタ」「M4デザイン - 盤レイアウト」— ACADE/EPLAN実画面調査に基づく再作成版)。

**共通ルール:** 全図面編集はCommand経由。tests-as-spec+gen_spec。i18n(en/ja)。ドキュメントEN+JA同期。.mdkproj/部品DBの形式変更はformat_version/schema_version+マイグレーション。

## 設計決定

- **図枠テンプレート(プロットフレーム)**: JSON定義 `{ id, name(en/ja), paper(w,h), frame_margin, zones{cols,rows,col_labels:"numeric",row_labels:"alpha"}, title_block{x,y,w,h相当のセル定義: [{rect, bind|text, font, align}]}, revision_block{anchor:"above_title", rows_max}, logo{rect}? }`。バインド=`{project.name}` `{project.no}` `{sheet.no}` `{sheet.total}` `{sheet.name}` `{rev.latest}` `{date}` `{scale}` `{designer}` `{approver}`+自由プロジェクト変数`{var.X}`
  - 同梱: `jis-a3`(現行ハードコード出力と**ゴールデン等価**をテストで固定してからデータ駆動へ移行)・`jis-a4-portrait`。ユーザー: `~/MadakeCAD/templates/frames/`
  - `Project.frame_template: Option<String>`(format_version++。未指定=jis-a3)。描画(svg.rs/renderer.ts)がテンプレートを参照するため差し替えは自動で全シートへ効く
  - ロゴ: PNGをbase64で.mdkprojへ埋め込まずパス参照+未解決時は枠のみ(M6配布を考慮しテンプレフォルダ相対)
- **シンボルエディタ**: 対象=ユーザーライブラリ`~/MadakeCAD/symbols/*.json`(SymbolDef形式そのまま)。エディタ内はローカル下書き(図面Commandとは別ドメイン)で、保存時に検証(グリッド外ピン・番号重複・スロット欠落)→ファイルへ。部品挿入に「ユーザー」カテゴリ(既存カテゴリ機構へ追加)。回転バリアントは自動導出(既存機構)のためプレビューのみ。「既存シンボルを複製して開始」が既定動線
- **盤レイアウト**: `Sheet.kind: "schematic" | "panel"`(format_version同時変更)。盤専用エンティティ: `PanelOutline{rect,name}` / `DinRail{y,x1,x2}` / `Duct{rect}` / `Footprint{rect, reference(回路図シンボルの参照記号へのリンク), part_no}`。部品DBに`footprint_size(W,H)`(v6)
  - 未配置部品=回路図シンボルのうちpanelシートにFootprintが無い参照記号。ドラッグ配置+レールスナップ
  - フェーズ内後半: バルーン(アイテム番号自動採番・BOM連動)、凡例表挿入、盤内部品配置表(帳票)、レール充填率・間隔チェック(検証Info/Warn)。盤内配線図(wire annotation)はフェーズ5へ
  - パネルシートでは回路図ツールを無効化し盤ツール(リボン「パネル」タブ実装)に切替

### Task 1: 図枠テンプレート(コア)

- [x] Step 1 (red): 現行jis-a3出力のゴールデン固定→テンプレJSON読み込み・検証/バインド解決(全種+未知バインドはそのまま表示しない)/jis-a3テンプレでの出力が現行と等価/A4縦/ユーザーテンプレ列挙/Project.frame_template(format_version互換)
- [x] Step 2 (green): svg.rs/renderer.tsのデータ駆動化(改訂欄・ゾーン・表題欄)。gen_spec→コミット

#### 確定したテンプレートJSONの形(2026-08-23、Task 1で決定)

```jsonc
{
  "id": "jis-a3", "name": "JIS A3 landscape", "name_ja": "JIS A3 横",
  "description": "...", "description_ja": "...",
  "paper": { "w": 420.0, "h": 297.0 },   // 想定用紙(一覧・プレビュー用)。描画はシートの用紙を使う
  "frame_margin": 10.0,
  "zones": { "cols": 4, "rows": 6, "col_labels": "numeric", "row_labels": "alpha" },
  "title_block": {
    "origin": "bottom_right",            // bottom_right | bottom_left | top_right | top_left
    "w": 120.0, "h": 32.0, "row_h": 8.0,
    // 罫線・セルの座標はすべて「表題欄の左上角からの相対mm」(負=枠の外)
    "grid_lines": [ { "x1": 0, "y1": 8, "x2": 120, "y2": 8 }, … ],
    "cells": [
      { "x": 2, "y": 5.5, "w": 22, "h": 8, "text": "図番", "size": 3.0 },
      { "x": 26, "y": 5.5, "w": 92, "h": 8, "bind": "{title_block.drawing_no}  Rev {rev.latest}", "size": 3.0 },
      { "x": -2, "y": 30, "bind": "{title_block.company}", "size": 3.0, "align": "end", "omit_if_empty": true }
    ]
  },
  "revision_block": { "rows_max": 6, "col_w": [14.0, 28.0, 56.0, 22.0] },
  "logo": null                            // { x, y, w, h, path? } 用紙座標mm。未解決なら枠だけ描く
}
```

決定事項:

- **セルは`text`(固定文字)か`bind`(バインド入り)のどちらか一方**。`align`=start/middle/end、`weight`=normal/bold、`omit_if_empty`=値が空ならセルごと描かない(会社名)。`w`/`h`はプレビュー・将来の文字詰め用で描画には未使用
- **バインド**: `{project.name}` `{project.no}`(モデル未実装のため常に空) `{sheet.no}` `{sheet.total}` `{sheet.name}` `{rev.latest}` `{title_block.*}`(company/title/drawing_no/scale/date/designed/drawn/checked/approved/rev)+別名`{date}` `{scale}` `{designer}` `{approver}`。**知らないバインドは空文字**(エラーにしない)
- **ゾーンの分割数はシート(`Sheet.zone_cols/zone_rows`)が正**。テンプレートの`zones.cols/rows`は「そのテンプレートが想定する分割」=新規シートの既定値・プレビュー用。図枠を替えても既存図面のゾーン住所(XRef)が変わらないようにするため。文字種(数字/英字)だけはテンプレートが決める
- **用紙はテンプレートで変わらない**。`paper`は想定用紙の記録で、描画は常にシートの用紙サイズ・向き
- **検証(`validate`)でエラーにするのは図面が破綻する矛盾だけ**: セルのtext/bind同時指定・両方なし、`size<=0`、改訂欄の列数が4以外、列幅合計が表題欄の幅と不一致、ゾーン0、負のマージン、表題欄の寸法0。**許容するもの**: 同じバインドを複数セルに書く/表題欄の外に出るセル(会社名)/知らないバインド
- **等価性の担保**: `tests/golden/frame-jis-a3.svg`・`frame-jis-a3-empty-a4.svg`(データ駆動化前の出力を固定)と**文字列完全一致**をテストで比較。キャンバス側は`src/canvas/frameTemplate.ts`が同じ計算を持ち、`DEFAULT_FRAME_TEMPLATE`は`jis-a3.json`と`toEqual`で同一を強制
- **露出**: IPC`list_frame_templates`/`open_frame_templates_folder`、Link API`GET /api/v1/frame-templates`、変更は`set_frame_template`コマンド(逆コマンド=旧id)。PDFは既存SVG経由で自動
- 未対応(将来): ロゴ画像の実描画(枠のみ)、テンプレートの`frame_margin`変更に`xref.rs`のゾーン住所計算が追従しない(同梱2種はどちらも10mm)

### Task 2: 図枠テンプレート(UI)

- [ ] Step 1 (red): TS: 選択ダイアログstore(一覧/プレビュー用データ/適用=set系Command 1回/適用範囲)
- [ ] Step 2 (green): 選択ダイアログ(デザイン準拠: 一覧+ライブプレビュー+バインド表示+ゾーン設定表示+このプロジェクト/グローバル既定)。リボン「プロジェクト」相当へ起動。実機確認(A4縦へ切替→全シート反映→undo)+コミット

### Task 3: シンボルエディタ(コア)

- [ ] Step 1 (red): ユーザーライブラリの読み書き・列挙/保存時検証3種/複製開始(既存defからコピー)/部品挿入への「ユーザー」カテゴリ合流/既存図面で使用中のユーザーシンボル定義変更の扱い(図面はスナップショット保持=影響なし、をテストで固定)
- [ ] Step 2 (green): 実装+IPC/Link API。gen_spec→コミット

### Task 4: シンボルエディタ(UI)

- [ ] Step 1 (red): TS: エディタstore(下書き・プリミティブ追加/編集・ピン(方向/番号)・スロット・検証結果・保存)
- [ ] Step 2 (green): エディタダイアログ(デザイン準拠: 属性パレット+2.5mmグリッドキャンバス+バリアント帯+チェック+フッタ)。リボン「管理」相当へ起動。実機確認(複製→編集→保存→配置→配線→undo)+コミット

### Task 5: 盤レイアウト(モデル+描画)

- [ ] Step 1 (red): Sheet.kind(format_version互換)/盤エンティティ4種のCommand経由add/update/remove+undo/部品DB v6(footprint_size)/未配置部品の導出/SVG・キャンバス描画(レール・ダクトハッチ・フットプリント+参照記号)
- [ ] Step 2 (green): 実装。gen_spec→コミット

### Task 6: 盤レイアウト(UI)

- [ ] Step 1 (red): TS: パネルタブのツール(レール/ダクト/盤枠の作図)・未配置部品パネル(一覧/ドラッグ配置コマンド)・レールスナップ・フットプリントプロパティ
- [ ] Step 2 (green): リボン「パネル」タブ実装(デザイン準拠)+シート種別選択(シート追加時)+未配置パネル。実機確認+コミット

### Task 7: 盤レイアウト(バルーン・凡例・帳票・チェック)

- [ ] Step 1 (red): バルーン自動採番(アイテム番号・BOM連動)/凡例表挿入/盤内部品配置表(帳票6種目)/間隔・充填率チェック(検証へ)
- [ ] Step 2 (green): 実装+リボン配線。実機確認+コミット

### Task 8: 受け入れと仕上げ

- [ ] 受け入れ: A4縦テンプレ切替が全出力(キャンバス/SVG/PDF)へ効きundo可/ユーザーシンボルを作成→配置→BOM・SVGが既存同等/回路図の全部品を盤へ配置(未配置ゼロ)→バルーン→配置表とBOMのアイテム番号一致/充填率・間隔チェックが検証に出る
- [ ] docs(03/04/07/08該当+新規ページ要否判断、EN+JA)・feature-inventory・roadmap(M4完了へ)。全テストgreen+後始末

## 受け入れ基準(仕様§7/§8/§9より)

- テンプレート差し替えで全シートの図枠・表題欄・改訂欄が一括で変わり、印刷出力も一致
- コンタクタ相当のユーザーシンボルを複製から作成→4方向バリアント→配置・配線・BOM・SVGまで既存同等
- 盤シートで未配置ゼロ→バルーン採番→配置表とBOMが一致
