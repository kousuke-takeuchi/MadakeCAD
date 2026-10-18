<script setup lang="ts">
// DS添付チップ / DS添付チップ エラー (.pen `sIHcy` / `cUmIA`)。
// 種別印 (IMG=sel-blue / PDF=err-bg) +ファイル名+大きさ+× (送信前だけ)。
// 添付できないものはチップを赤くし、大きさの代わりに理由を出す。
import { computed } from "vue";
import { formatSize, isAttachableMime, isPdf } from "../../stores/chatAttachments";

const props = defineProps<{
  name: string;
  mime: string;
  size: number;
  /** 添付できない理由 (あれば赤いチップ)。 */
  problem?: string | null;
  /** ×で外せる (送信前)。 */
  removable?: boolean;
  removeTitle?: string;
}>();
defineEmits<{ remove: [] }>();

const pdf = computed(() => isPdf(props.mime));
/** 種別印: PDF / IMG / 添付できない種類は「?」。 */
const kindLabel = computed(() => (pdf.value ? "PDF" : isAttachableMime(props.mime) ? "IMG" : "?"));
</script>

<template>
  <span class="chip" :class="{ error: !!problem }" :title="problem ?? `${name} (${mime})`">
    <span class="kind" :class="{ pdf }">{{ kindLabel }}</span>
    <span class="name">{{ name }}</span>
    <span class="size">{{ problem ?? formatSize(size) }}</span>
    <button v-if="removable" class="remove" :title="removeTitle" @click="$emit('remove')">×</button>
  </span>
</template>

<style scoped>
.chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 100%;
  padding: 3px 8px;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 6px;
  font-size: 11px;
  color: var(--ui-text);
}
.chip.error {
  background: var(--err-bg);
  border-color: var(--err-fg);
}
.kind {
  flex: none;
  width: 22px;
  height: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 3px;
  background: var(--sel-blue);
  color: var(--acad-blue);
  font-family: var(--mono-font);
  font-size: 8px;
  font-weight: 700;
}
.kind.pdf {
  background: var(--err-bg);
  color: var(--err-fg);
}
.name {
  flex: none;
  max-width: 160px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.size {
  flex: none;
  font-family: var(--mono-font);
  font-size: 10px;
  color: var(--ui-muted);
}
/* 添付できない理由は名前の後ろに出し、長ければ省略 (全文はホバー) */
.chip.error .size {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--err-fg);
  font-family: inherit;
}
.remove {
  flex: none;
  border: none;
  background: transparent;
  padding: 0 0 0 2px;
  font-size: 13px;
  line-height: 1;
  color: var(--ui-muted);
  cursor: pointer;
}
.remove:hover {
  color: var(--ui-text);
}
</style>
