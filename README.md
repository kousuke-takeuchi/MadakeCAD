# MadakeCAD

産業用電気図面CAD。JIS準拠の電気配線図(図枠・表題欄・改訂欄・ゾーン番号、電線品番/線色/線径管理)の作成と、配線検証・回路シミュレーション、AIによる自動作図を目標とする。

## アーキテクチャ

全ての編集操作はシリアライズ可能なCommandとして単一のエンジン(madake-core)で実行される。UI(Tauri IPC)とAI(内蔵MCPサーバー)は同じCommand APIを共有し、編集結果はpatchイベントとして全クライアントに配信される。

```
UI操作/MCPツール → Command(JSON) → madake-core → patch(JSON) → UI再描画
```

- `src-tauri/crates/madake-core` — ドキュメントモデル、Commandエンジン(undo/redo)、シンボルライブラリ、ファイルIO
- `src-tauri/crates/madake-mcp` — 内蔵MCPサーバー(rmcp / Streamable HTTP)とLink API(/api/v1)
- `src-tauri/crates/madake-cli` — `madake` コマンド(Link APIのターミナルクライアント)
- `src-tauri/src` — Tauri本体(IPCハンドラ、MCP起動、patch転送)
- `src/` — Vue 3 + TypeScript フロントエンド(現在は仮画面。UIデザインはPencilで作成中)

## 開発

前提: Rust(rustup)、Node.js。任意: [ngspice](https://ngspice.sourceforge.io/)(図面検証の電流・電圧をDC動作点解析で判定する。macOS: `brew install ngspice` / Linux: `apt install ngspice` / Windows: 公式インストーラ。未導入でも検証はグラフ近似で動作し、Info診断で近似モードと表示される。実行ファイルは環境変数`MADAKE_NGSPICE`→PATH→OS既定パスの順で探索)。

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

- `get_project` / `list_symbols` — 読み取り(`connector_{n}p`/`terminal_block_{n}p`の動的IDでピン数可変シンボルも配置可)
- `place_symbol` / `draw_wire` — 配置・配線
- `execute_commands` — 任意コマンド列(シート追加、表題欄設定、移動、削除など)
- `export_svg` / `export_pdf` / `export_bom` / `export_wire_list` / `get_netlist` — 出力・ネットリスト
- `run_verification` — 図面検証(ERC+電気検証)。Diagnostic配列を返す
- `search_parts` / `upsert_part` / `delete_part` — 部品DB(グローバル共有マスタ)の検索・登録・削除
- `undo` / `redo`

## madake CLI (ターミナル)

起動中のアプリにLink API(`http://127.0.0.1:9310/api/v1`)で接続する薄いクライアント。編集系は必ずCommandエンジンを通るので、CLIからの変更もundo/redoでき、画面に即反映される。

```bash
cd src-tauri && cargo install --path crates/madake-cli   # madake がPATHに入る
# または開発中は: cargo run -p madake-cli -- <サブコマンド>
```

```bash
madake status                       # 接続確認 + 図面の概要
madake project                      # シート一覧・電線品番
madake netlist [--sheet <シートID>]  # ネットリスト
madake verify [--sheet <シートID>]   # 図面検証 (ERC+電気検証。省略時は全シート)
madake parts [<検索語>] [--category <カテゴリ>]  # 部品DB検索
madake export svg|pdf|bom|wire-list <出力パス> [--sheet <シートID>]  # --sheetはsvg/pdfのみ
madake save <path.mdkproj>          # 保存
madake open <path.mdkproj>          # 読み込み
madake exec <commands.json>         # Command配列を実行(Commandエンジン経由)
madake undo / madake redo
```

共通オプション: `--port <番号>`(既定9310。`MADAKE_MCP_PORT`で起動した場合に指定)、`--json`(整形せず生JSONを出力。jq等との連携用)。

`exec`に渡すJSONは`Command`の配列。例:

```json
[{ "type": "add_sheet", "name": "動力系統", "size": "A3", "orientation": "Landscape" }]
```

アプリ未起動時は「MadakeCADアプリが起動していません」と表示して終了コード1を返す。

## 部品DB

グローバル共有の部品マスタ(SQLite)。既定の場所はOSのアプリデータフォルダ(`~/Library/Application Support/MadakeCAD/parts.sqlite`等)で、環境変数`MADAKE_PARTS_DB`で変更できる。型番・メーカ・定格(検証エンジンのcurrent_aと連動)・購入先/データシートURL・価格と、電線品番マスタ(線色+sq→品番)を持つ。初回作成時にダミー型番のサンプルが数件入る。部品挿入ダイアログ・`madake parts`・MCP・`/api/v1/parts`から利用できる。

## 保存形式

`.mdkproj` = 整形JSON(git差分可読)。

KiCad 8/9の回路図(`.kicad_sch`)は`madake open <path.kicad_sch>`・UIの「開く」・MCPツール`import_kicad`で読み込める(ジオメトリ・表題欄・ワイヤ・ラベル・主要シンボルを変換し、未対応シンボルはスキップ報告)。KiCadと本ライブラリはシンボルのピン形状が異なるため、取り込み後は検証(ERC)で未接続を洗い出して手直しする運用。

## ロードマップ

1. フェーズ0(完了): Commandエンジン、MCPサーバー、Tauri足場
2. フェーズ1(完了): Canvas2DエディタUI、ネットリスト、BOM/電線リスト、PDF/SVG出力、動的シンボル、表示クラス
3. フェーズ2(完了): 検証エンジン(ERC+ngspice電気検証)、部品DB(SQLite)、KiCadインポート
4. フェーズ3: ngspiceシミュレーション
5. フェーズM: FreeCAD連携 — SOLIDWORKS Electrical⇔SOLIDWORKS相当の電気・機械連携。内蔵サーバーのLink API(/api/v1)+FreeCADアドオンWBで、部品の3D対応付け・3D配線ルーティング・電線長の還元を行う(spec §7)
