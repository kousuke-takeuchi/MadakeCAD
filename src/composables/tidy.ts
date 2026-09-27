// 「自動で整える」ポップアップの中身: 定型プロンプトの組み立てと、その送信。
//
// 整えは専用アルゴリズムではなく**エージェントの反復**で行う(仕様:
// docs/internal/specs/m3-ai-first.md §3)。ここが作るのは「現状を測る →
// 直す → もう一度測る」というループ指示つきの普通のプロンプトで、送信も
// 普通のチャット送信1回。したがって1回の整え=1ターン=undo一発で戻せる。
//
// 図面には一切触らない(編集は全てエージェントがMCP→Commandエンジン経由で行う)。

import { i18n } from "../i18n";
import type { Entity, Sheet } from "../ipc";
import { entityKindLabel, useChatStore } from "../stores/chat";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";

/** 整えモード。ポップアップの3項目と1対1で対応する。 */
export type TidyMode = "layout" | "wiring" | "labels";

/** ポップアップに並ぶ順(デザイン「P反復モード行」と同じ)。 */
export const TIDY_MODES = ["layout", "wiring", "labels"] as const;

/** 反復の上限回数。改善が止まらなくてもここで打ち切らせる。 */
export const TIDY_LOOP_LIMIT = 3;

/** モードごとの「やること」。接続を変えないことは全モード共通で書く。 */
const MODE_RULES: Record<TidyMode, { title: string; body: string }> = {
  layout: {
    title: "配置整理",
    body: "\
- シンボルの原点を**2.5mmグリッドの上**へ載せる(off_grid を減らす)\n\
- **シンボルどうしの重なり**を解消する(symbol_overlaps を減らす)\n\
- 同じ役割のシンボルは行・列を**揃え**、読み順(左上→右下)に並べる\n\
- シンボルを動かしたら、繋がっていた配線の端点も一緒に付け替えて**接続を保つ**\
(部品の追加・削除、繋ぎ替えはしない)",
  },
  wiring: {
    title: "配線整理",
    body: "\
- 配線どうしの**交差**を減らす(crossings を減らす)\n\
- 線は**直交**(水平・垂直)を保ち、頂点は2.5mmグリッドの上へ載せる\n\
- 遠回り・不要な**曲がり**(同じ場所へ戻る折れ)を減らす\n\
- **接続**関係(どのピンとどのピンが繋がるか)は変えない。配線の追加・削除で\
ネットを変えてはならない",
  },
  labels: {
    title: "ラベル整頓",
    body: "\
- ラベル・**線番**・注記の重なりを解消する(label_overlaps を減らす)\n\
- 直すのは**位置を動かすだけ**。文字は**消さない**・書き換えない・付け足さない\n\
- ラベルは対応するシンボル・配線のすぐそばに置き、どれに付いているか分かるようにする\n\
- **接続**関係もネットラベルの綴りも変えない(見た目の重なりだけを直す)",
  },
};

/** 選択エンティティの表示名。参照記号があればそれ、無ければ種別名。 */
function entityLabel(entity: Entity): string {
  const reference = "reference" in entity ? entity.reference : "";
  return reference || entityKindLabel(entity.kind, "ja");
}

/**
 * 整える範囲の指定文。
 *
 * 選択があればそのidを列挙して「これ以外は動かさない」と釘を刺し、選択が無ければ
 * シート全体。`selection`にはアクティブシートに無いid(別シートの選択の残り)が
 * 混ざりうるので、シート内に見つかったものだけを対象にする。
 */
function scopeSection(sheet: Sheet | null, selection: Iterable<string>): string {
  const entities: Entity[] = [];
  if (sheet) {
    for (const id of selection) {
      const entity = sheet.entities[id];
      if (entity) entities.push(entity);
    }
  }
  const sheetName = sheet?.name ?? "(シートなし)";
  if (!entities.length) {
    return `- シート「${sheetName}」全体(sheet_id: ${sheet?.id ?? "(不明)"})`;
  }
  const lines = entities.map((e) => `  - ${entityLabel(e)} (id: ${e.id})`).join("\n");
  return `\
- シート「${sheetName}」(sheet_id: ${sheet?.id ?? "(不明)"})の**次のエンティティだけ**を編集する。\
**これ以外は動かさない**:\n${lines}`;
}

/**
 * 整えの定型プロンプトを組み立てる(純関数)。
 *
 * 「get_tidy_metricsで測る → 直す → もう一度測る」の反復と、その停止条件
 * (改善が止まる or 最大3回)、ビフォー/アフターの数値報告までを1つの指示にまとめる。
 */
export function tidyPrompt(
  mode: TidyMode,
  sheet: Sheet | null,
  selection: Iterable<string>,
): string {
  const rule = MODE_RULES[mode];
  return `\
図面を「${rule.title}」で整えてください。

## 整える範囲
${scopeSection(sheet, selection)}

## やること(${rule.title})
${rule.body}

## 進め方(必須)
1. まず\`get_tidy_metrics\`を実行して現状を測る(これがビフォーの数値)
2. 上の「やること」だけを編集する(回路の意味は変えない)
3. もう一度\`get_tidy_metrics\`を実行して、数値が減ったか確かめる
4. 減っていれば 2〜3 をもう一度繰り返す。**改善が止まったら**(数値が減らなくなったら)\
そこで終える。繰り返しは**最大${TIDY_LOOP_LIMIT}回**まで
5. 最後に\`run_verification\`でエラーが増えていないことを確かめる
6. 最終応答に**ビフォー/アフターの数値**を必ず書く\
(例: 整え: 交差 4→1・重なり 3→0・グリッド外 6→0)
`;
}

/**
 * 整えを実行する。普通のチャット送信1回として投げる(=undo一発で戻せる1ターン)。
 *
 * 応答の途中(ストリーミング中)は投げない。戻り値は会話id(送れなければnull)。
 */
export async function runTidy(mode: TidyMode): Promise<string | null> {
  const chat = useChatStore();
  const doc = useDocumentStore();
  const ui = useUiStore();
  if (chat.streaming) return null;

  const prompt = tidyPrompt(mode, doc.activeSheet, doc.selection);
  // 浮き入力カードから実行しても経過が見えるように、必ずエージェントタブを開く
  ui.openAgentTab();
  const id = await chat.send(prompt);
  if (!id) ui.log(i18n.global.t("chat.tidy.sendFailed"));
  return id;
}
