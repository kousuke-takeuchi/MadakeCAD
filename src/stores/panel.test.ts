// 未配置部品パネル (左ドック「未配置部品」ビュー) の仕様テスト
// (M4仕様 §8、.pen「M4デザイン - 盤レイアウト」)。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type UnplacedComponent } from "../ipc";
import { usePanelStore } from "./panel";

function component(reference: string, part_no = "MDK-RLY-24V"): UnplacedComponent {
  return { reference, part_no, symbol_id: "relay_coil", sheet_id: "s1", w: 25, h: 65 };
}

describe("unplaced parts store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 未配置部品の一覧はRust側 (回路図と盤シートの突き合わせ) から読み込む。
  it("loads the unplaced parts from the engine", async () => {
    vi.spyOn(ipc, "unplacedComponents").mockResolvedValue([component("K1"), component("F1", "")]);
    const panel = usePanelStore();
    await panel.load();
    expect(panel.count).toBe(2);
    expect(panel.rows.map((r) => r.reference)).toEqual(["K1", "F1"]);
    expect(panel.rows[1].status).toBe("unregistered");
  });

  // ja: 図面が変わっていなければ一覧を読み直さない (置くたびの往復を減らす)。
  it("does not reload while the drawing has not changed", async () => {
    const spy = vi.spyOn(ipc, "unplacedComponents").mockResolvedValue([component("K1")]);
    const panel = usePanelStore();
    await panel.load();
    await panel.load();
    expect(spy).toHaveBeenCalledTimes(1);
    await panel.load(true);
    expect(spy).toHaveBeenCalledTimes(2);
  });

  // ja: 部品を盤へ置いて図面が変わると、次の読み込みで一覧から消える。
  it("drops a part from the list once it has been placed", async () => {
    const spy = vi.spyOn(ipc, "unplacedComponents").mockResolvedValue([component("K1"), component("K2")]);
    const panel = usePanelStore();
    await panel.load();
    spy.mockResolvedValue([component("K2")]);
    await panel.load(true);
    expect(panel.rows.map((r) => r.reference)).toEqual(["K2"]);
  });

  // ja: 一覧が読めなくても画面は開いたまま (理由を控えて空一覧にする)。
  it("keeps the panel open when the list cannot be read", async () => {
    vi.spyOn(ipc, "unplacedComponents").mockRejectedValue(new Error("Link API 500"));
    const panel = usePanelStore();
    await panel.load();
    expect(panel.count).toBe(0);
    expect(panel.error).toContain("Link API 500");
  });

  // ja: 配置する部品を選ぶと、その行だけが選択表示になる。
  it("highlights the row of the part being placed", async () => {
    vi.spyOn(ipc, "unplacedComponents").mockResolvedValue([component("K1"), component("K2")]);
    const panel = usePanelStore();
    await panel.load();
    panel.select("K2");
    expect(panel.selected).toBe("K2");
    panel.select(null);
    expect(panel.selected).toBeNull();
  });
});
