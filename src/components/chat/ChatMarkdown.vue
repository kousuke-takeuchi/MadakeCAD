<script setup lang="ts">
// 返答のMarkdown (.pen「AIチャット 作業過程」Markdown本文)。HTMLは renderMarkdown が安全に作ったものだけを入れる。
// リンクはwebviewを移動させずOSのブラウザで開き、コードブロックのボタンは中のコードをコピーする。
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { openUrl } from "@tauri-apps/plugin-opener";
import { renderMarkdown, safeExternalUrl } from "../../chat/markdown";
import { inTauri } from "../../ipc";

const props = defineProps<{ text: string }>();
const { t } = useI18n();
const html = computed(() => renderMarkdown(props.text, { copyLabel: t("chat.message.copy") }));

async function onClick(event: MouseEvent) {
  const target = event.target as HTMLElement;
  const copy = target.closest<HTMLButtonElement>("[data-copy]");
  if (copy) {
    const code = copy.closest(".md-code")?.querySelector("code")?.textContent ?? "";
    await navigator.clipboard.writeText(code);
    copy.textContent = t("chat.message.copied");
    setTimeout(() => (copy.textContent = t("chat.message.copy")), 1500);
    return;
  }
  const link = target.closest("a");
  if (!link) return;
  // webviewを移動させない。http(s)だけOSのブラウザで開く
  event.preventDefault();
  const url = safeExternalUrl(link.getAttribute("href"));
  if (!url) return;
  if (inTauri) await openUrl(url);
  else window.open(url, "_blank", "noopener");
}
</script>

<template>
  <!-- renderMarkdown が生HTMLを文字にし、リンクと画像を制限している -->
  <div class="md" @click="onClick" v-html="html" />
</template>

<style scoped>
.md {
  width: 100%;
  font-size: 12px;
  line-height: 1.6;
  color: var(--ui-text);
  word-break: break-word;
}
.md :deep(p) {
  margin: 0 0 6px;
}
.md :deep(p:last-child) {
  margin-bottom: 0;
}
.md :deep(h1),
.md :deep(h2),
.md :deep(h3),
.md :deep(h4) {
  margin: 8px 0 4px;
  font-size: 13px;
  font-weight: 700;
}
.md :deep(ul),
.md :deep(ol) {
  margin: 0 0 6px;
  padding-left: 16px;
}
.md :deep(blockquote) {
  margin: 0 0 6px;
  padding-left: 10px;
  border-left: 2px solid var(--ribbon-line);
  color: var(--ui-muted);
}
.md :deep(hr) {
  border: none;
  border-top: 1px solid var(--ribbon-line);
  margin: 8px 0;
}
.md :deep(a) {
  color: var(--acad-blue);
  text-decoration: underline;
  cursor: pointer;
}
.md :deep(code) {
  font-family: var(--mono-font);
  font-size: 11px;
  padding: 0 3px;
  border-radius: 3px;
  background: var(--hover-bg);
}
.md :deep(table) {
  width: 100%;
  margin: 0 0 6px;
  border-collapse: collapse;
  font-size: 11px;
  border: 1px solid var(--ribbon-line);
}
.md :deep(th),
.md :deep(td) {
  padding: 5px 9px;
  border-top: 1px solid var(--ribbon-line);
  text-align: left;
}
.md :deep(th) {
  background: var(--hover-bg);
  font-weight: 700;
}
.md :deep(.md-code) {
  margin: 0 0 6px;
  border-radius: 8px;
  background: var(--hover-bg);
  overflow: hidden;
}
.md :deep(.md-code-head) {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px 10px;
  border-bottom: 1px solid var(--ribbon-line);
}
.md :deep(.md-lang) {
  font-family: var(--mono-font);
  font-size: 10px;
  color: var(--ui-muted);
}
.md :deep(.md-copy) {
  padding: 0;
  border: none;
  background: transparent;
  font: inherit;
  font-size: 10px;
  color: var(--ui-muted);
  cursor: pointer;
}
.md :deep(.md-copy:hover) {
  color: var(--acad-blue);
}
.md :deep(pre) {
  margin: 0;
  padding: 10px;
  overflow-x: auto;
}
.md :deep(pre code) {
  padding: 0;
  background: transparent;
  font-size: 10px;
  line-height: 1.5;
  white-space: pre;
}
</style>
