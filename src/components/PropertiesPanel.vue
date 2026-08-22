<script setup lang="ts">
// プロパティパレット (Pencilデザイン準拠): セクション見出し + ラベル列(灰)/値列(白)のグリッド。
import { ChevronDown, Pin } from "lucide-vue-next";
import { computed, inject, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { BLOCK_SYMBOL_ID } from "../canvas/dynamicSymbol";
import { harnessWireCount } from "../canvas/harness";
import { type NetSite, xrefJumpTarget, xrefSites } from "../canvas/xref";
import type { Entity } from "../ipc";
import { useBlockEditorStore } from "../stores/blockEditor";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";
import type { EditorController } from "../tools/controller";
import { harnessUpdateCommand, wireNumberCommand } from "./propertyCommands";

const store = useDocumentStore();
const ui = useUiStore();
const blockEditor = useBlockEditorStore();
const { t } = useI18n();
const controller = inject<EditorController>("controller")!;

const selected = computed<Entity | null>(() => {
  const sheet = store.activeSheet;
  if (!sheet || store.selection.size !== 1) return null;
  const [id] = [...store.selection];
  return sheet.entities[id] ?? null;
});

const wireColors = [
  ["red", "赤"], ["black", "黒"], ["white", "白"], ["blue", "青"],
  ["yellow", "黄"], ["green", "緑"], ["orange", "橙"], ["purple", "紫"],
  ["brown", "茶"], ["gray", "灰"], ["pink", "桃"], ["light_blue", "水色"],
] as const;
const sqValues = [0.2, 0.3, 0.5, 0.75, 1.25, 2.0, 3.5, 5.5];

const buf = reactive({
  color: "red",
  sq: 0.75,
  part_no: "",
  length_m: "",
  /** 線番 (ネット単位。確定でset_wire_numbersコマンドを送る)。 */
  wire_no: "",
  reference: "",
  value: "",
  name: "",
  /** ハーネスの備考。 */
  note: "",
});

/**
 * 選択中のネットラベルの相手先 (同名ラベルが置かれている他シートの所在)。
 * 相手がいなければ空 (プロパティには「なし」と出す)。
 */
const xrefTargets = computed<NetSite[]>(() => {
  const sheet = store.activeSheet;
  const e = selected.value;
  if (!sheet || !store.project || e?.kind !== "net_label") return [];
  return xrefSites(store.project, sheet.id, e.name);
});

/** 相手先クリック: 相手のシートへ切り替えて、相手のラベルを選択+ズームする。 */
function jumpToXref(site: NetSite) {
  const { sheetId, entityIds } = xrefJumpTarget(site);
  if (store.activeSheetId !== sheetId) store.activeSheetId = sheetId;
  controller.reveal(entityIds);
  ui.log(t("xref.jumpLog", { address: site.address, sheet: site.sheet_name }));
}

/** 選択中のハーネスが囲んでいる電線の本数 (幾何学的な内包で決まる)。 */
const harnessWires = computed(() => {
  const sheet = store.activeSheet;
  const e = selected.value;
  return sheet && e?.kind === "harness" ? harnessWireCount(sheet, e.id) : 0;
});

watch(
  selected,
  (e) => {
    if (!e) return;
    if (e.kind === "wire") {
      buf.color = e.color;
      buf.sq = e.sq;
      buf.part_no = e.part_no ?? "";
      buf.length_m = e.length_m?.toString() ?? "";
      buf.wire_no = e.net ?? "";
    } else if (e.kind === "symbol") {
      buf.reference = e.reference;
      buf.value = e.value;
    } else if (e.kind === "net_label") {
      buf.name = e.name;
    } else if (e.kind === "harness") {
      buf.name = e.name ?? "";
      buf.note = e.note ?? "";
    }
  },
  { immediate: true },
);

async function apply() {
  const sheet = store.activeSheet;
  const e = selected.value;
  if (!sheet || !e) return;
  let entity: Entity;
  if (e.kind === "wire") {
    entity = {
      ...e,
      color: buf.color,
      sq: buf.sq,
      part_no: buf.part_no || null,
      length_m: buf.length_m === "" ? null : Number(buf.length_m),
    };
    // 線番はネット単位の属性なので、専用のset_wire_numbersコマンドで書き換える
    const numberCommand = wireNumberCommand(sheet.id, e, buf.wire_no);
    const otherChanged =
      entity.color !== e.color ||
      entity.sq !== e.sq ||
      entity.part_no !== e.part_no ||
      entity.length_m !== e.length_m;
    if (otherChanged) await store.execute({ type: "update_entity", sheet_id: sheet.id, entity });
    if (numberCommand) {
      await store.execute(numberCommand);
      const number = buf.wire_no.trim();
      ui.log(
        number
          ? t("wireNumbers.setLog", { number })
          : t("wireNumbers.clearedLog"),
      );
    }
    return;
  } else if (e.kind === "symbol") {
    entity = { ...e, reference: buf.reference, value: buf.value };
  } else if (e.kind === "net_label") {
    entity = { ...e, name: buf.name };
  } else if (e.kind === "harness") {
    // ハーネスは名前・備考だけを書き換える (囲みの形はキャンバス側の操作で変える)
    const command = harnessUpdateCommand(sheet.id, e, buf.name, buf.note);
    if (command) {
      await store.execute(command);
      ui.log(t("harness.renamedLog", { name: buf.name.trim() }));
    }
    return;
  } else {
    return;
  }
  await store.execute({ type: "update_entity", sheet_id: sheet.id, entity });
}

/** 選択中がブロック (ピン表で形が決まるシンボル) か。 */
const isBlock = computed(
  () => selected.value?.kind === "symbol" && selected.value.symbol_id === BLOCK_SYMBOL_ID,
);

/** 選択中のブロックのピン構成 (「左5 / 右3」)。 */
const blockPinCounts = computed(() => {
  const e = selected.value;
  if (e?.kind !== "symbol") return "";
  const def = store.resolveInstance(e);
  if (!def) return t("blockEditor.broken");
  const count = (dir: string) => def.pins.filter((p) => p.dir === dir).length;
  return t("blockEditor.pinCounts", { left: count("left"), right: count("right") });
});

/** ピン表エディタを開く (確定でupdate_entityコマンド)。 */
function editBlockPins() {
  const e = selected.value;
  if (e?.kind === "symbol") blockEditor.startEdit(e);
}

const kindLabel = computed<Record<string, string>>(() => ({
  wire: "配線",
  symbol: "シンボル",
  junction: "ジャンクション",
  net_label: "ネットラベル",
  text: "テキスト",
  harness: t("harness.kindLabel"),
}));
</script>

<template>
  <aside class="panel">
    <div class="panel-head">
      <span>プロパティ</span>
      <Pin :size="11" class="muted" />
    </div>
    <div class="type-row">
      <span v-if="selected">{{ kindLabel[selected.kind] }} (1)</span>
      <span v-else-if="store.selection.size > 1">{{ store.selection.size }} 個選択</span>
      <span v-else class="muted">選択なし</span>
      <ChevronDown :size="11" class="muted" />
    </div>

    <template v-if="selected?.kind === 'wire'">
      <div class="sec"><ChevronDown :size="10" /> 一般</div>
      <div class="prow">
        <span class="plabel">色</span>
        <span class="pvalue">
          <span class="swatch" :data-color="buf.color" />
          <select v-model="buf.color" class="bare">
            <option v-for="[v, label] in wireColors" :key="v" :value="v">{{ label }} ({{ v }})</option>
          </select>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">線種</span>
        <span class="pvalue">Continuous</span>
      </div>
      <div class="sec"><ChevronDown :size="10" /> 電気属性</div>
      <div class="prow">
        <span class="plabel">線径</span>
        <span class="pvalue">
          <select v-model.number="buf.sq" class="bare">
            <option v-for="v in sqValues" :key="v" :value="v">{{ v }} sq</option>
          </select>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">電線品番</span>
        <span class="pvalue"><input v-model="buf.part_no" class="bare" placeholder="SAMPLE0001" /></span>
      </div>
      <div class="prow">
        <span class="plabel">長さ m</span>
        <span class="pvalue"><input v-model="buf.length_m" class="bare" placeholder="0.4" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("wireNumbers.propertyLabel") }}</span>
        <span class="pvalue">
          <input
            v-model="buf.wire_no"
            class="bare mono"
            :title="t('wireNumbers.propertyHint')"
            placeholder="1"
            spellcheck="false"
            @keyup.enter="apply"
          />
        </span>
      </div>
    </template>

    <template v-else-if="selected?.kind === 'symbol'">
      <div class="sec"><ChevronDown :size="10" /> 識別</div>
      <div class="prow">
        <span class="plabel">参照記号</span>
        <span class="pvalue"><input v-model="buf.reference" class="bare" /></span>
      </div>
      <div class="prow">
        <span class="plabel">型番/値</span>
        <span class="pvalue"><input v-model="buf.value" class="bare" placeholder="[redacted]" /></span>
      </div>
      <div class="prow">
        <span class="plabel">シンボル</span>
        <span class="pvalue">{{ selected.symbol_id }}</span>
      </div>
      <template v-if="isBlock">
        <div class="prow">
          <span class="plabel">{{ t("blockEditor.pinsRow") }}</span>
          <span class="pvalue mono">{{ blockPinCounts }}</span>
        </div>
        <div class="apply-row">
          <button class="secondary" :title="t('blockEditor.editHint')" @click="editBlockPins">
            {{ t("blockEditor.editPins") }}
          </button>
        </div>
      </template>
    </template>

    <template v-else-if="selected?.kind === 'harness'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("harness.kindLabel") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("harness.kindRow") }}</span>
        <span class="pvalue">{{ t("harness.kindLabel") }}</span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("harness.nameRow") }}</span>
        <span class="pvalue">
          <input
            v-model="buf.name"
            class="bare mono"
            :title="t('harness.nameHint')"
            placeholder="W1"
            spellcheck="false"
            @keyup.enter="apply"
          />
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("harness.wireCountRow") }}</span>
        <span class="pvalue stacked">
          <span>{{ t("harness.wireCount", { count: harnessWires }) }}</span>
          <span class="hint">
            {{
              buf.name.trim()
                ? t("harness.wireCountHint", { name: buf.name.trim() })
                : t("harness.wireCountHintUnnamed")
            }}
          </span>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("harness.noteRow") }}</span>
        <span class="pvalue">
          <input
            v-model="buf.note"
            class="bare"
            :placeholder="t('harness.notePlaceholder')"
            @keyup.enter="apply"
          />
        </span>
      </div>
    </template>

    <template v-else-if="selected?.kind === 'net_label'">
      <div class="sec"><ChevronDown :size="10" /> ネット</div>
      <div class="prow">
        <span class="plabel">ネット名</span>
        <span class="pvalue"><input v-model="buf.name" class="bare" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("xref.propertyLabel") }}</span>
        <span class="pvalue stacked" :title="t('xref.propertyHint')">
          <template v-if="xrefTargets.length">
            <button
              v-for="site in xrefTargets"
              :key="site.label_id"
              class="xref-link"
              :title="site.sheet_name"
              @click="jumpToXref(site)"
            >
              {{ site.address }}
            </button>
          </template>
          <template v-else>
            <span class="muted">{{ t("xref.none") }}</span>
            <span class="hint">{{ t("xref.noneHint") }}</span>
          </template>
        </span>
      </div>
    </template>

    <div v-if="selected" class="apply-row">
      <button class="apply" @click="apply">適用</button>
    </div>
  </aside>
