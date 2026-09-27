<script setup lang="ts">
// リボン (Pencilデザイン準拠)。タブとグループ構成はAutoCAD Electricalの慣習に合わせる。
import {
  Activity, AlignJustify, Cable, Copy, Cpu, File, FileClock, FileDown, FileInput, FileOutput, FilePlus,
  FileSpreadsheet, FileText, FolderOpen, FolderPlus, Frame, Grid3x3, Hash, Image, LayoutGrid,
  LayoutTemplate, ListOrdered, ListChecks, Move, MoveRight, Network, Pencil, Route, Save,
  Scissors, ShieldCheck, SquareDashed, Table2, Tag, Trash2, Type,
  type LucideIcon,
} from "lucide-vue-next";
import { computed, inject, ref } from "vue";
import { useI18n } from "vue-i18n";
import { VIEW_CLASSES, type ViewClass } from "../canvas/viewClasses";
import type { ReportFormat, ReportKind } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { useFileActions } from "../composables/fileActions";
import { fileNameOf, useProjectFileStore } from "../stores/projectFile";
import { useRevisionsStore } from "../stores/revisions";
import { usePdfBookStore, useReportDialogStore } from "../stores/reports";
import { useMacrosStore } from "../stores/macros";
import { useTemplatesStore } from "../stores/templates";
import { usePlcIoStore } from "../stores/plcIo";
import { useTerminalsStore } from "../stores/terminals";
import { useWireNumbersStore } from "../stores/wireNumbers";
import { useSimulationStore } from "../stores/simulation";
import { useVerificationStore } from "../stores/verification";
import type { EditorController } from "../tools/controller";
import { useUiStore } from "../stores/ui";

const controller = inject<EditorController>("controller")!;
const ui = useUiStore();
const files = useFileActions();
const projectFile = useProjectFileStore();
const verification = useVerificationStore();
const simulation = useSimulationStore();
const revisions = useRevisionsStore();
const wireNumbers = useWireNumbersStore();
const { t } = useI18n();

const reportDialog = useReportDialogStore();
const pdfBook = usePdfBookStore();
const terminals = useTerminalsStore();
const plcIo = usePlcIoStore();
const templates = useTemplatesStore();
const macros = useMacrosStore();

/** タブのid (表示名はi18nカタログ)。 */
type TabId = "home" | "project" | "schematic" | "panel" | "report" | "io" | "view" | "admin";
const tabs: TabId[] = ["home", "project", "schematic", "panel", "report", "io", "view", "admin"];
const activeTab = ref<TabId>("schematic");

// 表示タブ: 表示クラストグル (レイヤ、spec §4)。3個ずつの縦列に分ける
const viewIcons: Record<ViewClass, LucideIcon> = {
  wires: Route, symbols: Cpu, refs: Tag, net_labels: Hash, wire_numbers: ListOrdered,
  harness: SquareDashed, texts: Type, frame: Frame, grid: Grid3x3,
};
const viewClassColumns = Array.from(
  { length: Math.ceil(VIEW_CLASSES.length / 3) },
  (_, i) => VIEW_CLASSES.slice(i * 3, i * 3 + 3),
);

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

/** 線番の自動採番ダイアログを開く (リボン「配線」グループ)。 */
function openWireNumbers() {
  const sheet = useDocumentStore().activeSheet;
  if (!sheet) {
    ui.log(t("wireNumbers.noSheetLog"));
    return;
  }
  wireNumbers.openFor(sheet);
  ui.log(t("wireNumbers.openLog", { sheet: sheet.name }));
}

/** ハーネス境界の作図ツールに切り替える (リボン「配線」グループ)。矩形ドラッグで囲みを作る。 */
function startHarness() {
  if (!useDocumentStore().activeSheet) {
    ui.log(t("harness.noSheetLog"));
    return;
  }
  controller.setTool("harness");
  ui.log(t("harness.startLog"));
}

function todo(name: string) {
  ui.log(`${name}: 未実装 (今後のフェーズで対応予定)`);
}

/** 帳票生成ダイアログを開く (リボン「レポート」タブ>帳票グループ)。 */
async function openReport(kind: ReportKind, format?: ReportFormat) {
  await reportDialog.openFor(kind, format);
  ui.log(t("reports.openLog", { report: t(`reports.kind.${kind}`) }));
}

