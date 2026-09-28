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
import { dynamicSymbol, parseTerminalNo, TERMINAL_LEVEL_MAX } from "../canvas/dynamicSymbol";
// ジャンパ指定の正規化・並べ替え順の解釈はキャンバス描画と共通の純関数を使う
// (Pinia非依存モジュールが正。Rust側 madake-core/src/terminal_chart.rs の双子)
import {
  formatJumperSpec,
  parseJumperSpec,
  rewriteJumperLabels,
  terminalNumbers,
  terminalNumbersOfInstance,
  terminalRenumberPlan,
  terminalsAreAdjacent,
  validTerminalNumbers,
} from "../canvas/terminalJumpers";
import type { RevealTarget } from "../composables/reveal";
import { ipc, type Command, type Diagnostic, type RowOrder, type TerminalBlockInfo, type TerminalChart } from "../ipc";
import { useDocumentStore } from "./document";

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
      const doc = useDocumentStore();
      const block = this.block;
      if (!block) return [];
      const sheet = doc.project?.sheets.find((s) => s.id === block.sheet_id);
      const entity = sheet?.entities[block.entity_id];
      if (!entity || entity.kind !== "symbol") return [];
      const def = dynamicSymbol(entity.symbol_id);
      return def ? terminalNumbers(def) : [];
    },
    /**
     * 表示中の端子台の**現在の**端子番号 (`rawTerminalNumbers`と同じ並びの表示ラベル)。
     * 上書きが無ければ生の番号と同じ。採番プレビューがジャンパの追従を見せるのに使う
     * (今のジャンパ指定はこのラベルで書かれている)。
     */
    displayTerminalNumbers(): string[] {
      const doc = useDocumentStore();
      const block = this.block;
      if (!block) return [];
      const sheet = doc.project?.sheets.find((s) => s.id === block.sheet_id);
      const entity = sheet?.entities[block.entity_id];
      if (!entity || entity.kind !== "symbol") return [];
      const def = dynamicSymbol(entity.symbol_id);
      return def ? terminalNumbersOfInstance(def, entity.attrs) : [];
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
     * 段数を変える (`set_terminal_levels` Command 1回。Cmd+Zで戻る)。`levels`が今と同じなら
     * 何もしない。1〜`TERMINAL_LEVEL_MAX`の範囲チェックだけここでしておき (ステッパーの
     * +/-ボタンがその外へ出さないための門)、消える段への参照が残っているかどうかの判定は
     * Rust側に任せる (結線・ジャンパ・ピングループを跨いだ判定になるため、ここでは
     * 二重に持たない)。失敗したら理由を`error`に残す (ポップオーバーが`ui.log`で見せる)。
     */
    async setLevels(levels: number): Promise<boolean> {
      const block = this.block;
      if (!block || this.busy) return false;
      if (!Number.isInteger(levels) || levels < 1 || levels > TERMINAL_LEVEL_MAX) return false;
      if (levels === this.levels) return true;
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
