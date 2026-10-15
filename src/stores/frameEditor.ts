// 図枠エディタの下書きストア (M4残り⑥、.pen「M4残りデザイン⑥」`dhP9P`)。
//
// 扱うのは**ファイルのドメイン** (図枠テンプレートJSON) で、図面のCommandエンジンとは別物:
// 下書きをいくら触っても図面は変わらず、図面のundo履歴にも乗らない。エディタの中の
// ⌘Z/⇧⌘Zはこのストアが持つ版の積み重ね (past/future) で戻す (シンボルエディタと同じ扱い)。
// 「保存」で初めて`~/MadakeCAD/templates/frames/{id}.json`へ書き、一覧を読み直す。図面は
// 描くたびに図枠をidで引くので、その図枠を使っている全シートが次の描画から新しい図枠になる。
//
// 同梱の図枠はそのまま上書きできないので、同梱から開いた下書き・「新規 (コピーから)」は
// 保存すると「別名で保存」になる。

import { defineStore } from "pinia";
import {
  copyId,
  frameChecks,
  isValidFrameId,
  type FrameChecks,
  type FrameItem,
  type FrameTool,
  type Pt,
} from "../canvas/frameEditor";
import type { FrameTemplate } from "../canvas/frameTemplate";
import { ipc } from "../ipc";
import { useDocumentStore } from "./document";
import { useFrameTemplatesStore } from "./frameTemplates";

/** エディタ内undoの最大段数。 */
export const FRAME_UNDO_LIMIT = 100;

interface FrameEditorState {
  open: boolean;
  draft: FrameTemplate | null;
  /** 最後に保存した (開いた) ときのJSON。下書きと違えば「未保存の変更あり」。 */
  savedJson: string;
  /** まだファイルが無い下書き (同梱から開いた・コピーから新規)。保存は別名で保存になる。 */
  isNew: boolean;
  past: FrameTemplate[];
  future: FrameTemplate[];
  tool: FrameTool;
  selection: FrameItem | null;
  /** 作図領域のカーソル位置 (用紙mm。状態バーに出す)。 */
  cursor: Pt | null;
  /** プレースホルダを隠す (固定の線・文字だけを見る)。 */
  hidePlaceholders: boolean;
  /** プレースホルダを現在のプロジェクトの実際の値で見せる。 */
  showValues: boolean;
  /** 別名で保存の入力欄。 */
  saveAsOpen: boolean;
  saveAsId: string;
  saveAsName: string;
  /** 未保存のまま閉じようとしたときの確認。 */
  confirmClose: boolean;
  saving: boolean;
  error: string | null;
}

const json = (t: FrameTemplate | null) => JSON.stringify(t);
const clone = (t: FrameTemplate) => JSON.parse(JSON.stringify(t)) as FrameTemplate;