/**
 * テンプレート選択ダイアログを開く (リボン「プロジェクト」タブ>作図の開始、
 * および「回路図」タブ>部品を挿入の小ボタン)。
 * 自動では開かない: 空図面でも勝手にモーダルを出さず、この導線からだけ開く。
 */
async function openTemplates() {
  await templates.openDialog();
  ui.log(t("templates.openLog"));
}

/**
 * 選択範囲を回路マクロとして保存する (リボン「回路図」タブ>部品を挿入)。
 * 何も選択していなければダイアログを出さず、先に選ぶよう案内する。
 */
async function saveMacro() {
  const doc = useDocumentStore();
  const sheet = doc.activeSheet;
  if (!sheet || !(await macros.openSave(sheet.id, [...doc.selection]))) {
    ui.log(t("macros.noSelectionLog"));
  }
}

/** ユーザーテンプレートの置き場をファイラで開く (開けない環境ではパスを案内する)。 */
async function openUserFolder() {
  const { path, opened } = await templates.openUserFolder();
  ui.log(t(opened ? "templates.folderOpenedLog" : "templates.folderHintLog", { path }));
}

/** PDF一括出力ダイアログを開く (レポートタブ>出力グループ)。 */
function openPdfBook() {
  pdfBook.openDialog();
  ui.log(t("pdfBook.openLog"));
}

/** 端子台エディタを開く。`check`ならそのまま端子台チェックまで実行する。 */
async function openTerminalEditor(check = false) {
  const sheet = useDocumentStore().activeSheet;
  if (!sheet) {
    ui.log(t("terminals.noSheetLog"));
    return;
  }
  await terminals.openFor(sheet.id);
  ui.log(t("terminals.openLog", { sheet: sheet.name }));
  if (!terminals.entityId) {
    ui.log(t("terminals.noBlockLog"));
    return;
  }
  if (!check) return;
  await terminals.runCheck();
  const result = terminals.checkOk
    ? t("terminals.checkOk")
    : t("terminals.checkCounts", terminals.checkCounts);
  ui.log(t("terminals.checkLog", { reference: terminals.reference, result }));
}

/**
 * PLC I/O割付表エディタを開く。`generate`ならそのままI/O図面の生成設定まで開く。
 * IAでは「読み込み/書き出し」タブだが、同タブは未実装のため実装済みの
 * 「レポート」タブのPLC I/Oグループから起動する (端子台エディタと同じ扱い)。
 */
async function openPlcEditor(generate = false) {
  await plcIo.openEditor();
  ui.log(t("plcIo.openLog"));
  if (!plcIo.moduleRef) {
    ui.log(t("plcIo.noModuleLog"));
    return;
  }
  if (generate) openPlcGenerate();
}

/** I/O図面の生成設定ダイアログを開く (割付表エディタの上に重なる)。 */
function openPlcGenerate() {
  plcIo.openGenerate();
  ui.log(t("plcIo.generateOpenLog", { reference: plcIo.moduleRef ?? "" }));
}

/**
 * 「レポート」タブ (.pen「M4デザイン - リボン パネル/レポート/管理タブ」準拠)。
 * ケーブル一覧は帳票そのものが未実装なのでログのみ。
 * 電線リストはTask 1でFrom-Toリストへ統合したため1ボタンにまとめている。
 */
