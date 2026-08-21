import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Diagnostic } from "../ipc";
import { useVerificationStore } from "./verification";

const diag = (severity: Diagnostic["severity"], code: string): Diagnostic => ({
  severity,
  code,
  message: "m",
  sheet_id: "s1",
  entity_ids: ["e1"],
});

describe("verification store", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("runで診断を取得しパネルを開く。件数はseverity別に数える", async () => {
    const store = useVerificationStore();
    vi.spyOn(ipc, "verify").mockResolvedValue([
      diag("error", "erc.duplicate_reference"),
      diag("warning", "erc.unconnected_pin"),
      diag("warning", "elec.voltage_drop"),
      diag("info", "elec.no_current_attr"),
    ]);
    await store.run("s1");
    expect(store.panelOpen).toBe(true);
    expect(store.diagnostics).toHaveLength(4);
    expect(store.counts).toEqual({ error: 1, warning: 2, info: 1 });
  });

  it("closeでパネルを閉じ、診断は保持する", async () => {
    const store = useVerificationStore();
    vi.spyOn(ipc, "verify").mockResolvedValue([diag("error", "x")]);
    await store.run(null);
    store.close();
    expect(store.panelOpen).toBe(false);
    expect(store.diagnostics).toHaveLength(1);
  });

  it("取得失敗時はrunningが戻り診断は空のまま", async () => {
    const store = useVerificationStore();
    vi.spyOn(ipc, "verify").mockRejectedValue(new Error("down"));
    await expect(store.run(null)).rejects.toThrow("down");
    expect(store.running).toBe(false);
    expect(store.diagnostics).toHaveLength(0);
  });
});
