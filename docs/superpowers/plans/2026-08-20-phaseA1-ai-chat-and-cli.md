# フェーズA1: アプリ内AIチャット + madake CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** アプリ内チャット(フルUI)からClaude Code CLI(サブスクOAuth・キー不要)で図面を対話編集でき、編集箇所がキャンバス上にオーバーレイ表示され、同機能をターミナル(`madake` CLI)からも使える状態にする。

**Architecture:** エージェントループはRust側`madake-agent`クレートがローカルのClaude Code CLIをヘッドレス起動(`--output-format stream-json`+自前MCP自己接続)して実現。イベントはTauriイベント/SSEでフロントへ流し、ChatPanelがストリーミング描画する。編集は全てMCP→Commandエンジン経由なので自動適用+undoが成立する。`madake` CLIはLink API(127.0.0.1:9310/api/v1)のシンクライアント。

**Tech Stack:** Rust (tokio process, clap, reqwest), Claude Code CLI, Vue 3 + Pinia, Canvas2D

**Spec:** `docs/superpowers/specs/2026-08-20-madakecad-design.md` §5.2

## ユーザー決定事項 (2026-08-20)

- 編集は**自動適用+undo**が既定(設定で確認モードへ切替可)
- チャットは**フルUI**(折りたたみ/展開、ストリーミング、ツールチップ、モデルピッカー)。並列エージェント・自動反復はA2以降
- 会話履歴は**プロジェクト保存**(`<プロジェクト名>.chat.json`)
- コマンドラインUIは**廃止済み**。代替として`madake` CLIをターミナル向けに提供
- エージェント編集中は**シアンのオーバーレイアニメーション**で編集箇所を可視化(デザイン: .penの「エージェント編集オーバーレイ」)

## Global Constraints

- 全編集はCommandエンジン経由(CLAUDE.md絶対原則)。エージェントのツールはMCP(9310)の既存ツールを使い、二重実装しない
- Claude Code CLIのフラグは検証済み(2026-08-21、claude 2.1.237)。想定フラグ(`-p` `--output-format stream-json` `--verbose` `--resume <id>` `--model` `--mcp-config` `--allowedTools` `--append-system-prompt`)は全て存在。**追加**: テキストのデルタ配信には`--include-partial-messages`が必須(無指定だと完成メッセージ単位でしか出ない)。`--strict-mcp-config`でユーザー設定のMCPを除外し自前mcp-configのみ読ませる
- stream-json実出力の構造(実測): `system/init`(session_id)、`stream_event`(生APIイベント: content_block_delta の delta.type=text_delta がテキスト、thinking_delta は無視)、`assistant`(完成content block毎に発火。tool_useブロックは完全なinputを持つ=ToolUseStartedのトリガに使う)、`user`(tool_result。is_errorでToolUseFinished)、`result`(subtype=success、result/usage)。`system/status` `system/thinking_tokens` `rate_limit_event` `system/task_summary`等の未知タイプはスキップ
- APIキー等の秘密情報は扱わない(A1はCLIのOAuthセッションに委譲)。A3まで設定ファイルにトークンを置かない
- テスト: `cd src-tauri && cargo test` / `npx vitest run`。CLIは本物のclaudeを呼ばず、フィクスチャを吐くフェイクスクリプトで統合テストする
- デザイン準拠: `MadakeCAD.pen`の「AIチャットパネル」「AIチャット(展開状態)」「AIチャット - ポップアップ集」「エージェント編集オーバーレイ」+ `docs/design-system.md`

---

### Task 1: stream-jsonイベントパーサ (madake-agent)

**Files:**
- Create: `src-tauri/crates/madake-agent/Cargo.toml`(members追記: `src-tauri/Cargo.toml`)
- Create: `src-tauri/crates/madake-agent/src/lib.rs`, `src/events.rs`
- Test fixture: `src-tauri/crates/madake-agent/tests/fixtures/stream.jsonl`

**Interfaces:**
- Produces: `pub enum AgentEvent { SessionStarted { session_id: String }, TextDelta { text: String }, ToolUseStarted { tool: String, input: serde_json::Value }, ToolUseFinished { tool: String, is_error: bool }, TurnCompleted { result: String, usage: Option<Usage> }, Error { message: String } }` と `pub fn parse_stream_line(line: &str) -> Option<AgentEvent>`(claude CLIのstream-json 1行→イベント。未知タイプはNone)

