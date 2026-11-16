import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type SimOpResult } from "../ipc";
import { nextCaseName, useSimulationStore } from "./simulation";
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
    expect(spy).toHaveBeenCalledWith(null, ["SW1"], {});
    store.toggleOpen("SW1");
    await store.run(null);
    expect(spy).toHaveBeenLastCalledWith(null, [], {});
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


  const withLoads = (ids: string[]): SimOpResult => ({
    ...result,
    findings: [],
    loads: ids.map((id) => ({ entity_id: id, kind: "wire" as const, amps: 12, volts: [24, 23], limit: 10, limit_kind: "wire_ampacity" as const, ratio: 1.2 })),
  });
  const docWith = (ids: string[], sheetId = "s1") => {
    const doc = useDocumentStore();
    doc.project = { sheets: [{ id: sheetId, entities: Object.fromEntries(ids.map((id) => [id, { kind: "wire", id, points: [] }])) }] } as unknown as typeof doc.project;
    doc.activeSheetId = sheetId;
    doc.revision = 5;
    return doc;
  };

  // ja: 結果は計算したシートにだけ重ね、ほかのシートを開いているときは描かない
  it("the overlay is shown only on the sheet that was simulated", async () => {
    const doc = docWith(["w1"]);
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.run("s1");
    expect(store.overlay?.items.get("w1")?.level).toBe("over");
    doc.activeSheetId = "other";
    expect(store.overlay).toBeNull();
  });

  // ja: 計算の後で図面を編集すると結果は古いとみなされ、重ねる表示は「古い」印付きになる
  it("editing the drawing after a run marks the result stale", async () => {
    const doc = docWith(["w1"]);
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.run("s1");
    expect(store.stale).toBe(false);
    doc.revision = 6;
    expect(store.stale).toBe(true);
    expect(store.overlay?.stale).toBe(true);
  });

  // ja: 計算に失敗すると、前の結果の色は残らない
  it("a failed run leaves no overlay from the previous result", async () => {
    docWith(["w1"]);
    const store = useSimulationStore();
    const spy = vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.run("s1");
    spy.mockRejectedValue(new Error("ngspice"));
    await store.run("s1");
    expect(store.overlay).toBeNull();
  });

  // ja: 「図面に重ねる」を切ると表示が消え、パネルを閉じても消える
  it("turning off the drawing toggle or closing the panel hides the overlay", async () => {
    docWith(["w1"]);
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.run("s1");
    store.toggleShowOnDrawing();
    expect(store.overlay).toBeNull();
    store.toggleShowOnDrawing();
    expect(store.overlay).not.toBeNull();
    store.close();
    expect(store.overlay).toBeNull();
  });

  // ja: 図面で選んだ電線・部品の値を引ける (値の無い物を選んでいれば空)
  it("the selected wire or part's values can be looked up", async () => {
    const doc = docWith(["w1", "x"]);
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.run("s1");
    doc.selection = new Set(["x"]);
    expect(store.selectedLoad).toBeNull();
    doc.selection = new Set(["w1"]);
    expect(store.selectedLoad?.amps).toBe(12);
    expect(store.loadOf("w1")?.limit).toBe(10);
  });

  // ja: 計算中に図面を編集すると、その結果は最初から古いとみなされる (計算を始めたときの図面で判定する)
  it("an edit made while the simulation runs marks its result stale", async () => {
    const doc = docWith(["w1"]);
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockImplementation(async () => {
      doc.revision = 6;
      return withLoads(["w1"]);
    });
    await store.run("s1");
    expect(store.stale).toBe(true);
  });

  const fakeExecute = (doc: ReturnType<typeof useDocumentStore>) =>
    vi.spyOn(doc, "execute").mockImplementation(async (cmd) => {
      const cases = doc.project!.sim_cases ?? (doc.project!.sim_cases = []);
      if (cmd.type === "set_sim_case") {
        const i = cases.findIndex((c) => c.id === cmd.case.id);
        if (i >= 0) cases[i] = cmd.case;
        else cases.push(cmd.case);
      } else if (cmd.type === "remove_sim_case") {
        doc.project!.sim_cases = cases.filter((c) => c.id !== cmd.id);
      }
      return { revision: 1, ops: [] } as never;
    });

  // ja: 「通常」のまま条件を足すと「ケース 1」が作られて選ばれ、条件はそのケースに入る
  it("adding a condition while Normal is selected creates and selects Case 1", async () => {
    const doc = docWith(["w1"]);
    fakeExecute(doc);
    const store = useSimulationStore();
    expect(await store.addCondition("M1.current_a=40")).toBe(true);
    expect(store.selectedCase?.name).toBe("ケース 1");
    expect(store.selectedCase?.overrides).toEqual({ M1: { current_a: "40" } });
  });

  // ja: ケースの名前は空いている一番小さい番号になる
  it("new case names take the first free number", () => {
    const c = (name: string) => ({ id: name, name, open_switches: [], overrides: {}, faults: [] });
    expect(nextCaseName([])).toBe("ケース 1");
    expect(nextCaseName([c("ケース 1"), c("ケース 3")])).toBe("ケース 2");
  });

  // ja: 選んだケースで計算し、条件のチップはそのケースから作られる (消えた電線を指す故障は「見つからない」印付き)
  it("runs with the selected case and builds chips from it, marking faults whose wire is gone", async () => {
    const doc = docWith(["w1"]);
    doc.project!.sim_cases = [{ id: "c1", name: "Short", open_switches: [], overrides: {}, faults: [
      { kind: "short", target: { wire_id: "w1" } }, { kind: "open", wire_id: "gone" } ] }];
    const spy = vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    const store = useSimulationStore();
    store.selectCase("c1");
    await store.run("s1");
    expect(spy).toHaveBeenLastCalledWith("s1", [], { caseId: "c1" });
    expect(store.conditions.map((c) => [c.kind, c.missing])).toEqual([["short", false], ["open", true]]);
  });

  // ja: 選んでいるケースを消すと「通常」に戻る
  it("deleting the selected case falls back to Normal", async () => {
    const doc = docWith(["w1"]);
    fakeExecute(doc);
    const store = useSimulationStore();
    await store.addCondition("M1.current_a=40");
    await store.deleteCase();
    expect(store.caseId).toBeNull();
    expect(doc.project!.sim_cases).toEqual([]);
  });

  // ja: undo などでケースが無くなったら、選択は「通常」として扱われる
  it("a selected case that disappears from the project reads as Normal", async () => {
    const doc = docWith(["w1"]);
    fakeExecute(doc);
    const store = useSimulationStore();
    await store.addCondition("M1.current_a=40");
    doc.project!.sim_cases = [];
    expect(store.selectedCase).toBeNull();
  });

  // ja: 「短絡 ネット名」はそのネットの最初の電線への短絡としてケースに入る
  it("short NET is stored as a short on the net's first wire", async () => {
    const doc = docWith(["w1", "w2"]);
    fakeExecute(doc);
    vi.spyOn(ipc, "getNetlist").mockResolvedValue([{ name: "P24", pins: [], wire_ids: ["w2", "w1"] }] as never);
    const store = useSimulationStore();
    expect(await store.addCondition("短絡 P24")).toBe(true);
    expect(store.selectedCase?.faults).toEqual([{ kind: "short", target: { wire_id: "w2" } }]);
    expect(await store.addCondition("短絡 NOPE")).toBe(false);
  });

  // ja: ケースの名前の変更が断られたら false を返し、ケースは変わらない
  it("a refused rename returns false and leaves the case unchanged", async () => {
    const doc = docWith(["w1"]);
    fakeExecute(doc).mockRejectedValueOnce(new Error("既にあります"));
    const store = useSimulationStore();
    store.selectCase(null);
    doc.project!.sim_cases = [{ id: "c1", name: "A", open_switches: [], overrides: {}, faults: [] }];
    store.selectCase("c1");
    expect(await store.renameCase("B")).toBe(false);
    expect(store.selectedCase?.name).toBe("A");
  });

  // ja: 読めない条件の入力は断られ、ケースは作られない
  it("an unreadable condition is refused and creates no case", async () => {
    const doc = docWith(["w1"]);
    fakeExecute(doc);
    const store = useSimulationStore();
    expect(await store.addCondition("M1=40")).toBe(false);
    expect(doc.project!.sim_cases ?? []).toEqual([]);
  });

  // ja: 結果が出ているときにケースの条件を変えると、同じシートで自動で計算し直す
  it("changing the case after a run re-runs on the same sheet with the case", async () => {
    const doc = docWith(["w1"]);
    fakeExecute(doc);
    const store = useSimulationStore();
    const spy = vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.run("s1");
    await store.addCondition("M1.current_a=40");
    expect(spy).toHaveBeenLastCalledWith("s1", [], { caseId: store.caseId });
    expect(spy).toHaveBeenCalledTimes(2);
  });

  // ja: 結果がまだ無いときにケースを変えても、計算は始めない
  it("changing the case before any run does not start one", async () => {
    const doc = docWith(["w1"]);
    fakeExecute(doc);
    const store = useSimulationStore();
    const spy = vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.addCondition("M1.current_a=40");
    expect(spy).not.toHaveBeenCalled();
  });

  // ja: 条件は1つずつ外せ (部品の最後の属性を外すとその部品も消える)、属性名に「.」があっても外せる。まとめて全部外すとケースは空で残る
  it("conditions can be removed one by one, also with a dotted attribute, or all at once", async () => {
    const doc = docWith(["w1"]);
    fakeExecute(doc);
    const store = useSimulationStore();
    await store.addCondition("M1.rating.max=5");
    await store.addFault({ kind: "open", wire_id: "w1" });
    expect(store.conditions.map((c) => c.key)).toEqual(["set:M1.rating.max", "open:0"]);
    await store.removeCondition("set:M1.rating.max");
    expect(store.selectedCase?.overrides).toEqual({});
    await store.addCondition("M1.current_a=40");
    await store.clearConditions();
    expect(store.selectedCase?.faults).toEqual([]);
    expect(store.selectedCase?.overrides).toEqual({});
  });

  // ja: 同じ故障は2回足しても1つだけ
  it("the same fault is stored once", async () => {
    const doc = docWith(["w1"]);
    fakeExecute(doc);
    const store = useSimulationStore();
    await store.addFault({ kind: "short", target: { wire_id: "w1" } });
    await store.addFault({ kind: "short", target: { wire_id: "w1" } });
    expect(store.selectedCase?.faults.length).toBe(1);
  });

  // ja: ケースの複製は「元の名前 1」のような空いた名前で作られ、選ばれる
  it("duplicating a case makes a copy with a free name and selects it", async () => {
    const doc = docWith(["w1"]);
    fakeExecute(doc);
    const store = useSimulationStore();
    await store.addCondition("M1.current_a=40");
    const first = store.caseId;
    expect(await store.duplicateCase()).toBe(true);
    expect(store.caseId).not.toBe(first);
    expect(store.selectedCase?.name).toBe("ケース 1 1");
    expect(store.selectedCase?.overrides).toEqual({ M1: { current_a: "40" } });
  });
});
