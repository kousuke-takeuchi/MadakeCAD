// 作図領域の配色トークン。Pencilデザイン(AutoCAD Electrical風)に準拠。
// デザイン変更時はここだけを差し替える。

export const theme = {
  /** モデル空間の背景。 */
  background: "#212830",
  /** 図枠・シンボルなどの基本線色。 */
  line: "#e8eaec",
  /** ゾーン番号などの補助表示。 */
  dim: "#9aa2ab",
  /** グリッドドット。 */
  grid: "#3a424c",
  /** 選択ハイライト。 */
  selection: "#4a9eff",
  selectionFill: "rgba(74, 158, 255, 0.15)",
  /** クロスヘア。 */
  crosshair: "#5a626c",
  /** 参照記号などの注記。 */
  annotation: "#e8d44d",
  /** 線番 (IEC 62491)。注記と区別できる淡い水色で、暗い背景でも読める。 */
  wireNumber: "#9fd8ff",
  /** ハーネス境界 (IEC 61082-1の破線囲み)。配線より控えめな青灰色。 */
  harness: "#9aa8c8",
  /** シート間クロスリファレンス「/2.B3」。ネットラベル(注記色)に添える淡い青。 */
  xref: "#7fb2e5",
  /** エージェント編集オーバーレイの既定色(シアン。会話1本目の色でもある)。 */
  agent: "#29d3e6",
  /** オーバーレイのラベルチップ上の文字色。 */
  agentInk: "#0b3540",
} as const;

/**
 * 並列エージェント: 会話ごとの編集オーバーレイ色 (仕様: m3-ai-first.md §4)。
 *
 * 複数の会話が同時に図面を編集するので、どの会話の編集かを色で見分ける。
 * 暗い作図領域 (`theme.background`) で互いに判別でき、参照記号 (黄) や
 * 選択 (青) とも混ざらない4色。会話の**開始順**に割り当てる。
 */
export const agentPalette = ["#29d3e6", "#ffb454", "#b48cff", "#ff6fd8"] as const;

/** 開始順(0始まり)から会話の編集オーバーレイ色を選ぶ。5本目からは先頭へ戻る。 */
export function agentColorAt(order: number): string {
  const size = agentPalette.length;
  const index = Number.isFinite(order) ? Math.trunc(order) : 0;
  return agentPalette[((index % size) + size) % size];
}

function hexToRgba(hex: string, alpha: number): string {
  const v = Number.parseInt(hex.slice(1), 16);
  return `rgba(${(v >> 16) & 255}, ${(v >> 8) & 255}, ${v & 255}, ${alpha})`;
}

/** エージェント色に不透明度を付ける(パルス塗り・枠線用)。色は会話ごとに変わる。 */
export function agentRgba(alpha: number, color: string = theme.agent): string {
  return hexToRgba(color, alpha);
}

/** 線色名→画面表示色(黒背景用)。SVG出力(白地)とは別テーブル。 */
export function wireColorScreen(color: string): string {
  switch (color) {
    case "red":
    case "赤":
      return "#ff5252";
    case "black":
    case "黒":
      return "#e8eaec"; // 黒線は黒背景で見えないため白系で描く
    case "white":
    case "白":
      return "#f5f5f5";
    case "blue":
    case "青":
      return "#5c8dff";
    case "yellow":
    case "黄":
      return "#e8d44d";
    case "green":
    case "緑":
      return "#43d675";
    case "orange":
    case "橙":
      return "#ff9c40";
    case "purple":
    case "紫":
      return "#c07ae8";
    case "brown":
    case "茶":
      return "#c09060";
    case "gray":
    case "grey":
    case "灰":
      return "#a0a8b0";
    case "pink":
    case "桃":
      return "#ff8ab8";
    case "light_blue":
    case "sky":
    case "水":
    case "水色":
      return "#29d3e6";
    default:
      return color.startsWith("#") ? color : "#e8eaec";
  }
}
