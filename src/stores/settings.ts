// アプリ設定ストア。
// 正はRust側 (`~/.madakecad/settings.json`)。ここはその写しで、保存すると
// Rust側が正規化した結果(空パス→未指定など)で置き換える。
// 設定はエージェントへ即時反映される(次の送信から有効。再起動不要)。

import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { inTauri } from "../ipc";

/**
 * エージェントの実行方式。
 * - `claude_cli`: ローカルのClaude Code CLI (サブスクリプションのサインインを利用)
 * - `anthropic_api`: Anthropic Messages APIへ直接 (APIキー。**キーはOSキーチェーン**)
 * - `copilot_cli`: ローカルのGitHub Copilot CLI (**MadakeCADは資格情報を持たない**。
 *   認証はCopilot自身のGitHubサインイン)
 */
export type AgentProvider = "claude_cli" | "anthropic_api" | "copilot_cli";

export const AGENT_PROVIDERS: AgentProvider[] = ["claude_cli", "anthropic_api", "copilot_cli"];

/** Rust側 `AppSettings` のJSON表現。**APIキーはここには入らない**(キーチェーンに置く)。 */
export interface AppSettings {
  /** claude実行ファイルの明示パス。nullなら自動検出 */
  claude_path: string | null;
  /** 確認なしで図面編集を適用する(A1は常にON。保存のみ) */
  auto_apply: boolean;
  /** 送信のたびに図面コンテキストをエージェントへ渡す */
  auto_read_drawing: boolean;
  /** UI表示言語 (BCP 47小文字。既定は "en"。未知の値はenへフォールバック) */
  language: string;
  /**
   * エージェントへ追加で読ませる知識ファイル (Markdown) のパス。nullなら同梱の規格ノートのみ。
   * 内容は同梱ノートの後ろへ追記される (社内・顧客の流儀で上書きできる)。
   */
  knowledge_path: string | null;
  /** エージェントの実行方式(既定はClaude Code CLI) */
  provider: AgentProvider;
  /** `anthropic_api` のときに使うモデルID */
  api_model: string;
  /** copilot実行ファイルの明示パス。nullなら自動検出 */
  copilot_path: string | null;
  /** `copilot_cli` のときに使うモデル ("auto" ならCopilotが選ぶ) */
  copilot_model: string;
}

export function defaultSettings(): AppSettings {
  return {
    claude_path: null,
    auto_apply: true,
    auto_read_drawing: true,
    language: "en",
    knowledge_path: null,
    provider: "claude_cli",
    api_model: "claude-sonnet-5",
    copilot_path: null,
    copilot_model: "auto",
  };
}

/**
 * プロバイダの状態。**資格情報そのものは含まない**
 * (Anthropicはキーが保存済みかどうかだけ、Copilotは自身のサインインを使う)。
 */
export interface ProviderStatus {
  provider: AgentProvider;
  api_model: string;
  api_key_saved: boolean;
  /** OSキーチェーンが読めなかった理由(読めたときはnull) */
  keychain_error?: string | null;
  /** `copilot_cli` で使うモデル */
  copilot_model?: string;
  /** copilot CLIが見つかったか (null = まだ調べていない) */
  copilot_detected?: boolean | null;
  /** 見つかったcopilot CLIのバージョン表示 */
  copilot_version?: string | null;
}

/** 接続テストの結果。 */
export interface ConnectionTest {
  ok: boolean;
  model?: string;
  /** 失敗理由の区分(UIが翻訳する。未知の区分はerrorをそのまま出す) */
  error_kind?: string;
  error?: string;
}

/** 保存済みAPIキーの伏せ字表示(値は画面に出さない)。 */
export function maskedApiKey(): string {
  return "••••••••••••••••";
}

interface SettingsApi {
  get(): Promise<AppSettings>;
  set(settings: AppSettings): Promise<AppSettings>;
}

/** APIキーとプロバイダ状態の入口(キーはここから先=OSキーチェーンへしか行かない)。 */
interface ProviderApi {
  status(): Promise<ProviderStatus>;
  setKey(key: string): Promise<ProviderStatus>;
  clearKey(): Promise<ProviderStatus>;
  test(): Promise<ConnectionTest>;
}

const tauriSettingsApi: SettingsApi = {
  get: () => invoke<AppSettings>("get_settings"),
  set: (settings) => invoke<AppSettings>("set_settings", { settings }),
};