</template>

<style scoped>
.panel {
  width: 240px;
  flex: none;
  background: var(--palette-bg);
  border-left: 1px solid var(--ribbon-line);
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  user-select: none;
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px 9px;
  background: var(--palette-head);
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.type-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px 9px;
  font-size: 11px;
  color: var(--ui-text);
  border-bottom: 1px solid var(--ribbon-line);
}
.muted { color: var(--ui-muted); }
.sec {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 9px;
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-text);
  background: var(--hover-bg);
}
.prow {
  display: flex;
  border-bottom: 1px solid var(--ribbon-line);
  font-size: 10px;
}
.plabel {
  width: 88px;
  flex: none;
  padding: 5px 9px;
  color: var(--ui-muted);
}
.pvalue {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 8px;
  background: #fff;
  color: var(--ui-text);
}
.bare {
  width: 100%;
  border: none;
  outline: none;
  background: transparent;
  font-size: 10px;
  color: var(--ui-text);
  padding: 2px 0;
}
.mono { font-family: var(--mono-font); }
.hint { color: var(--ui-muted); }
.pvalue.stacked {
  flex-direction: column;
  align-items: flex-start;
  gap: 1px;
}
/* 参照リンク値 (XRef相手先「/2.B3」): acad-blue mono 600、クリックでジャンプ */
.xref-link {
  border: 0;
  background: none;
  padding: 0;
  font: 600 10px var(--mono-font);
  color: var(--acad-blue);
  cursor: pointer;
}
.xref-link:hover { text-decoration: underline; }
.swatch {
  width: 11px;
  height: 11px;
  border-radius: 2px;
  flex: none;
  background: #c00000;
}
.swatch[data-color="black"] { background: #202020; }
.swatch[data-color="white"] { background: #f0f0f0; border: 1px solid #ccc; }
.swatch[data-color="blue"] { background: #0000c0; }
.swatch[data-color="yellow"] { background: #c8a800; }
.swatch[data-color="green"] { background: #008040; }
.swatch[data-color="orange"] { background: #d07010; }
.swatch[data-color="purple"] { background: #8020a0; }
.swatch[data-color="brown"] { background: #805020; }
.swatch[data-color="gray"] { background: #808080; }
.swatch[data-color="pink"] { background: #d06090; }
.swatch[data-color="light_blue"] { background: #2090c0; }
.apply-row {
  display: flex;
  justify-content: flex-end;
  padding: 10px;
}
/* セカンダリ操作 (ピン表を編集…): 白+枠 */
.secondary {
  background: var(--card-bg);
  color: var(--ui-text);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 5px 12px;
  font-size: 11px;
  cursor: pointer;
}
.secondary:hover { background: var(--hover-bg); }
.apply {
  background: var(--acad-blue);
  color: #fff;
  border: none;
  border-radius: 4px;
  padding: 5px 16px;
  font-size: 12px;
  cursor: pointer;
}
</style>
