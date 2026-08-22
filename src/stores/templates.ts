// テンプレート選択ダイアログの状態 (M3仕様 §2、.pen「M3デザイン - テンプレート選択/検証ループ」)。
//
// テンプレートの中身 (Command列) はRust側 (madake-core) が持ち、ここは
// 「どれを選んだか」と「適用/キャンセル」だけを扱う。適用はRustで1回の編集として
// 実行されるので、undo一発で図面が元へ戻る。

import { defineStore } from "pinia";
import { ipc, type Template, type TemplateIssue } from "../ipc";

/** UI言語に合わせたテンプレート名 (日本語カタログ以外は英語名)。 */
export function templateName(template: Template, locale: string): string {
  return locale === "ja" && template.name_ja ? template.name_ja : template.name;
}

/** UI言語に合わせたテンプレートの説明。 */
export function templateDescription(template: Template, locale: string): string {
  return locale === "ja" && template.description_ja
    ? template.description_ja
    : template.description;
}

interface TemplateDialogState {
  open: boolean;
  templates: Template[];
  /** 読み込めなかったテンプレートファイル (ユーザーのフォルダに壊れたJSONがある等)。 */
  issues: TemplateIssue[];
  /** ユーザーテンプレートの置き場 (「+ ユーザーテンプレートを追加...」の案内先)。 */
  user_dir: string | null;
  /** 選択中のテンプレートid。 */
  selectedId: string | null;
  loading: boolean;
  running: boolean;
  error: string | null;
}

export const useTemplatesStore = defineStore("templates", {
  state: (): TemplateDialogState => ({
    open: false,
    templates: [],
    issues: [],
    user_dir: null,
    selectedId: null,
    loading: false,
    running: false,
    error: null,
  }),

  getters: {
    /** 選択中のテンプレート (未選択・一覧が空ならnull)。 */
    selected(state): Template | null {
      return state.templates.find((t) => t.id === state.selectedId) ?? null;
    },
  },

  actions: {
    /**
     * ダイアログを開き、テンプレート一覧を読み込む。
     * 先頭のテンプレートを選んだ状態で開く (何も選ばれていない画面を出さない)。
     */
    async openDialog() {
      this.open = true;
      this.error = null;
      this.loading = true;
      try {
        const list = await ipc.listTemplates();
        this.templates = list.templates;
        this.issues = list.issues;
        this.user_dir = list.user_dir ?? null;
        this.selectedId = list.templates[0]?.id ?? null;
      } catch (e) {
        this.templates = [];
        this.issues = [];
        this.user_dir = null;
        this.selectedId = null;
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },

    /** 一覧のタイルを選ぶ (右のプレビューが切り替わる)。 */
    select(id: string) {
      this.selectedId = id;
    },

    cancel() {
      this.open = false;
      this.error = null;
    },

    /**
     * ユーザーテンプレートの置き場をOSのファイラで開く (無ければ作られる)。
     * 開けない環境 (ブラウザ検証モード) では`opened=false`で置き場のパスだけ返し、
     * 呼び出し側が「ここへJSONを置けば並ぶ」と案内する。
     */
    async openUserFolder(): Promise<{ path: string; opened: boolean }> {
      try {
        return { path: await ipc.openTemplatesFolder(), opened: true };
      } catch {
        return { path: this.user_dir ?? "~/MadakeCAD/templates", opened: false };
      }
    },

    /**
     * 選択中のテンプレートをシートへ適用して閉じる。
     * 戻り値=適用したテンプレート (失敗時はnullで、ダイアログは開いたまま理由を出す)。
     */
    async apply(sheetId: string): Promise<Template | null> {
      const template = this.selected;
      if (!template || this.running) return null;
      this.running = true;
      this.error = null;
      try {
        await ipc.applyTemplate(template.id, sheetId);
        this.open = false;
        return template;
      } catch (e) {
        this.error = String(e);
        return null;
      } finally {
        this.running = false;
      }
    },
  },
});
