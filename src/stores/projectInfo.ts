// プロジェクト情報ダイアログの状態 (G16)。編集はこのストアの下書き(draft)の上だけで行い、
// 保存時に set_project_meta コマンドを1回だけ実行する (変わった欄だけを送る)。
// モデル(document ストアのミラー)を直接書き換えてはならない。

import { defineStore } from "pinia";
import { useDocumentStore } from "./document";

interface ProjectInfoDraft {
  name: string;
  no: string;
  company: string;
  drawingNo: string;
  title: string;
}

interface ProjectInfoState {
  open: boolean;
  draft: ProjectInfoDraft;
  original: ProjectInfoDraft;
  saving: boolean;
  error: string | null;
}

function empty(): ProjectInfoDraft {
  return { name: "", no: "", company: "", drawingNo: "", title: "" };
}

export const useProjectInfoStore = defineStore("projectInfo", {
  state: (): ProjectInfoState => ({
    open: false,
    draft: empty(),
    original: empty(),
    saving: false,
    error: null,
  }),

  getters: {
    /** 開いた時点から内容が変わったか。 */
    dirty(state): boolean {
      const a = state.draft;
      const b = state.original;
      return a.name !== b.name || a.no !== b.no || a.company !== b.company || a.drawingNo !== b.drawingNo || a.title !== b.title;
    },
  },

  actions: {
    /**
     * プロジェクト名・番号と、現在のシートの表題欄(会社名・図番・表題)を下書きへ
     * 複製してダイアログを開く。表題欄側は全シート共通の値として編集する
     * (保存時に全シートへ同じ値を書く)。
     */
    openDialog() {
      const doc = useDocumentStore();
      const project = doc.project;
      const sheet = doc.activeSheet;
      const draft: ProjectInfoDraft = {
        name: project?.name ?? "",
        no: project?.no ?? "",
        company: sheet?.title_block.company ?? "",
        drawingNo: sheet?.title_block.drawing_no ?? "",
        title: sheet?.title_block.title ?? "",
      };
      this.draft = { ...draft };
      this.original = { ...draft };
      this.error = null;
      this.open = true;
    },

    /** 編集を捨てて閉じる (コマンドは送らない)。 */
    cancel() {
      this.open = false;
      this.draft = empty();
      this.original = empty();
      this.error = null;
    },

    /**
     * set_project_metaコマンドを1回実行して閉じる。変わった欄だけを送るので、
     * 触っていない欄は他シートの値を上書きしない。変更が無ければ何も送らない。
     * 失敗したら開いたままエラーを表示する。戻り値=送信したか。
     */
    async save(): Promise<boolean> {
      if (this.saving) return false;
      if (!this.dirty) {
        this.cancel();
        return false;
      }
      const doc = useDocumentStore();
      const a = this.draft;
      const b = this.original;
      this.saving = true;
      this.error = null;
      try {
        await doc.execute({
          type: "set_project_meta",
          name: a.name !== b.name ? a.name : undefined,
          no: a.no !== b.no ? a.no : undefined,
          company: a.company !== b.company ? a.company : undefined,
          drawing_no: a.drawingNo !== b.drawingNo ? a.drawingNo : undefined,
          title: a.title !== b.title ? a.title : undefined,
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
