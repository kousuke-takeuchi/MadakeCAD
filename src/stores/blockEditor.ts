// ブロック(汎用機器)のピン表エディタの下書きストア。
//
// 部品挿入ダイアログの「ブロック(汎用)」パネル (mode="insert") と、配置済みブロックの
// 「ピン表を編集…」ダイアログ (mode="edit") が同じ下書きを共有する。
// 下書きを触っているあいだ図面は変わらず、
//   - 挿入: 組み立てた属性を配置ツールへ渡す (確定時に add_entity)
//   - 編集: `apply()` で update_entity コマンドを1回だけ実行する (undo一発)
// のどちらもCommandエンジンを通る (CLAUDE.md「アーキテクチャの絶対原則」)。

import { defineStore } from "pinia";
import {
  addRow,
  attrsFromDraft,
  blankDraft,
  draftFromAttrs,
  draftIssues,
  draftSize,
  draftSymbol,
  droppedWiredPins,
  moveRow,
  patchRow,
  removeRow,
  wiredPinNumbers,
  type BlockDraft,
  type BlockIssue,
  type BlockPinRow,
  type BlockSide,
  type BlockSize,
} from "../canvas/blockEditor";
import { BLOCK_PINS_ATTR, BLOCK_SIZE_ATTR, resolveInstanceDef } from "../canvas/dynamicSymbol";
import type { Entity, SymbolDef } from "../ipc";
import { useDocumentStore } from "./document";

/** 図面に置かれたブロック本体 (シンボルのエンティティ)。 */
type PlacedSymbol = Extract<Entity, { kind: "symbol" }>;

/** ピン表エディタの使われ方。 */
export type BlockEditorMode = "insert" | "edit";

interface BlockEditorState {
  /** エディタ (挿入パネル/編集ダイアログ) を使っているか。 */
  active: boolean;
  mode: BlockEditorMode;
  /** 編集中のブロック (mode="edit"のとき)。 */
  entityId: string | null;
  sheetId: string | null;
  reference: string;
  /** 編集開始時のブロック本体 (適用時に参照記号・型番・位置をそのまま持ち回る)。 */
  instance: PlacedSymbol | null;
  draft: BlockDraft | null;
  /** 開いた時点の属性 (変更の有無の判定に使う)。 */
  clean: string | null;
  /** 開いた時点で配線が届いていたピンの番号。 */
  wiredPins: string[];
  /** 上下辺の行を開いているか (既定は折りたたみ)。 */
  extraSidesOpen: boolean;
  saving: boolean;
  error: string | null;
}

