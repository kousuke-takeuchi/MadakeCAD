<script setup lang="ts">
// リボン (Pencilデザイン準拠)。タブとグループ構成はAutoCAD Electricalの慣習に合わせる。
import {
  Activity, AlignJustify, Cable, Circle, Columns3, Copy, Cpu, File, FileClock, FileDown, FileInput,
  FileOutput, FilePlus, FileSpreadsheet, FileText, FolderOpen, FolderPlus, Frame, Grid3x3, Hash,
  Image, LayoutGrid, LayoutPanelTop, LayoutTemplate, ListOrdered, ListChecks, Minus,
  Move, MoveRight, Network, Package, Pencil, Route, Rows3, Ruler, Save, Scissors, Shapes, ShieldCheck,
  Spline, Square, SquareDashed,
  Table2, Tag, Trash2, Type,
  type LucideIcon,
} from "lucide-vue-next";
import { computed, inject } from "vue";
import { useI18n } from "vue-i18n";
import {
  panelToolsAllowed,
  renumberPanelItemsCommand,
  schematicToolsAllowed,
  type PanelToolId,
} from "../canvas/panelTools";
import { footprints } from "../canvas/panel";
import { SHAPE_TOOLS, type ShapeToolId } from "../canvas/shapes";
import { VIEW_CLASSES, type ViewClass } from "../canvas/viewClasses";
import type { ReportFormat, ReportKind } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { useFileActions } from "../composables/fileActions";
import { fileNameOf, useProjectFileStore } from "../stores/projectFile";
import { useRevisionsStore } from "../stores/revisions";
import { useProjectInfoStore } from "../stores/projectInfo";
import { useSheetPaperStore } from "../stores/sheetPaper";
import { usePdfBookStore, useReportDialogStore } from "../stores/reports";
import { useMacrosStore } from "../stores/macros";
import { useTemplatesStore } from "../stores/templates";
import { useFrameTemplatesStore } from "../stores/frameTemplates";
import { useSymbolEditorStore } from "../stores/symbolEditor";
import { usePlcIoStore } from "../stores/plcIo";
import { useTerminalsStore } from "../stores/terminals";
import { useWireNumbersStore } from "../stores/wireNumbers";
import { useSimulationStore } from "../stores/simulation";
import { useVerificationStore } from "../stores/verification";
import type { EditorController } from "../tools/controller";
import { useUiStore, type RibbonTab } from "../stores/ui";
import { smallColumns } from "./ribbonLayout";

const controller = inject<EditorController>("controller")!;
const ui = useUiStore();
const files = useFileActions();
const projectFile = useProjectFileStore();
const verification = useVerificationStore();
const simulation = useSimulationStore();
const revisions = useRevisionsStore();
const projectInfo = useProjectInfoStore();
const sheetPaper = useSheetPaperStore();
const wireNumbers = useWireNumbersStore();
const { t } = useI18n();

const reportDialog = useReportDialogStore();
const pdfBook = usePdfBookStore();
const terminals = useTerminalsStore();
const plcIo = usePlcIoStore();
const templates = useTemplatesStore();
const frameTemplates = useFrameTemplatesStore();
const macros = useMacrosStore();
const symbolEditor = useSymbolEditorStore();

/** タブのid (表示名はi18nカタログ)。表示中のタブは ui ストアが持つ。 */
type TabId = RibbonTab;
const tabs: TabId[] = ["home", "project", "schematic", "panel", "report", "io", "view", "admin"];
const activeTab = computed({
  get: () => ui.ribbonTab,
  set: (tab: TabId) => ui.setRibbonTab(tab),
});

/** 表示中のシートが盤レイアウトシートか (盤ツールが使えるか)。 */
const onPanelSheet = computed(() => panelToolsAllowed(useDocumentStore().activeSheet));
/** 表示中のシートが回路図シートか (配線・部品挿入などが使えるか)。 */
const onSchematicSheet = computed(() => schematicToolsAllowed(useDocumentStore().activeSheet));

// 表示タブ: 表示クラストグル (レイヤ、spec §4)。3個ずつの縦列に分ける
const viewIcons: Record<ViewClass, LucideIcon> = {
  wires: Route, symbols: Cpu, refs: Tag, net_labels: Hash, wire_numbers: ListOrdered,
  wire_notes: Ruler, harness: SquareDashed, shapes: Shapes, panel: LayoutPanelTop, texts: Type,
  frame: Frame, grid: Grid3x3,
};
const viewClassColumns = Array.from(
  { length: Math.ceil(VIEW_CLASSES.length / 3) },
  (_, i) => VIEW_CLASSES.slice(i * 3, i * 3 + 3),
);

