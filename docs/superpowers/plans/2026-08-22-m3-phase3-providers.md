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

- [x] Step 1 (red): モックHTTPでtool callingループ/キーチェーン/Ollamaはキー無しで可/401・レート制限の表示
- [x] Step 2 (green): 実装+UI。ローカルOllamaがあれば実機確認(なければモックのみと記録)。コミット

  **実装の要点**: `openai_compat.rs`(`POST {base_url}/chat/completions`・`stream:true`+
  `stream_options.include_usage`)。systemは`messages[0]`、ツールはOpenAIのfunction形式
  (`tools[].function.parameters`)、`tool_calls`は`index`ごとに引数JSONを連結して実行し
  `role:"tool"`+`tool_call_id`で返す(上限16往復)。ツール失敗は`ERROR: `を前置して返す
  (OpenAIのツールメッセージには成否欄が無いため)。キーは`account="openai_compat_api_key"`、
  **接続先がローカル(localhost/127.0.0.1/*.local等)ならAuthorizationヘッダごと省略**。
  設定は`openai_base_url`(既定`https://api.openai.com/v1`)+`openai_model`(既定は空=必須入力)。
  エラー区分は`openai_auth`/`openai_rate_limit`/`openai_model_not_found`/`openai_network`ほか。

  **実機確認(2026-08-22、ローカルOllama 0.32.14)**: 実施済み。
  - `stream_options.include_usage`・`tools`・`tool_calls`の実チャンク形がパーサの想定と一致
    (`{"choices":[{"delta":{"tool_calls":[{"id":..,"index":0,"type":"function","function":{"name":..,"arguments":"{...}"}}]}}]}`
    → `finish_reason:"tool_calls"` → usageチャンク → `data: [DONE]`)。**キー無しで通る**
  - 接続テスト(`POST /api/v1/agent/test-connection`)はキー未保存のまま成功
  - **アプリで1ターン通し(`gemma4:26b`)が成功**: 「Sheet1の(50,50)にリレーK1を置いて」
    → ツール往復3回(最初の2回はモデルが`mcp__madakecad__place_symbol`という別名で呼んで
    失敗 → こちらが`ERROR: `付きで返す → モデルが`place_symbol`へ直して成功)→
    revision 0→1・エンティティ1件・`turn_applied`イベント・日本語の報告文まで到達。
    後始末に`POST /api/v1/agent/undo-turn`でエンティティ0件へ巻き戻し済み(設定も復元)
  - **モデル依存の注意**: `Agents-A1-4B`(HF GGUF)は2往復目でチャットテンプレートが
    `No user query found in messages.`(HTTP 500)を返す=モデル側テンプレートの制約であって
    リクエスト形の誤りではない(その旨をエラー本文にそのまま出す)。`ollama show`の
    Capabilitiesに`tools`があるモデルを選ぶこと

### Task 3: GeminiBackend

- [x] Step 1 (red): モックHTTPでfunction callingループ/キーチェーン/エラー表示
- [x] Step 2 (green): 実装+UI。コミット

  **実装の要点**: `gemini.rs`(`POST {base}/models/{model}:streamGenerateContent?alt=sse`、
  キーは**`x-goog-api-key`ヘッダのみ**でURLクエリには載せない)。systemは`systemInstruction`、
  ツールは`tools[0].functionDeclarations[]`、`functionCall`が返ったら実行して
  **role`"user"`の`functionResponse`**(`{id?, name, response:{result|error}}`)で返す
  (上限16往復)。usageは`usageMetadata`(`promptTokenCount`/`candidatesTokenCount`/
  `cachedContentTokenCount`)を**累計として置き換え**る(毎チャンクに載るため足し込まない)。
  設定は`provider: "gemini"` + `gemini_model`(既定`gemini-2.5-flash`。空欄は既定へ戻す)、
  キーはキーチェーン`account="gemini_api_key"`。ベースURLは`MADAKE_GEMINI_BASE_URL`で上書き可。
  エラー区分は`gemini_auth`/`gemini_rate_limit`/`gemini_model_not_found`/`gemini_server`/
  `gemini_request`/`gemini_network`/`gemini_unknown`/`gemini_no_key`/`gemini_no_model`。

  **JSON Schemaの落とし穴**: GeminiのSchemaはOpenAPIの部分集合で、`$schema`/`$ref`/`$defs`/
  `additionalProperties`/`allOf`等を送ると400になる。`gemini_schema()`が許可キー
  (type/format/title/description/nullable/enum/maxItems/minItems/properties/required/
  minProperties/maxProperties/minLength/maxLength/pattern/example/anyOf/propertyOrdering/
  default/items/minimum/maximum)だけを再帰的に残し、`format`は受け付ける綴り
  (date-time/enum/float/double/int32/int64)以外を落とす。削って型が消えたスキーマには
  `type: "string"`を補い、`properties`が空なら`parameters`ごと省く。

  **実機確認(2026-08-22、キー無し環境)**: プロバイダを`gemini`へ切替(Link API
  `PUT /api/v1/settings`)→`gemini_model`の空欄が既定へ正規化、`GET /api/v1/agent/provider`が
  `gemini_model`/`gemini_key_saved`を返すことを確認。キー未保存の接続テストは通信せず
  `gemini_no_key`+案内文。**無効キーで実APIも確認**: 本物の
  `generativelanguage.googleapis.com`が`400 INVALID_ARGUMENT`+`reason: API_KEY_INVALID`を返し、
  アプリ側が`gemini_auth`+「APIキーが受け付けられませんでした…」へ変換できた(probe用キーは
  確認後にキーチェーンから削除、設定も元に戻した)。ブラウザ(localhost:1420)からの
  画面確認はこの環境のブラウザpaneがLink APIへ到達できず未実施。

  **ユーザー確認事項(未完)**: 実キー(Google AI Studio発行)を設定 > エージェント へ保存し、
  (a) 接続テストの成功表示、(b) アプリで1ターン通し(作図+検証+undo後始末)、
  (c) 実際のfunction callingでMCPツールのスキーマが400にならないこと。

### Task 4: 仕上げ

- [x] docs/09(EN+JA)へプロバイダ表(CLI/API/Copilot/OpenAI互換/Ollama/Gemini)、feature-inventory・roadmap更新、m3仕様§5を完了へ。全テストgreen

  **実施内容(2026-08-22)**:
  - `docs/09-ai-assistant(.ja).md`: プロバイダ個別の箇条書きを**6経路の表**(認証方式 / 必要なもの / 確認済みの範囲)へ整理し、`## Providers`(`## プロバイダ`)節を新設。経路ごとの差(Copilot=システムプロンプト前置・Gemini=OpenAPI部分集合へのスキーマ削り・OpenAI互換=`ERROR: `前置)、設定手順、キーチェーンの説明(service/account・平文非保存・macOSの1回キャッシュ)、`MADAKE_*_BASE_URL`の上書きを追記。✅/🔶の意味を明文化し「計画中」はフェーズ4以降(整えバリアント・比較案UX)だけに
  - `docs/12-roadmap(.ja).md`: M3を「フェーズ1〜3完了」へ。フェーズ3の成果を1行追加し、残件はフェーズ4の2件+「値セット等はM4側」の1行に集約
  - `docs/internal/feature-inventory.md`: 「その他プロバイダ ⬜」を削除し、**実装 / 実機確認状況 / キーチェーン / 接続テスト**の4行へ分割。設定画面の行を「6経路すべて実動」へ。Link APIの`agent/*`一覧に`provider`・`api-key`・`test-connection`を追加。冒頭の更新日付にM3フェーズ3を記載
  - `docs/internal/specs/m3-ai-first.md`: §5を「フェーズ2・3で完了」+6経路の表(実装 / 認証 / 状態)へ。認証アーキタイプが3種に収束した経緯(デバイスコードOAuthはCopilot CLI側が処理するため不要)を記録。**ユーザー確認事項を1つの表へ集約**(Copilot=`/login`+JSONL実イベント形、Gemini=実キー+スキーマ400の有無、Anthropic=実キー通し)。受け入れ基準の達成をOllama経路で記録。冒頭にフェーズ3完了注記
  - `README(.ja).md`: AI特徴行を「6プロバイダから選べる」へ、前提を「claude CLI必須」から「**どれか1つ**あればよい」へ
  - `docs/02-getting-started(.ja).md`: 前提表にCopilot CLI・Ollamaを追加し、「AIチャットはどれか1つのプロバイダで動く」旨とプロバイダ設定手順(Ollamaはキー不要)を追記
  - `docs/internal/design-system.md`: 設定ダイアログのプロバイダ欄の記述を実装(5項目のドロップダウン+経路別の詳細グループ、OllamaはOpenAI互換内のプリセット)へ更新。**`MadakeCAD.pen`のデザインシステムボードへの反映は未**(Pen.app起動が要るため次のデザイン作業時に取り込む)
  - 総合検証: `cargo test --workspace` 748件green / `npx vitest run` 435件green(36ファイル) / `npx vue-tsc --noEmit` エラー0 / `python3 scripts/gen_spec.py --check` 最新

## M3フェーズ3 完了 (2026-08-22)

プロバイダは**6経路**そろった: Claude Code CLI(既定) / Anthropic API / GitHub Copilot CLI / OpenAI互換API / Ollama(ローカルプリセット) / Google Gemini。
全経路が同じ`ToolBridge`(内蔵MCPサーバー)とCommandエンジンを通り、キーはOSキーチェーンのみ(CLI経路は資格情報を持たない)。

**実機で通し確認済み**: Claude Code CLI、**Ollama**(キー不要のローカル経路。作図1ターン=ツール往復3回→revision 0→1→`undo-turn`で後始末)。
**ユーザー確認事項として残る**: GitHub Copilot(`copilot`→`/login`後の通しとJSONL実イベント形)、Google Gemini(実キーでの通しとfunction callingのスキーマ)、Anthropic API(実キーでの通し)。
**フェーズ4の残件**: 整えのバリアント(2〜4案の並列比較)、並列エージェントの比較案UX。

## 受け入れ基準

- Copilot CLI認証済み環境で、プロバイダ「GitHub Copilot」を選ぶだけでチャット作図(MCPツール経由の実編集+undo)が動く
- OpenAI互換(モック)とGemini(モック)のツールループがgreen。Ollama URLプリセットが設定UIにある
- どのプロバイダでもキーが設定ファイル・ログ・イベントへ平文で出ない
