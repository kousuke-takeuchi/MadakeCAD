// プロジェクト内検索UI (⌘F浮きバー+下部ドック結果パネル) の仕様テスト
// (計画: docs/superpowers/plans/2026-08-22-m4-phase2-macros-xref-nav.md Task 4)。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type SearchHit } from "../ipc";
import { useSearchStore } from "./search";

function hit(entityId: string, over: Partial<SearchHit> = {}): SearchHit {
  return {
    kind: "reference",
    text: "K1",
    detail: "MY2N",
    function: "coil",
    terminals: "A1-A2",
    sheet_id: "sheet-1",
    sheet_no: 1,
    sheet_name: "シート1",
    zone: "C2",
    entity_id: entityId,
    ...over,
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.restoreAllMocks();
});

describe("検索バー", () => {
  // ja: ⌘Fで検索バーが開く
  it("Cmd+F opens the floating search bar", () => {
    const search = useSearchStore();
    expect(search.barOpen).toBe(false);
    search.openBar();
    expect(search.barOpen).toBe(true);
  });

  // ja: Escで検索バーと結果パネルが閉じ、検索語は次に開いたときのために残る
  it("Escape closes the bar and the results panel but keeps the query for next time", async () => {
    const search = useSearchStore();
    vi.spyOn(ipc, "searchProject").mockResolvedValue([hit("e1")]);
    search.openBar();
    search.setQuery("K1");
    await search.run();
    expect(search.panelOpen).toBe(true);
    search.close();
    expect(search.barOpen).toBe(false);
    expect(search.panelOpen).toBe(false);
    expect(search.query).toBe("K1");
  });

  // ja: 検索すると件数が出て、結果パネルが開く
  it("running a search fills in the count and opens the results panel", async () => {
    const search = useSearchStore();
    vi.spyOn(ipc, "searchProject").mockResolvedValue([hit("e1"), hit("e2")]);
    search.setQuery("K1");
    await search.run();
    expect(search.count).toBe(2);
    expect(search.panelOpen).toBe(true);
    expect(search.activeIndex).toBe(-1);
  });

  // ja: 検索語が空なら検索そのものを行わず、結果を捨ててパネルを閉じる
  it("an empty query searches nothing and closes the panel", async () => {
    const search = useSearchStore();
    const call = vi.spyOn(ipc, "searchProject").mockResolvedValue([hit("e1")]);
    search.setQuery("K1");
    await search.run();
    search.setQuery("   ");
    await search.run();
    expect(call).toHaveBeenCalledTimes(1);
    expect(search.count).toBe(0);
    expect(search.panelOpen).toBe(false);
  });
});

describe("フィルタチップ", () => {
  // ja: 「すべて」のときは対象を絞らずに検索する
  it("the All chip searches without narrowing the targets", async () => {
    const search = useSearchStore();
    const call = vi.spyOn(ipc, "searchProject").mockResolvedValue([]);
    search.setQuery("K1");
    await search.run();
    expect(call).toHaveBeenCalledWith("K1", []);
  });

  // ja: チップを切り替えると、その対象で検索し直す
  it("switching chips searches again with that target", async () => {
    const search = useSearchStore();
    const call = vi.spyOn(ipc, "searchProject").mockResolvedValue([]);
    search.setQuery("K1");
    await search.setFilter("reference");
    expect(search.filter).toBe("reference");
    expect(call).toHaveBeenCalledWith("K1", ["reference"]);
    await search.setFilter("net");
    expect(call).toHaveBeenCalledWith("K1", ["net", "wire_no"]);
  });

  // ja: 前後の空白は検索語から落とす
  it("surrounding whitespace is trimmed from the query", async () => {
    const search = useSearchStore();
    const call = vi.spyOn(ipc, "searchProject").mockResolvedValue([]);
    search.setQuery("  K1  ");
    await search.run();
    expect(call).toHaveBeenCalledWith("K1", []);
  });
});

describe("Enter巡回と行クリック", () => {
  // ja: Enterで次のヒットへ進み、末尾からは先頭へ回り込む
  it("Enter walks to the next hit and wraps around at the end", async () => {
    const search = useSearchStore();
    vi.spyOn(ipc, "searchProject").mockResolvedValue([hit("e1"), hit("e2")]);
    search.setQuery("K1");
    await search.run();
    expect(search.step(1)?.entity_id).toBe("e1");
    expect(search.step(1)?.entity_id).toBe("e2");
    expect(search.step(1)?.entity_id).toBe("e1");
  });

  // ja: Shift+Enterで前のヒットへ戻る
  it("Shift+Enter walks back to the previous hit", async () => {
    const search = useSearchStore();
    vi.spyOn(ipc, "searchProject").mockResolvedValue([hit("e1"), hit("e2")]);
    search.setQuery("K1");
    await search.run();
    expect(search.step(-1)?.entity_id).toBe("e2");
    expect(search.step(-1)?.entity_id).toBe("e1");
  });

  // ja: 結果が0件なら巡回しても何も選ばれない
  it("with no hits there is nothing to walk to", async () => {
    const search = useSearchStore();
    vi.spyOn(ipc, "searchProject").mockResolvedValue([]);
    search.setQuery("zzz");
    await search.run();
    expect(search.step(1)).toBeNull();
    expect(search.activeIndex).toBe(-1);
  });

  // ja: 結果パネルの行をクリックすると、その行が巡回位置になる
  it("clicking a result row makes that row the current one", async () => {
    const search = useSearchStore();
    vi.spyOn(ipc, "searchProject").mockResolvedValue([hit("e1"), hit("e2")]);
    search.setQuery("K1");
    await search.run();
    expect(search.select(1)?.entity_id).toBe("e2");
    expect(search.activeIndex).toBe(1);
    expect(search.activeHit?.entity_id).toBe("e2");
    // 範囲外の行は無視する
    expect(search.select(9)).toBeNull();
    expect(search.activeIndex).toBe(1);
  });

  // ja: 検索し直すと巡回位置は先頭より前(未選択)に戻る
  it("searching again resets the walk position", async () => {
    const search = useSearchStore();
    vi.spyOn(ipc, "searchProject").mockResolvedValue([hit("e1"), hit("e2")]);
    search.setQuery("K1");
    await search.run();
    search.step(1);
    await search.run();
    expect(search.activeIndex).toBe(-1);
  });

  // ja: パネルの✕は結果パネルだけ閉じ、検索バーは開いたままにする
  it("the panel's close button closes only the results panel", async () => {
    const search = useSearchStore();
    vi.spyOn(ipc, "searchProject").mockResolvedValue([hit("e1")]);
    search.openBar();
    search.setQuery("K1");
    await search.run();
    search.closePanel();
    expect(search.panelOpen).toBe(false);
    expect(search.barOpen).toBe(true);
  });
});
