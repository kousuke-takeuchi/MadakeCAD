// 文字幅の見積もり。Rust側 madake-core/src/tidy.rs の text_width / is_wide と同じ規則。
//
// ここに置く (dynamicSymbol.tsではない) のは、pinGroups.tsのように座標変換を持たない
// 葉モジュールが、dynamicSymbol.tsを経由せず (循環importを起こさず) 読めるようにするため。
// dynamicSymbol.tsは互換のため再エクスポートする。

/** 全角幅で描かれる文字か (Rust側 tidy::is_wide と同じ範囲)。 */
function isWideChar(c: string): boolean {
  const u = c.codePointAt(0) ?? 0;
  return (
    (u >= 0x1100 && u <= 0x115f) ||
    (u >= 0x2e80 && u <= 0x303e) ||
    (u >= 0x3041 && u <= 0x33ff) ||
    (u >= 0x3400 && u <= 0x4dbf) ||
    (u >= 0x4e00 && u <= 0x9fff) ||
    (u >= 0xa000 && u <= 0xa4cf) ||
    (u >= 0xac00 && u <= 0xd7a3) ||
    (u >= 0xf900 && u <= 0xfaff) ||
    (u >= 0xfe30 && u <= 0xfe6f) ||
    (u >= 0xff00 && u <= 0xff60) ||
    (u >= 0xffe0 && u <= 0xffe6) ||
    (u >= 0x20000 && u <= 0x3fffd)
  );
}

/** 文字列を高さheightで描いたときの見積もり幅 mm (Rust側 tidy::text_width と同じ)。 */
export function textWidthMm(text: string, height: number): number {
  let advance = 0;
  for (const c of text) advance += isWideChar(c) ? 1.0 : 0.6;
  return height * advance;
}
