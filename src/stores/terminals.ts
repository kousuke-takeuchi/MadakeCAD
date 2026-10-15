// 端子台エディタの状態 (仕様: docs/internal/specs/m4-industrial-core.md §1.1)。
//
// グリッドの中身はRust側 (madake-coreの端子台チャート) が図面から導出したものをそのまま
// 表示するだけで、ここで結線を組み立て直したりはしない。編集できるのはジャンパ・
// 並べ替え順 (row_order)・端子番号 (terminal_numbers)・段数 (symbol_idの_{levels}l) で、
// どれも Command 1回として送る (undo一発で戻る)。
// v1のスコープ: 行表示 (多段端子台はLv列付き)+ジャンパ生成/削除+並べ替え+図面へジャンプ+
// 予備端子表示+端子台チェック+チャート/接続図の生成+端子番号の自動採番+多段端子の生成/解除。
// アクセサリ・部品割当・内外スワップ・予備端子の挿入はM4の以降のフェーズ。

import { defineStore } from "pinia";
import { DYNAMIC_PIN_MAX, dynamicSymbol, parseTerminalNo, TERMINAL_LEVEL_MAX } from "../canvas/dynamicSymbol";
import { levelLetter, splitLabelNumber } from "../canvas/terminalNo";
import { chartOrderPinNumbers, parsePinGroups, PIN_GROUPS_ATTR } from "../canvas/pinGroups";
// ジャンパ指定の正規化・並べ替え順の解釈はキャンバス描画と共通の純関数を使う
// (Pinia非依存モジュールが正。Rust側 madake-core/src/terminal_chart.rs の双子)
import {
  formatJumperSpec,
  insertSpareTerminals as insertSpareTerminalsPlan,
  multilevelSymbolId,
  parseJumperSpec,
  polesAndLevelsOf,
  relabelForLevels,
  rewriteJumperLabels,
  terminalLabels,
  terminalNumbers,
  terminalNumbersOfInstance,
  terminalRenumberPlan,
  terminalsAreAdjacent,
  validTerminalNumbers,
} from "../canvas/terminalJumpers";
import type { RevealTarget } from "../composables/reveal";
import {
  ipc,
  type Accessory,
  type AccessoryKind,
  type Command,
  type Diagnostic,
  type RowOrder,
  type SymbolDef,
  type SymbolInstance,
  type TerminalBlockInfo,
  type Part,
  type TerminalChart,
} from "../ipc";
import { useDocumentStore } from "./document";


/** 付属品の種別 (並びはポップオーバーのドロップダウン順)。 */
export const ACCESSORY_KINDS: AccessoryKind[] = ["end_bracket", "partition", "end_plate"];
/** 付属品の位置で「端子台の末尾」を表す語 (Rust側`ACCESSORY_END`と同じ)。 */
export const ACCESSORY_END = "end";

/** 付属品ポップオーバーの1行 (編集中の値)。位置は端子の表示ラベルか`"end"`。 */
export interface AccessoryDraft {
  position: string;
  kind: AccessoryKind;
  partNo: string;
}

/** チャートの付属品をポップオーバーの行へ写す。 */
export function accessoryDrafts(items: Accessory[]): AccessoryDraft[] {
  return items.map((a) => ({ position: a.before ?? ACCESSORY_END, kind: a.kind, partNo: a.part_no }));
}

/**
 * ポップオーバーの行を`attrs["accessories"]`の書式 (`端子番号:種別:型番` のカンマ列) へ戻す。
 * 型番は前後の空白を落とし、`,`と`:`は書式を壊すので取り除く。行が無ければ空文字
 * (呼び出し側で属性ごと消す)。
 */
export function formatAccessories(rows: AccessoryDraft[]): string {
  return rows
    .map((r) => `${r.position}:${r.kind}:${r.partNo.replace(/[,:]/g, "").trim()}`)
    .join(",");
}

export { TERMINAL_LEVEL_MAX } from "../canvas/dynamicSymbol";

export { formatJumperSpec, parseJumperSpec, ROW_ORDERS, rowOrderOf } from "../canvas/terminalJumpers";

/**
 * 採番案を適用できない理由 (デザイン「採番 (エラー)」の指摘行)。
 * `code`はi18nキーの末尾 (`terminals.renumber.issue<Code>`) で、`label`/`count`はその文の穴埋め。
 */
export type RenumberIssue =
  | { code: "duplicate"; label: string; count: number }
  | { code: "empty" }
  | { code: "comma"; label: string }
  | { code: "invalid" }
  | { code: "noBlock" };

/**
 * ポップオーバーの数値入力を、コマンドが受け取れる形 (0以上の整数) に丸める。
 * `start`/`step`はRust側で`u32`なので、負数や小数のまま送るとJSONの型エラーになり、
 * ポップオーバーではなくログに出てしまう (`min`属性は助言でしかなく、貼り付けを止められない)。
 */
function clampRenumberInput(start: number, step: number): [number, number] {
  const whole = (n: number) => (Number.isFinite(n) ? Math.max(0, Math.trunc(n)) : 0);
  return [whole(start), whole(step)];
}

/**
 * 段数ポップオーバーの「プレビュー」囲み (デザイン`C8rBw`の3行)。
 * すべて**適用と同じ純関数** ([`multilevelSymbolId`]/[`relabelForLevels`]/[`rewriteJumperLabels`]
 * = Rust側コマンドの双子) で計算するので、プレビューと結果は必ず一致する。
 */
