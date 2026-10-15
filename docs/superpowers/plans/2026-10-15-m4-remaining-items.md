# M4 残り6項目 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M4から先送りしていた6項目を、2026-10-15に確定したデザイン (`MadakeCAD.pen` のM4残りデザイン①〜⑥、
`docs/internal/design-system.md`「M4残り6項目」) どおりに実装する。ベンチマークの出典は各ボードの説明文と
同じ (EPLAN / ACADE ヘルプ)。

**Architecture:** 図面の編集はすべて `Command` / `Engine::execute()`。マクロ・図枠テンプレートはファイルの
ドメイン (undo対象外)。新しいエンティティ (ケーブル・銘板・寸法線) を足すタスクは `.mdkproj` の
`format_version` を上げ、旧形式のテストを付ける。

## Task 1: 回路マクロ — 保存ダイアログからのバリアント追加 (デザイン①)

**Files:** `src-tauri/crates/madake-core/src/macros.rs` / `src-tauri/crates/madake-mcp/src/lib.rs` /
`src/stores/macros.ts` / マクロ保存ダイアログ / 6カタログ / docs 03 (EN+JA)

- [x] **Step 1 (red):** `MacroMeta.variant` (A〜Z) と `meta.id` が既存マクロを指すとき、そのマクロへ
      バリアントを足す / 登録済みのキーなら置き換える (他のバリアント・値セットは残る) / `A`は既定の
      commandsを置き換える / 不正なキー (空・2文字・数字) は拒否 / プレースホルダの行き先は参照記号が
      同じシンボルへ新しいバリアントでも結び付き、そのバリアントを値セット付きで挿入できる /
      他のバリアントだけにある行き先は飛ばす (どのバリアントにも無い行き先は従来どおりエラー)
- [x] **Step 2 (green):** 実装。UIは名前欄の既存マクロ候補・バリアントチップ (登録済/選択/空き)・
      上書き確認
- [x] **Step 3:** 実機確認 → docs → `gen_spec.py` → コミット

## Task 2: 予約接点の配置 (デザイン②)

- [x] 部品DBの接点構成から未配置接点を導出 (`relay_xref`) → ナビゲータの未配置ノード → 「配置」/ドラッグで
      接点シンボルを置く Command 1回。接点構成の外は出さない

## Task 3: PLC 横バス様式・同居/分割配置・設定のプロジェクト保存 (デザイン③)

- [x] 生成設定を `.mdkproj` に保存 (format_version を上げる) / 横バス+縦ラングのページ生成 / 配置方針②③

## Task 4: 多芯ケーブル (デザイン④)

- [x] `Cable` エンティティ (定義線・芯線割当・シールド・長さ) と format_version / 定義線ツール /
      プロパティ / ERC「ケーブル芯数超過」/ ケーブル一覧帳票 / BOMの長さ合計行

## Task 5: 盤 銘板・寸法線・整列/等間隔・盤内配線図 (デザイン⑤)

- [x] 銘板 (フットプリントの子・説明に追従)・関連付き寸法線・整列/等間隔 Command・盤内配線図シート

## Task 6: 図枠エディタ (デザイン⑥)

- [x] 既存の図枠テンプレートJSONを画面で編集するエディタ (エディタ内undo・プレースホルダ選択・保存前チェック)
