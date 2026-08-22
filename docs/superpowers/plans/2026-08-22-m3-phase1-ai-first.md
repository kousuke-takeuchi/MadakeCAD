# MadakeCAD M3フェーズ1 (AI-first: 基盤信頼性+規格知識) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M3仕様の§1(A1負債解消)+§2(規格知識・検証ループ・テンプレート)+§2b〜2d(ナレッジ/AIレビュー/部品選定)を実装し、「非熟練者の一文指示からERC Errorゼロ・線番付きの図面が生成される」状態を作る。仕様=`docs/internal/specs/m3-ai-first.md`。§3(整えループ)・§4(並列)・§5(マルチプロバイダ)はフェーズ2。

**共通ルール:** 全編集はCommand経由。tests-as-spec(対訳仕様文)+タスクごとに`gen_spec.py`再生成。新規UI文字列はi18n(en/ja)。ドキュメントEN+JA同期。

## 設計決定

- **ターン安定ID**: `ChatMessage`に`turn_id: Uuid`を追加し、`undo_turn(turn_id)`へ移行(message_index指定は廃止)。chat.jsonの`format_version`を上げ、旧ファイルは読み込み時にturn_idを採番して移行
- **編集origin**: Engineの履歴エントリに`origin: user | agent | mcp`(既定user)。`Engine::execute_as(origin)`を追加し、ターン巻き戻しは「そのturnのagent編集のみ」を逆適用(間に挟まったユーザー編集は保持。逆適用が衝突する場合はターン巻き戻し不可としてエラー明示)
- **キャンセルseq**: broadcastイベントに`turn_seq`を付与し、キャンセル済みturnの遅延イベントをフロントで破棄
- **規格知識の管理形式(未決→決定→実装済み)**: 編集可能なMarkdown同梱(`src-tauri/resources/knowledge/standards.md` — JIS C 0617記号の使い分け・参照記号接頭辞・線色/sq慣習・線番/ハーネス/XRefの決まり・作図手順のベストプラクティス)。設定`knowledge_path`でユーザー追記ファイルを追加可。エージェント起動時に`--append-system-prompt`へ注入
- **検証ループ**: システムプロンプトで「図面編集後は必ず`run_verification`を実行し、Errorが残れば修正して再検証(最大3回)。最終結果を報告」と指示。図面コンテキストに検証サマリを含める(既存drawingContext拡張)
- **テンプレート**: Command列JSON(`resources/templates/*.json`: 24V制御基本・モータ起動・非常停止の汎用3種。参考図面由来はユーザー確認待ちのため含めない)。適用=既存`execute_commands`で一括実行(1ターン=undo一発)。UI=新規作成時+リボン「プロジェクト」相当からのテンプレート選択(デザイン: .pen「M3デザイン - テンプレート選択」※本フェーズで作成)
- **ナレッジサービス(2b)**: システムプロンプトにdocs/の構成(01〜13の目次と内部仕様の場所)と「操作・規格の質問にはドキュメントを読んで出典付きで答える」を記載。claude CLIはcwdのファイルを読めるため専用ツールは不要
- **AIレビュー(2c)**: プロンプトに「レビュー観点チェックリスト」(参照記号体系・線色/sq慣習・ラベル命名・レイアウト・規格)を含め、`run_verification`の結果と合わせた指摘一覧の書式を指定。修正はユーザー承認後
- **部品選定(2d)**: MCPに部品検索ツールが露出済みか確認し、比較表・代替提案はプロンプト書式で実現(新ツールは必要最小限)

### Task 1: ターン安定ID (madake-agent)

- [x] Step 1 (red): Rustテスト: ChatMessageにturn_idが付く/undo_turn(turn_id)が該当ターンのみ戻す/旧chat.json(format_version旧)が移行読み込みできる/存在しないturn_idはエラー
- [x] Step 2 (green): 実装(format_version 1→2、読み込み時にターン境界からturn_idを採番して移行)。`undo_turn(conversation_id, turn_id)`へ移行し、IPC(`agent_undo_turn`)・Link API(`POST /agent/undo-turn`の`turn_id`)・フロント(chat store/ChatDock)を更新。`TurnApplied`イベントに`turn_id`を追加。既に巻き戻し済みのターンは`TurnNotApplied`、未知IDは`UnknownTurn`で明示エラー

### Task 2: 編集origin+キャンセルseq (madake-core Engine + agent)

- [x] Step 1 (red): Rustテスト: execute_asでorigin記録/undo_turnがagent編集のみ逆適用しuser編集を保持/衝突時は明示エラー/turn_seq付きイベントとキャンセル後イベントの破棄(フロントはvitest)
- [x] Step 2 (green): 実装。IPC/MCP/Link APIの実行経路へoriginを配線(UI=user、エージェント=agent、外部API/CLI=mcp)。gen_spec→コミット

