// 整えバリアント (M3フェーズ4): 同じ整え指示を2〜4案の並列会話で走らせ、比較して1案だけ採用する。
//
// 仕組みはシート複製 (計画: docs/superpowers/plans/2026-09-27-m3-phase4-tidy-variants.md)。
// 複製・写し戻し・破棄のCommand組み立てはRust側 (madake-core variants.rs) で、
// ここは開始→案ごとの会話送信→指標の取得→採用/破棄の進行と、比較パネルの状態だけを持つ。
// 元id→複製idの対応表 (id_map) は開始で受け取り、終了でそのまま返す。

import { defineStore } from "pinia";
import { agentApi, useChatStore } from "./chat";
import { useDocumentStore } from "./document";
import { useUiStore } from "./ui";
import { i18n } from "../i18n";
import { ipc, type TidyMetrics, type VariantInfo, type VariantRun } from "../ipc";
import { tidyPrompt, type TidyMode } from "../composables/tidy";

/** 案の数の範囲 (ポップアップのチップ 2/3/4)。 */
export const VARIANT_COUNTS = [2, 3, 4] as const;

/** 1案の進行状態。 */
export interface VariantEntry extends VariantInfo {
  /** 整えを走らせている会話id (送信に失敗したらnull)。 */
  conversationId: string | null;
  metrics: TidyMetrics | null;
}

export interface VariantsRunState {
  mode: TidyMode;
  originalSheetId: string;
  variants: VariantEntry[];
  /** 元シートの指標 (比較の基準)。 */
  baseline: TidyMetrics | null;
}

interface VariantsState {
  run: VariantsRunState | null;
  panelOpen: boolean;
  finishing: boolean;
}

/** 元シートでの選択idを、対応表で複製側のidへ写す (対応の無いidは落とす)。 */
export function mapSelection(selection: Iterable<string>, idMap: Record<string, string>): string[] {
  const out: string[] = [];
  for (const id of selection) {
    const mapped = idMap[id];
    if (mapped) out.push(mapped);
  }
  return out;
}

/** 指標の合計 (Rustの`TidyMetrics::total`と同じ)。 */
export function metricsTotal(m: TidyMetrics): number {
  return m.crossings + m.label_overlaps + m.symbol_overlaps + m.off_grid;
}

function runInfo(run: VariantsRunState): VariantRun {
  return {
    original_sheet_id: run.originalSheetId,
    variants: run.variants.map(({ sheet_id, label, id_map }) => ({ sheet_id, label, id_map })),
  };
}

export const useVariantsStore = defineStore("variants", {
  state: (): VariantsState => ({
    run: null,
    panelOpen: false,
    finishing: false,
  }),

  getters: {
    /** 案の会話がまだ答えている最中か。 */
    isRunning(): (variant: VariantEntry) => boolean {
      const chat = useChatStore();
      return (variant) => !!variant.conversationId && !!chat.running[variant.conversationId];
    },
    runningCount(state): number {
      if (!state.run) return 0;
      return state.run.variants.filter((v) => this.isRunning(v)).length;
    },
    /** 全ての案が終わった (採用できる) か。 */
    allDone(state): boolean {
      return !!state.run && this.runningCount === 0;
    },
  },

  actions: {
    /**
     * 開始: 表示中のシートを`count`枚複製し、案ごとに新しい会話で整えを送る。
     * 選択があれば対応表で複製側のidへ写して範囲にする。戻り値=開始できたか。
     */
    async start(mode: TidyMode, count: number): Promise<boolean> {
      const t = i18n.global.t;
      const doc = useDocumentStore();
      const chat = useChatStore();
      const ui = useUiStore();
      if (this.run) {
        ui.log(t("chat.tidy.variantBusyLog"));
        return false;
      }
      const sheet = doc.activeSheet;
      if (!sheet) {
        ui.log(t("chat.tidy.variantNoSheetLog"));
        return false;
      }
      if (!(VARIANT_COUNTS as readonly number[]).includes(count)) return false;
      const selection = [...doc.selection];
      let started: { patch: Parameters<typeof doc.applyEdit>[0]; run: VariantRun };
      try {
        started = await ipc.startVariants(sheet.id, count);
      } catch (e) {
        ui.log(t("chat.tidy.variantStartFailedLog", { message: String(e) }));
        return false;
      }
      doc.applyEdit(started.patch);
      ui.log(t("chat.tidy.variantStartedLog", { mode: t(`chat.tidy.mode.${mode}`), count }));
      ui.openAgentTab();
      const variants: VariantEntry[] = [];
      for (const info of started.run.variants) {
        const copy = doc.project?.sheets.find((s) => s.id === info.sheet_id) ?? null;
        chat.newConversation();
        const conversationId = await chat.send(
          tidyPrompt(mode, copy, mapSelection(selection, info.id_map)),
        );
        variants.push({ ...info, conversationId, metrics: null });
      }
      this.run = { mode, originalSheetId: sheet.id, variants, baseline: null };
      this.panelOpen = true;
      await this.refreshMetrics();
      return true;
    },

    /** 元シートと各案の指標を取り直す (取れない案はnullのまま)。 */
    async refreshMetrics() {
      const run = this.run;
      if (!run) return;
      const fetch = (sheetId: string) => ipc.getTidyMetrics(sheetId).catch(() => null);
      const [baseline, ...metrics] = await Promise.all([
        fetch(run.originalSheetId),
        ...run.variants.map((v) => fetch(v.sheet_id)),
      ]);
      if (this.run !== run) return;
      run.baseline = baseline;
      run.variants.forEach((v, i) => {
        v.metrics = metrics[i] ?? null;
      });
    },

    /** 案のシートを図面タブで表示する。 */
    show(sheetId: string) {
      useDocumentStore().activeSheetId = sheetId;
    },

    /** 採用: 選んだ案を元シートへ写し戻し、全複製を消す (undo一発)。実行中の案があれば断る。 */
    async adopt(sheetId: string): Promise<boolean> {
      const run = this.run;
      if (!run || !this.allDone || this.finishing) return false;
      const chosen = run.variants.find((v) => v.sheet_id === sheetId);
      if (!chosen) return false;
      const ok = await this.finish(run, sheetId);
      if (ok) {
        useUiStore().log(i18n.global.t("variants.adoptedLog", { label: chosen.label }));
      }
      return ok;
    },

    /** 破棄: 走っている案の会話を止めてから全複製を消す (undo一発)。 */
    async discard(): Promise<boolean> {
      const run = this.run;
      if (!run || this.finishing) return false;
      for (const v of run.variants) {
        if (this.isRunning(v) && v.conversationId) {
          await agentApi.cancel(v.conversationId).catch(() => undefined);
        }
      }
      const ok = await this.finish(run, null);
      if (ok) useUiStore().log(i18n.global.t("variants.discardedLog"));
      return ok;
    },

    async finish(run: VariantsRunState, chosenSheetId: string | null): Promise<boolean> {
      const doc = useDocumentStore();
      this.finishing = true;
      try {
        const patch = await ipc.finishVariants(runInfo(run), chosenSheetId);
        doc.applyEdit(patch);
      } catch (e) {
        useUiStore().log(i18n.global.t("variants.finishFailedLog", { message: String(e) }));
        return false;
      } finally {
        this.finishing = false;
      }
      doc.activeSheetId = run.originalSheetId;
      doc.selection = new Set();
      this.run = null;
      this.panelOpen = false;
      return true;
    },

    open() {
      if (this.run) this.panelOpen = true;
    },
    close() {
      this.panelOpen = false;
    },
  },
});
