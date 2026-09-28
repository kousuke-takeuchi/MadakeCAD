// ネイティブダイアログ層の仕様テスト。
// Tauriのwebviewは`window.confirm`/`prompt`を実装しないので、アプリ本体はdialogプラグイン、
// Tauri外のブラウザ検証では`window.*`という2つの実体を使い分ける。両方をここで固定する。
import { describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  ask: vi.fn(),
  message: vi.fn(),
  open: vi.fn(),
  save: vi.fn(),
}));
import {
  browserDialogs,
  pluginDialogs,
  type BrowserDialogHost,
  type DialogPlugin,
} from "./nativeDialogs";

/** dialogプラグインのフェイク (呼ばれ方と引数を記録する)。 */
function fakePlugin(answers: {
  ask?: boolean;
  save?: string | null;
  open?: string | string[] | null;
}): DialogPlugin {
  return {
    ask: vi.fn(async () => answers.ask ?? false),
    message: vi.fn(async () => undefined),
    save: vi.fn(async () => answers.save ?? null),
    open: vi.fn(async () => answers.open ?? null),
  };
}

/** ブラウザのダイアログのフェイク。 */
function fakeHost(answers: { confirm?: boolean; prompt?: string | null }): BrowserDialogHost & {
  alerted: string[];
} {
  const alerted: string[] = [];
  return {
    alerted,
    confirm: vi.fn(() => answers.confirm ?? false),
    prompt: vi.fn(() => answers.prompt ?? null),
    alert: (text?: string) => alerted.push(text ?? ""),
  };
}

const filters = [{ name: "MadakeCAD project", extensions: ["mdkproj"] }];

describe("native dialogs: Tauriのdialogプラグイン", () => {
  // ja: アプリ本体の確認ダイアログはプラグインの`ask`を「警告」として出し、「はい」でtrueになる
  it("a confirmation goes through the plugin's ask as a warning and yes means true", async () => {
    const plugin = fakePlugin({ ask: true });
    expect(await pluginDialogs(plugin).confirm("discard?")).toBe(true);
    expect(plugin.ask).toHaveBeenCalledWith("discard?", { kind: "warning" });
  });

  // ja: 確認ダイアログで「いいえ」を選ぶとfalseになり、続行しない
  it("choosing no in a confirmation returns false", async () => {
    expect(await pluginDialogs(fakePlugin({ ask: false })).confirm("discard?")).toBe(false);
  });

  // ja: 確認ダイアログのタイトルと重大度は呼び出し側で上書きできる
  it("the confirmation title and kind can be overridden by the caller", async () => {
    const plugin = fakePlugin({ ask: true });
    await pluginDialogs(plugin).confirm("gone?", { title: "Unsaved changes", kind: "error" });
    expect(plugin.ask).toHaveBeenCalledWith("gone?", {
      title: "Unsaved changes",
      kind: "error",
    });
  });

  // ja: 通知はプラグインの`message`で出す (押せるのはOKだけ)
  it("a notice goes through the plugin's message dialog", async () => {
    const plugin = fakePlugin({});
    await pluginDialogs(plugin).notify("exported");
    expect(plugin.message).toHaveBeenCalledWith("exported", { kind: "info" });
  });

  // ja: 保存先はプラグインの保存ダイアログで選び、既定のファイル名とフィルタを渡す
  it("a save target is chosen with the plugin's save dialog, given a default name and filters", async () => {
    const plugin = fakePlugin({ save: "/work/panel.mdkproj" });
    expect(await pluginDialogs(plugin).save("panel.mdkproj", filters, "prompt")).toBe(
      "/work/panel.mdkproj",
    );
    expect(plugin.save).toHaveBeenCalledWith({ defaultPath: "panel.mdkproj", filters });
  });

  // ja: 開くファイルは1つだけ選べるダイアログで選ぶ
  it("a file to open is chosen with a single-selection dialog", async () => {
    const plugin = fakePlugin({ open: "/work/panel.mdkproj" });
    expect(await pluginDialogs(plugin).open(filters, "prompt")).toBe("/work/panel.mdkproj");
    expect(plugin.open).toHaveBeenCalledWith({ multiple: false, filters });
  });

  // ja: 開くダイアログをキャンセルするとnullになる (複数選択の結果もnull扱い)
  it("cancelling the open dialog yields null, and so does a multi-selection result", async () => {
    expect(await pluginDialogs(fakePlugin({ open: null })).open(filters, "p")).toBeNull();
    expect(await pluginDialogs(fakePlugin({ open: ["/a", "/b"] })).open(filters, "p")).toBeNull();
  });
});

describe("native dialogs: Tauri外 (ブラウザ検証) のフォールバック", () => {
  // ja: ブラウザ検証では確認は`window.confirm`で出す
  it("in a plain browser a confirmation uses window.confirm", async () => {
    const host = fakeHost({ confirm: true });
    expect(await browserDialogs(host).confirm("discard?")).toBe(true);
    expect(host.confirm).toHaveBeenCalledWith("discard?");
  });

  // ja: ブラウザ検証では通知は`window.alert`で出す
  it("in a plain browser a notice uses window.alert", async () => {
    const host = fakeHost({});
    await browserDialogs(host).notify("exported");
    expect(host.alerted).toEqual(["exported"]);
  });

  // ja: ブラウザ検証ではネイティブのファイルダイアログが無いので、保存先はファイル名を既定にして入力してもらう
  it("without a native file dialog the save target is typed in, defaulting to the file name", async () => {
    const host = fakeHost({ prompt: "/work/panel.mdkproj" });
    expect(await browserDialogs(host).save("panel.mdkproj", filters, "Save to:")).toBe(
      "/work/panel.mdkproj",
    );
    expect(host.prompt).toHaveBeenCalledWith("Save to:", "panel.mdkproj");
  });

  // ja: ブラウザ検証で開くパスも入力してもらい、キャンセルするとnullになる
  it("the path to open is typed in too, and cancelling yields null", async () => {
    const host = fakeHost({ prompt: null });
    expect(await browserDialogs(host).open(filters, "Open:")).toBeNull();
    expect(host.prompt).toHaveBeenCalledWith("Open:");
  });

  // ja: ダイアログを持たない実行環境 (テストランナー等) では確認はfalse・入力はnullになり、例外は出さない
  it("a host without dialogs answers no and null instead of throwing", async () => {
    const dialogs = browserDialogs({});
    expect(await dialogs.confirm("x")).toBe(false);
    expect(await dialogs.save("a.mdkproj", filters, "p")).toBeNull();
    expect(await dialogs.open(filters, "p")).toBeNull();
    await dialogs.notify("x");
  });
});
