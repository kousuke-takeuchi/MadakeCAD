<script setup lang="ts">
// 入力欄の上の添付チップ列と注意帯 (.pen「添付デザイン」①③)。ドラッグ中の案内 (②) も出す。
// 浮きカードと左ドックの入力欄で共有する。
import { useI18n } from "vue-i18n";
import { MAX_ATTACHMENTS, MAX_ATTACHMENT_BYTES, useChatAttachmentsStore } from "../../stores/chatAttachments";
import AttachmentChip from "./AttachmentChip.vue";

const attachments = useChatAttachmentsStore();
const { t } = useI18n();
</script>

<template>
  <div v-if="attachments.dragOver" class="drop">
    <span class="drop-title">{{ t("chat.attach.dropTitle") }}</span>
    <span class="drop-hint">{{ t("chat.attach.dropHint", { max: MAX_ATTACHMENT_BYTES / 1024 / 1024, count: MAX_ATTACHMENTS }) }}</span>
  </div>
  <template v-else-if="attachments.items.length">
    <div class="chips">
      <AttachmentChip
        v-for="a in attachments.items"
        :key="a.id"
        :name="a.name"
        :mime="a.mime"
        :size="a.size"
        :problem="a.problem ? t(a.problem.key, a.problem.params) : null"
        removable
        :remove-title="t('chat.attach.remove')"
        @remove="attachments.remove(a.id)"
      />
    </div>
    <p v-if="attachments.providerProblem" class="band">{{ t(attachments.providerProblem.key, attachments.providerProblem.params) }}</p>
  </template>
</template>

<style scoped>
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  flex-shrink: 0;
  max-height: 96px;
  overflow-y: auto;
}
.band {
  margin: 0;
  flex-shrink: 0;
  padding: 5px 8px;
  border-radius: 6px;
  background: var(--warn-bg);
  color: var(--warn-fg);
  font-size: 10px;
  line-height: 14px;
}
.drop {
  flex: 1;
  min-height: 64px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  border-radius: 8px;
  background: var(--sel-blue);
}
.drop-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--acad-blue);
}
.drop-hint {
  font-size: 10px;
  color: var(--ui-muted);
}
</style>
