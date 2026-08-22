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
      openai_base_url: "https://api.openai.com/v1",
      openai_model: "",
      gemini_model: "gemini-2.5-flash",
      default_frame_template: null,
    });
    expect(useSettingsStore().settings).toEqual(defaultSettings());
  });

  // ja: 図枠テンプレートのグローバル既定は初期状態では未設定(新しい図面は同梱のJIS A3図枠で始まる)
  it("starts with no global default frame template", () => {
    expect(defaultSettings().default_frame_template).toBeNull();
  });

  // ja: 図枠選択ダイアログの「グローバル既定」は設定として保存され、以降の表示にも残る
  it("saves the global default frame template chosen in the frame picker", async () => {
    const store = useSettingsStore();
    vi.spyOn(settingsApi, "set").mockImplementation(async (settings) => settings);
    expect(await store.save({ default_frame_template: "jis-a4-portrait" })).toBe(true);
    expect(settingsApi.set).toHaveBeenCalledWith(
      expect.objectContaining({ default_frame_template: "jis-a4-portrait" }),
    );
    expect(store.settings.default_frame_template).toBe("jis-a4-portrait");
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
      openai_base_url: "https://api.openai.com/v1",
      openai_model: "",
      gemini_model: "gemini-2.5-flash",
      default_frame_template: "jis-a4-portrait",
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
