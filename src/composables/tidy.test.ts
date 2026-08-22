// 「自動で整える」ポップアップの定型プロンプトと送信の仕様テスト。
// (計画: docs/superpowers/plans/2026-08-22-m3-phase2-tidy-parallel-api.md Task 2)
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { TIDY_LOOP_LIMIT, TIDY_MODES, runTidy, tidyPrompt } from "./tidy";
import { agentApi, useChatStore } from "../stores/chat";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";
import type { Entity, Project, Sheet } from "../ipc";

function symbol(id: string, reference: string): Entity {
  return {
    kind: "symbol",
    id,
    symbol_id: "fuse",
    at: { x: 0, y: 0 },
    rotation: 0,
    mirror: false,
    reference,
    value: "",
    attrs: {},
  };
}

function wire(id: string): Entity {
  return {
    kind: "wire",
    id,
    points: [
      { x: 0, y: 0 },
      { x: 10, y: 0 },
    ],
    color: "black",
    sq: 0.75,
    length_m: null,
    part_no: null,
    net: null,
  };
}

function sheet(entities: Entity[], name = "動力系統図"): Sheet {
  return {
    id: "sheet-1",
    name,
    size: "A3",
    orientation: "Landscape",
    zone_cols: 8,
    zone_rows: 4,
    title_block: {} as Sheet["title_block"],
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  };
}

/** シート1枚のプロジェクトをドキュメントストアへ流し込む(patchを介さない読取用の下ごしらえ)。 */
function seedDocument(entities: Entity[], selection: string[] = []) {
  const doc = useDocumentStore();
  doc.project = { sheets: [sheet(entities)] } as unknown as Project;
  doc.activeSheetId = "sheet-1";
  doc.selection = new Set(selection);
  return doc;
}

describe("tidyPrompt", () => {
  // ja: 整えモードは配置整理・配線整理・ラベル整頓の3種類
  it("offers three tidy modes: layout, wiring and labels", () => {
    expect(TIDY_MODES).toEqual(["layout", "wiring", "labels"]);
  });

  // ja: 配置整理は2.5mmグリッドへ載せる・シンボルの重なりを解消する・列を揃えることを指示する
  it("the layout prompt asks for the 2.5mm grid, no overlapping symbols and aligned rows", () => {
    const prompt = tidyPrompt("layout", sheet([]), []);
    expect(prompt).toContain("2.5mm");
    expect(prompt).toContain("重なり");
    expect(prompt).toContain("揃え");
    expect(prompt).toContain("symbol_overlaps");
  });

  // ja: 配線整理は交差を減らし直交を保ち、余分な曲がりを減らすことを指示する
  it("the wiring prompt asks to cut crossings, keep right angles and drop needless bends", () => {
    const prompt = tidyPrompt("wiring", sheet([]), []);
    expect(prompt).toContain("交差");
    expect(prompt).toContain("直交");
    expect(prompt).toContain("曲がり");
    expect(prompt).toContain("crossings");
  });

  // ja: ラベル整頓は位置を動かすだけで、ラベルや線番を消したり書き換えたりしないよう指示する
  it("the label prompt only moves labels and forbids deleting or rewriting them", () => {
    const prompt = tidyPrompt("labels", sheet([]), []);
    expect(prompt).toContain("線番");
    expect(prompt).toContain("位置を動かすだけ");
    expect(prompt).toContain("消さない");
    expect(prompt).toContain("label_overlaps");
  });

  // ja: どの整えも接続関係(どのピンとどのピンが繋がるか)は変えない
  it("no tidy mode is allowed to change the connections of the circuit", () => {
    for (const mode of TIDY_MODES) {
      expect(tidyPrompt(mode, sheet([]), []), mode).toContain("接続");
    }
  });

  // ja: どの整えも「計測 → 編集 → 再計測」をget_tidy_metricsで行うよう指示する
  it("every tidy prompt asks to measure with get_tidy_metrics before and after editing", () => {
    for (const mode of TIDY_MODES) {
      const prompt = tidyPrompt(mode, sheet([]), []);
      expect(prompt, mode).toContain("get_tidy_metrics");
      expect(prompt, mode).toContain("編集");
      expect(prompt, mode).toContain("もう一度");
    }
  });

  // ja: どの整えも改善が止まったら終わり、繰り返しは最大3回まで
  it("every tidy prompt stops when the numbers stop improving, and after three rounds at most", () => {
    expect(TIDY_LOOP_LIMIT).toBe(3);
    for (const mode of TIDY_MODES) {
      const prompt = tidyPrompt(mode, sheet([]), []);
      expect(prompt, mode).toContain("改善が止まったら");
      expect(prompt, mode).toContain(`最大${TIDY_LOOP_LIMIT}回`);
    }
  });

  // ja: どの整えも最終応答にビフォー/アフターの数値を書かせる
  it("every tidy prompt requires before/after numbers in the final answer", () => {
    for (const mode of TIDY_MODES) {
      const prompt = tidyPrompt(mode, sheet([]), []);
      expect(prompt, mode).toContain("ビフォー");
      expect(prompt, mode).toContain("アフター");
    }
  });

  // ja: 選択が無ければ整える対象はシート全体になる
  it("with no selection the tidy covers the whole sheet", () => {
    const prompt = tidyPrompt("layout", sheet([symbol("a", "F2")]), []);
    expect(prompt).toContain("シート「動力系統図」全体");
    expect(prompt).not.toContain("これ以外は動かさない");
  });

  // ja: 選択があれば選択したエンティティのidを並べ、それ以外は動かさないよう指示する
  it("with a selection the tidy lists the selected entity ids and forbids touching anything else", () => {
    const prompt = tidyPrompt("layout", sheet([symbol("a", "K1"), wire("b")]), ["a", "b"]);
    expect(prompt).toContain("K1 (id: a)");
    expect(prompt).toContain("配線 (id: b)");
    expect(prompt).toContain("これ以外は動かさない");
    expect(prompt).not.toContain("シート「動力系統図」全体");
  });

  // ja: 別シートの選択が残っていても、このシートに無いidは対象にしない(シート全体扱いに戻る)
  it("ids that are not on the active sheet fall back to tidying the whole sheet", () => {
    const prompt = tidyPrompt("layout", sheet([symbol("a", "K1")]), ["ghost"]);
    expect(prompt).toContain("シート「動力系統図」全体");
    expect(prompt).not.toContain("ghost");
  });

  // ja: シートが無くてもプロンプトは壊れない
  it("stays well-formed when there is no sheet at all", () => {
    const prompt = tidyPrompt("wiring", null, ["a"]);
    expect(prompt).toContain("get_tidy_metrics");
    expect(prompt).toContain("全体");
  });
});

