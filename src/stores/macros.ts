// 回路マクロUIの状態 (M4仕様 §2、.pen「M4デザイン - 回路マクロ」)。
//
// マクロの中身 (Command列) はRust側 (madake-core) が持ち、ここは「どれを選んだか」
// 「何を保存するか」「⌘Cで何を覚えているか」だけを扱う。挿入はRustで1回の編集として
// 実行されるので、undo一発で図面が元へ戻る。
//
// ⌘C/Vの無名マクロはファイルに書かず、このストアのメモリにだけ置く
// (`buildMacro`で組み立て、`applyMacroInline`でマクロそのものを渡して挿入する)。

import { defineStore } from "pinia";
import { macroVariantKeys, DEFAULT_VARIANT } from "../canvas/macroPreview";
import {
  ipc,
  type Macro,
  type MacroIssue,
  type MacroMeta,
  type MacroPlaceholder,
  type MacroValueSet,
} from "../ipc";

/** カテゴリツリーの「すべて」。実在の分類名と衝突しないよう記号を使う。 */
export const ALL_CATEGORIES = "*";
/** カテゴリが空のマクロを束ねるグループ (UIでは「ユーザー」)。 */
export const UNCATEGORIZED = "";

/** 挿入ダイアログのタイル1枚 (バリアント数バッジ付き)。 */
export interface MacroTile {
  macro: Macro;
  /** 既定の"A"を含めたバリアント数 (タイル右上のバッジ)。 */
  variantCount: number;
}

/**
 * 保存ダイアログの「プレースホルダ」表の1行 = 選択範囲の中の書き換えられる欄1つ。
 * ここへキー名を付けた行がプレースホルダになる。
 */
export interface PlaceholderCandidate {
  /** 行のid (`<エンティティid>|<欄>`)。 */
  id: string;
  entity: string;
  /** `"value"` または `"attrs.<名前>"`。 */
  field: string;
  reference: string;
  symbolId: string;
  /** 保存時点で入っている値 (表に薄く出す)。 */
  current: string;
}

/** 保存ダイアログの「値セット」表の1行 (入力中の値セット)。 */
export interface ValueSetDraft {
  label: string;
  /** プレースホルダのキー → 値。 */
  values: Record<string, string>;
}

/**
 * 表示名から安定したidを作る (英数字以外は`_`)。Rustの`derive_id`と同じ規則なので、
 * 「1.5 kW」→`1_5_kw`のように保存前後で同じidになる。
 */
export function deriveValueSetId(label: string): string {
  let id = "";
  for (const ch of label) {
    if (/[0-9A-Za-z]/.test(ch)) id += ch.toLowerCase();
    else if (!id.endsWith("_")) id += "_";
  }
  return id.replace(/^_+|_+$/g, "");
}

interface MacroState {
  macros: Macro[];
  /** 読み込めなかったマクロファイル (ユーザーのフォルダに壊れたJSONがある等)。 */
  issues: MacroIssue[];
  /** ユーザーマクロの置き場 (「ここにJSONを置けば並ぶ」の案内先)。 */
  user_dir: string | null;
  loading: boolean;

  /** 左ツリーで選択中のカテゴリ。 */
  category: string;
  /** タイルの絞り込み語 (名前・id の部分一致)。 */
  query: string;
  /** 選択中のタイル。 */
  selectedId: string | null;
  /** 選択中タイルのバリアントキー (配置中はTabでも変わる)。 */
  variantKey: string;
  /** 選択中タイルの値セットid (未選択=保存時の値のまま挿入する)。 */
  valueSetId: string | null;

  /** 保存ダイアログ。 */
  saveOpen: boolean;
  saveName: string;
  saveCategory: string;
  /** 選択範囲から組み立てた保存前のマクロ (プレビューと基準点の表示に使う)。 */
  savePreview: Macro | null;
  saveSheetId: string | null;
  saveEntityIds: string[];
  saving: boolean;
  /** プレースホルダ表で入力したキー名 (行id → キー。空の行はプレースホルダにしない)。 */
  placeholderKeys: Record<string, string>;
  /** 値セット表で入力中の行。 */
  valueSetDrafts: ValueSetDraft[];

  /** ⌘Cで覚えた無名マクロ (ファイルには書かない)。 */
  clipboard: Macro | null;

  error: string | null;
}

