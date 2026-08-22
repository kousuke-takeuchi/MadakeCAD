# MadakeCAD M4フェーズ1 (端子台チャート+帳票拡充) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M4優先順の先頭2項目を実装する。①端子台チャート(端子台エディタ+チャート表+端子接続図、仕様§1)、②帳票拡充(From-Toワイヤリスト・XRef表・帳票の図面シート化・PDF一括出力、仕様§5)。仕様=`docs/internal/specs/m4-industrial-core.md`、デザイン=`.pen`「M4デザイン - 端子台チャート/帳票様式」「M4デザイン - リボン パネル/レポート/管理タブ」(確定済み)。

**共通ルール:** 全編集はCommand経由(undo必須)。tests-as-spec(対訳仕様文)+タスクごとに`python3 scripts/gen_spec.py`再生成をコミットに含める。新規UI文字列はi18n(en/ja)。ドキュメントはEN正本+JA同期。

## 設計決定(スコープの割り切り含む)

- **端子台の内部/外部**: 貫通端子台の**左側ピン接続=内部側(盤内)、右側ピン接続=外部側(盤外)**と定義(参考図面・盤慣行)。回転した端子台は回転後の左右で判定
- **ジャンパ**: `SymbolInstance.attrs["jumpers"] = "1-2,3-4"`(確定済み)。隣接端子のみ・カンマ区切り・正規化(小-大順)。編集は`update_entity` Command
- **端子台エディタ v1のスコープ**: 行の導出表示(端子/内部側/外部側/線番/電線/ジャンパ)+ジャンパ生成・削除+予備端子表示(未結線端子)+端子台チェック(未結線・ジャンパ不整合)+チャート/接続図の生成起点。**並べ替え・多段端子・アクセサリはモデル拡張が必要なためフェーズ2**(仕様§1の残り)
- **From-Toワイヤリスト**: 電線リストを拡張し列=`From(参照:ピン) / To / 線番 / 色 / sq / 長さ / 品番 / ハーネス`。From/Toはワイヤ両端の接続先(ピン=参照記号:ピン番号、ネットラベル=ラベル名、未接続=空)。決定順: 参照記号昇順でFrom側を選ぶ
- **帳票の図面シート化**: 汎用テーブルレンダラ`report_sheet_svg(title, columns, rows, sheet_meta)`を新設し、図枠+表題欄付きA4横ページとして帳票を描く(複数ページ分割対応)。端子台チャート/From-To/BOM/XRef表に適用。CSVは従来どおり
- **端子接続図**: EPLAN端子図様式(外部=左、内部=右、ケーブル/ハーネスブラケット、予備端子も表示)のSVGページを1端子台=1ページで生成
- **PDF一括出力**: 表紙(プロジェクト名・シート一覧・改訂)+回路図全シート+選択帳票を1PDFへ結合。既存PDF機構を拡張
- **XRef表**: プロジェクト全体ネットのネット/所在一覧(M2の`extract_netlist_project`を利用)。リレーのコイル⇔接点対応はフェーズ2(コイル/接点モデルが未実装のため)
- **UI**: リボン「レポート」タブを実装(プレースホルダ→実タブ。デザイン確定済み: 帳票グループ+端子台グループ+出力グループ)。各帳票ボタン=生成ダイアログ(出力先: CSV/図面シート/PDF、対象シート)。端子台エディタはCAD調ダイアログ

### Task 1: From-Toワイヤリスト(コア)

- [x] Step 1 (red): Rustテスト: ワイヤ両端の接続先解決(ピン/ネットラベル/未接続)/From側の決定性/From-To CSVの列構成/線番・ハーネス列の統合/長さ列は空(M5で書き戻し)
- [x] Step 2 (green): 実装(reports系)。旧形式は温存せず`wire_list_csv`を新ヘッダ`シート,From,To,線番,線色,線径sq,長さm,電線品番,ハーネス`へ置き換え(列の後方互換は割り切り)。From決定ルール=両端表記の辞書順で小さい方、未接続の端は常にTo。gen_spec→コミット

### Task 2: 端子台チャート(コア)

