# MadakeCAD フェーズ6先行 (可読性ギャップG19〜G21) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** パリティ再検証(`docs/internal/specs/reproduction-gaps.md`)で新規判明した可読性ギャップ3件を解消する。残る差は「情報」でなく「可読性」のみのため、この3件で参考図面の見た目品質に到達する。

**共通ルール:** Command絶対原則・tests-as-spec+gen_spec・i18n・EN+JA同期・.mdkproj変更はformat_version。機密: 汎用名。

## 設計決定

- **G19 タグの向きと伸長方向**: 現状タグ本体はアンカーから常に+x側へ伸び、`right`タグが自分の導線・注記に被る。修正: **タグ本体はアンカーから先端の逆側へ伸びる**(right=本体が左へ・先端がアンカー側の右向き、left=その鏡像)。つまりアンカー=先端の付け根とする(KiCadのグローバルラベルと同じ挙動)。既存図面のタグ位置が変わるため、変更内容と理由を仕様文+台帳へ明記(タグはフェーズ5導入直後で実運用図面なし=互換破壊を許容)
- **G20 注記文字高のシート設定**: `Sheet.annotation_text_mm: Option<f64>`(format_version 7、未指定=従来2.0mm)。対象=配線注記・線番・タグ文字(小系テキスト。注記エンティティ自体はheight持ちなので対象外)。設定Command `update_sheet`拡張 or 既存シート更新Commandへ(現状を確認して自然に)。UI=シートのプロパティ(またはプロジェクト設定)に「注記文字高」(1.0〜3.5mm)。参考図面相当の1.4mmで5mmピッチに収まることをテストで担保
- **G21 c接点リレーの役割認識**: `relay_role()`が`relay_contact_co`を接点として認識し、コイル+c接点のみのリレーが`erc.coil_without_contact`を誤報しない。接点マップの端子対もc接点(11/12/14)を正しく列挙(IEC 60947-5-1: c接点は11-12(NC)/11-14(NO)の2行 or 11/12/14の1行=一般慣行を確認して決定、仕様文へ)

### Task 1: G19〜G21の実装+可読性再確認 ✅ (2026-08-23)

- [x] Step 1 (red): G19: タグジオメトリ(right=本体が左・アンカー=先端付け根/left鏡像/外形寸法不変/XRef逃がし位置の追従)。G20: format_version 7互換/未指定=2.0mm/1.4mm設定で配線注記・線番・タグが縮む/範囲外拒否/SVG+キャンバス反映。G21: relay_roleがco接点を認識/coのみでcoil_without_contact消滅/接点マップの端子対表記/接点数検証(contact_configのc接点カウント)
2. (green) 実装(SVG+キャンバス共有数式・プロパティUI・i18n)→ cargo/vitest/vue-tsc/gen_spec green → コミット
3. **可読性再確認**: repro/スクリプトへ 注記文字高1.4mm+タグ向き修正を反映して再実行(git管理外)。前回の衝突(タグ×導線・注記×次行タグ)が解消したことをスクリーンショットで確認し、台帳G19〜G21を✅へ+計画[x]化→コミット

## 受け入れ基準

- rightタグが自分の導線・注記に被らない(左右対称の正しい旗)
- 注記文字高1.4mmで5mmピッチのコネクタ列が衝突なく読める
- コイル+c接点のリレーがERC誤報しない
- 再現図面の残衝突ゼロ(台帳更新)

## 結果 (2026-08-23)

3件とも実装し、再現図面で再確認した。

- **G19**: 基準点=先端の付け根。本体は先端の反対側へ伸び、XRefも本体側の外へ逃がす
  (右向きタグは左へ右揃え)。左右で外形寸法は同じ。互換破壊は仕様文と台帳へ明記
- **G20**: `Sheet.annotation_text_mm` (format_version 7・1.0〜3.5mm・未設定=2.0mm)。
  配線注記・線番・ネットタグの文字と逃がし量が同じ倍率で伸縮。Command
  `set_sheet_annotation_text` + プロパティパレット「シートの注記」(i18n)
- **G21**: `relay_contact_co`に`RelayRole::ContactCo`。端子は`11-12-14`
  (IEC 60947-5-1の共通=1・b側=2・a側=4)。`2CO`のパースと、c接点1極をa/b接点へ
  充当する接点数検証。`erc.coil_without_contact`の誤報が消えた
- **可読性再確認** (260コマンド・注記文字高1.4mm): シート1の文字の重なり **0件**
  (同じ図面を既定2.0mmで測ると12件)、ERC+電気検証 3件 → **2件**、電線リストの
  電線品番 111/111、17ページのブック。新たにG22 (コイル所在と配線注記の重なり) を
  台帳へ追加
- テスト: cargo (madake-core 627 / madake-mcp) ・vitest 754 ・vue-tsc すべてgreen
