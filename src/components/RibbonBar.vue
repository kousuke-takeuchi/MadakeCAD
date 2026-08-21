<script setup lang="ts">
// リボン (Pencilデザイン準拠)。タブとグループ構成はAutoCAD Electricalの慣習に合わせる。
import {
  Activity, AlignJustify, Cable, Copy, Cpu, FileClock, FileDown, FileText, Frame, Grid3x3, Hash,
  Image, LayoutGrid, Move, MoveRight, Pencil, Route, Scissors, ShieldCheck, Tag, Trash2, Type,
  type LucideIcon,
} from "lucide-vue-next";
import { computed, inject, ref } from "vue";
import { useI18n } from "vue-i18n";
import { VIEW_CLASSES, type ViewClass } from "../canvas/viewClasses";
import { useDocumentStore } from "../stores/document";
import { useFileActions } from "../composables/fileActions";
import { useRevisionsStore } from "../stores/revisions";
import { useSimulationStore } from "../stores/simulation";
import { useVerificationStore } from "../stores/verification";
import type { EditorController } from "../tools/controller";
import { useUiStore } from "../stores/ui";

const controller = inject<EditorController>("controller")!;
const ui = useUiStore();
const files = useFileActions();
const verification = useVerificationStore();
const simulation = useSimulationStore();
const revisions = useRevisionsStore();
const { t } = useI18n();

const activeTab = ref("回路図");

// 表示タブ: 表示クラストグル (レイヤ、spec §4)。3個ずつの縦列に分ける
const viewIcons: Record<ViewClass, LucideIcon> = {
  wires: Route, symbols: Cpu, refs: Tag, net_labels: Hash, texts: Type, frame: Frame, grid: Grid3x3,
};
const viewClassColumns = Array.from(
  { length: Math.ceil(VIEW_CLASSES.length / 3) },
  (_, i) => VIEW_CLASSES.slice(i * 3, i * 3 + 3),
);
const tabs = ["ホーム", "プロジェクト", "回路図", "パネル", "レポート", "読み込み/書き出し", "表示", "管理"];

async function runSimulation() {
  const doc = useDocumentStore();
  await simulation.run(doc.activeSheetId);
  if (simulation.error) ui.log(`SIM  失敗: ${simulation.error}`);
  else ui.log(`SIM  DC動作点を計算しました (部品 ${simulation.result?.components.length ?? 0}件)`);
}

async function runVerification() {
  await verification.run(null);
  const c = verification.counts;
  ui.log(`検証完了: エラー ${c.error} / 警告 ${c.warning} / 情報 ${c.info}`);
}

/**
 * 改訂欄編集ダイアログを開く (IAでは「プロジェクト」タブ>シート>改訂欄。
 * 同タブは未実装のため、実装済みの「回路図」タブの編集グループから起動する)。
 */
function openRevisions() {
  const sheet = useDocumentStore().activeSheet;
  if (!sheet) {
    ui.log(t("revisions.noSheetLog"));
    return;
  }
  revisions.openFor(sheet);
  ui.log(t("revisions.openLog", { sheet: sheet.name }));
}

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
  /** 小ボタンの列(リボンの縦列)。 */
  small: RibbonItem[][];
}

const groups = computed<RibbonGroup[]>(() => [
  {
    name: "配線",
    big: {
      label: "配線",
      icon: Route,
      color: "var(--icon-wire)",
      action: () => controller.setTool("wire"),
      isActive: () => controller.tool === "wire",
    },
    small: [[
      { label: "複数母線", icon: AlignJustify, action: () => todo("複数母線") },
      { label: "線番挿入", icon: Hash, action: () => todo("線番挿入") },
      { label: "信号矢印", icon: MoveRight, action: () => todo("信号矢印") },
    ]],
  },
  {
    name: "部品を挿入",
    big: {
      label: "部品挿入",
      icon: Cpu,
      color: "var(--acad-blue)",
      action: () => (ui.symbolPickerOpen = true),
      isActive: () => controller.tool === "place",
    },
    small: [[
      { label: "端子台", icon: LayoutGrid, action: () => (ui.symbolPickerOpen = true) },
      { label: "回路コピー", icon: Copy, action: () => todo("回路コピー") },
    ]],
  },
  {
    name: "回路図を編集",
    big: {
      label: "編集",
      icon: Pencil,
      color: "var(--icon-edit)",
      action: () => controller.setTool("select"),
      isActive: () => controller.tool === "select",
    },
    small: [
      [
        { label: "移動", icon: Move, action: () => { controller.setTool("select"); ui.log("移動: 選択してドラッグ (グリッドスナップ)"); } },
        { label: "トリム", icon: Scissors, action: () => todo("トリム") },
        { label: "削除", icon: Trash2, action: () => controller.deleteSelection() },
      ],
      [
        { label: t("revisions.ribbonButton"), icon: FileClock, action: () => openRevisions() },
      ],
    ],
  },
  {
    name: "検証/レポート",
    big: {
      label: "検証",
      icon: ShieldCheck,
      color: "var(--ok-fg)",
      action: () => runVerification(),
    },
    small: [
      [
        { label: "部品表", icon: FileText, action: () => files.exportBom() },
        { label: "電線リスト", icon: Cable, action: () => files.exportWireList() },
        { label: "シミュレーション", icon: Activity, action: () => runSimulation() },
      ],
      [
        { label: "SVG出力", icon: Image, action: () => files.exportSvg() },
        { label: "PDF出力", icon: FileDown, action: () => files.exportPdf() },
      ],
    ],
  },
]);
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
              <div v-for="(col, ci) in g.small" :key="ci" class="ribbon-smalls">
                <button v-for="s in col" :key="s.label" class="ribbon-small" @click="s.action()">
                  <component :is="s.icon" :size="13" class="small-icon" />
                  {{ s.label }}
                </button>
              </div>
            </div>
            <div class="ribbon-group-label">{{ g.name }} ▾</div>
          </div>
        </template>
      </template>
      <template v-else-if="activeTab === '表示'">
        <div class="ribbon-group">
          <div class="ribbon-group-body">
            <div v-for="(col, ci) in viewClassColumns" :key="ci" class="ribbon-smalls">
              <button
                v-for="c in col"
                :key="c.id"
                class="ribbon-small view-toggle"
                :class="{ on: ui.isClassVisible(c.id) }"
                @click="ui.toggleViewClass(c.id)"
              >
                <component :is="viewIcons[c.id]" :size="13" class="small-icon" />
                {{ c.label }}
              </button>
            </div>
          </div>
          <div class="ribbon-group-label">表示クラス ▾</div>
        </div>
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
  font-size: 10px;
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
  border-radius: 4px;
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
  border-radius: 4px;
  cursor: pointer;
}
.ribbon-small:hover { background: var(--hover-bg); }
.small-icon { color: var(--ribbon-icon); flex: none; }
.view-toggle { color: var(--ui-muted); }
.view-toggle .small-icon { color: var(--ui-muted); }
.view-toggle.on { background: var(--sel-blue); color: var(--ui-text); }
.view-toggle.on .small-icon { color: var(--ribbon-icon); }
</style>
