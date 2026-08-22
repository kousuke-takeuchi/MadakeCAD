# MadakeCAD M3フェーズ3 (マルチプロバイダ拡充: Copilot CLI / OpenAI互換 / Gemini) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M3仕様§5の残りプロバイダを実装する。優先順(ユーザー指定): ①**GitHub Copilot CLIバックエンド**(ユーザーにクレジット残あり・最優先) ②OpenAI互換Backend(OpenAI/xAI/OpenRouter/Ollama) ③GeminiBackend。AgentBackendトレイト+プロバイダ設定UI(フェーズ2実装済み)へ追加する形。

**共通ルール:** tests-as-spec+gen_spec。i18n(en/ja)。キー類はOSキーチェーンのみ(平文禁止をテストで固定)。ドキュメントEN+JA同期。

## 調査結果(2026-08-22、ローカル実機+公式docs)

- **Copilot CLI 1.0.80**(`npm i -g @github/copilot`で導入済み、`copilot`コマンド):
  - 非対話: `copilot -p <text>`(完了で終了)、`-s`(応答のみ)、`--output-format json`=**JSONL(1行1オブジェクト)**
  - MCP: `--additional-mcp-config <json|@file>`(セッション限定で`~/.copilot/mcp-config.json`に追加)、`--disable-builtin-mcps`(既定のgithub-mcp-serverを無効化)、ツール許可`--allow-tool`/`--available-tools`(パターン例 `serverName(tool)`)、`--allow-all-tools`(非対話モードで必須)
  - セッション: `--resume [id]` / `--session-id <id>` / `--continue`
  - その他: `--model <model>`(`auto`可)、`--no-ask-user`、`--no-custom-instructions`、`--add-dir`、`--log-level`
  - 認証: GitHub OAuth(対話モードの`/login`)または `COPILOT_GITHUB_TOKEN`/`GH_TOKEN`/`GITHUB_TOKEN`。**本機は未認証**(実キー確認はユーザー作業)
- 出典: [CLI programmatic reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-programmatic-reference) / [Run CLI programmatically](https://docs.github.com/en/copilot/how-tos/copilot-cli/automate-copilot-cli/run-cli-programmatically) / `copilot --help`実出力

## 設計決定

- **CopilotCliBackend**: `AgentBackend`実装。`copilot -p <prompt> --output-format json --allow-all-tools --no-ask-user --disable-builtin-mcps --additional-mcp-config @<一時ファイル> --session-id <uuid>`(継続ターンは`--resume <uuid>`)。MCP設定はMadakeCADのStreamable HTTP(127.0.0.1:9310/mcp)を指す一時JSONを都度生成
  - **システムプロンプト**: Copilot CLIに`--append-system-prompt`相当が無いため、`knowledge::system_prompt`をプロンプト先頭へ区切り付きで前置(`--no-custom-instructions`でAGENTS.md干渉を排除)。この差異を仕様文へ明記
  - **JSONLイベント変換**: 実CLIのイベント形をprobe(認証後)で確定し、それまでは代表的な形(text/tool系/完了)を許容するtolerantなパーサ+フィクスチャテスト。認証前でも「未認証エラーの検知と案内」(`copilot`での`/login`手順)は実装・テストする
  - 設定: `provider: "copilot_cli"`追加+`copilot_path`(既定PATHの`copilot`)+`copilot_model`(既定`auto`)。検出バッジ=CLI存在+認証状態(`copilot -p ping`の失敗種別で判定)
- **OpenAI互換Backend**: base_url+モデル+キー(キーチェーン`account="openai_compat_api_key"`)。Chat Completions(tool calling)でMCPブリッジ(フェーズ2の`McpToolBridge`)を再利用。Ollama=キー不要のlocal URLプリセット
- **GeminiBackend**: generateContent(function calling)。キーは`account="gemini_api_key"`
- 設定UIのプロバイダドロップダウンへ3択追加(各詳細フォーム: Copilot=パス+モデル+認証状態/OpenAI互換=URL+モデル+キー/Gemini=モデル+キー)

### Task 1: CopilotCliBackend

- [x] Step 1 (red): Rustテスト(フェイクcopilot CLIフィクスチャ): 起動引数(−p/JSONL/MCP設定ファイル/allow-all-tools/session-id)/継続ターンで--resume/JSONLイベント変換(text・ツール・完了・エラー)/未認証エラーの検知と日本語案内/システムプロンプトの前置/copilot_path・copilot_model設定
- [x] Step 2 (green): 実装+プロバイダUI追加(i18n)→全テストgreen→gen_spec→コミット
- [x] Step 3: 実機probe(2026-08-22実施)。`copilot -p "reply pong" --output-format json --allow-all-tools`は**未認証で失敗**(exit 1、stdout空、stderrに`Error: No authentication information found.`+`/login`等の手順)。実出力をフィクスチャ`fake_copilot_unauth.sh`と検知ロジックへ反映済み。起動フラグ一式(`--no-ask-user`/`--no-custom-instructions`/`--disable-builtin-mcps`/`--additional-mcp-config @file`/`--session-id`/`--model`/`--log-level error`)が実機CLIに受け付けられること、MCP設定JSONの形が`copilot mcp add --transport http`の書き出しと一致することも実機で確認済み

  **ユーザー確認事項(未完)**: `copilot`を起動して`/login`でサインインしたうえで、(a) `--output-format json`のJSONL実形をパーサの別名表へ寄せる、(b) アプリで1ターン通し(作図+検証+undo後始末)。docs/09(EN/JA)に「確認できている範囲」として記載済み

### Task 2: OpenAI互換Backend(Ollamaプリセット含む)

- [ ] Step 1 (red): モックHTTPでtool callingループ/キーチェーン/Ollamaはキー無しで可/401・レート制限の表示
- [ ] Step 2 (green): 実装+UI。ローカルOllamaがあれば実機確認(なければモックのみと記録)。コミット

### Task 3: GeminiBackend

- [ ] Step 1 (red): モックHTTPでfunction callingループ/キーチェーン/エラー表示
- [ ] Step 2 (green): 実装+UI。コミット

### Task 4: 仕上げ

- [ ] docs/09(EN+JA)へプロバイダ表(CLI/API/Copilot/OpenAI互換/Ollama/Gemini)、feature-inventory・roadmap更新、m3仕様§5を完了へ。全テストgreen

## 受け入れ基準

- Copilot CLI認証済み環境で、プロバイダ「GitHub Copilot」を選ぶだけでチャット作図(MCPツール経由の実編集+undo)が動く
- OpenAI互換(モック)とGemini(モック)のツールループがgreen。Ollama URLプリセットが設定UIにある
- どのプロバイダでもキーが設定ファイル・ログ・イベントへ平文で出ない
