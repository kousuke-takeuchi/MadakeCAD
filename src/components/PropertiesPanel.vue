<script setup lang="ts">
// プロパティパレット (Pencilデザイン準拠): セクション見出し + ラベル列(灰)/値列(白)のグリッド。
import { ChevronDown, Pin } from "lucide-vue-next";
import { computed, inject, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { BLOCK_SYMBOL_ID } from "../canvas/dynamicSymbol";
import { harnessWireCount } from "../canvas/harness";
import { deviceDescriptions, dimensionGeometry, nameplateLines } from "../canvas/panelMarks";
import {
  SHAPE_STROKE_CHOICES,
  shapeUpdateCommand,
  type ShapePatch,
} from "../canvas/shapes";
import { panelUpdateCommand, railOfFootprint, type PanelPatch } from "../canvas/panelTools";
import { legendRows } from "../canvas/panel";
import { legendRows as wireLegendRows } from "../canvas/wireLegend";
import { autoWirePartNo } from "../canvas/wireParts";
import { type NetSite, xrefJumpTarget, xrefSites } from "../canvas/xref";
import { entityKindLabels } from "./entityKindLabels";
import { mechLinkText as formatMechLinkText } from "./mechLink";
import type { CoreEnd, Entity, TextAnchor, TextColorToken } from "../ipc";
import { coreEndPin, isCable, isRunCable } from "../canvas/cable";
import { FUNCTION_LABEL_ATTR, TEXT_ANCHORS, TEXT_COLOR_TOKENS } from "../canvas/textStyle";
import { COIL_SYMBOL_ID, isDeviceBlock } from "../canvas/relayXref";
import { MATES_WITH_ATTR, isConnectorInstance } from "../canvas/mate";
import { useBlockEditorStore } from "../stores/blockEditor";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";
import type { EditorController } from "../tools/controller";
import {
  annotationTextCommand,
  harnessUpdateCommand,
  cableUpdateCommand,
  nameplateUpdateCommand,
  symbolAttrs,
  wireNumberCommand,
  wireUpdateCommand,
} from "./propertyCommands";
import {
  MAX_ANNOTATION_TEXT_MM,
  MIN_ANNOTATION_TEXT_MM,
  annotationTextMm,
} from "../canvas/annotationText";

const store = useDocumentStore();
const ui = useUiStore();
const blockEditor = useBlockEditorStore();
const { t } = useI18n();
const controller = inject<EditorController>("controller")!;

const selected = computed<Entity | null>(() => {
  const sheet = store.activeSheet;
  if (!sheet || store.selection.size !== 1) return null;
  const [id] = [...store.selection];
  return sheet.entities[id] ?? null;
});

/** 選んでいるのが1本線の区間を持つケーブルか (芯の表に端A・端Bの列を出す)。 */
const selectedIsRun = computed(() => isCable(selected.value) && isRunCable(selected.value));

/** 芯の行き先の表記 (ピンは "USB1.2"、分岐の配線は「分岐の配線」、無ければ「—」)。 */
function coreEndText(end: CoreEnd | null | undefined): string {
  if (!end) return "—";
  const sheet = store.activeSheet;
  return (sheet ? coreEndPin(sheet, end) : null) ?? t("cable.endWire");
}

const wireColors = [
  "red", "black", "white", "blue", "yellow", "green", "orange", "purple", "brown", "gray", "pink", "light_blue",
] as const;
const sqValues = [0.2, 0.3, 0.5, 0.75, 1.25, 2.0, 3.5, 5.5];
/** ネットラベルのタグ向き (G3)。空 = 従来どおり文字だけ。 */
const tagDirs = [
  ["", "netLabel.tagNone"],
  ["left", "netLabel.tagLeft"],
  ["right", "netLabel.tagRight"],
] as const;

const buf = reactive({
  color: "red",
  sq: 0.75,
  part_no: "",
  length_m: "",
  /** 線番 (ネット単位。確定でset_wire_numbersコマンドを送る)。 */
  wire_no: "",
  reference: "",
  value: "",
  name: "",
  /** ハーネスの備考。 */
  note: "",
  /** 図形 (G10) の線色トークン・線幅mm・塗りトークン ("" = 塗らない)・破線。 */
  shapeStroke: "default" as TextColorToken,
  shapeWidth: 0.25,
  shapeFill: "" as TextColorToken | "",
  shapeDashed: false,
  /** ネットラベルのタグ向き ("" / "left" / "right")。 */
  tag: "",
  /** 注記の基準位置 (G4)。 */
  anchor: "start" as TextAnchor,
  /** 注記の色トークン (G4)。生の色コードは入れられない。 */
  textColor: "default" as TextColorToken,
  /** 注記を太字にするか (G4)。 */
  bold: false,
  /** シンボルの機能ラベル (G4の第3テキスト欄)。 */
  functionLabel: "",
  /** リレーコイルの実機ブロック表示 (G7)。 */
  deviceBlock: false,
  /** 嵌合コネクタ対 (G14): 相手の参照記号。空 = 未宣言。 */
  mate: "",
  /** 盤の要素 (M4 §8): 外形・ダクト・フットプリントの寸法、レールの取付線と両端。 */
  w: "",
  h: "",
  rotation: 0,
  railY: "",
  railX1: "",
  railX2: "",
  /** シートの注記文字高 (mm。G20)。空欄 = 既定 (2.0mm)。数値入力欄なので数値も入る。 */
  annotationText: "" as string | number,
  /** 多芯ケーブル (M4 §5): 芯数・線径・シールド・接地・長さ (入力欄の文字列)。 */
  cableCores: "" as string | number,
  cableSq: "" as string | number,
  cableShield: false,
  cableGrounded: false,
  cableLength: "" as string | number,
  /** 盤の銘板 (M4 §8)。 */
  npLine1: "",
  npLine2: "",
  npText: "" as string | number,
  npPosition: "below" as "below" | "above",
  npFollow: false,
});

/**
 * 選択中のネットラベルの相手先 (同名ラベルが置かれている他シートの所在)。
 * 相手がいなければ空 (プロパティには「なし」と出す)。
 */
const xrefTargets = computed<NetSite[]>(() => {
  const sheet = store.activeSheet;
  const e = selected.value;
  if (!sheet || !store.project || e?.kind !== "net_label") return [];
  return xrefSites(store.project, sheet.id, e.name);
});

/** 相手先クリック: 相手のシートへ切り替えて、相手のラベルを選択+ズームする。 */
function jumpToXref(site: NetSite) {
  const { sheetId, entityIds } = xrefJumpTarget(site);
  if (store.activeSheetId !== sheetId) store.activeSheetId = sheetId;
  controller.reveal(entityIds);
  ui.log(t("xref.jumpLog", { address: site.address, sheet: site.sheet_name }));
}

/** 選択中のエンティティに対応するFreeCADオブジェクト (M5-2の対応付け。無ければnull)。 */
const mechLink = computed(() => {
  const e = selected.value;
  if (!e) return null;
  return store.project?.mech_links?.find((l) => l.entity_id === e.id) ?? null;
});

/** 「3D対応付け」行の文面: オブジェクト名と、同期済みなら配置 (mm / deg)。 */
const mechLinkText = computed(() => formatMechLinkText(mechLink.value));

/** 選択中のハーネスが囲んでいる電線の本数 (幾何学的な内包で決まる)。 */
/**
 * 線色+線径から電線品番表 (G11) を引いた品番。品番欄が空のワイヤは、この品番で
 * 電線リストに載る。編集中の色・太さで引くので、選び直すとその場で変わる。
 */
const autoPartNo = computed(() =>
  selected.value?.kind === "wire" ? autoWirePartNo(store.project, buf.color, buf.sq) : "",
);

/** 選択中の寸法線の値 (部品の位置から決まる)。 */
const dimensionText = computed(() => {
  const sheet = store.activeSheet;
  const e = selected.value;
  return sheet && e?.kind === "dimension" ? (dimensionGeometry(sheet, e)?.text ?? "—") : "";
});

const harnessWires = computed(() => {
  const sheet = store.activeSheet;
  const e = selected.value;
  return sheet && e?.kind === "harness" ? harnessWireCount(sheet, e.id) : 0;
});

watch(
  selected,
  (e) => {
    if (!e) return;
    if (e.kind === "wire") {
      buf.color = e.color;
      buf.sq = e.sq;
      buf.part_no = e.part_no ?? "";
      buf.length_m = e.length_m?.toString() ?? "";
      buf.wire_no = e.net ?? "";
    } else if (e.kind === "symbol") {
      buf.reference = e.reference;
      buf.value = e.value;
      buf.functionLabel = e.attrs?.[FUNCTION_LABEL_ATTR] ?? "";
      buf.deviceBlock = isDeviceBlock(e);
      buf.mate = e.attrs?.[MATES_WITH_ATTR] ?? "";
    } else if (e.kind === "text") {
      buf.anchor = e.anchor ?? "start";
      buf.textColor = e.color ?? "default";
      buf.bold = e.bold ?? false;
    } else if (e.kind === "net_label") {
      buf.name = e.name;
      buf.tag = e.attrs?.tag === "left" || e.attrs?.tag === "right" ? e.attrs.tag : "";
    } else if (e.kind === "harness") {
      buf.name = e.name ?? "";
      buf.note = e.note ?? "";
    } else if (e.kind === "nameplate") {
      // 説明に追従する銘板は、図面に描いている文字 (回路図の説明) をそのまま見せる
      const shown = store.activeSheet ? nameplateLines(store.activeSheet, e, deviceDescriptions(store.project)) : e.lines;
      buf.npLine1 = shown[0] ?? "";
      buf.npLine2 = shown[1] ?? "";
      buf.npText = String(e.text_mm);
      buf.npPosition = e.position;
      buf.npFollow = e.follow_description;
      buf.part_no = e.part_no;
    } else if (isCable(e)) {
      buf.reference = e.reference;
      buf.part_no = e.part_no;
      buf.cableCores = String(e.core_count);
      buf.cableSq = e.sq > 0 ? String(e.sq) : "";
      buf.cableShield = e.shield != null;
      buf.cableGrounded = e.shield?.grounded ?? false;
      buf.cableLength = e.length_m != null ? String(e.length_m) : "";
    } else if (e.kind === "shape") {
      buf.shapeStroke = e.stroke;
      buf.shapeWidth = e.stroke_width_mm;
      buf.shapeFill = e.fill ?? "";
      buf.shapeDashed = e.dashed;
    } else if (e.kind === "footprint") {
      buf.reference = e.reference;
      buf.part_no = e.part_no;
      buf.w = String(e.w);
      buf.h = String(e.h);
      buf.rotation = e.rotation;
    } else if (e.kind === "panel_outline") {
      buf.name = e.name ?? "";
      buf.w = String(e.w);
      buf.h = String(e.h);
    } else if (e.kind === "duct") {
      buf.w = String(e.w);
      buf.h = String(e.h);
    } else if (e.kind === "din_rail") {
      buf.railY = String(e.y);
      buf.railX1 = String(e.x1);
      buf.railX2 = String(e.x2);
    }
  },
  { immediate: true },
);

// 選択が無いときはシートのプロパティを出す。シートを切り替えたときも、undoや他の
// クライアントの編集でシートの設定が変わったときも、下書きを実際の値へ取り直す
watch(
  () => [store.activeSheetId, store.activeSheet?.annotation_text_mm] as const,
  ([, height]) => {
    buf.annotationText = height != null ? String(height) : "";
  },
  { immediate: true },
);

/** シートの注記文字高 (mm)。設定が無ければ既定値を表示に使う。 */
const sheetAnnotationText = computed(() => annotationTextMm(store.activeSheet));

/**
 * 注記文字高を確定する (G20)。範囲外・数値でない入力は送らずに下書きを戻す。
 * 配線注記・線番・ネットタグがまとめて新しい高さで描き直される。
 */
async function applyAnnotationText() {
  const sheet = store.activeSheet;
  if (!sheet) return;
  const command = annotationTextCommand(sheet, buf.annotationText);
  if (!command) {
    buf.annotationText = sheet.annotation_text_mm != null ? String(sheet.annotation_text_mm) : "";
    return;
  }
  await store.execute(command);
  ui.log(t("annotationText.changedLog", { height: annotationTextMm(store.activeSheet) }));
}

/** 入力欄の文字を数値へ (空欄・文字はundefined = その欄は変えない)。 */
function num(text: string): number | undefined {
  const value = Number(text);
  return text.trim() === "" || Number.isNaN(value) ? undefined : value;
}

/** 盤の要素の編集内容 (種別ごとに使う欄だけが効く)。 */
function panelPatch(e: Entity): PanelPatch {
  switch (e.kind) {
    case "footprint":
      return { part_no: buf.part_no, w: num(buf.w), h: num(buf.h), rotation: buf.rotation };
    case "panel_outline":
      return { name: buf.name, w: num(buf.w), h: num(buf.h) };
    case "duct":
      return { w: num(buf.w), h: num(buf.h) };
    case "din_rail":
      return { y: num(buf.railY), x1: num(buf.railX1), x2: num(buf.railX2) };
    default:
      return {};
  }
}

/** 選択中のフットプリントが乗っているレール (無ければnull)。 */
const footprintRail = computed(() => {
  const sheet = store.activeSheet;
  const e = selected.value;
  if (!sheet || e?.kind !== "footprint") return null;
  return railOfFootprint(sheet, e);
});

/** 選択中の凡例表に並ぶ行数 (中身はシートのフットプリントから導出する)。 */
const legendRowCount = computed(() => {
  const sheet = store.activeSheet;
  return sheet ? legendRows(sheet).length : 0;
});

/** 選択中の電線品番凡例ブロックに並ぶ行数 (中身はシートの配線から導出する)。 */
const wireLegendRowCount = computed(() => {
  const sheet = store.activeSheet;
  return sheet ? wireLegendRows(sheet, store.project?.wire_parts).length : 0;
});

async function apply() {
  const sheet = store.activeSheet;
  const e = selected.value;
  if (!sheet || !e) return;
  let entity: Entity;
  if (e.kind === "wire") {
    // 色・線径・品番・長さ (長さの出所つき。FreeCAD計測値の手上書きは警告する)
    const wireEdit = wireUpdateCommand(sheet.id, e, buf);
    // 線番はネット単位の属性なので、専用のset_wire_numbersコマンドで書き換える
    const numberCommand = wireNumberCommand(sheet.id, e, buf.wire_no);
    if (wireEdit.command) await store.execute(wireEdit.command);
    if (wireEdit.overwroteMeasured) ui.log(t("mech.overwroteLog"));
    if (numberCommand) {
      await store.execute(numberCommand);
      const number = buf.wire_no.trim();
      ui.log(
        number
          ? t("wireNumbers.setLog", { number })
          : t("wireNumbers.clearedLog"),
      );
    }
    return;
  } else if (e.kind === "symbol") {
    // 機能ラベル (G4)・実機ブロック表示 (G7)・嵌合相手 (G14) はattrsに入れる。
    // パネルが持たない欄の属性はそのまま残す (symbolAttrsが正)
    const attrs = symbolAttrs(e, {
      functionLabel: buf.functionLabel,
      deviceBlock: buf.deviceBlock,
      mate: buf.mate,
    });
    entity = { ...e, reference: buf.reference, value: buf.value, attrs };
    if (isDeviceBlock(e) !== buf.deviceBlock) {
      ui.log(
        t("relay.deviceBlockChangedLog", {
          reference: buf.reference.trim(),
          state: t(buf.deviceBlock ? "relay.deviceBlockOn" : "relay.deviceBlockOff"),
        }),
      );
    }
  } else if (e.kind === "text") {
    // 注記の装飾 (G4)。既定色は色指定なし (null) に戻して、旧い図面と同じ形で保存する
    entity = {
      ...e,
      anchor: buf.anchor,
      color: buf.textColor === "default" ? null : buf.textColor,
      bold: buf.bold,
    };
  } else if (e.kind === "net_label") {
    // タグ向きはattrsに入れる。「なし」はキーごと消して従来の図面と同じ形に戻す
    const attrs = { ...(e.attrs ?? {}) };
    if (buf.tag) attrs.tag = buf.tag;
    else delete attrs.tag;
    entity = { ...e, name: buf.name, attrs };
    if ((e.attrs?.tag ?? "") !== buf.tag) {
      const tag = tagDirs.find(([v]) => v === buf.tag)?.[1] ?? "netLabel.tagNone";
      ui.log(t("netLabel.tagChangedLog", { name: buf.name.trim(), tag: t(tag) }));
    }
  } else if (e.kind === "nameplate") {
    const command = nameplateUpdateCommand(sheet.id, e, {
      line1: buf.npLine1,
      line2: buf.npLine2,
      text_mm: String(buf.npText),
      position: buf.npPosition,
      follow: buf.npFollow,
      part_no: buf.part_no,
    });
    if (command) {
      await store.execute(command);
      ui.log(t("panelMarks.nameplate.updatedLog"));
    }
    return;
  } else if (isCable(e)) {
    // ケーブルは型番・芯数・線径・シールド・長さを書き換える (芯線割当は定義線が決めたまま)
    const command = cableUpdateCommand(sheet.id, e, {
      reference: buf.reference,
      part_no: buf.part_no,
      core_count: String(buf.cableCores),
      sq: String(buf.cableSq),
      shield: buf.cableShield,
      grounded: buf.cableGrounded,
      length_m: String(buf.cableLength),
    });
    if (command) {
      await store.execute(command);
      ui.log(t("cable.updatedLog", { reference: buf.reference.trim() }));
    } else {
      ui.log(t("cable.invalidLog"));
    }
    return;
  } else if (e.kind === "harness") {
    // ハーネスは名前・備考だけを書き換える (囲みの形はキャンバス側の操作で変える)
    const command = harnessUpdateCommand(sheet.id, e, buf.name, buf.note);
    if (command) {
      await store.execute(command);
      ui.log(t("harness.renamedLog", { name: buf.name.trim() }));
    }
    return;
  } else if (e.kind === "shape") {
    // 図形 (G10) は見た目だけを書き換える (形はキャンバス側の操作で変える)
    const patch: ShapePatch = {
      stroke: buf.shapeStroke,
      stroke_width_mm: buf.shapeWidth,
      fill: buf.shapeFill === "" ? null : buf.shapeFill,
      dashed: buf.shapeDashed,
    };
    const command = shapeUpdateCommand(sheet.id, e, patch);
    if (command) {
      await store.execute(command);
      ui.log(t("shapes.styledLog", { kind: t(`shapes.kind.${e.shape}`) }));
    }
    return;
  } else if (
    e.kind === "footprint" ||
    e.kind === "panel_outline" ||
    e.kind === "duct" ||
    e.kind === "din_rail"
  ) {
    // 盤の要素 (M4 §8): 寸法・向き・型番の編集。変わっていなければ何も送らない
    const command = panelUpdateCommand(sheet.id, e, panelPatch(e));
    if (command) await store.execute(command);
    return;
  } else {
    return;
  }
  await store.execute({ type: "update_entity", sheet_id: sheet.id, entity });
}

/** 選択中がブロック (ピン表で形が決まるシンボル) か。 */
const isBlock = computed(
  () => selected.value?.kind === "symbol" && selected.value.symbol_id === BLOCK_SYMBOL_ID,
);

/** 選択中がリレーコイルか (実機ブロック切替 (G7) を出す条件)。 */
const isRelayCoil = computed(
  () => selected.value?.kind === "symbol" && selected.value.symbol_id === COIL_SYMBOL_ID,
);

/** 選択中が嵌合コネクタ一族か (嵌合相手欄 (G14) を出す条件)。 */
const isConnector = computed(
  () => selected.value?.kind === "symbol" && isConnectorInstance(selected.value.symbol_id),
);

/** 選択中のブロックのピン構成 (「左5 / 右3」)。 */
const blockPinCounts = computed(() => {
  const e = selected.value;
  if (e?.kind !== "symbol") return "";
  const def = store.resolveInstance(e);
  if (!def) return t("blockEditor.broken");
  const count = (dir: string) => def.pins.filter((p) => p.dir === dir).length;
  return t("blockEditor.pinCounts", { left: count("left"), right: count("right") });
});

/** ピン表エディタを開く (確定でupdate_entityコマンド)。 */
function editBlockPins() {
  const e = selected.value;
  if (e?.kind === "symbol") blockEditor.startEdit(e);
}

// 種別ラベルは`entityKindLabels`が正 (全種類そろっているかを型とテストで守る)
const kindLabel = computed<Record<string, string>>(() => entityKindLabels(t));
</script>

<template>
  <aside class="panel">
    <div class="panel-head">
      <span>{{ t("properties.title") }}</span>
      <Pin :size="11" class="muted" />
    </div>
    <div class="type-row">
      <span v-if="selected">{{ kindLabel[selected.kind] }} (1)</span>
      <span v-else-if="store.selection.size > 1">{{ t("properties.selectedCount", { count: store.selection.size }) }}</span>
      <span v-else class="muted">{{ t("properties.none") }}</span>
      <ChevronDown :size="11" class="muted" />
    </div>

    <template v-if="selected?.kind === 'wire'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("properties.general") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("properties.color") }}</span>
        <span class="pvalue">
          <span class="swatch" :data-color="buf.color" />
          <select v-model="buf.color" class="bare">
            <option v-for="v in wireColors" :key="v" :value="v">{{ t(`properties.colors.${v}`) }} ({{ v }})</option>
          </select>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("properties.lineType") }}</span>
        <span class="pvalue">Continuous</span>
      </div>
      <div class="sec"><ChevronDown :size="10" /> {{ t("properties.electrical") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("properties.gauge") }}</span>
        <span class="pvalue">
          <select v-model.number="buf.sq" class="bare">
            <option v-for="v in sqValues" :key="v" :value="v">{{ v }} sq</option>
          </select>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("properties.partNo") }}</span>
        <span class="pvalue">
          <input
            v-model="buf.part_no"
            class="bare"
            :placeholder="autoPartNo || 'SAMPLE0001'"
            :title="t('wireParts.manualHint')"
          />
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("wireParts.autoRow") }}</span>
        <span class="pvalue" :title="t('wireParts.autoHint')">
          {{ autoPartNo || t("wireParts.autoNone") }}
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("properties.length") }}</span>
        <span class="pvalue">
          <input v-model="buf.length_m" class="bare" placeholder="0.4" />
          <span v-if="selected.length_source === 'freecad'" class="badge" :title="t('mech.measuredHint')">{{ t("mech.measured") }}</span>
        </span>
      </div>
      <div v-if="mechLink" class="prow">
        <span class="plabel">{{ t("mech.linkLabel") }}</span>
        <span class="pvalue mono" :title="mechLink.fcstd_path">{{ mechLinkText }}</span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("wireNumbers.propertyLabel") }}</span>
        <span class="pvalue">
          <input
            v-model="buf.wire_no"
            class="bare mono"
            :title="t('wireNumbers.propertyHint')"
            placeholder="1"
            spellcheck="false"
            @keyup.enter="apply"
          />
        </span>
      </div>
    </template>

    <template v-else-if="selected?.kind === 'symbol'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("properties.identity") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("properties.reference") }}</span>
        <span class="pvalue"><input v-model="buf.reference" class="bare" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("properties.value") }}</span>
        <span class="pvalue"><input v-model="buf.value" class="bare" placeholder="RLY-24V" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("note.functionRow") }}</span>
        <span class="pvalue" :title="t('note.functionHint')">
          <input
            v-model="buf.functionLabel"
            class="bare"
            :placeholder="t('note.functionPlaceholder')"
            @keyup.enter="apply"
          />
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("properties.symbol") }}</span>
        <span class="pvalue">{{ selected.symbol_id }}</span>
      </div>
      <div v-if="mechLink" class="prow">
        <span class="plabel">{{ t("mech.linkLabel") }}</span>
        <span class="pvalue mono" :title="mechLink.fcstd_path">{{ mechLinkText }}</span>
      </div>
      <div v-if="isRelayCoil" class="prow">
        <span class="plabel">{{ t("relay.deviceBlockRow") }}</span>
        <span class="pvalue" :title="t('relay.deviceBlockHint')">
          <label class="check">
            <input v-model="buf.deviceBlock" type="checkbox" @change="apply" />
            {{ t("relay.deviceBlockLabel") }}
          </label>
        </span>
      </div>
      <div v-if="isConnector" class="prow">
        <span class="plabel">{{ t("mate.row") }}</span>
        <span class="pvalue" :title="t('mate.hint')">
          <input
            v-model="buf.mate"
            class="bare mono"
            :placeholder="t('mate.placeholder')"
            spellcheck="false"
            @keyup.enter="apply"
          />
        </span>
      </div>
      <template v-if="isBlock">
        <div class="prow">
          <span class="plabel">{{ t("blockEditor.pinsRow") }}</span>
          <span class="pvalue mono">{{ blockPinCounts }}</span>
        </div>
        <div class="apply-row">
          <button class="secondary" :title="t('blockEditor.editHint')" @click="editBlockPins">
            {{ t("blockEditor.editPins") }}
          </button>
        </div>
      </template>
    </template>

    <template v-else-if="selected?.kind === 'harness'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("harness.kindLabel") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("harness.kindRow") }}</span>
        <span class="pvalue" :title="t('harness.kindHint')">{{ t("harness.kindLabel") }}</span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("harness.nameRow") }}</span>
        <span class="pvalue">
          <input
            v-model="buf.name"
            class="bare mono"
            :title="t('harness.nameHint')"
            placeholder="W1"
            spellcheck="false"
            @keyup.enter="apply"
          />
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("harness.wireCountRow") }}</span>
        <span class="pvalue stacked">
          <span>{{ t("harness.wireCount", { count: harnessWires }) }}</span>
          <span class="hint">
            {{
              buf.name.trim()
                ? t("harness.wireCountHint", { name: buf.name.trim() })
                : t("harness.wireCountHintUnnamed")
            }}
          </span>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("harness.noteRow") }}</span>
        <span class="pvalue">
          <input
            v-model="buf.note"
            class="bare"
            :placeholder="t('harness.notePlaceholder')"
            @keyup.enter="apply"
          />
        </span>
      </div>
    </template>

    <!-- 盤の銘板 (M4 §8): 文字・文字高・位置・追従・型番 -->
    <template v-else-if="selected?.kind === 'nameplate'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("panelMarks.nameplate.kindLabel") }}</div>
      <div class="prow"><span class="plabel">{{ t("panelMarks.nameplate.line1") }}</span><span class="pvalue"><input v-model="buf.npLine1" class="bare" :disabled="buf.npFollow && !!selected.footprint" @keyup.enter="apply" /></span></div>
      <div class="prow"><span class="plabel">{{ t("panelMarks.nameplate.line2") }}</span><span class="pvalue"><input v-model="buf.npLine2" class="bare" @keyup.enter="apply" /></span></div>
      <div class="prow"><span class="plabel">{{ t("panelMarks.nameplate.textHeight") }}</span><span class="pvalue"><input v-model="buf.npText" class="bare mono" type="number" min="1" step="0.5" @keyup.enter="apply" /></span></div>
      <div class="prow" v-if="selected.footprint">
        <span class="plabel">{{ t("panelMarks.nameplate.position") }}</span>
        <span class="pvalue">
          <select v-model="buf.npPosition" class="bare">
            <option value="below">{{ t("panelMarks.nameplate.below") }}</option>
            <option value="above">{{ t("panelMarks.nameplate.above") }}</option>
          </select>
        </span>
      </div>
      <div class="prow"><span class="plabel">{{ t("panelMarks.nameplate.partNo") }}</span><span class="pvalue"><input v-model="buf.part_no" class="bare mono" @keyup.enter="apply" /></span></div>
      <div class="prow" v-if="selected.footprint">
        <span class="plabel" />
        <span class="pvalue"><label><input v-model="buf.npFollow" type="checkbox" /> {{ t("panelMarks.nameplate.follow") }}</label></span>
      </div>
    </template>

    <!-- 盤の寸法線 (M4 §8): 値と向き (値は部品の位置から決まるので読み取りだけ) -->
    <template v-else-if="selected?.kind === 'dimension'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("panelMarks.dimension.kindLabel") }}</div>
      <div class="prow"><span class="plabel">{{ t("panelMarks.dimension.valueRow") }}</span><span class="pvalue mono">{{ dimensionText }} mm</span></div>
      <div class="prow"><span class="plabel">{{ t("panelMarks.dimension.axisRow") }}</span><span class="pvalue">{{ t(`panelMarks.dimension.${selected.axis}`) }}</span></div>
      <p class="cable-hint">{{ t("panelMarks.dimension.ribbonHint") }}</p>
    </template>

    <!-- 多芯ケーブル (M4 §5、.pen「M4残りデザイン④」): 型番・芯数・線径・シールド・長さと芯線割当 -->
    <template v-else-if="selected?.kind === 'cable' || selected?.kind === 'run_cable'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("cable.kindLabel") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("cable.referenceRow") }}</span>
        <span class="pvalue"><input v-model="buf.reference" class="bare mono" spellcheck="false" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("cable.partRow") }}</span>
        <span class="pvalue"><input v-model="buf.part_no" class="bare mono" spellcheck="false" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("cable.coresRow") }}</span>
        <span class="pvalue"><input v-model="buf.cableCores" class="bare mono" type="number" min="1" step="1" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("cable.sqRow") }}</span>
        <span class="pvalue"><input v-model="buf.cableSq" class="bare mono" type="number" min="0" step="0.25" placeholder="—" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("cable.shieldRow") }}</span>
        <span class="pvalue">
          <label><input v-model="buf.cableShield" type="checkbox" /> {{ t("cable.shieldOn") }}</label>
          <label v-if="buf.cableShield"><input v-model="buf.cableGrounded" type="checkbox" /> {{ t("cable.grounded") }}</label>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("cable.lengthRow") }}</span>
        <span class="pvalue"><input v-model="buf.cableLength" class="bare mono" type="number" min="0" step="0.1" placeholder="—" @keyup.enter="apply" /></span>
      </div>
      <div class="sec"><ChevronDown :size="10" /> {{ t("cable.coresSection") }}</div>
      <table class="cores">
        <thead>
          <tr>
            <th>{{ t("cable.colCore") }}</th>
            <th>{{ t("cable.colColor") }}</th>
            <th v-if="!selectedIsRun">{{ t("cable.colWire") }}</th>
            <th v-if="selectedIsRun">{{ t("cable.colEndA") }}</th>
            <th v-if="selectedIsRun">{{ t("cable.colEndB") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="c in selected.cores" :key="c.core">
            <td class="mono">{{ c.core }}</td>
            <td>{{ c.color }}</td>
            <td v-if="!selectedIsRun" class="mono">{{ c.wire ? t("cable.assigned") : t("cable.spare") }}</td>
            <td v-if="selectedIsRun" class="mono">{{ coreEndText(c.end_a) }}</td>
            <td v-if="selectedIsRun" class="mono">{{ coreEndText(c.end_b) }}</td>
          </tr>
          <tr v-for="n in Math.max(0, selected.core_count - selected.cores.length)" :key="`spare-${n}`" class="spare">
            <td class="mono">{{ selected.cores.length + n }}</td>
            <td>—</td>
            <td>{{ t("cable.spare") }}</td>
            <td v-if="selectedIsRun">—</td>
          </tr>
        </tbody>
      </table>
      <p v-if="selected.cores.length > selected.core_count" class="cable-hint warn">{{ t("cable.overflowHint") }}</p>
      <p class="cable-hint">{{ selectedIsRun ? t("cable.runHint") : t("cable.hint") }}</p>
    </template>

    <!-- 図形 (G10): 電気的な意味を持たない作図要素。見た目だけを編集する -->
    <template v-else-if="selected?.kind === 'shape'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("shapes.kindLabel") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("shapes.kindRow") }}</span>
        <span class="pvalue">{{ t(`shapes.kind.${selected.shape}`) }}</span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("shapes.strokeRow") }}</span>
        <span class="pvalue" :title="t('shapes.strokeHint')">
          <select v-model="buf.shapeStroke" class="bare" @change="apply">
            <option v-for="c in TEXT_COLOR_TOKENS" :key="c" :value="c">
              {{ t(`note.color.${c}`) }}
            </option>
          </select>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("shapes.widthRow") }}</span>
        <span class="pvalue">
          <select v-model.number="buf.shapeWidth" class="bare mono" @change="apply">
            <option v-for="w in SHAPE_STROKE_CHOICES" :key="w" :value="w">{{ w }} mm</option>
          </select>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("shapes.fillRow") }}</span>
        <span class="pvalue" :title="t('shapes.fillHint')">
          <select v-model="buf.shapeFill" class="bare" @change="apply">
            <option value="">{{ t("shapes.fillNone") }}</option>
            <option v-for="c in TEXT_COLOR_TOKENS" :key="c" :value="c">
              {{ t(`note.color.${c}`) }}
            </option>
          </select>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("shapes.dashedRow") }}</span>
        <span class="pvalue">
          <input v-model="buf.shapeDashed" type="checkbox" @change="apply" />
        </span>
      </div>
      <div class="hint hint-block">{{ t("shapes.prop.hint") }}</div>
    </template>

    <!-- 盤レイアウト (M4 §8): フットプリント -->
    <template v-else-if="selected?.kind === 'footprint'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("panel.prop.kind.footprint") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.reference") }}</span>
        <span class="pvalue mono" :title="t('panel.prop.referenceHint')">
          {{ selected.reference || "—" }}
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.partNo") }}</span>
        <span class="pvalue">
          <input v-model="buf.part_no" class="bare mono" spellcheck="false" @keyup.enter="apply" />
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.width") }}</span>
        <span class="pvalue"><input v-model="buf.w" class="bare mono" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.height") }}</span>
        <span class="pvalue"><input v-model="buf.h" class="bare mono" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.rotation") }}</span>
        <span class="pvalue">
          <select v-model.number="buf.rotation" class="bare" @change="apply">
            <option :value="0">0°</option>
            <option :value="90">90°</option>
          </select>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.itemNo") }}</span>
        <span class="pvalue">
          <span v-if="selected.item_no != null" class="mono">{{ selected.item_no }}</span>
          <span v-else class="muted">{{ t("panel.prop.itemNoNone") }}</span>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.rail") }}</span>
        <span class="pvalue">
          <span v-if="footprintRail">{{ t("panel.prop.railName", { index: footprintRail.index }) }}</span>
          <span v-else class="muted">{{ t("panel.prop.railNone") }}</span>
        </span>
      </div>
    </template>

    <!-- 盤レイアウト: 凡例表 (中身は図面から導出するので置き場所だけ) -->
    <template v-else-if="selected?.kind === 'panel_legend'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("panel.prop.kind.panel_legend") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.legendRows") }}</span>
        <span class="pvalue mono">{{ legendRowCount }}</span>
      </div>
      <div class="hint hint-block">{{ t("panel.prop.legendHint") }}</div>
    </template>

    <!-- 電線品番凡例ブロック (G9残、中身は図面から導出するので置き場所だけ) -->
    <template v-else-if="selected?.kind === 'wire_legend'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("wireLegend.prop.title") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("wireLegend.prop.rows") }}</span>
        <span class="pvalue mono">{{ wireLegendRowCount }}</span>
      </div>
      <div class="hint hint-block">{{ t("wireLegend.prop.hint") }}</div>
    </template>

    <!-- 盤レイアウト: 盤の外形 (取付板つき) -->
    <template v-else-if="selected?.kind === 'panel_outline'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("panel.prop.kind.panel_outline") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.name") }}</span>
        <span class="pvalue"><input v-model="buf.name" class="bare" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.width") }}</span>
        <span class="pvalue"><input v-model="buf.w" class="bare mono" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.height") }}</span>
        <span class="pvalue"><input v-model="buf.h" class="bare mono" @keyup.enter="apply" /></span>
      </div>
    </template>

    <!-- 盤レイアウト: 配線ダクト -->
    <template v-else-if="selected?.kind === 'duct'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("panel.prop.kind.duct") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.width") }}</span>
        <span class="pvalue"><input v-model="buf.w" class="bare mono" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.height") }}</span>
        <span class="pvalue"><input v-model="buf.h" class="bare mono" @keyup.enter="apply" /></span>
      </div>
    </template>

    <!-- 盤レイアウト: DINレール (yは取付線) -->
    <template v-else-if="selected?.kind === 'din_rail'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("panel.prop.kind.din_rail") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.railY") }}</span>
        <span class="pvalue"><input v-model="buf.railY" class="bare mono" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.railX1") }}</span>
        <span class="pvalue"><input v-model="buf.railX1" class="bare mono" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("panel.prop.railX2") }}</span>
        <span class="pvalue"><input v-model="buf.railX2" class="bare mono" @keyup.enter="apply" /></span>
      </div>
    </template>

    <!-- 注記テキストの装飾 (G4): 基準位置・色トークン・太字 -->
    <template v-else-if="selected?.kind === 'text'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("note.section") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("note.textRow") }}</span>
        <span class="pvalue">{{ selected.text }}</span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("note.anchorRow") }}</span>
        <span class="pvalue" :title="t('note.anchorHint')">
          <select v-model="buf.anchor" class="bare" @change="apply">
            <option v-for="a in TEXT_ANCHORS" :key="a" :value="a">{{ t(`note.anchor.${a}`) }}</option>
          </select>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("note.colorRow") }}</span>
        <span class="pvalue" :title="t('note.colorHint')">
          <span class="swatch" :data-note-color="buf.textColor" />
          <select v-model="buf.textColor" class="bare" @change="apply">
            <option v-for="c in TEXT_COLOR_TOKENS" :key="c" :value="c">{{ t(`note.color.${c}`) }}</option>
          </select>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("note.boldRow") }}</span>
        <span class="pvalue" :title="t('note.boldHint')">
          <label class="check">
            <input v-model="buf.bold" type="checkbox" @change="apply" />
            {{ t("note.boldLabel") }}
          </label>
        </span>
      </div>
    </template>

    <template v-else-if="selected?.kind === 'net_label'">
      <div class="sec"><ChevronDown :size="10" /> {{ t("properties.net") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("properties.netName") }}</span>
        <span class="pvalue"><input v-model="buf.name" class="bare" @keyup.enter="apply" /></span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("netLabel.tagRow") }}</span>
        <span class="pvalue" :title="t('netLabel.tagHint')">
          <select v-model="buf.tag" class="bare" @change="apply">
            <option v-for="[value, key] in tagDirs" :key="value" :value="value">{{ t(key) }}</option>
          </select>
        </span>
      </div>
      <div class="prow">
        <span class="plabel">{{ t("xref.propertyLabel") }}</span>
        <span class="pvalue stacked" :title="t('xref.propertyHint')">
          <template v-if="xrefTargets.length">
            <button
              v-for="site in xrefTargets"
              :key="site.label_id"
              class="xref-link"
              :title="site.sheet_name"
              @click="jumpToXref(site)"
            >
              {{ site.address }}
            </button>
          </template>
          <template v-else>
            <span class="muted">{{ t("xref.none") }}</span>
            <span class="hint">{{ t("xref.noneHint") }}</span>
          </template>
        </span>
      </div>
    </template>

    <!-- 選択が無いときはシート全体の設定 (図面規格) を出す -->
    <template v-if="!selected && store.selection.size === 0">
      <div class="sec"><ChevronDown :size="10" /> {{ t("annotationText.section") }}</div>
      <div class="prow">
        <span class="plabel">{{ t("annotationText.row") }}</span>
        <span class="pvalue" :title="t('annotationText.hint')">
          <input
            v-model="buf.annotationText"
            class="bare mono"
            type="number"
            :min="MIN_ANNOTATION_TEXT_MM"
            :max="MAX_ANNOTATION_TEXT_MM"
            step="0.1"
            :placeholder="String(sheetAnnotationText)"
            @change="applyAnnotationText"
            @keyup.enter="applyAnnotationText"
          />
        </span>
      </div>
      <div class="hint hint-block">
        {{ t("annotationText.range", { min: MIN_ANNOTATION_TEXT_MM, max: MAX_ANNOTATION_TEXT_MM }) }}
      </div>
    </template>

    <div v-if="selected" class="apply-row">
      <button class="apply" @click="apply">{{ t("properties.apply") }}</button>
    </div>
  </aside>
