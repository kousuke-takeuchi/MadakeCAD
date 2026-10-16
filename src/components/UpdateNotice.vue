<script setup lang="ts">
// 更新の通知カード (.pen「自動更新デザイン」`qyYGO`の「起動時の通知」)。
// 起動時の確認で新しい版が見つかったときだけ、作図画面の右下に重ねる。新しい部品ではなく
// 既存のカード (card-bg + acad-blue枠、角丸8) にDSボタン主/副を載せたもの。
// 「後で」はこの起動中その版を出さない。ダウンロード中は進捗バーに変わる。
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useUpdatesStore } from "../stores/updates";
import { updateStatusLine } from "./updateStatus";

const updates = useUpdatesStore();
const { t } = useI18n();

const busy = computed(() => updates.status === "downloading" || updates.status === "installing");
const line = computed(() => updateStatusLine(updates.$state));
</script>

<template>
  <div v-if="updates.showNotice && updates.available" class="notice" role="status">
    <div class="head">
      <i class="dot" />
      <span class="title">{{ t("updates.notice.title", { version: updates.available.version }) }}</span>
    </div>
    <p v-if="!busy && updates.status !== 'error'" class="sub">
      {{ t("updates.notice.sub", { current: updates.currentVersion || "—" }) }}
    </p>
    <template v-else>
      <p class="sub" :class="line.tone">{{ t(line.key, line.params) }}</p>
      <div v-if="updates.status === 'downloading'" class="progress">
        <i :style="{ width: `${Math.round((updates.progress ?? 0.1) * 100)}%` }" />
      </div>
    </template>
    <div class="actions">
      <button class="link" @click="updates.openReleasePage()">{{ t("updates.releaseNotes") }}</button>
      <button class="btn secondary" :disabled="busy" @click="updates.dismiss()">{{ t("updates.notice.later") }}</button>
      <button class="btn primary" :disabled="busy" @click="updates.install(t('updates.unsavedConfirm'))">
        {{ updates.support === "manual" ? t("updates.openReleasePage") : t("updates.installRestart") }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.notice {
  position: fixed;
  right: 16px;
  bottom: 40px;
  width: 360px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px;
  background: var(--card-bg);
  border: 1px solid var(--acad-blue);
  border-radius: 8px;
  box-shadow: var(--shadow-panel-lg);
  z-index: 90;
}
.head { display: flex; align-items: center; gap: 8px; }
.dot { width: 8px; height: 8px; border-radius: 4px; background: var(--acad-blue); flex: none; }
.title { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.sub { margin: 0; font-size: 10px; color: var(--ui-muted); }
.sub.info { color: var(--info-fg); }
.sub.err { color: var(--err-fg); }
.progress { height: 6px; background: var(--input-bg); border-radius: 3px; overflow: hidden; }
.progress i { display: block; height: 100%; background: var(--acad-blue); }
.actions { display: flex; align-items: center; justify-content: flex-end; gap: 8px; }
.link { border: none; background: transparent; padding: 0; font-size: 11px; color: var(--acad-blue); cursor: pointer; }
.btn { border-radius: 4px; padding: 5px 12px; font-size: 11px; font-weight: 600; cursor: pointer; }
.btn:disabled { opacity: 0.5; cursor: default; }
.btn.primary { background: var(--acad-blue); border: 1px solid var(--acad-blue); color: var(--card-bg); }
.btn.secondary { background: var(--card-bg); border: 1px solid var(--ribbon-line); color: var(--ui-text); }
.btn.secondary:hover:not(:disabled) { background: var(--hover-bg); }
</style>
