// ファイル操作 (新規/開く/保存/名前を付けて保存/エクスポート)。
// OSのダイアログ・確認ダイアログはすべて`nativeDialogs` (Tauriのdialogプラグイン、
// Tauri外のブラウザ検証では`window.*`) を通す。webviewの`window.confirm`/`prompt`は
// 実装が無く無言で既定値を返すので、アプリの導線から直接呼んではならない。
// ファイルの状態(パス・未保存・最近使ったファイル)はprojectFile storeが持ち、ここは
// ダイアログと確認、ログ出力だけを担当する。

import { entityKindLabels, type EntityKind } from "../components/entityKindLabels";
import { i18n } from "../i18n";
import { ipc, type ExportReport } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { isDxfPath, isKicadPath, useProjectFileStore } from "../stores/projectFile";
import { useUiStore } from "../stores/ui";
import { nativeDialogs, type DialogFilter, type NativeDialogs } from "./nativeDialogs";

type Filter = DialogFilter;

/** ファイル操作が使うダイアログ群。テストではメモリ実装を差し込む。 */
export interface FileDialogs {
  pickSave(defaultPath: string, filters: Filter[]): Promise<string | null>;
  pickOpen(filters: Filter[]): Promise<string | null>;
  /** 未保存の編集を捨ててよいかの確認。trueで続行。 */
  confirmDiscard(message: string): Promise<boolean>;
}

/**
 * 保存先をOSの保存ダイアログで選ぶ (Tauri外ではプロンプト入力)。
 * 帳票ダイアログの「参照...」もこれを使う (モーダルの上のモーダルにはならない)。
 */
export async function pickSave(defaultPath: string, filters: Filter[]): Promise<string | null> {
  return nativeDialogs.save(defaultPath, filters, i18n.global.t("file.promptSave"));
}

/**
 * 「開けなかった」が「ファイルが無い」ときだけtrue。
 * 共有ドライブ未マウント・権限の揺れのような一時的な失敗で、有効なエントリを
 * 最近使ったファイルから恒久的に消さないための判定。
 */
export function isMissingFileError(message: string): boolean {
  const lower = message.toLowerCase();
  return (
    lower.includes("no such file") ||
    lower.includes("not found") ||
    lower.includes("notfound") ||
    lower.includes("os error 2") ||
    message.includes("見つかりません")
  );
}

/** 読み込める図面形式 (プロジェクト以外)。 */
export type ImportKind = "kicad" | "dxf";
/** 書き出せる図面形式 (シート1枚)。 */
export type SheetExportKind = "svg" | "pdf" | "dxf" | "kicad";

/** 拡張子から保存ダイアログのフィルタを作る (csv / pdf / svg …)。 */
export function filterFor(extension: string): Filter[] {
  return [{ name: extension.toUpperCase(), extensions: [extension] }];
}

/** 既定のダイアログ群 (ネイティブ層をそのまま使う)。 */
export function osFileDialogs(dialogs: NativeDialogs = nativeDialogs): FileDialogs {
  return {
    pickSave: (defaultPath, filters) =>
      dialogs.save(defaultPath, filters, i18n.global.t("file.promptSave")),
    pickOpen: (filters) => dialogs.open(filters, i18n.global.t("file.promptOpen")),
    confirmDiscard: (message) =>
      dialogs.confirm(message, { title: i18n.global.t("file.discardTitle"), kind: "warning" }),
  };
}