- [x] **Step 1: 失敗するテストを書く** — 実物のclaude出力(実装時に`claude -p "hi" --output-format stream-json --verbose`で採取してfixtures化)から: system/initでSessionStarted、assistantメッセージのtext/tool_useブロック、resultでTurnCompletedが得られること。未知行はNone
- [x] **Step 2: cargo test で失敗確認**
- [x] **Step 3: 実装**(serde_json::Valueで緩くパースし、必要フィールドのみ取り出す)
- [x] **Step 4: cargo test 成功確認**
- [x] **Step 5: コミット** `feat(agent): stream-jsonイベントパーサ`

### Task 2: ClaudeCodeCliBackend (プロセス起動+ストリーミング)

**Files:**
- Create: `src-tauri/crates/madake-agent/src/backend.rs`
- Test: フェイクCLI `tests/fixtures/fake_claude.sh`(fixturesのJSONLをcatするだけ)

**Interfaces:**
- Produces: `pub struct ClaudeCodeCliBackend { pub executable: PathBuf, pub model: Option<String>, pub mcp_port: u16 }`、`pub async fn send(&self, prompt: &str, session: Option<&str>, tx: mpsc::Sender<AgentEvent>) -> Result<()>`
- コマンド構築: `claude -p <prompt> --output-format stream-json --verbose [--resume <session>] [--model <m>] --mcp-config <一時ファイル> --allowedTools "mcp__madakecad__*" --append-system-prompt <図面コンテキスト>`。mcp-config一時ファイルは`{"mcpServers":{"madakecad":{"type":"http","url":"http://127.0.0.1:<port>/mcp"}}}`
- 検出: `pub async fn detect(executable: Option<PathBuf>) -> Result<DetectResult>`(`claude --version`実行。PATH→設定パスの順)

- [x] **Step 1: コマンド構築の単体テスト**(引数列が正しいこと。resume/model有無の分岐)
- [x] **Step 2: フェイクCLIでの統合テスト**(executableをfake_claude.shにしてsend→イベント列がchannelに届く)
- [x] **Step 3: 実装**(tokio::process、stdout行読み→parse_stream_line→tx.send。stderrはエラーイベント化)
- [x] **Step 4: cargo test 成功、コミット** `feat(agent): Claude Code CLIバックエンド`

### Task 3: 会話マネージャ (セッション・undo追跡・履歴永続化)

**Files:**
- Create: `src-tauri/crates/madake-agent/src/conversation.rs`

**Interfaces:**
- Produces: `pub struct Conversation { pub id: Uuid, pub session_id: Option<String>, pub messages: Vec<ChatMessage>, pub model: Option<String> }`、`ChatMessage { role, text, tool_calls: Vec<ToolCall>, applied_revisions: (u64, u64) }`(ターン開始/終了時のEngine revisionを記録→「元に戻す」= (end-start)回undo)
- 永続化: `save_chat(path, &[Conversation])` / `load_chat(path)`(整形JSON。プロジェクト保存パスの隣に`<stem>.chat.json`)

- [x] **Step 1: 失敗するテスト**(revision追跡: ターン中に3コマンド→applied_revisions差が3。JSONラウンドトリップ)
- [x] **Step 2-4: 赤→実装→緑、コミット** `feat(agent): 会話マネージャとチャット履歴永続化`

### Task 4: Tauri/Link API統合 (agentコマンドとイベント配信)

**Files:**
- Modify: `src-tauri/src/lib.rs`(AppStateにAgentManager追加、IPC: `agent_send` `agent_cancel` `agent_list_conversations` `agent_undo_turn` `agent_detect`)
- Modify: `src-tauri/crates/madake-mcp/src/link_api.rs`(`POST /api/v1/agent/send`、`GET /api/v1/agent/events`(SSE)— ブラウザ検証用)
- Modify: save_project/load_projectでchat.jsonも保存/読込

**Interfaces:**
- AgentEventは`agent:event`イベント(Tauri emit)+SSEで配信。payload: `{conversation_id, event}`
- 図面コンテキスト: send時に`--append-system-prompt`へ「アクティブシートid・シート一覧・ネット数」を要約して渡す(設定「図面自動読み取り」ON時)

- [x] **Step 1: ビルド+フェイクCLIでのスモーク**(agent_send→agent:eventが流れる)
- [x] **Step 2: コミット** `feat: エージェントをIPC/Link APIに公開`