async function runSimulation() {
  const doc = useDocumentStore();
  await simulation.run(doc.activeSheetId);
  if (simulation.error) ui.log(t("ribbon.simFailedLog", { error: simulation.error }));
  else ui.log(t("ribbon.simDoneLog", { count: simulation.result?.components.length ?? 0 }));
}

async function runVerification() {
  await verification.run(null);
  const c = verification.counts;
  ui.log(t("ribbon.verifyDoneLog", c));
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

/** プロジェクト情報ダイアログを開く (G16)。プロジェクト名・番号・表題欄の一括設定。 */
function openProjectInfo() {
  projectInfo.openDialog();
  ui.log(t("projectInfo.openLog"));
}

/** 用紙変更ダイアログを開く (G15)。 */
function openSheetPaper() {
  const sheet = useDocumentStore().activeSheet;
  if (!sheet) {
    ui.log(t("sheetPaper.noSheetLog"));
    return;
  }
  sheetPaper.openFor(sheet);
  ui.log(t("sheetPaper.openLog", { sheet: sheet.name }));
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

/**
 * 部品挿入ダイアログを開く (リボン「回路図」タブ)。
 * 盤レイアウトシートでは回路図の部品を置けないので、理由をログに出して開かない。
 */
function openSymbolPicker() {
  if (!onSchematicSheet.value) {
    ui.log(t("panel.schematicToolLog"));
    return;
  }
  ui.symbolPickerOpen = true;
}

/**
 * 図形の作図ツールに切り替える (リボン「図形」グループ、G10)。
 * 筐体の囲み・強調枠はこれで描く (ハーネス境界は電線をまとめる用途に限る = G11)。
 */
function startShape(tool: ShapeToolId) {
  if (!onSchematicSheet.value) {
    ui.log(t("panel.schematicToolLog"));
    return;
  }
  controller.setTool(tool);
  ui.log(t(`shapes.startLog.${SHAPE_TOOLS[tool]}`));
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

/**
 * 電線品番凡例ブロックの挿入モードに入る (リボン「配線」グループ、G9残)。回路図シートを
 * クリックすると表が乗る。中身は描画のたびにそのシートの配線から導出する。
 */
function startWireLegend() {
  if (!onSchematicSheet.value) {
    ui.log(t("panel.schematicToolLog"));
    return;
  }
  controller.setTool("wire_legend");
  ui.log(t("wireLegend.startLog"));
}

function todo(name: string) {
  ui.log(t("ribbon.todoLog", { name }));
}

/** 未実装の小ボタン (淡く出したまま、押すとログに理由を出す)。 */
function todoItem(label: string, icon: LucideIcon): RibbonItem {
  return {
    label,
    icon,
    action: () => todo(label),
    isDisabled: () => true,
    title: t("panel.todoPhase"),
  };
}

/**
 * 盤の作図ツールへ切り替える (リボン「パネル」タブ>盤グループ)。
 * 盤レイアウトシート以外では切り替わらず、理由をログに出す (ツール側のガード)。
 */
function startPanelTool(tool: PanelToolId) {
  const sheet = useDocumentStore().activeSheet;
  if (!sheet) {
    ui.log(t("panel.noSheetLog"));
    return;
  }
  controller.setTool(tool);
  if (controller.tool === tool) ui.log(t(`panel.toolLog.${tool}`));
}

/**
 * バルーン採番 (リボン「パネル」タブ>配置)。表示中の盤シートのフットプリントへ
 * 読み順で 1..N のアイテム番号を振る (Command 1回なのでundo一発で戻せる)。
 */
async function renumberPanelItems() {
  const sheet = useDocumentStore().activeSheet;
  if (!sheet || !onPanelSheet.value) {
    ui.log(t("panel.panelToolLog"));
    return;
  }
  const command = renumberPanelItemsCommand(sheet);
  if (!command) {
    ui.log(t("panel.balloonEmptyLog"));
    return;
  }
  await useDocumentStore().execute(command);
  ui.log(t("panel.balloonDoneLog", { count: footprints(sheet).length }));
}

/**
 * 「間隔チェック」= 図面検証の実行 (リボン「パネル」タブ>配置)。盤の重なり
 * (`panel.overlap`)・吊り配置 (`panel.off_rail`)・レール充填率 (`panel.rail_fill`) は
 * プロジェクト全体の検証に含まれるので、結果は下部の検証パネルに並ぶ。
 */
async function runPanelChecks() {
  await verification.run(null);
  const c = verification.counts;
  ui.log(t("panel.clearanceDoneLog", { error: c.error, warning: c.warning, info: c.info }));
}

/**
 * 凡例表の挿入モードに入る (リボン「パネル」タブ>帳票)。盤面をクリックすると
 * そこへ表が乗る。中身は描画のたびにシートのフットプリントから導出する。
 */
function startPanelLegend() {
  const sheet = useDocumentStore().activeSheet;
  if (!sheet) {
    ui.log(t("panel.noSheetLog"));
    return;
  }
  controller.setTool("panel_legend");
  if (controller.tool === "panel_legend") ui.log(t("panel.toolLog.panel_legend"));
}

/** 左ドックの「未配置部品」ビューを開く (リボン「パネル」タブ>フットプリントの大ボタン)。 */
function openUnplaced() {
  if (!onPanelSheet.value) {
    ui.log(t("panel.panelToolLog"));
    return;
  }
  ui.setLeftPanelTab("panel");
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
 * 図枠テンプレートの選択ダイアログを開く (リボン「管理」タブ>テンプレート、
 * および「プロジェクト」タブ>作図の開始の小ボタン。IAどおりの2導線)。
 */
async function openFrameTemplates() {
  await frameTemplates.openDialog();
  ui.log(t("frameTemplates.openLog"));
}

/**
 * シンボルエディタを開く (リボン「管理」タブ>ライブラリの大ボタン)。
 * 既定の動線が「既存シンボルを複製して開始」なので、白紙の下書きで開いたうえで
 * 複製元のポップオーバーを出しておく (空から描き始めたければ閉じるだけでよい)。
 */
async function openSymbolEditor() {
  await symbolEditor.openBlank();
  symbolEditor.openPicker();
  ui.log(t("symbolEditor.openLog"));
}

/** ユーザーシンボルの置き場をファイラで開く (リボン「管理」タブ>ライブラリの小ボタン)。 */
async function openSymbolsFolder() {
  const { path, opened } = await symbolEditor.openUserFolder();
  ui.log(t(opened ? "symbolEditor.folderOpenedLog" : "symbolEditor.folderHintLog", { path }));
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
 *
 * DXFの読み込みで配線レイヤが決まらなかったときだけ、「配線レイヤ」グループが増える:
 * 読み込み報告が挙げたレイヤ名がそのまま小ボタンになり、押すとそのレイヤだけを配線と
 * して同じファイルを読み直す (配線レイヤの指定を打ち込まずに絞れる)。
 */
const ioGroups = computed<RibbonGroup[]>(() => {
  const groups: RibbonGroup[] = [
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
  ];
  const lastDxf = projectFile.lastDxf;
  if (lastDxf) {
    groups.push({
      name: t("io.wireLayerGroup"),
      small: smallColumns(
        lastDxf.layers.map((layer) => ({
          label: layer,
          icon: Cable,
          action: () => files.reimportDxfWithWireLayer(layer),
        })),
      ),
    });
  }
  return groups;
});

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
        { label: t("frameTemplates.ribbonButton"), icon: Frame, action: () => openFrameTemplates() },
        { label: t("templates.ribbonUserFolder"), icon: FolderPlus, action: () => openUserFolder() },
      ],
    ],
  },
]);

