// 参照サーフィン (Surfer, spec §4) のポップアップ状態。
//
// キャンバス上の要素をAlt(Option)+クリックすると、「同じものが図面のどこに出てくるか」を
// 一覧するポップアップが出る。↑↓で巡回、Enterでジャンプ、Escで閉じる。
// 一覧の作り方は`canvas/surfer.ts`の純関数、ジャンプは検索・ナビゲータと同じreveal機構。

import { defineStore } from "pinia";
import { cycleIndex } from "../canvas/search";
import { surferTargetFor, type SurferSite, type SurferTarget } from "../canvas/surfer";
import type { Project } from "../ipc";
import { useDevicesStore } from "./devices";

interface SurferState {
  target: SurferTarget | null;
  /** 今どの所在を見ているか (0始まり)。開いた直後は先頭。 */
  index: number;
  /** ポップアップを出す画面座標 (クリック位置)。 */
  at: { x: number; y: number };
}

export const useSurferStore = defineStore("surfer", {
  state: (): SurferState => ({
    target: null,
    index: 0,
    at: { x: 0, y: 0 },
  }),

  getters: {
    open(state): boolean {
      return state.target !== null;
    },
    sites(state): SurferSite[] {
      return state.target?.sites ?? [];
    },
    /** 今見ている所在 (空ならnull)。 */
    activeSite(state): SurferSite | null {
      return state.target?.sites[state.index] ?? null;
    },
  },

  actions: {
    /**
     * Alt+クリックされた要素のサーフィンを始める。
     * 巡回先が無い要素 (参照記号も名前も番号も無い) では何も開かず false を返す。
     */
    async openAt(
      project: Project | null,
      sheetId: string,
      entityId: string,
      at: { x: number; y: number },
    ): Promise<boolean> {
      const devices = useDevicesStore();
      await devices.load();
      const target = surferTargetFor(project, devices.devices, sheetId, entityId);
      if (!target || target.sites.length === 0) {
        this.target = null;
        return false;
      }
      this.target = target;
      this.at = at;
      // 押した要素そのものが一覧にあれば、そこから巡回を始める
      const here = target.sites.findIndex((s) => s.entityId === entityId);
      this.index = here >= 0 ? here : 0;
      return true;
    },

    close() {
      this.target = null;
      this.index = 0;
    },

    /** ↑↓の巡回。端まで行ったら反対の端へ回り込む。 */
    step(step: number): SurferSite | null {
      this.index = Math.max(0, cycleIndex(this.sites.length, this.index, step));
      return this.activeSite;
    },

    /** 行クリック。その行を選んで返す (呼び出し側がジャンプする)。 */
    select(index: number): SurferSite | null {
      if (index < 0 || index >= this.sites.length) return null;
      this.index = index;
      return this.sites[index];
    },
  },
});
