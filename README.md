# MadakeCAD

産業用電気図面CAD。JIS準拠の電気配線図(図枠・表題欄・改訂欄・ゾーン番号、電線品番/線色/線径管理)の作成と、配線検証・回路シミュレーション、AIによる自動作図を目標とする。

## アーキテクチャ

全ての編集操作はシリアライズ可能なCommandとして単一のエンジン(madake-core)で実行される。UI(Tauri IPC)とAI(内蔵MCPサーバー)は同じCommand APIを共有し、編集結果はpatchイベントとして全クライアントに配信される。

```
UI操作/MCPツール → Command(JSON) → madake-core → patch(JSON) → UI再描画
```

- `src-tauri/crates/madake-core` — ドキュメントモデル、Commandエンジン(undo/redo)、シンボルライブラリ、ファイルIO
- `src-tauri/crates/madake-mcp` — 内蔵MCPサーバー(rmcp / Streamable HTTP)
- `src-tauri/src` — Tauri本体(IPCハンドラ、MCP起動、patch転送)
- `src/` — Vue 3 + TypeScript フロントエンド(現在は仮画面。UIデザインはPencilで作成中)

## 開発

前提: Rust(rustup)、Node.js。

```bash
npm install
npm run tauri dev
```

テスト:

```bash
cd src-tauri && cargo test
```

## MCP連携

アプリ起動中、`http://127.0.0.1:9310/mcp`(環境変数`MADAKE_MCP_PORT`で変更可)でMCPサーバーが待ち受ける。Claude Codeはこのリポジトリの`.mcp.json`で自動接続され、以下のツールで図面を直接編集できる:

- `get_project` / `list_symbols` — 読み取り
- `place_symbol` / `draw_wire` — 配置・配線
- `execute_commands` — 任意コマンド列(シート追加、表題欄設定、移動、削除など)
- `undo` / `redo`

## 保存形式

`.mdkproj` = 整形JSON(git差分可読)。KiCad `.kicad_sch` インポートはフェーズ2で対応予定。

## ロードマップ

1. フェーズ0(完了): Commandエンジン、MCPサーバー、Tauri足場
2. フェーズ1: Canvas2DエディタUI(Pencilデザイン確定後)、ネットリスト、BOM/電線リスト、PDF/SVG出力
3. フェーズ2: 配線検証(電圧降下・線径適合)、部品DB(SQLite)、KiCadインポート
4. フェーズ3: ngspiceシミュレーション
5. フェーズM: FreeCAD連携 — SOLIDWORKS Electrical⇔SOLIDWORKS相当の電気・機械連携。内蔵サーバーのLink API(/api/v1)+FreeCADアドオンWBで、部品の3D対応付け・3D配線ルーティング・電線長の還元を行う(spec §7)
