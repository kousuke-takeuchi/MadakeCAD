<script setup lang="ts">
import { onBeforeUnmount, onMounted, provide, reactive } from "vue";
import { useFileActions } from "../composables/fileActions";
import { fileShortcutOf, type FileShortcut } from "./editorShortcuts";
import { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";
import CanvasView from "./CanvasView.vue";
import SearchResultsPanel from "./SearchResultsPanel.vue";
import SimulationPanel from "./SimulationPanel.vue";
import VerificationPanel from "./VerificationPanel.vue";
import VariantsPanel from "./VariantsPanel.vue";
import FileTabs from "./FileTabs.vue";
import LeftPanel from "./LeftPanel.vue";
import PropertiesPanel from "./PropertiesPanel.vue";
import RibbonBar from "./RibbonBar.vue";
import StatusBar from "./StatusBar.vue";
import RevisionsDialog from "./RevisionsDialog.vue";
import ProjectInfoDialog from "./ProjectInfoDialog.vue";
import SheetPaperDialog from "./SheetPaperDialog.vue";
import WireNumberDialog from "./WireNumberDialog.vue";
import TerminalEditorDialog from "./TerminalEditorDialog.vue";
import PlcIoDialog from "./PlcIoDialog.vue";
import PlcGenerateDialog from "./PlcGenerateDialog.vue";
import ReportDialog from "./ReportDialog.vue";
import PdfBookDialog from "./PdfBookDialog.vue";
import TemplateDialog from "./TemplateDialog.vue";
import FrameTemplateDialog from "./FrameTemplateDialog.vue";
import MacroSaveDialog from "./MacroSaveDialog.vue";
import SymbolPickerDialog from "./SymbolPickerDialog.vue";
import SymbolEditorDialog from "./SymbolEditorDialog.vue";
import BlockEditorDialog from "./BlockEditorDialog.vue";
import TitleBar from "./TitleBar.vue";
import SettingsDialog from "./settings/SettingsDialog.vue";
import { useRevisionsStore } from "../stores/revisions";
import { useProjectInfoStore } from "../stores/projectInfo";
import { useSheetPaperStore } from "../stores/sheetPaper";
import { useWireNumbersStore } from "../stores/wireNumbers";
import { useTerminalsStore } from "../stores/terminals";
import { usePlcIoStore } from "../stores/plcIo";
import { usePdfBookStore, useReportDialogStore } from "../stores/reports";
import { useMacrosStore } from "../stores/macros";
import { useSymbolEditorStore } from "../stores/symbolEditor";
import { useBlockEditorStore } from "../stores/blockEditor";
import { useSearchStore } from "../stores/search";

const store = useDocumentStore();
const ui = useUiStore();
const search = useSearchStore();
const revisions = useRevisionsStore();
const projectInfo = useProjectInfoStore();
const sheetPaper = useSheetPaperStore();
const wireNumbers = useWireNumbersStore();
const terminals = useTerminalsStore();
const plcIo = usePlcIoStore();
const reportDialog = useReportDialogStore();
const pdfBook = usePdfBookStore();
const macros = useMacrosStore();
const symbolEditor = useSymbolEditorStore();
const blockEditor = useBlockEditorStore();
const controller = reactive(new EditorController(store)) as EditorController;
provide("controller", controller);
const files = useFileActions();

/**
 * モーダル (ダイアログ・エディタ) が開いているか。
 * 開いている間はキャンバスのショートカットもファイル操作のショートカットも走らせない
 * (モーダルの下でドキュメントが差し替わるのを防ぐ)。
 */
function anyModalOpen(): boolean {
  return (
    ui.settingsOpen ||
    ui.symbolPickerOpen ||
    revisions.open ||
    projectInfo.open ||
    sheetPaper.open ||
    wireNumbers.open ||
    terminals.open ||
    plcIo.open ||
    reportDialog.open ||
    pdfBook.open ||
    macros.saveOpen ||
    symbolEditor.open ||
    blockEditor.active
  );
}

/** ショートカットの種類から実際のファイル操作へ。 */
const fileActionOf: Record<FileShortcut, () => Promise<boolean>> = {
  new: () => files.newProject(),
  open: () => files.openProject(),
  save: () => files.saveProject(),
  saveAs: () => files.saveProjectAs(),
};

function isEditableTarget(ev: KeyboardEvent) {
  const t = ev.target as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
}

async function onKeyDown(ev: KeyboardEvent) {
  // ⌘F: 浮き検索バー。入力欄に居ても効かせる (検索バー自身の入力欄から開き直せるように)
  if ((ev.metaKey || ev.ctrlKey) && ev.key.toLowerCase() === "f") {
    ev.preventDefault();
    search.openBar();
    return;
  }
  // ファイル操作 (⌘N/⌘O/⌘S/⇧⌘S)。入力欄に居ても効かせる (保存はどこからでも) が、
  // モーダル中は効かせない (開いたままのモーダルの下で図面が差し替わるのを防ぐ)
  const shortcut = fileShortcutOf(ev, anyModalOpen());
  if (shortcut) {
    ev.preventDefault();
    void fileActionOf[shortcut]();
    return;
  }
  if (ev.key === "Escape" && ui.settingsOpen) {
    ui.settingsOpen = false;
    ev.preventDefault();
    return;
  }
  // ピン表エディタは部品挿入ダイアログの中にも出るので、先にこちらを閉じる
  if (ev.key === "Escape" && blockEditor.active) {
    blockEditor.close();
    ev.preventDefault();
    return;
  }
  if (ev.key === "Escape" && ui.symbolPickerOpen) {
    ui.symbolPickerOpen = false;
    ev.preventDefault();
    return;
  }
  if (ev.key === "Escape" && revisions.open) {
    revisions.cancel();
    ev.preventDefault();
    return;
  }
  if (ev.key === "Escape" && projectInfo.open) {
    projectInfo.cancel();
    ev.preventDefault();
    return;
  }
  if (ev.key === "Escape" && sheetPaper.open) {
    sheetPaper.cancel();
    ev.preventDefault();
    return;
  }
  if (ev.key === "Escape" && wireNumbers.open) {
    wireNumbers.cancel();
    ev.preventDefault();
    return;
  }
  if (ev.key === "Escape" && terminals.open) {
    terminals.close();
    ev.preventDefault();
    return;
  }
  // 生成設定は割付表エディタの上に重なるので、先に生成設定だけ閉じる
  if (ev.key === "Escape" && plcIo.generateOpen) {
    plcIo.cancelGenerate();
    ev.preventDefault();
    return;
  }
  if (ev.key === "Escape" && plcIo.open) {
    plcIo.close();
    ev.preventDefault();
    return;
  }
  if (ev.key === "Escape" && reportDialog.open) {
    reportDialog.cancel();
    ev.preventDefault();
    return;
  }
  if (ev.key === "Escape" && pdfBook.open) {
    pdfBook.cancel();
    ev.preventDefault();
    return;
  }
  if (ev.key === "Escape" && macros.saveOpen) {
    macros.cancelSave();
    ev.preventDefault();
    return;
  }
  // シンボルエディタ: ポップオーバー→ダイアログの順に閉じ、Deleteで選択中の図形を消す
  if (symbolEditor.open) {
    if (ev.key === "Escape") {
      if (symbolEditor.pickerOpen) symbolEditor.closePicker();
      else symbolEditor.requestClose();
      ev.preventDefault();
      return;
    }
    if ((ev.key === "Delete" || ev.key === "Backspace") && !isEditableTarget(ev)) {
      symbolEditor.deleteSelection();
      ev.preventDefault();
      return;
    }
  }
  if (isEditableTarget(ev)) return;
  // モーダル表示中はキャンバスのショートカットを走らせない
  if (anyModalOpen()) return;
  if (await controller.onKeyDown(ev)) ev.preventDefault();
}
function onKeyUp(ev: KeyboardEvent) {
  if (isEditableTarget(ev)) return;
  controller.onKeyUp(ev);
}

onMounted(() => {
  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("keyup", onKeyUp);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeyDown);
  window.removeEventListener("keyup", onKeyUp);
});
</script>

<template>
  <div class="editor">
    <TitleBar @new="files.newProject()" @open="files.openProject()" @save="files.saveProject()" />
    <RibbonBar />
    <div class="main-row">
      <LeftPanel />
      <!-- 図面タブはキャンバスの切り替え用なので、キャンバス直上に置く -->
      <div class="center-col">
        <FileTabs />
        <CanvasView />
        <SimulationPanel />
        <VerificationPanel />
        <VariantsPanel />
        <SearchResultsPanel />
      </div>
      <PropertiesPanel />
    </div>
    <StatusBar />
    <SymbolPickerDialog />
    <SettingsDialog />
    <RevisionsDialog />
    <ProjectInfoDialog />
    <SheetPaperDialog />
    <WireNumberDialog />
    <TerminalEditorDialog />
    <PlcIoDialog />
    <PlcGenerateDialog />
    <ReportDialog />
    <PdfBookDialog />
    <TemplateDialog />
    <FrameTemplateDialog />
    <MacroSaveDialog />
    <SymbolEditorDialog />
    <BlockEditorDialog />
  </div>
</template>

<style scoped>
.editor {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}
.main-row {
  display: flex;
  flex: 1;
  min-height: 0;
}
.center-col {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
}
</style>
