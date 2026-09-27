# MadakeCADへの貢献

[English (canonical)](CONTRIBUTING.md)

関心を持っていただきありがとうございます。MadakeCADは産業用電気図面CAD(Tauri 2 + Vue 3 + Rust)で、**ドキュメント先行・デザイン先行・テスト先行**で開発しています。ここは短い案内で、詳しくは[`docs/internal/`](docs/internal/README.md)を参照してください。

## 基本ルール

1. **図面への編集はすべて`Command`**として`madake-core`の`Engine::execute()`を通します。`Project` / `Sheet` / `Entity`を直接書き換えてはいけません(`CLAUDE.md`「アーキテクチャの絶対原則」)。UI・MCP・REST・CLI・FreeCADアドオンが同じ経路を共有するので、undo/redoとpatch配信が一元化されます。
2. **テストが仕様書です。** すべてのテストに英日1行ずつの仕様文を付けます(Rust: `#[test]`直前の`///`2行 / Vitest: `it("…")`+`// ja:`コメント / Python: 2行のdocstring)。`docs/13-specification.md`は`python3 scripts/gen_spec.py`で生成し、CIで鮮度を確認します。テストの無い挙動は存在しないものとして扱います。
3. **ドキュメントは英語が正本**、日本語版は`*.ja.md`。新規文書は両方を書きます。
4. **UI文字列はメッセージカタログ**(`src/locales/*.json`)に置き、コンポーネントへ直書きしません。全カタログのキー一致はテストで強制されます。
5. **UIはデザインが先。** 画面はPencil(`MadakeCAD.pen`)で設計し、`docs/internal/design-system.md`に記載してから実装します。既存のトークン・部品を再利用し、足りなければ先にデザインシステムへ追加します。

## 進め方

1. 触る領域の仕様(`docs/internal/specs/`)とマスタースペック(`docs/superpowers/specs/`)を読みます。挙動が変わるなら先に仕様を更新します。
2. 小さな修正以外は`docs/superpowers/plans/`に計画(日付付き。既存の計画を参考に)を置き、タスクごとにred→greenでコミットします。
3. プッシュ前に確認:

   ```bash
   cd src-tauri && cargo test            # Rust(全クレート)
   npx vitest run && npx vue-tsc --noEmit   # フロント
   python3 -m unittest discover -s freecad-addon/tests   # FreeCADアドオン
   python3 scripts/gen_spec.py --check   # 仕様書の鮮度
   ```

4. コミットは小さく、説明的に。メッセージは日本語でも英語でも構いません。
5. テンプレートに沿ってプルリクエストを開き、従った仕様・計画をリンクします。

## どこに何があるか

| 領域 | パス |
|---|---|
| ドキュメントモデル・Command・シンボルライブラリ・ファイルIO・帳票・検証 | `src-tauri/crates/madake-core` |
| 内蔵MCPサーバーとREST Link API | `src-tauri/crates/madake-mcp` |
| AIアシスタントのバックエンド | `src-tauri/crates/madake-agent` |
| `madake` CLI(薄いクライアント) | `src-tauri/crates/madake-cli` |
| Tauri本体(IPCハンドラ) | `src-tauri/src` |
| Vue 3 UI・Canvas2Dレンダラ・Piniaストア | `src/` |
| FreeCADアドオン | `freecad-addon/` |

## 問題の報告

Issueテンプレートを使ってください。セキュリティに関わる問題は[SECURITY.md](SECURITY.md)へ。社外秘の図面は添付せず、再現できる最小の`.mdkproj`があると助かります。

## ライセンス

貢献はプロジェクトと同じMIT OR Apache-2.0でライセンスされることに同意したものとします([README](README.ja.md#ライセンス)参照)。
