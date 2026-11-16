// DC動作点シミュレーション結果ストア (spec §3.6)。

import { defineStore } from "pinia";
import { ipc, type SimFinding, type SimOpResult } from "../ipc";
import { parseSimCondition } from "./simConditions";

export const useSimulationStore = defineStore("simulation", {
  state: () => ({
    result: null as SimOpResult | null,
    error: null as string | null,
    panelOpen: false,
    running: false,
    /** 開路扱いにするスイッチ/接点の参照記号 (what-if)。 */
    openSwitches: [] as string[],
    /** 部品の属性の置き換え (参照記号 → {属性: 値})。その実行だけに効く。 */
    overrides: {} as Record<string, Record<string, string>>,
    /** 接地へ短絡させるネット名。 */
    shorts: [] as string[],
    /** 最後に計算したシート (定格超過の行から部品へ移るときに使う)。null = 先頭シート。 */
    sheetId: null as string | null,
  }),
  getters: {
    /** 定格超過 (結果に無ければ空)。 */
    findings(state): SimFinding[] {
      return state.result?.findings ?? [];
    },
    /** 定格超過の重さごとの件数。 */
    counts(): Record<SimFinding["severity"], number> {
      const c = { error: 0, warning: 0, info: 0 };
      for (const f of this.findings) c[f.severity] += 1;
      return c;
    },
    /** 条件の一覧 (チップの並び順: 置き換え → 短絡)。 */
    conditions(state): { key: string; label: string; kind: "set" | "short" }[] {
      const out: { key: string; label: string; kind: "set" | "short" }[] = [];
      for (const [reference, attrs] of Object.entries(state.overrides)) {
        for (const [attr, value] of Object.entries(attrs)) {
          out.push({ key: `set:${reference}.${attr}`, label: `${reference}.${attr}=${value}`, kind: "set" });
        }
      }
      for (const net of state.shorts) out.push({ key: `short:${net}`, label: net, kind: "short" });
      return out;
    },
  },
  actions: {
    async run(sheetId: string | null) {
      this.running = true;
      this.sheetId = sheetId;
      try {
        this.result = await ipc.simulateOp(sheetId, this.openSwitches, {
          overrides: this.overrides,
          shorts: this.shorts,
        });
        this.error = null;
      } catch (e) {
        this.result = null;
        this.error = e instanceof Error ? e.message : String(e);
      } finally {
        this.running = false;
        this.panelOpen = true;
      }
    },
    toggleOpen(reference: string) {
      const i = this.openSwitches.indexOf(reference);
      if (i >= 0) this.openSwitches.splice(i, 1);
      else this.openSwitches.push(reference);
    },
    /** 条件の入力を足す。読めなければ false (何も変えない)。同じ属性・同じネットは上書き・重複しない。 */
    addCondition(text: string): boolean {
      const c = parseSimCondition(text);
      if (c.kind === "error") return false;
      if (c.kind === "set") {
        this.overrides = { ...this.overrides, [c.reference]: { ...(this.overrides[c.reference] ?? {}), [c.attr]: c.value } };
      } else if (!this.shorts.includes(c.net)) {
        this.shorts = [...this.shorts, c.net];
      }
      return true;
    },
    /** 条件を1つ外す (`conditions` の key)。 */
    removeCondition(key: string) {
      if (key.startsWith("short:")) {
        const net = key.slice("short:".length);
        this.shorts = this.shorts.filter((n) => n !== net);
        return;
      }
      const body = key.slice("set:".length);
      const dot = body.indexOf(".");
      const reference = body.slice(0, dot);
      const attr = body.slice(dot + 1);
      const rest = { ...(this.overrides[reference] ?? {}) };
      delete rest[attr];
      const next = { ...this.overrides };
      if (Object.keys(rest).length) next[reference] = rest;
      else delete next[reference];
      this.overrides = next;
    },
    clearConditions() {
      this.overrides = {};
      this.shorts = [];
    },
    close() {
      this.panelOpen = false;
    },
  },
});
