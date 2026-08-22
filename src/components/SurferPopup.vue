<script setup lang="ts">
// 参照サーフィン (Surfer) ポップアップ (デザイン: .pen「M4デザイン - 検索/デバイスナビゲータ/Surfer」)。
// キャンバス上の要素をAlt(Option)+クリックすると、同じデバイス・同じネット・同じ線番が
// 図面のどこに出てくるかを一覧する。↑↓で巡回、Enterでジャンプ、Escで閉じる。
import { onBeforeUnmount, watch } from "vue";
import { useI18n } from "vue-i18n";
import { functionLabel } from "../canvas/search";
import { surferJumpTarget, type SurferSite } from "../canvas/surfer";
import { useReveal } from "../composables/reveal";
import { useSurferStore } from "../stores/surfer";
import { useUiStore } from "../stores/ui";

const { t } = useI18n();
const surfer = useSurferStore();
const ui = useUiStore();
const reveal = useReveal();

const TITLE_KEYS = {
  device: "surfer.titleDevice",
  net: "surfer.titleNet",
  wire_no: "surfer.titleWireNo",
} as const;

function siteLabel(site: SurferSite): string {
  if (!site.function) return `${site.sheetName} · ${site.zone}`;
  const spec = functionLabel(site.function, site.terminals);
  return t(spec.key, spec.params ?? {});
}

function jump(site: SurferSite | null, close: boolean) {
  if (!site) return;
  reveal(surferJumpTarget(site));
  ui.log(t("surfer.jumpLog", { title: surfer.target?.title ?? "", address: site.address }));
  if (close) surfer.close();
}

function onKeydown(event: KeyboardEvent) {
  if (!surfer.open) return;
  // ポップアップが開いている間は↑↓/Enter/Escを消費する (キャンバスのツールへ流さない)
  const handled = ["Escape", "ArrowDown", "ArrowUp", "Enter"].includes(event.key);
  if (!handled) return;
  event.preventDefault();
  event.stopPropagation();
  switch (event.key) {
    case "Escape":
      surfer.close();
      break;
    case "ArrowDown":
      jump(surfer.step(1), false);
      break;
    case "ArrowUp":
      jump(surfer.step(-1), false);
      break;
    case "Enter":
      jump(surfer.activeSite, true);
      break;
  }
}

watch(
  () => surfer.open,
  (open) => {
    if (open) window.addEventListener("keydown", onKeydown, { capture: true });
    else window.removeEventListener("keydown", onKeydown, { capture: true });
  },
);
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown, { capture: true }));
</script>

<template>
  <template v-if="surfer.open && surfer.target">
    <div class="backdrop" @click="surfer.close()" @contextmenu.prevent="surfer.close()" />
    <div class="popup" :style="{ left: `${surfer.at.x}px`, top: `${surfer.at.y}px` }">
      <div class="head">
        {{ t(TITLE_KEYS[surfer.target.kind], { title: surfer.target.title }) }}
      </div>
      <button
        v-for="(site, i) in surfer.sites"
        :key="`${site.entityId}-${i}`"
        class="row"
        :class="{ on: i === surfer.index }"
        @click="jump(surfer.select(i), true)"
      >
        <span class="name">{{ siteLabel(site) }}</span>
        <span class="spacer" />
        <span class="addr">{{ site.address }}</span>
      </button>
      <div class="hint">{{ t("surfer.hint") }}</div>
    </div>
  </template>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 24;
}
/* デザイン: 幅320・白・角丸12・枠+shadow-popup */
.popup {
  position: fixed;
  z-index: 25;
  width: 320px;
  display: flex;
  flex-direction: column;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 12px;
  box-shadow: var(--shadow-popup);
  overflow: hidden;
}
.head {
  padding: 8px 12px 4px;
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-placeholder);
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: none;
  background: transparent;
  text-align: left;
  padding: 6px 12px;
  cursor: pointer;
}
.row:hover {
  background: var(--hover-bg);
}
.row.on {
  background: var(--sel-blue);
}
.name {
  font-size: 11px;
  color: var(--ui-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.spacer {
  flex: 1;
}
.addr {
  flex: none;
  font-family: var(--mono-font);
  font-size: 10px;
  font-weight: 600;
  color: var(--acad-blue);
}
.hint {
  padding: 4px 12px 8px;
  font-size: 10px;
  color: var(--ui-muted);
}
</style>
