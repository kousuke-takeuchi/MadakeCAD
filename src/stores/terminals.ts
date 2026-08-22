// 端子台エディタの状態 (仕様: docs/internal/specs/m4-industrial-core.md §1.1)。
//
// グリッドの中身はRust側 (madake-coreの端子台チャート) が図面から導出したものをそのまま
// 表示するだけで、ここで結線を組み立て直したりはしない。編集できるのはジャンパだけで、
// update_entity コマンド1回として送る (undo一発で戻る)。
// v1のスコープ: 行表示+ジャンパ生成/削除+予備端子表示+端子台チェック+チャート/接続図の生成。
// 並べ替え・多段端子・アクセサリはモデル拡張が要るためフェーズ2。

import { defineStore } from "pinia";
import { ipc, type Command, type Diagnostic, type TerminalBlockInfo, type TerminalChart } from "../ipc";
import { useDocumentStore } from "./document";

/** グリッドの1行 (デザイン「端子台エディタ」の列順)。 */
export interface TerminalEditorRow {
  /** 行番号 (1始まり)。 */
  no: number;
  /** 端子番号。 */
  terminal: string;
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

/** ジャンパ指定 (`"1-2,3-4"`) を解析する。小さい番号が先・昇順・重複なしに正規化する。 */
export function parseJumperSpec(spec: string): [number, number][] {
  const pairs: [number, number][] = [];
  for (const item of spec.split(",")) {
    const parts = item.split("-").map((p) => p.trim());
    if (parts.length !== 2) continue;
    if (!/^\d+$/.test(parts[0]) || !/^\d+$/.test(parts[1])) continue;
    const a = Math.min(Number(parts[0]), Number(parts[1]));
    const b = Math.max(Number(parts[0]), Number(parts[1]));
    // 掛けられるのは隣り合う端子どうしだけ (Rust側の parse_jumpers と同じ規則)
    if (b - a !== 1) continue;
    if (!pairs.some(([x, y]) => x === a && y === b)) pairs.push([a, b]);
  }
  pairs.sort((p, q) => p[0] - q[0]);
  return pairs;
}

/** ジャンパ指定の文字列表現 (`attrs["jumpers"]` に入れる形)。 */
export function formatJumperSpec(pairs: readonly [number, number][]): string {
  return pairs.map(([a, b]) => `${a}-${b}`).join(",");
}

/**
 * 選んだ端子どうしにジャンパを掛けた指定を返す。既存のジャンパは残る。
 * 端子が2つ未満、または隣り合っていない端子が混ざっていればnull (掛けられない)。
 */
export function withJumpers(spec: string, terminals: readonly number[]): string | null {
  const sorted = [...terminals].sort((a, b) => a - b);
  if (sorted.length < 2) return null;
  const pairs = parseJumperSpec(spec);
  for (let i = 1; i < sorted.length; i++) {
    const [a, b] = [sorted[i - 1], sorted[i]];
    if (b - a !== 1) return null;
    if (!pairs.some(([x, y]) => x === a && y === b)) pairs.push([a, b]);
  }
  pairs.sort((p, q) => p[0] - q[0]);
  return formatJumperSpec(pairs);
}

/** 選んだ端子に掛かっているジャンパだけを外した指定を返す。 */
export function withoutJumpers(spec: string, terminals: readonly number[]): string {
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
    /** 極数。 */
    terminalCount(state): number {
      return state.chart?.terminal_count ?? this.block?.terminal_count ?? 0;
    },
    /** グリッドの行 (端子番号順)。 */
    rows(state): TerminalEditorRow[] {
      return (state.chart?.rows ?? []).map((r, i) => ({
        no: i + 1,
        terminal: r.terminal,
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
    /** 選択中の端子番号 (数値・昇順)。数値でない端子番号は無視する。 */
    selectedTerminals(state): number[] {
      return state.selected
        .filter((t) => /^\d+$/.test(t))
        .map(Number)
        .sort((a, b) => a - b);
    },
    /** 現在のジャンパ指定 (図面の値)。 */
    jumperSpec(state): string {
      return formatJumperSpec(state.chart?.jumpers ?? []);
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
    /** シートの端子台一覧を読み込み、先頭の端子台を表示して開く。 */
    async openFor(sheetId: string | null) {
      this.sheetId = sheetId;
      this.selected = [];
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
      await this.reloadChart();
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

    close() {
      this.open = false;
      this.chart = null;
      this.blocks = [];
      this.entityId = null;
      this.selected = [];
      this.diagnostics = null;
      this.error = null;
    },
  },
});