- [x] Step 1 (red): Rustテスト: 1端子台の行導出(端子番号順/内部側=左ピン/外部側=右ピン/線番/電線色・sq・品番/未結線端子は予備表示)/ジャンパattrsの解析・正規化・不正値エラー/チャートCSV/端子台チェック(未結線・存在しない端子へのジャンパ)
- [x] Step 2 (green): terminal_chart系モジュール実装+`update_entity`でのジャンパ編集テスト(undo)。gen_spec→コミット

### Task 3: 帳票の図面シート化+PDF一括

- [x] Step 1 (red): Rustテスト: 汎用テーブルページ(図枠+表題+列見出し+行/ページ分割/長文セルの省略)/端子台チャートのシート化/From-Toのシート化/PDF一括(表紙+回路+帳票のページ数・順序)
- [x] Step 2 (green): report_sheet_svg+PDF結合実装。gen_spec→コミット
  - `report_sheet.rs`(A4横・25行/ページ・列幅は相対比・セルは省略記号で切る)、行データは `reports::bom_rows` / `xref::xref_table_rows` を追加してモジュール側に置いた
  - PDF結合は pdf-writer で実装 (`pdf::svgs_to_pdf` / `project_pdf_pages` / `export_project_pdf`)。ページ寸法のスケール不具合 (96dpi→72pt換算漏れでA3が133%) もあわせて修正
  - 露出: Link API `POST /export/pdf-book`・MCP `export_pdf_book`・CLI `madake export pdf-book --reports ... [--no-cover]`・Tauri IPC `export_pdf_book`
  - 目視確認: `cargo run -p madake-core --example pdf_demo -- --book <path>` (表紙+回路2+帳票5=8ページ)

### Task 4: 端子接続図(グラフィカル)

- [x] Step 1 (red): Rustテスト: 端子ストリップ縦並び/外部=左・内部=右の引出線/ケーブル・ハーネスのブラケットと名前/予備端子/ジャンパ表示/1端子台=1ページ
- [x] Step 2 (green): SVG実装(デザイン「紙 端子接続図」準拠)。gen_spec→コミット
  - `terminal_diagram.rs`: `terminal_diagram_svg(project, tb_id)` = 1端子台1ページ(A4横・端子15個/ページで自動分割、表題に「(端子 1〜15)」)。図枠・表題欄・`fit_text`は`report_sheet`と共有(`page_open`/`report_title_block`/`table_top`/`body_bottom`を`pub(crate)`化)
  - レイアウト: 端子箱24×7mm・ピッチ10mm・引出線34mm・ハーネス帯20mm。**外部側=左・内部側=右**(チャートの内部/外部判定はそのまま、描画で左右を入れ替え)。予備端子は薄塗り+「(予備)」、ジャンパは端子箱の内部側の縁に縦線+「サドルジャンパ」
  - ブラケット: 同一ハーネス名の電線をページ内の最初〜最後の端子で角括弧に括りハーネス名を記す(1本だけなら名前のみ、無所属は何も付けない)
  - 情報源はチャートに一本化: `TerminalRow`に`internal_wire`/`external_wire`/`internal_harness`/`external_harness`を追加
  - 露出: `ReportKind::TerminalDiagram`(JSON `terminal-diagram`)をPDF一括・CLI `--reports`・Link API・MCPへ追加
  - 目視確認: `cargo run -p madake-core --example pdf_demo -- --book <path>`(表紙+回路2+帳票6=9ページ。デモ図面にハーネスW3とTB1のジャンパ1-2を追加)

### Task 5: 端子台エディタUI+リボン「レポート」タブ