export const useBlockEditorStore = defineStore("blockEditor", {
  state: (): BlockEditorState => ({
    active: false,
    mode: "insert",
    entityId: null,
    sheetId: null,
    reference: "",
    instance: null,
    draft: null,
    clean: null,
    wiredPins: [],
    extraSidesOpen: false,
    saving: false,
    error: null,
  }),

  getters: {
    /** 図面に持たせる属性 (配置・適用に使う)。 */
    attrs(state): Record<string, string> {
      return state.draft ? attrsFromDraft(state.draft) : {};
    },

    /** ピン表の指摘 (これがある間は配置・適用できない)。 */
    issues(state): BlockIssue[] {
      return state.draft ? draftIssues(state.draft) : [];
    },

    /** プレビュー (配置後に描かれるシンボル定義そのもの)。 */
    previewDef(state): SymbolDef | null {
      return state.draft ? draftSymbol(state.draft) : null;
    },

    /** 外形を自動で決めているか。 */
    sizeMode(state): "auto" | "manual" {
      return state.draft?.size ? "manual" : "auto";
    },

    /** ピンから決まる外形寸法 (「自動」の表示と、手動へ切り替えたときの初期値)。 */
    autoSize(state): BlockSize {
      return state.draft ? draftSize({ ...state.draft, size: null }) : { w: 0, h: 0 };
    },

    /** いまの外形寸法 (手動ならその値)。 */
    size(state): BlockSize {
      return state.draft ? draftSize(state.draft) : { w: 0, h: 0 };
    },

    /** 開いた時点からピン表が変わったか。 */
    dirty(state): boolean {
      return state.draft !== null && JSON.stringify(attrsFromDraft(state.draft)) !== state.clean;
    },

    /**
     * この編集で接続が外れるピンの番号 (配線済みなのに消える・番号が変わるピン)。
     * 警告を出すだけで、編集そのものは止めない。
     */
    warnings(state): string[] {
      return state.draft ? droppedWiredPins(state.wiredPins, state.draft) : [];
    },

    /** 配置・適用できるか (指摘が1つも無ければよい。警告は止めない)。 */
    canApply(state): boolean {
      return !state.saving && state.draft !== null && this.issues.length === 0;
    },
  },

  actions: {
    /** 部品挿入で「ブロック(汎用)」を選んだ: 既定のピン表で編集を始める。 */
    startInsert() {
      this.reset(blankDraft());
    },

    /** 配置済みブロックのピン表を開く (プロパティの「ピン表を編集…」)。 */
    startEdit(entity: Entity) {
      if (entity.kind !== "symbol") return;
      const doc = useDocumentStore();
      const sheet = doc.activeSheet;
      this.reset(draftFromAttrs(entity.attrs));
      this.mode = "edit";
      this.entityId = entity.id;
      this.sheetId = sheet?.id ?? null;
      this.reference = entity.reference;
      this.instance = { ...entity, attrs: { ...entity.attrs } };
      this.clean = JSON.stringify(attrsFromDraft(this.draft!));
      const def = resolveInstanceDef(entity, doc.symbolDefs);
      this.wiredPins = sheet && def ? wiredPinNumbers(sheet, entity, def) : [];
    },

    /** 下書きを差し替えて開く。 */
    reset(draft: BlockDraft) {
      this.active = true;
      this.mode = "insert";
      this.entityId = null;
      this.sheetId = null;
      this.reference = "";
      this.instance = null;
      this.draft = draft;
      this.clean = JSON.stringify(attrsFromDraft(draft));
      this.wiredPins = [];
      this.extraSidesOpen = draft.rows.top.length > 0 || draft.rows.bottom.length > 0;
      this.error = null;
    },

    addRow(side: BlockSide) {
      if (this.draft) addRow(this.draft, side);
    },

    removeRow(side: BlockSide, index: number) {
      if (this.draft) removeRow(this.draft, side, index);
    },

    moveRow(side: BlockSide, index: number, delta: number) {
      if (this.draft) moveRow(this.draft, side, index, delta);
    },

    updateRow(side: BlockSide, index: number, patch: Partial<BlockPinRow>) {
      const row = this.draft?.rows[side][index];
      if (row) patchRow(row, patch);
    },

    /** 外形を自動/手動へ切り替える (手動にした瞬間の寸法は自動と同じ)。 */
    setSizeMode(mode: "auto" | "manual") {
      if (!this.draft) return;
      this.draft.size = mode === "manual" ? this.autoSize : null;
    },

    /** 手動の外形寸法を書き換える (自動のときは何もしない)。 */
    setSize(patch: Partial<BlockSize>) {
      if (this.draft?.size) Object.assign(this.draft.size, patch);
    },

    toggleExtraSides() {
      this.extraSidesOpen = !this.extraSidesOpen;
    },

    /**
     * 編集を図面へ反映する (update_entityコマンド1回)。
     * 変更が無ければ何も送らずfalseを返す。ブロック以外の属性はそのまま残す。
     */
    async apply(): Promise<boolean> {
      const entity = this.instance;
      const sheetId = this.sheetId;
      if (this.mode !== "edit" || !entity || !sheetId || !this.draft || !this.canApply) return false;
      if (!this.dirty) {
        this.close();
        return false;
      }
      const attrs = { ...entity.attrs };
      delete attrs[BLOCK_PINS_ATTR];
      delete attrs[BLOCK_SIZE_ATTR];
      Object.assign(attrs, attrsFromDraft(this.draft));
      this.saving = true;
      this.error = null;
      try {
        await useDocumentStore().execute({
          type: "update_entity",
          sheet_id: sheetId,
          entity: { ...entity, attrs },
        });
      } catch (e) {
        this.error = String(e);
        return false;
      } finally {
        this.saving = false;
      }
      this.close();
      return true;
    },

    /** 閉じる (下書きは捨てる)。 */
    close() {
      this.active = false;
      this.draft = null;
      this.clean = null;
      this.entityId = null;
      this.sheetId = null;
      this.reference = "";
      this.instance = null;
      this.wiredPins = [];
      this.error = null;
    },
  },
});
