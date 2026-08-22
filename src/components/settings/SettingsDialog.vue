<script setup lang="ts">
// 設定ダイアログ (Pencilデザイン「設定ダイアログ - CAD調リデザイン」準拠)。
// EPLAN/ACADEのオプションダイアログ様式: 左カテゴリツリー+グループボックス+OK/キャンセル/適用。
// 実装済みは「一般」(言語) と「エージェント」カテゴリで、他カテゴリはプレースホルダを表示する。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { Check, KeyRound, RefreshCw, X } from "lucide-vue-next";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { SUPPORTED_LOCALES, resolveLocale, setLocale } from "../../i18n";
import { useChatStore } from "../../stores/chat";
import {
  AGENT_PROVIDERS,
  OLLAMA_BASE_URL,
  maskedApiKey,
  useSettingsStore,
  type AgentProvider,
} from "../../stores/settings";
import { useUiStore } from "../../stores/ui";

const ui = useUiStore();
const settings = useSettingsStore();
// claude CLIの検出状態はチャットストアが持つ(チャットパネルの接続バッジと共通)
const chat = useChatStore();
const { t, te } = useI18n();

type TabId = "general" | "agent" | "chat" | "mcp" | "account";
const tabs: TabId[] = ["general", "agent", "chat", "mcp", "account"];
const activeTab = ref<TabId>("agent");

/** パス入力欄(適用/OKするまでは設定に反映しない)。 */
const pathInput = ref("");
/** 知識ファイル(エージェントへ追記で読ませるMarkdown)のパス入力欄。 */
const knowledgeInput = ref("");
/** Anthropic API経由で使うモデルIDの入力欄。 */
const apiModelInput = ref("");
/** GitHub Copilot経由で使うモデルの入力欄 (`auto`でCopilotが選ぶ)。 */
const copilotModelInput = ref("");
/** copilot実行ファイルのパス入力欄 (空なら自動検出)。 */
const copilotPathInput = ref("");
/** OpenAI互換APIの接続先URLの入力欄 (`/chat/completions` の1つ上)。 */
const openaiBaseUrlInput = ref("");
/** OpenAI互換APIで使うモデル名の入力欄 (接続先ごとに違うので既定値は無い)。 */
const openaiModelInput = ref("");
/**
 * OpenAI互換APIのキーの入力欄。**保存したらすぐ空にする**(Anthropicのキー欄と同じ約束)。
 * ローカルのOllamaならキーは不要なので、空のままでも接続できる。
 */
const openaiKeyInput = ref("");
/**
 * APIキーの入力欄。**保存したらすぐ空にする**(キーを画面にもメモリにも残さない)。
 * 保存済みのキーは伏せ字だけを表示し、値を読み戻すことはできない。
 */
const apiKeyInput = ref("");
const detecting = ref(false);

const detected = computed(() => chat.detect);
const provider = computed(() => settings.settings.provider);
const usingApi = computed(() => provider.value === "anthropic_api");
const usingCopilot = computed(() => provider.value === "copilot_cli");
const usingOpenAi = computed(() => provider.value === "openai_compat");
/** claude CLIの検出結果を出すのはCLIプロバイダのときだけ。 */
const usingClaudeCli = computed(() => provider.value === "claude_cli");
/** いま選んでいるプロバイダで送信できる状態か(バッジ表示)。 */
const providerReady = computed(() => settings.agentReady(!!detected.value));
const pathDirty = computed(() => pathInput.value.trim() !== (settings.settings.claude_path ?? ""));
const knowledgeDirty = computed(
  () => knowledgeInput.value.trim() !== (settings.settings.knowledge_path ?? ""),
);
const apiModelDirty = computed(() => apiModelInput.value.trim() !== settings.settings.api_model);
const copilotModelDirty = computed(
  () => copilotModelInput.value.trim() !== settings.settings.copilot_model,
);
const copilotPathDirty = computed(
  () => copilotPathInput.value.trim() !== (settings.settings.copilot_path ?? ""),
);
const openaiBaseUrlDirty = computed(
  () => openaiBaseUrlInput.value.trim() !== settings.settings.openai_base_url,
);
const openaiModelDirty = computed(
  () => openaiModelInput.value.trim() !== settings.settings.openai_model,
);
const dirty = computed(
  () =>
    pathDirty.value ||
    knowledgeDirty.value ||
    apiModelDirty.value ||
    copilotModelDirty.value ||
    copilotPathDirty.value ||
    openaiBaseUrlDirty.value ||
    openaiModelDirty.value,
);
const activeLocale = computed(() => resolveLocale(settings.settings.language));

