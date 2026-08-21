<script setup lang="ts">
// 設定ダイアログ (Pencilデザイン「設定ダイアログ - CAD調リデザイン」準拠)。
// EPLAN/ACADEのオプションダイアログ様式: 左カテゴリツリー+グループボックス+OK/キャンセル/適用。
// 実装済みは「エージェント」カテゴリのみで、他カテゴリはプレースホルダを表示する。
import { Check, RefreshCw, X } from "lucide-vue-next";
import { computed, ref, watch } from "vue";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useUiStore } from "../../stores/ui";

const ui = useUiStore();
const settings = useSettingsStore();
// claude CLIの検出状態はチャットストアが持つ(チャットパネルの接続バッジと共通)
const chat = useChatStore();

type TabId = "general" | "agent" | "chat" | "mcp" | "account";
const tabs: { id: TabId; label: string }[] = [
  { id: "general", label: "一般" },
  { id: "agent", label: "エージェント" },
  { id: "chat", label: "チャット" },
  { id: "mcp", label: "MCP" },
  { id: "account", label: "アカウント" },
];
const activeTab = ref<TabId>("agent");

/** パス入力欄(適用/OKするまでは設定に反映しない)。 */
const pathInput = ref("");
const detecting = ref(false);

const detected = computed(() => chat.detect);
const pathDirty = computed(() => pathInput.value.trim() !== (settings.settings.claude_path ?? ""));

const checks = computed(() => [
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

/** 実行パスを保存し、そのパスで検出し直す(適用)。 */
async function apply(): Promise<boolean> {
  if (settings.saving || !pathDirty.value) return true;
  const path = pathInput.value.trim();
  if (!(await settings.save({ claude_path: path || null }))) return false;
  pathInput.value = settings.settings.claude_path ?? "";
  ui.log(path ? `設定: claude実行パスを ${path} に変更` : "設定: claude実行パスを自動検出に戻しました");
  await redetect();
  return true;
}

/** OK = 適用して閉じる。 */
async function confirm() {
  if (await apply()) close();
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
      <div class="titlebar">
        <span class="title">設定</span>
        <button class="close" title="閉じる" @click="close"><X :size="14" /></button>
      </div>

      <div class="body-row">
        <div class="tree">
          <button
            v-for="t in tabs"
            :key="t.id"
            class="tree-item"
            :class="{ active: t.id === activeTab }"
            @click="activeTab = t.id"
          >
            {{ t.label }}
          </button>
        </div>

        <div class="content">
          <template v-if="activeTab === 'agent'">
            <div class="group">
              <div class="group-head"><span>プロバイダ</span><i /></div>
              <div class="form-row">
                <label class="form-label">プロバイダ:</label>
                <div class="field static">
                  <span>Anthropic Claude (Claude Code CLI)</span>
                  <span class="caret">▾</span>
                </div>
                <span v-if="detected" class="status ok"><i /> 検出済み</span>
                <span v-else class="status off"><i /> 未検出</span>
              </div>
              <p class="caption indent-label">
                サブスクリプションのサインインをそのまま利用します。APIキーの入力は不要です
                (認証はClaude Code CLIのサインインに委譲します)。
              </p>
            </div>

            <div class="group">
              <div class="group-head"><span>検出</span><i /></div>
              <div class="form-row">
                <label class="form-label">実行ファイル:</label>
                <div class="field mono-field">
                  <span v-if="detected">{{ detected.path }} · {{ detected.version }}</span>
                  <span v-else class="muted">見つかりません</span>
                </div>
                <span v-if="detected" class="detect-ok"><Check :size="12" /> 検出しました</span>
                <span v-else class="detect-warn">claude CLI が見つかりません</span>
                <span class="spacer" />
                <button class="btn secondary" :disabled="detecting" @click="redetect">
                  <RefreshCw :size="12" :class="{ spin: detecting }" />
                  {{ detecting ? "検出中..." : "再検出" }}
                </button>
              </div>
              <p v-if="!detected" class="caption indent-label">
                ターミナルで <span class="mono">claude --version</span> が通るか確認するか、
                下の「Claude実行ファイル」にパスを指定してください。
              </p>
            </div>

            <div class="group">
              <div class="group-head"><span>エージェント設定</span><i /></div>
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
              <div class="group-head"><span>詳細設定</span><i /></div>
              <div class="form-row">
                <label class="form-label" for="claude-path">Claude実行ファイル:</label>
                <input
                  id="claude-path"
                  v-model="pathInput"
                  class="field input mono"
                  placeholder="/usr/local/bin/claude"
                  spellcheck="false"
                  @keydown.enter="(e) => !(e as KeyboardEvent).isComposing && apply()"
                />
              </div>
              <p class="caption indent-label">
                自動検出の代わりに使うClaude実行ファイルのパス。空にすると自動検出に戻ります。
              </p>
            </div>

            <p v-if="settings.error" class="error">{{ settings.error }}</p>
          </template>

          <p v-else class="placeholder">
            「{{ tabs.find((t) => t.id === activeTab)?.label }}」の設定は今後のフェーズで実装予定です
          </p>
        </div>
      </div>

      <div class="footer">
        <button class="btn primary" :disabled="settings.saving" @click="confirm">OK</button>
        <button class="btn secondary" @click="close">キャンセル</button>
        <button class="btn secondary" :disabled="settings.saving || !pathDirty" @click="apply">適用</button>
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
.mono { font-family: var(--mono-font); }
.spacer { flex: 1; }

.caption { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }
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
