# MadakeCAD フェーズ1後半 (端子台/コネクタ動的シンボル+PDF出力) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** フェーズ1の積み残しを完了する。(1) 端子台・コネクタのピン数可変シンボル(参考図面の端子台結線表現に必須)、(2) 印刷品質のPDF出力。レイヤはUIデザイン(Pencil)确定後の別タスクとする。

**Architecture:** 全編集はCommand API経由(CLAUDE.md絶対原則)。動的シンボルは「symbol_idの命名規則から `SymbolDef` を生成する解決関数」としてmadake-coreに置き、ネットリスト/SVG/MCP/フロントの全消費者が同じ解決を通る。PDFは既存の`sheet_to_svg`出力をsvg2pdfで変換する(SVGが単一の描画ソース)。

**Tech Stack:** Rust (svg2pdf 0.13 / usvg 0.45 / fontdb 0.23 — ローカルregistryで検証済みのバージョン組), Vue 3 + TS + Vitest

**Spec:** `docs/superpowers/specs/2026-08-20-madakecad-design.md` §3.3(端子台/コネクタのピン数可変シンボル)

## Global Constraints

- 座標はmm・左上原点・Y下向き。ピンは2.5mmグリッド上。回転0/90/180/270のみ
- テスト: `cd src-tauri && cargo test` / `npx vitest run`
- UI見た目の変更(部品挿入ダイアログのピン数入力、リボンのPDFボタン、レイヤ)はPencilデザイン先行。本プランではコア+API+CLIまでを実装し、UIタスクはデザイン確定後
- コミットは小さく、日本語メッセージ、`Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`

## 設計決定

### 動的シンボルID規則

- `connector_{n}p`(例: `connector_6p`): コネクタ、ピンはボックス右側、番号1..n
- `terminal_block_{n}p`(例: `terminal_block_8p`): 端子台、各端子は左右両側に接続点(貫通)、番号1..n
- n は 1..=50。ピッチ5mm(2.5mmグリッド倍数)、縦並び、**中央揃え**(オフセット `(i - (n-1)/2) * 5.0` は常に2.5mm倍数)
- 既存静的 `connector_2p` は削除し動的生成に置換。n=2の中央揃え座標は旧定義と完全一致(ピン(7.5,±2.5)、枠(-4,-5)-(4,5))なので既存図面は影響なし
- `resolve_symbol(id) -> Option<SymbolDef>`: builtin優先→動的パターン解析。全消費者(netlist/svg/MCP/Link API/agent/フロント)がこれを使う
- `sheet_symbol_defs(sheet) -> Vec<SymbolDef>`: builtin + シート内で使用中の動的IDの定義を集める(`&[SymbolDef]`シグネチャの既存関数へ渡す用)

### 端子台の電気的意味

端子台の1端子は左右2つの接続点を持ち、**同一ピン番号**を与える(例: 左右とも "3")。
ネットリスト抽出で「同一シンボルインスタンス内の同一ピン番号の接続点」をunionし、
貫通端子として左右を内部短絡する。BOM/電線リストには従来どおり `T1:3` の形で現れる。

### PDF出力

- `madake-core/src/pdf.rs`: `sheet_to_pdf(sheet, symbols) -> Result<Vec<u8>, PdfError>`。実装は `sheet_to_svg` → usvgパース(fontdbでシステムフォント読込、sans-serifファミリをmacOSでは日本語グリフを持つ`Hiragino Sans`へ割当) → `svg2pdf::to_pdf`
- 露出: MCPツール `export_pdf`、Link API `GET /api/v1/export/pdf?sheet=`、CLI `madake export pdf <path>`
- UIのリボン「PDF出力」ボタンはUIタスク(デザイン確定後)

---

### Task 1: ネットリスト: 同一ピン番号の内部短絡 (madake-core)

**Files:** Modify `src-tauri/crates/madake-core/src/netlist.rs`