const tauriProviderApi: ProviderApi = {
  status: () => invoke<ProviderStatus>("agent_provider_status"),
  setKey: (key) => invoke<ProviderStatus>("agent_set_api_key", { key }),
  clearKey: () => invoke<ProviderStatus>("agent_clear_api_key"),
  test: () => invoke<ConnectionTest>("agent_test_connection"),
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

const httpProviderApi: ProviderApi = {
  status: () => http<ProviderStatus>("/agent/provider"),
  setKey: (key) =>
    http<ProviderStatus>("/agent/api-key", { method: "PUT", body: JSON.stringify({ key }) }),
  clearKey: () => http<ProviderStatus>("/agent/api-key", { method: "DELETE" }),
  test: () => http<ConnectionTest>("/agent/test-connection", { method: "POST" }),
};

/** 差し替え可能なバックエンド入口(テストではここをスタブする)。 */
export const settingsApi: SettingsApi = inTauri ? tauriSettingsApi : httpSettingsApi;
export const providerApi: ProviderApi = inTauri ? tauriProviderApi : httpProviderApi;

interface SettingsState {
  settings: AppSettings;
  loaded: boolean;
  saving: boolean;
  /** 直近の保存・読込エラー(UIに表示する) */
  error: string | null;
  /**
   * Anthropic APIキーがOSキーチェーンに保存されているか。
   * **キーそのものはフロントに持たない**(保存後は伏せ字だけを表示する)。
   */
  apiKeySaved: boolean;
  /** APIキーの保存・削除の実行中 */
  keySaving: boolean;
  /** 接続テストの実行中 */
  testing: boolean;
  /** 直近の接続テストの結果(未実施ならnull) */
  testResult: ConnectionTest | null;
  /**
   * OSキーチェーンが読めなかった理由(読めたときはnull)。
   * 「保存したのにキー未設定と出る」ときに何が起きているかを画面へ出す。
   */
  keychainError: string | null;
  /** GitHub Copilot CLIが見つかっているか(接続バッジ用) */
  copilotDetected: boolean;
  /** 見つかったGitHub Copilot CLIのバージョン表示(未検出ならnull) */
  copilotVersion: string | null;
}

export const useSettingsStore = defineStore("settings", {
  state: (): SettingsState => ({
    settings: defaultSettings(),
    loaded: false,
    saving: false,
    error: null,
    apiKeySaved: false,
    keySaving: false,
    testing: false,
    testResult: null,
    keychainError: null,
    copilotDetected: false,
    copilotVersion: null,
  }),

  getters: {
    /** Anthropic API直結を選んでいるか。 */
    usingApiProvider: (state): boolean => state.settings.provider === "anthropic_api",
    /** GitHub Copilot CLIを選んでいるか。 */
    usingCopilotProvider: (state): boolean => state.settings.provider === "copilot_cli",
    /**
     * いま選んでいるプロバイダで送信できるか(接続バッジ用)。
     * claude CLIの検出結果はチャットストアが持つので引数で受ける。
     */
    agentReady() {
      return (cliDetected: boolean): boolean => {
        if (this.usingApiProvider) return this.apiKeySaved;
        if (this.usingCopilotProvider) return this.copilotDetected;
        return cliDetected;
      };
    },
  },

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

    /** プロバイダの状態(APIキーが保存済みか)を読み込む。 */
    async loadProviderStatus() {
      try {
        const status = await providerApi.status();
        this.apiKeySaved = status.api_key_saved;
        this.keychainError = status.keychain_error ?? null;
        // null (まだ調べていない) は「未検出」として扱う
        this.copilotDetected = status.copilot_detected ?? false;
        this.copilotVersion = status.copilot_version ?? null;
      } catch (e) {
        this.error = messageOf(e);
      }
    },

    /**
     * APIキーをOSキーチェーンへ保存する。戻り値は保存できたか。
     * **受け取った文字列はストアに残さない**(呼び出し側も入力欄を空にすること)。
     */
    async saveApiKey(key: string): Promise<boolean> {
      if (!key.trim()) {
        this.error = "empty-api-key";
        return false;
      }
      this.keySaving = true;
      try {
        const status = await providerApi.setKey(key);
        this.apiKeySaved = status.api_key_saved;
        this.keychainError = status.keychain_error ?? null;
        this.error = null;
        this.testResult = null;
        return true;
      } catch (e) {
        this.error = messageOf(e);
        return false;
      } finally {
        this.keySaving = false;
      }
    },

    /** 保存済みのAPIキーを消す。 */
    async clearApiKey(): Promise<boolean> {
      this.keySaving = true;
      try {
        const status = await providerApi.clearKey();
        this.apiKeySaved = status.api_key_saved;
        this.keychainError = status.keychain_error ?? null;
        this.error = null;
        this.testResult = null;
        return true;
      } catch (e) {
        this.error = messageOf(e);
        return false;
      } finally {
        this.keySaving = false;
      }
    },

    /** 設定が実際に使えるかを小さなリクエストで確かめる。 */
    async testConnection() {
      this.testing = true;
      this.testResult = null;
      try {
        this.testResult = await providerApi.test();
      } catch (e) {
        this.testResult = { ok: false, error: messageOf(e) };
      } finally {
        this.testing = false;
      }
    },
  },
});

function messageOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
