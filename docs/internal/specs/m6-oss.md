# M6 機能仕様: オープンソース公開

対応する柱: G5。「OSSとして提供すること自体が重要」(ユーザー方針)。公開は一度きりのイベントではなく、**早期に決めるべき事項(ライセンス)と、公開時までに揃える事項**に分ける。

## 1. ライセンス選定(早期決定が必要)

依存の状況(2026-08-21時点): Tauri/Vue/rmcp/axum/svg2pdf等は全てMIT/Apache系。rusqlite(bundled SQLite=Public Domain)も問題なし。ngspice・claude CLIは**別プロセス実行**なのでライセンス伝播しない。→ どのライセンスも選択可能。

| 候補 | 特徴 | 向く場合 |
|---|---|---|
| **Apache-2.0 OR MIT(デュアル)** | Rustエコシステム標準。特許条項(Apache)+最大の採用しやすさ | 企業利用・コントリビュータを最大化したい(**提案**) |
| AGPL-3.0 | 改変版のSaaS提供にもソース公開義務 | 商用フォーク対策を最優先する場合 |
| GPL-3.0 | 派生物のソース公開義務 | コピーレフト方針の場合 |

- [x] **決定(2026-08-21)**: **MIT OR Apache-2.0 デュアル**。LICENSE-MIT / LICENSE-APACHE・全クレートの`license`フィールド・package.json・READMEに反映済み

## 2. 公開前チェックリスト(リポジトリ衛生)

状態(2026-09-27): 道具は揃った。`scripts/audit_confidential.py`が作業ツリーと全コミット履歴を`docs/references/confidential-terms.txt`(git管理外の用語表)で走査する。公開直前に手元で実行し、ヒットがあれば「公開用エクスポート」(未決事項)で対処する。第三者権利の確認とセキュリティ既定の確認は下記のとおり。

- **第三者権利の確認**(2026-08-21追加): ベンチマーク製品(ACADE/EPLAN)の素材(アイコン・画像・文言・スクリーンショット)を一切含まないことの確認。特定画面のデッドコピーが無いことのデザインレビュー。J-PlatPatでAutodesk / Friedhelm Loh(EPLAN)出願の画像意匠を簡易確認。商標言及は比較・参考の指示的使用に限る(機能概念の参考・独自実装はOSSの確立慣行であり問題ない)

- 社外秘の完全排除の再監査: `docs/references/`(既にgit管理外)、コミット履歴の実品番・企業名スキャン(過去に1度除去済み: 1ff7a8f)。**必要なら履歴書き換えではなく公開用に新規リポジトリへエクスポート**
- 参考図面依存の記述を一般化(「参考図面準拠」→ 設定可能な様式として説明)
- セキュリティ既定の確認: サーバーは127.0.0.1バインド+オリジンガード(実装済み)、APIキーはキーチェーン(A3)

## 3. クロスプラットフォーム

- ✅ **Windows / Linuxビルド**(2026-09-27): PDFフォント割当を`pdf.rs`の`preferred_families()`でOS別に固定(macOS=Hiragino Sans/Menlo、Windows=Yu Gothic UI/Consolas、Linux=Noto Sans CJK JP/DejaVu Sans Mono。テストで検証)。CI(`ci.yml`)はUbuntuフル+macOS 4クレート+Windows `madake-core`/`madake-cli`の`rust-cross`ジョブ。OS別のビルド依存は`docs/02-getting-started.md`に記載
- ✅ **配布物**(2026-09-27): `release.yml`(タグ`v*`または手動)が`tauri-apps/tauri-action`で macOS(arm64/x86_64 `.dmg`)・Windows(`.msi`/NSIS)・Linux(`.AppImage`/`.deb`)をドラフトのプレリリースへ添付。コード署名(macOS notarization優先)は公開後の課題
- 自動更新(tauri-updater)は公開後の課題

## 4. 国際化(i18n)

横断仕様 [i18n.md](i18n.md) へ移管(2026-08-21)。基盤(vue-i18n・言語設定・カタログ・キー一致テスト)は即時導入済み。✅ **F3完了(2026-09-27)**: 残UIリテラルを全面移行、zh/es/fr/deカタログを同梱(6言語をキー・プレースホルダ・複数形の一致テストで保護)、翻訳者向け手順をi18n仕様 §6に記載。既定言語は英語。

## 5. コミュニティ整備

- ✅ `CONTRIBUTING.md`(+`.ja`。開発フロー: docs→design→plan→TDD、`docs/internal/architecture.md`参照)(2026-09-27)
- ✅ 行動規範 `CODE_OF_CONDUCT.md`(Contributor Covenant 2.1)、`SECURITY.md`、Issueテンプレート(`bug_report`/`feature_request`+`config.yml`)、`.github/PULL_REQUEST_TEMPLATE.md`(2026-09-27)
- 既存ドキュメント群(requirements/architecture/setup等)が実質のオンボーディング資料。READMEの「Contributing」から辿れる

## 受け入れ基準

- クリーンな環境(3OS)で`docs/02-getting-started.md`通りにビルド・起動・テストが通る
- ライセンス表記が全クレート・配布物で一貫
- 公開リポジトリに社外秘が存在しない(履歴含む)

## 未決事項

残るのは公開そのもののユーザー判断(実装計画: `docs/superpowers/plans/2026-09-27-m6-oss-release.md`)。

- [x] ライセンス: MIT OR Apache-2.0(2026-08-21決定)
- [ ] 公開方法: 現リポジトリ公開 vs 公開用エクスポート(履歴の扱い)
- [ ] プロジェクト名/ドメイン(MadakeCADのまま公開か)
- [ ] 公開タイミング(M2完了後の早期公開 vs M4後の機能充実後)
