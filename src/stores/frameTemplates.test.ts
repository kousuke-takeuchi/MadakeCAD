// 図枠テンプレート選択ダイアログの仕様テスト
// (M4仕様 §9、.pen「M4デザイン - 図枠テンプレート」)。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { DEFAULT_FRAME_TEMPLATE } from "../canvas/frameTemplate";
import { ipc, type Command, type FrameTemplate, type Project, type Sheet } from "../ipc";
import { useDocumentStore } from "./document";
import { useFrameTemplatesStore } from "./frameTemplates";
import { defaultSettings, settingsApi, type AppSettings } from "./settings";

function frameTemplate(id: string, overrides: Partial<FrameTemplate> = {}): FrameTemplate {
  return { ...DEFAULT_FRAME_TEMPLATE, id, name: id, name_ja: id, builtin: true, ...overrides };
}

const a3 = frameTemplate("jis-a3", { name: "JIS A3 landscape", name_ja: "JIS A3 横" });
const a4 = frameTemplate("jis-a4-portrait", { name: "JIS A4 portrait", name_ja: "JIS A4 縦" });
const shop = frameTemplate("shop-a3", { name: "Shop frame", name_ja: "", builtin: false });

function testProject(frame: string | null = null): Project {
  const sheet: Sheet = {
    id: "s1",
    name: "Sheet1",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: {},
  };
  return {
    format_version: 3,
    name: "t",
    wire_parts: [],
    plc_assignments: [],
    sheets: [sheet],
    frame_template: frame,
  };
}

/** 図枠テンプレートの一覧IPCと、開いている図面を用意する。 */
function setup(frame: string | null = null, templates: FrameTemplate[] = [a3, a4, shop]) {
  vi.spyOn(ipc, "listFrameTemplates").mockResolvedValue({
    templates,
    issues: [],
    user_dir: "/Users/me/MadakeCAD/templates/frames",
  });
  const doc = useDocumentStore();
  doc.project = testProject(frame);
  doc.activeSheetId = "s1";
  return doc;
}

/** ダイアログが送ったコマンドを記録する。 */
function spyCommands(): Command[] {
  const sent: Command[] = [];
  vi.spyOn(ipc, "executeCommand").mockImplementation(async (command) => {
    sent.push(command);
    return { revision: 1, ops: [] };
  });
  return sent;
}

/** 保存された設定を記録する。 */
function spySettings(): AppSettings[] {
  const saved: AppSettings[] = [];
  vi.spyOn(settingsApi, "get").mockResolvedValue(defaultSettings());
  vi.spyOn(settingsApi, "set").mockImplementation(async (settings) => {
    saved.push(settings);
    return settings;
  });
  return saved;
}