/**
 * 「管理」タブ (.pen「M4デザイン - リボン パネル/レポート/管理タブ」)。
 * デザインどおりライブラリ→テンプレートの順に並べる (プロジェクト・設定グループは
 * 中身が他タブ・設定ダイアログにあるため、実装済みの2グループだけを出す)。
 */
const adminGroups = computed<RibbonGroup[]>(() => [
  {
    name: t("ribbon.adminGroup.library"),
    big: {
      label: t("symbolEditor.ribbonButton"),
      icon: Shapes,
      color: "var(--acad-blue)",
      action: () => openSymbolEditor(),
    },
    small: [
      [
        { label: t("ribbon.adminSmall.partsDb"), icon: Package, action: () => todo(t("ribbon.adminSmall.partsDb")) },
        { label: t("ribbon.adminSmall.macros"), icon: Copy, action: () => todo(t("ribbon.adminSmall.macros")) },
        { label: t("ribbon.adminSmall.userLibrary"), icon: FolderPlus, action: () => openSymbolsFolder() },
      ],
    ],
  },
  {
    name: t("ribbon.adminGroup.templates"),
    big: {
      label: t("frameTemplates.ribbonButton"),
      icon: Frame,
      color: "var(--icon-edit)",
      action: () => openFrameTemplates(),
    },
    small: [
      [
        {
          label: t("frameTemplates.ribbonTitleBlock"),
          icon: Table2,
          action: () => todo(t("frameTemplates.ribbonTitleBlock")),
        },
        { label: t("templates.ribbonButton"), icon: LayoutTemplate, action: () => openTemplates() },
      ],
    ],
  },
]);