export const useFrameEditorStore = defineStore("frameEditor", {
  state: (): FrameEditorState => ({
    open: false,
    draft: null,
    savedJson: "",
    isNew: false,
    past: [],
    future: [],
    tool: "select",
    selection: null,
    cursor: null,
    hidePlaceholders: false,
    showValues: false,
    saveAsOpen: false,
    saveAsId: "",
    saveAsName: "",
    confirmClose: false,
    saving: false,
    error: null,
  }),

  getters: {
    /** 開いた (保存した) ときから下書きが変わったか。 */
    dirty(state): boolean {
      return json(state.draft) !== state.savedJson;
    },
    checks(state): FrameChecks {
      return state.draft ? frameChecks(state.draft) : { errors: [], unknownBinds: [], outside: 0 };
    },
    canUndo: (state) => state.past.length > 0,
    canRedo: (state) => state.future.length > 0,
    /** 一覧にあるid (別名で保存の衝突確認)。 */
    takenIds(): string[] {
      return useDocumentStore().frameTemplates.map((t) => t.id);
    },
  },

  actions: {
    /**
     * 図枠を開く。`copy`か同梱の図枠なら、`-copy`のidを付けた新しい下書きとして開く
     * (同梱はそのまま上書きできない)。
     */
    openFor(template: FrameTemplate, mode: "edit" | "copy") {
      const draft = clone(template);
      const asNew = mode === "copy" || template.builtin !== false;
      if (asNew) {
        draft.id = copyId(template.id, this.takenIds);
        draft.builtin = false;
      }
      Object.assign(this, {
        open: true,
        draft,
        savedJson: json(draft),
        isNew: asNew,
        past: [],
        future: [],
        tool: "select" as FrameTool,
        selection: null,
        saveAsOpen: false,
        confirmClose: false,
        error: null,
      });
    },

    /** 編集を1段積む (同じものなら何もしない)。新しい編集はredoを捨てる。 */
    commit(next: FrameTemplate) {
      if (!this.draft || json(next) === json(this.draft)) return;
      this.past.push(this.draft);
      if (this.past.length > FRAME_UNDO_LIMIT) this.past.shift();
      this.future = [];
      this.draft = next;
      this.error = null;
    },

    undo() {
      const prev = this.past.pop();
      if (!prev || !this.draft) return;
      this.future.push(this.draft);
      this.draft = prev;
      this.selection = null;
    },

    redo() {
      const next = this.future.pop();
      if (!next || !this.draft) return;
      this.past.push(this.draft);
      this.draft = next;
      this.selection = null;
    },

    setTool(tool: FrameTool) {
      this.tool = tool;
      if (tool !== "select") this.selection = null;
    },

    select(item: FrameItem | null) {
      this.selection = item;
    },

    /**
     * 保存。保存前チェックの誤りがあれば書かない。まだファイルが無い下書きは別名で保存の欄を開く。
     * 書けたら一覧を読み直し (図枠ダイアログ・図面の描画が新しい図枠を使う)、trueを返す。
     */
    async save(): Promise<boolean> {
      if (!this.draft) return false;
      if (this.checks.errors.length) {
        this.error = this.checks.errors.join(", ");
        return false;
      }
      if (this.isNew) {
        this.openSaveAs();
        return false;
      }
      return this.write(this.draft);
    },

    openSaveAs() {
      if (!this.draft) return;
      this.saveAsId = this.isNew ? this.draft.id : copyId(this.draft.id, this.takenIds);
      this.saveAsName = this.draft.name_ja;
      this.saveAsOpen = true;
    },

    /** 別名で保存: idはファイル名にできるもの、既にある図枠と同じidは選べない。 */
    async saveAs(): Promise<boolean> {
      if (!this.draft) return false;
      const id = this.saveAsId.trim();
      if (!isValidFrameId(id)) {
        this.error = "id";
        return false;
      }
      if (this.takenIds.includes(id)) {
        this.error = "taken";
        return false;
      }
      // 別名は新しい図枠なので、英語名も入力した名前にする (元の図枠の名前を引き継がない)
      const name = this.saveAsName.trim() || id;
      const next = { ...clone(this.draft), id, name_ja: name, name };
      const ok = await this.write(next);
      if (ok) this.saveAsOpen = false;
      return ok;
    },

    async write(template: FrameTemplate): Promise<boolean> {
      if (this.saving) return false;
      this.saving = true;
      this.error = null;
      try {
        const { template: saved } = await ipc.saveFrameTemplate(template);
        this.draft = saved;
        this.savedJson = json(saved);
        this.isNew = false;
        await useFrameTemplatesStore().reload();
        useFrameTemplatesStore().select(saved.id);
        return true;
      } catch (e) {
        this.error = String(e);
        return false;
      } finally {
        this.saving = false;
      }
    },

    /** 閉じる。未保存の変更があれば確認を出す (`force`で破棄して閉じる)。 */
    requestClose(force = false) {
      if (this.dirty && !force) {
        this.confirmClose = true;
        return;
      }
      this.close();
    },

    close() {
      this.open = false;
      this.draft = null;
      this.past = [];
      this.future = [];
      this.confirmClose = false;
      this.saveAsOpen = false;
      this.error = null;
    },
  },
});
