// AIプロバイダ設定 (Claude Code CLI / Anthropic API) のストアのテスト。
// APIキーはOSキーチェーンにだけ置くので、フロントは「保存済みかどうか」しか持たない。
// ここではその約束 (キーを状態に残さない・伏せ字表示・接続テストの結果表示) を固定する。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
import {
  AGENT_PROVIDERS,
  DEFAULT_GEMINI_MODEL,
  DEFAULT_OPENAI_BASE_URL,
  OLLAMA_BASE_URL,
  isLocalUrl,
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

describe("GitHub Copilot CLI provider", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
  });

  // ja: プロバイダの選択肢にGitHub Copilot CLIがある
  it("offers GitHub Copilot CLI as a provider", () => {
    expect(AGENT_PROVIDERS).toContain("copilot_cli");
  });

  // ja: Copilotの既定はモデルをCopilotに任せる`auto`で、実行ファイルはPATHから探す
  it("defaults Copilot to the auto model and PATH lookup", () => {
    expect(defaultSettings().copilot_model).toBe("auto");
    expect(defaultSettings().copilot_path).toBeNull();
  });

  // ja: GitHub Copilot CLIを選ぶと設定として保存される
  it("choosing GitHub Copilot saves it", async () => {
    const set = vi
      .spyOn(settingsApi, "set")
      .mockResolvedValue({ ...defaultSettings(), provider: "copilot_cli" });

    const store = useSettingsStore();
    const ok = await store.save({ provider: "copilot_cli" });

    expect(ok).toBe(true);
    expect(set).toHaveBeenCalledWith({ ...defaultSettings(), provider: "copilot_cli" });
    expect(store.settings.provider).toBe("copilot_cli");
  });

  // ja: Copilotのモデルと実行ファイルのパスは設定として保存できる
  it("saves the Copilot model and executable path", async () => {
    const saved = {
      ...defaultSettings(),
      provider: "copilot_cli" as const,
      copilot_model: "claude-sonnet-4.5",
      copilot_path: "/usr/local/bin/copilot",
    };
    vi.spyOn(settingsApi, "set").mockResolvedValue(saved);

    const store = useSettingsStore();
    await store.save({ copilot_model: "claude-sonnet-4.5", copilot_path: "/usr/local/bin/copilot" });

    expect(store.settings.copilot_model).toBe("claude-sonnet-4.5");
    expect(store.settings.copilot_path).toBe("/usr/local/bin/copilot");
  });

  // ja: プロバイダの状態からCopilot CLIが見つかったか(とバージョン)が分かる
  it("the provider status reports whether the Copilot CLI was found", async () => {
    vi.spyOn(providerApi, "status").mockResolvedValue({
      provider: "copilot_cli",
      api_model: "claude-sonnet-5",
      api_key_saved: false,
      copilot_model: "auto",
      copilot_detected: true,
      copilot_version: "GitHub Copilot CLI 1.0.80.",
    });

    const store = useSettingsStore();
    await store.loadProviderStatus();

    expect(store.copilotDetected).toBe(true);
    expect(store.copilotVersion).toContain("1.0.80");
  });

  // ja: Copilotを選んでいるときの接続バッジは、Copilot CLIが見つかったかどうかを見る
  it("the connection badge follows the Copilot CLI detection for the Copilot route", () => {
    const store = useSettingsStore();
    store.settings.provider = "copilot_cli";

    store.copilotDetected = true;
    expect(store.agentReady(false)).toBe(true);
    store.copilotDetected = false;
    // claude CLIが見つかっていてもCopilotが無ければ「未検出」
    expect(store.agentReady(true)).toBe(false);
  });

  // ja: Copilotが未認証のときは、接続テストがサインイン手順つきで失敗を返す
  it("a signed-out Copilot fails the connection test with sign-in guidance", async () => {
    vi.spyOn(providerApi, "test").mockResolvedValue({
      ok: false,
      error_kind: "copilot_auth",
      error: "GitHub Copilot CLIが未認証です。",
    });

    const store = useSettingsStore();
    await store.testConnection();

    expect(store.testResult?.ok).toBe(false);
    expect(store.testResult?.error_kind).toBe("copilot_auth");
  });

  // ja: Copilotの認証情報(GitHubトークン)はフロントの状態に一切持たない
  it("never keeps a GitHub credential in the frontend state", async () => {
    vi.spyOn(providerApi, "status").mockResolvedValue({
      provider: "copilot_cli",
      api_model: "claude-sonnet-5",
      api_key_saved: false,
      copilot_model: "auto",
      copilot_detected: true,
    });

    const store = useSettingsStore();
    await store.loadProviderStatus();

    const state = JSON.stringify(store.$state).toLowerCase();
    for (const secret of ["token", "gh_token", "password"]) {
      expect(state).not.toContain(secret);
    }
  });
});

