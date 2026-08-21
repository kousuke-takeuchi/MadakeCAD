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

- **第三者権利の確認**(2026-08-21追加): ベンチマーク製品(ACADE/EPLAN)の素材(アイコン・画像・文言・スクリーンショット)を一切含まないことの確認。特定画面のデッドコピーが無いことのデザインレビュー。J-PlatPatでAutodesk / Friedhelm Loh(EPLAN)出願の画像意匠を簡易確認。商標言及は比較・参考の指示的使用に限る(機能概念の参考・独自実装はOSSの確立慣行であり問題ない)

- 社外秘の完全排除の再監査: `docs/references/`(既にgit管理外)、コミット履歴の実品番・企業名スキャン(過去に1度除去済み: 1ff7a8f)。**必要なら履歴書き換えではなく公開用に新規リポジトリへエクスポート**
- 参考図面依存の記述を一般化(「参考図面準拠」→ 設定可能な様式として説明)
- セキュリティ既定の確認: サーバーは127.0.0.1バインド+オリジンガード(実装済み)、APIキーはキーチェーン(A3)

## 3. クロスプラットフォーム

- **Windows / Linuxビルド**: コアはOS非依存を維持済み。残作業: PDFフォント割当(Windows=Yu Gothic UI等/Linux=Noto Sans CJK)、ngspice既定パス(実装済み)、CI(GitHub Actions: 3OSでcargo test+vitest+バンドル)
- **配布物**: macOS `.dmg` / Windows `.msi` / Linux `.AppImage`(Tauri bundler)。コード署名は macOS notarization を優先、他は後続
- 自動更新(tauri-updater)は公開後の課題

## 4. 国際化(i18n)

- UI文字列の辞書化(日本語→日英2言語)。vue-i18n等はデザイン/実装フェーズで選定(レジストリ確認)
- 図面側は規格様式の切替(JIS↔IEC)としてM4以降のテーマ。まずUIのみ
- ドキュメント: READMEの英語版(README.en.md)

## 5. コミュニティ整備

- CONTRIBUTING.md(開発フロー: docs→design→plan→TDD、`docs/internal/architecture.md`参照)
- 行動規範(Contributor Covenant)、Issue/PRテンプレート
- 既存ドキュメント群(requirements/architecture/setup等)が実質のオンボーディング資料

## 受け入れ基準

- クリーンな環境(3OS)で`docs/02-getting-started.md`通りにビルド・起動・テストが通る
- ライセンス表記が全クレート・配布物で一貫
- 公開リポジトリに社外秘が存在しない(履歴含む)

## 未決事項

- [x] ライセンス: MIT OR Apache-2.0(2026-08-21決定)
- [ ] 公開方法: 現リポジトリ公開 vs 公開用エクスポート(履歴の扱い)
- [ ] プロジェクト名/ドメイン(MadakeCADのまま公開か)
- [ ] 公開タイミング(M2完了後の早期公開 vs M4後の機能充実後)
