<script setup lang="ts">
// 設定ダイアログ (Pencilデザイン「AI設定 - Claude詳細」準拠)。
// A1範囲は「エージェント」タブのClaude欄(検出状態・実行パス・動作トグル)のみ。
// 他タブはタブだけ出してプレースホルダを表示する。
import {
  Asterisk, CheckCircle2, FolderOpen, MessageSquare, RefreshCw, Settings, Sparkles,
  Terminal, TriangleAlert, User, X, type LucideIcon,
} from "lucide-vue-next";
import { computed, ref, watch } from "vue";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useUiStore } from "../../stores/ui";

const ui = useUiStore();
const settings = useSettingsStore();
// claude CLIの検出状態はチャットストアが持つ(チャットパネルの接続バッジと共通)
const chat = useChatStore();

type TabId = "general" | "agent" | "chat" | "mcp" | "account";
const tabs: { id: TabId; label: string; icon: LucideIcon }[] = [
  { id: "general", label: "一般", icon: Settings },
  { id: "agent", label: "エージェント", icon: Sparkles },
  { id: "chat", label: "チャット", icon: MessageSquare },
  { id: "mcp", label: "MCP", icon: Terminal },
  { id: "account", label: "アカウント", icon: User },
];
const activeTab = ref<TabId>("agent");

/** パス入力欄(保存するまでは設定に反映しない)。 */
const pathInput = ref("");
const detecting = ref(false);

const detected = computed(() => chat.detect);
const pathDirty = computed(() => pathInput.value.trim() !== (settings.settings.claude_path ?? ""));