const checks = computed(() => [
  {
    key: "auto_apply" as const,
    label: t("settings.agent.autoApprove"),
    description: t("settings.agent.autoApproveDesc"),
    value: settings.settings.auto_apply,
  },
  {
    key: "auto_read_drawing" as const,
    label: t("settings.agent.autoRead"),
    description: t("settings.agent.autoReadDesc"),
    value: settings.settings.auto_read_drawing,
  },
]);

// 開いたときに最新の設定を読み、未検出ならCLIを探す
watch(
  () => ui.settingsOpen,
  async (open) => {
    if (!open) return;
    activeTab.value = "agent";
    await settings.load();
    pathInput.value = settings.settings.claude_path ?? "";
    knowledgeInput.value = settings.settings.knowledge_path ?? "";
    apiModelInput.value = settings.settings.api_model;
    copilotModelInput.value = settings.settings.copilot_model;
    copilotPathInput.value = settings.settings.copilot_path ?? "";
    openaiBaseUrlInput.value = settings.settings.openai_base_url;
    openaiModelInput.value = settings.settings.openai_model;
    apiKeyInput.value = "";
    openaiKeyInput.value = "";
    settings.testResult = null;
    await settings.loadProviderStatus();
    if (!chat.detect) await redetect();
  },
);

function close() {
  ui.settingsOpen = false;
}

async function redetect() {
  detecting.value = true;
  try {
    await chat.detectCli();
  } finally {
    detecting.value = false;
  }
}

/** 変更したパス設定を保存する(適用)。実行パスを変えた場合は検出し直す。 */
async function apply(): Promise<boolean> {
  if (settings.saving || !dirty.value) return true;
  const redetectAfter = pathDirty.value;
  const path = pathInput.value.trim();
  const knowledge = knowledgeInput.value.trim();
  const knowledgeChanged = knowledgeDirty.value;
  const modelChanged = apiModelDirty.value;
  const copilotModelChanged = copilotModelDirty.value;
  const copilotPathChanged = copilotPathDirty.value;
  const copilotPath = copilotPathInput.value.trim();
  const openaiChanged = openaiBaseUrlDirty.value || openaiModelDirty.value;
  if (
    !(await settings.save({
      claude_path: path || null,
      knowledge_path: knowledge || null,
      api_model: apiModelInput.value.trim(),
      copilot_model: copilotModelInput.value.trim(),
      copilot_path: copilotPath || null,
      openai_base_url: openaiBaseUrlInput.value.trim(),
      openai_model: openaiModelInput.value.trim(),
    }))
  )
    return false;
  pathInput.value = settings.settings.claude_path ?? "";
  knowledgeInput.value = settings.settings.knowledge_path ?? "";
  apiModelInput.value = settings.settings.api_model;
  copilotModelInput.value = settings.settings.copilot_model;
  copilotPathInput.value = settings.settings.copilot_path ?? "";
  openaiBaseUrlInput.value = settings.settings.openai_base_url;
  openaiModelInput.value = settings.settings.openai_model;
  if (openaiChanged) {
    ui.log(
      t("settings.agent.openaiSetLog", {
        url: settings.settings.openai_base_url,
        model: settings.settings.openai_model || t("settings.agent.openaiModelUnset"),
      }),
    );
    settings.testResult = null;
  }
  if (modelChanged) {
    ui.log(t("settings.agent.apiModelSetLog", { model: settings.settings.api_model }));
  }
  if (copilotModelChanged) {
    ui.log(t("settings.agent.copilotModelSetLog", { model: settings.settings.copilot_model }));
  }
  if (copilotPathChanged) {
    ui.log(
      copilotPath
        ? t("settings.agent.copilotPathSetLog", { path: copilotPath })
        : t("settings.agent.copilotPathClearedLog"),
    );
    // 実行ファイルを変えたら検出をやり直す(バッジを実態に合わせる)
    await settings.loadProviderStatus();
  }
  if (redetectAfter) {
    ui.log(path ? t("settings.agent.pathSetLog", { path }) : t("settings.agent.pathClearedLog"));
    await redetect();
  }
  if (knowledgeChanged) {
    ui.log(
      knowledge
        ? t("settings.agent.knowledgeSetLog", { path: knowledge })
        : t("settings.agent.knowledgeClearedLog"),
    );
  }
  return true;
}

