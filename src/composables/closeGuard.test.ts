// 閉じる / リロード時の未保存ガードの仕様テスト。
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
  browserCloseGuard,
  tauriCloseGuard,
  type BeforeUnloadEventLike,
  type CloseRequestWindow,
} from "./closeGuard";

/** Tauriのウィンドウのフェイク: 閉じる要求を任意に発火できる。 */
function fakeWindow() {
  let handler: ((event: { preventDefault(): void }) => Promise<void> | void) | null = null;
  let detached = false;
  const window_: CloseRequestWindow = {
    async onCloseRequested(h) {
      handler = h;
      return () => {
        detached = true;
      };
    },
  };
  return {
    window_,
    get detached() {
      return detached;
    },
    /** 閉じる要求を1回出し、閉じるのが止められたかを返す。 */
    async requestClose(): Promise<boolean> {
      let prevented = false;
      await handler?.({
        preventDefault() {
          prevented = true;
        },
      });
      return prevented;
    },
  };
}

/** ブラウザのフェイク: `beforeunload`の登録・解除・発火を見る。 */
function fakeUnloadHost() {
  const handlers = new Set<(event: BeforeUnloadEventLike) => void>();
  return {
    host: {
      addEventListener(_type: "beforeunload", h: (event: BeforeUnloadEventLike) => void) {
        handlers.add(h);
      },
      removeEventListener(_type: "beforeunload", h: (event: BeforeUnloadEventLike) => void) {
        handlers.delete(h);
      },
    },
    get count() {
      return handlers.size;
    },
    /** 離脱しようとして、確認が出たか (preventDefaultされたか) を返す。 */
    unload(): boolean {
      let prevented = false;
      const event: BeforeUnloadEventLike = {
        preventDefault() {
          prevented = true;
        },
        returnValue: undefined,
      };
      for (const h of handlers) h(event);
      return prevented;
    },
  };
}

describe("close guard: アプリ (Tauri)", () => {
  // ja: 未保存の編集を捨てないと答えると、ウィンドウは閉じない
  it("answering no to the discard confirmation keeps the window open", async () => {
    const tauri = fakeWindow();
    await tauriCloseGuard(async () => tauri.window_).install({
      dirty: () => true,
      confirmDiscard: async () => false,
    });
    expect(await tauri.requestClose()).toBe(true);
  });

  // ja: 未保存の編集を捨ててよいと答えると、ウィンドウはそのまま閉じる
  it("answering yes to the discard confirmation lets the window close", async () => {
    const tauri = fakeWindow();
    await tauriCloseGuard(async () => tauri.window_).install({
      dirty: () => true,
      confirmDiscard: async () => true,
    });
    expect(await tauri.requestClose()).toBe(false);
  });

  // ja: 未保存の編集が無ければ確認は出ず、すぐ閉じる
  it("with nothing unsaved the window closes without asking", async () => {
    const tauri = fakeWindow();
    const confirmDiscard = vi.fn(async () => true);
    await tauriCloseGuard(async () => tauri.window_).install({
      dirty: () => false,
      confirmDiscard,
    });
    expect(await tauri.requestClose()).toBe(false);
    // 未保存かどうかの判定は確認側 (fileActions) が持つので、確認は毎回呼ばれてtrueを返す
    expect(confirmDiscard).toHaveBeenCalled();
  });

  // ja: ガードを取り外すと閉じる要求に割り込まなくなる
  it("detaching the guard stops it from intercepting close requests", async () => {
    const tauri = fakeWindow();
    const detach = await tauriCloseGuard(async () => tauri.window_).install({
      dirty: () => true,
      confirmDiscard: async () => false,
    });
    detach();
    expect(tauri.detached).toBe(true);
  });
});

describe("close guard: ブラウザ検証", () => {
  // ja: ブラウザ検証では未保存の編集があるときだけ離脱確認を出す
  it("in a plain browser the leave confirmation appears only when something is unsaved", () => {
    const unsaved = fakeUnloadHost();
    void browserCloseGuard(unsaved.host).install({
      dirty: () => true,
      confirmDiscard: async () => true,
    });
    expect(unsaved.unload()).toBe(true);

    const clean = fakeUnloadHost();
    void browserCloseGuard(clean.host).install({
      dirty: () => false,
      confirmDiscard: async () => true,
    });
    expect(clean.unload()).toBe(false);
  });

  // ja: ブラウザ検証でもガードを取り外すと離脱確認は出なくなる
  it("detaching the guard removes the browser leave confirmation", async () => {
    const browser = fakeUnloadHost();
    const detach = await browserCloseGuard(browser.host).install({
      dirty: () => true,
      confirmDiscard: async () => true,
    });
    expect(browser.count).toBe(1);
    detach();
    expect(browser.count).toBe(0);
    expect(browser.unload()).toBe(false);
  });
});
