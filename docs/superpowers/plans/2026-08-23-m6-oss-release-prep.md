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

- [x] Step 1 (red): PDFフォント選択のOS別フォールバック(テスト: 各OS想定で候補順が正しい・見つからない場合の挙動)(origin M6 / 統合修正波4 で実施: `src-tauri/crates/madake-core/src/pdf.rs` `preferred_families()`+`preferred_families_follow_the_operating_system` テスト)
- [x] Step 2 (green): 実装+CI 3OS化(.github/workflows/ci.yml)+バンドル設定確認(dmg/msi/AppImage)。**CIはpushしないので構文検証まで**(actionlint相当でチェック)。コミット(origin M6 / 統合修正波4 で実施: `.github/workflows/ci.yml`の`rust-cross`/`frontend`/`freecad`/`bundle`3OSマトリクスと`.github/workflows/release.yml`。**未push・CI実走は未確認**。構文はこのクローズアウトでPyYAMLロード確認済み(両ファイルOK、ジョブ列挙一致)。actionlint自体は未実施(環境に無い、統合修正波4の記載どおり))

### Task 3: i18n仕上げ

- [ ] Step 1 (red): 残リテラル検出テスト(Vueテンプレート/スクリプト内の日本語直書きを検出する仕組み=lintテスト)。既存キー一致テストの拡張(未着手のまま残っている。origin M6計画は**別の検証で代替**: `src/i18n/i18n.test.ts`のキー一致・非空・プレースホルダ一致・複数形数一致テストへ拡張した(6ロケール)。統合修正波4 C.4 も「残リテラル検出テストは未着手」と明記し、続きとして「非EN値はEN値と異なる」テスト+固有名詞許容リストを挙げている。lint的な日本語直書き検出そのものは依然オープン)
- [x] Step 2 (green): 残リテラル移行+新言語の方針実装(決定を記録)。コミット(origin M6 / 統合修正波4 で実施: `src/locales/{zh,es,fr,de}.json`を英語から実訳で追加、`SUPPORTED_LOCALES`を6言語化。翻訳方針は「プレースホルダのみ」ではなく実訳を採用(2026-09-27決定)。ただし統合修正波4 C.4 の実測でzh 62.7%/es 61.6%/fr 60.6%/de 60.6%(英語と同一値=未訳とみなす)、README/i18n仕様に正直に記載済み。残りは英語表示のまま)

### Task 4: コミュニティ整備

