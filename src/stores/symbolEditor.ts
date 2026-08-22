// シンボルエディタの下書きストア (M4仕様 §7、.pen「M4デザイン - シンボルエディタ」)。
//
// ここが扱うのは**ファイルのドメイン**で、図面のCommandエンジンとは別物:
// 下書きをいくら触っても図面は変わらず、undo履歴にも乗らない。
// 「ユーザーライブラリへ保存」で初めて `~/MadakeCAD/symbols/{id}.json` へ書き込む。
//
// 保存すると、そのidを使ってすでに配置してある部品も新しい定義で描き直される
// (図面はシンボル定義のスナップショットを持たない。Task 3で仕様として固定済み)。
//
// 検証は2段構え: 画面のチェックリストは `canvas/symbolEditor.ts` の同一ロジック、
// 最終的な可否は保存時にRust (`madake_core::symbol_library::validate`) が決める。

import { defineStore } from "pinia";
import {
  SLOT_KEYS,
  hitTest,
  nextPinNumber,
  nextPinSpot,
  snapToGrid,
  symbolCanSave,
  symbolChecks,
  translatePrimitive,
  type EditorHit,
  type SlotKey,
  type SymbolCheck,
  type SymbolTool,
} from "../canvas/symbolEditor";
import { symbolMatchesQuery } from "../canvas/symbolLibrary";
import type { Pt } from "../canvas/viewport";
import {
  ipc,
  USER_SYMBOL_CATEGORY,
  type PinDef,
  type PinDir,
  type Point,
  type Primitive,
  type SymbolDef,
  type SymbolError,
  type TextSlot,
} from "../ipc";
import { useDocumentStore } from "./document";

/** 属性テキストの既定の文字高 (mm)。図面の注記と同じ大きさ。 */
export const SLOT_TEXT_HEIGHT = 2.5;

/** キャンバスの当たり判定の許容差 (mm)。グリッド半目盛り。 */
export const HIT_TOLERANCE_MM = 1.25;

/** 空から始めるときの下書き (参照記号の置き場だけ入っている)。 */
export function blankDraft(): SymbolDef {
  return {
    id: "",
    name: "",
    name_ja: "",
    category: USER_SYMBOL_CATEGORY,
    ref_prefix: "",
    keywords: [],
    primitives: [],
    pins: [],
    text_slots: [{ key: "TAG", at: { x: 0, y: -7.5 }, height: SLOT_TEXT_HEIGHT }],
  };
}

/** 属性スロットの既定位置 (参照記号・型番は外形の上、説明・定格は下)。 */
const SLOT_DEFAULT_AT: Record<SlotKey, Point> = {
  TAG: { x: 0, y: -7.5 },
  PART: { x: 0, y: -5 },
  DESC: { x: 0, y: 10 },
  RATING: { x: 0, y: 12.5 },
};

/** 保存が拒否されたときのRustの返し方 (Vec<SymbolError> か 文字列) をならす。 */
export function symbolSaveErrors(e: unknown): SymbolError[] {
  if (Array.isArray(e)) {
    return e.filter((x) => x && typeof x === "object" && "message" in x) as SymbolError[];
  }
  const message = e instanceof Error ? e.message : String(e);
  return [{ code: "symbol.save_failed", message }];
}

interface SymbolEditorState {
  open: boolean;
  /** 編集中の下書き。閉じているときはnull。 */
  draft: SymbolDef | null;
  /** 直近に保存した (または開いた) 時点の下書き。未保存変更の判定に使う。 */
  clean: string | null;
  tool: SymbolTool;
  selection: EditorHit | null;
  /** 属性テキストのプレビュー値 (キャンバスに実配置で出す見本。ファイルには残らない)。 */
  samples: Record<string, string>;
  /** ユーザーシンボルの置き場 (`~/MadakeCAD/symbols`)。 */
  userDir: string | null;
  /** 直近の保存先パス。 */
  savedPath: string | null;
  /** 保存が拒否された理由 (全部)。 */
  saveErrors: SymbolError[];
  error: string | null;
  saving: boolean;
  /** 未保存の変更があるまま閉じようとした状態 (フッタに確認を出す)。 */
  confirmingClose: boolean;
  /** 「既存シンボルを複製して新規…」のポップオーバー。 */
  pickerOpen: boolean;
  pickerQuery: string;
  /** キャンバスの座標バーに出す位置 (mm)。 */
  cursor: Point | null;
}