interface RibbonItem {
  label: string;
  icon: LucideIcon;
  action: () => void;
  isActive?: () => boolean;
  /**
   * 使えない操作 (まだ実装していない・いまのシートでは使えない)。
   * デザイン原則5「未実装機能もUIに見せ、操作時に未実装をログする」に従い、
   * **見た目だけを淡くし、押すと理由をログに出す**。`disabled`属性・`aria-disabled`は
   * 付けない: 押しても何も起きないボタンにすると「なぜ使えないか」を伝えられないため。
   */
  isDisabled?: () => boolean;
  /** 同じ表示名が並びうるとき(最近使ったファイル)の識別子。既定はlabel。 */
  key?: string;
  /** ツールチップ (未実装のフェーズ・使えない理由、フルパスなどの補足)。 */
  title?: string;
}
interface RibbonGroup {
  name: string;
  /** 大ボタン。無いグループ(最近使ったファイル)は小ボタンの列だけを並べる。 */
  big?: RibbonItem & { color: string };
  /** 小ボタンの列(リボンの縦列)。 */
  small: RibbonItem[][];
}

/**
 * 「パネル」タブ (.pen「M4デザイン - リボン パネル/レポート/管理タブ」)。
 * 盤レイアウトシートを開くと自動でこのタブになる (`ui.syncSheetKind`)。
 * Task 7で実装する操作 (バルーン採番・間隔チェック・盤の帳票) と、フェーズ6の
 * フットプリント作り込みは**淡いdisabled表示のまま並べ、押すと未実装をログする**。
 */
const panelGroups = computed<RibbonGroup[]>(() => [
  {
    name: t("ribbon.panelGroup.enclosure"),
    big: {
      label: t("panel.ribbonCreate"),
      icon: LayoutPanelTop,
      color: "var(--icon-edit)",
      action: () => startPanelTool("panel_outline"),
      isActive: () => controller.tool === "panel_outline",
      isDisabled: () => !onPanelSheet.value,
    },
    small: [
      [
        {
          label: t("panel.tool.panel_outline"),
          icon: LayoutPanelTop,
          action: () => startPanelTool("panel_outline"),
          isActive: () => controller.tool === "panel_outline",
          isDisabled: () => !onPanelSheet.value,
        },
        {
          label: t("panel.tool.din_rail"),
          icon: Rows3,
          action: () => startPanelTool("din_rail"),
          isActive: () => controller.tool === "din_rail",
          isDisabled: () => !onPanelSheet.value,
        },
        {
          label: t("panel.tool.duct"),
          icon: Columns3,
          action: () => startPanelTool("duct"),
          isActive: () => controller.tool === "duct",
          isDisabled: () => !onPanelSheet.value,
        },
      ],
    ],
  },
  {
    name: t("ribbon.panelGroup.footprint"),
    big: {
      label: t("panel.ribbonUnplaced"),
      icon: Package,
      color: "var(--acad-blue)",
      action: () => openUnplaced(),
      isDisabled: () => !onPanelSheet.value,
    },
    small: [
      [
        todoItem(t("ribbon.panelSmall.insertByPart"), Cpu),
        todoItem(t("ribbon.panelSmall.lookupEdit"), Table2),
        todoItem(t("ribbon.panelSmall.nameplate"), Tag),
      ],
    ],
  },
  {
    name: t("ribbon.panelGroup.placement"),
    big: {
      label: t("ribbon.panelSmall.balloon"),
      icon: Hash,
      color: "var(--ribbon-icon)",
      action: () => void renumberPanelItems(),
      isDisabled: () => !onPanelSheet.value,
    },
    small: [
      [
        todoItem(t("ribbon.panelSmall.align"), AlignJustify),
        todoItem(t("ribbon.panelSmall.distribute"), Move),
        todoItem(t("ribbon.panelSmall.alignRail"), Rows3),
      ],
      [
        todoItem(t("ribbon.panelSmall.dimension"), Ruler),
        {
          // 「間隔チェック」= 検証の実行。盤の重なり・吊り配置・レール充填率が検証結果に出る
          label: t("ribbon.panelSmall.clearance"),
          icon: ListChecks,
          action: () => void runPanelChecks(),
        },
      ],
    ],
  },
  {
    name: t("ribbon.panelGroup.reports"),
    big: {
      label: t("panel.ribbonLayoutTable"),
      icon: Table2,
      color: "var(--ok-fg)",
      action: () => void openReport("panel-placement"),
    },
    small: [
      [
        todoItem(t("ribbon.panelSmall.panelWiring"), Route),
        {
          label: t("ribbon.panelSmall.legend"),
          icon: FileText,
          action: () => startPanelLegend(),
          isActive: () => controller.tool === "panel_legend",
          isDisabled: () => !onPanelSheet.value,
        },
      ],
    ],
  },
]);

