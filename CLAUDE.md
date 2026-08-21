# MadakeCAD

産業用電気図面CAD(Tauri 2 + Vue 3 + Rust)。JIS準拠の電気配線図の作成、配線検証・回路シミュレーション、AIによる自動作図を目標とする。

## 必読ドキュメント

- 要件定義書: `docs/requirements.md`(背景・成功基準・機能/非機能要件)
- 全体設計(スペック): `docs/superpowers/specs/2026-08-20-madakecad-design.md`(設計判断の記録)
- システム設計(構成図・ファイルマップ・状態遷移・変更レシピ): `docs/architecture.md`
- 技術スタック: `docs/tech-stack.md` / データ設計: `docs/data-model.md`
- 機能インベントリ(実装済み・未実装の棚卸し): `docs/features.md`
- 環境構築: `docs/setup.md`
- 実装計画: `docs/superpowers/plans/` 配下(日付順)
- 開発方針の原点と参考図面: `docs/references/README.md`(社外秘情報を含むためREADMEごとgit管理外。各自ローカルに保持)

## アーキテクチャの絶対原則

**ドキュメントへの全ての編集は madake-core の `Command` として `Engine::execute()` を通すこと。**
モデル(`Project`/`Sheet`/`Entity`)を直接書き換えるコードを追加してはならない。UI(Tauri IPC)、MCPサーバー、将来のヘッドレス実行は全て同じCommand APIを共有し、undo/redo履歴とpatch配信(`doc:patch`イベント)が一元化されている。新しい編集操作が必要な場合は`Command` enumに追加し、`apply()`で逆コマンドを返すこと。

```
UI操作/MCPツール → Command(JSON) → Engine(madake-core) → Patch(JSON) → broadcast → UI/クライアント反映
```

## 構成

- `src-tauri/crates/madake-core` — ドキュメントモデル、Commandエンジン(undo/redo)、JISシンボルライブラリ、ファイルIO。UI非依存。ロジックは原則ここに置く
- `src-tauri/crates/madake-mcp` — 内蔵MCPサーバー(rmcp 3.x / Streamable HTTP、127.0.0.1:9310/mcp)。`SharedDoc`がUIとMCP共通の編集入口
- `src-tauri/crates/madake-cli` — `madake` コマンド(bin名`madake`)。Link APIを叩くだけの薄いクライアントで、ロジックは持たない
- `src-tauri/src` — Tauri本体(IPCハンドラ、MCP起動、patchのwebview転送)
- `src/` — Vue 3 + TypeScript + Pinia。図面キャンバスはCanvas2D自作レンダラ(SVG/WebGL不使用)

## コマンド

```bash
npm run tauri dev                 # アプリ起動(vite + cargo)
cd src-tauri && cargo test        # Rustテスト(コアはここに集中)
npx vue-tsc --noEmit              # フロント型チェック
```

ターミナルから起動中のアプリを操作する`madake` CLI(廃止したコマンドラインUIの代替):

```bash
cargo run -p madake-cli -- status                    # 接続確認+概要 (cargo install --path crates/madake-cli で madake として常用)
cargo run -p madake-cli -- netlist [--sheet <ID>]
cargo run -p madake-cli -- verify [--sheet <ID>]     # 図面検証 (ERC+電気検証)
cargo run -p madake-cli -- parts [<検索語>] [--category <c>]  # 部品DB検索
cargo run -p madake-cli -- sim [--open SW1,K1]       # DC動作点シミュレーション
cargo run -p madake-cli -- export svg|pdf|bom|wire-list <path> [--sheet <ID>]
cargo run -p madake-cli -- save|open <path.mdkproj>
cargo run -p madake-cli -- exec <commands.json>      # Command配列JSON → POST /api/v1/commands
cargo run -p madake-cli -- undo|redo
```

共通オプション`--port`(既定9310)/`--json`(生JSON)。使い方の詳細は`README.md`。

