// ファイル操作 (開く/保存/エクスポート)。Tauri外ではprompt入力にフォールバックする。

import { open as dialogOpen, save as dialogSave } from "@tauri-apps/plugin-dialog";
import { inTauri, ipc } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";

interface Filter {
  name: string;
  extensions: string[];
}

async function pickSave(defaultPath: string, filters: Filter[]): Promise<string | null> {
  if (inTauri) return dialogSave({ defaultPath, filters });
  return window.prompt("保存先の絶対パス:", `/tmp/${defaultPath}`);
}

async function pickOpen(filters: Filter[]): Promise<string | null> {
  if (inTauri) {
    const r = await dialogOpen({ multiple: false, filters });
    return typeof r === "string" ? r : null;
  }
  return window.prompt("開くファイルの絶対パス:");
}

export function useFileActions() {
  const store = useDocumentStore();
  const ui = useUiStore();

  return {
    async openProject() {
      const path = await pickOpen([{ name: "MadakeCADプロジェクト", extensions: ["mdkproj"] }]);
      if (!path) return;
      const patch = await ipc.loadProject(path);
      store.applyPatch(patch);
      ui.log(`OPEN  ${path} を開きました`);
    },
    async saveProject() {
      const path = await pickSave(
        `${store.project?.name ?? "project"}.mdkproj`,
        [{ name: "MadakeCADプロジェクト", extensions: ["mdkproj"] }],
      );
      if (!path) return;
      await ipc.saveProject(path);
      ui.log(`SAVE  ${path} に保存しました`);
    },
    async exportSvg() {
      const sheet = store.activeSheet;
      if (!sheet) return;
      const path = await pickSave(`${sheet.name}.svg`, [{ name: "SVG", extensions: ["svg"] }]);
      if (!path) return;
      await ipc.exportSvg(sheet.id, path);
      ui.log(`EXPORT  SVGを ${path} に書き出しました`);
    },
    async exportBom() {
      const path = await pickSave("部品表.csv", [{ name: "CSV", extensions: ["csv"] }]);
      if (!path) return;
      await ipc.exportBom(path);
      ui.log(`EXPORT  部品表を ${path} に書き出しました`);
    },
    async exportWireList() {
      const path = await pickSave("電線リスト.csv", [{ name: "CSV", extensions: ["csv"] }]);
      if (!path) return;
      await ipc.exportWireList(path);
      ui.log(`EXPORT  電線リストを ${path} に書き出しました`);
    },
  };
}
