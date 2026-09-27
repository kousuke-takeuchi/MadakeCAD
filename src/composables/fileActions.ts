// ファイル操作 (新規/開く/保存/名前を付けて保存/エクスポート)。
// OSのダイアログはTauriのdialogプラグイン、Tauri外(ブラウザ検証)ではprompt入力にフォールバックする。
// ファイルの状態(パス・未保存・最近使ったファイル)はprojectFile storeが持ち、ここは
// ダイアログと確認、ログ出力だけを担当する。

import { open as dialogOpen, save as dialogSave } from "@tauri-apps/plugin-dialog";
import { i18n } from "../i18n";
import { inTauri, ipc } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { isDxfPath, isKicadPath, useProjectFileStore } from "../stores/projectFile";
import { useUiStore } from "../stores/ui";

interface Filter {
  name: string;
  extensions: string[];
}

/** ファイル操作が使うダイアログ群。テストではメモリ実装を差し込む。 */
export interface FileDialogs {
  pickSave(defaultPath: string, filters: Filter[]): Promise<string | null>;
  pickOpen(filters: Filter[]): Promise<string | null>;
  /** 未保存の編集を捨ててよいかの確認。trueで続行。 */
  confirmDiscard(message: string): boolean;
}

/**
 * 保存先をOSの保存ダイアログで選ぶ (Tauri外ではプロンプト入力)。
 * 帳票ダイアログの「参照...」もこれを使う (モーダルの上のモーダルにはならない)。
 */
export async function pickSave(defaultPath: string, filters: Filter[]): Promise<string | null> {
  if (inTauri) return dialogSave({ defaultPath, filters });
  return window.prompt(i18n.global.t("file.promptSave"), `/tmp/${defaultPath}`);
}

/** 読み込める図面形式 (プロジェクト以外)。 */
export type ImportKind = "kicad" | "dxf";
/** 書き出せる図面形式 (シート1枚)。 */
export type SheetExportKind = "svg" | "pdf" | "dxf" | "kicad";

/** 拡張子から保存ダイアログのフィルタを作る (csv / pdf / svg …)。 */
export function filterFor(extension: string): Filter[] {
  return [{ name: extension.toUpperCase(), extensions: [extension] }];
}

async function pickOpen(filters: Filter[]): Promise<string | null> {
  if (inTauri) {
    const r = await dialogOpen({ multiple: false, filters });
    return typeof r === "string" ? r : null;
  }
  return window.prompt(i18n.global.t("file.promptOpen"));
}

const osDialogs: FileDialogs = {
  pickSave,
  pickOpen,
  confirmDiscard: (message) => window.confirm(message),
};