### Task 5: フロント チャットストア (イベントリデューサ)

**Files:**
- Create: `src/stores/chat.ts` + `src/stores/chat.test.ts`

**Interfaces:**
- state: `{ conversations, activeId, streaming: bool, panelOpen: "collapsed"|"expanded", model }`
- `applyAgentEvent(event)`: TextDelta連結、ToolUseStarted→チップ(running)、Finished→✓/✗、TurnCompleted→applied表示
- vitestでイベント列→メッセージ状態を検証(ストリーミング途中/完了/エラー)

- [x] TDD一式、コミット `feat(ui): チャットストア`

### Task 6: ChatPanel UI (デザイン準拠フル実装)

**Files:**
- Create: `src/components/chat/ChatPanel.vue`(折りたたみ⇔展開)、`ChatMessage.vue`、`ToolChip.vue`、`ModelPicker.vue`
- Modify: `src/components/CanvasView.vue`(パネルを作図領域左下にオーバーレイ配置)

**要件(デザイン準拠):**
- 折りたたみ: 入力+添付+トークン表示+モデル選択+送信。展開: ヘッダ(接続中バッジ/最小化)+会話+入力
- ツールチップ: `✓ place_symbol fuse F2 5A を (140,90) に配置`形式(inputから要約生成)
- 「✓ 図面に適用済み (rev N)/元に戻す」→ `agent_undo_turn`
- ModelPicker: プロバイダ別グループ(A1はAnthropicのみ+今後の枠)。`--model`へ反映
- Enter送信/Shift+Enter改行。ストリーミング中はローダ表示+キャンセル

- [x] 実装→ブラウザ検証(localhost:1420+フェイクCLI)でスクリーンショット確認、コミット `feat(ui): AIチャットパネル`

### Task 7: エージェント編集オーバーレイ (キャンバスアニメーション)

