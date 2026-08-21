# 技術スタック

作成: 2026-08-21。バージョンは`package.json` / `src-tauri/Cargo.toml`(lock: `Cargo.lock`)が正で、本書は選定理由の記録。

## 全体構成

```
┌──────────────────────────── Tauri 2 アプリ ────────────────────────────┐
│  Vue 3 + TS + Pinia (src/)      Rust (src-tauri/)                      │
│  ├ Canvas2D自作レンダラ    IPC   ├ madake-core   モデル/Command/検証/sim │
│  ├ Piniaストア(patchミラー)◄───► ├ madake-mcp    MCP + Link API (axum)  │
│  └ リボン/パネル/ダイアログ       ├ madake-agent  AIチャット(claude CLI)  │
│                                  └ Tauri本体     IPC・patch転送         │
└────────────────────────────────────────────────────────────────────────┘
     ▲ HTTP 127.0.0.1:9310            ▲ サブプロセス
     │ /mcp (AI)・/api/v1 (REST+SSE)  │
  Claude Code / FreeCADアドオン(将来) ngspice(シミュレーション)
  madake CLI / ブラウザ検証
```

## フロントエンド

| 技術 | バージョン | 選定理由 |
|---|---|---|
| Vue 3 + TypeScript | vue ^3.5 / ts ~5.6 | ユーザー選好。Composition APIとPiniaの相性 |
| Pinia | ^4.0 | patchミラー型の状態管理(ストアはRustからのpatchのみを信頼) |
| Vite / Vitest | ^6.0 / ^4.1 | Tauri標準の開発サーバ+同系テストランナー |
| Canvas2D(自作レンダラ) | - | SVG/WebGL不採用。印刷品質は別経路(svg.rs)で担保し、画面は速度とスナップ制御を優先。依存最小 |
| lucide-vue-next | ^1.0 | アイコン。Pencilデザインと同じlucide名で対応付く |
| @tauri-apps/plugin-dialog | ^2.7 | ネイティブのファイル開く/保存ダイアログ |

## Rust(バックエンド)

| 技術 | バージョン | 選定理由 |
|---|---|---|
| Tauri | 2.x (lock: 2.11) | 軽量シェル。Rustコアと同居、将来のWin/Linux移植 |
| serde / serde_json | 1 | Command・モデル・patchの全シリアライズ基盤 |
| uuid | 1 (v4) | EntityId/SheetId。FreeCAD連携の対応付けキーにもなる |
| schemars | 1 | Command等のJSONスキーマ生成(MCPツールの入力スキーマ公開) |
| rmcp | 3.1.4 | 公式Rust MCP SDK。Streamable HTTPサーバ |
| axum | 0.8 | Link API(REST+SSE)。rmcpと同一Routerに同居 |
| rusqlite | 0.40 (bundled) | 部品DB。bundledでSQLiteを同梱しOS非依存 |
| dirs | 6 | 部品DBの既定パス(OSアプリデータdir) |
| svg2pdf + usvg + fontdb | 0.13 / 0.45 / 0.23 | PDF出力。既存SVGをベクタのまま変換、日本語フォント埋め込み。バージョン組はレジストリで互換確認済み |
| clap | 4 | madake CLIの引数解釈 |
| reqwest (blocking) | 0.13 | madake CLI→Link APIのHTTPクライアント |
| thiserror | 2 | エラー型 |

## 外部ツール(実行時・任意)

| ツール | 用途 | 必須か |
|---|---|---|
| ngspice (46+) | 電気検証の電流・電圧計算、DC動作点シミュレーション。サブプロセス`ngspice -b`で実行、`MADAKE_NGSPICE`→PATH→OS既定パスの順に探索 | 任意(未導入時: 検証はグラフ近似へフォールバック、シミュレーションは導入案内エラー) |
| Claude Code CLI | アプリ内AIチャットのバックエンド(Pro/Max OAuth再利用、自アプリのMCPへ自己接続) | チャット利用時のみ |
| Pen.app (Pencil) | UIデザイン(`MadakeCAD.pen`)。デザイン先行ワークフロー | 開発時のみ |

## 設計上の不採用・保留

- **SVG/WebGL描画**: 不採用(Canvas2D自作。理由は上記)
- **既製ERCライブラリ**: 存在しない(2026-08調査。ERCはCADのデータモデルに密結合)。自前実装+将来`kicad-cli sch erc`クロスチェック
- **libngspice FFI**: サブプロセス方式を採用(バージョン差・クラッシュ隔離・OS非依存で有利)
- **AutoCAD風コマンドラインUI**: 廃止(2026-08-20)。ショートカット+AIチャット+`madake` CLIで代替

## バージョン管理の規約

- 外部crateのAPIは推測せず、ローカルレジストリ(`~/.cargo/registry/src`)かdocs.rsで確認してから使う(CLAUDE.md)
- `.mdkproj`・部品DBのスキーマ変更は`format_version`/`schema_version`を上げ、マイグレーションとテストを書く
