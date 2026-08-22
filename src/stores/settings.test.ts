import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
import { useSettingsStore, settingsApi, defaultSettings, type AppSettings } from "./settings";

describe("settings store", () => {
  beforeEach(() => setActivePinia(createPinia()));

  // ja: 既定値は自動適用・図面自動読み取りがON
  it("defaults enable auto-apply and auto-read-drawing", () => {
    expect(defaultSettings()).toEqual({
      claude_path: null,
      auto_apply: true,
      auto_read_drawing: true,
      language: "en",
      knowledge_path: null,
      provider: "claude_cli",
      api_model: "claude-sonnet-5",
      copilot_path: null,
      copilot_model: "auto",
    });
    expect(useSettingsStore().settings).toEqual(defaultSettings());
  });

  // ja: loadでバックエンドの設定を取り込む
  it("load pulls the settings from the backend", async () => {
    const stored: AppSettings = {
      claude_path: "/opt/homebrew/bin/claude",
      auto_apply: false,
      auto_read_drawing: false,
      language: "ja",
      knowledge_path: "/home/me/house-rules.md",
      provider: "anthropic_api",
      api_model: "claude-opus-4-6",
      copilot_path: null,
      copilot_model: "auto",
    };
    vi.spyOn(settingsApi, "get").mockResolvedValue(stored);

    const store = useSettingsStore();
    await store.load();

    expect(store.settings).toEqual(stored);
    expect(store.loaded).toBe(true);
    expect(store.error).toBeNull();
  });

  // ja: saveは変更分をマージして送り、正規化後の戻り値を採用する
  it("save merges the changes, sends them, and adopts the normalized response", async () => {
    const set = vi
      .spyOn(settingsApi, "set")
      .mockResolvedValue({
        ...defaultSettings(),
        claude_path: "/usr/local/bin/claude",
        auto_read_drawing: false,
      });

    const store = useSettingsStore();
    const ok = await store.save({ claude_path: "  /usr/local/bin/claude  ", auto_read_drawing: false });

    expect(ok).toBe(true);
    expect(set).toHaveBeenCalledWith({
      ...defaultSettings(),
      claude_path: "  /usr/local/bin/claude  ",
      auto_read_drawing: false,
    });
    // サーバー側で空白を落とした値がそのまま表示に使われる
    expect(store.settings.claude_path).toBe("/usr/local/bin/claude");
    expect(store.saving).toBe(false);
  });

  // ja: 知識ファイルのパスを保存でき、空文字はバックエンドが未設定へ正規化する
  it("saves the knowledge-file path and adopts the backend's normalized value", async () => {
    const set = vi
      .spyOn(settingsApi, "set")
      .mockResolvedValue({ ...defaultSettings(), knowledge_path: null });

    const store = useSettingsStore();
    const ok = await store.save({ knowledge_path: "  " });

    expect(ok).toBe(true);
    expect(set).toHaveBeenCalledWith({ ...defaultSettings(), knowledge_path: "  " });
    expect(store.settings.knowledge_path).toBeNull();
  });

  // ja: save失敗時はエラーを保持し、表示中の設定を変えない
  it("a failed save keeps the error and leaves the shown settings untouched", async () => {
    vi.spyOn(settingsApi, "set").mockRejectedValue(new Error("Link API 400: だめ"));

    const store = useSettingsStore();
    const ok = await store.save({ auto_read_drawing: false });

    expect(ok).toBe(false);
    expect(store.settings).toEqual(defaultSettings());
    expect(store.error).toContain("だめ");
    expect(store.saving).toBe(false);
  });
});
