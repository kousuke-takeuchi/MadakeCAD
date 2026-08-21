import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Part } from "../ipc";
import { usePartsStore } from "./parts";

const part = (part_no: string): Part => ({
  part_no,
  maker: "サンプル",
  name: "テスト部品",
  category: "relay",
  symbol_id: "relay_coil",
  rated_voltage: "DC24V",
  rated_current_a: 0.05,
  purchase_url: "",
  datasheet_url: "",
  price: null,
  currency: "JPY",
  note: "",
  model_3d: "",
  mounting: "",
});

describe("parts store", () => {
  beforeEach(() => setActivePinia(createPinia()));

  // ja: searchは部品APIの結果を保持する
  it("search stores the results from the parts API", async () => {
    const store = usePartsStore();
    vi.spyOn(ipc, "searchParts").mockResolvedValue([part("MDK-RLY-24V")]);
    await store.search("RLY");
    expect(store.results).toHaveLength(1);
    expect(store.results[0].part_no).toBe("MDK-RLY-24V");
    expect(store.loading).toBe(false);
  });

  // ja: 検索失敗時は結果を空にしloadingを戻す
  it("a failed search clears the results and resets loading", async () => {
    const store = usePartsStore();
    vi.spyOn(ipc, "searchParts").mockResolvedValue([part("A")]);
    await store.search("");
    vi.spyOn(ipc, "searchParts").mockRejectedValue(new Error("down"));
    await store.search("x");
    expect(store.results).toHaveLength(0);
    expect(store.loading).toBe(false);
  });
});
