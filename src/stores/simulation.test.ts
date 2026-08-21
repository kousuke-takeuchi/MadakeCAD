import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type SimOpResult } from "../ipc";
import { useSimulationStore } from "./simulation";

const result: SimOpResult = {
  voltage: 24,
  nets: [{ name: "N001", volts_min: 23.9, volts_max: 24, wire_ids: [] }],
  components: [{ reference: "L1", entity_id: "e1", amps: 2, watts: 48 }],
  warnings: [],
};

describe("simulation store", () => {
  beforeEach(() => setActivePinia(createPinia()));

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
    expect(spy).toHaveBeenCalledWith(null, ["SW1"]);
    store.toggleOpen("SW1");
    await store.run(null);
    expect(spy).toHaveBeenLastCalledWith(null, []);
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
});
