# MadakeCAD フェーズ2前半 (検証エンジン: ERC+電気検証) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** spec §3.5の検証エンジンを実装する。ERC(未接続ピン・参照記号・宙ぶらりん配線・ラベル競合)と電気検証(電源到達性・線径適合・電圧降下・ヒューズ簡易チェック)を`Diagnostic`のリストとして返し、MCP/Link API/CLI/UIへ露出する。部品DB(SQLite)とKiCadインポートはフェーズ2後半の別プラン。

**Architecture:** 検証はmadake-coreの純関数(`verify.rs`)。ネットリスト(`extract_netlist`)の上に構築し、モデルは変更しない(読み取りのみ、Command不要)。UI表示はLink API/IPC経由で診断リストを取得して描画。

**Tech Stack:** Rust (madake-core), Vue 3 + TS + Vitest

**Spec:** `docs/superpowers/specs/2026-08-20-madakecad-design.md` §3.5

## Global Constraints

- 検証は読み取り専用の純関数。`Engine`ロックの外でシートのクローンに対して実行してもよい
- テスト: `cd src-tauri && cargo test` / `npx vitest run`。TDD(red→green)
- UI(検証結果パネル)はPencilデザイン先行
- コミットは小さく、日本語、`Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`

## 設計決定

### Diagnostic型 (spec §3.5)

```rust
pub enum Severity { Error, Warning, Info }
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,        // 安定ID (例: "erc.unconnected_pin")
    pub message: String,     // 日本語の説明文
    pub sheet_id: SheetId,
    pub entity_ids: Vec<EntityId>, // UIで選択・ズームする対象
}
pub fn verify_sheet(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Diagnostic>
pub fn verify_project(project: &Project) -> Vec<Diagnostic>  // 全シート分を連結
```

### ERCルール

| code | severity | 内容 |
|---|---|---|
| `erc.unconnected_pin` | Warning | どのネットにも属さないピン。端子台は端子(番号)単位で判定し、左右どちらか一方でも接続されていれば接続扱い |
| `erc.empty_reference` | Warning | 参照記号が空のシンボル |
| `erc.duplicate_reference` | Error | 同一シート内の参照記号重複。ただし**リレー(category=="relay")同士は除外**(コイルK1+接点K1、多接点は正当) |
| `erc.dangling_wire` | Warning | ワイヤ端点がピン・ジャンクション・他ワイヤ端点・ネットラベルのどれにも一致しない |
| `erc.label_conflict` | Error | 1ネットに異なる名前のネットラベルが2つ以上(異電位ネットの直結の代表例) |

### 電気検証ルール(第一版の割り切り)

回路方程式は解かず、ネットリストのグラフ近似で検証する。電流源は**負荷シンボルの属性
`attrs["current_a"]`**(例: "1.5")。未設定の負荷は電流不明としてスキップ(Infoで通知)。

| code | severity | 内容 |
|---|---|---|
| `elec.unreachable_load` | Warning | 電源(batteryシンボル)からワイヤ+2ピン部品(ヒューズ/スイッチ/接点は導通扱い)をたどって到達できない負荷(category=="output"または"relay"のコイル) |
| `elec.wire_ampacity` | Error | ワイヤの`sq`の許容電流 < そのネットの負荷電流合計。許容電流表(第一版、AVS系の慣用値): 0.3sq=7A, 0.5sq=9A, 0.75sq=12A, 1.25sq=16A, 2sq=22A, 3.5sq=33A。表にないsqは線形補間せず直近下位 |
| `elec.voltage_drop` | Warning | `length_m`設定済みワイヤについて、往復抵抗 2×ρ×L/A (ρ=0.0175Ω·mm²/m) × ネット負荷電流合計 が電源電圧(battery.value 例"DC24V"→24、解釈不能なら24V既定)の3%超 |
| `elec.fuse_rating` | Warning | ヒューズ(ref_prefix F)のvalueから定格電流を解釈(例 "5A")し、そのネットの負荷電流合計が定格超過、または定格がワイヤ許容電流超過 |
| `elec.no_current_attr` | Info | 負荷に`current_a`が無く電流計算から除外した |

ヒューズ「協調」(直列ヒューズの選択性)は本プランでは扱わない(部品DB導入後)。

### 露出

- MCPツール `run_verification`(シートID省略可)→ Diagnostic配列JSON
- Link API `GET /api/v1/verify[?sheet_id=]`
- CLI `madake verify [--sheet <ID>]`(人間可読の一覧+`--json`)
- UI: リボン「検証」ボタン(既存、現在todo)→ 検証結果パネル(Pencilデザイン後)。診断行クリックで該当エンティティを選択+ズーム

---

### Task 1: Diagnostic基盤+ERC (madake-core)

**Files:** Create `src-tauri/crates/madake-core/src/verify.rs`。Modify `lib.rs`

- [x] Step 1 (red): テスト: 未接続ピン/空参照/重複参照(リレー除外含む)/宙ぶらりんワイヤ/ラベル競合の5ルール+問題なしシートで空
- [x] Step 2 (green): `verify_sheet`実装(ERCのみ)
- [x] Step 3: cargo test グリーン、コミット

### Task 2: 電気検証 (madake-core)

**Files:** Modify `verify.rs`

- [x] Step 1 (red): テスト: 到達性(スイッチ経由で届く/断線で届かない)、許容電流超過、電圧降下、ヒューズ定格、current_a未設定Info
- [x] Step 2 (green): 導通グラフ(ネット+2ピン部品の橋渡し)と各チェック実装
- [x] Step 3: cargo test グリーン、コミット

### Task 3: API/CLI露出

**Files:** Modify `madake-mcp/src/lib.rs`・`link_api.rs`、`madake-cli`(verifyサブコマンド)、README/CLAUDE.md

- [x] Step 1 (red): link_api統合テスト: `/api/v1/verify`が診断JSONを返す
- [x] Step 2 (green): MCP `run_verification` / Link API / CLI `madake verify` 実装、ドキュメント更新
- [x] Step 3: 全テストグリーン、コミット

### Task 4: UI: 検証結果パネル(デザイン先行)

- [x] Step 1: Pencilデザイン: 検証結果パネル(場所・行構成・severity色は状態色トークン)+リボン検証ボタンの結果バッジ
- [x] Step 2 (red): vitest: 診断ストア(取得・選択連動)
- [x] Step 3 (green): 実装(リボン「検証」→パネル表示、行クリックで選択+ズーム)、実機ブラウザ検証、コミット

## 進捗

- 2026-08-21: プラン作成。Task 1〜4完了。cargo test 19スイート+vitest 101件+vue-tscグリーン。
  実機検証済み: CLI `madake verify` とリボン「検証」→結果パネル→行クリックで選択+ズームを確認。
  割り切り(第一版): 電流はattrs[current_a]の合計+導通部品越しmax伝播の近似。ヒューズ協調は部品DB後