</template>

<style scoped>
.panel {
  width: 240px;
  flex: none;
  background: var(--palette-bg);
  border-left: 1px solid var(--ribbon-line);
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  user-select: none;
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px 9px;
  background: var(--palette-head);
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.type-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px 9px;
  font-size: 11px;
  color: var(--ui-text);
  border-bottom: 1px solid var(--ribbon-line);
}
.muted { color: var(--ui-muted); }
.sec {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 9px;
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-text);
  background: var(--hover-bg);
}
.prow {
  display: flex;
  border-bottom: 1px solid var(--ribbon-line);
  font-size: 10px;
}
.plabel {
  width: 88px;
  flex: none;
  padding: 5px 9px;
  color: var(--ui-muted);
}
.pvalue {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 8px;
  background: #fff;
  color: var(--ui-text);
}
.bare {
  width: 100%;
  border: none;
  outline: none;
  background: transparent;
  font-size: 10px;
  color: var(--ui-text);
  padding: 2px 0;
}
.mono { font-family: var(--mono-font); }
.badge {
  margin-left: 6px;
  font-size: 10px;
  font-weight: 600;
  border-radius: 999px;
  padding: 1px 6px;
  color: var(--info-fg);
  background: var(--info-bg);
}
.hint { color: var(--ui-muted); }
/* 行の下に単独で置く補足 (プロパティ行と同じ字送り・余白のcaption) */
.hint-block {
  font-size: 10px;
  line-height: 1.45;
  padding: 5px 9px;
}
.pvalue.stacked {
  flex-direction: column;
  align-items: flex-start;
  gap: 1px;
}
/* 参照リンク値 (XRef相手先「/2.B3」): acad-blue mono 600、クリックでジャンプ */
.xref-link {
  border: 0;
  background: none;
  padding: 0;
  font: 600 10px var(--mono-font);
  color: var(--acad-blue);
  cursor: pointer;
}
.xref-link:hover { text-decoration: underline; }
.swatch {
  width: 11px;
  height: 11px;
  border-radius: 2px;
  flex: none;
  background: #c00000;
}
.swatch[data-color="black"] { background: #202020; }
.swatch[data-color="white"] { background: #f0f0f0; border: 1px solid #ccc; }
.swatch[data-color="blue"] { background: #0000c0; }
.swatch[data-color="yellow"] { background: #c8a800; }
.swatch[data-color="green"] { background: #008040; }
.swatch[data-color="orange"] { background: #d07010; }
.swatch[data-color="purple"] { background: #8020a0; }
.swatch[data-color="brown"] { background: #805020; }
.swatch[data-color="gray"] { background: #808080; }
.swatch[data-color="pink"] { background: #d06090; }
.swatch[data-color="light_blue"] { background: #2090c0; }
/* 注記色トークンの見本 (印刷色。画面のキャンバスは canvas/theme.ts の textColors) */
.swatch[data-note-color="default"] { background: var(--note-default); }
.swatch[data-note-color="accent"] { background: var(--note-accent); }
.swatch[data-note-color="warning"] { background: var(--note-warning); }
.swatch[data-note-color="success"] { background: var(--note-success); }
.swatch[data-note-color="muted"] { background: var(--note-muted); }
.check {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 10px;
  cursor: pointer;
}
.apply-row {
  display: flex;
  justify-content: flex-end;
  padding: 10px;
}
/* セカンダリ操作 (ピン表を編集…): 白+枠 */
.secondary {
  background: var(--card-bg);
  color: var(--ui-text);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 5px 12px;
  font-size: 11px;
  cursor: pointer;
}
.secondary:hover { background: var(--hover-bg); }
.apply {
  background: var(--acad-blue);
  color: #fff;
  border: none;
  border-radius: 4px;
  padding: 5px 16px;
  font-size: 12px;
  cursor: pointer;
}
.cores { width: 100%; border-collapse: collapse; font-size: 10px; color: var(--ui-text); }
.cores th { text-align: left; font-weight: 600; color: var(--ui-muted); padding: 2px 10px; border-bottom: 1px solid var(--ribbon-line); }
.cores td { padding: 2px 10px; border-bottom: 1px solid var(--ribbon-line); }
.cores tr.spare td { color: var(--ui-placeholder); }
.cable-hint { margin: 0; padding: 4px 10px; font-size: 10px; line-height: 1.4; color: var(--ui-muted); }
.cable-hint.warn { color: var(--err-fg); }
</style>
