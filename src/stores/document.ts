// ドキュメントミラーストア。
// 信頼できる唯一の情報源はRust側(madake-core)。ここはpatch(doc:patchイベント)だけを
// 適用するミラーで、モデルを直接書き換えるコードを置いてはならない。

import { defineStore } from "pinia";
import { resolveInstanceDef, resolveSymbolDef } from "../canvas/dynamicSymbol";
import {
  DEFAULT_FRAME_TEMPLATE,
  DEFAULT_FRAME_TEMPLATE_ID,
  bindContextForProject,
  bindContextForSheet,
  type BindContext,
  type FrameTemplate,
} from "../canvas/frameTemplate";
import {
  ipc,
  type Command,
  type FrameTemplateList,
  type Patch,
  type Project,
  type Sheet,
  type SymbolDef,
  type SymbolInstance,
} from "../ipc";

interface DocumentState {
  project: Project | null;
  revision: number;
  canUndo: boolean;
  canRedo: boolean;
  symbols: SymbolDef[];
  /** 使える図枠テンプレート (同梱+ユーザー)。描画はこの中からプロジェクトのidを引く。 */
  frameTemplates: FrameTemplate[];
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
    frameTemplates: [],
    activeSheetId: null,
    selection: new Set(),
  }),

  getters: {
    /** 静的ライブラリのid→定義Map。 */
    symbolDefs(state): Map<string, SymbolDef> {
      return new Map(state.symbols.map((d) => [d.id, d]));
    },
    /** symbol_id→定義。静的ライブラリ優先、動的ID(terminal_block_{n}p等)にフォールバック。 */
    resolveSymbol(): (id: string) => SymbolDef | undefined {
      return (id) => resolveSymbolDef(id, this.symbolDefs);
    },
    /**
     * 配置済みシンボル→定義。ブロック(symbol_id="block")は形がインスタンスの属性で
     * 決まるので、symbol_idだけで引く`resolveSymbol`ではなくこちらを使う。
     */
    resolveInstance(): (inst: SymbolInstance) => SymbolDef | undefined {
      return (inst) => resolveInstanceDef(inst, this.symbolDefs);
    },
    activeSheet(state) {
      return (
        state.project?.sheets.find((s) => s.id === state.activeSheetId) ??
        state.project?.sheets[0] ??
        null
      );
    },

    /**
     * いま図面を描いている図枠テンプレート (M4 §9)。
     * 未指定・知らないidは同梱の`jis-a3`へ落とす(他所で作った図面も必ず開ける)。
     */
    frameTemplate(state): FrameTemplate {
      const byId = (id: string) => state.frameTemplates.find((t) => t.id === id);
      return (
        byId(state.project?.frame_template ?? "") ??
        byId(DEFAULT_FRAME_TEMPLATE_ID) ??
        DEFAULT_FRAME_TEMPLATE
      );
    },

    /** シートの表題欄バインドをプロジェクト文脈で解決する (シート番号・総数が入る)。 */
    bindContextFor(state) {
      return (sheet: Sheet): BindContext =>
        state.project ? bindContextForProject(state.project, sheet) : bindContextForSheet(sheet);
    },

    /**
     * 表示中のシートをキャンバスへ描くときの図枠指定 (`renderSheet`の`frame`/`bind`)。
     * シートが無ければnull。
     */
    frameOptions(): { frame: FrameTemplate; bind: BindContext } | null {
      const sheet = this.activeSheet;
      return sheet ? { frame: this.frameTemplate, bind: this.bindContextFor(sheet) } : null;
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
      await this.loadFrameTemplates();
      await ipc.onPatch((patch) => this.applyPatch(patch));
    },

    /**
     * 部品挿入に並ぶシンボルの一覧を読み直す (同梱+ユーザーの`~/MadakeCAD/symbols`)。
     * シンボルエディタでユーザーシンボルを保存したあとに呼ぶと、保存した記号が
     * 「ユーザー」カテゴリへ現れ、すでに配置済みの部品も新しい定義で描き直される。
     */
    async reloadSymbols(): Promise<SymbolDef[]> {
      try {
        this.symbols = await ipc.listSymbols();
      } catch {
        // 一覧が読めなくても図面は開いたままにする (次の保存でまた試す)
      }
      return this.symbols;
    },

    /**
     * 図枠テンプレートの一覧を読み込む (起動時と、ユーザーが図枠JSONを足したあと)。
     * 読めなくても図面は開ける: 一覧が空なら同梱のjis-a3で描く。
     */
    async loadFrameTemplates(): Promise<FrameTemplateList> {
      try {
        const list = await ipc.listFrameTemplates();
        this.frameTemplates = list.templates;
        return list;
      } catch {
        this.frameTemplates = [];
        return { templates: [], issues: [] };
      }
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
          case "plc_assignments_replaced":
            if (this.project) this.project.plc_assignments = op.assignments;
            break;
          case "frame_template_changed":
            if (this.project) this.project.frame_template = op.frame_template;
            break;
        }
      }
      this.revision = patch.revision;
    },

    /**
     * コマンドを実行する。patchは戻り値で即時適用(イベントは重複適用されない)。
     * 呼び出し側が結果 (変更されたエンティティ) を要約できるよう、適用したpatchを返す。
     */
    async execute(command: Command): Promise<Patch> {
      const patch = await ipc.executeCommand(command);
      this.applyEdit(patch);
      return patch;
    },

    /**
     * 自分が起こした編集のpatchをミラーへ適用し、undo/redoの可否も更新する。
     * `execute()`を通さない専用の編集入口(テンプレート適用など)もここを通す。
     */
    applyEdit(patch: Patch) {
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
