// アプリ設定ストア。
// 正はRust側 (`~/.madakecad/settings.json`)。ここはその写しで、保存すると
// Rust側が正規化した結果(空パス→未指定など)で置き換える。
// 設定はエージェントへ即時反映される(次の送信から有効。再起動不要)。

import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { inTauri } from "../ipc";

/** Rust側 `AppSettings` のJSON表現。 */
export interface AppSettings {
  /** claude実行ファイルの明示パス。nullなら自動検出 */
  claude_path: string | null;
  /** 確認なしで図面編集を適用する(A1は常にON。保存のみ) */
  auto_apply: boolean;
  /** 送信のたびに図面コンテキストをエージェントへ渡す */
  auto_read_drawing: boolean;
}

export function defaultSettings(): AppSettings {
  return { claude_path: null, auto_apply: true, auto_read_drawing: true };
}

interface SettingsApi {
  get(): Promise<AppSettings>;
  set(settings: AppSettings): Promise<AppSettings>;
}

const tauriSettingsApi: SettingsApi = {
  get: () => invoke<AppSettings>("get_settings"),
  set: (settings) => invoke<AppSettings>("set_settings", { settings }),
};

// Tauri外(ブラウザでのUI開発・E2E検証)では、起動中のMadakeCADのLink APIに接続する。
const API_BASE = "http://127.0.0.1:9310/api/v1";

async function http<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...init,
  });
  if (!res.ok) throw new Error(`Link API ${res.status}: ${await res.text()}`);
  return (await res.json()) as T;
}

const httpSettingsApi: SettingsApi = {
  get: () => http<AppSettings>("/settings"),
  set: (settings) => http<AppSettings>("/settings", { method: "PUT", body: JSON.stringify(settings) }),
};

/** 差し替え可能なバックエンド入口(テストではここをスタブする)。 */
export const settingsApi: SettingsApi = inTauri ? tauriSettingsApi : httpSettingsApi;

interface SettingsState {
  settings: AppSettings;
  loaded: boolean;
  saving: boolean;
  /** 直近の保存・読込エラー(UIに表示する) */
  error: string | null;
}

export const useSettingsStore = defineStore("settings", {
  state: (): SettingsState => ({
    settings: defaultSettings(),
    loaded: false,
    saving: false,
    error: null,
  }),

  actions: {
    /** 設定を読み込む(ダイアログを開いたとき)。 */
    async load() {
      try {
        this.settings = await settingsApi.get();
        this.loaded = true;
        this.error = null;
      } catch (e) {
        this.error = messageOf(e);
      }
    },

    /**
     * 変更分だけを渡して保存する。戻り値は保存できたか。
     * 失敗時は`error`に理由が入り、表示中の値は変更しない。
     */
    async save(patch: Partial<AppSettings>): Promise<boolean> {
      const next = { ...this.settings, ...patch };
      this.saving = true;
      try {
        this.settings = await settingsApi.set(next);
        this.loaded = true;
        this.error = null;
        return true;
      } catch (e) {
        this.error = messageOf(e);
        return false;
      } finally {
        this.saving = false;
      }
    },
  },
});

function messageOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