**Files:**
- Modify: `src/canvas/renderer.ts`(オーバーレイ描画)、`src/canvas/theme.ts`(agent色 #29D3E6)
- Create: `src/canvas/agentOverlay.ts` + テスト

**Interfaces:**
- `class AgentOverlay { noteToolStart(tool, input): void; noteEntityUpserted(entity): void; activeRegions(now): Region[] }` — ToolUseStarted/patchから対象領域(bbox+マージン)を算出し、開始〜完了+1.5秒までパルス表示(sinで透明度0.1〜0.25)。ラベルチップ「⚡ エージェントが編集中...」
- requestAnimationFrameはストリーミング中のみ回す(アイドル時のGPU消費ゼロ)

- [x] bbox算出の単体テスト(wire/symbolから領域)→実装→ブラウザ検証、コミット `feat(ui): エージェント編集オーバーレイ`

### Task 8: madake CLI (ターミナルツール)

**Files:**
- Create: `src-tauri/crates/madake-cli/`(workspace追加、bin名 `madake`)

**Interfaces (clap):**
- `madake status` / `madake project` / `madake netlist [--sheet <id>]`
- `madake export svg|bom|wire-list <path>` / `madake save <path>` / `madake open <path>`
- `madake exec <commands.json>`(Command列をLink APIへ) / `madake undo` / `madake redo`
- 接続先は`--port`(既定9310)。アプリ未起動時は明確なエラー
- reqwest blockingでLink APIを叩くだけの薄いクライアント。出力はJSON(--json)または表形式

- [x] コマンド構築/出力整形の単体テスト→実機アプリ相手のスモーク(status/netlist/export svg)→コミット `feat(cli): madakeコマンド`
- [x] README/CLAUDE.mdにCLIの使い方を追記

### Task 9: 設定の最小実装 (A1範囲)

**Files:**
- Modify: `src-tauri/src/lib.rs`(アプリ設定: claude実行パス・自動適用・図面自動読み取り。`~/.madakecad/settings.json`)
- Create: `src/components/settings/`(設定ダイアログはA1では「エージェント」タブのClaude欄+動作トグルのみ。他タブはプレースホルダ)

- [x] 設定の保存/読込テスト→UI接続→コミット `feat: AI設定(最小)`

## 検証 (受け入れ条件)

1. `claude`にサインイン済みの環境で、チャットに「24V系にヒューズF2を追加して」→ 図面にF2と配線が現れ、ツールチップに実行内容、完了後「適用済み (rev N)」表示
2. 「元に戻す」でそのターンの編集が全て巻き戻る
3. 編集中、対象領域にシアンのパルスオーバーレイが出る
4. アプリ再起動(プロジェクト再読込)で会話履歴が復元される
5. `madake netlist` / `madake export svg /tmp/a.svg` がターミナルから動く
6. フェイクCLIによる自動テストがCIで完結(本物のclaude不要)

## Self-Review結果

- spec §5.2のA1範囲(チャットUI+ClaudeCodeCliBackend)+ユーザー追加要望(オーバーレイ・CLI・コマンドラインUI廃止)を全てタスク化した
- モデルピッカーのマルチプロバイダ表示はA3の設定実装と依存するため、A1はAnthropicグループのみ表示に留める(Task 6に明記)
- 型整合: AgentEvent(Task 1)をTask 2/4/5が同名で使用。applied_revisions(Task 3)をTask 6の「元に戻す」が参照

## 既知の制限 (A2持ち越し)

Task 4のレビュー指摘のうち、A1では設計変更が大きいため意図的に見送った項目。

### 1. ターン安定IDが無い(`message_index`指定の脆さ)

`agent_undo_turn` / `POST /api/v1/agent/undo-turn` は対象ターンを
「会話内の`messages`添字」で指定する。添字はメッセージ追加・履歴移行で容易にずれるため、
本来はターンごとの安定ID(UUID)を持たせるべき。

A1での防御:
- サーバー側で「最新の適用済みアシスタントターンでなければ`NotLatestTurn`エラー」ガードを掛けた
  (後続ターンに編集が残っている場合・ドキュメントのundoスタックがターン終了時と食い違う場合・
  実行中ターンは拒否)。誤った添字を渡しても別ターンを巻き戻すことはない
- UIも「元に戻す」ボタンを最新ターンにのみ出す

A2で`ChatMessage`にターンIDを追加し、`undo_turn(turn_id)`へ移行する
(`chat.json`の`format_version`を上げる)。

### 2. ユーザー編集がターン中に混ざった場合のundo境界

undo回数は「ターン前後のundoスタック深さの増分」(`ChatMessage::applied_undo_depth`)で決める。
revision差分はundo/redoでも進むため使えない。ただし深さ増分は
「そのターンの間にドキュメントへ積まれた編集」であって「エージェントが行った編集」ではない。
ターン中にユーザーがUIで編集すると、その編集もターンの巻き戻し対象に含まれる。

正確に切り分けるにはCommandエンジン側で編集の出所(origin: user / agent / mcp)を
履歴エントリに持たせる必要がある。A2で`Engine`の`HistoryEntry`にoriginを追加して対応する。
`applied_revisions`は表示・デバッグ用に残してある。

### 3. キャンセル遅延イベントの誤着弾(broadcast側のターンseq未導入)

`AgentManager::cancel`はターンタスクをabortするが、`ConversationEvent`は
`{conversation_id, event}`しか持たないため、キャンセル直前にbroadcastされたイベントが
次のターンのものとしてUIに着弾し得る(現実装ではrxのdropで即座に止まるので窓は狭い)。
配信ペイロードにターンseqを載せ、購読側が古いseqを捨てられるようにするのがA2の対応。

### 4. broadcast Lagged時の再同期通知が無い

`/api/v1/agent/events`(SSE)とTauriの`agent:event`転送は、
`broadcast::error::RecvError::Lagged`を`continue`で読み飛ばしている。
遅い購読者はイベントを取りこぼしたまま気付けない(テキストが欠けたチャットが残る)。
A2で「取りこぼし通知イベント」を流し、UIが`agent_list_conversations`で会話を
まるごと再取得して再同期できるようにする。

### 5. undo_turnの深さ検査と実行の間の競合窓

`undo_turn`は「現在のundoスタック深さ == ターン終了時の深さ」をstateロック下で検査するが、
その後の`doc.undo()`ループはロック外で回る。検査とループの間に別クライアント(MCP/Link API)が
編集を積むと無関係のコマンドを巻き戻し得る。A2でDocBridgeに「深さがNのときだけundoする」
CAS的APIを追加して閉じる。

### 6. pendingローカル会話のid引き取りがイベント内容と無関係

フロントの`send()`解決前に別クライアント起点の未知会話イベントが届くと、
`ensureConversation`のpending引き取りがその無関係なidを取り込む余地がある(発生確率は低い)。
恒久対策は制限3と同じくターンseq/採番idの照合待ち。