- [ ] Step 1 (red): テスト「同一シンボルの同番号ピン2点が別ワイヤに繋がるとき同一ネットになる」
- [ ] Step 2 (green): `extract_netlist`で同一entity_id+同一pin番号のピンノードをunion。Netの`pins`には重複排除して1つだけ載せる
- [ ] Step 3: `cargo test -p madake-core` グリーン、コミット

### Task 2: 動的シンボル解決 (madake-core)

**Files:** Modify `symbol.rs`(生成関数+resolve)、`lib.rs`(re-export)、`svg.rs`/`netlist.rs`は変更不要(`&[SymbolDef]`のまま)。Modify `madake-mcp/src/lib.rs`・`link_api.rs`・`agent.rs`(builtin_symbols直参照を置換)

- [ ] Step 1 (red): テスト: `resolve_symbol("terminal_block_8p")`が8端子・左右16接続点・グリッド上・中央揃え、`resolve_symbol("connector_2p")`が旧静的定義と同一座標、`resolve_symbol("connector_0p")`/`"connector_51p"`/`"connector_p"`はNone
- [ ] Step 2 (green): `dynamic_symbol()`+`resolve_symbol()`+`sheet_symbol_defs()`実装。静的`connector_2p`を削除
- [ ] Step 3: MCP `place_symbol`のID検証と`get_netlist`/`export_svg`/Link API/agentの`builtin_symbols()`を`resolve_symbol`/`sheet_symbol_defs`へ置換。`list_symbols`の説明文に動的ID規則を追記
- [ ] Step 4: 全cargo testグリーン、コミット

### Task 3: フロントの動的シンボル解決 (Vue/TS)

**Files:** Modify `src/canvas/renderer.ts`(defs Map→resolver)、`src/stores/document.ts`。Create `src/canvas/dynamicSymbol.ts` + vitest

- [ ] Step 1 (red): vitest: TS版`resolveSymbol`がRustと同じ座標を返す(terminal_block_3p / connector_2p の代表値)
- [ ] Step 2 (green): Rust生成ロジックをTSへ移植(単純な純関数)。レンダラ・SymbolPreviewの定義解決を静的Map+動的フォールバックに変更
- [ ] Step 3: `npx vitest run` + `npx vue-tsc --noEmit` グリーン、コミット

### Task 4: PDF出力コア (madake-core)

**Files:** Create `src/pdf.rs`。Modify `Cargo.toml`(svg2pdf/usvg/fontdb追加)、`lib.rs`

- [ ] Step 1 (red): テスト: `sheet_to_pdf`の出力が`%PDF-`で始まり非自明なサイズ、表題欄テキスト入りシートで成功する
- [ ] Step 2 (green): svg2pdf変換実装(フォントDB初期化は`OnceLock`で1回)
- [ ] Step 3: cargo test グリーン、コミット

### Task 5: PDFのAPI/CLI露出

**Files:** Modify `madake-mcp/src/lib.rs`(`export_pdf`ツール)、`link_api.rs`(`/export/pdf`)、`madake-cli`(`export pdf`)、`README.md`・CLAUDE.mdのコマンド一覧

- [ ] Step 1 (red): link_apiの統合テスト(既存export系テストに倣う)で`/api/v1/export/pdf`が`application/pdf`を返す
- [ ] Step 2 (green): 実装(SVG export系のコードパスに並べる)
- [ ] Step 3: CLI `madake export pdf out.pdf [--sheet <ID>]`実装+README更新、全テストグリーン、コミット

### Task 6: UI(デザイン確定後・別途)

- [ ] Pencilデザイン: 部品挿入ダイアログの「ピン数」入力(端子台/コネクタ選択時)、リボン「PDF出力」ボタン、レイヤ表示トグルの要否検討
- [ ] デザイン確定後に実装(SymbolPickerDialog.vue / RibbonBar.vue)

## 進捗

- 2026-08-21: プラン作成。実装開始
