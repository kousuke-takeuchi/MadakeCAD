<script setup lang="ts">
// リボン (Pencilデザイン準拠)。タブとグループ構成はAutoCAD Electricalの慣習に合わせる。
import {
  AlignJustify, Cable, Copy, Cpu, FileText, Hash, Image, LayoutGrid, Move, MoveRight,
  Pencil, Route, Scissors, ShieldCheck, Trash2, type LucideIcon,
} from "lucide-vue-next";
import { inject, ref } from "vue";
import { useFileActions } from "../composables/fileActions";
import type { EditorController } from "../tools/controller";
import { useUiStore } from "../stores/ui";

const controller = inject<EditorController>("controller")!;
const ui = useUiStore();
const files = useFileActions();

const activeTab = ref("回路図");
const tabs = ["ホーム", "プロジェクト", "回路図", "パネル", "レポート", "読み込み/書き出し", "表示", "管理"];

function todo(name: string) {
  ui.log(`${name}: 未実装 (今後のフェーズで対応予定)`);
}

interface RibbonItem {
  label: string;
  icon: LucideIcon;
  action: () => void;
  isActive?: () => boolean;
}
interface RibbonGroup {
  name: string;
  big: RibbonItem & { color: string };
  small: RibbonItem[];
}

const groups: RibbonGroup[] = [
  {
    name: "配線",
    big: {
      label: "配線",
      icon: Route,
      color: "#c0392b",
      action: () => controller.setTool("wire"),
      isActive: () => controller.tool === "wire",
    },
    small: [
      { label: "複数母線", icon: AlignJustify, action: () => todo("複数母線") },
      { label: "線番挿入", icon: Hash, action: () => todo("線番挿入") },
      { label: "信号矢印", icon: MoveRight, action: () => todo("信号矢印") },
    ],
  },
  {
    name: "部品を挿入",
    big: {
      label: "部品挿入",
      icon: Cpu,
      color: "#1f6fbf",
      action: () => (ui.symbolPickerOpen = true),
      isActive: () => controller.tool === "place",
    },
    small: [
      { label: "端子台", icon: LayoutGrid, action: () => todo("端子台") },
      { label: "回路コピー", icon: Copy, action: () => todo("回路コピー") },
    ],
  },
  {
    name: "回路図を編集",
    big: {
      label: "編集",
      icon: Pencil,
      color: "#b7791f",
      action: () => controller.setTool("select"),
      isActive: () => controller.tool === "select",
    },
    small: [
      { label: "移動", icon: Move, action: () => { controller.setTool("select"); ui.log("移動: 選択してドラッグ (グリッドスナップ)"); } },
      { label: "トリム", icon: Scissors, action: () => todo("トリム") },
      { label: "削除", icon: Trash2, action: () => controller.deleteSelection() },
    ],
  },
  {
    name: "検証/レポート",
    big: {
      label: "検証",
      icon: ShieldCheck,
      color: "#1f8a4c",
      action: () => todo("検証 (ERC)"),
    },
    small: [
      { label: "部品表", icon: FileText, action: () => files.exportBom() },
      { label: "電線リスト", icon: Cable, action: () => files.exportWireList() },
      { label: "SVG出力", icon: Image, action: () => files.exportSvg() },
    ],
  },
];
</script>

<template>
  <div class="ribbon">
    <div class="ribbon-tabs">
      <button
        v-for="t in tabs"
        :key="t"
        class="ribbon-tab"
        :class="{ active: t === activeTab }"
        @click="activeTab = t"
      >
        {{ t }}
      </button>
    </div>
    <div class="ribbon-body">
      <template v-if="activeTab === '回路図'">
        <template v-for="(g, i) in groups" :key="g.name">
          <div v-if="i > 0" class="ribbon-sep" />
          <div class="ribbon-group">
            <div class="ribbon-group-body">
              <button
                class="ribbon-big"
                :class="{ active: g.big.isActive?.() }"
                @click="g.big.action()"
              >
                <component :is="g.big.icon" :size="26" :color="g.big.color" />
                <span>{{ g.big.label }}</span>
              </button>
              <div class="ribbon-smalls">
                <button v-for="s in g.small" :key="s.label" class="ribbon-small" @click="s.action()">
                  <component :is="s.icon" :size="13" class="small-icon" />
                  {{ s.label }}
                </button>
              </div>
            </div>
            <div class="ribbon-group-label">{{ g.name }} ▾</div>
          </div>
        </template>
      </template>
      <div v-else class="ribbon-placeholder">「{{ activeTab }}」タブは今後のフェーズで実装予定です</div>
    </div>
  </div>
</template>

<style scoped>
.ribbon {
  background: var(--ribbon-bg);
  border-bottom: 1px solid var(--ribbon-line);
  user-select: none;
  flex: none;
}
.ribbon-tabs {
  display: flex;
  gap: 0;
  padding: 2px 10px 0;
  background: var(--ribbon-strip);
}
.ribbon-tab {
  border: none;
  background: transparent;
  padding: 5px 13px;
  font-size: 12px;
  color: var(--ui-text);
  border-radius: 4px 4px 0 0;
  cursor: pointer;
}
.ribbon-tab.active {
  background: var(--ribbon-bg);
  color: var(--acad-blue);
  font-weight: 700;
}
.ribbon-body {
  display: flex;
  align-items: stretch;
  padding: 4px 6px 2px;
  height: 96px;
}
.ribbon-placeholder {
  display: flex;
  align-items: center;
  padding: 0 16px;
  font-size: 11px;
  color: var(--ui-muted);
}
.ribbon-group {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 2px 6px 0;
}
.ribbon-group-body {
  display: flex;
  gap: 4px;
  align-items: flex-start;
  flex: 1;
}
.ribbon-group-label {
  font-size: 9px;
  color: var(--ribbon-label);
  padding: 1px 4px;
}
.ribbon-sep {
  width: 1px;
  background: var(--ribbon-line);
  margin: 4px 2px;
}
.ribbon-big {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  border: none;
  background: transparent;
  padding: 6px 8px;
  font-size: 10px;
  color: var(--ui-text);
  border-radius: 3px;
  cursor: pointer;
}
.ribbon-big:hover { background: var(--hover-bg); }
.ribbon-big.active { background: var(--sel-blue); }
.ribbon-smalls {
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding-top: 3px;
}
.ribbon-small {
  display: flex;
  align-items: center;
  gap: 5px;
  border: none;
  background: transparent;
  text-align: left;
  font-size: 10px;
  color: var(--ui-text);
  padding: 3px 6px;
  border-radius: 2px;
  cursor: pointer;
}
.ribbon-small:hover { background: var(--hover-bg); }
.small-icon { color: #4a6fa5; flex: none; }
</style>
