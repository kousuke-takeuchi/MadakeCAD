// アプリ内アップデートの状態 (.pen「自動更新デザイン - バージョン確認と更新」`qyYGO`)。
//
// 確認先はGitHub Releasesの最新リリースに添付された`latest.json` (tauri-plugin-updater。
// 設定は src-tauri/tauri.conf.json の plugins.updater)。更新ファイルはリリース用の鍵で署名
// されていて、アプリに埋め込んだ公開鍵で確かめてから入れ替える (確かめるのはプラグイン)。
// プレリリースは「最新」に含まれないので更新として出てこない。
//
// - 起動時の確認 (`startupCheck`): 設定`check_updates`がオンのときだけ。見つからない・
//   通信できないときは何も出さない (オフラインで邪魔をしない)
// - 設定の「今すぐ確認」(`check`): 結果 (最新です / 新しい版 / 失敗の理由) を必ず出す
// - 「更新して再起動」(`install`): 未保存の図面があれば先に確認し、ダウンロード→入れ替え→再起動
// - Linuxの.deb/.rpm (`manual`) は知らせるだけで、リリースページを開く

import { defineStore } from "pinia";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { nativeDialogs } from "../composables/nativeDialogs";
import { ipc, inTauri, type UpdateSupport } from "../ipc";
import { useProjectFileStore } from "./projectFile";

/** リリースの一覧ページ (リリースノート・手動での入れ替え)。 */
export const RELEASES_URL = "https://github.com/kousuke-takeuchi/MadakeCAD/releases/latest";

export type UpdateStatus = "idle" | "checking" | "upToDate" | "available" | "downloading" | "installing" | "error";

export interface AvailableUpdate {
  version: string;
  /** 公開日 (latest.jsonの`pub_date`。無ければnull)。 */
  date: string | null;
  /** リリースノート (latest.jsonの`notes`)。 */
  notes: string;
}

interface UpdatesState {
  support: UpdateSupport | null;
  currentVersion: string;
  status: UpdateStatus;
  available: AvailableUpdate | null;
  /** ダウンロード済みバイト数と全体 (全体が分からなければnull)。 */
  downloaded: number;
  total: number | null;
  lastChecked: Date | null;
  error: string | null;
  /** 起動時の確認で見つけたか (作図画面右下の通知カードを出す)。 */
  foundAtStartup: boolean;
  /** 「後で」を押した版 (この起動中はその版の通知を出さない)。 */
  dismissedVersion: string | null;
}

/** プラグインの更新オブジェクト (Rust側の資源を持つので、リアクティブにはしない)。 */
let pending: Update | null = null;

export const useUpdatesStore = defineStore("updates", {
  state: (): UpdatesState => ({
    support: null,
    currentVersion: "",
    status: "idle",
    available: null,
    downloaded: 0,
    total: null,
    lastChecked: null,
    error: null,
    foundAtStartup: false,
    dismissedVersion: null,
  }),

  getters: {
    /** この環境で確認できるか (開発ビルド・ブラウザ検証モードは確認しない)。 */
    canCheck: (state) => state.support === "supported" || state.support === "manual",
    /** 作図画面右下の通知カードを出すか。 */
    showNotice: (state) =>
      state.foundAtStartup &&
      !!state.available &&
      state.available.version !== state.dismissedVersion &&
      (state.status === "available" || state.status === "downloading" || state.status === "installing" || state.status === "error"),
    /** ダウンロードの進み具合 0..1 (全体が分からなければnull)。 */
    progress: (state) => (state.total ? Math.min(1, state.downloaded / state.total) : null),
  },

  actions: {
    /** 現在のバージョンと、この環境での扱いを読む (起動時に1回)。 */
    async init() {
      if (this.support) return;
      this.support = await ipc.updateSupport().catch(() => "unsupported" as const);
      if (inTauri) this.currentVersion = await getVersion().catch(() => "");
    },

    /** 起動時の確認。設定でオフ、または確認できない環境なら何もしない。失敗しても何も出さない。 */
    async startupCheck(enabled: boolean) {
      await this.init();
      if (!enabled || !this.canCheck) return;
      const found = await this.check(true);
      if (found) this.foundAtStartup = true;
    },

    /**
     * 新しい版を確認する。見つかればtrue。`silent`なら失敗を状態に残さない (起動時の確認)。
     */
    async check(silent = false): Promise<boolean> {
      await this.init();
      if (!this.canCheck || this.status === "checking" || this.status === "downloading" || this.status === "installing") {
        return false;
      }
      this.status = "checking";
      this.error = null;
      try {
        const update = await check();
        this.lastChecked = new Date();
        await pending?.close().catch(() => undefined);
        pending = update;
        if (!update) {
          this.available = null;
          this.status = "upToDate";
          return false;
        }
        this.available = { version: update.version, date: update.date ?? null, notes: update.body ?? "" };
        this.status = "available";
        return true;
      } catch (e) {
        this.status = silent ? "idle" : "error";
        this.error = silent ? null : String(e);
        return false;
      }
    },

    /**
     * 更新して再起動する。未保存の図面があれば先に確認し、断られたら何もしない。
     * ダウンロード中は進み具合を持ち、入れ替えが済んだらアプリを再起動する。
     * `manual` (Linuxの.deb/.rpm) はリリースページを開くだけ。
     */
    async install(confirmText: string): Promise<boolean> {
      if (this.support === "manual") {
        await this.openReleasePage();
        return false;
      }
      if (!pending || this.status !== "available") return false;
      if (useProjectFileStore().dirty && !(await nativeDialogs.confirm(confirmText, { kind: "warning" }))) return false;
      this.status = "downloading";
      this.downloaded = 0;
      this.total = null;
      this.error = null;
      try {
        await pending.downloadAndInstall((event) => {
          if (event.event === "Started") this.total = event.data.contentLength ?? null;
          else if (event.event === "Progress") this.downloaded += event.data.chunkLength;
          else this.status = "installing";
        });
        await relaunch();
        return true;
      } catch (e) {
        this.status = "error";
        this.error = String(e);
        return false;
      }
    },

    /** 通知カードの「後で」: この起動中はその版の通知を出さない。 */
    dismiss() {
      this.dismissedVersion = this.available?.version ?? null;
      if (this.status === "error") this.status = this.available ? "available" : "idle";
    },

    /** リリースページを開く (リリースノート・手動での入れ替え)。 */
    async openReleasePage() {
      if (inTauri) await openUrl(RELEASES_URL);
      else window.open(RELEASES_URL, "_blank", "noopener");
    },
  },
});