// 回路図のツールは盤レイアウトシートでは使えない (M4 §8)。押すと理由をログに出す
const schematicOnly = () => !onSchematicSheet.value;

const groups = computed<RibbonGroup[]>(() => [
  {
    name: t("ribbon.schematicGroup.wiring"),
    big: {
      label: t("ribbon.buttons.wire"),
      icon: Route,
      color: "var(--icon-wire)",
      action: () => controller.setTool("wire"),
      isActive: () => controller.tool === "wire",
      isDisabled: schematicOnly,
    },
    // 5つ目 (電線品番凡例) は1列に入らないので、smallColumnsで2列目へ送る
    // (1列に積むとリボンの下端からはみ出し、下の画面に隠れて押せなくなる)
    small: smallColumns([
      {
        label: t("ribbon.buttons.multiBus"),
        icon: AlignJustify,
        action: () => todo(t("ribbon.buttons.multiBus")),
        isDisabled: schematicOnly,
      },
      {
        label: t("wireNumbers.ribbonButton"),
        icon: ListOrdered,
        action: () => openWireNumbers(),
        isDisabled: schematicOnly,
      },
      {
        label: t("ribbon.buttons.signalArrow"),
        icon: MoveRight,
        action: () => todo(t("ribbon.buttons.signalArrow")),
        isDisabled: schematicOnly,
      },
      {
        label: t("harness.ribbonButton"),
        icon: SquareDashed,
        action: () => startHarness(),
        isActive: () => controller.tool === "harness",
        isDisabled: schematicOnly,
        title: t("harness.ribbonHint"),
      },
      {
        label: t("wireLegend.ribbonButton"),
        icon: Table2,
        action: () => startWireLegend(),
        isActive: () => controller.tool === "wire_legend",
        isDisabled: schematicOnly,
        title: t("wireLegend.ribbonHint"),
      },
    ]),
  },
  {
    name: t("ribbon.schematicGroup.insert"),
    big: {
      label: t("ribbon.buttons.insertPart"),
      icon: Cpu,
      color: "var(--acad-blue)",
      action: () => openSymbolPicker(),
      isActive: () => controller.tool === "place",
      isDisabled: schematicOnly,
    },
    small: [[
      {
        label: t("ribbon.buttons.terminalBlock"),
        icon: LayoutGrid,
        action: () => openSymbolPicker(),
        isDisabled: schematicOnly,
      },
      {
        label: t("templates.ribbonButton"),
        icon: LayoutTemplate,
        action: () => openTemplates(),
        isDisabled: schematicOnly,
      },
      {
        label: t("macros.ribbonSave"),
        icon: Copy,
        action: () => saveMacro(),
        isDisabled: schematicOnly,
      },
    ]],
  },
  {
    name: t("ribbon.schematicGroup.edit"),
    big: {
      label: t("ribbon.buttons.edit"),
      icon: Pencil,
      color: "var(--icon-edit)",
      action: () => controller.setTool("select"),
      isActive: () => controller.tool === "select",
    },
    small: [
      [
        { label: t("ribbon.buttons.move"), icon: Move, action: () => { controller.setTool("select"); ui.log(t("ribbon.moveLog")); } },
        { label: t("ribbon.buttons.trim"), icon: Scissors, action: () => todo(t("ribbon.buttons.trim")) },
        { label: t("ribbon.buttons.delete"), icon: Trash2, action: () => controller.deleteSelection() },
      ],
      [
        { label: t("revisions.ribbonButton"), icon: FileClock, action: () => openRevisions() },
        { label: t("projectInfo.ribbonButton"), icon: FileText, action: () => openProjectInfo() },
        { label: t("sheetPaper.ribbonButton"), icon: LayoutTemplate, action: () => openSheetPaper() },
      ],
    ],
  },
  {
    name: t("shapes.ribbonGroup"),
    big: {
      label: t("shapes.tool.rect"),
      icon: Square,
      color: "var(--icon-edit)",
      action: () => startShape("shape_rect"),
      isActive: () => controller.tool === "shape_rect",
      isDisabled: schematicOnly,
      title: t("shapes.ribbonHint"),
    },
    small: [[
      {
        label: t("shapes.tool.line"),
        icon: Minus,
        action: () => startShape("shape_line"),
        isActive: () => controller.tool === "shape_line",
        isDisabled: schematicOnly,
      },
      {
        label: t("shapes.tool.circle"),
        icon: Circle,
        action: () => startShape("shape_circle"),
        isActive: () => controller.tool === "shape_circle",
        isDisabled: schematicOnly,
      },
      {
        label: t("shapes.tool.polyline"),
        icon: Spline,
        action: () => startShape("shape_polyline"),
        isActive: () => controller.tool === "shape_polyline",
        isDisabled: schematicOnly,
        title: t("shapes.polylineHint"),
      },
    ]],
  },
  {
    name: t("ribbon.schematicGroup.verify"),
    big: {
      label: t("ribbon.buttons.verify"),
      icon: ShieldCheck,
      color: "var(--ok-fg)",
      action: () => runVerification(),
    },
    small: [
      [
        { label: t("ribbon.buttons.bom"), icon: FileText, action: () => files.exportBom() },
        { label: t("ribbon.buttons.wireList"), icon: Cable, action: () => files.exportWireList() },
        { label: t("ribbon.buttons.simulation"), icon: Activity, action: () => runSimulation() },
      ],
      [
        { label: t("ribbon.buttons.svg"), icon: Image, action: () => files.exportSvg() },
        { label: t("ribbon.buttons.pdf"), icon: FileDown, action: () => files.exportPdf() },
      ],
    ],
  },
]);

