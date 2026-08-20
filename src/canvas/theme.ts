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
} as const;

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