export const useMacrosStore = defineStore("macros", {
  state: (): MacroState => ({
    macros: [],
    issues: [],
    user_dir: null,
    loading: false,
    category: ALL_CATEGORIES,
    query: "",
    selectedId: null,
    variantKey: DEFAULT_VARIANT,
    valueSetId: null,
    saveOpen: false,
    saveName: "",
    saveCategory: "",
    savePreview: null,
    saveSheetId: null,
    saveEntityIds: [],
    saving: false,
    placeholderKeys: {},
    valueSetDrafts: [],
    clipboard: null,
    error: null,
  }),

  getters: {
    /**
     * 左ツリーのカテゴリ一覧: 「すべて」が先頭、次に分類名を昇順、
     * 分類の無いマクロ (UIでは「ユーザー」) が最後。
     */
    categories(state): { key: string; count: number }[] {
      const counts = new Map<string, number>();
      for (const m of state.macros) {
        const key = m.category.trim();
        counts.set(key, (counts.get(key) ?? 0) + 1);
      }
      const named = [...counts.entries()]
        .filter(([key]) => key !== UNCATEGORIZED)
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([key, count]) => ({ key, count }));
      const rest = counts.get(UNCATEGORIZED);
      return [
        { key: ALL_CATEGORIES, count: state.macros.length },
        ...named,
        ...(rest ? [{ key: UNCATEGORIZED, count: rest }] : []),
      ];
    },

    /** 選択中カテゴリ+検索語に一致するタイル (バリアント数バッジ付き)。 */
    tiles(state): MacroTile[] {
      const q = state.query.trim().toLowerCase();
      return state.macros
        .filter((m) => state.category === ALL_CATEGORIES || m.category.trim() === state.category)
        .filter(
          (m) =>
            !q ||
            m.name.toLowerCase().includes(q) ||
            m.name_ja.toLowerCase().includes(q) ||
            m.id.toLowerCase().includes(q),
        )
        .map((macro) => ({ macro, variantCount: macroVariantKeys(macro).length }));
    },

    /** 選択中のマクロ (未選択・一覧が空ならnull)。 */
    selected(state): Macro | null {
      return state.macros.find((m) => m.id === state.selectedId) ?? null;
    },

    /** 選択中マクロのバリアントキー (右パネルのバリアント行)。 */
    variantKeys(): string[] {
      return this.selected ? macroVariantKeys(this.selected) : [];
    },

    /**
     * 選択中マクロの値セット (挿入ダイアログのドロップダウン)。
     * 値セットを持たないマクロでは空 = 選ぶものが無いのでドロップダウンを出さない。
     */
    valueSets(): MacroValueSet[] {
      return this.selected?.value_sets ?? [];
    },

    /** 保存ダイアログの対象エンティティ数 (プレビュー欄の「n エンティティ」)。 */
    saveEntityCount(state): number {
      return state.saveEntityIds.length;
    },

    /**
     * プレースホルダ表の候補行: 選択範囲のシンボルごとに、型番・値の欄と
     * 今ある属性を1行ずつ並べる (型番欄は空でも並べる。空の定格にキーを付けられるように)。
     */
    placeholderCandidates(state): PlaceholderCandidate[] {
      const rows: PlaceholderCandidate[] = [];
      for (const c of state.savePreview?.commands ?? []) {
        if (c.type !== "add_entity" || c.entity.kind !== "symbol") continue;
        const e = c.entity;
        const base = { entity: e.id, reference: e.reference, symbolId: e.symbol_id };
        rows.push({ id: `${e.id}|value`, field: "value", current: e.value, ...base });
        for (const name of Object.keys(e.attrs).sort()) {
          rows.push({
            id: `${e.id}|attrs.${name}`,
            field: `attrs.${name}`,
            current: e.attrs[name],
            ...base,
          });
        }
      }
      return rows;
    },

    /**
     * 入力したキー名からプレースホルダを組み立てる。
     * 同じキー名を付けた欄は**1つのプレースホルダの複数の行き先**にまとまるので、
     * 「モータ容量」を1回選ぶだけで関係する欄がまとめて決まる。
     */
    savePlaceholders(): MacroPlaceholder[] {
      const byKey = new Map<string, MacroPlaceholder>();
      for (const row of this.placeholderCandidates) {
        const key = (this.placeholderKeys[row.id] ?? "").trim();
        if (!key) continue;
        const slot = byKey.get(key) ?? { key, label: key, label_ja: key, targets: [] };
        slot.targets.push({ entity: row.entity, field: row.field });
        byKey.set(key, slot);
      }
      return [...byKey.values()];
    },

    /** 宣言済みのキー名 (値セット表の列)。 */
    placeholderKeyList(): string[] {
      return this.savePlaceholders.map((p) => p.key);
    },

    /** 入力中の行から値セットを組み立てる (名前が空の行は値セットにしない)。 */
    saveValueSets(state): MacroValueSet[] {
      const keys = this.placeholderKeyList;
      return state.valueSetDrafts
        .filter((d) => d.label.trim().length > 0)
        .map((d) => {
          const label = d.label.trim();
          const values: Record<string, string> = {};
          for (const key of keys) {
            const v = d.values[key];
            if (v !== undefined && v !== "") values[key] = v;
          }
          return { id: deriveValueSetId(label), label, label_ja: label, values };
        });
    },

    /** 保存できるか (名前が空のあいだは保存させない。名前がマクロのidになるため)。 */
    canSave(state): boolean {
      return state.saveEntityIds.length > 0 && state.saveName.trim().length > 0 && !state.saving;
    },
  },

  actions: {
    /** マクロ一覧を読み込む (部品挿入ダイアログを開いたとき・保存したとき)。 */
    async load() {
      this.loading = true;
      this.error = null;
      try {
        const list = await ipc.listMacros();
        this.macros = list.macros;
        this.issues = list.issues;
        this.user_dir = list.user_dir ?? null;
        if (!this.macros.some((m) => m.id === this.selectedId)) {
          this.selectedId = this.macros[0]?.id ?? null;
          this.variantKey = DEFAULT_VARIANT;
          this.valueSetId = null;
        }
      } catch (e) {
        this.macros = [];
        this.issues = [];
        this.user_dir = null;
        this.selectedId = null;
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },

    setCategory(key: string) {
      this.category = key;
    },

    /**
     * タイルを選ぶ (右のプレビューが切り替わり、バリアントは既定へ戻る)。
     * 値セットの選択も外す (値セットはマクロごとのものなので持ち越さない)。
     */
    select(id: string) {
      this.selectedId = id;
      this.variantKey = DEFAULT_VARIANT;
      this.valueSetId = null;
    },

    setVariant(key: string) {
      this.variantKey = key;
    },

    /** 挿入に使う値セットを選ぶ (nullで「保存時の値のまま」)。 */
    setValueSet(id: string | null) {
      this.valueSetId = id;
    },

    /** プレースホルダ表の1行にキー名を付ける (空にするとその行はプレースホルダでなくなる)。 */
    setPlaceholderKey(rowId: string, key: string) {
      this.placeholderKeys[rowId] = key;
    },

    /** 値セットの行を1つ足す。 */
    addValueSet() {
      this.valueSetDrafts.push({ label: "", values: {} });
    },

    removeValueSet(index: number) {
      this.valueSetDrafts.splice(index, 1);
    },

    setValueSetLabel(index: number, label: string) {
      const draft = this.valueSetDrafts[index];
      if (draft) draft.label = label;
    },

    setValueSetValue(index: number, key: string, value: string) {
      const draft = this.valueSetDrafts[index];
      if (draft) draft.values[key] = value;
    },

    /**
     * 選択範囲をマクロにする保存ダイアログを開く。
     * 選択が空なら開かない (マクロにする回路が無い)。
     */
    async openSave(sheetId: string, entityIds: string[]): Promise<boolean> {
      if (entityIds.length === 0) return false;
      this.saveOpen = true;
      this.saveSheetId = sheetId;
      this.saveEntityIds = [...entityIds];
      this.saveName = "";
      this.saveCategory = "";
      this.savePreview = null;
      this.placeholderKeys = {};
      this.valueSetDrafts = [];
      this.error = null;
      try {
        this.savePreview = await ipc.buildMacro(sheetId, this.saveEntityIds, { name: "" });
      } catch (e) {
        this.error = String(e);
      }
      return true;
    },

    cancelSave() {
      this.saveOpen = false;
      this.error = null;
    },

    /**
     * 入力した名前・カテゴリで選択範囲をユーザー領域へ保存し、一覧を読み直して閉じる。
     * 失敗したらダイアログは開いたまま理由を残す (入力をやり直せる)。
     */
    async save(): Promise<{ macro: Macro; path: string } | null> {
      if (!this.saveSheetId || !this.canSave) return null;
      const name = this.saveName.trim();
      const meta: MacroMeta = {
        name,
        name_ja: name,
        category: this.saveCategory.trim(),
      };
      // 何も指定していないマクロは今までどおりの中身のまま保存する (後方互換)。
      const placeholders = this.savePlaceholders;
      const valueSets = this.saveValueSets;
      if (placeholders.length) meta.placeholders = placeholders;
      if (valueSets.length) meta.value_sets = valueSets;
      this.saving = true;
      this.error = null;
      try {
        const result = await ipc.saveMacro(this.saveSheetId, this.saveEntityIds, meta);
        this.saving = false;
        await this.load();
        this.selectedId = result.macro.id;
        this.variantKey = DEFAULT_VARIANT;
        this.valueSetId = null;
        this.saveOpen = false;
        return result;
      } catch (e) {
        this.error = String(e);
        this.saving = false;
        return null;
      }
    },

    /**
     * ⌘C: 選択範囲を無名マクロとしてメモリに覚える (ファイルには書かない)。
     * 選択が空なら何もしない (前のコピー内容も消さない)。
     */
    async copy(sheetId: string, entityIds: string[]): Promise<Macro | null> {
      if (entityIds.length === 0) return null;
      try {
        this.clipboard = await ipc.buildMacro(sheetId, entityIds, { name: "" });
        this.error = null;
        return this.clipboard;
      } catch (e) {
        this.error = String(e);
        return null;
      }
    },

    /** ⌘V: 覚えている無名マクロを返す (何度でも貼り付けられるので消さない)。 */
    paste(): Macro | null {
      return this.clipboard;
    },

    /**
     * ユーザーマクロの置き場をOSのファイラで開く (無ければ作られる)。
     * 開けない環境 (ブラウザ検証モード) では`opened=false`で置き場のパスだけ返す。
     */
    async openUserFolder(): Promise<{ path: string; opened: boolean }> {
      try {
        return { path: await ipc.openMacrosFolder(), opened: true };
      } catch {
        return { path: this.user_dir ?? "~/MadakeCAD/macros", opened: false };
      }
    },
  },
});