- [x] Step 1: CONTRIBUTING.md(.ja)・CODE_OF_CONDUCT.md・SECURITY.md・.github/ISSUE_TEMPLATE/*・PULL_REQUEST_TEMPLATE.md。開発フロー(docs→design→plan→TDD・tests-as-spec・i18n・design-first)を明文化(origin M6 / 統合修正波4 で実施: `CONTRIBUTING.md`+`CONTRIBUTING.ja.md`・`CODE_OF_CONDUCT.md`・`SECURITY.md`・`.github/ISSUE_TEMPLATE/{bug_report,feature_request}.yml`+`config.yml`・`.github/PULL_REQUEST_TEMPLATE.md`)
- [x] Step 2: README最終化(バッジ・スクリーンショット・クイックスタート検証)+`docs/02-getting-started`の3OS手順。コミット(origin M6 / 統合修正波4 で実施: `README.md`/`README.ja.md`にCI/spec/status/platform/license バッジ、Contributingへの導線、翻訳率の正直な記載。`docs/02-getting-started(.ja).md`にOS別ビルド依存表・3OS対応の明記。このクローズアウトで`README(.ja).md`の`platform`バッジが「Win/Linux planned」のまま3OS CI完了後も古かったのを発見し「macOS | Windows | Linux」へ修正した。スクリーンショット画像そのものは同梱していない(README本文は説明文+バッジのみで、画像埋め込みは無い。過剰な主張ではないので未対応のまま))

### Task 5: 公開手順書と受け入れ

- [x] Step 1: `docs/internal/release-runbook.md` — 公開時にユーザーが実行する手順(リポジトリ作成・可視性・トピック・リリースタグ・配布物添付・公開後の初動)。**実行はしない**(origin M6 / 統合修正波4 で実施: `docs/internal/release-runbook.md`(日本語正+英語要約§8)。冒頭で「一つも実行していない」と宣言。手順1に`audit_confidential.py`実行を配置)
- [x] Step 2: 受け入れ: クリーンなクローン相当(`git clone`をローカルへ)から`docs/02-getting-started`通りにビルド・テストが通ることを確認。全テストgreen+gen_spec。未決事項(公開方法・名称・タイミング)をユーザー判断事項として整理。コミット(このクローズアウトで実施。結果は本コミットの下部「クリーンクローン受け入れ結果」節を参照)

## 受け入れ基準

- 公開リポジトリに社外秘が存在しない(作業ツリー・履歴とも監査済み、結果が文書化されている)
- 3OS CIワークフローが用意され、ローカルで構文・想定が検証されている
- コミュニティ文書一式が揃い、READMEから辿れる
- 公開手順書があり、ユーザーが判断すべき事項が明示されている
- **公開そのものは行われていない**(ユーザーの操作を待つ)

## 未決事項(ユーザー判断、2026-09-28クローズアウト時点)

- **公開方法**: 現リポジトリをそのまま公開(A案) vs 新規リポジトリへのスカッシュエクスポート(B案)。監査記録・runbookともB案を推奨(理由: 全コミット履歴を対象にした機密スキャンで**29件のヒット**が確認されている。作業ツリーは`scripts/audit_confidential.py --tree`で0件=クリーンだが、履歴は書き換えていない)
- **名称**: リポジトリ名・プロダクト名は未確定(READMEやrunbookは仮称のまま)
- **タイミング**: 公開時期は未決定
- **zh/es/fr/de の翻訳**: 実測(統合修正波4、全1276キー)でja 97.3%に対しzh 62.7%/es 61.6%/fr 60.6%/de 60.6%。残りは英語表示のまま(README/i18n仕様に明記済み)。443キー分の翻訳そのものはこの計画のスコープ外として据え置かれている

## クリーンクローン受け入れ結果(Task 5 Step 2、2026-09-28実施)

`git clone /Users/ktakeuchi/Documents/MadakeCAD /tmp/madake-clean-clone`(master@79bb6fd相当)をローカルクローンし、`docs/02-getting-started.md`の手順を順に実行。完了後クローンは削除済み。

| 手順 | 結果 |
|---|---|
| `node --version` / `cargo --version` | v26.5.0 / 1.97.1。両方ともPATH変更なしで通った |
| `npm install` | OK(97 packages, 0 vulnerabilities) |
| `cd src-tauri && cargo test --workspace` | OK。876+154+257件相当が全緑(実測: 個々のクレート合計で0 failed)。所要 約2分(fix-wave-4記載の15-25分より大幅に速い。register/依存キャッシュが効いた可能性) |
| `npx vitest run` | OK。75 files, 1035 passed |
| `npx vue-tsc --noEmit` | OK(出力なし=クリーン) |
| `python3 scripts/gen_spec.py --check` | OK(`spec docs up to date; all tests have bilingual spec comments`) |
| `python3 -m unittest discover -s freecad-addon/tests` | OK。38 tests |
| アプリビルド | `npm run tauri build`はインストーラ署名・パッケージング工程まで含み今回は省略。代わりに`cd src-tauri && cargo build -p madake-cad`を実行しOK(24秒、`Finished dev profile`) |

**doc/02の不備として見つかり、このクローズアウトで修正したもの**(EN+JA両方):
- `npx vue-tsc --noEmit`がJA版にはあったがEN版に無かった(EN/JA不同期)→EN版へ追加
- `python3 scripts/gen_spec.py --check`と`python3 -m unittest discover -s freecad-addon/tests`がどちらの言語版にも記載が無かった→両方へ追加
- `cargo test`がワークスペース全体を対象にすること(`default-members`が無い)を明記し、`cargo test --workspace`と等価であると注記
- アプリビルドの節に、フル`tauri build`を省いて`cargo build -p madake-cad`だけでコンパイル確認する選択肢を追記
- README(.ja).mdの`Platform`バッジが「macOS (Win/Linux planned)」のまま3OS CI完了後も古かったのを`macOS | Windows | Linux`へ修正(Task 4 Step 2との関連で発見)

結論: 手順は(上記の追記後)クリーンクローンから最後まで通る。全テストgreenで、gen_specも最新。