export interface LevelsPreview {
  /** `terminal_block_8p` → `terminal_block_8p_2l`。 */
  symbolFrom: string;
  symbolTo: string;
  /** 端子番号 (先頭2極ぶんだけ。続きがあれば末尾が`,…`になる)。 */
  numbersFrom: string;
  numbersTo: string;
  /** ジャンパ指定の追従 (ジャンパが無ければnull)。 */
  jumpers: { from: string; to: string } | null;
}

/**
 * 段を減らせない理由の1件 (デザイン`j83rxy`の「解除で消える段」の囲みが並べる行)。
 * `reason`はi18nキーの末尾 (`terminals.levels.blocker<Reason>`)。
 */
export interface LevelsBlocker {
  /** 消える段の端子番号 (例 "6b")。 */
  terminal: string;
  /** その端子が繋がっている相手の表記 (例 "K2:A2")。無ければ空。 */
  connection: string;
  reason: "wire" | "jumper" | "pinGroup" | "accessory";
}

/**
 * 予備挿入ポップオーバーの「プレビュー」囲み (デザイン`eTFE5`の2行)。
 * `sparePreview`が**適用と同じ純関数** ([`insertSpareTerminalsPlan`] = Rust側
 * `insert_spare_terminals`の双子) で計算するので、プレビューと結果は必ず一致する。
 */
export interface SparePreview {
  /** `terminal_block_8p` → `terminal_block_9p`。 */
  symbolFrom: string;
  symbolTo: string;
  /** 新しく増える予備端子のラベル (多段なら1極につき段数ぶん、例 `["9a","9b"]`)。 */
  spareLabels: string[];
}

/**
 * 予備挿入案が適用できない理由 (デザイン「予備端子の挿入 (エラー)」の指摘行)。
 * `code`はi18nキーの末尾 (`terminals.spareInsert.issue<Code>`)。
 */
export type SpareIssue =
  | { code: "count" }
  | { code: "poleCap"; max: number }
  /** 挿入位置の端子番号に数字が無いので新しい番号を決められない (例 `"予備"`)。 */
  | { code: "nonNumeric"; label: string }
  /** 「空き番号のまま残す」指定だが、予備に渡る番号が既に使われている。 */
  | { code: "keepDuplicate" }
  | { code: "invalid" };

/** グリッドの1行 (デザイン「端子台エディタ」の列順)。 */
export interface TerminalEditorRow {
  /** 行番号 (1始まり)。 */
  no: number;
  /** 端子番号。 */
  terminal: string;
  /** 段記号 (例 "a")。多段端子台だけが持ち、単段の端子台では空文字。 */
  level: string;
  /** 外部側 (盤外) の接続先。 */
  external: string;
  /** 外部側の電線が属するハーネス・ケーブル名。 */
  externalCable: string;
  /** この端子に掛かっているジャンパ (例 "1-2")。 */
  jumper: string;
  /** 内部側 (盤内) の接続先。 */
  internal: string;
  wireNo: string;
  /** 電線の仕様 (線色・sq・品番)。 */
  wire: string;
  /** 予備端子 (両側とも未結線)。 */
  spare: boolean;
  selected: boolean;
}

/**
 * 選んだ端子どうしにジャンパを掛けた指定を返す。既存のジャンパは残る。
 * 端子が2つ未満、または隣り合っていない端子が混ざっていればnull (掛けられない)。
 */
export function withJumpers(spec: string, terminals: readonly string[]): string | null {
  if (terminals.length < 2) return null;
  const pairs = parseJumperSpec(spec);
  for (let i = 1; i < terminals.length; i++) {
    const [a, b] = [terminals[i - 1], terminals[i]];
    if (!terminalsAreAdjacent(a, b)) return null;
    if (!pairs.some(([x, y]) => (x === a && y === b) || (x === b && y === a))) pairs.push([a, b]);
  }
  return formatJumperSpec(parseJumperSpec(formatJumperSpec(pairs)));
}

/** 選んだ端子に掛かっているジャンパだけを外した指定を返す。 */
export function withoutJumpers(spec: string, terminals: readonly string[]): string {
  const keep = parseJumperSpec(spec).filter(
    ([a, b]) => !terminals.includes(a) && !terminals.includes(b),
  );
  return formatJumperSpec(keep);
}

interface TerminalsState {
  open: boolean;
  /** エディタを開いたシート (端子台一覧の範囲)。 */
  sheetId: string | null;
  blocks: TerminalBlockInfo[];
  /** 表示中の端子台のentity id。 */
  entityId: string | null;
  chart: TerminalChart | null;
  /** 選択中の端子番号 (グリッドの行選択)。 */
  selected: string[];
  /** 直前の端子台チェックの結果。未実行ならnull。 */
  diagnostics: Diagnostic[] | null;
  loading: boolean;
  busy: boolean;
  error: string | null;
  /**
   * 「図面へジャンプ」でエディタを閉じたときの覚え書き (次に開いたときここへ戻す)。
   * 自分で閉じた (×・スクリム) ときはnullに戻すので、復元はジャンプ直後の1回だけ。
   */
  resume: { sheetId: string | null; entityId: string; selected: string[] } | null;
}

/** 重大度の並び順 (エラー→警告→情報)。 */
const SEVERITY_ORDER = { error: 0, warning: 1, info: 2 } as const;