const osDialogs: FileDialogs = osFileDialogs();

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
  async function confirmDiscard(): Promise<boolean> {
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
  async function openPath(path: string, wireLayers?: string[]): Promise<boolean> {
    try {
      if (isKicadPath(path) || isDxfPath(path)) {
        const kind: ImportKind = isKicadPath(path) ? "kicad" : "dxf";
        const { report } =
          kind === "kicad" ? await file.importKicad(path) : await file.importDxf(path, wireLayers);
        const skipped = report.skipped.length
          ? t("importSkipped", { count: report.skipped.length })
          : "";
        ui.log(t("importLog", { ...report, what: t(`importKind.${kind}`), skipped }));
        for (const warning of report.warnings) ui.log(t("importWarningLog", { warning }));
        // 配線レイヤが決まらなかったときは、レイヤを選んで読み直せることを知らせる
        if (file.lastDxf) {
          ui.log(t("dxfWireLayerHintLog", { layers: file.lastDxf.layers.join(", ") }));
        }
      } else {
        await file.open(path);
        ui.log(t("openLog", { path }));
      }
      return true;
    } catch (e) {
      const message = String(e);
      // ファイルが無くなっていたときだけ一覧から外す。一時的に開けなかった (共有ドライブが
      // 未マウント・権限の揺れ) だけのエントリは残す
      if (isMissingFileError(message)) file.removeRecent(path);
      ui.log(t("openFailedLog", { path, message }));
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
    /**
     * 閉じる/リロードしてよいか (未保存の編集があれば確認する)。
     * ウィンドウの閉じる要求のガード (`closeGuard`) がこれを使う。
     */
    confirmClose(): Promise<boolean> {
      return confirmDiscard();
    },
    /** 空の新規プロジェクトを始める。未保存の編集があれば先に確認する。 */
    async newProject(): Promise<boolean> {
      if (!(await confirmDiscard())) return false;
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
      if (!(await confirmDiscard())) return false;
      const path = await dialogs.pickOpen([projectFilter(), kicadFilter(), dxfFilter()]);
      if (!path) return false;
      return openPath(path);
    },
    /** KiCad回路図 / DXFを選んで読み込む (リボン「読み込み/書き出し」)。未保存の編集があれば先に確認する。 */
    async importFile(kind: ImportKind): Promise<boolean> {
      if (!(await confirmDiscard())) return false;
      const path = await dialogs.pickOpen([kind === "kicad" ? kicadFilter() : dxfFilter()]);
      if (!path) return false;
      return openPath(path);
    },
    /**
     * 直前に読み込んだDXFを、選んだ配線レイヤで読み直す (リボンの「配線レイヤ」ボタン)。
     * 警告に挙がったレイヤ名がそのままボタンになるので、指定を打ち込まなくても絞れる。
     */
    async reimportDxfWithWireLayer(layer: string): Promise<boolean> {
      const last = file.lastDxf;
      if (!last) return false;
      if (!(await confirmDiscard())) return false;
      ui.log(t("dxfReimportLog", { layer, path: last.path }));
      return openPath(last.path, [layer]);
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
      let report: ExportReport | null = null;
      try {
        if (kind === "svg") await ipc.exportSvg(sheet.id, path);
        else if (kind === "pdf") await ipc.exportPdf(sheet.id, path);
        else if (kind === "dxf") report = await ipc.exportDxf(sheet.id, path);
        else report = await ipc.exportKicad(sheet.id, path);
      } catch (e) {
        ui.log(t("exportFailedLog", { what: t(`exportKind.${kind}`), path, message: String(e) }));
        return false;
      }
      ui.log(t("exportLog", { what: t(`exportKind.${kind}`), path }));
      // DXF / KiCadは「何を何件出して、何を出さなかったか」を書き出し側が返す
      if (report) {
        const labels = entityKindLabels(i18n.global.t);
        const counts = Object.entries(report.counts)
          .map(([k, n]) => `${labels[k as EntityKind] ?? k} ${n}`)
          .join(" / ");
        if (counts) ui.log(t("exportCountsLog", { counts }));
        for (const skipped of report.skipped) ui.log(t("exportSkippedLog", { skipped }));
      }
      return true;
    },
    /** 最近使ったファイルを開く。開けなければ一覧から外す。 */
    async openRecent(path: string): Promise<boolean> {
      if (!(await confirmDiscard())) return false;
      return openPath(path);
    },
    /**
     * 上書き保存。保存先が未定なら「名前を付けて保存」になる。
     * 保存先が決まっているかの判定はstoreの`save()`に任せる (分岐は1か所)。
     */
    async saveProject(): Promise<boolean> {
      const target = file.path;
      try {
        if (await file.save()) {
          ui.log(t("saveLog", { path: target }));
          return true;
        }
      } catch (e) {
        ui.log(t("saveFailedLog", { path: target, message: String(e) }));
        return false;
      }
      return saveProjectAs();
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
