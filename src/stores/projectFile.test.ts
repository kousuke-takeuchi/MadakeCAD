// プロジェクトファイル(開く/保存/最近使ったファイル)の仕様テスト。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Patch, type Project } from "../ipc";
import { useDocumentStore } from "./document";
import {
  RECENT_LIMIT,
  RECENT_STORAGE_KEY,
  fileNameOf,
  isDxfPath,
  isKicadPath,
  localRecentStorage,
  useProjectFileStore,
  type RecentStorage,
} from "./projectFile";

function project(name = "t"): Project {
  return { format_version: 2, name, wire_parts: [], plc_assignments: [], sheets: [] };
}

function replaced(revision: number, name = "t"): Patch {
  return { revision, ops: [{ op: "project_replaced", project: project(name) }] };
}

/** ドキュメントに編集が1つ届いた状態にする (revisionが進む)。 */
function edit(revision: number) {
  useDocumentStore().applyPatch({ revision, ops: [{ op: "wire_parts_replaced", wire_parts: [] }] });
}

function memoryStorage(initial: string[] = []): RecentStorage & { saved: string[][] } {
  let list = initial;
  const saved: string[][] = [];
  return {
    saved,
    get: () => list,
    set: (next) => {
      list = next;
      saved.push(next);
    },
  };
}