describe("OpenAI-compatible provider (incl. Ollama)", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
  });

  // ja: プロバイダの選択肢にOpenAI互換APIがある
  it("offers an OpenAI-compatible endpoint as a provider", () => {
    expect(AGENT_PROVIDERS).toContain("openai_compat");
  });

  // ja: 既定の接続先はOpenAI本体で、モデル名は空(接続先ごとに違うので必ず選ばせる)
  it("defaults to OpenAI itself with an empty model box", () => {
    expect(defaultSettings().openai_base_url).toBe(DEFAULT_OPENAI_BASE_URL);
    expect(defaultSettings().openai_model).toBe("");
  });

  // ja: 「Ollama (ローカル)」プリセットはローカルの11434番へ向ける
  it("the Ollama preset points at the local Ollama server", () => {
    expect(OLLAMA_BASE_URL).toBe("http://localhost:11434/v1");
    expect(isLocalUrl(OLLAMA_BASE_URL)).toBe(true);
    expect(isLocalUrl("https://api.openai.com/v1")).toBe(false);
  });

  // ja: OpenAI互換APIのURLとモデル名は設定として保存できる
  it("saves the endpoint URL and the model name", async () => {
    const saved = {
      ...defaultSettings(),
      provider: "openai_compat" as const,
      openai_base_url: OLLAMA_BASE_URL,
      openai_model: "qwen3:4b",
    };
    vi.spyOn(settingsApi, "set").mockResolvedValue(saved);

    const store = useSettingsStore();
    await store.save({ openai_base_url: OLLAMA_BASE_URL, openai_model: "qwen3:4b" });

    expect(store.settings.openai_base_url).toBe(OLLAMA_BASE_URL);
    expect(store.settings.openai_model).toBe("qwen3:4b");
  });

  // ja: OpenAI互換APIのキーはAnthropicのキーとは別枠で保存され、値は状態に残らない
  it("saves its key separately from the Anthropic one and never keeps the value", async () => {
    const KEY = "sk-openai-must-not-be-kept";
    const setKey = vi.spyOn(providerApi, "setKey").mockResolvedValue({
      provider: "openai_compat",
      api_model: "claude-sonnet-5",
      api_key_saved: false,
      openai_key_saved: true,
    });

    const store = useSettingsStore();
    const ok = await store.saveApiKey(KEY, "openai_compat");

    expect(ok).toBe(true);
    expect(setKey).toHaveBeenCalledWith(KEY, "openai_compat");
    expect(store.openaiKeySaved).toBe(true);
    // Anthropic側の「保存済み」表示は動かない
    expect(store.apiKeySaved).toBe(false);
    expect(JSON.stringify(store.$state)).not.toContain(KEY);
  });

  // ja: OpenAI互換APIのキーを削除すると、そのプロバイダ指定で削除が呼ばれる
  it("removing its key asks the backend for that provider's entry", async () => {
    const clearKey = vi.spyOn(providerApi, "clearKey").mockResolvedValue({
      provider: "openai_compat",
      api_model: "claude-sonnet-5",
      api_key_saved: false,
      openai_key_saved: false,
    });

    const store = useSettingsStore();
    store.openaiKeySaved = true;
    const ok = await store.clearApiKey("openai_compat");

    expect(ok).toBe(true);
    expect(clearKey).toHaveBeenCalledWith("openai_compat");
    expect(store.openaiKeySaved).toBe(false);
  });

  // ja: 接続バッジは、キー保存済み(またはローカルURL)でモデル名が入っているときだけ「接続済み」
  it("the connection badge needs a model plus either a saved key or a local URL", () => {
    const store = useSettingsStore();
    store.settings.provider = "openai_compat";
    store.settings.openai_base_url = DEFAULT_OPENAI_BASE_URL;
    store.settings.openai_model = "gpt-4o";

    // 社外のURL: キーが要る
    expect(store.agentReady(true)).toBe(false);
    store.openaiKeySaved = true;
    expect(store.agentReady(false)).toBe(true);

    // ローカルのOllama: キー無しでも接続済み
    store.openaiKeySaved = false;
    store.settings.openai_base_url = OLLAMA_BASE_URL;
    store.settings.openai_model = "qwen3:4b";
    expect(store.agentReady(false)).toBe(true);

    // モデル名が空なら、ローカルでも「未接続」
    store.settings.openai_model = "  ";
    expect(store.agentReady(false)).toBe(false);
  });

  // ja: プロバイダの状態からOpenAI互換APIのURL・モデル・キーの保存状況が分かる
  it("the provider status reports the endpoint, the model and whether a key is saved", async () => {
    vi.spyOn(providerApi, "status").mockResolvedValue({
      provider: "openai_compat",
      api_model: "claude-sonnet-5",
      api_key_saved: false,
      openai_base_url: OLLAMA_BASE_URL,
      openai_model: "qwen3:4b",
      openai_key_saved: false,
    });

    const store = useSettingsStore();
    await store.loadProviderStatus();

    expect(store.openaiKeySaved).toBe(false);
  });

  // ja: キーが弾かれたときは、接続テストがOpenAI互換API用の区分つきで失敗を返す
  it("a rejected key fails the connection test with its own error kind", async () => {
    vi.spyOn(providerApi, "test").mockResolvedValue({
      ok: false,
      error_kind: "openai_auth",
      error: "APIキーが受け付けられませんでした。",
    });

    const store = useSettingsStore();
    await store.testConnection();

    expect(store.testResult?.ok).toBe(false);
    expect(store.testResult?.error_kind).toBe("openai_auth");
  });
});