- [x] Step 1 (red): vitest: エディタstore(行導出の表示整形/ジャンパ生成・削除→update_entityコマンド/チェック結果表示)/レポートタブの生成ダイアログ(出力先・対象の組み立て)
- [x] Step 2 (green): 端子台エディタダイアログ(デザイン準拠: グリッド+ツールバー(ジャンパ生成/削除・チェック)+フッタ(接続図生成/チャート生成))。リボン「レポート」タブ実装(帳票/端子台/出力グループ)。i18n。実機確認(端子台+配線を作って一連操作→undoで復帰)+コミット
  - コア追加: `terminal_block_infos`(端子台一覧)/`terminal_chart_in_project`・`check_terminal_block_in_project`(entity idで引く)/`terminal_charts_csv`(先頭に端子台列)/`xref_table_csv`/`report_bytes(kind, format, entity_id)`(CSV=行数・PDF=ページ数を返す。端子接続図のCSVは`ReportError::UnsupportedFormat`)
  - 露出: Tauri IPC `list_terminal_blocks` `get_terminal_chart` `check_terminal_block` `export_report`、Link API `GET /terminals` `GET /terminals/chart` `GET /terminals/check` `POST /export/report`。`export_pdf_book`をipc.tsへ追加
  - UI: `TerminalEditorDialog.vue`(グリッド9列。Lv・型番・配置はモデル拡張待ちで非表示、フェーズ2の操作はdisabledプレースホルダ)/`ReportDialog.vue`/`PdfBookDialog.vue`/リボン「レポート」タブ。ストアは`stores/terminals.ts`・`stores/reports.ts`
  - ジャンパは隣接端子のみ・`update_entity`1回・全部外れたら属性ごと削除。リボンのタブ名もi18n化(タブidで管理)
  - 実機確認(http://localhost:1420): TB1(4極)+抵抗2個+配線2本+線番採番 → エディタの行が図面と一致 → ジャンパ1-2生成(グリッド反映)→ チェック(情報3=予備端子3)→ チャートCSV・接続図PDF(1ページ)・チャート図面シートPDF・PDF一括(7ページ=表紙+回路1+帳票5)→ undo連打で空図面へ復帰(can_undo=false・要素0)。英語ロケール表示も確認

### Task 6: 露出と仕上げ

- [x] Step 1: CLI `madake export terminal-chart|from-to <path> [--sheet]`等の追加(薄いクライアント)。MCP説明文更新
  - CLI: `madake export bom|wire-list|terminal-chart|terminal-diagram|xref-table <path> [--format csv|pdf] [--terminal <参照記号|ID>]`(帳票5種は全て`POST /export/report`へ)+`madake terminals [--sheet <ID>]`(端子台一覧)
  - `--format`省略時は出力先の拡張子から判定(`.pdf`→図面シートPDF、他はCSV)。`--terminal`は参照記号を`GET /terminals`で引いてentity idへ解決(UUID表記はそのまま送る。未発見・複数該当は候補付きで中断)
  - 帳票はプロジェクト全体が対象(`report_bytes`にシート絞り込みが無い)ため、`--sheet`単独指定は`--terminal`を案内して拒否し、`--sheet`は`--terminal`の探索範囲としてのみ効く
  - MCP: ツール`list_terminal_blocks`/`get_terminal_chart`/`check_terminal_block`/`export_report`を追加、`execute_commands`の説明にジャンパ(`update_entity`で`attrs["jumpers"]`)を明記、サーバーinstructionsに帳票5種を追記
  - Link API `GET /terminals*`・`POST /export/report`のエンドポイントテストを追加(tests/link_export.rs)
- [x] Step 2: docs/04・05(EN+JA)へ端子台チャート・From-To・PDF一括を追記、feature-inventory・ロードマップM4を🔶へ。デモ図面に端子台チャートページを追加
  - 更新: docs/04・05・10(EN+JA)、docs/12-roadmap(M4を🔶「フェーズ1完了」へ)、README/README.ja(特徴・現況・仕様項目バッジ)、CLAUDE.mdのCLI節、docs/internal/feature-inventory(帳票・端子台エディタ・MCP/API/CLI行)、m4仕様§0表・§1・§5のステータス
  - デモ(`examples/pdf_demo -- --book`)はTask 4で既に全帳票入り(表紙+回路2+帳票6=9ページ)。図面自体は不変のためsample-drawing.svgは据え置き
- [x] Step 3: 全テストgreen+受け入れ基準の実機検証(下記)→undoで図面復帰

**M4フェーズ1 完了(2026-08-22)。** 端子台チャート(§1)と帳票拡充(§5)が、コア→Link API/MCP/CLI→UIまで一通り繋がった。次はM4の優先順に従い回路マクロ(§2)。

## 受け入れ基準

- TB1(4極)+結線+線番採番後、端子台チャート(CSV/図面シート)が図面の結線・線番・電線と完全一致
- ジャンパ"1-2"を設定するとエディタ・チャート・接続図の3か所に反映され、undoで消える
- From-ToリストのFrom/Toが全ワイヤで正しく、ハーネス・線番列がM2実装と一致
- PDF一括出力が 表紙→回路シート→帳票 の順で1ファイルになる
