# 内部資料(開発用)

エージェント/コントリビュータが開発するための内部ドキュメント。公開用ドキュメント(初見者向け・連番)は[`docs/`直下](../01-overview.md)、目次は[README](../../README.md)。

| ドキュメント | 内容 |
|---|---|
| [requirements.md](requirements.md) | 要件定義書: ビジョン5本柱(G1-G5)・マイルストーン(M1-M6)・機能/非機能要件・設計原則 |
| [architecture.md](architecture.md) | システム設計書: 構成図・ファイルマップ・データフロー・状態遷移・**変更レシピ** |
| [data-model.md](data-model.md) | データ設計書: ドキュメントモデル・部品DB ER図・派生データ |
| [tech-stack.md](tech-stack.md) | 技術スタック: バージョン・選定理由・不採用判断 |
| [design-system.md](design-system.md) | デザインシステム: UIトークン・コンポーネント規約・操作規約(ビジュアルの正は`MadakeCAD.pen`) |
| [feature-inventory.md](feature-inventory.md) | 機能インベントリ: 実装済み/未実装の棚卸し |
| [specs/](specs/README.md) | 機能仕様集: M2〜M6の詳細仕様+[ギャップ分析](specs/gap-analysis.md)(ACADE/EPLAN比較) |
| [../superpowers/specs/](../superpowers/specs/2026-08-20-madakecad-design.md) | マスタースペック: アーキテクチャ設計判断の記録 |
| [../superpowers/plans/](../superpowers/plans/) | 実装プラン(日付順、TDDステップ・進捗付き) |
| `../references/` | 開発方針の原点・参考図面(社外秘のためgit管理外) |

進行方針(2026-08-21決定): **機能仕様 → 全体デザイン(Pencil) → 実装計画 → 順次実装**。ドキュメントは英語が正本のルール(CLAUDE.md)だが、内部資料は段階的に移行中(現状は日本語が正)。
