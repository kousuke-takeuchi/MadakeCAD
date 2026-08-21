// 改訂欄編集ダイアログの状態 (仕様: docs/internal/specs/m2-drawing-parity.md §1)。
// 編集はこのストアの下書き(draft)の上だけで行い、保存時に set_revisions コマンドを
// 1回だけ実行する。モデル(document ストアのミラー)を直接書き換えてはならない。

import { defineStore } from "pinia";
import type { Revision, Sheet } from "../ipc";
import { useDocumentStore } from "./document";

/** 改訂欄の並び順は「古い→新しい」。表示は逆順(最新が最上段)。 */
export interface RevisionRow {
  /** draft内の位置 (0=最も古い)。 */
  index: number;
  rev: Revision;
}

/** アルファベット記号(A..Z, AA..)を1つ進める。 */
function incrementMark(mark: string): string {
  const chars = mark.split("");
  for (let i = chars.length - 1; i >= 0; i--) {
    if (chars[i] !== "Z") {
      chars[i] = String.fromCharCode(chars[i].charCodeAt(0) + 1);
      return chars.join("");
    }
    chars[i] = "A";
  }
  return `A${chars.join("")}`;
}

/**
 * 次の改訂記号を返す。既存のアルファベット記号の最大値+1で、
 * 空欄や数字だけの記号は無視する。1件も無ければ "A"。
 */
export function nextRevisionMark(revisions: readonly Revision[]): string {
  let max = "";
  for (const r of revisions) {
    const mark = (r.mark ?? "").trim().toUpperCase();
    if (!/^[A-Z]+$/.test(mark)) continue;
    if (mark.length > max.length || (mark.length === max.length && mark > max)) max = mark;
  }
  return max === "" ? "A" : incrementMark(max);
}

/** 日付をYYYY-MM-DD (ローカル時刻) で返す。既定は今日。 */
export function todayIso(date: Date = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** 保存する一覧を整える: 各欄をトリムし、全欄が空の行を落とす。 */
function normalize(draft: readonly Revision[]): Revision[] {
  return draft
    .map((r) => ({
      mark: r.mark.trim(),
      date: r.date.trim(),
      description: r.description.trim(),
      by: r.by.trim(),
    }))
    .filter((r) => r.mark || r.date || r.description || r.by);
}

interface RevisionsState {
  open: boolean;
  sheetId: string | null;
  sheetName: string;
  /** 編集中の一覧 (古い順)。シートの配列とは別物のコピー。 */
  draft: Revision[];
  /** 開いた時点の一覧。変更の有無の判定に使う。 */
  original: Revision[];
  /** 選択中の行 (draftの位置)。 */
  selected: number | null;
  saving: boolean;
  error: string | null;
}

export const useRevisionsStore = defineStore("revisions", {
  state: (): RevisionsState => ({
    open: false,
    sheetId: null,
    sheetName: "",
    draft: [],
    original: [],
    selected: null,
    saving: false,
    error: null,
  }),

  getters: {
    /** 表示用の行 (最新が先頭)。 */
    rows(state): RevisionRow[] {
      return state.draft.map((rev, index) => ({ index, rev })).reverse();
    },
    /** 開いた時点から内容が変わったか。 */
    dirty(state): boolean {
      return JSON.stringify(normalize(state.draft)) !== JSON.stringify(normalize(state.original));
    },
  },

  actions: {
    /** シートの改訂一覧を下書きへ複製してダイアログを開く。 */
    openFor(sheet: Sheet) {
      const list = (sheet.revisions ?? []).map((r) => ({ ...r }));
      this.sheetId = sheet.id;
      this.sheetName = sheet.name;
      this.draft = list;
      this.original = list.map((r) => ({ ...r }));
      this.selected = null;
      this.error = null;
      this.open = true;
    },

    /** 末尾(=最新)へ1行足す。記号は次のアルファベット、日付は今日。 */
    addRow(): number {
      const row: Revision = {
        mark: nextRevisionMark(this.draft),
        date: todayIso(),
        description: "",
        by: "",
      };
      this.draft.push(row);
      this.selected = this.draft.length - 1;
      return this.selected;
    },

    updateRow(index: number, patch: Partial<Revision>) {
      const row = this.draft[index];
      if (row) Object.assign(row, patch);
    },

    removeRow(index: number) {
      if (index < 0 || index >= this.draft.length) return;
      this.draft.splice(index, 1);
      this.selected = null;
    },

    select(index: number | null) {
      this.selected = index;
    },

    /** 編集を捨てて閉じる (コマンドは送らない)。 */
    cancel() {
      this.open = false;
      this.draft = [];
      this.original = [];
      this.selected = null;
      this.error = null;
    },

    /**
     * set_revisionsコマンドを1回実行して閉じる。変更が無ければ何も送らない。
     * 失敗したら開いたままエラーを表示する。戻り値=保存した行数 (未送信ならnull)。
     */
    async save(): Promise<number | null> {
      if (!this.sheetId || this.saving) return null;
      if (!this.dirty) {
        this.cancel();
        return null;
      }
      const revisions = normalize(this.draft);
      this.saving = true;
      this.error = null;
      try {
        await useDocumentStore().execute({
          type: "set_revisions",
          sheet_id: this.sheetId,
          revisions,
        });
      } catch (e) {
        this.error = String(e);
        return null;
      } finally {
        this.saving = false;
      }
      this.cancel();
      return revisions.length;
    },
  },
});
