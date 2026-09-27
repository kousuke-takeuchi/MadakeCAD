// プロジェクトファイルの状態: いま開いている`.mdkproj`のパス、未保存の編集の有無、
// 最近使ったファイル。図面そのものはdocument storeのミラーで、ここは「どのファイルと
// 対応しているか」だけを持つ。ファイルの読み書きはRust側(IPC)で行う。

import { defineStore } from "pinia";
import { ipc, type KicadImportResult, type Patch } from "../ipc";
import { useDocumentStore } from "./document";

/** 最近使ったファイルの上限 (リボンのホームタブに3件ずつの縦列で並ぶ)。 */
export const RECENT_LIMIT = 8;
/** 最近使ったファイルの保存キー (localStorage)。 */
export const RECENT_STORAGE_KEY = "madakecad.recentProjects";

/** 最近使ったファイル一覧の保管先。既定はlocalStorage、テストはメモリ実装を差し込む。 */
export interface RecentStorage {
  get(): string[];
  set(list: string[]): void;
}

function localRecentStorage(): RecentStorage {
  return {
    get() {
      try {
        const raw = globalThis.localStorage?.getItem(RECENT_STORAGE_KEY);
        const parsed: unknown = raw ? JSON.parse(raw) : [];
        return Array.isArray(parsed) ? parsed.filter((p): p is string => typeof p === "string") : [];
      } catch {
        return [];
      }
    },
    set(list) {
      try {
        globalThis.localStorage?.setItem(RECENT_STORAGE_KEY, JSON.stringify(list));
      } catch {
        // 保管できない環境(プライベートモード等)では一覧はセッション内だけ生きる
      }
    },
  };
}

/** パスの末尾のファイル名 (区切りは`/`と`\`の両方を見る)。 */
export function fileNameOf(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** `.kicad_sch`ならKiCadの回路図として読み込む (プロジェクトファイルではない)。 */
export function isKicadPath(path: string): boolean {
  return path.toLowerCase().endsWith(".kicad_sch");
}

/** `.dxf`ならAutoCAD Electrical / EPLAN の中間形式として読み込む (プロジェクトファイルではない)。 */
export function isDxfPath(path: string): boolean {
  return path.toLowerCase().endsWith(".dxf");
}

interface ProjectFileState {
  /** いま開いている`.mdkproj`のパス。新規・KiCad読込直後はnull (保存先未定)。 */
  path: string | null;
  /** ディスク上のファイルと一致しているドキュメントrevision。 */
  savedRevision: number;
  /** 最近使ったファイル (新しい順)。 */
  recent: string[];
  storage: RecentStorage;
}

export const useProjectFileStore = defineStore("projectFile", {
  state: (): ProjectFileState => ({
    path: null,
    savedRevision: 0,
    recent: [],
    storage: localRecentStorage(),
  }),

  getters: {
    /** ディスクに無い編集があるか。 */
    dirty(state): boolean {
      return useDocumentStore().revision !== state.savedRevision;
    },
    /** 開いているファイルの名前 (パス無しならnull)。 */
    fileName(state): string | null {
      return state.path ? fileNameOf(state.path) : null;
    },
  },

  actions: {
    /**
     * 起動直後に呼ぶ: 読み込み済みのドキュメントを「保存済み」の基準にし、
     * 最近使ったファイルを保管先から読む。
     */
    attach(storage?: RecentStorage) {
      if (storage) this.storage = storage;
      this.savedRevision = useDocumentStore().revision;
      this.recent = this.storage.get().slice(0, RECENT_LIMIT);
    },

    /** 読み込んだプロジェクトをミラーへ入れる。置換なのでundo/redo履歴は空になる。 */
    applyReplacement(patch: Patch) {
      const doc = useDocumentStore();
      doc.applyPatch(patch);
      doc.canUndo = false;
      doc.canRedo = false;
    },

    /** 空の新規プロジェクトを始める。保存先は無く、未保存の編集も無い状態から始まる。 */
    async create(name: string): Promise<Patch> {
      const patch = await ipc.newProject(name);
      this.applyReplacement(patch);
      this.path = null;
      this.savedRevision = patch.revision;
      return patch;
    },

    /** `.mdkproj`を開く。成功したら保存先として覚え、最近使ったファイルの先頭に置く。 */
    async open(path: string): Promise<Patch> {
      const patch = await ipc.loadProject(path);
      this.applyReplacement(patch);
      this.path = path;
      this.savedRevision = patch.revision;
      this.addRecent(path);
      return patch;
    },

    /**
     * KiCad回路図を読み込む。プロジェクトファイルではないので保存先は無く、
     * 読み込んだ内容は未保存の編集として扱う (閉じる前に保存を促す)。
     */
    async importKicad(path: string): Promise<KicadImportResult> {
      const result = await ipc.importKicad(path);
      this.applyReplacement(result.patch);
      this.path = null;
      this.addRecent(path);
      return result;
    },

    /**
     * DXF (AutoCAD Electrical / EPLAN の中間形式) を読み込む。KiCadと同じく保存先は無く、
     * 読み込んだ内容は未保存の編集として扱う。
     */
    async importDxf(path: string): Promise<KicadImportResult> {
      const result = await ipc.importDxf(path);
      this.applyReplacement(result.patch);
      this.path = null;
      this.addRecent(path);
      return result;
    },

    /**
     * 開いているファイルへ上書き保存する。保存先が無ければ何もせずnullを返す
     * (呼び出し側が「名前を付けて保存」へ回す)。
     */
    async save(): Promise<string | null> {
      if (!this.path) return null;
      await this.saveAs(this.path);
      return this.path;
    },

    /**
     * 指定パスへ保存し、そのパスを保存先として覚える。保存中に届いた編集
     * (エージェント等) はディスクに無いので、保存開始時のrevisionを基準にする。
     */
    async saveAs(path: string): Promise<void> {
      const revisionAtSave = useDocumentStore().revision;
      await ipc.saveProject(path);
      this.path = path;
      this.savedRevision = revisionAtSave;
      this.addRecent(path);
    },

    /** 最近使ったファイルの先頭へ置く (重複は先頭へ移動、上限を超えた分は落とす)。 */
    addRecent(path: string) {
      this.recent = [path, ...this.recent.filter((p) => p !== path)].slice(0, RECENT_LIMIT);
      this.storage.set(this.recent);
    },

    /** 開けなくなったファイルなどを一覧から外す。 */
    removeRecent(path: string) {
      this.recent = this.recent.filter((p) => p !== path);
      this.storage.set(this.recent);
    },
  },
});