describe("projectFile store: 開いているファイルと未保存の編集", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
    const doc = useDocumentStore();
    doc.project = project();
    doc.revision = 5;
  });

  // ja: 起動直後は読み込み済みの図面が「保存済み」の基準になり、未保存の編集は無い
  it("right after startup the loaded drawing counts as saved, with nothing unsaved", () => {
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    expect(file.dirty).toBe(false);
    expect(file.path).toBeNull();
    expect(file.fileName).toBeNull();
  });

  // ja: 図面を編集すると未保存の編集ありになる
  it("editing the drawing marks it as having unsaved changes", () => {
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    edit(6);
    expect(file.dirty).toBe(true);
  });

  // ja: undoで保存時と同じ内容に戻しても未保存の印は消えない (revisionは戻らない)
  it("undoing back to the saved content still counts as unsaved", async () => {
    vi.spyOn(ipc, "saveProject").mockResolvedValue();
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    await file.saveAs("/work/panel.mdkproj");
    expect(file.dirty).toBe(false);
    edit(6);
    expect(file.dirty).toBe(true);
    // undoも1つの編集としてrevisionを進めるので、内容が戻っても未保存のまま
    edit(7);
    expect(file.dirty).toBe(true);
  });

  // ja: ファイルを開くとそのパスが保存先になり、未保存の編集は無くなる
  it("opening a file makes its path the save target and clears unsaved changes", async () => {
    vi.spyOn(ipc, "loadProject").mockResolvedValue(replaced(10, "loaded"));
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    edit(6);
    await file.open("/work/panel.mdkproj");
    expect(ipc.loadProject).toHaveBeenCalledWith("/work/panel.mdkproj");
    expect(file.path).toBe("/work/panel.mdkproj");
    expect(file.fileName).toBe("panel.mdkproj");
    expect(file.dirty).toBe(false);
    expect(useDocumentStore().project?.name).toBe("loaded");
  });

  // ja: ファイルを開くとundo/redoの履歴は空になる (前の図面には戻れない)
  it("opening a file empties the undo/redo history", async () => {
    vi.spyOn(ipc, "loadProject").mockResolvedValue(replaced(10));
    const doc = useDocumentStore();
    doc.canUndo = true;
    doc.canRedo = true;
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    await file.open("/work/panel.mdkproj");
    expect(doc.canUndo).toBe(false);
    expect(doc.canRedo).toBe(false);
  });

  // ja: 保存先が決まっていれば「保存」はダイアログ無しで同じファイルへ上書きする
  it("save writes to the current file when one is known", async () => {
    vi.spyOn(ipc, "loadProject").mockResolvedValue(replaced(10));
    vi.spyOn(ipc, "saveProject").mockResolvedValue();
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    await file.open("/work/panel.mdkproj");
    edit(11);
    expect(await file.save()).toBe("/work/panel.mdkproj");
    expect(ipc.saveProject).toHaveBeenCalledWith("/work/panel.mdkproj");
    expect(file.dirty).toBe(false);
  });

  // ja: 保存先が無ければ「保存」は何も書かずnullを返す (名前を付けて保存へ回す)
  it("save does nothing and returns null when no file is known", async () => {
    vi.spyOn(ipc, "saveProject").mockResolvedValue();
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    expect(await file.save()).toBeNull();
    expect(ipc.saveProject).not.toHaveBeenCalled();
  });

  // ja: 「名前を付けて保存」は新しいパスへ書き、以後そのパスが保存先になる
  it("save-as writes to the new path, which becomes the save target", async () => {
    vi.spyOn(ipc, "saveProject").mockResolvedValue();
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    edit(6);
    await file.saveAs("/work/new.mdkproj");
    expect(ipc.saveProject).toHaveBeenCalledWith("/work/new.mdkproj");
    expect(file.path).toBe("/work/new.mdkproj");
    expect(file.dirty).toBe(false);
  });

  // ja: 保存中に届いた編集はディスクに無いので、保存後も未保存の編集ありのまま
  it("an edit that arrives while saving still counts as unsaved afterwards", async () => {
    vi.spyOn(ipc, "saveProject").mockImplementation(async () => {
      edit(7);
    });
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    edit(6);
    await file.saveAs("/work/new.mdkproj");
    expect(file.dirty).toBe(true);
  });

  // ja: 新規プロジェクトは保存先が無く、未保存の編集も無い状態から始まる
  it("a new project starts with no file and no unsaved changes", async () => {
    vi.spyOn(ipc, "newProject").mockResolvedValue(replaced(10, "Untitled"));
    vi.spyOn(ipc, "loadProject").mockResolvedValue(replaced(9));
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    await file.open("/work/panel.mdkproj");
    await file.create("Untitled");
    expect(ipc.newProject).toHaveBeenCalledWith("Untitled");
    expect(file.path).toBeNull();
    expect(file.dirty).toBe(false);
    expect(useDocumentStore().project?.name).toBe("Untitled");
  });

  // ja: KiCad回路図の読み込みは保存先が無く、内容は未保存の編集として扱う
  it("importing a KiCad schematic leaves no file and counts as unsaved", async () => {
    vi.spyOn(ipc, "importKicad").mockResolvedValue({
      patch: replaced(10, "kicad"),
      report: { symbols: 2, wires: 3, junctions: 0, labels: 1, texts: 0, skipped: [], warnings: [] },
    });
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    const result = await file.importKicad("/work/board.kicad_sch");
    expect(result.report.symbols).toBe(2);
    expect(file.path).toBeNull();
    expect(file.dirty).toBe(true);
    expect(file.recent[0]).toBe("/work/board.kicad_sch");
  });
  // ja: DXFの読み込みも保存先が無く、内容は未保存の編集として扱う
  it("importing a DXF drawing leaves no file and counts as unsaved", async () => {
    vi.spyOn(ipc, "importDxf").mockResolvedValue({
      patch: replaced(10, "dxf"),
      report: { symbols: 1, wires: 4, junctions: 0, labels: 0, texts: 0, skipped: ["HCR1 x1"], warnings: [] },
    });
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    const result = await file.importDxf("/work/panel.dxf");
    expect(ipc.importDxf).toHaveBeenCalledWith("/work/panel.dxf", undefined);
    expect(result.report.wires).toBe(4);
    expect(file.path).toBeNull();
    expect(file.dirty).toBe(true);
    expect(file.recent[0]).toBe("/work/panel.dxf");
  });
});

