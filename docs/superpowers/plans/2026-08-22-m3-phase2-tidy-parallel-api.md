# MadakeCAD M3フェーズ2 (整えループ+並列エージェント+Anthropic API) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M3仕様の§3(自動反復=整えループ)・§4(並列エージェント)・§5の第1弾(AnthropicApiBackend+OSキーチェーン)を実装する。仕様=`docs/internal/specs/m3-ai-first.md`。OpenAI互換/Geminiバックエンドはフェーズ3。

**共通ルール:** 全編集はCommand経由。tests-as-spec+タスクごとにgen_spec再生成。UI文字列はi18n(en/ja)。ドキュメントEN+JA同期。デザインは`.pen`「AIチャット - ポップアップ集」(整えポップアップ・並列会話)+「AI設定」系フレーム(プロバイダ設定)を正とする。

## 設計決定

- **整えメトリクス(コア)**: `madake-core::tidy`に決定的なメトリクス関数を追加 — 配線交差数(線分交差、共有端点は除外)・ラベル/シンボルの重なり数(バウンディングボックス)・グリッド外エンティティ数。MCPツール`get_tidy_metrics`で露出し、エージェントが改善目標に使う
- **整えループの実行形態**: エージェント反復(専用アルゴリズムではなくLLMに任せる)。ポップアップ(配置整理/配線整理/ラベル整頓)は定型プロンプト+選択範囲を組み立てて通常ターンとして送る。プロンプトに「get_tidy_metricsを実行→編集→再計測、改善が止まるか3回で終了」のループ指示。1回の整え=1ターン(undo一発)
- **並列エージェント**: AgentManagerの会話別ターン実行を排他から並行へ(図面編集はCommandエンジンが直列化するため安全)。会話ごとに編集オーバーレイ色を割当(色パレットをtheme追加)。**比較案UX(未決→決定)**: フェーズ2では「並行実行+会話別色」まで。シート複製比較・パッチプレビューは需要を見てフェーズ3
- **AnthropicApiBackend**: `AgentBackend`トレイトを確認/整備し、Messages API直結の実装を追加(ツール=既存MCPツールをAPIのtool定義へブリッジ、ループはRust側)。モデルは`claude-sonnet-5`既定+設定で変更可。**APIキーはOSキーチェーン保存**(keyringクレート。バージョンはレジストリ確認)。設定ファイル・ログへの平文出力禁止をテストで担保
- **プロバイダ設定UI**: 設定>エージェントのプロバイダを実選択に(Claude Code CLI / Anthropic API)。API選択時: キー入力(保存はキーチェーン・表示は伏せ字)+モデル選択+接続テスト。デザインは既存AI設定フレーム+CAD調規約

### Task 1: 整えメトリクス(コア+MCP)

- [x] Step 1 (red): Rustテスト: 交差数(交差あり/共有端点は数えない/平行)/重なり数(ラベル同士・ラベルとシンボル/接しているだけは数えない)/グリッド外検出/決定性。MCP `get_tidy_metrics`の応答形
- [x] Step 2 (green): 実装+露出。gen_spec→コミット

**決定した数え方** (`madake-core::tidy`):
- 交差 = 線分どうしが**面で交わる**組。端点で出会う接続 (L字・T分岐) は数えない。同じ配線の連続線分 (曲がり角) も除く。同一直線上の部分重なりは「直す場所1つ」として1件
- 重なり = 外接矩形が**面で重なる**組。辺・角が接しているだけ (食い込み ≤ CONNECT_EPS) は数えない。文字×文字・文字×シンボル外形=`label_overlaps`、シンボル外形どうし=`symbol_overlaps`。シンボル自身の参照記号・型番は持ち主とは重ならない扱い
- グリッド外 = 2.5mm格子に乗らないシンボル原点・配線頂点の数 (許容誤差 CONNECT_EPS)。ずれた点1つにつき1件
- 文字の外接矩形はSVG/PDF出力と同じ配置規則で求める (`svg.rs`の配置定数を共有)

### Task 2: 整えポップアップ(UI+プロンプト)

