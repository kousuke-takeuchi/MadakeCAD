// 注記テキストの装飾 (G4) と、シンボルの機能ラベル。
// Rust側 madake-core/src/text_style.rs と同じ規則・同じトークン名で対応する:
//   - 注記の色は**トークン名**だけ (生の色コードはコマンドが拒否する)。画面 (暗い作図領域)
//     はここの色、紙・PDFは text_style.rs の印刷色で描く
//   - 機能ラベルはシンボル定義の FUNCTION スロットに従い、スロットが無ければ外形の下へ自動

import type {
  Point,
  SymbolDef,
  SymbolInstance,
  TextAnchor,
  TextColorToken,
  TextEntity,
  TextSlot,
} from "../ipc";
import { textWidthMm } from "./textWidth";
import { textColors, theme } from "./theme";

/** 使える注記色のトークン名 (プロパティの選択肢の並び順)。 */
export const TEXT_COLOR_TOKENS: TextColorToken[] = [
  "default",
  "accent",
  "warning",
  "success",
  "muted",
];

/** 色を指定していない注記が使うトークン。 */
export const DEFAULT_TEXT_COLOR: TextColorToken = "default";

/** 選べる基準位置 (プロパティの選択肢の並び順)。 */
export const TEXT_ANCHORS: TextAnchor[] = ["start", "middle", "end"];

/** トークン名が使える注記色か (生の色コードは false)。 */
export function isTextColorToken(value: string): value is TextColorToken {
  return (TEXT_COLOR_TOKENS as string[]).includes(value);
}

/** 注記を画面へ描くときの色。未指定・知らない名前は既定色。 */
export function textScreenColor(color?: string | null): string {
  return color && isTextColorToken(color) ? textColors[color] : textColors[DEFAULT_TEXT_COLOR];
}

/** 基準位置 → Canvas2Dの textAlign (SVGの text-anchor と同じ意味)。 */
export function canvasTextAlign(anchor?: TextAnchor | null): CanvasTextAlign {
  return anchor === "middle" ? "center" : anchor === "end" ? "right" : "left";
}

/** 選択中の注記を描く色 (選択ハイライトが色トークンより優先する)。 */
export function noteColor(color: string | null | undefined, selected: boolean): string {
  return selected ? theme.selection : textScreenColor(color);
}

/** 注記のベースラインより下へ出る分 (文字高さに対する比。g・yの下ひげ)。 */
const DESCENDER_RATIO = 0.3;

/**
 * 注記が紙面で占める矩形 (用紙mm)。クリックで注記を掴むための当たり判定に使う。
 * 基準位置に合わせて左右どちらへ伸びるかが変わる。
 */
export function noteBounds(note: TextEntity): { min: Point; max: Point } {
  const w = textWidthMm(note.text, note.height);
  const anchor = note.anchor ?? "start";
  const left = anchor === "middle" ? note.at.x - w / 2 : anchor === "end" ? note.at.x - w : note.at.x;
  const a = { x: left, y: note.at.y - note.height };
  const b = { x: left + w, y: note.at.y + note.height * DESCENDER_RATIO };
  // 回した注記 (90・180・270、時計回り) は、箱も基準点を中心に回す (描画と同じ)
  const deg = noteTurnDeg(note.rotation);
  const turn = (q: Point): Point => {
    const dx = q.x - note.at.x;
    const dy = q.y - note.at.y;
    if (deg === 90) return { x: note.at.x - dy, y: note.at.y + dx };
    if (deg === 180) return { x: note.at.x - dx, y: note.at.y - dy };
    if (deg === 270) return { x: note.at.x + dy, y: note.at.y - dx };
    return q;
  };
  const [p1, p2] = [turn(a), turn(b)];
  return {
    min: { x: Math.min(p1.x, p2.x), y: Math.min(p1.y, p2.y) },
    max: { x: Math.max(p1.x, p2.x), y: Math.max(p1.y, p2.y) },
  };
}

/** 参照記号・型番・ネットラベルの文字高さ (mm)。svg.rsのLABEL_FONTと一致させること。 */
export const LABEL_FONT = 2.5;

/** 参照記号のベースラインを、シンボル外形の上端からどれだけ上に置くか (mm)。
 * svg.rsのREF_LABEL_DYと一致させること (キャンバス描画・XRefテキストの行揃えで共通)。 */
export const REF_LABEL_DY = 4.5;

/** 型番・値のベースラインを、シンボル外形の上端からどれだけ上に置くか (mm)。
 * svg.rsのVALUE_LABEL_DYと一致させること。 */
export const VALUE_LABEL_DY = 1.0;

/** 機能ラベル (第3のテキスト欄) をシンボルインスタンスへ入れる属性キー。 */
export const FUNCTION_LABEL_ATTR = "function_label";

/** シンボル定義で機能ラベルの位置を決めるスロットのキー。 */
export const SLOT_FUNCTION = "FUNCTION";

/** 機能ラベルのベースラインを、シンボル外形の下端からどれだけ下に置くか (mm)。 */
export const FUNCTION_LABEL_DY = 3.5;

/** 機能ラベルの既定の文字高さ (mm)。参照記号・型番と同じ。 */
export const FUNCTION_LABEL_FONT = 2.5;

/** シンボルに付けられた機能ラベルの文字列。属性が無い・空白だけなら null。 */
export function functionLabelText(inst: SymbolInstance): string | null {
  const raw = inst.attrs?.[FUNCTION_LABEL_ATTR]?.trim();
  return raw ? raw : null;
}

/** シンボル定義が機能ラベルの位置を決めているならそのスロット。 */
export function functionLabelSlot(def: SymbolDef): TextSlot | undefined {
  return def.text_slots?.find((s) => s.key === SLOT_FUNCTION);
}

/**
 * スロットを持たないシンボル (同梱シンボルは全部そう) の機能ラベルの位置。
 * 外形のすぐ下・配置基準点を中心に置く (参照記号と型番は外形の上なので重ならない)。
 */
export function functionLabelFallbackAt(
  inst: SymbolInstance,
  bounds: { min: Point; max: Point },
): Point {
  return functionLabelBelow(inst.at.x, bounds.max.y);
}

/**
 * 外形の下端 (bottomY) と中心X (centerX) を指定した機能ラベルの位置。
 * シンボル定義の外形とは別の外形の下へ置くとき (実機ブロック表示 (G7) のコイル) にも
 * 同じ行送りで置けるようにしたもの。Rust側 text_style::function_label_below と同一。
 */
export function functionLabelBelow(centerX: number, bottomY: number): Point {
  return { x: centerX, y: bottomY + FUNCTION_LABEL_DY };
}

/** 注記の回転 (度)。90・180・270 だけを描く。それ以外 (0 や 90 度刻みでない値) は 0 = 回さない。svg.rs と同じ。 */
export function noteTurnDeg(rotation: number | null | undefined): 0 | 90 | 180 | 270 {
  const r = ((Number(rotation) || 0) % 360 + 360) % 360;
  return r === 90 || r === 180 || r === 270 ? r : 0;
}

/** 大文字の高さ ÷ 文字サイズ。svg.rs の CAPTION_CAP_RATIO と同じ。 */
export const CAPTION_CAP_RATIO = 0.72;