describe("projectFile store: 最近使ったファイル", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
    useDocumentStore().project = project();
  });

  // ja: 起動時に保管先から最近使ったファイルを読む
  it("recent files are read from storage at startup", () => {
    const file = useProjectFileStore();
    file.attach(memoryStorage(["/a.mdkproj", "/b.mdkproj"]));
    expect(file.recent).toEqual(["/a.mdkproj", "/b.mdkproj"]);
  });

  // ja: 開いたファイル・保存したファイルは一覧の先頭に入り、保管先へ書かれる
  it("opened and saved files go to the front of the list and are persisted", async () => {
    vi.spyOn(ipc, "loadProject").mockResolvedValue(replaced(10));
    vi.spyOn(ipc, "saveProject").mockResolvedValue();
    const storage = memoryStorage(["/old.mdkproj"]);
    const file = useProjectFileStore();
    file.attach(storage);
    await file.open("/a.mdkproj");
    await file.saveAs("/b.mdkproj");
    expect(file.recent).toEqual(["/b.mdkproj", "/a.mdkproj", "/old.mdkproj"]);
    expect(storage.saved[storage.saved.length - 1]).toEqual(file.recent);
  });

  // ja: 同じファイルを開き直すと重複せず先頭へ移る
  it("reopening a file moves it to the front instead of duplicating it", () => {
    const file = useProjectFileStore();
    file.attach(memoryStorage(["/a.mdkproj", "/b.mdkproj"]));
    file.addRecent("/b.mdkproj");
    expect(file.recent).toEqual(["/b.mdkproj", "/a.mdkproj"]);
  });

  // ja: 一覧は上限件数までで、古いものから落ちる
  it("the list is capped, dropping the oldest entries", () => {
    const file = useProjectFileStore();
    file.attach(memoryStorage());
    for (let i = 0; i < RECENT_LIMIT + 2; i++) file.addRecent(`/p${i}.mdkproj`);
    expect(file.recent).toHaveLength(RECENT_LIMIT);
    expect(file.recent[0]).toBe(`/p${RECENT_LIMIT + 1}.mdkproj`);
    expect(file.recent).not.toContain("/p0.mdkproj");
  });

  // ja: 一覧から外したファイルは保管先からも消える
  it("removing a file drops it from the list and from storage", () => {
    const storage = memoryStorage(["/a.mdkproj", "/b.mdkproj"]);
    const file = useProjectFileStore();
    file.attach(storage);
    file.removeRecent("/a.mdkproj");
    expect(file.recent).toEqual(["/b.mdkproj"]);
    expect(storage.saved[storage.saved.length - 1]).toEqual(["/b.mdkproj"]);
  });

  // ja: 保管データが重複していても起動時の一覧は重複なしになる
  it("duplicates in stored data are removed when the list is read at startup", () => {
    const file = useProjectFileStore();
    file.attach(memoryStorage(["/a.mdkproj", "/b.mdkproj", "/a.mdkproj"]));
    expect(file.recent).toEqual(["/a.mdkproj", "/b.mdkproj"]);
  });

  // ja: 保管データが上限を超えていても起動時の一覧は上限件数までになる
  it("stored data over the cap is trimmed when the list is read at startup", () => {
    const stored = Array.from({ length: RECENT_LIMIT + 3 }, (_, i) => `/p${i}.mdkproj`);
    const file = useProjectFileStore();
    file.attach(memoryStorage(stored));
    expect(file.recent).toHaveLength(RECENT_LIMIT);
    expect(file.recent[0]).toBe("/p0.mdkproj");
  });
});

/** localStorageのフェイク (値を保持し、書き込みを拒否させることもできる)。 */
function fakeLocalStorage(initial?: string, readonly = false) {
  const values = new Map<string, string>();
  if (initial !== undefined) values.set(RECENT_STORAGE_KEY, initial);
  return {
    values,
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      if (readonly) throw new Error("QuotaExceededError");
      values.set(key, value);
    },
  };
}

/** `globalThis.localStorage`を差し替えて`fn`を走らせ、必ず元に戻す。 */
function withLocalStorage<T>(stub: unknown, fn: () => T): T {
  const target = globalThis as unknown as { localStorage?: unknown };
  const had = "localStorage" in target;
  const previous = target.localStorage;
  Object.defineProperty(target, "localStorage", { value: stub, configurable: true, writable: true });
  try {
    return fn();
  } finally {
    if (had) {
      Object.defineProperty(target, "localStorage", {
        value: previous,
        configurable: true,
        writable: true,
      });
    } else {
      delete target.localStorage;
    }
  }
}