実装メモ:
- 巻き戻しは`Engine::revert_range(start_depth, end_depth, EditOrigin::Agent)`。ターンの記録した**undo深さ区間**の中でagent由来の履歴エントリだけを新しい逆Commandとして適用する。衝突判定は「逆コマンドの適用が`CoreError`になること」(手編集で対象が消えた`UpdateEntity`の戻し等)で、その場合は適用前のプロジェクトへ巻き戻して`RevertConflict`(部分適用しない)
- 巻き戻し自体は通常の履歴エントリ(origin=user)として積むため、undoで「巻き戻しの取り消し」ができ、redo履歴は通常の編集と同じく破棄される
- 「最新の適用済みターンのみ」制限(`NotLatestTurn`)は撤廃。ターン中・ターン後のユーザー手編集は保持されるため、古いターンも安全に戻せる(UIは従来どおり直前ターンの1ボタン)
- MCPサーバーの入口はUI/外部と共通なので、originは`SharedDoc`の「エージェントターン実行中」カウンタ(RAIIガード。中断時も必ず解除)で見分ける
- `turn_seq`は**エージェントイベント(ConversationEvent)にのみ**付与。patchへの付与は見送り: 遅延patchを捨てると図面ミラーがバックエンドと乖離するため、キャンセル後もpatchは必ず適用する(patchの重複・古さは既存の`revision`で判定)

### Task 3: 規格知識+検証ループ+図面コンテキスト拡張

- [x] Step 1 (red): Rustテスト: システムプロンプトにknowledge/standards.mdの内容と検証ループ指示が含まれる/knowledge_path設定の追記が反映/図面コンテキストに検証サマリ(Error/Warn件数と先頭数件)が入る
- [x] Step 2 (green): standards.md執筆(JIS/IEC要点・MadakeCAD作図手順)+注入実装+設定追加(AppSettings拡張はlanguageの前例に倣いテスト込み)。gen_spec→コミット

実装メモ:
- **注入経路**: `AgentManager::send`が`madake_agent::knowledge::system_prompt(settings, drawing_context)`で`--append-system-prompt`を組み立てる。順は 図面コンテキスト → 作図・検証ループのルール → 同梱standards.md → 設定の知識ファイル(後勝ちで上書きできる)。**「図面の自動読み取り」OFFでも規格知識と検証ループ指示は必ず載る**(図面を伏せるだけで、規格を忘れさせない)
- **standards.mdの解決**: 実体は`src-tauri/resources/knowledge/standards.md`(編集可能なMarkdown同梱)。Tauri起動時に`resolve_standards_resource`がリソース(配布時)→`CARGO_MANIFEST_DIR/resources`(開発時)の順に実体を探し、`MADAKE_STANDARDS_PATH`へ入れる。どちらも無い場合はビルド時に`include_str!`で埋め込んだ同内容へフォールバック(エージェントが無知にならない)
- **図面コンテキストの検証サマリ**: `drawing_context`が`verify_project`を呼び「エラーN件/警告N件/情報N件」+重要度順の先頭3件(code・メッセージ)を載せる。全件は`run_verification`で取り直させる
- 設定UI: 「エージェント > 詳細設定」に知識ファイルのパス入力行を追加(i18n済み)。ファイル選択ダイアログではなくパス直接入力(claude実行ファイル欄と同じ様式)
- 残件: standards.mdは日本語のみ(エージェント向け。UI言語に応じた英語版はM3以降)。テンプレート(Task 5)からもこの知識を参照させるかは未検討

### Task 4: ナレッジ/AIレビュー/部品選定のプロンプト整備

- [ ] Step 1 (red): Rustテスト: プロンプトにdocsガイド・レビューチェックリスト・比較表書式が含まれる/MCPの部品検索ツールの説明が選定用途を含む
- [ ] Step 2 (green): 実装+チャットからの実機確認(「線番の付け方は?」出典付き回答/「レビューして」指摘一覧/「MY2Nの代替は?」比較表)。結果を記録してコミット

### Task 5: テンプレート

- [ ] Step 0 (design): .pen「M3デザイン - テンプレート選択」(メインエージェントが作成済みであること)
- [ ] Step 1 (red): Rustテスト: テンプレートJSONの列挙・読み込み/適用が1ターン(undo一発)/3テンプレートがERCクリーン。TS: 選択UIのコマンド組み立て
- [ ] Step 2 (green): 同梱テンプレ3種作成+UI実装(i18n)。実機確認+コミット

### Task 6: 受け入れ検証と仕上げ

- [ ] Step 1: 受け入れ基準の実機検証: 「24V電源からヒューズ経由でランプを2灯、個別スイッチ付きで」の一文指示→ERC Errorゼロ・線番付き図面が生成される(チャット経由、検証ループの自走を確認)。ターン巻き戻し(手編集を挟んでも安全)も確認
- [ ] Step 2: docs/09-ai-assistant(EN+JA)更新、feature-inventory・roadmap M3を🔶へ。全テストgreen+gen_spec --check。図面後始末

## 受け入れ基準

- ターン中の手編集後でも「元に戻す」がエージェント編集のみを戻す。過去ターンの巻き戻しがturn_idで安全に効く
- 一文指示から検証ループ自走でERC Errorゼロ・線番付き図面
- 「線番の付け方は?」に出典付きで回答。「レビューして」で決定的検証+慣行観点の指摘一覧
- テンプレート適用がundo一発で戻る
