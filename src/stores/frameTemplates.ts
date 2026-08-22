// 図枠テンプレート選択ダイアログの状態 (M4仕様 §9、.pen「M4デザイン - 図枠テンプレート」)。
//
// テンプレートの実体 (JSON) はRust側が持ち、ここは「どれを選んだか」「どこへ効かせるか」
// だけを扱う。図面への適用は `set_frame_template` コマンド1回なので、undo一発で元の
// 図枠へ戻る。モデル (document ストアのミラー) を直接書き換えてはならない。
//
// 適用範囲 (デザインのラジオ):
// - このプロジェクト: いま開いている図面の図枠を差し替える (Command)
// - グローバル既定  : 新しく作る図面の初期値として設定へ保存する (図面は変えない)

import { defineStore } from "pinia";
import {
  DEFAULT_FRAME_TEMPLATE_ID,
  frameTemplateSummary,
  type FrameTemplate,
  type FrameTemplateSummary,
} from "../canvas/frameTemplate";
import { ipc, type TemplateIssue } from "../ipc";
import { useDocumentStore } from "./document";
import { useSettingsStore } from "./settings";

/** 適用範囲 (デザインのラジオ「このプロジェクト」/「グローバル既定」)。 */
export type FrameScope = "project" | "global";

/** 一覧の1行。 */
export interface FrameTemplateRow {
  template: FrameTemplate;
  /** 同梱かユーザーの図枠か (一覧のソース表示)。 */
  source: "builtin" | "user";
  /** いま図面が使っている図枠か。 */
  current: boolean;
}

/** 適用の結果 (`changed`=図面・設定を実際に書き換えたか)。 */
export interface FrameApplyResult {
  template: FrameTemplate;
  changed: boolean;
}

interface FrameTemplatesState {
  open: boolean;
  /** 読み込めなかった図枠JSON (ユーザーのフォルダに壊れたファイルがある等)。 */
  issues: TemplateIssue[];
  /** ユーザー図枠の置き場 (「+ JSONを読み込み…」の案内先)。 */
  userDir: string | null;
  selectedId: string | null;
  scope: FrameScope;
  loading: boolean;
  running: boolean;
  error: string | null;
}

export const useFrameTemplatesStore = defineStore("frameTemplates", {
  state: (): FrameTemplatesState => ({
    open: false,
    issues: [],
    userDir: null,
    selectedId: null,
    scope: "project",
    loading: false,
    running: false,
    error: null,
  }),

  getters: {
    /** 一覧に並ぶ図枠 (同梱→ユーザーの順。実体はdocumentストアが持つ)。 */
    rows(): FrameTemplateRow[] {
      const current = this.currentId;
      return useDocumentStore().frameTemplates.map((template) => ({
        template,
        source: template.builtin === false ? "user" : "builtin",
        current: template.id === current,
      }));
    },

    /** いま図面が実際に使っている図枠のid (未指定・未知idは同梱のjis-a3)。 */
    currentId(): string {
      return useDocumentStore().frameTemplate.id;
    },

    selected(state): FrameTemplate | null {
      return this.rows.find((r) => r.template.id === state.selectedId)?.template ?? null;
    },

    /** 情報行 (用紙・ゾーン・表題欄・改訂欄) の数値。 */
    summary(): FrameTemplateSummary | null {
      return this.selected ? frameTemplateSummary(this.selected) : null;
    },
  },

  actions: {
    /**
     * ダイアログを開き、図枠テンプレートの一覧を読み込む。
     * いま図面が使っている図枠を選んだ状態で開く (何も選ばれていない画面を出さない)。
     */
    async openDialog() {
      this.open = true;
      this.error = null;
      this.loading = true;
      this.scope = "project";
      try {
        await this.reload();
        this.selectedId = this.currentId || DEFAULT_FRAME_TEMPLATE_ID;
      } finally {
        this.loading = false;
      }
    },

    /** 一覧を読み直す (ユーザーがフォルダへJSONを足したあと)。 */
    async reload() {
      const list = await useDocumentStore().loadFrameTemplates();
      this.issues = list.issues;
      this.userDir = list.user_dir ?? null;
    },

    /** 一覧の行を選ぶ (中央のプレビューと右の情報が切り替わる)。 */
    select(id: string) {
      this.selectedId = id;
      this.error = null;
    },

    /** 適用範囲を切り替える。 */
    setScope(scope: FrameScope) {
      this.scope = scope;
    },

    /**
     * 選んだ図枠を適用する (閉じない)。適用範囲によって行き先が変わる:
     * - このプロジェクト: `set_frame_template` コマンドを1回だけ実行する。
     *   いま使っている図枠と同じなら**何も送らない**(空のundo段を積まない)。
     * - グローバル既定: 設定 (`default_frame_template`) へ保存する。図面は変えない。
     *
     * 失敗したらnullを返し、ダイアログは開いたまま理由を出す。
     */
    async apply(): Promise<FrameApplyResult | null> {
      const template = this.selected;
      if (!template || this.running) return null;
      this.running = true;
      this.error = null;
      try {
        if (this.scope === "global") {
          const settings = useSettingsStore();
          if (settings.settings.default_frame_template === template.id) {
            return { template, changed: false };
          }
          if (!(await settings.save({ default_frame_template: template.id }))) {
            this.error = settings.error ?? "";
            return null;
          }
          return { template, changed: true };
        }
        if (template.id === this.currentId) return { template, changed: false };
        await useDocumentStore().execute({
          type: "set_frame_template",
          frame_template: template.id,
        });
        return { template, changed: true };
      } catch (e) {
        this.error = String(e);
        return null;
      } finally {
        this.running = false;
      }
    },

    /** 適用してから閉じる (フッタのOK)。適用に失敗したら開いたまま。 */
    async ok(): Promise<FrameApplyResult | null> {
      const applied = await this.apply();
      if (applied) this.close();
      return applied;
    },

    /** 何も適用せずに閉じる。 */
    cancel() {
      this.close();
    },

    close() {
      this.open = false;
      this.error = null;
    },

    /**
     * ユーザー図枠の置き場をOSのファイラで開き、戻ってきたときに一覧を読み直す。
     * 開けない環境 (ブラウザ検証モード) では`opened=false`で置き場のパスだけ返し、
     * 呼び出し側が「ここへJSONを置けば並ぶ」と案内する。
     */
    async openUserFolder(): Promise<{ path: string; opened: boolean }> {
      try {
        const path = await ipc.openFrameTemplatesFolder();
        await this.reload();
        return { path, opened: true };
      } catch {
        return { path: this.userDir ?? "~/MadakeCAD/templates/frames", opened: false };
      }
    },
  },
});
