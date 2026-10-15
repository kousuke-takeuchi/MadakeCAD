// 図枠エディタの下書きストア (M4残り⑥) の仕様テスト。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { addLine } from "../canvas/frameEditor";
import { DEFAULT_FRAME_TEMPLATE, type FrameTemplate } from "../canvas/frameTemplate";
import { ipc } from "../ipc";
import { useDocumentStore } from "./document";
import { FRAME_UNDO_LIMIT, useFrameEditorStore } from "./frameEditor";

const builtin = (): FrameTemplate => ({ ...JSON.parse(JSON.stringify(DEFAULT_FRAME_TEMPLATE)), builtin: true });
const user = (): FrameTemplate => ({ ...builtin(), id: "mine", name_ja: "自社様式", builtin: false });

describe("frame editor store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
    useDocumentStore().frameTemplates = [builtin(), user()];
    vi.spyOn(ipc, "listFrameTemplates").mockResolvedValue({ templates: [builtin(), user()], issues: [] });
  });

  // ja: 同梱の図枠を開くと、-copyのidを付けた新しい下書きになる (同梱はそのまま上書きできない)
  it("opens a bundled frame as a new draft with a -copy id", () => {
    const store = useFrameEditorStore();
    store.openFor(builtin(), "edit");
    expect(store.draft?.id).toBe("jis-a3-copy");
    expect(store.isNew).toBe(true);
    expect(store.dirty).toBe(false);
  });

  // ja: エディタ内の編集は図面のコマンドを送らず、⌘Zで1段ずつ戻り⇧⌘Zでやり直せる
  it("undoes and redoes editor edits step by step without sending any drawing command", () => {
    const execute = vi.spyOn(useDocumentStore(), "execute");
    const store = useFrameEditorStore();
    store.openFor(user(), "edit");
    const one = addLine(store.draft!, { x: 300, y: 260 }, { x: 310, y: 260 });
    store.commit(one);
    store.commit(addLine(one, { x: 300, y: 262 }, { x: 310, y: 262 }));
    expect(store.dirty).toBe(true);
    store.undo();
    expect(store.draft).toEqual(one);
    store.undo();
    expect(store.dirty).toBe(false);
    store.redo();
    expect(store.draft).toEqual(one);
    expect(execute).not.toHaveBeenCalled();
  });

  // ja: 何も変えない編集は段を積まず、新しい編集はやり直しの段を捨てる。段数には上限がある
  it("skips no-op edits, drops the redo steps on a new edit and caps the history", () => {
    const store = useFrameEditorStore();
    store.openFor(user(), "edit");
    store.commit(store.draft!);
    expect(store.canUndo).toBe(false);
    store.commit(addLine(store.draft!, { x: 300, y: 260 }, { x: 310, y: 260 }));
    store.undo();
    store.commit(addLine(store.draft!, { x: 300, y: 270 }, { x: 310, y: 270 }));
    expect(store.canRedo).toBe(false);
    for (let i = 0; i < FRAME_UNDO_LIMIT + 5; i++) store.commit(addLine(store.draft!, { x: 300, y: 260 }, { x: 301 + i, y: 260 }));
    expect(store.past).toHaveLength(FRAME_UNDO_LIMIT);
  });

  // ja: ユーザー図枠の保存は同じidで書き、一覧を読み直して未保存の印を消す
  it("saves a user frame under its own id, reloads the list and clears the unsaved mark", async () => {
    const save = vi.spyOn(ipc, "saveFrameTemplate").mockImplementation(async (t) => ({ template: t, path: "/x/mine.json" }));
    const store = useFrameEditorStore();
    store.openFor(user(), "edit");
    store.commit(addLine(store.draft!, { x: 300, y: 260 }, { x: 310, y: 260 }));
    expect(await store.save()).toBe(true);
    expect(save.mock.calls[0][0].id).toBe("mine");
    expect(store.dirty).toBe(false);
    expect(ipc.listFrameTemplates).toHaveBeenCalled();
  });

  // ja: まだファイルの無い下書きの保存は、書かずに別名で保存の欄を開く
  it("opens save-as instead of writing a draft that has no file yet", async () => {
    const save = vi.spyOn(ipc, "saveFrameTemplate");
    const store = useFrameEditorStore();
    store.openFor(builtin(), "copy");
    expect(await store.save()).toBe(false);
    expect(store.saveAsOpen).toBe(true);
    expect(store.saveAsId).toBe("jis-a3-copy");
    expect(save).not.toHaveBeenCalled();
  });

  // ja: 別名で保存は、ファイル名にできないidと既にあるidを断り、通れば新しいidと入力した名前で書く
  it("refuses an unsafe or taken id on save-as and otherwise writes under the new id and the entered name", async () => {
    const save = vi.spyOn(ipc, "saveFrameTemplate").mockImplementation(async (t) => ({ template: t, path: "" }));
    const store = useFrameEditorStore();
    store.openFor(builtin(), "copy");
    store.openSaveAs();
    store.saveAsId = "../x";
    expect(await store.saveAs()).toBe(false);
    store.saveAsId = "mine";
    expect(await store.saveAs()).toBe(false);
    expect(store.error).toBe("taken");
    store.saveAsId = "custom-2026";
    store.saveAsName = "自社様式 2026";
    expect(await store.saveAs()).toBe(true);
    expect(save.mock.calls[0][0]).toMatchObject({ id: "custom-2026", name_ja: "自社様式 2026", name: "自社様式 2026" });
    expect(store.isNew).toBe(false);
  });

  // ja: 保存前チェックに誤りがあれば保存しない
  it("does not save while the pre-save check reports an error", async () => {
    const save = vi.spyOn(ipc, "saveFrameTemplate");
    const store = useFrameEditorStore();
    store.openFor(user(), "edit");
    const broken = { ...store.draft!, revision_block: { rows_max: 6, col_w: [1, 1, 1, 1] } };
    store.commit(broken);
    expect(await store.save()).toBe(false);
    expect(save).not.toHaveBeenCalled();
  });

  // ja: 未保存の変更があるときに閉じると確認を出し、破棄を選べば閉じる
  it("asks before discarding unsaved changes and closes when told to discard", () => {
    const store = useFrameEditorStore();
    store.openFor(user(), "edit");
    store.commit(addLine(store.draft!, { x: 300, y: 260 }, { x: 310, y: 260 }));
    store.requestClose();
    expect(store.open).toBe(true);
    expect(store.confirmClose).toBe(true);
    store.requestClose(true);
    expect(store.open).toBe(false);
  });
});
