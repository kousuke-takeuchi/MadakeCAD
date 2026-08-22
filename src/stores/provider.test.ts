// AIプロバイダ設定 (Claude Code CLI / Anthropic API) のストアのテスト。
// APIキーはOSキーチェーンにだけ置くので、フロントは「保存済みかどうか」しか持たない。
// ここではその約束 (キーを状態に残さない・伏せ字表示・接続テストの結果表示) を固定する。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
import {
  useSettingsStore,
  settingsApi,
  providerApi,
  defaultSettings,
  maskedApiKey,
} from "./settings";

describe("AI provider settings", () => {
  beforeEach(() => {
    // スパイはモジュールのオブジェクトに刺さるのでテストをまたいで残る。毎回外す
    vi.restoreAllMocks();
    setActivePinia(createPinia());
  });

  // ja: 既定のプロバイダはClaude Code CLIで、API経由に切り替えたときのモデルはClaude Sonnet 5
  it("defaults to the Claude Code CLI with Claude Sonnet 5 ready for the API route", () => {
    expect(defaultSettings().provider).toBe("claude_cli");
    expect(defaultSettings().api_model).toBe("claude-sonnet-5");
  });

  // ja: プロバイダを選び直すと設定として保存される
  it("choosing a provider saves it", async () => {
    const set = vi
      .spyOn(settingsApi, "set")
      .mockResolvedValue({ ...defaultSettings(), provider: "anthropic_api" });

    const store = useSettingsStore();
    const ok = await store.save({ provider: "anthropic_api" });

    expect(ok).toBe(true);
    expect(set).toHaveBeenCalledWith({ ...defaultSettings(), provider: "anthropic_api" });
    expect(store.settings.provider).toBe("anthropic_api");
  });

  // ja: プロバイダの状態を読み込むと「キーが保存済みか」が分かる
  it("loading the provider status tells whether a key is saved", async () => {
    vi.spyOn(providerApi, "status").mockResolvedValue({
      provider: "anthropic_api",
      api_model: "claude-sonnet-5",
      api_key_saved: true,
    });

    const store = useSettingsStore();
    await store.loadProviderStatus();

    expect(store.apiKeySaved).toBe(true);
  });

  // ja: 保存したAPIキーの文字列はフロントの状態に一切残らない
  it("the api key itself is never kept in the frontend state", async () => {
    const KEY = "sk-ant-must-not-be-kept";
    vi.spyOn(providerApi, "setKey").mockResolvedValue({
      provider: "anthropic_api",
      api_model: "claude-sonnet-5",
      api_key_saved: true,
    });

    const store = useSettingsStore();
    const ok = await store.saveApiKey(KEY);

    expect(ok).toBe(true);
    expect(store.apiKeySaved).toBe(true);
    expect(JSON.stringify(store.$state)).not.toContain(KEY);
  });

  // ja: キー欄が空のままなら送信せず、入力を促すエラーになる
  it("a blank key box is refused without calling the backend", async () => {
    const setKey = vi.spyOn(providerApi, "setKey");

    const store = useSettingsStore();
    const ok = await store.saveApiKey("   ");

    expect(ok).toBe(false);
    expect(setKey).not.toHaveBeenCalled();
    expect(store.error).toBeTruthy();
  });

  // ja: キーを削除すると「保存済み」表示が消える
  it("removing the key clears the saved badge", async () => {
    vi.spyOn(providerApi, "clearKey").mockResolvedValue({
      provider: "anthropic_api",
      api_model: "claude-sonnet-5",
      api_key_saved: false,
    });

    const store = useSettingsStore();
    store.apiKeySaved = true;
    const ok = await store.clearApiKey();

    expect(ok).toBe(true);
    expect(store.apiKeySaved).toBe(false);
  });

  // ja: 接続テストは成功すると確かめたモデル名を表示する
  it("a successful connection test shows the model it reached", async () => {
    vi.spyOn(providerApi, "test").mockResolvedValue({ ok: true, model: "claude-sonnet-5" });

    const store = useSettingsStore();
    await store.testConnection();

    expect(store.testResult).toEqual({ ok: true, model: "claude-sonnet-5" });
    expect(store.testing).toBe(false);
  });

  // ja: 接続テストが失敗すると理由をそのまま表示する
  it("a failed connection test shows the reason", async () => {
    vi.spyOn(providerApi, "test").mockResolvedValue({
      ok: false,
      error_kind: "auth",
      error: "APIキーが受け付けられませんでした。",
    });

    const store = useSettingsStore();
    await store.testConnection();

    expect(store.testResult?.ok).toBe(false);
    // 画面はerror_kindで対訳を引く。対訳が無い区分のときだけerrorをそのまま出す
    expect(store.testResult?.error_kind).toBe("auth");
    expect(store.testResult?.error).toContain("APIキー");
  });

  // ja: 接続テストの実行中はtestingが立ち、前回の結果は消える
  it("the previous test result is cleared while a new test runs", async () => {
    let resolve!: (v: { ok: boolean }) => void;
    vi.spyOn(providerApi, "test").mockReturnValue(
      new Promise((r) => {
        resolve = r;
      }),
    );

    const store = useSettingsStore();
    store.testResult = { ok: false, error: "前回の失敗" };
    const pending = store.testConnection();
    expect(store.testing).toBe(true);
    expect(store.testResult).toBeNull();

    resolve({ ok: true });
    await pending;
    expect(store.testing).toBe(false);
  });

  // ja: 接続バッジはClaude Code CLIならCLIの検出、Anthropic APIならキーの保存状況を見る
  it("the connection badge follows the CLI for the CLI route and the saved key for the API route", () => {
    const store = useSettingsStore();

    // Claude Code CLI: CLIが見つかっていれば「接続済み」
    expect(store.agentReady(true)).toBe(true);
    expect(store.agentReady(false)).toBe(false);

    // Anthropic API: CLIが無くてもキーが保存されていれば「接続済み」
    store.settings.provider = "anthropic_api";
    store.apiKeySaved = true;
    expect(store.agentReady(false)).toBe(true);
    store.apiKeySaved = false;
    expect(store.agentReady(true)).toBe(false);
  });

  // ja: 保存済みのキーは伏せ字で表す(値そのものは画面に出さない)
  it("a saved key is shown as dots, never as its value", () => {
    const masked = maskedApiKey();
    expect(masked.length).toBeGreaterThan(7);
    expect(masked.replace(/•/g, "")).toBe("");
  });
});
