// 未配置部品パネル (左ドックの「未配置部品」ビュー、M4仕様 §8) の状態。
//
// 一覧の中身はRust側 (`madake_core::panel::unplaced_components`) が回路図と盤シートを
// 突き合わせて作る。外形寸法も部品DBから引いたものが載っているので、ここは
// 「読み込み・選択行」だけを持つ。図面を変えるのは配置 (`add_entity`) 側だけ。

import { defineStore } from "pinia";
import { unplacedRow, type UnplacedRow } from "../canvas/panelTools";
import { ipc, type UnplacedComponent } from "../ipc";
import { useDocumentStore } from "./document";

interface PanelState {
  unplaced: UnplacedComponent[];
  loading: boolean;
  /** 一覧が読めなかった理由 (読めなくても画面は開いたままにする)。 */
  error: string | null;
  /** 最後に読み込んだときのドキュメントrevision (図面が変わったら読み直す)。 */
  revision: number;
  /** 配置しようとしている部品の参照記号 (行の選択表示)。 */
  selected: string | null;
}

export const usePanelStore = defineStore("panel", {
  state: (): PanelState => ({
    unplaced: [],
    loading: false,
    error: null,
    revision: -1,
    selected: null,
  }),

  getters: {
    /** 一覧に並ぶ行 (参照記号・型番・外形寸法と寸法の出どころ)。 */
    rows(state): UnplacedRow[] {
      return state.unplaced.map(unplacedRow);
    },
    count(state): number {
      return state.unplaced.length;
    },
  },

  actions: {
    /** 未配置部品を読み込む。図面が変わっていなければ何もしない (`force`で強制)。 */
    async load(force = false) {
      const revision = useDocumentStore().revision;
      if (!force && this.revision === revision) return;
      this.loading = true;
      try {
        this.unplaced = await ipc.unplacedComponents();
        this.error = null;
        this.revision = revision;
      } catch (e) {
        this.unplaced = [];
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },

    /** 配置する部品の行を選ぶ (nullで解除)。 */
    select(reference: string | null) {
      this.selected = reference;
    },

    /** 参照記号から未配置部品を引く。 */
    find(reference: string): UnplacedComponent | undefined {
      return this.unplaced.find((c) => c.reference === reference);
    },
  },
});