describe("runTidy", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
  });

  // ja: 整えの実行は普通のチャット送信1回(=undo一発で戻せる1ターン)になる
  it("runs one tidy as exactly one chat turn", async () => {
    seedDocument([symbol("a", "K1")]);
    const send = vi.spyOn(agentApi, "send").mockResolvedValue("11111111-1111-4111-8111-111111111111");
    await runTidy("layout");
    expect(send).toHaveBeenCalledTimes(1);
    const prompt = send.mock.calls[0][1];
    expect(prompt).toContain("get_tidy_metrics");
    expect(prompt).toContain("シート");
  });

  // ja: 整えを実行するとエージェントタブが開いて経過が見える
  it("opens the agent tab so the user can watch the tidy run", async () => {
    seedDocument([symbol("a", "K1")]);
    vi.spyOn(agentApi, "send").mockResolvedValue("11111111-1111-4111-8111-111111111111");
    const ui = useUiStore();
    await runTidy("wiring");
    expect(ui.leftPanelTab).toBe("chat");
    expect(useChatStore().panelOpen).toBe("expanded");
  });

  // ja: 選択中のエンティティがあれば、送るプロンプトにそのidが入る
  it("sends the selected entity ids when a selection is active", async () => {
    seedDocument([symbol("a", "K1"), wire("b")], ["b"]);
    const send = vi.spyOn(agentApi, "send").mockResolvedValue("11111111-1111-4111-8111-111111111111");
    await runTidy("labels");
    expect(send.mock.calls[0][1]).toContain("(id: b)");
  });

  // ja: 開いている会話が答えている途中は整えを二重に投げない
  it("does not start a tidy while the open conversation is still answering", async () => {
    seedDocument([symbol("a", "K1")]);
    const chat = useChatStore();
    const conversationId = "11111111-1111-4111-8111-111111111111";
    chat.conversations.push({
      id: conversationId,
      session_id: null,
      messages: [],
      model: null,
      updated_at: 0,
    });
    chat.setActive(conversationId);
    chat.setRunning(conversationId, true);
    const send = vi.spyOn(agentApi, "send").mockResolvedValue("11111111-1111-4111-8111-111111111111");
    expect(await runTidy("layout")).toBeNull();
    expect(send).not.toHaveBeenCalled();
  });
});