const reportGroups = computed<RibbonGroup[]>(() => [
  {
    name: t("ribbon.reportGroup.reports"),
    big: {
      label: t("reports.kind.terminal-chart"),
      icon: Table2,
      color: "var(--ok-fg)",
      action: () => openReport("terminal-chart"),
    },
    small: [
      [
        { label: t("reports.kind.bom"), icon: FileText, action: () => openReport("bom") },
        { label: t("reports.kind.wire-list"), icon: Cable, action: () => openReport("wire-list") },
        {
          label: t("reports.ribbonCableList"),
          icon: Cable,
          action: () => ui.log(t("reports.todoLog", { report: t("reports.ribbonCableList") })),
        },
      ],
      [
        { label: t("reports.kind.xref"), icon: Network, action: () => openReport("xref") },
        { label: t("reports.ribbonPlcIo"), icon: Cpu, action: () => openReport("plc-io") },
      ],
    ],
  },
  {
    name: t("ribbon.reportGroup.terminals"),
    big: {
      label: t("terminals.ribbonButton"),
      icon: LayoutGrid,
      color: "var(--acad-blue)",
      action: () => openTerminalEditor(),
    },
    small: [
      [
        {
          label: t("reports.kind.terminal-diagram"),
          icon: Route,
          action: () => openReport("terminal-diagram"),
        },
        { label: t("terminals.check"), icon: ListChecks, action: () => openTerminalEditor(true) },
      ],
    ],
  },
  {
    name: t("ribbon.reportGroup.plc"),
    big: {
      label: t("plcIo.ribbonButton"),
      icon: Cpu,
      color: "var(--icon-wire)",
      action: () => openPlcEditor(),
    },
    small: [
      [
        { label: t("plcIo.ribbonGenerate"), icon: Grid3x3, action: () => openPlcEditor(true) },
        { label: t("reports.ribbonPlcIo"), icon: FileSpreadsheet, action: () => openReport("plc-io") },
      ],
    ],
  },
  {
    name: t("ribbon.reportGroup.output"),
    big: {
      label: t("pdfBook.ribbonButton"),
      icon: FileDown,
      color: "var(--icon-edit)",
      action: () => openPdfBook(),
    },
    small: [
      [
        {
          label: t("reports.ribbonSheetify"),
          icon: Frame,
          action: () => openReport("wire-list", "pdf"),
        },
        {
          label: t("reports.ribbonCsv"),
          icon: FileSpreadsheet,
          action: () => openReport("wire-list", "csv"),
        },
        { label: t("reports.ribbonPickTargets"), icon: ListChecks, action: () => openPdfBook() },
      ],
    ],
  },
]);

/**
 * 「ホーム」タブ: プロジェクトファイルの新規/開く/保存/名前を付けて保存と、
 * 最近使ったファイル (3件ずつの縦列、ボタンの表示はファイル名・ツールチップにフルパス)。
 * ショートカットはタイトルバーのボタンと同じ (Cmd+N / Cmd+O / Cmd+S / Cmd+Shift+S)。
 */
const homeGroups = computed<RibbonGroup[]>(() => {
  const groups: RibbonGroup[] = [
    {
      name: t("file.group"),
      big: {
        label: t("file.save"),
        icon: Save,
        color: "var(--acad-blue)",
        action: () => files.saveProject(),
      },
      small: [
        [
          { label: t("file.new"), icon: FilePlus, action: () => files.newProject() },
          { label: t("file.open"), icon: FolderOpen, action: () => files.openProject() },
          { label: t("file.saveAs"), icon: FileOutput, action: () => files.saveProjectAs() },
        ],
      ],
    },
  ];
  const recent = projectFile.recent.map<RibbonItem>((path) => ({
    key: path,
    label: fileNameOf(path),
    title: path,
    icon: File,
    action: () => files.openRecent(path),
  }));
  if (recent.length) {
    groups.push({
      name: t("file.recentGroup"),
      small: Array.from({ length: Math.ceil(recent.length / 3) }, (_, i) =>
        recent.slice(i * 3, i * 3 + 3),
      ),
    });
  }
  return groups;
});

/**
 * 「読み込み/書き出し」タブ: 他CADとの受け渡し。KiCad回路図(.kicad_sch)と、
 * AutoCAD Electrical / EPLANの中間形式であるDXFの読み込み・書き出し、帳票CSV。
 * (ACADEの.dwg・EPLANの.elk/.zw1は非公開形式なので、両製品が読み書きできるDXFで受け渡す)
 */
const ioGroups = computed<RibbonGroup[]>(() => [
  {
    name: t("io.importGroup"),
    big: {
      label: t("io.open"),
      icon: FolderOpen,
      color: "var(--acad-blue)",
      action: () => files.openProject(),
    },
    small: [
      [
        { label: t("io.importKicad"), icon: FileInput, action: () => files.importFile("kicad") },
        { label: t("io.importDxf"), icon: FileInput, action: () => files.importFile("dxf") },
      ],
    ],
  },
  {
    name: t("io.exportGroup"),
    big: {
      label: t("io.exportDxf"),
      icon: FileOutput,
      color: "var(--icon-edit)",
      action: () => files.exportSheet("dxf"),
    },
    small: [
      [
        { label: t("io.exportKicad"), icon: FileOutput, action: () => files.exportSheet("kicad") },
        { label: t("io.exportSvg"), icon: Image, action: () => files.exportSheet("svg") },
        { label: t("io.exportPdf"), icon: FileDown, action: () => files.exportSheet("pdf") },
      ],
    ],
  },
  {
    name: t("io.reportGroup"),
    small: [
      [
        { label: t("io.exportBom"), icon: FileText, action: () => files.exportBom() },
        { label: t("io.exportWireList"), icon: Cable, action: () => files.exportWireList() },
      ],
    ],
  },
]);

