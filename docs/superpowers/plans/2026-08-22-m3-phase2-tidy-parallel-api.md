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

- [ ] Step 1 (red): Rustテスト: 交差数(交差あり/共有端点は数えない/平行)/重なり数(ラベル同士・ラベルとシンボル/接しているだけは数えない)/グリッド外検出/決定性。MCP `get_tidy_metrics`の応答形
- [ ] Step 2 (green): 実装+露出。gen_spec→コミット

### Task 2: 整えポップアップ(UI+プロンプト)

- [ ] Step 1 (red): vitest: ポップアップ3種の定型プロンプト組み立て(選択範囲の有無)/ループ指示文の内容/1整え=1ターン
- [ ] Step 2 (green): デザイン済みポップアップの実装(i18n)。knowledge.rsへ整えループ指示を追記(Rustテスト)。実機確認(乱雑な配置→配置整理→メトリクス改善+undo一発)+コミット

### Task 3: 並列エージェント

- [ ] Step 1 (red): Rustテスト: 2会話の同時ターン実行(直列化されない)/図面編集の整合(revision順)/会話別turn_seq独立。TS: 会話別オーバーレイ色の割当
- [ ] Step 2 (green): 排他解除+色割当+UI(会話切替中も他会話が動く表示)。実機確認+コミット

### Task 4: AnthropicApiBackend+キーチェーン

- [ ] Step 1 (red): Rustテスト: AgentBackend実装のツールブリッジ(MCPツール定義→API tool定義)/ツール実行ループ(モック)/キーチェーン保存・取得・削除(テストはモック/スキップ可能に)/設定にキーが平文で残らない
- [ ] Step 2 (green): 実装(keyringはレジストリでバージョン確認)。設定UI(プロバイダ選択+キー入力+接続テスト、i18n)。CLI無し環境での動作を実機確認+コミット

### Task 5: 受け入れと仕上げ

- [ ] Step 1: 受け入れ: 乱雑な回路が「配置整理」で交差・重なり減+2.5mmグリッドに収束しundo一発/2会話並行で別々の編集が色分け表示/claude CLI無しでAPIキーのみでチャット作図が動きキーがファイルに残らない
- [ ] Step 2: docs/09(EN+JA)・feature-inventory・roadmap更新。全テストgreen。図面後始末

## 受け入れ基準

- 「配置整理」で交差数・重なり数が減り、全エンティティが2.5mmグリッド上、undo一発で戻る
- 2会話の同時実行で編集が会話色で区別され、図面が壊れない
- APIキーのみでチャット作図(キーはキーチェーンのみ、ログ・設定ファイルに平文なし)
