// 用紙変更ダイアログの状態 (G15)。編集はこのストアの下書き(draft)の上だけで行い、
// 保存時に set_sheet_paper コマンドを1回だけ実行する。
// モデル(document ストアのミラー)を直接書き換えてはならない。

import { defineStore } from "pinia";
import type { Orientation, PaperSize, Sheet } from "../ipc";
import { useDocumentStore } from "./document";

interface SheetPaperState {
  open: boolean;
  sheetId: string | null;
  sheetName: string;
  size: PaperSize;
  orientation: Orientation;
  originalSize: PaperSize;
  originalOrientation: Orientation;
  saving: boolean;
  error: string | null;
}

export const useSheetPaperStore = defineStore("sheetPaper", {
  state: (): SheetPaperState => ({
    open: false,
    sheetId: null,
    sheetName: "",
    size: "A3",
    orientation: "Landscape",
    originalSize: "A3",
    originalOrientation: "Landscape",
    saving: false,
    error: null,
  }),

  getters: {
    /** 開いた時点から内容が変わったか。 */
    dirty(state): boolean {
      return state.size !== state.originalSize || state.orientation !== state.originalOrientation;
    },
  },

  actions: {
    /** シートの現在の用紙サイズ・向きを下書きへ複製してダイアログを開く。 */
    openFor(sheet: Sheet) {
      this.sheetId = sheet.id;
      this.sheetName = sheet.name;
      this.size = sheet.size;
      this.orientation = sheet.orientation;
      this.originalSize = sheet.size;
      this.originalOrientation = sheet.orientation;
      this.error = null;
      this.open = true;
    },

    /** 編集を捨てて閉じる (コマンドは送らない)。 */
    cancel() {
      this.open = false;
      this.sheetId = null;
      this.error = null;
    },

    /**
     * set_sheet_paperコマンドを1回実行して閉じる。既存のエンティティ座標は動かない
     * (Rust側の仕様どおり)。変更が無ければ何も送らない。
     * 失敗したら開いたままエラーを表示する。戻り値=送信したか。
     */
    async save(): Promise<boolean> {
      if (!this.sheetId || this.saving) return false;
      if (!this.dirty) {
        this.cancel();
        return false;
      }
      const doc = useDocumentStore();
      this.saving = true;
      this.error = null;
      try {
        await doc.execute({
          type: "set_sheet_paper",
          sheet_id: this.sheetId,
          size: this.size,
          orientation: this.orientation,
        });
      } catch (e) {
        this.error = String(e);
        return false;
      } finally {
        this.saving = false;
      }
      this.cancel();
      return true;
    },
  },
});