export const useSymbolEditorStore = defineStore("symbolEditor", {
  state: (): SymbolEditorState => ({
    open: false,
    draft: null,
    clean: null,
    tool: "select",
    selection: null,
    samples: {},
    userDir: null,
    savedPath: null,
    saveErrors: [],
    error: null,
    saving: false,
    confirmingClose: false,
    pickerOpen: false,
    pickerQuery: "",
    cursor: null,
  }),

  getters: {
    /** 保存していない変更があるか (閉じる前の確認に使う)。 */
    dirty(state): boolean {
      return state.draft !== null && JSON.stringify(state.draft) !== state.clean;
    },

    /**
     * 使ってはいけないid (同梱シンボル)。ここと衝突させると、そのidを使っている
     * 既存図面の記号が黙って変わってしまう。
     */
    reservedIds(): string[] {
      return useDocumentStore()
        .symbols.filter((d) => d.category !== USER_SYMBOL_CATEGORY)
        .map((d) => d.id);
    },

    /** 右パネルの「保存前チェック」。 */
    checks(state): SymbolCheck[] {
      return state.draft ? symbolChecks(state.draft, this.reservedIds) : [];
    },

    /** エラーが1つも無ければ保存できる (注意は保存できる)。 */
    canSave(state): boolean {
      return !state.saving && state.draft !== null && symbolCanSave(this.checks);
    },

    /** フッタに出す保存先。idが決まるまでは空。 */
    filePath(state): string {
      const id = state.draft?.id.trim() ?? "";
      if (!id) return "";
      return `${state.userDir ?? "~/MadakeCAD/symbols"}/${id}.json`;
    },

    /** 接続点表の行 (番号・方向・名前)。 */
    pinRows(state): PinDef[] {
      return state.draft?.pins ?? [];
    },

    /** 属性パレットの行 (置いてあるか・見本の値)。 */
    slotAt(state): (key: string) => TextSlot | undefined {
      return (key) => state.draft?.text_slots?.find((s) => s.key === key);
    },

    /**
     * 属性テキストの見本 (キャンバスに実配置で描く文字)。
     * 参照記号は接頭辞+`?`(配置時に採番される形)、他は入力欄の値かキー名。
     */
    sampleFor(state): (key: string) => string {
      return (key) => {
        const custom = state.samples[key]?.trim();
        if (custom) return custom;
        if (key === "TAG") return `${state.draft?.ref_prefix ?? ""}?`;
        return key;
      };
    },

    /** 「複製から新規」ポップオーバーの候補 (同梱+ユーザーの全シンボル)。 */
    pickerRows(state): SymbolDef[] {
      return useDocumentStore().symbols.filter((d) => symbolMatchesQuery(d, state.pickerQuery));
    },
  },

  actions: {
    /** ユーザーシンボルの置き場を読む (フッタの保存先表示に使う。失敗しても編集はできる)。 */
    async loadUserDir() {
      try {
        const library = await ipc.listUserSymbols();
        this.userDir = library.user_dir ?? null;
      } catch {
        this.userDir = null;
      }
    },

    /** 空のシンボルから編集を始める。 */
    async openBlank() {
      this.reset(blankDraft());
      await this.loadUserDir();
    },

    /**
     * 既存シンボルを複製して編集を始める (既定の動線)。
     * `duplicate_symbol` が返すのは**下書き**で、ファイルにはまだ書かれていない。
     */
    async openDuplicate(sourceId: string) {
      this.error = null;
      try {
        const draft = await ipc.duplicateSymbol(sourceId);
        this.reset(draft);
      } catch (e) {
        this.error = String(e);
        if (!this.open) this.reset(blankDraft());
      }
      await this.loadUserDir();
    },

    /** 下書きを差し替えてエディタを開く (未保存マークは消える)。 */
    reset(draft: SymbolDef) {
      this.draft = { ...draft, category: USER_SYMBOL_CATEGORY, text_slots: draft.text_slots ?? [] };
      this.clean = JSON.stringify(this.draft);
      this.open = true;
      this.tool = "select";
      this.selection = null;
      this.saveErrors = [];
      this.savedPath = null;
      this.confirmingClose = false;
      this.pickerOpen = false;
      this.pickerQuery = "";
      this.samples = {};
    },

    setTool(tool: SymbolTool) {
      this.tool = tool;
      if (tool !== "select") this.selection = null;
    },

    select(hit: EditorHit | null) {
      this.selection = hit;
    },

    /** キャンバスのクリック位置から掴めるものを選ぶ。 */
    selectAt(at: Pt) {
      if (!this.draft) return;
      this.selection = hitTest(this.draft, at, HIT_TOLERANCE_MM);
    },

    setCursor(at: Point | null) {
      this.cursor = at;
    },

    /** 図形を描き足す (描いた図形が選択された状態になる)。 */
    addPrimitive(prim: Primitive) {
      if (!this.draft) return;
      this.draft.primitives.push(prim);
      this.selection = { kind: "primitive", index: this.draft.primitives.length - 1 };
    },

    /** 選択中のもの (図形・接続点・属性テキスト) を動かす。 */
    moveSelection(dx: number, dy: number) {
      const hit = this.selection;
      if (!this.draft || !hit) return;
      if (hit.kind === "primitive") {
        this.draft.primitives[hit.index] = translatePrimitive(
          this.draft.primitives[hit.index],
          dx,
          dy,
        );
        return;
      }
      if (hit.kind === "pin") {
        const pin = this.draft.pins[hit.index];
        pin.at = snapToGrid({ x: pin.at.x + dx, y: pin.at.y + dy });
        return;
      }
      const slot = this.draft.text_slots?.[hit.index];
      if (slot) slot.at = { x: slot.at.x + dx, y: slot.at.y + dy };
    },

    /** 選択中のものを消す (属性テキストは必須のTAGでも外せる。チェックで指摘される)。 */
    deleteSelection() {
      const hit = this.selection;
      if (!this.draft || !hit) return;
      if (hit.kind === "primitive") this.draft.primitives.splice(hit.index, 1);
      else if (hit.kind === "pin") this.draft.pins.splice(hit.index, 1);
      else this.draft.text_slots?.splice(hit.index, 1);
      this.selection = null;
    },

    /** 方向別追加ボタン: 図形の外側1グリッドへ接続点を足す。 */
    addPin(dir: PinDir) {
      if (!this.draft) return;
      this.addPinAt(nextPinSpot(this.draft, dir), dir);
    },

    /** キャンバスで接続点を置く (位置は必ずグリッドの交点へ丸める)。 */
    addPinAt(at: Pt, dir: PinDir) {
      if (!this.draft) return;
      this.draft.pins.push({
        number: nextPinNumber(this.draft),
        name: "",
        at: snapToGrid(at),
        dir,
      });
      this.selection = { kind: "pin", index: this.draft.pins.length - 1 };
    },

    /** 接続点表の1行を書き換える (位置はグリッドへ丸める)。 */
    updatePin(index: number, patch: Partial<PinDef>) {
      const pin = this.draft?.pins[index];
      if (!pin) return;
      if (patch.number !== undefined) pin.number = patch.number;
      if (patch.name !== undefined) pin.name = patch.name;
      if (patch.dir !== undefined) pin.dir = patch.dir;
      if (patch.at !== undefined) pin.at = { x: patch.at.x, y: patch.at.y };
    },

    removePin(index: number) {
      this.draft?.pins.splice(index, 1);
      this.selection = null;
    },

    /** 属性テキストを置く/外す (外した属性はチェックで注意になる)。 */
    toggleSlot(key: SlotKey) {
      if (!this.draft) return;
      const slots = this.draft.text_slots ?? (this.draft.text_slots = []);
      const index = slots.findIndex((s) => s.key === key);
      if (index >= 0) {
        slots.splice(index, 1);
        this.selection = null;
        return;
      }
      slots.push({ key, at: { ...SLOT_DEFAULT_AT[key] }, height: SLOT_TEXT_HEIGHT });
      this.selection = { kind: "slot", index: slots.length - 1 };
    },

    /**
     * 属性ツールでクリックした位置へ属性テキストを置く。
     * まだ置いていないキーを`TAG → PART → DESC → RATING`の順に1つずつ置く。
     */
    placeSlotAt(at: Pt) {
      if (!this.draft) return;
      const slots = this.draft.text_slots ?? (this.draft.text_slots = []);
      const next = SLOT_KEYS.find((key) => !slots.some((s) => s.key === key));
      if (next === undefined) {
        // 4種すべて置いてあるときは、選択中の属性を動かす
        const hit = this.selection;
        if (hit?.kind === "slot" && slots[hit.index]) slots[hit.index].at = { ...at };
        return;
      }
      slots.push({ key: next, at: { ...at }, height: SLOT_TEXT_HEIGHT });
      this.selection = { kind: "slot", index: slots.length - 1 };
    },

    /** 属性テキストの見本の値 (キャンバスに実配置で出す。ファイルには残らない)。 */
    setSample(key: string, value: string) {
      this.samples = { ...this.samples, [key]: value };
    },

    /** 右パネルのシンボル情報を書き換える。 */
    setInfo(patch: Partial<Pick<SymbolDef, "id" | "name" | "name_ja" | "ref_prefix">>) {
      if (!this.draft) return;
      Object.assign(this.draft, patch);
    },

    /** 検索キー (カンマ区切り) を書き換える。 */
    setKeywords(text: string) {
      if (!this.draft) return;
      this.draft.keywords = text
        .split(/[,、]/)
        .map((k) => k.trim())
        .filter((k) => k.length > 0);
    },

    /**
     * ユーザーライブラリへ保存する。
     *
     * 検証に落ちたら**何も書かれず**理由が全部返る (`saveErrors`)。成功したら
     * 部品挿入の一覧を読み直して、そのまま「ユーザー」カテゴリから配置できるようにする。
     * 保存はCommandエンジンを通らないので**undoでは戻らない**。
     */
    async save(): Promise<string | null> {
      if (!this.draft || this.saving) return null;
      this.saving = true;
      this.saveErrors = [];
      this.error = null;
      try {
        const { symbol, path } = await ipc.saveUserSymbol(this.draft);
        this.draft = { ...symbol, text_slots: symbol.text_slots ?? [] };
        this.clean = JSON.stringify(this.draft);
        this.savedPath = path;
        this.confirmingClose = false;
        await useDocumentStore().reloadSymbols();
        return path;
      } catch (e) {
        this.saveErrors = symbolSaveErrors(e);
        return null;
      } finally {
        this.saving = false;
      }
    },

    /**
     * 閉じてよいか確かめてから閉じる。未保存の変更があるあいだは閉じずに
     * `confirmingClose`を立て、フッタで「破棄して閉じる」を出す。
     */
    requestClose(): boolean {
      if (this.dirty) {
        this.confirmingClose = true;
        return false;
      }
      this.close();
      return true;
    },

    /** 確認を出したあとで、変更を捨てて閉じる。 */
    discardAndClose() {
      this.close();
    },

    /** 確認をやめて編集へ戻る。 */
    keepEditing() {
      this.confirmingClose = false;
    },

    close() {
      this.open = false;
      this.draft = null;
      this.clean = null;
      this.selection = null;
      this.confirmingClose = false;
      this.pickerOpen = false;
      this.pickerQuery = "";
      this.saveErrors = [];
      this.cursor = null;
    },

    openPicker() {
      this.pickerOpen = true;
      this.pickerQuery = "";
    },

    closePicker() {
      this.pickerOpen = false;
    },

    setPickerQuery(query: string) {
      this.pickerQuery = query;
    },

    /** ポップオーバーで選んだシンボルを複製して下書きに入れる。 */
    async pickDuplicate(sourceId: string) {
      this.pickerOpen = false;
      await this.openDuplicate(sourceId);
    },

    /** ユーザーシンボルの置き場をOSのファイラで開く (開けない環境ではパスを返すだけ)。 */
    async openUserFolder(): Promise<{ path: string; opened: boolean }> {
      try {
        const path = await ipc.openSymbolsFolder();
        this.userDir = path;
        return { path, opened: true };
      } catch {
        return { path: this.userDir ?? "~/MadakeCAD/symbols", opened: false };
      }
    },
  },
});
