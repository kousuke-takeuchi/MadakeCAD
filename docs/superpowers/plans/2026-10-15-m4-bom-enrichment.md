# M4 BOM拡張 (部品DBからメーカ・品名・単価・購入先を引き当てる) 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 部品表 (BOM) の各行の型番を部品DBで引き当て、メーカ・品名・単価・通貨・金額・購入先の列を足す
(仕様 `docs/internal/specs/m4-industrial-core.md` §5「BOM拡張」。M4から先送りしていた項目)。

**Architecture:** 部品DBを読むのは madake-core の`PartsDb`だが、帳票 (`reports`/`report_sheet`/`pdf`) は
プロジェクトだけを見る純関数のまま保つ。間に**引き当て表**`BomCatalog` (型番→`CatalogEntry`) を挟み、
帳票関数はそれを引数で受け取る。DBを開いている層 (Tauri IPC・MCP・Link API) が出力の直前に
`PartsDb::bom_catalog`で作って渡す。DBが無い・開けないときは空の表を渡すだけで、列は空欄になる
(帳票の生成は失敗しない)。ドキュメントは書き換えないので Command は増えない。

**Tech Stack:** Rust (madake-core / madake-mcp / Tauri) のみ。フロントエンドとデザインの変更は無い
(BOMの図面シートは既存の汎用帳票表で描くので、列が増えるだけ)。

## ベンチマーク (出典)

- ACADE: BOMの列は ITEM / QTY / CATALOG / MFG / DESCRIPTION など。カタログDB (`default_cat.mdb`) で
  部品番号を引き当てた結果が帳票に出る — [Bill of Material Data Fields to Report](https://help.autodesk.com/cloudhelp/2025/ENU/AutoCAD-Electrical/files/GUID-D7D515A7-AE42-4994-9BD3-D966B8344BD9.htm) /
  [About Assigning Catalog Information to Components](https://help.autodesk.com/cloudhelp/2025/ENU/AutoCAD-Electrical/files/GUID-298FFE38-D700-4171-8005-616B1B92EA94.htm)
- EPLAN: 部品データはメーカ・型番・発注番号・説明に加え、通貨ごとの購入単価 (`ARTICLE_PURCHASEPRICE_1/2`、
  価格単位あたり) を持つ — [Eplan API: ARTICLE_PURCHASEPRICE_1](https://www.eplan.help/en-US/infoportal/content/api/2.9/Eplan.EplApi.MasterDatau~Eplan.EplApi.MasterData.MDPartsDatabaseItemPropertyList~ARTICLE_PURCHASEPRICE_1.html) /
  [Download commercial data](https://www.eplan.help/en-us/Infoportal/Content/EDP_Cloud/Content/htm/DataPortalCS_h_Commdatadownload.htm)

## 決定事項

1. 列 (CSVと図面シートで同一): `アイテム番号, 参照記号, 型番/値, シンボル, 数量, メーカ, 品名, 単価, 通貨, 金額, 購入先`
   (旧5列のCSVとは非互換。列位置で読んでいるスクリプトは直す)
2. 引き当てのキーは「型番/値」欄の文字列そのもの (前後の空白を除く)。部品行・電線集約行・付属品行の
   すべてが対象。空欄と電線集約の`(未割当)`は引き当てない
3. 部品DBに無い型番は、追加列を空欄にして行はそのまま残す (行を消さない・作らない)
4. 金額 = 数量 × 単価。電線集約行の数量はm単位なので、電線の単価は「1mあたり」として扱う
   (部品DBの単価の単位は登録者の責任。docsに明記)。数値は小数第2位までで、末尾の0は書かない
5. 単価が1つでも引けたときだけ、末尾に**通貨ごとの合計行**を足す (型番/値=`合計`、通貨、金額)。
   単価の無い行があれば合計行の品名欄に「単価未登録 N 行を除く」と書く
6. `report_bytes`の件数 (CSVの行数) は合計行を含む

## Task 1: 引き当て表とBOMの列 (madake-core)

**Files:** `src-tauri/crates/madake-core/src/reports.rs` / `src-tauri/crates/madake-core/src/parts.rs` /
`src-tauri/crates/madake-core/src/report_sheet.rs` / `src-tauri/crates/madake-core/src/pdf.rs`

- [x] **Step 1 (red):** `bom_rows(project, &catalog)`が型番で引き当てた列を出すこと / DBに無い型番は空欄で
      行は残ること / 金額=数量×単価 (電線のm数も) / 通貨ごとの合計行と「単価未登録 N 行を除く」/
      単価が1つも無ければ合計行を出さないこと / 空の表なら追加列が全部空で行数は従来どおり /
      `PartsDb::bom_catalog`が登録済みの型番だけを返すこと / 図面シートとCSVが同じ列であること
- [x] **Step 2 (green):** 実装。`report_bytes`・`report_csv`・`ReportKind::pages`・`PdfBookOptions`に
      引き当て表を通す (PDF一括は`PdfBookOptions.catalog`)
- [x] `gen_spec.py` → コミット

## Task 2: 出力の入口で部品DBから引き当てる (Tauri / MCP / Link API)

**Files:** `src-tauri/src/lib.rs` / `src-tauri/crates/madake-mcp/src/lib.rs` /
`src-tauri/crates/madake-mcp/src/link_api.rs` / docs 04 (EN+JA)・仕様§5・ロードマップ・機能インベントリ

- [ ] **Step 1 (red):** Link API `POST /export/report` (bom, csv) が部品DBの値を列に出すこと
      (テスト用の一時DB) / DBに無い型番は空欄
- [ ] **Step 2 (green):** 3つの入口 (`export_report`・`export_bom`・`export_pdf_book`) で
      `SharedParts`から引き当て表を作って渡す
- [ ] **Step 3:** docs/04 (EN+JA) にBOMの新しい列と非互換を書き、仕様§5・§0表・ロードマップ・
      `feature-inventory.md`の状態文を更新 → `gen_spec.py` → 監査 → コミット
