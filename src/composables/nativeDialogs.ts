// ネイティブなダイアログ (確認・通知・ファイル選択) の1枚の層。
//
// Tauriのwebview (wry) は `window.confirm` / `window.prompt` / `window.alert` を実装して
// いないため、アプリの中でそれらを呼ぶと**ダイアログが出ないまま既定値が返る**
// (confirm=false / prompt=null)。つまり「未保存の編集を捨てますか」は常に「捨てない」に
// なり、確認を挟む導線が無言で何もしなくなる。
//
// そこでアプリの導線はすべてこの層を通す。Tauriの中ではdialogプラグイン
// (`ask`/`message`/`save`/`open`)、Tauri外 (localhost:1420 のブラウザ検証) では
// `window.*` にフォールバックする。どちらの実体も差し替え可能な形で組み立てるので、
// 実機無しで両方の分岐をテストできる。

import { ask, message, open as pluginOpen, save as pluginSave } from "@tauri-apps/plugin-dialog";
import { inTauri } from "../ipc";

/** 保存/開くダイアログの拡張子フィルタ。 */
export interface DialogFilter {
  name: string;
  extensions: string[];
}

/** 確認・通知ダイアログの見え方。 */
export interface DialogLook {
  /** ダイアログのタイトル (既定はアプリ名)。 */
  title?: string;
  /** 重大度。未保存の破棄などはwarning。 */
  kind?: "info" | "warning" | "error";
}

/** アプリが使うネイティブダイアログ。 */
export interface NativeDialogs {
  /** はい/いいえの確認。trueで続行 (キャンセル・閉じるはfalse)。 */
  confirm(text: string, look?: DialogLook): Promise<boolean>;
  /** 読むだけの通知 (OKだけ)。 */
  notify(text: string, look?: DialogLook): Promise<void>;
  /** 保存先を選ぶ。キャンセルならnull。`promptText`はTauri外の入力プロンプトの文面。 */
  save(
    defaultPath: string,
    filters: DialogFilter[],
    promptText: string,
  ): Promise<string | null>;
  /** 開くファイルを選ぶ。キャンセルならnull。`promptText`はTauri外の入力プロンプトの文面。 */
  open(filters: DialogFilter[], promptText: string): Promise<string | null>;
}

/** dialogプラグインのうちこの層が使う関数だけ (テストではフェイクを渡す)。 */
export interface DialogPlugin {
  ask(text: string, options?: DialogLook): Promise<boolean>;
  message(text: string, options?: DialogLook): Promise<unknown>;
  save(options: { defaultPath?: string; filters?: DialogFilter[] }): Promise<string | null>;
  open(options: {
    multiple?: boolean;
    filters?: DialogFilter[];
  }): Promise<string | string[] | null>;
}

/** ブラウザ側のダイアログ (Tauri外のフォールバック)。 */
export interface BrowserDialogHost {
  confirm?(text?: string): boolean;
  prompt?(text?: string, value?: string): string | null;
  alert?(text?: string): void;
}

/** Tauriのdialogプラグインを使う実体 (アプリ本体の経路)。 */
export function pluginDialogs(plugin: DialogPlugin): NativeDialogs {
  return {
    confirm: (text, look) => plugin.ask(text, { kind: "warning", ...look }),
    async notify(text, look) {
      await plugin.message(text, { kind: "info", ...look });
    },
    save: (defaultPath, filters) => plugin.save({ defaultPath, filters }),
    async open(filters) {
      const picked = await plugin.open({ multiple: false, filters });
      return typeof picked === "string" ? picked : null;
    },
  };
}

/**
 * Tauri外 (ブラウザでの実機検証) のフォールバック。
 * ネイティブのファイルダイアログが無いので、保存先・開くパスは入力してもらう。
 */
export function browserDialogs(host: BrowserDialogHost): NativeDialogs {
  return {
    async confirm(text) {
      return host.confirm ? host.confirm(text) : false;
    },
    async notify(text) {
      host.alert?.(text);
    },
    async save(defaultPath, _filters, promptText) {
      return host.prompt ? host.prompt(promptText, defaultPath) : null;
    },
    async open(_filters, promptText) {
      return host.prompt ? host.prompt(promptText) : null;
    },
  };
}

/** アプリが使う実体。Tauriの中ではプラグイン、外ではブラウザのダイアログ。 */
export const nativeDialogs: NativeDialogs = inTauri
  ? pluginDialogs({ ask, message, save: pluginSave, open: pluginOpen })
  : browserDialogs(globalThis as BrowserDialogHost);