/** OK = 適用して閉じる。 */
async function confirm() {
  if (await apply()) close();
}

async function toggle(key: "auto_apply" | "auto_read_drawing", value: boolean) {
  const patch = key === "auto_apply" ? { auto_apply: value } : { auto_read_drawing: value };
  if (!(await settings.save(patch))) return;
  const label = t(key === "auto_apply" ? "settings.agent.autoApprove" : "settings.agent.autoRead");
  ui.log(t(value ? "settings.agent.toggleOnLog" : "settings.agent.toggleOffLog", { label }));
}

/** エージェントのプロバイダを切り替える(即時保存)。 */
async function changeProvider(ev: Event) {
  const next = (ev.target as HTMLSelectElement).value as AgentProvider;
  if (!(await settings.save({ provider: next }))) return;
  settings.testResult = null;
  await settings.loadProviderStatus();
  ui.log(t("settings.agent.providerChangedLog", { provider: t(providerLabelKey(next)) }));
}

/**
 * 接続テストの失敗表示。バックエンドが返す区分(`error_kind`)に対訳があれば
 * それを使い、無ければ説明文をそのまま出す。
 */
const testErrorText = computed(() => {
  const result = settings.testResult;
  if (!result || result.ok) return "";
  const key = `settings.agent.testError.${result.error_kind ?? ""}`;
  return te(key) ? t(key) : (result.error ?? "");
});

/** ストアが立てた合図(`empty-api-key`)は翻訳して出す。それ以外は原文のまま。 */
const settingsError = computed(() =>
  settings.error === "empty-api-key" ? t("settings.agent.apiKeyEmpty") : settings.error,
);

function providerLabelKey(p: AgentProvider): string {
  if (p === "anthropic_api") return "settings.agent.providerApi";
  if (p === "copilot_cli") return "settings.agent.providerCopilot";
  if (p === "openai_compat") return "settings.agent.providerOpenAi";
  return "settings.agent.providerCli";
}

/** プロバイダごとの説明文(選択直下のキャプション)。 */
const providerHintKey = computed(() => {
  if (usingApi.value) return "settings.agent.providerHintApi";
  if (usingCopilot.value) return "settings.agent.providerHintCopilot";
  if (usingOpenAi.value) return "settings.agent.providerHintOpenAi";
  return "settings.agent.providerHint";
});

/** プロバイダごとの接続バッジ文言。 */
const providerBadgeKey = computed(() => {
  if (usingApi.value) {
    return providerReady.value
      ? "settings.agent.keySavedBadge"
      : "settings.agent.keyMissingBadge";
  }
  if (usingOpenAi.value) {
    return providerReady.value
      ? "settings.agent.openaiReadyBadge"
      : "settings.agent.openaiNotReadyBadge";
  }
  return providerReady.value ? "settings.agent.detectedBadge" : "settings.agent.notDetectedBadge";
});

/** 入力したAPIキーをOSキーチェーンへ保存する(保存後は入力欄を空にする)。 */
async function saveApiKey() {
  if (!(await settings.saveApiKey(apiKeyInput.value))) return;
  apiKeyInput.value = "";
  ui.log(t("settings.agent.apiKeySavedLog"));
}

/** 保存済みのAPIキーを消す。 */
async function removeApiKey() {
  if (!(await settings.clearApiKey())) return;
  apiKeyInput.value = "";
  ui.log(t("settings.agent.apiKeyRemovedLog"));
}

/** 入力したOpenAI互換APIのキーをOSキーチェーンへ保存する(保存後は入力欄を空にする)。 */
async function saveOpenAiKey() {
  if (!(await settings.saveApiKey(openaiKeyInput.value, "openai_compat"))) return;
  openaiKeyInput.value = "";
  ui.log(t("settings.agent.openaiKeySavedLog"));
}

