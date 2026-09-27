# MadakeCAD M6 (オープンソース公開の準備) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M6仕様のうち、公開そのもの(公開方法・名称・時期=ユーザー判断)を除く準備をすべて揃える: §2 リポジトリ衛生の道具、§3 クロスプラットフォーム(3OSのCI・配布ワークフロー・PDFフォント)、§4 i18n(残リテラルの移行・zh/es/fr/deカタログ・翻訳者向け文書)、§5 コミュニティ整備(CONTRIBUTING・行動規範・Issue/PRテンプレート・SECURITY)。

**共通ルール:** tests-as-spec+gen_spec。ドキュメントEN+JA同期。UI文字列はカタログ経由(i18n仕様 §4)。

**Spec:** `docs/internal/specs/m6-oss.md`、`docs/internal/specs/i18n.md` F3

## 設計決定(2026-09-27)

- **CI**: 既存`ci.yml`にOSマトリクスを追加。Ubuntuはフルスイート、macOSは4クレート(bashのフェイクCLIが動く)、WindowsはOS非依存の`madake-core`+`madake-cli`(エージェントのテストはシェルスクリプトのフェイクに依存するため)。配布物は別ワークフロー`release.yml`(タグ`v*`または手動)で`tauri-apps/tauri-action`により macOS(arm64/x86_64 dmg)・Windows(msi/nsis)・Linux(AppImage/deb)を**ドラフトリリース**へ添付。コード署名・notarization・自動更新は公開後の課題(仕様どおり)
- **PDFフォント**: `pdf.rs`の書体割当をOS別の純関数`preferred_families()`にし、macOS=Hiragino Sans/Menlo、Windows=Yu Gothic UI/Consolas、Linux=Noto Sans CJK JP/DejaVu Sans Mono(テストで固定)。Linuxは`fonts-noto-cjk`の導入を`02-getting-started`に記載
- **i18n**: 残リテラルのうち**UI文字列**(パネル・リボン・ツールチップ・ログ・チャットの定型文)をカタログへ移行。**AIへのプロンプト本文**(`tidy.ts`の整え指示、`drawingContext.ts`の図面コンテキスト)と**図面内容**(`renderer.ts`の表題欄・改訂欄見出し=作図言語はプロジェクト設定、i18n仕様 §2)は対象外。zh/es/fr/deは英語カタログからの翻訳で作り、キー一致テストを全ロケールへ拡張。言語ドロップダウンは`SUPPORTED_LOCALES`から自動で増える
- **衛生**: 社外秘用語の再監査は用語表がgit管理外(`docs/references/`)なので、`scripts/audit_confidential.py`(作業ツリー+全履歴を用語表で走査)を用意し、公開前に手元で実行する運用にする

### Task 1: クロスプラットフォーム(§3)

- [x] Step 1 (red): `pdf.rs`テスト: `preferred_families()`が実行中OSの想定書体を返す
- [x] Step 2 (green): `preferred_families()`+`fontdb()`への適用、`ci.yml`のOSマトリクス、`release.yml`、`docs/02-getting-started(.ja).md`(OS別ビルド依存・フォント・`npm run tauri build`)

### Task 2: コミュニティ整備(§5)+衛生(§2)

- [x] `CONTRIBUTING.md`(+`.ja`)、`CODE_OF_CONDUCT.md`(Contributor Covenant 2.1)、`SECURITY.md`、`.github/ISSUE_TEMPLATE/{bug_report,feature_request}.yml`+`config.yml`、`.github/PULL_REQUEST_TEMPLATE.md`、`scripts/audit_confidential.py`、`.gitignore`

### Task 3: i18n F3(§4)

- [x] Step 1 (red): `i18n.test.ts`を全ロケールのキー一致・非空へ拡張、`SUPPORTED_LOCALES`にzh/es/fr/de
- [x] Step 2 (green): 残UIリテラルの移行(RibbonBar回路図タブ・PropertiesPanel・ProjectPanel・StatusBar・TitleBar・LeftPanel・FileTabs・Simulation/VerificationPanel・App・chat系・viewClasses・chat.tsの表示名)、`zh.json`/`es.json`/`fr.json`/`de.json`、翻訳者向け文書(`docs/internal/specs/i18n.md`に節を追加)

### Task 4: ドキュメント

- [x] m6仕様(各節の状態)、i18n仕様(F3)、roadmap(M6)、README(ステータス段落の更新)、feature-inventory、仕様書再生成。全テストgreen

## 受け入れ基準

- CIが3OSで走り、`release.yml`がタグで3OSの配布物をドラフトリリースへ添付する
- 全ロケールのカタログがキー一致し、言語設定で6言語を切り替えられる
- CONTRIBUTING/行動規範/テンプレートが揃い、READMEから辿れる

## 実施記録(2026-09-27)

- Task 1: `pdf.rs::preferred_families()`(macOS=Hiragino Sans/Menlo、Windows=Yu Gothic UI/Consolas、他=Noto Sans CJK JP/DejaVu Sans Mono)+テスト。`ci.yml`に`rust-cross`ジョブ(macOS=4クレート、Windows=`madake-core`+`madake-cli`)。`release.yml`(タグ`v*`/手動、tauri-action、macOS arm64/x86_64・ubuntu-22.04・windows、ドラフトのプレリリース)。`docs/02-getting-started(.ja).md`にOS別ビルド依存表と配布物の段落
- Task 2: `CONTRIBUTING.md`(+ja)・`CODE_OF_CONDUCT.md`(Contributor Covenant 2.1)・`SECURITY.md`・`.github/ISSUE_TEMPLATE/{bug_report,feature_request}.yml`+`config.yml`・`.github/PULL_REQUEST_TEMPLATE.md`・`scripts/audit_confidential.py`(用語表`docs/references/confidential-terms.txt`はgit管理外、`.gitignore`済み)
- Task 3: 残UIリテラルをカタログへ移行(App/TitleBar/StatusBar/ProjectPanel/FileTabs/LeftPanel/PropertiesPanel/RibbonBar回路図タブ・ビュークラス/Simulation・VerificationPanel/chat系5コンポーネント+`models.ts`/`stores/ui.ts`/`stores/chat.ts`の表示名・要約・相対時刻)。en/jaカタログは828キー。`zh.json`/`es.json`/`fr.json`/`de.json`を英語から翻訳、`SUPPORTED_LOCALES`を6言語へ、`i18n.test.ts`をキー一致・非空・プレースホルダ一致・複数形の数一致・言語名の自称へ拡張。エージェント向けプロンプト(`tidy.ts`/`drawingContext.ts`)の種別名は`entityKindLabel(kind, "ja")`で日本語固定。翻訳者向け手順はi18n仕様 §6
- Task 4: m6仕様(§2〜§5の状態)、i18n仕様(F3・§6)、roadmap M6、README(ステータス段落・Contributingのリンク)、feature-inventory(PDFフォント・Win/Linuxビルド・配布・i18n・コミュニティ整備)、仕様書再生成
- 対象外として残したもの: `MadakeCAD.pen`への反映(Pen.app未接続。UIの見た目は変えていない)、コード署名・notarization・自動更新(公開後)、公開方法・名称・時期(ユーザー判断)
