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
  /** 配線注記 (線径・線色・電線長)。線番より控えめな灰緑で、線番と読み分けられる。 */
  wireNote: "#9fc8a8",
  /** ハーネス境界 (IEC 61082-1の破線囲み)。配線より控えめな青灰色。 */
  harness: "#9aa8c8",
  /** シート間クロスリファレンス「/2.B3」。ネットラベル(注記色)に添える淡い青。 */
  xref: "#7fb2e5",
  /** 図枠プレビューで空欄へ重ねる`{バインド}`の表示色 (デザインの青文字)。 */
  bindPlaceholder: "#4a9eff",
  /** エージェント編集オーバーレイの既定色(シアン。会話1本目の色でもある)。 */
  agent: "#29d3e6",
  /** オーバーレイのラベルチップ上の文字色。 */
  agentInk: "#0b3540",
  /** 作図領域の操作チップ (.pen DSキャンバス操作チップ): 地・枠・主文・操作ヒント。 */
  chipBg: "rgba(46, 51, 59, 0.9)",
  chipBorder: "#1f6fbf",
  chipText: "#e8eaec",
  chipHint: "#9aa2ab",
} as const;

/**
 * 注記の色トークン (G4) → **画面色**。暗い作図領域で読める明るい色を割り当てる。
 *
 * 紙・PDFの色は Rust側 `madake-core/src/text_style.rs` の `TEXT_COLORS` が持つ
 * (default=黒 / accent=青 / warning=赤 / success=緑 / muted=灰)。
 * 図面データが持つのはトークン名だけなので、配色を変えても図面は書き換えなくてよい。
 */
export const textColors = {
  /** 通常の注記 (図枠・シンボルと同じ基本線色)。 */
  default: "#e8eaec",
  /** 強調・参照 (青系)。 */
  accent: "#7fb2e5",
  /** 注意喚起 (赤系)。 */
  warning: "#ff8a7a",
  /** 確認済み・良好 (緑系)。 */
  success: "#7fd9a2",
  /** 補足・控えめ (灰)。 */
  muted: "#9aa2ab",
} as const;

/**
 * シンボルエディタのキャンバス配色 (.pen「M4デザイン - シンボルエディタ」)。
 *
 * 作図領域 (暗い`theme`) とは逆に**白紙**の上で記号を描くので、別テーブルにしてある。
 * 値はUIトークン (`src/App.vue`) と同じ色を使い、コンポーネントには直書きしない。
 */
export const symbolEditorTheme = {
  /** 白紙。 */
  paper: "#ffffff",
  /** 2.5mmグリッド線。 */
  grid: "#edeff2",
  /** 5目盛りごとの太いグリッド線。 */
  gridMajor: "#dfe3e8",
  /** 記号の線 (紙面と同じ黒)。 */
  line: "#2b2f33",
  /** 接続点 (赤ドット)。 */
  pin: "#dc2626",
  /** 接続点の番号。 */
  pinLabel: "#6b7178",
  /** 属性テキストの見本 (配置時に実値へ置き換わる)。 */
  attr: "#1f6fbf",
  /** 原点マーク・注記。 */
  origin: "#8a9099",
  /** 選択ハイライト。 */
  selection: "#1f6fbf",
  selectionFill: "rgba(31, 111, 191, 0.15)",
} as const;

/**
 * 盤レイアウトシートの配色 (.pen「M4デザイン - 盤レイアウト」)。
 *
 * 暗い作図領域 (`theme`) の上に**白い盤面**が乗るので、盤の中に描くものは
 * シンボルエディタ (`symbolEditorTheme`) と同じ明るい地の配色にする。
 */
export const panelTheme = {
  /** 盤の外形 (筐体) の面。 */
  enclosure: "#ffffff",
  /** 盤の外形の輪郭。 */
  enclosureEdge: "#9aa2ab",
  /** 取付板の面 (外形よりわずかに沈ませる)。 */
  plate: "#fafbfc",
  /** 取付板の輪郭。 */
  plateEdge: "#c8ccd2",
  /** DINレールの面。 */
  rail: "#e8e8e8",
  /** DINレールの輪郭・取付線。 */
  railEdge: "#c8ccd2",
  /** ダクトの面と斜線ハッチ。 */
  duct: "#ffffff",
  ductHatch: "#c8ccd2",
  ductEdge: "#9aa2ab",
  /** フットプリントの面・輪郭。 */
  footprint: "#ffffff",
  footprintEdge: "#2b2f33",
  /** フットプリントの参照記号 (等幅) と型番。 */
  reference: "#2b2f33",
  partNo: "#6b7178",
  /** 盤の名前。 */
  label: "#6b7178",
  /** 銘板の面・輪郭・文字。 */
  nameplate: "#ffffff",
  nameplateEdge: "#2b2f33",
  nameplateText: "#2b2f33",
  /** 寸法線・補助線・寸法値。 */
  dimension: "#6b7178",
  /** バルーン (アイテム番号) の白丸・輪郭と引出線・番号。 */
  balloon: "#ffffff",
  balloonEdge: "#2b2f33",
  balloonText: "#2b2f33",
  /** 凡例表の面・輪郭・罫線と、列見出し・本文の文字。 */
  legend: "#ffffff",
  legendEdge: "#2b2f33",
  legendRule: "#c8ccd2",
  legendHead: "#6b7178",
  legendText: "#2b2f33",
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
