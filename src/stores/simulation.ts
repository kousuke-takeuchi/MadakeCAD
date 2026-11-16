// DC動作点シミュレーション結果ストア (spec §3.6)。
// 条件は計算ケース (プロジェクトの sim_cases) に保存する。編集は Command (set_sim_case / remove_sim_case) を通す。
// 「通常」(caseId = null) は条件の無い基本の状態で、保存しない。

import { defineStore } from "pinia";
import { ipc, type EntityLoad, type Point, type SimCase, type SimFault, type SimFinding, type SimOpResult } from "../ipc";
import { buildOverlay, type OverlayItem } from "../canvas/simOverlay";
import { faultPoints } from "../canvas/simCaseView";
import { parseSimCondition } from "./simConditions";
import { useDocumentStore } from "./document";

/** 新しいケースの名前: 空いている一番小さい番号の「ケース N」。 */
export function nextCaseName(cases: SimCase[], base = "ケース"): string {
  const names = new Set(cases.map((c) => c.name.trim()));
  for (let n = 1; ; n += 1) {
    if (!names.has(`${base} ${n}`)) return `${base} ${n}`;
  }
}

/** 条件のチップ1つ。 */
export interface SimConditionChip {
  key: string;
  /** 置き換えは「参照記号.属性=値」、短絡・断線は対象の説明 (電線 / 参照記号 ピンN)。 */
  label: string;
  kind: "set" | "short" | "open";
  /** 対象の電線・部品が図面に無い。 */
  missing: boolean;
}

const sameFault = (a: SimFault, b: SimFault) => JSON.stringify(a) === JSON.stringify(b);

