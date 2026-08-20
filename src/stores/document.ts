// ドキュメントミラーストア。
// 信頼できる唯一の情報源はRust側(madake-core)。ここはpatch(doc:patchイベント)だけを
// 適用するミラーで、モデルを直接書き換えるコードを置いてはならない。

import { defineStore } from "pinia";
import { ipc, type Command, type Patch, type Project, type SymbolDef } from "../ipc";

interface DocumentState {
  project: Project | null;
  revision: number;
  canUndo: boolean;
  canRedo: boolean;
  symbols: SymbolDef[];
  /** 現在表示中のシートid。 */
  activeSheetId: string | null;
  /** 選択中エンティティid集合。 */
  selection: Set<string>;
}

export const useDocumentStore = defineStore("document", {
  state: (): DocumentState => ({
    project: null,
    revision: 0,
    canUndo: false,
    canRedo: false,
    symbols: [],
    activeSheetId: null,
    selection: new Set(),
  }),

  getters: {
    activeSheet(state) {
      return (
        state.project?.sheets.find((s) => s.id === state.activeSheetId) ??
        state.project?.sheets[0] ??
        null
      );
    },
  },

  actions: {
    /** 起動時に全状態を取得し、patch購読を開始する。 */
    async bootstrap() {
      const [snapshot, symbols] = await Promise.all([ipc.getProject(), ipc.listSymbols()]);
      this.project = snapshot.project;
      this.revision = snapshot.revision;
      this.canUndo = snapshot.can_undo;
      this.canRedo = snapshot.can_redo;
      this.symbols = symbols;
      this.activeSheetId = snapshot.project.sheets[0]?.id ?? null;
      await ipc.onPatch((patch) => this.applyPatch(patch));
    },

    /** Rustからのpatchをミラーに適用する。 */
    applyPatch(patch: Patch) {
      // エンジンのrevisionは単調増加。同一patchがinvoke戻り値とdoc:patchイベントの
      // 両方から届くため、適用済み(以下)のrevisionは捨てる
      if (patch.revision <= this.revision) return;
      for (const op of patch.ops) {
        switch (op.op) {
          case "project_replaced":
            this.project = op.project;
            this.activeSheetId = op.project.sheets[0]?.id ?? null;
            this.selection = new Set();
            break;
          case "sheet_added":
            this.project?.sheets.splice(op.index, 0, op.sheet);
            break;
          case "sheet_removed": {
            if (!this.project) break;
            this.project.sheets = this.project.sheets.filter((s) => s.id !== op.sheet_id);
            if (this.activeSheetId === op.sheet_id) {
              this.activeSheetId = this.project.sheets[0]?.id ?? null;
            }
            break;
          }
          case "sheet_meta_updated": {
            const sheet = this.project?.sheets.find((s) => s.id === op.sheet.id);
            if (sheet) {
              Object.assign(sheet, { ...op.sheet, entities: sheet.entities });
            }
            break;
          }
          case "entity_upserted": {
            const sheet = this.project?.sheets.find((s) => s.id === op.sheet_id);
            if (sheet) sheet.entities[op.entity.id] = op.entity;
            break;
          }
          case "entity_removed": {
            const sheet = this.project?.sheets.find((s) => s.id === op.sheet_id);
            if (sheet) {
              delete sheet.entities[op.id];
              this.selection.delete(op.id);
            }
            break;
          }
          case "wire_parts_replaced":
            if (this.project) this.project.wire_parts = op.wire_parts;
            break;
        }
      }
      this.revision = patch.revision;
    },

    /** コマンドを実行する。patchは戻り値で即時適用(イベントは重複適用されない)。 */
    async execute(command: Command) {
      const patch = await ipc.executeCommand(command);
      this.applyPatch(patch);
      this.canUndo = true;
      this.canRedo = false;
    },

    async undo() {
      const patch = await ipc.undo();
      if (patch) this.applyPatch(patch);
      const snap = await ipc.getProject();
      this.canUndo = snap.can_undo;
      this.canRedo = snap.can_redo;
    },

    async redo() {
      const patch = await ipc.redo();
      if (patch) this.applyPatch(patch);
      const snap = await ipc.getProject();
      this.canUndo = snap.can_undo;
      this.canRedo = snap.can_redo;
    },
  },
});