- [x] Step 1 (red): vitest: ポップアップ3種の定型プロンプト組み立て(選択範囲の有無)/ループ指示文の内容/1整え=1ターン
- [x] Step 2 (green): デザイン済みポップアップの実装(i18n)。knowledge.rsへ整えループ指示を追記(Rustテスト)。実機確認(乱雑な配置→配置整理→メトリクス改善+undo一発)+コミット

**実装したもの**:
- `src/composables/tidy.ts` — 定型プロンプト(`tidyPrompt`)と送信(`runTidy`)。プロンプトは「範囲/やること/進め方」の3節で、進め方に`get_tidy_metrics`の計測→編集→再計測・停止条件(改善が止まる or 最大3回)・ビフォー/アフター報告を含む。選択があればエンティティidを列挙して「これ以外は動かさない」と指定、無ければシート全体
- `src/components/chat/ChatTidyMenu.vue` — .pen「P自動反復」どおりのポップアップ。入力フッタの杖ボタン(`wand-sparkles`)から開く。反復モードチップを押した瞬間に1ターン送信(undo一発)。バリアント数はフェーズ3予定のためログのみ
- `madake-agent::knowledge` WORKFLOW_RULESに「図面を整えるときのルール」を追記(ポップアップを使わず自然文で頼まれたときも同じループを回す)
- `.pen`のチャットフッタ3か所へ整えボタンを追加、design-system.mdに「整えポップアップ」を記載

### Task 3: 並列エージェント

- [x] Step 1 (red): Rustテスト: 2会話の同時ターン実行(直列化されない)/図面編集の整合(revision順)/会話別turn_seq独立。TS: 会話別オーバーレイ色の割当
- [x] Step 2 (green): 排他解除+色割当+UI(会話切替中も他会話が動く表示)。実機確認+コミット

