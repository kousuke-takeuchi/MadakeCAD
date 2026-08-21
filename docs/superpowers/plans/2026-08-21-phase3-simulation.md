# MadakeCAD フェーズ3 (回路シミュレーション) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ngspice基盤(フェーズ2aで前倒し実装済みのSPICE変換+ランナー)を**ユーザー向けのシミュレーション機能**に仕立てる。DC動作点(各ネットの電圧・各部品の電流/電力)を実行・表示でき、スイッチ/接点の開閉を指定したwhat-if解析ができる。部品DBにSPICEモデル列を追加する。

**スコープの割り切り(specから調整):** 過渡解析(.tran)は**本プランでは見送る**。現状の素子モデル(抵抗のみ)では時間応答が平坦で意味がなく、部品DBのSPICEモデル(L/C/半導体)が実際に使われるようになってから価値が出るため。specにこの判断を反映する。

## 設計決定

- **SimOpResult** (madake-core/sim.rs):
  ```rust
  pub struct SimOpResult {
    pub voltage: f64,                     // 電源電圧
    pub nets: Vec<NetVoltage { name, volts_min, volts_max, wire_ids }>,  // ネットは配線抵抗でノードが分かれるためmin/max
    pub components: Vec<ComponentCurrent { reference, entity_id, amps, watts }>,
    pub warnings: Vec<String>,
  }
  pub fn simulate_op(sheet, symbols, open_switches: &[String]) -> Result<SimOpResult, SimError>
  ```
- **スイッチ開閉のwhat-if**: `open_switches`に参照記号(例: "SW1", "K1")を渡すと、その導通部品の橋をデッキから除外(=開路)。`spice::build_deck`に`DeckOptions { open_switches }`を追加
- **ネット電圧**: ネットリストのネット⇔SPICEノードの対応は「ネットに属するピン/ワイヤのノード集合」で取り、min/maxを報告(配線抵抗で同一ネット内でも電圧差が出るのは仕様=電圧降下そのもの)
- **部品電流**: 負荷・導通部品の橋素子の電流(|Δv|/R)。電力 = Δv×I
- **ngspice未導入**: `SimError::NgspiceNotFound`で明示(検証と違いシミュレーションは近似フォールバックしない。導入手順をメッセージに含める)
- **部品DB**: スキーマv2で`spice_model`列(SPICE素子行テンプレート、将来の過渡解析・非線形モデル用)を追加。v1→v2マイグレーションをテスト
- **露出**: MCPツール`simulate_op`、Link API `POST /api/v1/simulate/op {open_switches}`、CLI `madake sim [--open SW1,K1]`
- **UI**(Pencilデザイン先行): リボン「検証/レポート」グループに「シミュレーション」R大ボタン、結果パネル(検証結果パネルと同型: ネット電圧表+部品電流表、警告)

### Task 1: シミュレーションコア (madake-core/sim.rs + spice.rs拡張)

- [ ] Step 1 (red): テスト: 直列回路のsimulate_op(ネット電圧・F1/L1の電流と電力)、open_switches=["SW1"]で負荷電流≈0、ngspice未導入エラー(env偽装)
- [ ] Step 2 (green): DeckOptions+simulate_op実装、コミット

### Task 2: API/CLI露出

- [ ] Step 1 (red): link_api統合テスト: POST /simulate/op が結果JSONを返す(ngspice検出時のみ実行)
- [ ] Step 2 (green): MCP/Link API/CLI実装、README/CLAUDE.md更新、コミット

### Task 3: 部品DB spice_model列 (スキーマv2)

- [ ] Step 1 (red): テスト: v1のDBを開くとv2へマイグレーションされ既存データ保持、spice_modelのCRUD
- [ ] Step 2 (green): 実装、コミット

### Task 4: UI: シミュレーション結果パネル(デザイン先行)

- [ ] Step 1: Pencilデザイン: リボン「シミュレーション」ボタン+結果パネル(ネット電圧・部品電流の表、スイッチ開閉チップ)
- [ ] Step 2 (red): vitest: simストア(実行・結果・開閉トグル)
- [ ] Step 3 (green): 実装+実機ブラウザ検証、コミット

## 進捗

- 2026-08-21: プラン作成