/** 表示中のタブのグループ。まだ実装していないタブはnull (プレースホルダを出す)。 */
const activeGroups = computed<RibbonGroup[] | null>(() => {
  if (activeTab.value === "schematic") return groups.value;
  if (activeTab.value === "panel") return panelGroups.value;
  if (activeTab.value === "home") return homeGroups.value;
  if (activeTab.value === "io") return ioGroups.value;
  if (activeTab.value === "project") return projectGroups.value;
  if (activeTab.value === "report") return reportGroups.value;
  if (activeTab.value === "admin") return adminGroups.value;
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
                :class="{ active: g.big.isActive?.(), disabled: g.big.isDisabled?.() }"
                :title="g.big.title"
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
                  :class="{ active: s.isActive?.(), disabled: s.isDisabled?.() }"
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
                :title="c.hintKey ? t(c.hintKey) : undefined"
                @click="ui.toggleViewClass(c.id)"
              >
                <component :is="viewIcons[c.id]" :size="13" class="small-icon" />
                {{ t(c.labelKey) }}
              </button>
            </div>
          </div>
          <div class="ribbon-group-label">{{ t("view.groupLabel") }} ▾</div>
        </div>
      </template>
      <div v-else class="ribbon-placeholder">
        {{ t("ribbon.placeholder", { tab: t(`ribbon.tab.${activeTab}`) }) }}
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
/* 使えない操作 (未実装・このシートでは使えない): 淡く出したまま押せる (押すと理由をログ) */
.ribbon-big.disabled,
.ribbon-small.disabled { color: var(--off-fg); }
.ribbon-big.disabled :deep(svg),
.ribbon-small.disabled :deep(svg) { color: var(--off-fg); opacity: 0.5; }
.small-icon { color: var(--ribbon-icon); flex: none; }
.view-toggle { color: var(--ui-muted); }
.view-toggle .small-icon { color: var(--ui-muted); }
.view-toggle.on { background: var(--sel-blue); color: var(--ui-text); }
.view-toggle.on .small-icon { color: var(--ribbon-icon); }
</style>