**分かったこと・実装したもの**:
- **Rust側の排他は元々無かった**: `AgentManager::send`のBusy判定は会話単位(`running: HashMap<Uuid, _>`)で、ターンは`tokio::spawn`。待ち合わせ用フェイクCLI(`fake_claude_rendezvous.sh`: 相手のターンが来るまで待ってから応答する)を足したテストで、2会話が実際に同時に走ることを確かめた。**塞いでいたのはフロント**(`chat.streaming`がグローバルで、どこか1会話でも走っていると送信不可だった)
- **`AgentManager::undo_turn`**: 巻き戻した区間にすっぽり入る**他会話のターン**も巻き戻し済みにする(`ManagerState::mark_swept_turns`)。エージェント編集は由来だけを見て戻すため会話別には選り分けられず、並行ターンの編集は一緒に戻る。放置すると「適用済み」表示のまま空振りする「元に戻す」が残る
- **フロント**: `chat.streaming`=**開いている会話**が答えているか(送信ガード・停止ボタン)、`anyStreaming`/`runningIds`/`runningCount`を追加。実行中は`running: Record<id, boolean>`で持つのでイベント由来のまま会話一覧を取り直しても消えない
- **会話色**: `theme.agentPalette`(#29D3E6 / #FFB454 / #B48CFF / #FF6FD8)を会話の開始順に割当(`conversationColor`)。編集オーバーレイ(`Region.color`)と会話履歴ポップアップの行ドット・スピナー、タブ行の「N会話実行中」バッジで使う。patchは会話を持たないので、複数会話が同時に走っている間のpatch由来の領域は既定色

### Task 4: AnthropicApiBackend+キーチェーン

- [x] Step 1 (red): Rustテスト: AgentBackend実装のツールブリッジ(MCPツール定義→API tool定義)/ツール実行ループ(モック)/キーチェーン保存・取得・削除(テストはモック/スキップ可能に)/設定にキーが平文で残らない
- [x] Step 2 (green): 実装(keyringはレジストリでバージョン確認)。設定UI(プロバイダ選択+キー入力+接続テスト、i18n)。CLI無し環境での動作を実機確認+コミット

**実装したもの**:
- **`AgentBackend`トレイト** (`madake-agent::backend`): `run_turn(TurnRequest, tx) -> Result<()>`の1本だけ。`TurnRequest`は`{prompt, session, system_prompt, history}`で、出力は今までどおり`AgentEvent`のストリーム。既存の`ClaudeCodeCliBackend`をこのトレイトに載せ替え(`send_with_context`へ委譲)、`AgentManager`は`Box<dyn AgentBackend>`を回すだけになった。**UIはバックエンドの違いを知らない**
- **`AnthropicApiBackend`** (`madake-agent::anthropic`): Messages API直結 (`POST /v1/messages`、`stream: true`のSSE)。`system`は`knowledge::system_prompt`(CLI経路と同一)。ツールは`ToolBridge`の定義をAPIの`tools`へ変換し、`stop_reason=tool_use`ならこちらでツールを実行して`tool_result`を積み直す往復をRust側で回す(上限16往復)。APIはステートレスなので会話の過去の本文を毎回送り直す。エラーは種類つき(`auth`/`overloaded`/`rate_limit`/`model_not_found`/`server`/`request`/`network`)で返し、UIが対訳を出す。接続先は`MADAKE_ANTHROPIC_BASE_URL`で差し替え可能(テスト・社内ゲートウェイ用)
- **ツールブリッジ** (`madake-agent::tools` + `madake-mcp::tool_bridge`): ディスパッチを書き写さず、**内蔵MCPサーバーをプロセス内パイプ越しに呼ぶMCPクライアント**(rmcpの`serve_directly`×2 + `tokio::io::duplex`)。ツールを1つ足せばCLI経由でもAPI経由でも同時に増える。編集は当然Commandエンジン・undo履歴・patch配信を通る(テストで確認)
- **キーチェーン** (`madake-agent::secrets`、`keyring 4.1.6`): `service="MadakeCAD"` / `account="anthropic_api_key"`。設定ファイルには項目自体を作らない。`SecretStore`トレイトで差し替え可能にしてCI・テストはメモリ保管、実キーチェーンのテストは`MADAKE_KEYCHAIN_TESTS=1`のときだけ走る。**読み出しはプロセスに1回だけキャッシュする**(macOSはアプリの署名が変わると許可ダイアログを出すため、状態取得のたびに聞かれると操作が止まる)。読めなかった理由は`keychain_error`として設定画面へ出し、3秒で諦めて画面を固めない
- **設定**: `provider: "claude_cli" | "anthropic_api"`(既定はclaude_cli。知らない値はclaude_cliへフォールバック)+`api_model`(既定`claude-sonnet-5`)。設定UIはプロバイダを実選択のドロップダウンにし、API選択時だけ「Anthropic API」グループ(伏せ字のAPIキー欄+保存/削除+「キー保存済み」バッジ、モデル欄、接続テスト)を出す。接続バッジは CLI=検出結果 / API=キーの保存状況
- **実機確認** (キー無しの範囲): プロバイダ切替→UIが切り替わる/ダミーキー保存→「キー保存済み」+伏せ字表示/接続テスト→本物のAPIから401→「APIキーが受け付けられませんでした」/削除→「キー未設定」へ戻る/キー未設定のまま送信→チャットに設定画面への案内。**実キーでの通し(作図・ツール往復)は未確認**
- **ついでに直した既定バグ**: 新規会話の初回送信が失敗したとき、エラーが画面に出ず「作業しています...」のまま止まっていた(ストアへ入れる前の生オブジェクトを書き換えていてリアクティブに届いていなかった)

### Task 5: 受け入れと仕上げ

- [ ] Step 1: 受け入れ: 乱雑な回路が「配置整理」で交差・重なり減+2.5mmグリッドに収束しundo一発/2会話並行で別々の編集が色分け表示/claude CLI無しでAPIキーのみでチャット作図が動きキーがファイルに残らない
- [ ] Step 2: docs/09(EN+JA)・feature-inventory・roadmap更新。全テストgreen。図面後始末

## 受け入れ基準

- 「配置整理」で交差数・重なり数が減り、全エンティティが2.5mmグリッド上、undo一発で戻る
- 2会話の同時実行で編集が会話色で区別され、図面が壊れない
- APIキーのみでチャット作図(キーはキーチェーンのみ、ログ・設定ファイルに平文なし)
