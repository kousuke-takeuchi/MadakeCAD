// DC動作点シミュレーション結果ストア (spec §3.6)。

import { defineStore } from "pinia";
import { ipc, type EntityLoad, type SimFinding, type SimOpResult } from "../ipc";
import { buildOverlay, type OverlayItem } from "../canvas/simOverlay";
import { parseSimCondition } from "./simConditions";
import { useDocumentStore } from "./document";

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
    /** 結果を計算したときの文書の revision (今と違えば結果は古い)。 */
    resultRevision: null as number | null,
    /** 結果を図面に重ねて表示するか。 */
    showOnDrawing: true,
  }),
  getters: {
    /** 計算の後で図面が変わったか。 */
    stale(state): boolean {
      return state.result !== null && state.resultRevision !== useDocumentStore().revision;
    },
    /** 図面に重ねる表示。パネルが閉じている・切ってある・結果が無い・別のシートなら null。 */
    overlay(state): { items: Map<string, OverlayItem>; stale: boolean } | null {
      const doc = useDocumentStore();
      const sheet = doc.activeSheet;
      if (!state.panelOpen || !state.showOnDrawing || !state.result || !sheet || state.sheetId !== doc.activeSheetId) {
        return null;
      }
      return { items: buildOverlay(state.result, sheet), stale: this.stale };
    },
    /** エンティティ id から負荷を引く (無ければ null)。 */
    loadOf(state): (id: string) => EntityLoad | null {
      const byId = new Map((state.result?.loads ?? []).map((l) => [l.entity_id, l]));
      return (id: string) => byId.get(id) ?? null;
    },
    /** 図面で選んだ物のうち、最初に負荷のある物の値。 */
    selectedLoad(): EntityLoad | null {
      for (const id of useDocumentStore().selection) {
        const l = this.loadOf(id);
        if (l) return l;
      }
      return null;
    },
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
    /** 条件の一覧 (チップの並び順: 置き換え → 短絡)。label は置き換えなら表示する文字そのもの、短絡ならネット名だけ (画面が「短絡」を足す)。 */
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
      // 計算を始めたときの図面の revision (計算中に編集されたら、結果は最初から古い)
      const revisionAtStart = useDocumentStore().revision;
      try {
        this.result = await ipc.simulateOp(sheetId, this.openSwitches, {
          overrides: this.overrides,
          shorts: this.shorts,
        });
        this.error = null;
        // 計算したシート (指定が無ければ先頭シート)。定格超過の行から部品へ移るとき、このシートへ切り替える
        this.sheetId = sheetId ?? useDocumentStore().project?.sheets[0]?.id ?? null;
        this.resultRevision = revisionAtStart;
      } catch (e) {
        this.result = null;
        this.resultRevision = null;
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
    /** 条件が変わったら、結果が出ている (または失敗している) ときだけ同じシートで計算し直す (結果と条件をずらさない)。 */
    rerunAfterChange() {
      if (this.result || this.error) void this.run(this.sheetId);
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
      this.rerunAfterChange();
      return true;
    },
    /** 条件を1つ外す (`conditions` の key)。 */
    removeCondition(key: string) {
      if (key.startsWith("short:")) {
        const net = key.slice("short:".length);
        this.shorts = this.shorts.filter((n) => n !== net);
        this.rerunAfterChange();
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
      this.rerunAfterChange();
    },
    clearConditions() {
      this.overrides = {};
      this.shorts = [];
      this.rerunAfterChange();
    },
    toggleShowOnDrawing() {
      this.showOnDrawing = !this.showOnDrawing;
    },
    close() {
      this.panelOpen = false;
    },
  },
});