describe("Google Gemini provider", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
  });

  // ja: プロバイダの選択肢にGoogle Geminiがあり、既定のモデル名が入っている
  it("offers Google Gemini as a provider with a default model", () => {
    expect(AGENT_PROVIDERS).toContain("gemini");
    expect(defaultSettings().gemini_model).toBe(DEFAULT_GEMINI_MODEL);
    expect(DEFAULT_GEMINI_MODEL).toBe("gemini-2.5-flash");
  });

  // ja: Geminiのモデル名は設定として保存できる
  it("saves the Gemini model name", async () => {
    const saved = {
      ...defaultSettings(),
      provider: "gemini" as const,
      gemini_model: "gemini-2.5-pro",
    };
    vi.spyOn(settingsApi, "set").mockResolvedValue(saved);

    const store = useSettingsStore();
    await store.save({ provider: "gemini", gemini_model: "gemini-2.5-pro" });

    expect(store.settings.provider).toBe("gemini");
    expect(store.settings.gemini_model).toBe("gemini-2.5-pro");
  });

  // ja: GeminiのキーはAnthropic・OpenAI互換とは別枠で保存され、値は状態に残らない
  it("saves its key separately from the other providers and never keeps the value", async () => {
    const KEY = "AIza-must-not-be-kept";
    const setKey = vi.spyOn(providerApi, "setKey").mockResolvedValue({
      provider: "gemini",
      api_model: "claude-sonnet-5",
      api_key_saved: false,
      openai_key_saved: false,
      gemini_key_saved: true,
    });

    const store = useSettingsStore();
    const ok = await store.saveApiKey(KEY, "gemini");

    expect(ok).toBe(true);
    expect(setKey).toHaveBeenCalledWith(KEY, "gemini");
    expect(store.geminiKeySaved).toBe(true);
    // ほかのプロバイダの「保存済み」表示は動かない
    expect(store.apiKeySaved).toBe(false);
    expect(store.openaiKeySaved).toBe(false);
    expect(JSON.stringify(store.$state)).not.toContain(KEY);
  });

  // ja: Geminiのキーを削除すると、そのプロバイダ指定で削除が呼ばれる
  it("removing its key asks the backend for that provider's entry", async () => {
    const clearKey = vi.spyOn(providerApi, "clearKey").mockResolvedValue({
      provider: "gemini",
      api_model: "claude-sonnet-5",
      api_key_saved: false,
      gemini_key_saved: false,
    });

    const store = useSettingsStore();
    store.geminiKeySaved = true;
    const ok = await store.clearApiKey("gemini");

    expect(ok).toBe(true);
    expect(clearKey).toHaveBeenCalledWith("gemini");
    expect(store.geminiKeySaved).toBe(false);
  });

  // ja: 接続バッジは、キーが保存済みでモデル名が入っているときだけ「使えます」
  it("the connection badge needs a saved key plus a model name", () => {
    const store = useSettingsStore();
    store.settings.provider = "gemini";
    store.settings.gemini_model = "gemini-2.5-flash";

    // キーが無いうちは、claude CLIが見つかっていても「使えない」
    expect(store.agentReady(true)).toBe(false);
    store.geminiKeySaved = true;
    expect(store.agentReady(false)).toBe(true);

    // モデル名が空なら、キーがあっても「使えない」
    store.settings.gemini_model = "  ";
    expect(store.agentReady(false)).toBe(false);
  });

  // ja: プロバイダの状態からGeminiのモデルとキーの保存状況が分かる
  it("the provider status reports the Gemini model and whether a key is saved", async () => {
    vi.spyOn(providerApi, "status").mockResolvedValue({
      provider: "gemini",
      api_model: "claude-sonnet-5",
      api_key_saved: false,
      gemini_model: "gemini-2.5-flash",
      gemini_key_saved: true,
    });

    const store = useSettingsStore();
    await store.loadProviderStatus();

    expect(store.geminiKeySaved).toBe(true);
    expect(JSON.stringify(store.$state)).not.toContain("AIza");
  });

  // ja: キーが弾かれたときは、接続テストがGemini用の区分つきで失敗を返す
  it("a rejected key fails the connection test with its own error kind", async () => {
    vi.spyOn(providerApi, "test").mockResolvedValue({
      ok: false,
      error_kind: "gemini_auth",
      error: "APIキーが受け付けられませんでした。",
    });

    const store = useSettingsStore();
    await store.testConnection();

    expect(store.testResult?.ok).toBe(false);
    expect(store.testResult?.error_kind).toBe("gemini_auth");
  });
});
