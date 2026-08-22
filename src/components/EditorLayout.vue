<script setup lang="ts">
import { onBeforeUnmount, onMounted, provide, reactive } from "vue";
import { useFileActions } from "../composables/fileActions";
import { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";
import CanvasView from "./CanvasView.vue";
import SearchResultsPanel from "./SearchResultsPanel.vue";
import SimulationPanel from "./SimulationPanel.vue";
import VerificationPanel from "./VerificationPanel.vue";
import FileTabs from "./FileTabs.vue";
import LeftPanel from "./LeftPanel.vue";
import PropertiesPanel from "./PropertiesPanel.vue";
import RibbonBar from "./RibbonBar.vue";
import StatusBar from "./StatusBar.vue";
import RevisionsDialog from "./RevisionsDialog.vue";
import WireNumberDialog from "./WireNumberDialog.vue";
import TerminalEditorDialog from "./TerminalEditorDialog.vue";
import ReportDialog from "./ReportDialog.vue";
import PdfBookDialog from "./PdfBookDialog.vue";
import TemplateDialog from "./TemplateDialog.vue";
import MacroSaveDialog from "./MacroSaveDialog.vue";
import SymbolPickerDialog from "./SymbolPickerDialog.vue";
import TitleBar from "./TitleBar.vue";
import SettingsDialog from "./settings/SettingsDialog.vue";
import { useRevisionsStore } from "../stores/revisions";
import { useWireNumbersStore } from "../stores/wireNumbers";
import { useTerminalsStore } from "../stores/terminals";
import { usePdfBookStore, useReportDialogStore } from "../stores/reports";
import { useMacrosStore } from "../stores/macros";
import { useSearchStore } from "../stores/search";

const store = useDocumentStore();
const ui = useUiStore();
const search = useSearchStore();
const revisions = useRevisionsStore();
const wireNumbers = useWireNumbersStore();
const terminals = useTerminalsStore();
const reportDialog = useReportDialogStore();
const pdfBook = usePdfBookStore();
const macros = useMacrosStore();
const controller = reactive(new EditorController(store)) as EditorController;
provide("controller", controller);
const files = useFileActions();

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
  if (ev.key === "Escape" && ui.settingsOpen) {
    ui.settingsOpen = false;
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
  if (isEditableTarget(ev)) return;
  // モーダル表示中はキャンバスのショートカットを走らせない
  if (
    revisions.open ||
    wireNumbers.open ||
    terminals.open ||
    reportDialog.open ||
    pdfBook.open ||
    macros.saveOpen
  ) {
    return;
  }
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
    <TitleBar @open="files.openProject()" @save="files.saveProject()" />
    <RibbonBar />
    <div class="main-row">
      <LeftPanel />
      <!-- 図面タブはキャンバスの切り替え用なので、キャンバス直上に置く -->
      <div class="center-col">
        <FileTabs />
        <CanvasView />
        <SimulationPanel />
        <VerificationPanel />
        <SearchResultsPanel />
      </div>
      <PropertiesPanel />
    </div>
    <StatusBar />
    <SymbolPickerDialog />
    <SettingsDialog />
    <RevisionsDialog />
    <WireNumberDialog />
    <TerminalEditorDialog />
    <ReportDialog />
    <PdfBookDialog />
    <TemplateDialog />
    <MacroSaveDialog />
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