describe("projectFile store: 最近使ったファイルの保管 (localStorage)", () => {
  // ja: 書いた一覧はlocalStorageから読み戻せる (アプリを閉じても残る)
  it("a written list can be read back from localStorage", () => {
    const store = fakeLocalStorage();
    withLocalStorage(store, () => {
      const storage = localRecentStorage();
      storage.set(["/a.mdkproj", "/b.mdkproj"]);
      expect(localRecentStorage().get()).toEqual(["/a.mdkproj", "/b.mdkproj"]);
    });
    expect(store.values.get(RECENT_STORAGE_KEY)).toBe('["/a.mdkproj","/b.mdkproj"]');
  });

  // ja: 保管が空なら一覧も空から始まる
  it("an empty store starts with an empty list", () => {
    withLocalStorage(fakeLocalStorage(), () => {
      expect(localRecentStorage().get()).toEqual([]);
    });
  });

  // ja: 壊れたJSONが保管されていても空の一覧になる (起動を止めない)
  it("broken JSON in storage yields an empty list instead of failing startup", () => {
    withLocalStorage(fakeLocalStorage("{not json"), () => {
      expect(localRecentStorage().get()).toEqual([]);
    });
  });

  // ja: 配列でない値が保管されていても空の一覧になる
  it("a stored value that is not an array yields an empty list", () => {
    withLocalStorage(fakeLocalStorage('{"a":1}'), () => {
      expect(localRecentStorage().get()).toEqual([]);
    });
  });

  // ja: 文字列でない要素は一覧から落とす
  it("elements that are not strings are dropped from the list", () => {
    withLocalStorage(fakeLocalStorage('["/a.mdkproj",3,null,"/b.mdkproj"]'), () => {
      expect(localRecentStorage().get()).toEqual(["/a.mdkproj", "/b.mdkproj"]);
    });
  });

  // ja: 保管できない環境 (プライベートモード・容量超過) でも例外にせず、一覧はセッション内だけ生きる
  it("a store that refuses writes does not throw; the list only lives for the session", () => {
    withLocalStorage(fakeLocalStorage(undefined, true), () => {
      expect(() => localRecentStorage().set(["/a.mdkproj"])).not.toThrow();
      expect(localRecentStorage().get()).toEqual([]);
    });
  });

  // ja: localStorageが無い実行環境でも一覧は空として扱う
  it("a runtime without localStorage is treated as an empty list", () => {
    withLocalStorage(undefined, () => {
      const storage = localRecentStorage();
      expect(storage.get()).toEqual([]);
      expect(() => storage.set(["/a.mdkproj"])).not.toThrow();
    });
  });
});

describe("projectFile helpers", () => {
  // ja: パスからファイル名を取り出す (`/`区切りと`\`区切りの両方)
  it("fileNameOf returns the last path segment for both separators", () => {
    expect(fileNameOf("/work/panel.mdkproj")).toBe("panel.mdkproj");
    expect(fileNameOf("C:\\work\\panel.mdkproj")).toBe("panel.mdkproj");
    expect(fileNameOf("panel.mdkproj")).toBe("panel.mdkproj");
  });

  // ja: 拡張子が.dxf(大文字小文字を問わず)ならDXF図面とみなす
  it("isDxfPath recognises the .dxf extension regardless of case", () => {
    expect(isDxfPath("/a/b.dxf")).toBe(true);
    expect(isDxfPath("/a/B.DXF")).toBe(true);
    expect(isDxfPath("/a/b.dwg")).toBe(false);
  });

  // ja: 拡張子が.kicad_sch(大文字小文字を問わず)ならKiCad回路図とみなす
  it("isKicadPath recognises the .kicad_sch extension regardless of case", () => {
    expect(isKicadPath("/a/b.kicad_sch")).toBe(true);
    expect(isKicadPath("/a/B.KICAD_SCH")).toBe(true);
    expect(isKicadPath("/a/b.mdkproj")).toBe(false);
  });
});