export const useSimulationStore = defineStore("simulation", {
  state: () => ({
    result: null as SimOpResult | null,
    error: null as string | null,
    panelOpen: false,
    running: false,
    /** 開路扱いにするスイッチ/接点の参照記号 (what-if、ケースとは別に足す)。 */
    openSwitches: [] as string[],
    /** 選んでいる計算ケースの id (null = 通常)。 */
    caseId: null as string | null,
    /** 最後に計算したシート (定格超過の行から部品へ移るときに使う)。null = 先頭シート。 */
    sheetId: null as string | null,
    /** 結果を計算したときの文書の revision (今と違えば結果は古い)。 */
    resultRevision: null as number | null,
    /** 結果を図面に重ねて表示するか。 */
    showOnDrawing: true,
  }),
  getters: {
    /** プロジェクトの計算ケース。 */
    cases(): SimCase[] {
      return useDocumentStore().project?.sim_cases ?? [];
    },
    /** 選んでいるケース。消えていれば (undo など) null = 通常。 */
    selectedCase(state): SimCase | null {
      if (!state.caseId) return null;
      return this.cases.find((c) => c.id === state.caseId) ?? null;
    },
    /** 計算の後で図面が変わったか。 */
    stale(state): boolean {
      return state.result !== null && state.resultRevision !== useDocumentStore().revision;
    },
    /** 図面に重ねる表示。パネルが閉じている・切ってある・結果が無い・別のシートなら null。 */
    overlay(state): { items: Map<string, OverlayItem>; stale: boolean; faults: { kind: "short" | "open"; at: Point }[] } | null {
      const doc = useDocumentStore();
      const sheet = doc.activeSheet;
      if (!state.panelOpen || !state.showOnDrawing || !state.result || !sheet || state.sheetId !== doc.activeSheetId) {
        return null;
      }
      return {
        items: buildOverlay(state.result, sheet),
        stale: this.stale,
        faults: faultPoints(this.selectedCase, sheet, doc.resolveInstance),
      };
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
    /** 選んでいるケースの条件のチップ (置き換え → 故障の順)。 */
    conditions(): SimConditionChip[] {
      const c = this.selectedCase;
      if (!c) return [];
      const doc = useDocumentStore();
      const entities = doc.activeSheet?.entities ?? {};
      const out: SimConditionChip[] = [];
      for (const [reference, attrs] of Object.entries(c.overrides)) {
        for (const [attr, value] of Object.entries(attrs)) {
          out.push({ key: `set:${reference}.${attr}`, label: `${reference}.${attr}=${value}`, kind: "set", missing: false });
        }
      }
      c.faults.forEach((f, i) => {
        if (f.kind === "open") {
          out.push({ key: `open:${i}`, label: "電線", kind: "open", missing: !(f.wire_id in entities) });
        } else if ("wire_id" in f.target) {
          out.push({ key: `short:${i}`, label: "電線", kind: "short", missing: !(f.target.wire_id in entities) });
        } else {
          const e = entities[f.target.entity_id];
          const ref = e && e.kind === "symbol" ? e.reference || e.symbol_id : "";
          out.push({ key: `short:${i}`, label: `${ref} ピン${f.target.pin}`, kind: "short", missing: !e });
        }
      });
      return out;
    },
  },
  actions: {
    async run(sheetId: string | null) {
      this.running = true;
      // 計算を始めたときの図面の revision (計算中に編集されたら、結果は最初から古い)
      const revisionAtStart = useDocumentStore().revision;
      if (this.caseId && !this.selectedCase) this.caseId = null;
      try {
        this.result = await ipc.simulateOp(sheetId, this.openSwitches, this.caseId ? { caseId: this.caseId } : {});
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
    /** ケースを選ぶ (null = 通常) と、結果が出ていれば計算し直す。 */
    selectCase(id: string | null) {
      this.caseId = id;
      this.rerunAfterChange();
    },
    /** ケースを保存する (Command)。断られたら false。 */
    async saveCase(c: SimCase): Promise<boolean> {
      try {
        await useDocumentStore().execute({ type: "set_sim_case", case: c });
        return true;
      } catch {
        return false;
      }
    },
    /** 編集するケース。通常を選んでいれば「ケース N」を作って選ぶ。 */
    async ensureCase(): Promise<SimCase | null> {
      if (this.selectedCase) return this.selectedCase;
      const c: SimCase = { id: crypto.randomUUID(), name: nextCaseName(this.cases), open_switches: [], overrides: {}, faults: [] };
      if (!(await this.saveCase(c))) return null;
      this.caseId = c.id;
      return this.selectedCase;
    },
    /** 選んでいるケースを書き換えて保存し、計算し直す。 */
    async editCase(change: (c: SimCase) => SimCase): Promise<boolean> {
      const c = await this.ensureCase();
      if (!c) return false;
      const ok = await this.saveCase(change(JSON.parse(JSON.stringify(c)) as SimCase));
      if (ok) this.rerunAfterChange();
      return ok;
    },
    async newCase(): Promise<boolean> {
      const c: SimCase = { id: crypto.randomUUID(), name: nextCaseName(this.cases), open_switches: [], overrides: {}, faults: [] };
      if (!(await this.saveCase(c))) return false;
      this.selectCase(c.id);
      return true;
    },
    async renameCase(name: string): Promise<boolean> {
      const c = this.selectedCase;
      if (!c) return false;
      return this.saveCase({ ...c, name });
    },
    async duplicateCase(): Promise<boolean> {
      const c = this.selectedCase;
      if (!c) return false;
      const copy: SimCase = { ...JSON.parse(JSON.stringify(c)), id: crypto.randomUUID(), name: nextCaseName(this.cases, c.name) };
      if (!(await this.saveCase(copy))) return false;
      this.selectCase(copy.id);
      return true;
    },
    async deleteCase(): Promise<boolean> {
      const c = this.selectedCase;
      if (!c) return false;
      try {
        await useDocumentStore().execute({ type: "remove_sim_case", id: c.id });
      } catch {
        return false;
      }
      this.selectCase(null);
      return true;
    },
    /** 部品の属性をこのケースで置き換える (value が null か空なら置き換えを外す)。 */
    setOverride(reference: string, attr: string, value: string | null): Promise<boolean> {
      return this.editCase((c) => {
        const attrs = { ...(c.overrides[reference] ?? {}) };
        if (value === null || value.trim() === "") delete attrs[attr];
        else attrs[attr] = value.trim();
        const overrides = { ...c.overrides };
        if (Object.keys(attrs).length) overrides[reference] = attrs;
        else delete overrides[reference];
        return { ...c, overrides };
      });
    },
    /** 故障を足す (同じ故障は重ねない)。 */
    addFault(fault: SimFault): Promise<boolean> {
      return this.editCase((c) => (c.faults.some((f) => sameFault(f, fault)) ? c : { ...c, faults: [...c.faults, fault] }));
    },
    /** 故障を外す。 */
    removeFault(fault: SimFault): Promise<boolean> {
      return this.editCase((c) => ({ ...c, faults: c.faults.filter((f) => !sameFault(f, fault)) }));
    },
    /**
     * 条件の入力を足す。`参照記号.属性=値` は置き換え、`短絡 ネット名` はそのネットの最初の電線への短絡。
     * 読めない・ネットが無いときは false (何も変えない)。
     */
    async addCondition(text: string): Promise<boolean> {
      const c = parseSimCondition(text);
      if (c.kind === "error") return false;
      if (c.kind === "set") return this.setOverride(c.reference, c.attr, c.value);
      const doc = useDocumentStore();
      const sheetId = this.sheetId ?? doc.activeSheetId;
      if (!sheetId) return false;
      const nets = await ipc.getNetlist(sheetId);
      const net = nets.find((n) => n.name === c.net || n.label === c.net);
      const wireId = net?.wire_ids[0];
      if (!wireId) return false;
      return this.addFault({ kind: "short", target: { wire_id: wireId } });
    },
    /** 条件を1つ外す (`conditions` の key)。 */
    removeCondition(key: string): Promise<boolean> {
      const c = this.selectedCase;
      if (!c) return Promise.resolve(false);
      if (key.startsWith("set:")) {
        const body = key.slice("set:".length);
        const dot = body.indexOf(".");
        return this.setOverride(body.slice(0, dot), body.slice(dot + 1), null);
      }
      const i = Number(key.slice(key.indexOf(":") + 1));
      const fault = c.faults[i];
      return fault ? this.removeFault(fault) : Promise.resolve(false);
    },
    /** 選んでいるケースの条件を全部外す (ケースは残す)。 */
    clearConditions(): Promise<boolean> {
      if (!this.selectedCase) return Promise.resolve(false);
      return this.editCase((c) => ({ ...c, open_switches: [], overrides: {}, faults: [] }));
    },
    toggleShowOnDrawing() {
      this.showOnDrawing = !this.showOnDrawing;
    },
    close() {
      this.panelOpen = false;
    },
  },
});