/** 保存済みのOpenAI互換APIのキーを消す。 */
async function removeOpenAiKey() {
  if (!(await settings.clearApiKey("openai_compat"))) return;
  openaiKeyInput.value = "";
  ui.log(t("settings.agent.openaiKeyRemovedLog"));
}

/**
 * 「Ollama (ローカル)」プリセット。接続先をローカルのOllamaへ向けて即座に保存する。
 * OllamaはAPIキーを要求しないので、キー欄は空のままでよい。
 */
async function useOllamaPreset() {
  openaiBaseUrlInput.value = OLLAMA_BASE_URL;
  if (!(await apply())) return;
  await settings.loadProviderStatus();
}

/** 表示言語を保存して即時切替する。 */
async function changeLanguage(ev: Event) {
  const language = (ev.target as HTMLSelectElement).value;
  if (!(await settings.save({ language }))) return;
  setLocale(settings.settings.language);
  ui.log(t("settings.general.languageChangedLog", { language: t(`language.${activeLocale.value}`) }));
}
</script>

<template>
  <div v-if="ui.settingsOpen" class="overlay" @click.self="close">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("settings.title") }}</span>
        <button class="close" :title="t('settings.close')" @click="close"><X :size="14" /></button>
      </div>

      <div class="body-row">
        <div class="tree">
          <button
            v-for="id in tabs"
            :key="id"
            class="tree-item"
            :class="{ active: id === activeTab }"
            @click="activeTab = id"
          >
            {{ t(`settings.category.${id}`) }}
          </button>
        </div>

        <div class="content">
          <template v-if="activeTab === 'general'">
            <div class="group">
              <div class="group-head"><span>{{ t("settings.general.languageGroup") }}</span><i /></div>
              <div class="form-row">
                <label class="form-label" for="ui-language">{{ t("settings.general.displayLanguage") }}</label>
                <select
                  id="ui-language"
                  class="field select"
                  :value="activeLocale"
                  :disabled="settings.saving"
                  @change="changeLanguage"
                >
                  <option v-for="l in SUPPORTED_LOCALES" :key="l" :value="l">{{ t(`language.${l}`) }}</option>
                </select>
              </div>
              <p class="caption indent-label">{{ t("settings.general.languageHint") }}</p>
            </div>
            <p v-if="settings.error" class="error">{{ settings.error }}</p>
          </template>

          <template v-else-if="activeTab === 'agent'">
            <div class="group">
              <div class="group-head"><span>{{ t("settings.agent.providerGroup") }}</span><i /></div>
              <div class="form-row">
                <label class="form-label" for="agent-provider">{{ t("settings.agent.providerLabel") }}</label>
                <select
                  id="agent-provider"
                  class="field select wide"
                  :value="provider"
                  :disabled="settings.saving"
                  @change="changeProvider"
                >
                  <option v-for="p in AGENT_PROVIDERS" :key="p" :value="p">
                    {{ t(providerLabelKey(p)) }}
                  </option>
                </select>
                <span v-if="providerReady" class="status ok"><i /> {{ t(providerBadgeKey) }}</span>
                <span v-else class="status off"><i /> {{ t(providerBadgeKey) }}</span>
              </div>
              <p class="caption indent-label">{{ t(providerHintKey) }}</p>
            </div>

            <div v-if="usingCopilot" class="group">
              <div class="group-head"><span>{{ t("settings.agent.copilotGroup") }}</span><i /></div>
              <div class="form-row">
                <label class="form-label">{{ t("settings.agent.executableLabel") }}</label>
                <div class="field mono-field">
                  <span v-if="settings.copilotDetected">
                    {{ settings.settings.copilot_path ?? "copilot" }}
                    <template v-if="settings.copilotVersion"> · {{ settings.copilotVersion }}</template>
                  </span>
                  <span v-else class="muted">{{ t("settings.agent.notFound") }}</span>
                </div>
                <span v-if="settings.copilotDetected" class="detect-ok">
                  <Check :size="12" /> {{ t("settings.agent.detectedOk") }}
                </span>
                <span v-else class="detect-warn">{{ t("settings.agent.copilotMissing") }}</span>
              </div>
              <i18n-t
                v-if="!settings.copilotDetected"
                keypath="settings.agent.copilotMissingHint"
                tag="p"
                class="caption indent-label"
              >
                <template #command><span class="mono">npm install -g @github/copilot</span></template>
              </i18n-t>

              <div class="form-row">
                <label class="form-label" for="copilot-model">{{ t("settings.agent.copilotModelLabel") }}</label>
                <input
                  id="copilot-model"
                  v-model="copilotModelInput"
                  class="field input mono"
                  spellcheck="false"
                  placeholder="auto"
                  @keydown.enter="(e) => !(e as KeyboardEvent).isComposing && apply()"
                />
              </div>
              <p class="caption indent-label">{{ t("settings.agent.copilotModelHint") }}</p>

              <div class="form-row">
                <label class="form-label" for="copilot-path">{{ t("settings.agent.copilotPathLabel") }}</label>
                <input
                  id="copilot-path"
                  v-model="copilotPathInput"
                  class="field input mono"
                  placeholder="/usr/local/bin/copilot"
                  spellcheck="false"
                  @keydown.enter="(e) => !(e as KeyboardEvent).isComposing && apply()"
                />
              </div>
              <p class="caption indent-label">{{ t("settings.agent.copilotPathHint") }}</p>

              <div class="form-row">
                <span class="form-label" />
                <button
                  class="btn secondary test-btn"
                  :disabled="settings.testing || !settings.copilotDetected"
                  @click="settings.testConnection()"
                >
                  <RefreshCw :size="12" :class="{ spin: settings.testing }" />
                  {{ settings.testing ? t("settings.agent.testing") : t("settings.agent.testConnection") }}
                </button>
              </div>
              <p v-if="settings.testResult?.ok" class="caption indent-label ok">
                <Check :size="11" />
                {{ t("settings.agent.testOk", { model: settings.testResult.model ?? settings.settings.copilot_model }) }}
              </p>
              <p v-else-if="settings.testResult" class="caption indent-label warn">{{ testErrorText }}</p>
              <p v-else class="caption indent-label">{{ t("settings.agent.copilotTestHint") }}</p>
            </div>

            <div v-if="usingApi" class="group">
              <div class="group-head"><span>{{ t("settings.agent.apiGroup") }}</span><i /></div>
              <div class="form-row">
                <label class="form-label" for="api-key">{{ t("settings.agent.apiKeyLabel") }}</label>
                <div v-if="settings.apiKeySaved" class="field mono-field">
                  <span><KeyRound :size="11" /> {{ maskedApiKey() }}</span>
                </div>
                <input
                  v-else
                  id="api-key"
                  v-model="apiKeyInput"
                  class="field input mono"
                  type="password"
                  autocomplete="off"
                  spellcheck="false"
                  :placeholder="t('settings.agent.apiKeyPlaceholder')"
                  @keydown.enter="(e) => !(e as KeyboardEvent).isComposing && saveApiKey()"
                />
                <button
                  v-if="settings.apiKeySaved"
                  class="btn secondary"
                  :disabled="settings.keySaving"
                  @click="removeApiKey"
                >
                  {{ t("settings.agent.apiKeyRemove") }}
                </button>
                <button
                  v-else
                  class="btn secondary"
                  :disabled="settings.keySaving || !apiKeyInput.trim()"
                  @click="saveApiKey"
                >
                  {{ t("settings.agent.apiKeySave") }}
                </button>
              </div>
              <p class="caption indent-label">{{ t("settings.agent.apiKeyHint") }}</p>
              <p v-if="settings.keychainError" class="caption indent-label warn">
                {{ t("settings.agent.keychainError", { reason: settings.keychainError }) }}
              </p>

              <div class="form-row">
                <label class="form-label" for="api-model">{{ t("settings.agent.apiModelLabel") }}</label>
                <input
                  id="api-model"
                  v-model="apiModelInput"
                  class="field input mono"
                  spellcheck="false"
                  placeholder="claude-sonnet-5"
                  @keydown.enter="(e) => !(e as KeyboardEvent).isComposing && apply()"
                />
              </div>
              <p class="caption indent-label">{{ t("settings.agent.apiModelHint") }}</p>

              <div class="form-row">
                <span class="form-label" />
                <button
                  class="btn secondary test-btn"
                  :disabled="settings.testing || !settings.apiKeySaved"
                  @click="settings.testConnection()"
                >
                  <RefreshCw :size="12" :class="{ spin: settings.testing }" />
                  {{ settings.testing ? t("settings.agent.testing") : t("settings.agent.testConnection") }}
                </button>
              </div>
              <p v-if="settings.testResult?.ok" class="caption indent-label ok">
                <Check :size="11" />
                {{ t("settings.agent.testOk", { model: settings.testResult.model ?? settings.settings.api_model }) }}
              </p>
              <p v-else-if="settings.testResult" class="caption indent-label warn">{{ testErrorText }}</p>
              <p v-else-if="!settings.apiKeySaved" class="caption indent-label">
                {{ t("settings.agent.testNeedsKey") }}
              </p>
            </div>

            <div v-if="usingOpenAi" class="group">
              <div class="group-head"><span>{{ t("settings.agent.openaiGroup") }}</span><i /></div>
              <div class="form-row">
                <label class="form-label" for="openai-url">{{ t("settings.agent.openaiUrlLabel") }}</label>
                <input
                  id="openai-url"
                  v-model="openaiBaseUrlInput"
                  class="field input mono"
                  spellcheck="false"
                  autocomplete="off"
                  placeholder="https://api.openai.com/v1"
                  @keydown.enter="(e) => !(e as KeyboardEvent).isComposing && apply()"
                />
                <button
                  class="btn secondary"
                  :disabled="settings.saving"
                  @click="useOllamaPreset"
                >
                  {{ t("settings.agent.ollamaPreset") }}
                </button>
              </div>
              <p class="caption indent-label">{{ t("settings.agent.openaiUrlHint") }}</p>

              <div class="form-row">
                <label class="form-label" for="openai-model">{{ t("settings.agent.openaiModelLabel") }}</label>
                <input
                  id="openai-model"
                  v-model="openaiModelInput"
                  class="field input mono"
                  spellcheck="false"
                  :placeholder="t('settings.agent.openaiModelPlaceholder')"
                  @keydown.enter="(e) => !(e as KeyboardEvent).isComposing && apply()"
                />
              </div>
              <p class="caption indent-label">{{ t("settings.agent.openaiModelHint") }}</p>

              <div class="form-row">
                <label class="form-label" for="openai-key">{{ t("settings.agent.openaiKeyLabel") }}</label>
                <div v-if="settings.openaiKeySaved" class="field mono-field">
                  <span><KeyRound :size="11" /> {{ maskedApiKey() }}</span>
                </div>
                <input
                  v-else
                  id="openai-key"
                  v-model="openaiKeyInput"
                  class="field input mono"
                  type="password"
                  autocomplete="off"
                  spellcheck="false"
                  :placeholder="t('settings.agent.openaiKeyPlaceholder')"
                  @keydown.enter="(e) => !(e as KeyboardEvent).isComposing && saveOpenAiKey()"
                />
                <button
                  v-if="settings.openaiKeySaved"
                  class="btn secondary"
                  :disabled="settings.keySaving"
                  @click="removeOpenAiKey"
                >
                  {{ t("settings.agent.apiKeyRemove") }}
                </button>
                <button
                  v-else
                  class="btn secondary"
                  :disabled="settings.keySaving || !openaiKeyInput.trim()"
                  @click="saveOpenAiKey"
                >
                  {{ t("settings.agent.apiKeySave") }}
                </button>
              </div>
              <p class="caption indent-label">
                {{ settings.openaiIsLocal ? t("settings.agent.openaiKeyLocalHint") : t("settings.agent.openaiKeyHint") }}
              </p>
              <p v-if="settings.keychainError" class="caption indent-label warn">
                {{ t("settings.agent.keychainError", { reason: settings.keychainError }) }}
              </p>

              <div class="form-row">
                <span class="form-label" />
                <button
                  class="btn secondary test-btn"
                  :disabled="settings.testing || !providerReady"
                  @click="settings.testConnection()"
                >
                  <RefreshCw :size="12" :class="{ spin: settings.testing }" />
                  {{ settings.testing ? t("settings.agent.testing") : t("settings.agent.testConnection") }}
                </button>
              </div>
              <p v-if="settings.testResult?.ok" class="caption indent-label ok">
                <Check :size="11" />
                {{ t("settings.agent.testOk", { model: settings.testResult.model ?? settings.settings.openai_model }) }}
              </p>
              <p v-else-if="settings.testResult" class="caption indent-label warn">{{ testErrorText }}</p>
              <p v-else-if="!providerReady" class="caption indent-label">
                {{ t("settings.agent.openaiTestHint") }}
              </p>
            </div>

            <div v-if="usingClaudeCli" class="group">
              <div class="group-head"><span>{{ t("settings.agent.detectionGroup") }}</span><i /></div>
              <div class="form-row">
                <label class="form-label">{{ t("settings.agent.executableLabel") }}</label>
                <div class="field mono-field">
                  <span v-if="detected">{{ detected.path }} · {{ detected.version }}</span>
                  <span v-else class="muted">{{ t("settings.agent.notFound") }}</span>
                </div>
                <span v-if="detected" class="detect-ok"><Check :size="12" /> {{ t("settings.agent.detectedOk") }}</span>
                <span v-else class="detect-warn">{{ t("settings.agent.cliMissing") }}</span>
                <span class="spacer" />
                <button class="btn secondary" :disabled="detecting" @click="redetect">
                  <RefreshCw :size="12" :class="{ spin: detecting }" />
                  {{ detecting ? t("settings.agent.detecting") : t("settings.agent.redetect") }}
                </button>
              </div>
              <i18n-t
                v-if="!detected"
                keypath="settings.agent.cliMissingHint"
                tag="p"
                class="caption indent-label"
              >
                <template #command><span class="mono">claude --version</span></template>
              </i18n-t>
            </div>

            <div class="group">
              <div class="group-head"><span>{{ t("settings.agent.settingsGroup") }}</span><i /></div>
              <template v-for="c in checks" :key="c.key">
                <label class="check-row">
                  <button
                    class="checkbox"
                    :class="{ on: c.value }"
                    role="checkbox"
                    :aria-checked="c.value"
                    :disabled="settings.saving"
                    @click="toggle(c.key, !c.value)"
                  >
                    <Check v-if="c.value" :size="11" :stroke-width="3" />
                  </button>
                  <span class="check-label">{{ c.label }}</span>
                </label>
                <p class="caption indent-check">{{ c.description }}</p>
              </template>
            </div>

            <div class="group">
              <div class="group-head"><span>{{ t("settings.agent.advancedGroup") }}</span><i /></div>
              <template v-if="usingClaudeCli">
                <div class="form-row">
                  <label class="form-label" for="claude-path">{{ t("settings.agent.claudePathLabel") }}</label>
                  <input
                    id="claude-path"
                    v-model="pathInput"
                    class="field input mono"
                    placeholder="/usr/local/bin/claude"
                    spellcheck="false"
                    @keydown.enter="(e) => !(e as KeyboardEvent).isComposing && apply()"
                  />
                </div>
                <p class="caption indent-label">{{ t("settings.agent.claudePathHint") }}</p>
              </template>
              <div class="form-row">
                <label class="form-label" for="knowledge-path">{{ t("settings.agent.knowledgeLabel") }}</label>
                <input
                  id="knowledge-path"
                  v-model="knowledgeInput"
                  class="field input mono"
                  placeholder="/Users/me/madakecad/house-rules.md"
                  spellcheck="false"
                  @keydown.enter="(e) => !(e as KeyboardEvent).isComposing && apply()"
                />
              </div>
              <p class="caption indent-label">{{ t("settings.agent.knowledgeHint") }}</p>
            </div>

            <p v-if="settings.error" class="error">{{ settingsError }}</p>
          </template>

          <p v-else class="placeholder">
            {{ t("settings.placeholder", { category: t(`settings.category.${activeTab}`) }) }}
          </p>
        </div>
      </div>

      <div class="footer">
        <button class="btn primary" :disabled="settings.saving" @click="confirm">{{ t("settings.ok") }}</button>
        <button class="btn secondary" @click="close">{{ t("settings.cancel") }}</button>
        <button class="btn secondary" :disabled="settings.saving || !dirty" @click="apply">
          {{ t("settings.apply") }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: var(--scrim);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.dialog {
  width: min(900px, 94vw);
  height: min(620px, 90vh);
  background: var(--ribbon-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 8px;
  box-shadow: var(--shadow-panel-lg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* --- タイトルバー --- */
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--palette-head);
  flex: none;
}
.title { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.close {
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  cursor: pointer;
  padding: 0;
}

/* --- 左カテゴリツリー --- */
.body-row { flex: 1; min-height: 0; display: flex; }
.tree {
  width: 180px;
  flex: none;
  background: var(--palette-bg);
  border-right: 1px solid var(--ribbon-line);
  padding: 8px 0;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
}
.tree-item {
  border: none;
  background: transparent;
  text-align: left;
  padding: 6px 14px;
  font-size: 12px;
  color: var(--ui-muted);
  cursor: pointer;
}
.tree-item:hover { background: var(--hover-bg); }
.tree-item.active {
  background: var(--sel-blue);
  color: var(--ui-text);
  font-weight: 600;
}

/* --- 内容・グループボックス --- */
.content {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.group { display: flex; flex-direction: column; gap: 9px; }
.group-head { display: flex; align-items: center; gap: 8px; }
.group-head span { font-size: 11px; font-weight: 600; color: var(--ui-text); flex: none; }
.group-head i { flex: 1; height: 1px; background: var(--ribbon-line); }

.form-row { display: flex; align-items: center; gap: 10px; }
.form-label { width: 130px; flex: none; font-size: 11px; color: var(--ui-text); }
.field {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 300px;
  padding: 5px 8px;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  font-size: 12px;
  color: var(--ui-text);
}
.field.static { cursor: default; }
.field .caret { font-size: 10px; color: var(--ui-muted); }
.field.mono-field { font-family: var(--mono-font); font-size: 11px; }
.field.mono-field .muted { color: var(--ui-placeholder); }
.field.input { outline: none; font-size: 11px; }
.field.input:focus { border-color: var(--acad-blue); }
.field.select { width: 220px; outline: none; cursor: pointer; }
.field.select.wide { width: 300px; }
.field.select:focus { border-color: var(--acad-blue); }
.field.mono-field span { display: flex; align-items: center; gap: 6px; }
.mono { font-family: var(--mono-font); }
.spacer { flex: 1; }

.caption { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }
.caption.warn { color: var(--warn-fg); }
.caption.ok { color: var(--ok-fg); display: flex; align-items: center; gap: 4px; }
.test-btn { flex: none; }
.indent-label { padding-left: 140px; }
.indent-check { padding-left: 22px; margin-top: -3px; }

/* --- 状態表示 --- */
.status { display: flex; align-items: center; gap: 5px; font-size: 10px; flex: none; }
.status i { width: 6px; height: 6px; border-radius: 50%; background: currentColor; }
.status.ok { color: var(--ok-fg); }
.status.off { color: var(--off-fg); }
.detect-ok { display: flex; align-items: center; gap: 4px; font-size: 11px; color: var(--ok-fg); flex: none; }
.detect-warn { font-size: 11px; color: var(--warn-fg); flex: none; }

/* --- チェックボックス --- */
.check-row { display: flex; align-items: center; gap: 8px; cursor: pointer; width: fit-content; }
.checkbox {
  width: 14px;
  height: 14px;
  flex: none;
  border: 1px solid var(--ribbon-line);
  border-radius: 2px;
  background: var(--card-bg);
  color: #ffffff;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  padding: 0;
}
.checkbox.on { background: var(--acad-blue); border-color: var(--acad-blue); }
.checkbox:disabled { opacity: 0.6; cursor: default; }
.check-label { font-size: 12px; color: var(--ui-text); }

/* --- フッタ・ボタン --- */
.footer {
  flex: none;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 10px 14px;
  border-top: 1px solid var(--ribbon-line);
}
.btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-width: 72px;
  border-radius: 4px;
  padding: 5px 14px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}
.btn:disabled { opacity: 0.5; cursor: default; }
.btn.primary { background: var(--acad-blue); border: 1px solid var(--acad-blue); color: var(--card-bg); }
.btn.secondary { background: var(--card-bg); border: 1px solid var(--ribbon-line); color: var(--ui-text); }
.btn.secondary:hover:not(:disabled) { background: var(--hover-bg); }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

.placeholder { margin: 0; font-size: 12px; color: var(--ui-muted); padding: 14px 0; }
.error { margin: 0; font-size: 11px; color: var(--err-fg); }
</style>
