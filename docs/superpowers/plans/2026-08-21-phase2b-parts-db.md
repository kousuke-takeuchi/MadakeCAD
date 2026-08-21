# MadakeCAD フェーズ2後半 (部品DB: SQLite) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to実装 this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** グローバル共有の部品DB(SQLite)を実装する。部品マスタ(型番・メーカ・定格・購入先/データシートURL・価格)と電線品番マスタを一元管理し、部品挿入ダイアログからDB部品を検索・選択して配置(型番・定格が図面へ自動設定)できるようにする。

**要件(ユーザー決定 2026-08-21):** グローバル共有DB / 基本項目+データシートURL+価格+電線品番もDBへ / UIは部品選択連携まで / サンプル数件同梱。

**Architecture:** DBは**ドキュメント外**の共有マスタなのでCommandエンジンを通らない(絶対原則の対象はドキュメント編集)。図面へは「配置時に型番(value)・定格(attrs.current_a)を書き込む」形で連携し、.mdkprojは自己完結を維持する(既存`Project::wire_parts`は図面側スナップショットとして残し、DBがマスタ)。DBアクセスはmadake-coreの`parts`モジュール(rusqlite 0.40 bundled、OS非依存)。

**Tech Stack:** rusqlite 0.40 (bundled) / dirs 6 (アプリデータdir) — レジストリで確認済み

## 設計決定

- **DBパス**: 環境変数`MADAKE_PARTS_DB` → `dirs::data_dir()/MadakeCAD/parts.sqlite`。`PartsDb::open(path)`はパス注入可能(テストは一時ファイル)
- **スキーマv1** (metaテーブルにschema_version、将来マイグレーション):
  - `parts`: part_no(UNIQUE), maker, name, category, symbol_id(既定シンボル・動的ID可), rated_voltage(表記のままTEXT), rated_current_a(REAL、検証エンジンのcurrent_aと連動), purchase_url, datasheet_url, price(REAL)+currency, note, model_3d(予約・フェーズM), mounting(予約)
  - `wire_parts`: part_no(PK), color, sq, purchase_url, price_per_m, note
- **サンプルデータ**: 新規作成時のみ投入(ダミー型番 MDK-FUSE-5A / MDK-RLY-24V / MDK-TB-8P / MDK-LAMP-24V / MDK-CONN-3P、電線 MDK-W-*)
- **露出**: Link API `GET /api/v1/parts?query=&category=` / `POST /api/v1/parts`(upsert) / `DELETE /api/v1/parts/{part_no}` / `GET・POST /api/v1/wire-parts`。MCPツール `search_parts` / `upsert_part`。CLI `madake parts [<query>]`
- **配置連携**: 部品挿入ダイアログに「部品DB」検索セクション(Pencilデザイン先行)。部品選択→配置で `symbol_id`のシンボルを `value=part_no`、`attrs.current_a=rated_current_a` 付きで配置

### Task 1: PartsDbコア (madake-core/parts.rs)

- [ ] Step 1 (red): テスト: open→スキーマ作成+サンプル投入(新規時のみ)、upsert/get/search(部分一致・カテゴリ)、delete、wire_parts CRUD+色sq検索、再open冪等
- [ ] Step 2 (green): rusqlite実装、cargo test グリーン、コミット

### Task 2: API/CLI露出

- [ ] Step 1 (red): link_api統合テスト: GET/POST/DELETE /parts(一時DBパス)
- [ ] Step 2 (green): SharedParts状態をLink API/MCP/Tauriへ配線、MCPツール、CLI `madake parts`
- [ ] Step 3: 全テストグリーン、README/CLAUDE.md更新、コミット

### Task 3: UI: 部品DB検索から配置 (デザイン先行)

- [ ] Step 1: Pencilデザイン: 部品挿入ダイアログに「部品DB」セクション(検索結果行: 型番・名称・メーカ・定格。選択で配置)
- [ ] Step 2 (red): vitest: partsストア(検索)、controllerの部品付き配置(value/attrs反映)
- [ ] Step 3 (green): ipc(parts_search)+ダイアログ実装、実機ブラウザ検証(DB部品を配置→プロパティに型番・定格)、コミット

## 進捗

- 2026-08-21: プラン作成
