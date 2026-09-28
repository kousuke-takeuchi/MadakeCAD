// ウィンドウを閉じる / webviewをリロードするときの未保存ガード。
//
// 新規・開く・読み込みの前には確認が入るのに、ウィンドウを閉じる (⌘Q・×) と未保存の編集が
// 無言で消えていた。Tauriでは`onCloseRequested`を取り、確認で「いいえ」なら閉じるのを止める。
// Tauri外 (localhost:1420 のブラウザ検証) では`beforeunload`でブラウザ標準の離脱確認に任せる
// (ブラウザは文面を差し替えさせないので、ここでは「未保存かどうか」だけを伝える)。

import { inTauri } from "../ipc";

/** 閉じる要求に割り込む口。`install`の戻り値は取り外し関数。 */
export interface CloseGuard {
  install(hooks: {
    /** 未保存の編集があるか。 */
    dirty: () => boolean;
    /** 捨てて良いかの確認 (未保存でなければtrueを返す)。 */
    confirmDiscard: () => Promise<boolean>;
  }): Promise<() => void>;
}

/** Tauriのウィンドウのうちこの層が使う部分だけ。 */
export interface CloseRequestWindow {
  onCloseRequested(
    handler: (event: { preventDefault(): void }) => Promise<void> | void,
  ): Promise<() => void>;
}

/**
 * Tauriのウィンドウに取り付けるガード。
 * 未保存なら確認ダイアログを出し、「いいえ」なら`preventDefault()`で閉じるのを止める。
 */
export function tauriCloseGuard(getWindow: () => Promise<CloseRequestWindow>): CloseGuard {
  return {
    async install({ confirmDiscard }) {
      const window_ = await getWindow();
      return window_.onCloseRequested(async (event) => {
        if (!(await confirmDiscard())) event.preventDefault();
      });
    },
  };
}

/** `beforeunload`を購読できる相手 (ブラウザのwindow)。 */
export interface UnloadHost {
  addEventListener?(type: "beforeunload", handler: (event: BeforeUnloadEventLike) => void): void;
  removeEventListener?(type: "beforeunload", handler: (event: BeforeUnloadEventLike) => void): void;
}

/** `beforeunload`のうち離脱確認を出すために触る部分だけ。 */
export interface BeforeUnloadEventLike {
  preventDefault(): void;
  returnValue: unknown;
}

/**
 * ブラウザ検証時のガード。未保存なら`beforeunload`で離脱確認を出す
 * (文面はブラウザが決めるので、ここでは出す・出さないだけを決める)。
 */
export function browserCloseGuard(host: UnloadHost): CloseGuard {
  return {
    async install({ dirty }) {
      const handler = (event: BeforeUnloadEventLike) => {
        if (!dirty()) return;
        event.preventDefault();
        event.returnValue = "";
      };
      host.addEventListener?.("beforeunload", handler);
      return () => host.removeEventListener?.("beforeunload", handler);
    },
  };
}

/** アプリが使う実体。 */
export const closeGuard: CloseGuard = inTauri
  ? tauriCloseGuard(async () => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      return getCurrentWindow();
    })
  : browserCloseGuard(globalThis as UnloadHost);
