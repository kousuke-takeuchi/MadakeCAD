# はじめに: 環境構築

**English (canonical): [02-getting-started.md](02-getting-started.md)**

macOSでの手順(コアはOS非依存だが、現状の動作確認はmacOSのみ)。

## 1. 必要ソフトウェア

| ソフトウェア | 必須 | 入手 | 確認コマンド |
|---|---|---|---|
| Rust (rustup) | ✅ | https://rustup.rs | `cargo --version` |
| Node.js 20+ | ✅ | https://nodejs.org / nvm | `node --version` |
| ngspice | 任意 | `brew install ngspice` | `ngspice --version` |
| Claude Code CLI | 任意 | https://claude.com/claude-code | `claude --version` |
| Pen.app (Pencil) | 開発時 | https://pen.dev | - |

- **ngspice**: 電気検証の実回路解析とDCシミュレーションに使用。未導入でも検証は近似モードで動作(シミュレーションは導入案内エラー)。Linux: `apt install ngspice` / Windows: 公式インストーラ(Spice64)。既定パス以外は環境変数`MADAKE_NGSPICE`で実行ファイルを指定
- **Claude Code CLI**: アプリ内AIチャット用(Pro/MaxのOAuthセッションを再利用。APIキー不要)
- rustupの`cargo`は`~/.cargo/bin`に入る。PATHに無ければ `export PATH="$HOME/.cargo/bin:$PATH"`

## 2. セットアップ

```bash
git clone <このリポジトリ>
cd MadakeCAD
npm install
```

動作確認(テスト):

```bash
cd src-tauri && cargo test          # Rust(初回はビルドで数分)
cd .. && npx vitest run             # フロント
npx vue-tsc --noEmit                # 型チェック
```

アプリ起動:

```bash
npm run tauri dev                   # vite(1420) + cargoビルド + ネイティブウィンドウ
```

起動中は `127.0.0.1:9310` で内蔵サーバー(MCP `/mcp`・Link API `/api/v1`)が生きている。

## 3. madake CLI(任意)

```bash
cd src-tauri && cargo install --path crates/madake-cli   # `madake` がPATHへ
madake status                                            # 起動中アプリへの接続確認
```

## 4. AI連携(任意)

- **Claude Codeから図面編集**: リポジトリの`.mcp.json`で自動接続される(アプリ起動中に`claude`を開くだけ)
- **アプリ内チャット**: `claude`にサインイン済みであればそのまま使える(設定→AI設定で実行ファイルパス変更可)

## 5. UI開発の検証経路

`npm run tauri dev` 起動中に http://localhost:1420 をブラウザで開くと、フロントがLink API経由で実バックエンドに接続される。スクリーンショット・クリック操作の検証はこの経路で行う(ネイティブウィンドウのキャプチャ不要)。

UIの見た目を変える場合は先に`MadakeCAD.pen`でデザイン(ワークフローは[design-system](internal/design-system.md)参照)。

## 6. トラブルシューティング

| 症状 | 原因と対処 |
|---|---|
| `cargo: command not found` | rustup未導入 or PATH。`export PATH="$HOME/.cargo/bin:$PATH"` |
| ポート9310が使用中 | 別インスタンスが起動中。`MADAKE_MCP_PORT=19310 npm run tauri dev`で回避(CLIは`--port 19310`) |
| ポート1420が使用中 | 前回のviteが残留。該当プロセスをkillして再起動 |
| 検証結果に「近似モードで判定しました」(Info) | ngspice未検出。`brew install ngspice`(検出順: `MADAKE_NGSPICE`→PATH→OS既定パス) |
| シミュレーションが「ngspiceが見つかりません」 | 同上(シミュレーションはフォールバックしない仕様) |
| ブラウザで「起動エラー: Failed to fetch」 | アプリ(バックエンド)より先にページを開いた。アプリ起動後にリロード |
| CLIが「アプリが起動していません」 | `npm run tauri dev`を先に起動。別ポート時は`--port` |
| 部品DBを初期化したい | アプリ終了後に `~/Library/Application Support/MadakeCAD/parts.sqlite` を削除(再作成時にサンプル再投入) |
| テストとアプリを同時に動かしたい | テスト側は9310を使わない設計だが、ポート衝突時は`MADAKE_MCP_PORT`で分離 |
| Pen.appのMCPに繋がらない | Pen.appを起動してから(`.mcp.json`の`pencil`はアプリ起動中のみ接続可) |