export function useFileActions(dialogs: FileDialogs = osDialogs) {
  const store = useDocumentStore();
  const file = useProjectFileStore();
  const ui = useUiStore();
  const t = (key: string, params?: Record<string, unknown>) =>
    i18n.global.t(`file.${key}`, params ?? {});

  const projectFilter = (): Filter => ({
    name: t("projectFilter"),
    extensions: ["mdkproj"],
  });

  /** 未保存の編集があれば捨ててよいか確認する。無ければそのまま続行。 */
  function confirmDiscard(): boolean {
    if (!file.dirty) return true;
    return dialogs.confirmDiscard(t("discardConfirm"));
  }

  /** 既定の保存ファイル名 (開いているファイル、無ければプロジェクト名)。 */
  function defaultSavePath(): string {
    return file.path ?? `${store.project?.name || t("untitled")}.mdkproj`;
  }

  const kicadFilter = (): Filter => ({ name: t("kicadFilter"), extensions: ["kicad_sch"] });
  const dxfFilter = (): Filter => ({ name: t("dxfFilter"), extensions: ["dxf"] });

  /** パスを開く (拡張子でKiCad / DXF読み込みとプロジェクト読み込みを振り分ける)。 */
  async function openPath(path: string): Promise<boolean> {
    try {
      if (isKicadPath(path) || isDxfPath(path)) {
        const kind: ImportKind = isKicadPath(path) ? "kicad" : "dxf";
        const { report } = kind === "kicad" ? await file.importKicad(path) : await file.importDxf(path);
        const skipped = report.skipped.length
          ? t("importSkipped", { count: report.skipped.length })
          : "";
        ui.log(t("importLog", { ...report, what: t(`importKind.${kind}`), skipped }));
        for (const warning of report.warnings) ui.log(t("importWarningLog", { warning }));
      } else {
        await file.open(path);
        ui.log(t("openLog", { path }));
      }
      return true;
    } catch (e) {
      file.removeRecent(path);
      ui.log(t("openFailedLog", { path, message: String(e) }));
      return false;
    }
  }

  async function saveTo(path: string): Promise<boolean> {
    try {
      await file.saveAs(path);
      ui.log(t("saveLog", { path }));
      return true;
    } catch (e) {
      ui.log(t("saveFailedLog", { path, message: String(e) }));
      return false;
    }
  }

  /** 保存先を選んで保存する。 */
  async function saveProjectAs(): Promise<boolean> {
    const path = await dialogs.pickSave(defaultSavePath(), [projectFilter()]);
    if (!path) return false;
    return saveTo(path);
  }

  return {
    /** 空の新規プロジェクトを始める。未保存の編集があれば先に確認する。 */
    async newProject(): Promise<boolean> {
      if (!confirmDiscard()) return false;
      try {
        await file.create(t("untitled"));
      } catch (e) {
        ui.log(t("newFailedLog", { message: String(e) }));
        return false;
      }
      ui.log(t("newLog"));
      return true;
    },
    /** OSのダイアログでファイルを選んで開く。未保存の編集があれば先に確認する。 */
    async openProject(): Promise<boolean> {
      if (!confirmDiscard()) return false;
      const path = await dialogs.pickOpen([projectFilter(), kicadFilter(), dxfFilter()]);
      if (!path) return false;
      return openPath(path);
    },
    /** KiCad回路図 / DXFを選んで読み込む (リボン「読み込み/書き出し」)。未保存の編集があれば先に確認する。 */
    async importFile(kind: ImportKind): Promise<boolean> {
      if (!confirmDiscard()) return false;
      const path = await dialogs.pickOpen([kind === "kicad" ? kicadFilter() : dxfFilter()]);
      if (!path) return false;
      return openPath(path);
    },
    /** 表示中のシートをDXF / KiCad回路図 / SVG / PDFで書き出す。 */
    async exportSheet(kind: SheetExportKind): Promise<boolean> {
      const sheet = store.activeSheet;
      if (!sheet) {
        ui.log(t("noSheetLog"));
        return false;
      }
      const extension = kind === "kicad" ? "kicad_sch" : kind;
      const path = await dialogs.pickSave(`${sheet.name}.${extension}`, [
        kind === "kicad" ? kicadFilter() : kind === "dxf" ? dxfFilter() : filterFor(kind)[0],
      ]);
      if (!path) return false;
      try {
        if (kind === "svg") await ipc.exportSvg(sheet.id, path);
        else if (kind === "pdf") await ipc.exportPdf(sheet.id, path);
        else if (kind === "dxf") await ipc.exportDxf(sheet.id, path);
        else await ipc.exportKicad(sheet.id, path);
      } catch (e) {
        ui.log(t("exportFailedLog", { what: t(`exportKind.${kind}`), path, message: String(e) }));
        return false;
      }
      ui.log(t("exportLog", { what: t(`exportKind.${kind}`), path }));
      return true;
    },
    /** 最近使ったファイルを開く。開けなければ一覧から外す。 */
    async openRecent(path: string): Promise<boolean> {
      if (!confirmDiscard()) return false;
      return openPath(path);
    },
    /** 上書き保存。保存先が未定なら「名前を付けて保存」になる。 */
    async saveProject(): Promise<boolean> {
      if (!file.path) return saveProjectAs();
      return saveTo(file.path);
    },
    saveProjectAs,
    exportSvg(): Promise<boolean> {
      return this.exportSheet("svg");
    },
    exportPdf(): Promise<boolean> {
      return this.exportSheet("pdf");
    },
    async exportBom() {
      const path = await dialogs.pickSave(t("bomFile"), filterFor("csv"));
      if (!path) return;
      await ipc.exportBom(path);
      ui.log(t("exportLog", { what: t("exportBom"), path }));
    },
    async exportWireList() {
      const path = await dialogs.pickSave(t("wireListFile"), filterFor("csv"));
      if (!path) return;
      await ipc.exportWireList(path);
      ui.log(t("exportLog", { what: t("exportWireList"), path }));
    },
  };
}