/** 「プロジェクト」タブ: 作図を始めるための導線 (テンプレート)。 */
const projectGroups = computed<RibbonGroup[]>(() => [
  {
    name: t("ribbon.projectGroup.start"),
    big: {
      label: t("templates.ribbonButton"),
      icon: LayoutTemplate,
      color: "var(--acad-blue)",
      action: () => openTemplates(),
    },
    small: [
      [
        { label: t("templates.ribbonPick"), icon: LayoutGrid, action: () => openTemplates() },
        { label: t("templates.ribbonUserFolder"), icon: FolderPlus, action: () => openUserFolder() },
      ],
    ],
  },
]);

interface RibbonItem {
  label: string;
  icon: LucideIcon;
  action: () => void;
  isActive?: () => boolean;
  /** 同じ表示名が並びうるとき(最近使ったファイル)の識別子。既定はlabel。 */
  key?: string;
  /** ツールチップ (フルパスなど、ボタンに書ききれない補足)。 */
  title?: string;
}
interface RibbonGroup {
  name: string;
  /** 大ボタン。無いグループ(最近使ったファイル)は小ボタンの列だけを並べる。 */
  big?: RibbonItem & { color: string };
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
      { label: t("wireNumbers.ribbonButton"), icon: ListOrdered, action: () => openWireNumbers() },
      { label: "信号矢印", icon: MoveRight, action: () => todo("信号矢印") },
      {
        label: t("harness.ribbonButton"),
        icon: SquareDashed,
        action: () => startHarness(),
        isActive: () => controller.tool === "harness",
      },
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
      { label: t("templates.ribbonButton"), icon: LayoutTemplate, action: () => openTemplates() },
      { label: t("macros.ribbonSave"), icon: Copy, action: () => saveMacro() },
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

/** 表示中のタブのグループ。まだ実装していないタブはnull (プレースホルダを出す)。 */
const activeGroups = computed<RibbonGroup[] | null>(() => {
  if (activeTab.value === "schematic") return groups.value;
  if (activeTab.value === "home") return homeGroups.value;
  if (activeTab.value === "io") return ioGroups.value;
  if (activeTab.value === "project") return projectGroups.value;
  if (activeTab.value === "report") return reportGroups.value;
  return null;
});
</script>

<template>
  <div class="ribbon">
    <div class="ribbon-tabs">
      <button
        v-for="tab in tabs"
        :key="tab"
        class="ribbon-tab"
        :class="{ active: tab === activeTab }"
        @click="activeTab = tab"
      >
        {{ t(`ribbon.tab.${tab}`) }}
      </button>
    </div>
    <div class="ribbon-body">
      <template v-if="activeGroups">
        <template v-for="(g, i) in activeGroups" :key="g.name">
          <div v-if="i > 0" class="ribbon-sep" />
          <div class="ribbon-group">
            <div class="ribbon-group-body">
              <button
                v-if="g.big"
                class="ribbon-big"
                :class="{ active: g.big.isActive?.() }"
                @click="g.big.action()"
              >
                <component :is="g.big.icon" :size="26" :color="g.big.color" />
                <span>{{ g.big.label }}</span>
              </button>
              <div v-for="(col, ci) in g.small" :key="ci" class="ribbon-smalls">
                <button
                  v-for="s in col"
                  :key="s.key ?? s.label"
                  class="ribbon-small"
                  :class="{ active: s.isActive?.() }"
                  :title="s.title"
                  @click="s.action()"
                >
                  <component :is="s.icon" :size="13" class="small-icon" />
                  {{ s.label }}
                </button>
              </div>
            </div>
            <div class="ribbon-group-label">{{ g.name }} ▾</div>
          </div>
        </template>
      </template>
      <template v-else-if="activeTab === 'view'">
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
      <div v-else class="ribbon-placeholder">
        「{{ t(`ribbon.tab.${activeTab}`) }}」タブは今後のフェーズで実装予定です
      </div>
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
.ribbon-small.active { background: var(--sel-blue); }
.small-icon { color: var(--ribbon-icon); flex: none; }
.view-toggle { color: var(--ui-muted); }
.view-toggle .small-icon { color: var(--ui-muted); }
.view-toggle.on { background: var(--sel-blue); color: var(--ui-text); }
.view-toggle.on .small-icon { color: var(--ribbon-icon); }
</style>