const toggles = computed(() => [
  {
    key: "auto_apply" as const,
    label: "自動許可モード",
    description: "確認なしでClaudeが図面編集を続行します。全編集はundoで戻せます (A1では常に自動適用)",
    value: settings.settings.auto_apply,
  },
  {
    key: "auto_read_drawing" as const,
    label: "図面の自動読み取り",
    description: "会話開始時にプロジェクトとネットリストを共有します",
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

/** 実行パスを保存し、そのパスで検出し直す。 */
async function savePath(ev?: Event) {
  if (settings.saving || !pathDirty.value) return;
  if (ev instanceof KeyboardEvent && ev.isComposing) return;
  const path = pathInput.value.trim();
  if (!(await settings.save({ claude_path: path || null }))) return;
  pathInput.value = settings.settings.claude_path ?? "";
  ui.log(path ? `設定: claude実行パスを ${path} に変更` : "設定: claude実行パスを自動検出に戻しました");
  await redetect();
}

async function toggle(key: "auto_apply" | "auto_read_drawing", value: boolean) {
  const patch = key === "auto_apply" ? { auto_apply: value } : { auto_read_drawing: value };
  if (!(await settings.save(patch))) return;
  const label = key === "auto_apply" ? "自動許可モード" : "図面の自動読み取り";
  ui.log(`設定: ${label}を${value ? "ON" : "OFF"}にしました`);
}
</script>

<template>
  <div v-if="ui.settingsOpen" class="overlay" @click.self="close">
    <div class="dialog">
      <div class="tab-row">
        <div class="tab-group">
          <button
            v-for="t in tabs"
            :key="t.id"
            class="tab"
            :class="{ active: t.id === activeTab }"
            @click="activeTab = t.id"
          >
            <component :is="t.icon" :size="13" />
            {{ t.label }}
          </button>
        </div>
        <div class="tab-space" />
        <button class="close" title="閉じる" @click="close"><X :size="15" /></button>
      </div>

      <div class="body">
        <template v-if="activeTab === 'agent'">
          <div class="card provider-head">
            <Asterisk :size="22" class="provider-icon" />
            <div class="provider-text">
              <div class="provider-name">Anthropic Claude</div>
              <div class="provider-sub">Claude Code CLI (サブスクリプションのサインインをそのまま利用)</div>
            </div>
            <span v-if="detected" class="badge ok"><i /> 検出済み</span>
            <span v-else class="badge off"><i /> 未検出</span>
          </div>

          <div class="columns">
            <div class="col-main">
              <div class="card">
                <div class="card-title">はじめに</div>
                <p class="card-text">
                  ターミナルで <code>claude</code> にサインイン済みなら、そのままチャットから図面を編集できます。
                  APIキーの入力は不要です (認証はClaude Code CLIのサインインに委譲します)。
                </p>
              </div>

              <div class="card">
                <div class="card-title">検出結果</div>
                <div v-if="detected" class="detect ok-box">
                  <CheckCircle2 :size="15" class="detect-icon ok-fg" />
                  <div class="detect-text">
                    <div class="detect-line">claude CLI を検出しました</div>
                    <div class="detect-meta">
                      <span class="mono">{{ detected.path }}</span>
                      <span class="mono ver">{{ detected.version }}</span>
                    </div>
                  </div>
                </div>
                <div v-else class="detect warn-box">
                  <TriangleAlert :size="15" class="detect-icon warn-fg" />
                  <div class="detect-text">
                    <div class="detect-line">claude CLI が見つかりません</div>
                    <div class="detect-meta">
                      ターミナルで <span class="mono">claude --version</span> が通るか確認するか、
                      右の「Claude実行ファイル」にパスを指定してください
                    </div>
                  </div>
                </div>
                <button class="btn secondary" :disabled="detecting" @click="redetect">
                  <RefreshCw :size="13" :class="{ spin: detecting }" />
                  {{ detecting ? "検出中..." : "再検出" }}
                </button>
              </div>

              <p v-if="settings.error" class="error">{{ settings.error }}</p>
            </div>

            <div class="col-side">
              <div class="card">
                <div class="section-title">エージェント設定</div>
                <div v-for="t in toggles" :key="t.key" class="toggle-row">
                  <div class="toggle-text">
                    <div class="toggle-label">{{ t.label }}</div>
                    <div class="toggle-desc">{{ t.description }}</div>
                  </div>
                  <button
                    class="toggle"
                    :class="{ on: t.value }"
                    :disabled="settings.saving"
                    :title="t.value ? 'ON' : 'OFF'"
                    @click="toggle(t.key, !t.value)"
                  >
                    <span class="knob" />
                  </button>
                </div>
              </div>

              <div class="card">
                <div class="card-title-row">
                  <span class="card-title plain">Claude実行ファイル</span>
                  <span class="adv">ADVANCED</span>
                </div>
                <p class="card-text small">
                  自動検出の代わりに使うclaude実行ファイルのパス。空にすると自動検出へ戻ります。
                </p>
                <div class="path-row">
                  <label class="path-input">
                    <FolderOpen :size="12" class="path-icon" />
                    <input
                      v-model="pathInput"
                      class="mono"
                      placeholder="/usr/local/bin/claude"
                      spellcheck="false"
                      @keydown.enter="savePath"
                    />
                  </label>
                  <button class="btn primary" :disabled="settings.saving || !pathDirty" @click="savePath">
                    保存
                  </button>
                </div>
              </div>
            </div>
          </div>
        </template>

        <div v-else class="card placeholder">
          「{{ tabs.find((t) => t.id === activeTab)?.label }}」の設定は今後のフェーズで実装予定です
        </div>
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
  width: min(1000px, 94vw);
  height: min(820px, 90vh);
  background: var(--ribbon-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 16px;
  box-shadow: var(--shadow-panel-lg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* --- タブ --- */
.tab-row {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 10px 16px;
  flex: none;
}
.tab-group {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 4px 6px;
  border: 1px solid var(--ribbon-line);
  border-radius: 12px;
}
.tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border: 1px solid transparent;
  border-radius: 9px;
  background: transparent;
  font-size: 12px;
  color: var(--ui-text);
  cursor: pointer;
}
.tab:hover { background: var(--hover-bg); }
.tab.active {
  background: var(--hover-bg);
  border-color: var(--ribbon-line);
}
.tab-space { flex: 1; }
.close {
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  cursor: pointer;
}

/* --- 本体 --- */
.body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 6px 24px 24px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.card {
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 12px;
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.card-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--ui-text);
}
.card-title-row { display: flex; align-items: center; gap: 7px; }
.card-title.plain { margin: 0; }
.section-title {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 1px;
  color: var(--ui-muted);
}
.card-text {
  margin: 0;
  font-size: 12px;
  line-height: 18px;
  color: var(--ui-muted);
}
.card-text.small { font-size: 10px; line-height: 15px; }
.card-text code, .mono { font-family: var(--mono-font); }
.adv {
  background: var(--hover-bg);
  color: var(--ui-muted);
  border-radius: 5px;
  padding: 2px 7px;
  font-size: 8px;
  font-weight: 600;
  letter-spacing: 1px;
}
.placeholder {
  font-size: 12px;
  color: var(--ui-muted);
  align-items: center;
  padding: 28px 16px;
}

/* --- プロバイダ見出し --- */
.provider-head {
  flex-direction: row;
  align-items: center;
  gap: 12px;
  padding: 14px 16px;
}
.provider-icon { color: var(--acad-blue); flex: none; }
.provider-text { flex: 1; display: flex; flex-direction: column; gap: 3px; }
.provider-name { font-size: 17px; font-weight: 700; color: var(--ui-text); }
.provider-sub { font-size: 12px; color: var(--ui-placeholder); }
.badge {
  display: flex;
  align-items: center;
  gap: 5px;
  border-radius: 10px;
  padding: 3px 10px;
  font-size: 10px;
  flex: none;
}
.badge i { width: 6px; height: 6px; border-radius: 50%; background: currentColor; }
.badge.ok { background: var(--ok-bg); color: var(--ok-fg); }
.badge.off { background: var(--off-bg); color: var(--off-fg); }

/* --- 2カラム --- */
.columns { display: flex; align-items: flex-start; gap: 12px; }
.col-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 12px; }
.col-side { width: 320px; flex: none; display: flex; flex-direction: column; gap: 12px; }

/* --- 検出結果 --- */
.detect {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  border-radius: 8px;
  padding: 9px 11px;
}
.detect.ok-box { background: var(--ok-bg); }
.detect.warn-box { background: var(--warn-bg); }
.detect-icon { flex: none; margin-top: 1px; }
.ok-fg { color: var(--ok-fg); }
.warn-fg { color: var(--warn-fg); }
.detect-text { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
.detect-line { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.detect-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 10px;
  font-size: 10px;
  line-height: 15px;
  color: var(--ui-muted);
  word-break: break-all;
}
.detect-meta .ver { color: var(--ui-placeholder); }

/* --- ボタン --- */
.btn {
  display: flex;
  align-items: center;
  gap: 6px;
  border-radius: 8px;
  padding: 6px 12px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  width: fit-content;
}
.btn:disabled { opacity: 0.5; cursor: default; }
.btn.primary { background: var(--acad-blue); border: 1px solid var(--acad-blue); color: var(--card-bg); }
.btn.secondary { background: var(--card-bg); border: 1px solid var(--ribbon-line); color: var(--ui-text); }
.btn.secondary:hover:not(:disabled) { background: var(--hover-bg); }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

/* --- トグル --- */
.toggle-row { display: flex; align-items: flex-start; gap: 10px; }
.toggle-text { flex: 1; display: flex; flex-direction: column; gap: 3px; }
.toggle-label { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.toggle-desc { font-size: 10px; line-height: 15px; color: var(--ui-muted); }
.toggle {
  position: relative;
  width: 38px;
  height: 22px;
  flex: none;
  border: none;
  border-radius: 11px;
  background: var(--off-fg);
  cursor: pointer;
  padding: 0;
}
.toggle.on { background: var(--acad-blue); }
.toggle:disabled { opacity: 0.6; cursor: default; }
.knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: var(--card-bg);
  transition: left 0.12s ease;
}
.toggle.on .knob { left: 18px; }

/* --- パス入力 --- */
.path-row { display: flex; align-items: center; gap: 7px; }
.path-input {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  background: var(--input-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 8px;
  padding: 7px 10px;
}
.path-icon { color: var(--ui-placeholder); flex: none; }
.path-input input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  font-size: 10px;
  color: var(--ui-text);
}
.error { margin: 0; font-size: 11px; color: var(--err-fg); }
</style>
