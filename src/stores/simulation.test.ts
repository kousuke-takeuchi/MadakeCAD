import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type SimOpResult } from "../ipc";
import { useSimulationStore } from "./simulation";
import { useDocumentStore } from "./document";

const result: SimOpResult = {
  voltage: 24,
  nets: [{ name: "N001", volts_min: 23.9, volts_max: 24, wire_ids: [] }],
  components: [{ reference: "L1", entity_id: "e1", amps: 2, watts: 48 }],
  warnings: [],
};

describe("simulation store", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
  });

  // ja: runでDC解析結果を取得してパネルを開く
  it("run fetches the DC result and opens the panel", async () => {
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockResolvedValue(result);
    await store.run(null);
    expect(store.panelOpen).toBe(true);
    expect(store.result?.components[0].reference).toBe("L1");
    expect(store.error).toBeNull();
  });

  // ja: 開閉トグルは再実行時にopen_switchesとして渡される
  it("toggled switches are passed as open_switches on the next run", async () => {
    const store = useSimulationStore();
    const spy = vi.spyOn(ipc, "simulateOp").mockResolvedValue(result);
    store.toggleOpen("SW1");
    await store.run(null);
    expect(spy).toHaveBeenCalledWith(null, ["SW1"], { overrides: {}, shorts: [] });
    store.toggleOpen("SW1");
    await store.run(null);
    expect(spy).toHaveBeenLastCalledWith(null, [], { overrides: {}, shorts: [] });
  });

  // ja: 失敗時(ngspice未導入等)はエラーメッセージを保持したままパネルを開く
  it("failures (e.g. ngspice missing) keep the error message and still open the panel", async () => {
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockRejectedValue(new Error("ngspiceが見つかりません"));
    await store.run(null);
    expect(store.panelOpen).toBe(true);
    expect(store.result).toBeNull();
    expect(store.error).toContain("ngspice");
  });

  // ja: 条件の入力を足すと、次の実行で属性の置き換えと短絡が送られる。同じ属性は上書き、同じネットは1回だけ
  it("added conditions are sent as overrides and shorts on the next run", async () => {
    const store = useSimulationStore();
    const spy = vi.spyOn(ipc, "simulateOp").mockResolvedValue(result);
    expect(store.addCondition("M1.current_a=40")).toBe(true);
    expect(store.addCondition("M1.v_min=20")).toBe(true);
    expect(store.addCondition("M1.current_a=31")).toBe(true);
    expect(store.addCondition("短絡 P24")).toBe(true);
    expect(store.addCondition("short P24")).toBe(true);
    await store.run(null);
    expect(spy).toHaveBeenLastCalledWith(null, [], {
      overrides: { M1: { current_a: "31", v_min: "20" } },
      shorts: ["P24"],
    });
    expect(store.conditions.map((c) => c.label)).toEqual(["M1.current_a=31", "M1.v_min=20", "P24"]);
  });

  // ja: 読めない条件の入力は断られ、条件は変わらない
  it("an unreadable condition is refused and changes nothing", () => {
    const store = useSimulationStore();
    expect(store.addCondition("M1=40")).toBe(false);
    expect(store.conditions).toEqual([]);
  });

  // ja: 条件は1つずつ外せ (部品の最後の属性を外すとその部品も消える)、まとめて全部外せる
  it("conditions can be removed one by one or all at once", () => {
    const store = useSimulationStore();
    store.addCondition("M1.current_a=40");
    store.addCondition("短絡 P24");
    store.removeCondition("set:M1.current_a");
    expect(store.overrides).toEqual({});
    expect(store.shorts).toEqual(["P24"]);
    store.removeCondition("short:P24");
    expect(store.shorts).toEqual([]);
    store.addCondition("M1.current_a=40");
    store.addCondition("短絡 P24");
    store.clearConditions();
    expect(store.conditions).toEqual([]);
  });

  // ja: 定格超過は重さごとに数えられ、結果に定格超過が無ければ空になる
  it("rating findings are counted by severity and empty when the result has none", async () => {
    const store = useSimulationStore();
    expect(store.findings).toEqual([]);
    const f = (severity: "error" | "warning" | "info") => ({
      code: "sim.fuse_rating", severity, reference: "F1", entity_ids: ["e1"], value: 8, limit: 5, message: "F1",
    });
    vi.spyOn(ipc, "simulateOp").mockResolvedValue({ ...result, findings: [f("error"), f("warning"), f("warning")] });
    await store.run(null);
    expect(store.counts).toEqual({ error: 1, warning: 2, info: 0 });
  });

  // ja: 結果が出ているときに条件を変えると、同じシートで自動で計算し直す (画面の条件と結果がずれない)
  it("changing a condition after a run re-runs on the same sheet", async () => {
    const store = useSimulationStore();
    const spy = vi.spyOn(ipc, "simulateOp").mockResolvedValue(result);
    await store.run("s1");
    store.addCondition("短絡 P24");
    expect(spy).toHaveBeenLastCalledWith("s1", [], { overrides: {}, shorts: ["P24"] });
    store.removeCondition("short:P24");
    expect(spy).toHaveBeenLastCalledWith("s1", [], { overrides: {}, shorts: [] });
    expect(spy).toHaveBeenCalledTimes(3);
  });

  // ja: 結果がまだ無いときに条件を変えても、計算は始めない
  it("changing a condition before any run does not start one", () => {
    const store = useSimulationStore();
    const spy = vi.spyOn(ipc, "simulateOp").mockResolvedValue(result);
    store.addCondition("M1.current_a=40");
    store.clearConditions();
    expect(spy).not.toHaveBeenCalled();
  });

  // ja: 計算したシートを覚え、シートの指定が無ければ先頭シートを覚える
  it("a run remembers its sheet, or the first sheet when none is given", async () => {
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockResolvedValue(result);
    await store.run("s2");
    expect(store.sheetId).toBe("s2");
    const doc = useDocumentStore();
    doc.project = { sheets: [{ id: "first" }] } as unknown as typeof doc.project;
    await store.run(null);
    expect(store.sheetId).toBe("first");
  });

  // ja: 属性名に「.」を含む置き換えも、チップから正しく外せる
  it("an override whose attribute contains a dot can be removed from its chip", () => {
    const store = useSimulationStore();
    store.addCondition("M1.rating.max=5");
    expect(store.overrides).toEqual({ M1: { "rating.max": "5" } });
    const key = store.conditions[0].key;
    expect(key).toBe("set:M1.rating.max");
    store.removeCondition(key);
    expect(store.overrides).toEqual({});
  });
});