Rustは`~/.cargo/bin`にある(rustup)。PATHに無ければ `export PATH="$HOME/.cargo/bin:$PATH"`。

## 座標系・単位

- 用紙座標系: mm単位、左上原点、Y下向き。全エンティティ座標はこれ
- グリッド/ピンピッチ: 2.5mm。シンボル定義のピンは必ず2.5mmグリッド上
- 回転は0/90/180/270のみ

## ドメイン用語(参照図面の慣習)

- 線径は「sq」(mm2断面積: 0.3sq, 0.75sq, 3.5sq等)、線色+sqから電線品番を引く(`Project::wire_parts`)
- 図枠=JIS(表題欄・改訂欄・ゾーン番号)。目標品質はユーザー提供の参考図面(社外秘、`docs/references/README.md`参照)
- シンボルはJIS C 0617系。参照記号接頭辞: R/F/C/D/SW/PB/K(リレー)/J(コネクタ)等

## デザインシステム

`docs/design-system.md`が共通デザインルールの正(メイン画面のCAD調トークン: ribbon-bg背景+白カード+acad-blueアクセント)。実装の色定義は`src/App.vue`のCSS変数と`src/canvas/theme.ts`のみ。コンポーネントへの生色コード直書き禁止。

**デザイン作業の手順(必須)**: 新しい画面・UIをデザインするときは、
1. まず`MadakeCAD.pen`の「デザインシステム - 共通コンポーネント」ボードと`docs/design-system.md`を参照し、既存のトークン・コンポーネントを再利用する
2. 足りないコンポーネントがあれば、**先にデザインシステムボードへ追加**(命名・状態バリエーション含む)してから画面で使用する
3. 追加したコンポーネントは`docs/design-system.md`にも同時に記載する(両者は常に同期)

## UIデザインワークフロー

UIはPencil(pen.dev)で先にデザインし、確定後に実装する。デザインファイルはリポジトリ直下の`MadakeCAD.pen`(Pen.appで開く。暗号化されておりRead/Grep不可、必ずpencil MCPツールで読む)。PencilのMCPサーバーは`.mcp.json`の`pencil`(Pen.app起動中のみ接続可)。UIの見た目に関わる実装はデザイン確定前に進めないこと。

## FreeCAD連携(将来フェーズM、設計済み)

内蔵HTTPサーバーにLink API(`/api/v1`、素のJSON REST)を追加し、FreeCADアドオンWB「MadakeCAD Link」から部品対応付け・3D配線ルーティング・電線長の書き戻しを行う(spec §7)。**Link API経由の書き込みも必ずCommandエンジンを通すこと。** entity UUIDが対応付けのキーで、FreeCAD側は`madake_id`カスタムプロパティ、MadakeCAD側は`Project.mech_links`に保存する。電気データはMadakeCAD、ジオメトリ(配置・経路長)はFreeCADがマスタ。

## 開発プロセス

superpowersプラグインの方法論に従う: brainstorming→spec、writing-plans→plan、TDD(red/green)、タスクごとに小さくコミット。仕様変更時はspec/planを先に更新する。

## 注意

- `.mdkproj`は整形JSON。フォーマット変更時は`format_version`を上げてマイグレーションを書く
- rmcp/Tauri等のAPIはバージョン差が大きい。推測せずローカルの`~/.cargo/registry/src`かdocs.rsで確認
- アプリ起動中はMCP(/mcp)とLink API(/api/v1、REST+SSE)がポート9310で生きている。テストで同時起動する場合は`MADAKE_MCP_PORT`で回避
- **UIの実機検証**: `npm run tauri dev`起動中に http://localhost:1420 をブラウザ(Playwright/Browserツール)で開くと、フロントがLink API経由で実バックエンドに接続される。スクリーンショット確認・クリック操作テストはこの経路で行う(ネイティブウィンドウのキャプチャは不要)
