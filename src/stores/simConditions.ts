// シミュレーションの条件 (属性の置き換えと短絡) の入力を読む (デザイン: MadakeCAD.pen「シミュレーション結果パネル」の条件バー)。

/** 入力1行を読んだ結果。 */
export type SimCondition =
  | { kind: "set"; reference: string; attr: string; value: string }
  | { kind: "short"; net: string };

/** 読めなかったとき。 */
export interface SimConditionError {
  kind: "error";
}

/**
 * 条件の入力を読む。`参照記号.属性=値` は属性の置き換え、`短絡 ネット名` (または `short ネット名`) は短絡。
 * 前後と区切りの空白は無視する。どちらの形でもなければ error。
 */
export function parseSimCondition(text: string): SimCondition | SimConditionError {
  const s = text.trim();
  const short = /^(?:短絡|short)\s+(\S+)$/i.exec(s);
  if (short) return { kind: "short", net: short[1] };
  const set = /^([^.=\s]+)\s*\.\s*([^=\s]+)\s*=\s*(\S.*)$/.exec(s);
  if (set) return { kind: "set", reference: set[1], attr: set[2], value: set[3].trim() };
  return { kind: "error" };
}
