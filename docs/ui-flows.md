# 画面遷移図・ユーザーフロー

作成: 2026-08-21。画面の詳細は[画面設計書](ui-screens.md)。

## 1. 基本作図フロー(作図→検証→出力)

```mermaid
flowchart TD
    A[起動: 無題プロジェクト] --> B{作図}
    B -->|リボン: 部品挿入| C[部品挿入ダイアログ]
    C -->|シンボル/極数指定/部品DB行を選択| D[配置ツール<br>クリックで配置 R=回転 Esc=解除]
    B -->|リボン: 配線| E[配線ツール<br>クリックで頂点 直交拘束<br>ダブルクリックで確定]
    B -->|選択ツール| F[移動/削除/プロパティ編集<br>参照記号・型番]
    D --> B
    E --> B
    F --> B
    B -->|リボン: 検証| G[検証結果パネル]
    G -->|行クリック| H[該当エンティティを選択+ズーム] --> F
    G -->|問題なし| I{出力}
    B -->|リボン: シミュレーション| S[シミュレーション結果パネル<br>ネット電圧・部品電流]
    S --> B
    I -->|PDF/SVG出力| J[保存ダイアログ→ファイル生成]
    I -->|部品表/電線リスト| K[CSV生成]
    B -->|タイトルバー: 保存| L[.mdkproj + .chat.json]
```

## 2. ツールの状態遷移(キャンバス)

```mermaid
stateDiagram-v2
    [*] --> select
    select --> wire : リボン「配線」/ L
    select --> place : 部品挿入ダイアログで選択
    wire --> wire : クリック(頂点追加)
    wire --> select : ダブルクリック/Esc(確定)
    place --> place : クリック(配置して継続)<br>R(90°回転)
    place --> select : Esc
    note right of select
      共通: 中ボタン/Space=パン、ホイール=ズーム
      ドラッグ=移動、空白ドラッグ=矩形選択
      ⌘Z/⇧⌘Z=undo/redo
    end note
```

## 3. 部品挿入ダイアログ内の分岐

```mermaid
flowchart LR
    O[リボン: 部品挿入] --> D[ダイアログ表示]
    D --> Q[検索入力<br>シンボル絞り込み+部品DB検索を兼ねる]
    D -->|静的シンボルのセル| P1[即・配置ツールへ]
    D -->|端子台/コネクタのセル| N[極数バー表示<br>1〜50 ステッパー] -->|配置| P2[動的シンボルで配置ツールへ]
    D -->|部品DB行| P3[型番+定格つきで配置ツールへ]
    D -->|X / 外側クリック / Esc| C[閉じる]
```

## 4. KiCad移行フロー

```mermaid
flowchart LR
    A[タイトルバー: 開く] --> B{ファイル種別}
    B -->|.mdkproj| C[プロジェクト読込<br>チャット履歴も復元]
    B -->|.kicad_sch| D[KiCadインポート<br>変換+ImportReport]
    D --> E[ステータスバーへ要約<br>シンボル/配線/スキップ数]
    E --> F[リボン: 検証] --> G[未接続をERC警告で洗い出し] --> H[手直し]
```

## 5. AI編集フロー(チャット)

```mermaid
flowchart TD
    A[チャット入力<br>浮きカード or 左ドック] --> B[claude CLI ヘッドレス実行<br>stream-json]
    B --> C[自アプリのMCPへ自己接続<br>place_symbol / draw_wire / execute_commands ...]
    C --> D[Commandエンジン] --> E[Patch broadcast]
    E --> F[キャンバス即時反映<br>+編集領域のシアンパルス]
    B --> G[会話にツールチップ表示<br>✓ place_symbol …]
    G --> H{ターン完了}
    H -->|適用済み rev N| I[「元に戻す」= undo N回]
```

## 6. エラー・例外フロー

```mermaid
flowchart LR
    A[検証/シミュレーション実行] --> B{ngspiceある?}
    B -->|はい| C[DC動作点の実解で判定/表示]
    B -->|いいえ・検証| D[グラフ近似で判定<br>+approximate_mode Info]
    B -->|いいえ・シミュレーション| E[エラーパネル表示<br>導入手順を案内]
    F[アプリ未起動でmadake CLI] --> G[「アプリが起動していません」<br>exit 1]
    H[KiCad未対応シンボル] --> I[スキップ+ImportReportに列挙]
```

## 7. 外部クライアントの入口(遷移図の外側)

| 入口 | 経路 | 備考 |
|---|---|---|
| Claude Code / Desktop | MCP `127.0.0.1:9310/mcp` | `.mcp.json`で自動接続 |
| madake CLI | Link API `/api/v1` | 全編集はCommand経由なのでUIと同じundo履歴に乗る |
| ブラウザ(UI検証) | `http://localhost:1420` → Link API | Tauri外ではipcが自動フォールバック |
| FreeCADアドオン(将来) | Link API `/api/v1` | spec §7 |
