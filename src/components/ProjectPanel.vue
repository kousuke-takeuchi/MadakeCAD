<script setup lang="ts">
// プロジェクトマネージャー (Pencilデザイン準拠): ツールバー + プロジェクトツリー + 詳細ペイン。
import {
  ChevronDown, File, Folder, FolderOpen, FolderPlus, Pin, Printer, RefreshCw, Settings, X,
} from "lucide-vue-next";
import { useFileActions } from "../composables/fileActions";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";

const store = useDocumentStore();
const ui = useUiStore();
const files = useFileActions();

async function addSheet() {
  await store.execute({
    type: "add_sheet",
    name: `Sheet${(store.project?.sheets.length ?? 0) + 1}`,
    size: "A3",
    orientation: "Landscape",
  });
  const sheets = store.project?.sheets ?? [];
  store.activeSheetId = sheets[sheets.length - 1]?.id ?? null;
  ui.log("NEWSHEET  シートを追加しました");
}

function todo(name: string) {
  ui.log(`${name}: 未実装`);
}

const toolButtons = [
  { icon: FolderPlus, title: "シートを追加", action: addSheet },
  { icon: FolderOpen, title: "プロジェクトを開く", action: () => files.openProject() },
  { icon: RefreshCw, title: "再読み込み", action: () => todo("再読み込み") },
  { icon: Printer, title: "印刷", action: () => todo("印刷") },
  { icon: Settings, title: "プロジェクト設定", action: () => todo("プロジェクト設定") },
];
</script>

<template>
  <aside class="panel">
    <div class="panel-head">
      <span>プロジェクト マネージャー</span>
      <span class="head-icons">
        <Pin :size="11" />
        <X :size="11" />
      </span>
    </div>
    <div class="toolbar">
      <button v-for="b in toolButtons" :key="b.title" :title="b.title" @click="b.action()">
        <component :is="b.icon" :size="12" />
      </button>
    </div>
    <div class="tree">
      <div class="project-row">
        <ChevronDown :size="11" class="muted" />
        <Folder :size="13" class="folder" />
        <span class="project-name">{{ store.project?.name ?? "無題" }}</span>
      </div>
      <button
        v-for="s in store.project?.sheets ?? []"
        :key="s.id"
        class="row"
        :class="{ active: s.id === store.activeSheet?.id }"
        @click="store.activeSheetId = s.id"
      >
        <File :size="12" :class="s.id === store.activeSheet?.id ? 'file-active' : 'muted'" />
        {{ s.name }}.mdkproj
      </button>
    </div>
    <div class="spacer" />
    <div class="detail-head">詳細</div>
    <div class="detail">
      <div class="detail-row"><span>図面</span><b>{{ store.activeSheet?.name ?? "-" }}</b></div>
      <div class="detail-row"><span>図番</span><b>{{ store.activeSheet?.title_block.drawing_no || "-" }}</b></div>
      <div class="detail-row">
        <span>用紙</span><b>{{ store.activeSheet?.size }} {{ store.activeSheet?.orientation === "Portrait" ? "縦" : "横" }}</b>
      </div>
      <div class="detail-row">
        <span>シート</span>
        <b>
          {{ (store.project?.sheets.findIndex((s) => s.id === store.activeSheet?.id) ?? 0) + 1 }} /
          {{ store.project?.sheets.length ?? 0 }}
        </b>
      </div>
      <div class="detail-row">
        <span>要素数</span><b>{{ Object.keys(store.activeSheet?.entities ?? {}).length }}</b>
      </div>
    </div>
  </aside>
</template>

<style scoped>
/* 左ドック (LeftPanel) のタブ内容。幅と右境界はドック側が持つ */
.panel {
  width: 100%;
  flex: 1;
  min-height: 0;
  background: var(--palette-bg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
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
.head-icons { display: flex; gap: 6px; color: var(--ui-muted); }
.toolbar {
  display: flex;
  gap: 3px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--ribbon-line);
}
.toolbar button {
  width: 22px;
  height: 20px;
  border: none;
  background: transparent;
  border-radius: 4px;
  color: var(--ribbon-icon);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}
.toolbar button:hover { background: var(--hover-bg); }
.tree { overflow-y: auto; }
.project-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 9px;
}
.project-name { font-size: 11px; font-weight: 700; color: var(--ui-text); }
.folder { color: #d9a422; }
.muted { color: var(--ui-muted); }
.file-active { color: var(--acad-blue); }
.row {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  border: none;
  background: transparent;
  text-align: left;
  padding: 3px 9px 3px 28px;
  font-size: 11px;
  color: var(--ui-text);
  cursor: pointer;
}
.row:hover { background: var(--hover-bg); }
.row.active { background: var(--sel-blue); }
.spacer { flex: 1; }
.detail-head {
  padding: 4px 9px;
  background: var(--palette-head);
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-text);
}
.detail { padding: 2px 0 8px; }
.detail-row {
  display: flex;
  gap: 8px;
  padding: 2px 9px;
  font-size: 10px;
  color: var(--ui-muted);
}
.detail-row span { width: 44px; flex: none; }
.detail-row b { color: var(--ui-text); font-weight: 500; }
</style>