describe("frame template dialog store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: ダイアログを開くと同梱とユーザーの図枠が一覧され、いま図面が使っている図枠が選ばれた状態で出る
  it("opens with the bundled and user frames listed and the project's current frame selected", async () => {
    setup("jis-a4-portrait");
    const store = useFrameTemplatesStore();
    await store.openDialog();
    expect(store.open).toBe(true);
    expect(store.rows.map((r) => r.template.id)).toEqual(["jis-a3", "jis-a4-portrait", "shop-a3"]);
    expect(store.selectedId).toBe("jis-a4-portrait");
    expect(store.selected!.id).toBe("jis-a4-portrait");
  });

  // ja: 図枠が未指定の図面では、実際に描かれている同梱のjis-a3が選ばれた状態で開く
  it("selects the bundled JIS A3 frame for a project that has never chosen one", async () => {
    setup(null);
    const store = useFrameTemplatesStore();
    await store.openDialog();
    expect(store.selectedId).toBe("jis-a3");
    expect(store.rows.find((r) => r.current)!.template.id).toBe("jis-a3");
  });

  // ja: 一覧の各行は同梱かユーザーかの区別を持つ(どこから来た図枠かが分かる)
  it("marks each row as a bundled or a user frame", async () => {
    setup();
    const store = useFrameTemplatesStore();
    await store.openDialog();
    expect(store.rows.map((r) => r.source)).toEqual(["builtin", "builtin", "user"]);
  });

  // ja: 行を選ぶとプレビューと情報行がその図枠の内容に切り替わる
  it("switches the preview and the summary line to the frame that was clicked", async () => {
    setup();
    const store = useFrameTemplatesStore();
    await store.openDialog();
    store.select("shop-a3");
    expect(store.selected!.id).toBe("shop-a3");
    expect(store.summary).toEqual({
      paper: "420×297",
      zones: "4×6",
      titleBlock: "120×32",
      revisionRows: 6,
    });
  });

  // ja: 適用範囲の既定は「このプロジェクト」で、適用すると set_frame_template を1回だけ実行する
  it("applies to this project with a single set_frame_template command", async () => {
    setup(null);
    const sent = spyCommands();
    const store = useFrameTemplatesStore();
    await store.openDialog();
    expect(store.scope).toBe("project");
    store.select("jis-a4-portrait");
    const applied = await store.apply();
    expect(sent).toEqual([{ type: "set_frame_template", frame_template: "jis-a4-portrait" }]);
    expect(applied).toEqual({ template: a4, changed: true });
  });

  // ja: いま使っている図枠をそのまま適用してもコマンドは送らない(空のundo段を積まない)
  it("sends nothing when the frame that is already in use is applied again", async () => {
    setup("jis-a4-portrait");
    const sent = spyCommands();
    const store = useFrameTemplatesStore();
    await store.openDialog();
    const applied = await store.apply();
    expect(sent).toEqual([]);
    expect(applied).toEqual({ template: a4, changed: false });
  });

  // ja: 適用しても一覧の「使用中」は選んだ図枠へ移り、続けて適用を押しても二重にコマンドは飛ばない
  it("moves the in-use marker to the applied frame so a second click sends nothing", async () => {
    const doc = setup(null);
    const sent = spyCommands();
    const store = useFrameTemplatesStore();
    await store.openDialog();
    store.select("jis-a4-portrait");
    await store.apply();
    // 実行したコマンドのpatchが図面ミラーへ届く (バックエンドからの通知)
    doc.applyPatch({
      revision: 2,
      ops: [{ op: "frame_template_changed", frame_template: "jis-a4-portrait" }],
    });
    expect(store.rows.find((r) => r.current)!.template.id).toBe("jis-a4-portrait");
    await store.apply();
    expect(sent).toHaveLength(1);
  });

  // ja: 適用範囲を「グローバル既定」にすると図面は編集せず、新規プロジェクトの既定として設定に保存する
  it("saves the frame as the default for new projects instead of editing the drawing", async () => {
    setup(null);
    const sent = spyCommands();
    const saved = spySettings();
    const store = useFrameTemplatesStore();
    await store.openDialog();
    store.select("jis-a4-portrait");
    store.setScope("global");
    await store.apply();
    expect(sent).toEqual([]);
    expect(saved.map((s) => s.default_frame_template)).toEqual(["jis-a4-portrait"]);
  });

  // ja: 図面の図枠を替えたあとに適用範囲を切り替えても、それぞれの適用は1回ずつしか起きない
  it("keeps the project edit and the global default apart when both are used in one visit", async () => {
    setup(null);
    const sent = spyCommands();
    const saved = spySettings();
    const store = useFrameTemplatesStore();
    await store.openDialog();
    store.select("jis-a4-portrait");
    await store.apply();
    store.setScope("global");
    await store.apply();
    expect(sent).toHaveLength(1);
    expect(saved).toHaveLength(1);
  });

  // ja: OKは適用してからダイアログを閉じる
  it("OK applies and then closes the dialog", async () => {
    setup(null);
    const sent = spyCommands();
    const store = useFrameTemplatesStore();
    await store.openDialog();
    store.select("jis-a4-portrait");
    await store.ok();
    expect(sent).toHaveLength(1);
    expect(store.open).toBe(false);
  });

  // ja: 適用は閉じずに反映する(続けて別の図枠を試せる)
  it("Apply keeps the dialog open so another frame can be tried", async () => {
    setup(null);
    spyCommands();
    const store = useFrameTemplatesStore();
    await store.openDialog();
    store.select("jis-a4-portrait");
    await store.apply();
    expect(store.open).toBe(true);
  });

  // ja: キャンセルは何も適用せずに閉じる
  it("Cancel closes without applying anything", async () => {
    setup(null);
    const sent = spyCommands();
    const saved = spySettings();
    const store = useFrameTemplatesStore();
    await store.openDialog();
    store.select("jis-a4-portrait");
    store.cancel();
    expect(store.open).toBe(false);
    expect(sent).toEqual([]);
    expect(saved).toEqual([]);
  });

  // ja: 適用が失敗したらダイアログは開いたままで理由を表示する
  it("stays open and shows the reason when applying fails", async () => {
    setup(null);
    vi.spyOn(ipc, "executeCommand").mockRejectedValue(new Error("engine busy"));
    const store = useFrameTemplatesStore();
    await store.openDialog();
    store.select("jis-a4-portrait");
    expect(await store.apply()).toBeNull();
    expect(store.open).toBe(true);
    expect(store.error).toContain("engine busy");
  });

  // ja: 読み込めなかった図枠JSONは一覧の代わりに理由として並べる(壊れたファイルで一覧が空にならない)
  it("reports unreadable frame JSON files instead of dropping the list", async () => {
    vi.spyOn(ipc, "listFrameTemplates").mockResolvedValue({
      templates: [a3],
      issues: [{ path: "/Users/me/MadakeCAD/templates/frames/broken.json", message: "EOF" }],
      user_dir: "/Users/me/MadakeCAD/templates/frames",
    });
    const doc = useDocumentStore();
    doc.project = testProject();
    const store = useFrameTemplatesStore();
    await store.openDialog();
    expect(store.rows).toHaveLength(1);
    expect(store.issues[0].message).toBe("EOF");
  });

  // ja: 「+ JSONを読み込み…」はユーザー図枠の置き場を開き、戻ってきたときに一覧を読み直す
  it("opens the user frame folder and reloads the list afterwards", async () => {
    setup();
    const open = vi.spyOn(ipc, "openFrameTemplatesFolder").mockResolvedValue("/Users/me/frames");
    const store = useFrameTemplatesStore();
    await store.openDialog();
    expect(await store.openUserFolder()).toEqual({ path: "/Users/me/frames", opened: true });
    expect(open).toHaveBeenCalledOnce();
    expect(ipc.listFrameTemplates).toHaveBeenCalledTimes(2);
  });

  // ja: フォルダを開けない環境(ブラウザ検証)では置き場のパスだけを案内する
  it("just names the folder when it cannot be opened (browser verification mode)", async () => {
    setup();
    vi.spyOn(ipc, "openFrameTemplatesFolder").mockRejectedValue(new Error("not supported"));
    const store = useFrameTemplatesStore();
    await store.openDialog();
    expect(await store.openUserFolder()).toEqual({
      path: "/Users/me/MadakeCAD/templates/frames",
      opened: false,
    });
  });
});
