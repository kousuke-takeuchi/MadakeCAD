# MadakeCAD M6 (OSS公開準備) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M6仕様(`docs/internal/specs/m6-oss.md`)の公開前作業を完了し、「あとはユーザーが公開ボタンを押すだけ」の状態にする。ライセンス(MIT OR Apache-2.0)は決定・適用済み。

> **重要(エージェントへ):** **リポジトリの公開・GitHubへのpush・リモート作成・外部サービスへの登録は絶対に行わないこと。** 公開はユーザー本人が判断・実行する外向きの不可逆操作。本計画は準備のみ(ローカルの整備・検査・ドキュメント作成)。公開手順は「手順書」として文書化するに留める。

**共通ルール:** tests-as-spec+gen_spec・ドキュメントEN+JA同期・機密の完全排除。

## 設計決定

- **公開方法(未決→提案)**: 現リポジトリの履歴には社外秘の除去コミット(`1ff7a8f`)が残るため、**公開用に新規リポジトリへスカッシュエクスポート**(履歴を1コミットに畳む or 節目コミットのみ再構成)を推奨。判断材料を揃えてユーザーへ提示(履歴の価値 vs 機密リスク)。**実行はユーザー判断**
- **社外秘スキャン**: `docs/references/`(git管理外を再確認)+**全コミット履歴**を対象に、顧客名・実品番・ネット名等のパターンで機械的スキャン。検出物の一覧をユーザーへ報告(削除方法は提示のみ)
- **クロスプラットフォーム**: GitHub Actions CIを3OS(macOS/Windows/Linux)へ拡張(cargo test+vitest+vue-tsc+gen_spec --check+バンドルビルド)。PDFフォントのOS別フォールバック(Windows: Yu Gothic UI/Meiryo、Linux: Noto Sans CJK JP)を実装+テスト。**CIの実行はpush時なので、ワークフローファイル追加まで**
- **i18n残**: UI残リテラルの全面移行(日本語直書きの一掃)+zh/es/fr/deカタログ(**機械翻訳を人手確認なしで入れない**方針: en/jaを正とし、他言語は「翻訳募集」プレースホルダとしてキー構造だけ用意するか、確信を持てる範囲のみ訳す。判断して記録)
- **コミュニティ**: CONTRIBUTING(.ja)・CODE_OF_CONDUCT(Contributor Covenant 2.1)・SECURITY.md・Issue/PRテンプレート・`.github/`一式
- **第三者権利レビュー**: ACADE/EPLANの素材(アイコン・文言・スクショ)を含まないことの機械検査+デザインのデッドコピーが無いことのレビュー記録

### Task 1: 機密・権利の監査 ✅(2026-08-23完了)

> 結果: [`docs/internal/release-audit.md`](../../internal/release-audit.md) / 依存ライセンス: [`docs/internal/third-party-licenses.md`](../../internal/third-party-licenses.md)。
> 顧客企業名・コネクタ実型番は作業ツリー・履歴とも0件、参考図面PDFはコミット歴なし。強コピーレフト依存も0件。
> **未解決の要対応**: `MadakeCAD.pen` に顧客由来の識別子がプレースホルダとして残る(作業ツリー+push済みの14コミット)。公開方法は**新規リポジトリへのスカッシュエクスポートを推奨**(監査記録 §1.5)。

- [x] Step 1: 履歴全体+作業ツリーの機密スキャン(顧客名・企業名・実品番パターン・参考図面ファイル名)。結果を`docs/internal/release-audit.md`(一般化した集計のみ。検出文字列そのものは書かない)へ
- [x] Step 2: 第三者権利レビュー(同梱アセット・アイコン・文言の出所確認、依存ライセンス一覧の生成 `cargo license`相当+npm)。`docs/internal/third-party-licenses.md`(EN+JA不要・機械生成)
- [x] Step 3: 参考図面依存の記述を一般化(残っていれば)。コミット

### Task 2: クロスプラットフォーム

- [ ] Step 1 (red): PDFフォント選択のOS別フォールバック(テスト: 各OS想定で候補順が正しい・見つからない場合の挙動)
- [ ] Step 2 (green): 実装+CI 3OS化(.github/workflows/ci.yml)+バンドル設定確認(dmg/msi/AppImage)。**CIはpushしないので構文検証まで**(actionlint相当でチェック)。コミット

### Task 3: i18n仕上げ

- [ ] Step 1 (red): 残リテラル検出テスト(Vueテンプレート/スクリプト内の日本語直書きを検出する仕組み=lintテスト)。既存キー一致テストの拡張
- [ ] Step 2 (green): 残リテラル移行+新言語の方針実装(決定を記録)。コミット

### Task 4: コミュニティ整備

- [ ] Step 1: CONTRIBUTING.md(.ja)・CODE_OF_CONDUCT.md・SECURITY.md・.github/ISSUE_TEMPLATE/*・PULL_REQUEST_TEMPLATE.md。開発フロー(docs→design→plan→TDD・tests-as-spec・i18n・design-first)を明文化
- [ ] Step 2: README最終化(バッジ・スクリーンショット・クイックスタート検証)+`docs/02-getting-started`の3OS手順。コミット

### Task 5: 公開手順書と受け入れ

- [ ] Step 1: `docs/internal/release-runbook.md` — 公開時にユーザーが実行する手順(リポジトリ作成・可視性・トピック・リリースタグ・配布物添付・公開後の初動)。**実行はしない**
- [ ] Step 2: 受け入れ: クリーンなクローン相当(`git clone`をローカルへ)から`docs/02-getting-started`通りにビルド・テストが通ることを確認。全テストgreen+gen_spec。未決事項(公開方法・名称・タイミング)をユーザー判断事項として整理。コミット

## 受け入れ基準

- 公開リポジトリに社外秘が存在しない(作業ツリー・履歴とも監査済み、結果が文書化されている)
- 3OS CIワークフローが用意され、ローカルで構文・想定が検証されている
- コミュニティ文書一式が揃い、READMEから辿れる
- 公開手順書があり、ユーザーが判断すべき事項が明示されている
- **公開そのものは行われていない**(ユーザーの操作を待つ)
