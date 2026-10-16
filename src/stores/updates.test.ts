// アプリ内アップデートの状態ストアの仕様テスト (.pen「自動更新デザイン」`qyYGO`)。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn(async () => "0.2.0") }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn(async () => undefined) }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { nativeDialogs } from "../composables/nativeDialogs";
import { ipc } from "../ipc";
import { useProjectFileStore } from "./projectFile";
import { useUpdatesStore } from "./updates";

type Events = (e: { event: string; data?: Record<string, number> }) => void;

/** プラグインの`Update`に似せた更新 (ダウンロードは20バイトを2回で受け取る)。 */
function fakeUpdate(version = "0.3.0") {
  return {
    version,
    date: "2026-10-20",
    body: "・図枠エディタ",
    close: vi.fn(async () => undefined),
    downloadAndInstall: vi.fn(async (on: Events) => {
      on({ event: "Started", data: { contentLength: 20 } });
      on({ event: "Progress", data: { chunkLength: 8 } });
      on({ event: "Progress", data: { chunkLength: 12 } });
      on({ event: "Finished" });
    }),
  };
}

describe("updates store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    vi.spyOn(ipc, "updateSupport").mockResolvedValue("supported");
  });

  // ja: 起動時の確認で新しい版が見つかると、その版と公開日・リリースノートを持って右下の通知を出す
  it("shows the notice with the version, date and notes when the startup check finds an update", async () => {
    vi.mocked(check).mockResolvedValue(fakeUpdate() as never);
    const store = useUpdatesStore();
    await store.startupCheck(true);
    expect(store.available).toEqual({ version: "0.3.0", date: "2026-10-20", notes: "・図枠エディタ" });
    expect(store.showNotice).toBe(true);
  });

  // ja: 設定で起動時の確認をオフにしていれば確認しない。開発ビルドとブラウザ検証モードでも確認しない
  it("does not check at startup when turned off, in a development build or in browser mode", async () => {
    await useUpdatesStore().startupCheck(false);
    setActivePinia(createPinia());
    vi.spyOn(ipc, "updateSupport").mockResolvedValue("dev");
    await useUpdatesStore().startupCheck(true);
    setActivePinia(createPinia());
    vi.spyOn(ipc, "updateSupport").mockResolvedValue("unsupported");
    await useUpdatesStore().startupCheck(true);
    expect(check).not.toHaveBeenCalled();
  });

  // ja: 起動時の確認が通信できずに失敗しても、何も表示しない
  it("stays silent when the startup check cannot reach the server", async () => {
    vi.mocked(check).mockRejectedValue(new Error("offline"));
    const store = useUpdatesStore();
    await store.startupCheck(true);
    expect(store.status).toBe("idle");
    expect(store.error).toBeNull();
    expect(store.showNotice).toBe(false);
  });

  // ja: 「今すぐ確認」は、最新なら「最新です」、失敗なら理由を出す
  it("reports up to date or the reason of a failure when checked by hand", async () => {
    vi.mocked(check).mockResolvedValue(null);
    const store = useUpdatesStore();
    expect(await store.check()).toBe(false);
    expect(store.status).toBe("upToDate");
    expect(store.lastChecked).not.toBeNull();
    vi.mocked(check).mockRejectedValue(new Error("signature"));
    await store.check();
    expect(store.status).toBe("error");
    expect(store.error).toContain("signature");
  });

  // ja: 「更新して再起動」はダウンロードの進み具合を数え、入れ替えが済んだらアプリを再起動する
  it("counts the download progress and relaunches once the update is installed", async () => {
    const update = fakeUpdate();
    vi.mocked(check).mockResolvedValue(update as never);
    const store = useUpdatesStore();
    await store.check();
    expect(await store.install("unsaved?")).toBe(true);
    expect(store.downloaded).toBe(20);
    expect(store.progress).toBe(1);
    expect(store.status).toBe("installing");
    expect(relaunch).toHaveBeenCalled();
  });

  // ja: 未保存の図面があるときは先に確認し、断られたらダウンロードしない
  it("asks first when the drawing has unsaved changes and does nothing if refused", async () => {
    const update = fakeUpdate();
    vi.mocked(check).mockResolvedValue(update as never);
    vi.spyOn(useProjectFileStore(), "dirty", "get").mockReturnValue(true);
    const confirm = vi.spyOn(nativeDialogs, "confirm").mockResolvedValue(false);
    const store = useUpdatesStore();
    await store.check();
    expect(await store.install("未保存の変更は失われます")).toBe(false);
    expect(confirm).toHaveBeenCalledWith("未保存の変更は失われます", { kind: "warning" });
    expect(update.downloadAndInstall).not.toHaveBeenCalled();
  });

  // ja: ダウンロードや入れ替えに失敗したら理由を出し、再起動しない
  it("shows the reason and does not relaunch when the download or install fails", async () => {
    const update = fakeUpdate();
    update.downloadAndInstall.mockRejectedValue(new Error("bad signature"));
    vi.mocked(check).mockResolvedValue(update as never);
    const store = useUpdatesStore();
    await store.check();
    expect(await store.install("x")).toBe(false);
    expect(store.status).toBe("error");
    expect(store.error).toContain("bad signature");
    expect(relaunch).not.toHaveBeenCalled();
  });

  // ja: 「後で」を押した版は、この起動中は通知を出さない
  it("hides the notice for the dismissed version for the rest of the session", async () => {
    vi.mocked(check).mockResolvedValue(fakeUpdate() as never);
    const store = useUpdatesStore();
    await store.startupCheck(true);
    store.dismiss();
    expect(store.showNotice).toBe(false);
  });

  // ja: Linuxの.deb/.rpmでは入れ替えず、リリースページを開くだけにする
  it("only opens the release page for a Linux .deb or .rpm install", async () => {
    vi.spyOn(ipc, "updateSupport").mockResolvedValue("manual");
    const update = fakeUpdate();
    vi.mocked(check).mockResolvedValue(update as never);
    const open = vi.fn();
    vi.stubGlobal("window", { open });
    const store = useUpdatesStore();
    await store.check();
    expect(await store.install("x")).toBe(false);
    expect(update.downloadAndInstall).not.toHaveBeenCalled();
    expect(open).toHaveBeenCalled();
    vi.unstubAllGlobals();
  });
});