export const useTerminalsStore = defineStore("terminals", {
  state: (): TerminalsState => ({
    open: false,
    sheetId: null,
    blocks: [],
    entityId: null,
    chart: null,
    selected: [],
    diagnostics: null,
    loading: false,
    busy: false,
    error: null,
    resume: null,
  }),

  getters: {
    /** 表示中の端子台の概要。 */
    block(state): TerminalBlockInfo | null {
      return state.blocks.find((b) => b.entity_id === state.entityId) ?? null;
    },
    /** 表示中の端子台の参照記号 (例 "TB1")。 */
    /** 付属品 (チャートが解析したもの。書いた順)。 */
    accessories(state): Accessory[] {
      return state.chart?.accessories ?? [];
    },
    /** 読めなかった付属品の項。 */
    accessoryIssues(state) {
      return state.chart?.accessory_issues ?? [];
    },
    /** 端子台の現在の型番 (`SymbolInstance.value`)。 */
    partNo(state): string {
      return state.chart?.value ?? "";
    },
    reference(state): string {
      return state.chart?.reference ?? this.block?.reference ?? "";
    },
    /** 端子数 (多段端子台では極数×段数)。 */
    terminalCount(state): number {
      return state.chart?.terminal_count ?? this.block?.terminal_count ?? 0;
    },
    /**
     * 極数 (多段端子台でも段数は掛けない)。端子台の見出しはこちらで呼ぶ。
     * 一覧より先にチャートだけが読めている場合に備えて、`terminalCount`と同じく
     * チャート (行の端子番号から数えた別々の極の数) へフォールバックする。
     */
    poleCount(state): number {
      if (this.block) return this.block.poles;
      const poles = new Set(
        (state.chart?.rows ?? []).map((r) => parseTerminalNo(r.terminal)[0]),
      );
      return poles.size;
    },
    /** グリッドの行 (並びはチャートのまま=現在の`rowOrder`どおり)。 */
    rows(state): TerminalEditorRow[] {
      return (state.chart?.rows ?? []).map((r, i) => ({
        no: i + 1,
        terminal: r.terminal,
        level: r.level,
        external: r.external,
        externalCable: r.external_harness,
        jumper: r.jumper,
        internal: r.internal,
        wireNo: r.wire_no,
        wire: r.wire,
        spare: r.spare,
        selected: state.selected.includes(r.terminal),
      }));
    },
    /** 予備端子 (未結線) の数。 */
    spareCount(): number {
      return this.rows.filter((r) => r.spare).length;
    },
    /**
     * 多段端子台か (Lv列を出すかどうか)。一覧より先にチャートだけが読めているときは
     * 行の段記号から判定する (`poleCount`と同じフォールバック)。
     */
    hasLevels(state): boolean {
      if (this.block) return this.block.levels > 1;
      return (state.chart?.rows ?? []).some((r) => r.level !== "");
    },
    /**
     * 現在の段数 (単段は1)。一覧より先にチャートだけが読めているときは、行の段記号の
     * 種類数から数える (`hasLevels`と同じフォールバック)。
     */
    levels(state): number {
      if (this.block) return this.block.levels;
      const letters = new Set((state.chart?.rows ?? []).map((r) => r.level).filter((l) => l !== ""));
      return letters.size || 1;
    },
    /** 現在の並べ替え順 (チャートが無ければ既定の数値順)。 */
    rowOrder(state): RowOrder {
      return state.chart?.row_order ?? "number";
    },
    /** 選択中の端子番号 (極番号→段記号の順)。多段端子台では "1a" のような段記号付き。 */
    selectedTerminals(state): string[] {
      return [...state.selected].sort((a, b) => {
        const [poleA, levelA] = parseTerminalNo(a);
        const [poleB, levelB] = parseTerminalNo(b);
        return poleA - poleB || (levelA ?? "").localeCompare(levelB ?? "") || a.localeCompare(b);
      });
    },
    /** 現在のジャンパ指定 (図面の値)。 */
    jumperSpec(state): string {
      return formatJumperSpec(state.chart?.jumpers ?? []);
    },
    /**
     * 表示中の端子台の生の端子番号 (ピン順、シンボル定義の既定)。採番ポップオーバーの
     * プレビュー計算に使う (取れなければ空)。
     */
    rawTerminalNumbers(): string[] {
      const def = this.blockDef;
      return def ? terminalNumbers(def) : [];
    },
    /**
     * 表示中の端子台のシンボルインスタンス (図面の実体)。ポップオーバーのプレビューが
     * `attrs` (ジャンパ・ピングループ・付属品・端子番号の上書き) と`symbol_id`を見るため。
     */
    blockInstance(): SymbolInstance | null {
      const doc = useDocumentStore();
      const block = this.block;
      if (!block) return null;
      const sheet = doc.project?.sheets.find((s) => s.id === block.sheet_id);
      const entity = sheet?.entities[block.entity_id];
      return entity && entity.kind === "symbol" ? entity : null;
    },
    /** 表示中の端子台のシンボル定義 (取れなければnull)。 */
    blockDef(): SymbolDef | null {
      const inst = this.blockInstance;
      return inst ? dynamicSymbol(inst.symbol_id) : null;
    },
    /**
     * 表示中の端子台の**現在の**端子番号 (`rawTerminalNumbers`と同じ並びの表示ラベル)。
     * 上書きが無ければ生の番号と同じ。採番プレビューがジャンパの追従を見せるのに使う
     * (今のジャンパ指定はこのラベルで書かれている)。
     */
    displayTerminalNumbers(): string[] {
      const inst = this.blockInstance;
      const def = this.blockDef;
      return def && inst ? terminalNumbersOfInstance(def, inst.attrs) : [];
    },
    /**
     * 「予備挿入」ポップオーバーの挿入位置ドロップダウンの選択肢 (極ごとに1つ。多段端子台では
     * 段aのラベルを代表にする — 挿入は極単位で、段は自動で付いてくるため)。
     *
     * **並びは隣のグリッドと同じ行順** (チャートの行をそのまま辿る)。生のピン順ではない:
     * 予備を挿入した端子台では生の順と番号順がわざと食い違うので、生の順で並べると
     * ドロップダウンだけが予備を末尾に見せてグリッドと食い違ってしまう。
     */
    spareAfterOptions(): string[] {
      return this.rows.filter((r) => r.level === "" || r.level === levelLetter(0)).map((r) => r.terminal);
    },
    /** 選択中の端子にジャンパを掛けられるか (隣り合う2端子以上)。 */
    canAddJumper(): boolean {
      return withJumpers(this.jumperSpec, this.selectedTerminals) !== null;
    },
    /** 選択中の端子に外せるジャンパがあるか。 */
    canRemoveJumper(): boolean {
      if (this.selectedTerminals.length === 0) return false;
      return withoutJumpers(this.jumperSpec, this.selectedTerminals) !== this.jumperSpec;
    },
    /** チェック結果の件数 (重大度ごと)。未実行なら全て0。 */
    checkCounts(state): { error: number; warning: number; info: number } {
      const counts = { error: 0, warning: 0, info: 0 };
      for (const d of state.diagnostics ?? []) counts[d.severity] += 1;
      return counts;
    },
    /** チェックで問題が1件も出なかったか。 */
    checkOk(state): boolean {
      return (state.diagnostics ?? []).length === 0;
    },
  },

  actions: {
    /**
     * シートの端子台一覧を読み込み、先頭の端子台を表示して開く。
     * 直前に「図面へジャンプ」で閉じていたときは、その端子台と行選択へ戻す (覚え書きは使い切る)。
     */
    async openFor(sheetId: string | null) {
      const resume = this.resume;
      this.resume = null;
      this.sheetId = resume ? resume.sheetId : sheetId;
      this.entityId = resume?.entityId ?? null;
      this.selected = resume ? [...resume.selected] : [];
      this.diagnostics = null;
      this.error = null;
      this.open = true;
      await this.reloadBlocks();
    },

    /** 端子台一覧を読み直し、表示中の端子台 (無ければ先頭) のチャートを取り直す。 */
    async reloadBlocks() {
      this.loading = true;
      try {
        this.blocks = await ipc.listTerminalBlocks(this.sheetId);
        const keep = this.blocks.some((b) => b.entity_id === this.entityId);
        const next = keep ? this.entityId : (this.blocks[0]?.entity_id ?? null);
        this.entityId = next;
        await this.reloadChart();
      } catch (e) {
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },

    /** 表示中の端子台のチャートを読み直す。 */
    async reloadChart() {
      this.chart = this.entityId ? await ipc.getTerminalChart(this.entityId) : null;
    },

    /** 別の端子台へ切り替える (選択と直前のチェック結果は捨てる)。 */
    async selectBlock(entityId: string) {
      this.entityId = entityId;
      this.selected = [];
      this.diagnostics = null;
      this.error = null;
      // 一覧 (極数・段数など) も併せて読み直す。チャートだけを取り直すと、切り替えている間に
      // 他の編集で一覧側の情報 (Lv列の要不要など) が古いままになりうる (フェーズ6の積み残し)
      await this.reloadBlocks();
    },

    /** 行の選択を切り替える。 */
    toggleRow(terminal: string) {
      const i = this.selected.indexOf(terminal);
      if (i >= 0) this.selected.splice(i, 1);
      else this.selected.push(terminal);
    },

    /**
     * 端子台シンボルの`value`と属性を書き換える update_entity コマンド。`attrs`の値が`null`の
     * キーは属性ごと消す。端子台が見つからなければnull。
     */
    symbolUpdateCommand(patch: { value?: string; attrs?: Record<string, string | null> }): Command | null {
      const doc = useDocumentStore();
      const block = this.block;
      if (!block) return null;
      const sheet = doc.project?.sheets.find((s) => s.id === block.sheet_id);
      const entity = sheet?.entities[block.entity_id];
      if (!entity || entity.kind !== "symbol") return null;
      const attrs = { ...entity.attrs };
      for (const [k, v] of Object.entries(patch.attrs ?? {})) {
        if (v === null || v === "") delete attrs[k];
        else attrs[k] = v;
      }
      return {
        type: "update_entity",
        sheet_id: block.sheet_id,
        entity: { ...entity, value: patch.value ?? entity.value, attrs },
      };
    },

    /** update_entity を1回実行してチャートを読み直す (失敗は`error`へ)。 */
    async applyUpdate(command: Command | null): Promise<boolean> {
      if (!command || this.busy) return false;
      this.busy = true;
      this.error = null;
      try {
        await useDocumentStore().execute(command);
        await this.reloadBlocks();
        return true;
      } catch (e) {
        this.error = String(e);
        return false;
      } finally {
        this.busy = false;
      }
    },

    /**
     * 部品DBの端子台型番を割り当てる (update_entity 1回で`value`=型番と
     * `attrs["terminal_kind"]`=種別を書く。種別が空の部品なら属性を消す)。
     */
    async assignPart(part: Pick<Part, "part_no" | "terminal_kind">): Promise<boolean> {
      return this.applyUpdate(
        this.symbolUpdateCommand({
          value: part.part_no,
          attrs: { terminal_kind: part.terminal_kind ?? null },
        }),
      );
    },

    /** 付属品の指定を書き換える (update_entity 1回。行が無ければ属性ごと消す)。 */
    async applyAccessories(rows: AccessoryDraft[]): Promise<boolean> {
      return this.applyUpdate(this.symbolUpdateCommand({ attrs: { accessories: formatAccessories(rows) } }));
    },

    /** ジャンパ指定を書き換える update_entity コマンド。端子台が見つからなければnull。 */
    jumperCommand(spec: string): Command | null {
      const doc = useDocumentStore();
      const block = this.block;
      if (!block) return null;
      const sheet = doc.project?.sheets.find((s) => s.id === block.sheet_id);
      const entity = sheet?.entities[block.entity_id];
      if (!entity || entity.kind !== "symbol") return null;
      const attrs = { ...entity.attrs };
      // ジャンパが1本も残らないときは属性ごと消す (未設定の端子台と同じ状態に戻す)
      if (spec) attrs.jumpers = spec;
      else delete attrs.jumpers;
      return {
        type: "update_entity",
        sheet_id: block.sheet_id,
        entity: { ...entity, attrs },
      };
    },

    /** 選択中の隣り合う端子にジャンパを掛ける (Command 1回)。 */
    async addJumper(): Promise<boolean> {
      const spec = withJumpers(this.jumperSpec, this.selectedTerminals);
      if (spec === null) return false;
      return this.applyJumperSpec(spec);
    },

    /** 選択中の端子に掛かっているジャンパを外す (Command 1回)。 */
    async removeJumper(): Promise<boolean> {
      if (!this.canRemoveJumper) return false;
      return this.applyJumperSpec(withoutJumpers(this.jumperSpec, this.selectedTerminals));
    },

    /** ジャンパ指定を実際に適用し、チャートを読み直す。 */
    async applyJumperSpec(spec: string): Promise<boolean> {
      const command = this.jumperCommand(spec);
      if (!command || this.busy) return false;
      this.busy = true;
      this.error = null;
      try {
        await useDocumentStore().execute(command);
        this.selected = [];
        await this.reloadBlocks();
        return true;
      } catch (e) {
        this.error = String(e);
        return false;
      } finally {
        this.busy = false;
      }
    },

    /**
     * 並べ替え順を変える (update_entity コマンド1回。既定の数値順を選んだときは
     * `attrs["row_order"]`自体を消す=未設定の端子台と同じ状態に戻す。ジャンパの
     * delete-when-emptyと同じ流儀)。
     */
    async setRowOrder(order: RowOrder): Promise<boolean> {
      const doc = useDocumentStore();
      const block = this.block;
      if (!block || this.busy) return false;
      const sheet = doc.project?.sheets.find((s) => s.id === block.sheet_id);
      const entity = sheet?.entities[block.entity_id];
      if (!entity || entity.kind !== "symbol") return false;
      const attrs = { ...entity.attrs };
      if (order === "number") delete attrs.row_order;
      else attrs.row_order = order;
      this.busy = true;
      this.error = null;
      try {
        await doc.execute({
          type: "update_entity",
          sheet_id: block.sheet_id,
          entity: { ...entity, attrs },
        });
        await this.reloadBlocks();
        return true;
      } catch (e) {
        this.error = String(e);
        return false;
      } finally {
        this.busy = false;
      }
    },

    /**
     * 採番ポップオーバーのプレビュー (開始番号・増分・接頭辞から、実際に適用したときの
     * 端子番号一覧を計算する。図面はまだ変えない。`RenumberTerminals`コマンドと同じ
     * アルゴリズム ([`terminalRenumberPlan`]) を使うので、プレビューと適用結果は必ず一致する)。
     */
    renumberPreview(start: number, step: number, prefix: string): string[] {
      const [s, t] = clampRenumberInput(start, step);
      return terminalRenumberPlan(this.rawTerminalNumbers, s, t, prefix);
    },

    /**
     * プレビューの対応行 (`1 → 11`)。デザイン「採番 (既定)」の囲みが並べる表。
     * 多段端子台では段記号を保ったままの対応 (`1a → 11a`) がそのまま出る。
     */
    renumberMapping(start: number, step: number, prefix: string): { from: string; to: string }[] {
      const plan = this.renumberPreview(start, step, prefix);
      // 左側は**今グリッドに出ている番号** (既に採番済みなら上書きラベル)。採番の書き換えは
      // 旧表示ラベル→新表示ラベルで効くので、これがユーザーの見ている対応そのものになる
      return this.displayTerminalNumbers.map((from, i) => ({ from, to: plan[i] ?? "" }));
    },

    /**
     * 採番に追従したあとのジャンパ指定 (`"1-2,3-4" → "11-12,13-14"`)。ジャンパが無ければnull。
     * 適用と同じ規則 ([`rewriteJumperLabels`]) で計算するので、プレビューと結果は必ず一致する。
     */
    renumberJumperPreview(
      start: number,
      step: number,
      prefix: string,
    ): { from: string; to: string } | null {
      const spec = this.block?.jumpers ?? "";
      if (!spec) return null;
      const display = this.displayTerminalNumbers;
      const plan = this.renumberPreview(start, step, prefix);
      const rename = new Map(display.map((d, i) => [d, plan[i] ?? d] as const));
      return { from: spec, to: rewriteJumperLabels(spec, rename) };
    },

    /**
     * 採番案が適用できない理由 (適用できるならnull)。デザイン「採番 (エラー)」の指摘行が出す。
     *
     * 判定はRust側の`valid_terminal_numbers`と同じ規則 (個数・重複・空要素・カンマ) なので、
     * **ここがnullでない案は必ずコマンドが拒否する**し、nullなら通る。増分0の採番が全端子を
     * 同じ番号にしてしまうのがいちばん多い当たり方なので、重複は具体的なラベルと件数で返す。
     */
    renumberIssue(start: number, step: number, prefix: string): RenumberIssue | null {
      const raw = this.rawTerminalNumbers;
      if (raw.length === 0) return { code: "noBlock" };
      const plan = this.renumberPreview(start, step, prefix).map((s) => s.trim());
      const blank = plan.find((s) => s === "");
      if (blank !== undefined) return { code: "empty" };
      const comma = plan.find((s) => s.includes(","));
      if (comma !== undefined) return { code: "comma", label: comma };
      const counts = new Map<string, number>();
      for (const s of plan) counts.set(s, (counts.get(s) ?? 0) + 1);
      for (const [label, count] of counts) {
        if (count > 1) return { code: "duplicate", label, count };
      }
      // 上の3つでRust側の検証を漏れなく写しているかを、同じ判定関数で最後に確かめる
      return validTerminalNumbers(plan, raw.length) ? null : { code: "invalid" };
    },

    /** 採番 (RenumberTerminals) コマンド。端子台が見つからなければnull。 */
    renumberCommand(start: number, step: number, prefix: string): Command | null {
      const block = this.block;
      if (!block) return null;
      const [s, t] = clampRenumberInput(start, step);
      return {
        type: "renumber_terminals",
        sheet_id: block.sheet_id,
        entity_id: block.entity_id,
        start: s,
        step: t,
        prefix,
      };
    },

    /**
     * 開始番号・増分・接頭辞で端子番号を振り直す (Command 1回)。
     * 適用できない案 ([`renumberIssue`]) は**コマンドを送らずに**断る
     * (ポップオーバーが指摘行を出したまま適用ボタンを無効にしているので、ここは念のための門)。
     */
    async renumberTerminals(start: number, step: number, prefix: string): Promise<boolean> {
      if (this.renumberIssue(start, step, prefix) !== null) return false;
      const command = this.renumberCommand(start, step, prefix);
      if (!command || this.busy) return false;
      this.busy = true;
      this.error = null;
      try {
        await useDocumentStore().execute(command);
        this.selected = [];
        await this.reloadBlocks();
        return true;
      } catch (e) {
        this.error = String(e);
        return false;
      } finally {
        this.busy = false;
      }
    },

    /**
     * 段数ポップオーバーのプレビュー (デザイン`C8rBw`)。`levels`を適用したときの
     * `symbol_id`・端子番号・ジャンパ指定を、**適用と同じ純関数**で計算する (図面は変えない)。
     * 端子台が読めない・段数が範囲外・今と同じ段数ならnull (プレビューを出さない)。
     */
    levelsPreview(levels: number): LevelsPreview | null {
      const inst = this.blockInstance;
      const def = this.blockDef;
      if (!inst || !def) return null;
      const symbolTo = multilevelSymbolId(inst.symbol_id, levels, def);
      if (symbolTo === null || symbolTo === inst.symbol_id) return null;
      const [poles, oldLevels] = polesAndLevelsOf(def);
      const from = this.displayTerminalNumbers;
      const to = relabelForLevels(from, poles, oldLevels, levels);
      // 先頭2極ぶんだけ見せる (デザインの `1,2,… → 1a,1b,2a,2b,…`)
      const head = (list: readonly string[], step: number) =>
        list.slice(0, step * 2).join(",") + (poles > 2 ? ",…" : "");
      const spec = inst.attrs.jumpers ?? "";
      const rename = new Map<string, string>();
      for (let p = 0; p < poles; p++) {
        for (let j = 0; j < Math.min(oldLevels, levels); j++) {
          rename.set(from[p * oldLevels + j] ?? "", to[p * levels + j] ?? "");
        }
      }
      return {
        symbolFrom: inst.symbol_id,
        symbolTo,
        numbersFrom: head(from, oldLevels),
        numbersTo: head(to, levels),
        jumpers: spec ? { from: spec, to: rewriteJumperLabels(spec, rename) } : null,
      };
    },

    /**
     * 段を`levels`へ減らせない理由の一覧 (デザイン`j83rxy`の「解除で消える段」)。
     * 空配列なら (今分かっている範囲では) 適用できる。段を増やすときは常に空。
     *
     * 判定はRust側`SetTerminalLevels`の拒否と同じ4種 (結線・ジャンパ・ピングループ・付属品) を
     * **すでに読み込んであるチャートと`attrs`から先読みする**もので、ポップオーバーは1件でも
     * あれば適用ボタンを無効にする。結線の有無はチャートの行 (`spare`) 由来なので、配線を伴わない
     * ネットラベル・ジャンクションだけが載っている端子はここには出ず、Rust側が座標一致で拒否する
     * (そのときはApply後の`error`が理由を見せる。二重の門で、どちらも図面は変えない)。
     */
    levelsBlockers(levels: number): LevelsBlocker[] {
      const inst = this.blockInstance;
      const def = this.blockDef;
      if (!inst || !def) return [];
      const [poles, oldLevels] = polesAndLevelsOf(def);
      if (levels >= oldLevels || poles === 0) return [];
      const display = this.displayTerminalNumbers;
      const vanishing: string[] = [];
      for (let p = 0; p < poles; p++) {
        for (let j = levels; j < oldLevels; j++) {
          const label = display[p * oldLevels + j];
          if (label !== undefined) vanishing.push(label);
        }
      }
      const rows = new Map((this.chart?.rows ?? []).map((r) => [r.terminal, r]));
      const jumperPairs = parseJumperSpec(inst.attrs.jumpers ?? "", display, this.rawTerminalNumbers);
      const labels = terminalLabels(def, inst.attrs);
      const pgOrder = chartOrderPinNumbers(def).map((n) => labels.displayOf(n));
      const groups = parsePinGroups(inst.attrs[PIN_GROUPS_ATTR] ?? "", pgOrder).groups;
      const accessoryLabels = new Set(
        (inst.attrs.accessories ?? "")
          .split(",")
          .map((item) => item.split(":")[0]?.trim() ?? "")
          .filter((label) => label !== "" && label !== "end"),
      );
      const out: LevelsBlocker[] = [];
      for (const terminal of vanishing) {
        const row = rows.get(terminal);
        const connection = [row?.internal ?? "", row?.external ?? ""].filter((s) => s !== "").join(" / ");
        const reason: LevelsBlocker["reason"] | null = (() => {
          if (row && !row.spare) return "wire";
          if (jumperPairs.some(([a, b]) => a === terminal || b === terminal)) return "jumper";
          const inGroup = groups.some((g) => {
            const a = pgOrder.indexOf(g.first);
            const b = pgOrder.indexOf(g.last);
            if (a < 0 || b < 0) return g.first === terminal || g.last === terminal;
            return pgOrder.slice(Math.min(a, b), Math.max(a, b) + 1).includes(terminal);
          });
          if (inGroup) return "pinGroup";
          if (accessoryLabels.has(terminal)) return "accessory";
          return null;
        })();
        if (reason !== null) out.push({ terminal, connection, reason });
      }
      return out;
    },

    /**
     * 段数を変える (`set_terminal_levels` Command 1回。Cmd+Zで戻る)。`levels`が今と同じなら
     * 何もしない。1〜`TERMINAL_LEVEL_MAX`の範囲チェックと、消える段への参照の先読み
     * ([`levelsBlockers`]) をここで済ませ、**当たっている案はコマンドを送らずに断る**
     * (ポップオーバーが「解除で消える段」を並べて適用ボタンを無効にしているので、ここは
     * 念のための門。採番の`renumberIssue`と同じ形)。先読みで拾えない結線 (配線を伴わない
     * ネットラベル・ジャンクション) はRust側が座標一致で拒否するので、失敗したら理由を
     * `error`に残す (ポップオーバーがそのまま見せる)。
     */
    async setLevels(levels: number): Promise<boolean> {
      const block = this.block;
      if (!block || this.busy) return false;
      if (!Number.isInteger(levels) || levels < 1 || levels > TERMINAL_LEVEL_MAX) return false;
      if (levels === this.levels) return true;
      if (this.levelsBlockers(levels).length > 0) return false;
      this.busy = true;
      this.error = null;
      try {
        await useDocumentStore().execute({
          type: "set_terminal_levels",
          sheet_id: block.sheet_id,
          entity_id: block.entity_id,
          levels,
        });
        this.selected = [];
        await this.reloadBlocks();
        return true;
      } catch (e) {
        this.error = String(e);
        return false;
      } finally {
        this.busy = false;
      }
    },

    /**
     * 予備挿入ポップオーバーの数値入力を、コマンドが受け取れる形 (1以上の整数) に丸める。
     */
    clampSpareCount(count: number): number {
      return Number.isFinite(count) ? Math.max(1, Math.trunc(count)) : 1;
    },

    /**
     * 予備挿入ポップオーバーの「プレビュー」(デザイン`eTFE5`の2行)。`after`(表示ラベル。
     * `null`なら末尾)の直後に`count`個の予備を挿入したときの`symbol_id`と新しく増える
     * 予備端子のラベルを、**適用と同じ純関数**([`insertSpareTerminalsPlan`])で計算する
     * (図面はまだ変えない)。適用できない案 ([`spareIssue`]) はnull (プレビューを出さない —
     * デザイン「予備端子の挿入 (エラー)」がプレビュー枠自体を出さないのと同じ形)。
     */
    sparePreview(after: string | null, count: number, renumberFollowing = true): SparePreview | null {
      const inst = this.blockInstance;
      const def = this.blockDef;
      if (!inst || !def) return null;
      const [poles, levels] = polesAndLevelsOf(def);
      const plan = insertSpareTerminalsPlan(
        inst.symbol_id,
        this.displayTerminalNumbers,
        poles,
        levels,
        after,
        count,
        renumberFollowing,
      );
      if (!plan) return null;
      return {
        symbolFrom: inst.symbol_id,
        symbolTo: plan.symbolId,
        spareLabels: plan.numbers.slice(poles * Math.max(levels, 1)),
      };
    },

    /**
     * 予備挿入案が適用できない理由 (適用できるならnull)。デザイン「予備端子の挿入 (エラー)」の
     * 指摘行が出す。極数の上限は[`insertSpareTerminalsPlan`]自身は見ない (循環importを避けて
     * Rust側と同じ定数[`DYNAMIC_PIN_MAX`]をここで直接確認する)ので、その門は先に通す。
     *
     * 純関数は理由を返さない (`null`だけ) ので、ユーザーに直せることが分かる2つの理由は
     * ここで切り分ける: 挿入位置のラベルに数字が無い / 「空き番号のまま残す」指定で番号が
     * ぶつかった (詰める指定なら通る)。それ以外はまとめて`invalid`。
     */
    spareIssue(after: string | null, count: number, renumberFollowing = true): SpareIssue | null {
      if (!Number.isInteger(count) || count < 1) return { code: "count" };
      const [poles, levels] = polesAndLevelsOf(this.blockDef);
      if (poles > 0 && poles + count > DYNAMIC_PIN_MAX) return { code: "poleCap", max: DYNAMIC_PIN_MAX };
      if (this.sparePreview(after, count, renumberFollowing) !== null) return null;
      const display = this.displayTerminalNumbers;
      const step = Math.max(levels, 1);
      const refLabel =
        after === null
          ? [...this.spareAfterOptions].pop()
          : display[Math.floor(display.indexOf(after) / step) * step];
      if (refLabel !== undefined && splitLabelNumber(refLabel) === null) {
        return { code: "nonNumeric", label: refLabel };
      }
      if (!renumberFollowing && this.sparePreview(after, count, true) !== null) {
        return { code: "keepDuplicate" };
      }
      return { code: "invalid" };
    },

    /** 予備挿入 (InsertSpareTerminals) コマンド。端子台が見つからなければnull。 */
    spareCommand(after: string | null, count: number, renumberFollowing = true): Command | null {
      const block = this.block;
      if (!block) return null;
      return {
        type: "insert_spare_terminals",
        sheet_id: block.sheet_id,
        entity_id: block.entity_id,
        after,
        count,
        renumber_following: renumberFollowing,
      };
    },

    /**
     * 予備を挿入する (Command 1回。Cmd+Zでsymbol_id・terminal_numbers・位置がまとめて戻る)。
     * 適用できない案 ([`spareIssue`]) は**コマンドを送らずに**断る (ポップオーバーが指摘行を
     * 出したまま適用ボタンを無効にしているので、ここは念のための門。採番・段数と同じ形)。
     */
    async insertSpareTerminals(
      after: string | null,
      count: number,
      renumberFollowing = true,
    ): Promise<boolean> {
      if (this.spareIssue(after, count, renumberFollowing) !== null) return false;
      const command = this.spareCommand(after, count, renumberFollowing);
      if (!command || this.busy) return false;
      this.busy = true;
      this.error = null;
      try {
        await useDocumentStore().execute(command);
        this.selected = [];
        await this.reloadBlocks();
        return true;
      } catch (e) {
        this.error = String(e);
        return false;
      } finally {
        this.busy = false;
      }
    },

    /**
     * 表示中の端子台へ図面をジャンプする (検索結果・XRefと同じ reveal 経路。呼び出し側の
     * `useReveal()`をそのまま渡す — Piniaストアからは `useReveal` 自身を呼べないため)。
     *
     * エディタは全画面モーダルなので、そのままではジャンプ先がスクリムの下に隠れる。
     * **先にエディタを閉じてから**revealし、開き直したときに同じ端子台・同じ行選択へ
     * 戻れるよう覚え書き (`resume`) を残す。
     */
    gotoTerminal(reveal: (target: RevealTarget) => void) {
      const block = this.block;
      if (!block) return;
      const memo = {
        sheetId: this.sheetId,
        entityId: block.entity_id,
        selected: [...this.selected],
      };
      const target: RevealTarget = { sheetId: block.sheet_id, entityIds: [block.entity_id] };
      this.close();
      this.resume = memo;
      reveal(target);
    },

    /** 端子台チェックを実行する (未結線の端子・不正なジャンパ)。 */
    async runCheck(): Promise<Diagnostic[] | null> {
      if (!this.entityId) return null;
      this.error = null;
      try {
        const diags = await ipc.checkTerminalBlock(this.entityId);
        this.diagnostics = [...diags].sort(
          (a, b) => SEVERITY_ORDER[a.severity] - SEVERITY_ORDER[b.severity],
        );
        return this.diagnostics;
      } catch (e) {
        this.error = String(e);
        return null;
      }
    },

    /**
     * 図面が変わったときにグリッドを追従させる (undo/redo・AIチャット・CLIなど、
     * このダイアログの外からの編集を含む)。直前のチェック結果は図面と食い違うので捨てる。
     *
     * 自分の編集 (ジャンパ・並べ替え) は`busy`の間に自分で読み直すので、その revision 通知は
     * 無視する (list+chart を二重に取りに行かない)。
     */
    async refresh() {
      if (!this.open || this.busy) return;
      this.selected = [];
      this.diagnostics = null;
      await this.reloadBlocks();
    },

    close() {
      this.open = false;
      this.resume = null;
      this.chart = null;
      this.blocks = [];
      this.entityId = null;
      this.selected = [];
      this.diagnostics = null;
      this.error = null;
    },
  },
});
