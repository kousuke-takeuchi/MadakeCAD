# コネクタの絵: ブロックのピンがどんなコネクタかを箱のふちに描く

日付: 2026-10-25 · 状態: 設計の決定はユーザー承認済み、設計書はレビュー待ち · 英語版 (正本): [2026-10-25-connector-pictograms-design.md](2026-10-25-connector-pictograms-design.md)

## 目的

ブロックのピンが**どんな実物のコネクタか**を、ブロックのふちに白黒の小さな絵で描けるようにする。図面が実物のように読める: USB-A の差し口が 4 つ並んだハブ、ネジの端子台が付いたアンプ、JST の差し口がある自作基板 (ユーザー:「端子台が付いているイラストのシンボル」「USBもシンボルとして見た目的に分かりやすい表現」)。

AOGU の図面での合格の形 (マージの後に当てはめる。「進め方」参照):

- ハブ `39` は、上のふちに USB-A メスが 4 つ並び (`U1`〜`U4`)、下のふちに micro-B メス `PWR`、右のふちに本体から出ている OTG ケーブルを micro-B オス `OTG` として描く。
- アンプ `53` は 3.5mm ジャック `IN` と、`5V`・`GND` のピンをまとめて囲む 1 つの端子台 `TB1` を描く。
- 基板 `33` は `J3`・`J9` を JST PH メスとして `J3 PH`・`J9 PH` の文字付きで描き、基板 `44` は JST XH メスを描く。
- 同じ絵がキャンバス、SVG・PDF・PNG・`get_sheet_image`、DXF、KiCad 出力に出る。
- コネクタの無い図面は、見た目・出力・保存がこれまでと全く同じ (ファイル形式の番号だけ変わる)。

## いまの困りごと

ブロックのピンは、引き出し線と 2 つの文字だけ。`block_symbol_from_spec` はどのピンも同じに描く: 箱のふちから 2.5 mm の引き出し線、箱の外の線の上に番号、箱の中に名前 (`block.rs:309-357`)。「これは USB-A のメス」と伝える方法はピン名 (`USB-A メス`) だけ。ハブ・ドングル・DCDC・バッテリー・自作基板 3 枚がある図面では、どれが USB で、どれが端子台で、どれが JST か、ケーブルを差す側 (メス) か付いている側 (オス) かを、名前を 1 つずつ読まないと分からない。

ライブラリのかみ合わせ記号 (`connector_plug` / `connector_socket`) はピンの横に置く別の部品で、ブロック自身の差し口を表すものではない。これは今のまま残す。

## いまあるもの (コードで確認済み)

- **ブロックのピン表** (`madake-core/src/block.rs`): `BlockPin { no, name, contacts }` は `#[serde(deny_unknown_fields)]` (`block.rs:50-62`)、`BlockPins` も同じ (`block.rs:64-76`)。ピン表は JSON の**文字列** `attrs["block_pins"]`。Rust は読むだけで書き戻さないので、保存される文字はエディタやエージェントが書いたそのまま。
- **検査**: `parse_block_spec` (`block.rs:149-197`) が JSON、ピン数 (`BLOCK_PIN_MAX` = 200)、番号の空・重複、`check_contacts` (`block.rs:201-227`) を調べる。エラー文は**英語** (例 `block pin {no}: duplicate contact {c}`)。`Engine` は追加・更新のときに同じ読み取りをする (`command.rs:1778` `validate_entity`) ので、悪いピン表はコマンドの時点で断られる。属性が読めないブロックは定義が無くなり、描かれない。
- **定数** (`block.rs:29-48`): `BLOCK_PIN_PITCH_MM` 5.0、`BLOCK_LEAD_MM` 2.5、`BLOCK_PIN_TEXT_MM` 2.0、`BLOCK_TEXT_PAD_MM` 1.5、`BLOCK_SIZE_STEP_MM` 5.0、`BLOCK_MIN_W_MM` 15、`BLOCK_MIN_H_MM` 10。
- **形** (`block_symbol_from_spec`、`block.rs:298-380`): 外形は原点中心の `Rect` 1 つ (`block.rs:301-305`)。ピンは辺ごとに 5 mm ピッチで中央揃え (`offset`、`block.rs:119-121`)。左右: ピンの点は 2.5 mm 外、番号は `(ふち ∓ 1.25, off − 2.0)`、名前は箱の中でふちから 1.5 mm のところから書く。上下: 番号**も**名前も箱の中で、ふちから 2.0 mm と 4.5 mm (`block.rs:327-336`)。引き出し線はふち → ピンの点の `Line`。
- **箱の大きさ** (`block_box`、`block.rs:246-267`): `block_size` の指定があればそれ。無ければ 幅 = max(15, 2 × 1.5 + 左の一番長い名前 + 右の一番長い名前 + 5, 列数 × 5 + 5)、高さ = max(10, 行数 × 5 + 5, `caption_inset(上)` + `caption_inset(下)` + 5)、どちらも 5 mm 刻みに切り上げ。`caption_inset` (`block.rs:134-142`) は 0 / 5.0 / 7.5。指定寸法の下限は `min_size` (`block.rs:230-237`、辺ごとに n × 5)。`block_box` を使うのはシンボルの生成とエディタの寸法表示だけ。
- **図形** (`symbol.rs:68-82`): `Line { pts }`、`Circle { center, r, filled }`、`Arc { center, r, start_deg, end_deg }` (用紙で時計回り、Y は下向き)、`Rect { p1, p2, filled }`、`Text { at, text, height }` (いつも横書きで `at` が中心)。`"type"` の札付きで JSON になり、フロントエンドと同じ形。
- **描画**: SVG の `render_symbol` (`svg.rs:385-455`) は塗りの `Circle`・`Rect` を `fill="#000"`、塗り無しを `fill="none"` で描き、線の太さは `SYMBOL_STROKE` 0.3 mm (`svg.rs:21`)。**白の塗りは無い**ので、塗り無しの図形の下にあるものは透けて見える。PDF・PNG・`get_sheet_image` はこの SVG から作る。DXF (`dxf.rs:420-459`): 塗りの `Rect` は `SOLID` を足し、塗りの `Circle` は円の中に収まる正方形の `SOLID` を足す。KiCad (`kicad.rs:562-590`): 塗り → `(fill (type outline))`。ピンは長さ 0 で書く (`kicad.rs:605`) ので、引き出し線は私たちの `Line` だけ。キャンバスの `drawSymbol` (`src/canvas/renderer.ts:405-476`): 塗りの円・四角は線無しで塗り、塗り無しは線で描く。
- **文字の大きさ**: `tidy.rs:210-213` はシンボルの `Text` を `at ± (text_width / 2, height / 2)` として測る。`text_width` (`tidy.rs:108-114`) = 高さ × (半角 0.6、全角 1.0 の合計)。TS の双子は `textWidthMm` (`src/canvas/textWidth.ts:27-31`)。
- **TS の双子** (`src/canvas/dynamicSymbol.ts`): 定数 (`:226-249`)、`BlockPin { no, name?, contacts? }` (`:254-259`)、`checkContacts` (`:452-467`、Rust と同じ文)、`parseBlockSpec` (`:469-530`。辺のキーは調べるがピンのキーは調べない)、`blockBox` (`:533-551`)、`blockSymbol` (`:557-`)。Rust と同じ座標になることは、`dynamicSymbol.test.ts` (`describe("blockSymbol")`、`:171`) と `block.rs` のテストに同じ期待値を書いて守っている。共有の正解ファイルの前例: `madake-core/tests/golden/*.svg` (`svg.rs:1587`)。TS のテストは `src-tauri/` の JSON を読み込める (`frameTemplate.test.ts:5`)。
- **エディタ**: `src/canvas/blockEditor.ts` (`BlockPinRow { no, name, contacts? }` `:40`、`isFilled` `:90`、`pinsOf` `:141`、`attrsFromDraft` `:162`、`draftFromAttrs` `:173`、`draftIssues` `:211`、`BlockIssue` `:53`)。`src/components/BlockPinColumn.vue` (1 行に 3 つの入力: 番号・名前・接点)。指摘の文は `BlockPinsEditor.vue:25-53`。`stores/blockEditor.ts:175` の `updateRow`。`PropertiesPanel.vue:476-477` はピン数と `portCount` を出す。言語は 6 つ `src/locales/{en,ja,zh,fr,de,es}.json`、キーがそろっているかは `src/i18n/i18n.test.ts` が調べる。
- **ファイル形式** 18 (`model.rs:60`)。接点のあるブロックは、`io::save_project` が `FileWriteGuard` を立てている間 (`io.rs:38-40`) だけ、ファイル専用の種類 `port_block` で書く (`model.rs:612-616` `is_port_block`、`:649`、`:678`)。新しい形式を断る仕組みの無い `v0.4.0` は知らない種類で失敗し、ブロックを黙って消さない。形式 17 のビルドは番号で断る (`io.rs:60` `check_format_version`)。IPC・MCP は `kind: "symbol"` のまま。
- **MCP・エージェント**: `place_symbol` の attrs の説明 (`madake-mcp/src/lib.rs:475-480`) とツールの説明 (`lib.rs:1242`) が `block_pins` と `contacts` を説明している。`knowledge.rs:65-71` にポートのピンの決まりがある。ブロックの MCP テストは `madake-mcp/tests/drawing_tools.rs:640-`。
- **ピングループの括弧** (`pin_groups.rs`) と整えの数え方は `PinDef.at` と図形を読むので、ピンの点が動いてもそのまま付いてくる。

## 設計

### 1. データ

**ブロックのピンの JSON** (`block.rs`): 省略できるキーを 2 つ足す。

```rust
pub struct BlockPin {
    pub no: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub contacts: Vec<String>,
    /// 箱のふちに描くコネクタの種類。None = 普通のピン (今と同じに描く)。
    #[serde(default)]
    pub connector: Option<ConnectorKind>,
    /// まとめて描くコネクタ (端子台・JST の差し口) の名前。例 "J3"、"TB1"。
    #[serde(default)]
    pub connector_ref: Option<String>,
}
```

```json
{"top":[{"no":"1","name":"U1","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]}],
 "left":[{"no":"4","name":"5V","connector":"screw","connector_ref":"TB1"},
         {"no":"5","name":"GND","connector":"screw","connector_ref":"TB1"}]}
```

**`ConnectorKind`** (新しい `madake-core/src/connector.rs`)、`#[serde(rename_all = "snake_case")]`、決まった種類だけで自由入力は無い:

| serde の名前 | 絵 | まとめる | 主な使いどころ (ユーザー確認済み) |
|---|---|---|---|
| `usb_a_f` | USB-A メス | いいえ | ハブの差し口、DCDC・バッテリーの USB 出力 |
| `usb_a_m` | USB-A オス | いいえ | LTE ドングル |
| `usb_c_f` | USB-C メス | いいえ | 基板 33 の USB-C IN |
| `usb_c_m` | USB-C オス | いいえ | |
| `micro_b_f` | micro-B メス | いいえ | ReSpeaker、ハブの電源 |
| `micro_b_m` | micro-B オス | いいえ | ハブの OTG ケーブルの先 |
| `jack35_f` | 3.5mm ジャック | いいえ | マイク 49、アイソレーター 51、アンプ 53 の入力、X58 |
| `jack35_m` | 3.5mm プラグ | いいえ | |
| `dc_f` | DC ジャック | いいえ | バッテリーの入力 |
| `dc_m` | DC プラグ | いいえ | DCDC 12V の出力 |
| `screw` | ネジの端子台 | はい | アンプの 5V・GND、スピーカー出力 |
| `jst_ph_f` | JST PH メス | はい | 基板 33 の J3・J9 |
| `jst_xh_f` | JST XH メス | はい | 基板 44 |

`ConnectorKind::is_grouped()` は `screw`・`jst_ph_f`・`jst_xh_f` で真。`ConnectorKind::series()` は JST で `Some("PH")` / `Some("XH")`、ほかは `None`。

**検査** (`parse_block_spec` に、ピンごとの検査の後で呼ぶ新しい `check_connectors(&BlockPins)` を足す。文は `check_contacts` と同じ書き方の英語):

1. 知らない `connector` の値は読み取りで失敗する: `block_pins is not valid JSON: unknown variant ...` (serde の文。今ある JSON エラーの通り道)。
2. `connector_ref` は前後の空白を落とす。落として空なら: `block pin {no}: connector_ref must not be empty`。
3. `connector` 無しの `connector_ref`: `block pin {no}: connector_ref needs a grouped connector (screw, jst_ph_f, jst_xh_f)`。
4. まとめない種類に `connector_ref`: `block pin {no}: connector_ref is only for screw, jst_ph_f and jst_xh_f, not {kind}`。
5. 空でない `connector_ref` が 2 つのまとまりになる (2 つの辺にある、ほかのピンで途切れる、種類が違う): `block pin {no}: connector_ref {ref} is used by another group; its pins must be next to each other on one side`。

**まとめ方** (関数 `connector_groups(side: &[BlockPin]) -> Vec<Group>` 1 つ、TS にも双子): 1 つの辺を表の順に見て、まとめる種類のピンがまとまりを始める。次のピンが**同じ種類で、前後の空白を落とした `connector_ref` も同じ** (無しと無しは同じ) ならそのまとまりに入る。違う辺のピンはまとまらない。まとめない種類のピンは 1 本だけのまとまり。`connector` の無いピンはどのまとまりにも入らない。名前の無いネジの端子台が 2 つ隣り合うと 1 つになるので、分けたいときは名前 (`TB1`・`TB2`) を付ける。

まとまりの**文字**: JST は `<ref> PH` / `<ref> XH`、`screw` は `<ref>`。名前の無い JST は `PH` / `XH` だけ、名前の無いネジの端子台は何も書かない。

**ほかと関係しない。** `connector` は絵のためだけ。`contacts` (USB の差し口はふつう `connector: usb_a_f` と接点 `VBUS, D-, D+, GND` の両方を持つ)、ネットリスト、ERC、帳票、BOM には何も効かない。`PinDef` にコネクタの欄は**足さない**。`get_pins` も変えない (エージェントは種類を `block_pins` から読む)。

**TS の双子** (`src/canvas/connector.ts`): `export type ConnectorKind = "usb_a_f" | ... | "jst_xh_f"`、`CONNECTOR_KINDS` (表の順の 13 個)、`isGroupedConnector(kind)`、`connectorSeries(kind)`、`connectorGroups(side)`。`dynamicSymbol.ts` の `BlockPin` に `connector?: ConnectorKind; connector_ref?: string` を足す。`parseBlockSpec` は同じ 5 つの検査を同じ順・同じ文でする。ただし 1 は `block_pins is not valid JSON: unknown connector {値}` を投げる (serde の文はまねできないので、この場合だけテストは `block_pins is not valid JSON` の頭の部分を比べる)。

### 2. 形

数はすべて mm。絵はどれも、ピンの**ふちの点** E (ピンが箱の外形と交わる点) に置いた**ローカルの座標**で作る: `u` は箱から外向き、`v` はふちに沿う向き。座標の向きは、右の辺の向きを回したもの (V = U を用紙の上で時計回りに 90° 回した向き。Y は下向き):

| 辺 | U (外向き) | V | 回転 θ |
|---|---|---|---|
| 右 | (1, 0) | (0, 1) | 0° |
| 下 | (0, 1) | (−1, 0) | 90° |
| 左 | (−1, 0) | (0, −1) | 180° |
| 上 | (0, −1) | (1, 0) | 270° |

ローカルの点 (u, v) は E + u·U + v·V になる。`Rect` は 2 つの角を移してから (小さい方, 大きい方) にそろえる。`Arc` は中心を移し、角度を `start + θ`、`end + θ` にする (start は [0, 360) に収め、回る量はそのまま) ので、時計回りは時計回りのまま。回転だけなので、左右の辺の「/」は上下の辺では「\」になる。これは受け入れる。

**新しい定数** (`connector.rs`、双子は `connector.ts`):

| 定数 | 値 | 意味 |
|---|---|---|
| `CONNECTOR_IN_MM` | 1.5 | 箱の中に入る深さ |
| `CONNECTOR_OUT_MM` | 2.5 | 箱の外に出る深さ |
| `CONNECTOR_HALF_SPAN_MM` | 2.0 | ふちに沿った長さの半分 (外形は 4 mm) |
| `CONNECTOR_LEAD_MM` | 5.0 | コネクタのあるピンの、ふち → ピンの点 |
| `CONNECTOR_NUM_GAP_MM` | 0.25 | 外形 → ピン番号 のすき間 |
| `CONNECTOR_LABEL_TOP_MM` | 7.0 | 上下の辺のまとまりの文字の中心の、ふちからの深さ (箱の中) |
| `CONNECTOR_LABEL_INSET_MM` | 10.0 | まとまりの文字を書く上下の辺の `caption_inset` |

5 mm ピッチに収まるか: 隣り合う 4 mm の外形の間は 1.0 mm、線の太さ 0.3 mm を引いて見た目 0.7 mm。自動の大きさの箱は一番外のピンから角まで 5 mm 以上、指定寸法の下限でも 2.5 mm あるので、半分 2 mm の外形は自分の辺の角に届かない。

#### 2.1 絵 (ローカル座標)

共通の部品: **E-rect** = `Rect (−1.5, −2) – (2.5, 2)` 塗り無し (メスの外形。ふちをまたぐ)、**H-rect** = `Rect (−1.5, −2) – (0, 2)` 塗り無し (オスの箱の中の部分)。メス = 白抜きの口がふちに入る、オス = **塗った差し込み部**がふちから u = 2.5 まで外へ出る。絵はどれも u = 2.5 で終わり、引き出し線 (§2.2) はそこから始まる。

| 種類 | 図形 (この順) |
|---|---|
| `usb_a_f` | E-rect、舌 `Rect (−0.8, −0.9) – (1.8, −0.1)` 塗り |
| `usb_a_m` | H-rect、差し込み部 `Rect (0, −1.2) – (2.5, 1.2)` 塗り |
| `usb_c_f` | E-rect、口 (角の丸い形): `Line (−0.2, −0.8) – (1.2, −0.8)`、`Arc 中心 (1.2, 0) 半径 0.8、−90° → 90°`、`Line (1.2, 0.8) – (−0.2, 0.8)`、`Arc 中心 (−0.2, 0) 半径 0.8、90° → 270°`、舌 `Line (0, 0) – (1.0, 0)` |
| `usb_c_m` | H-rect、差し込み部 `Rect (0, −0.8) – (1.7, 0.8)` 塗り、丸い先 `Circle 中心 (1.7, 0) 半径 0.8` 塗り |
| `micro_b_f` | 外形の `Line` (閉じる): (−1.5, −2)、(0, −2)、(2.5, −1.4)、(2.5, 1.4)、(0, 2)、(−1.5, 2)、(−1.5, −2) (箱の中はまっすぐ、外は台形)、舌 `Line (−0.6, 0) – (1.6, 0)` |
| `micro_b_m` | H-rect、差し込み部 `Rect (0, −0.6) – (2.5, 0.6)` 塗り |
| `jack35_f` | E-rect、穴 `Circle 中心 (0.5, 0) 半径 1.0`、中心ピン `Circle 中心 (0.5, 0) 半径 0.35` 塗り |
| `jack35_m` | H-rect、つば `Rect (0, −1.0) – (0.8, 1.0)` 塗り、先 `Rect (0.8, −0.4) – (2.5, 0.4)` 塗り |
| `dc_f` | E-rect、穴 `Circle 中心 (0.5, 0) 半径 1.5`、中心ピン `Circle 中心 (0.5, 0) 半径 0.6` 塗り |
| `dc_m` | H-rect、中空の筒を 2 枚の壁で: `Rect (0, −1.4) – (2.5, −0.5)` 塗り、`Rect (0, 0.5) – (2.5, 1.4)` 塗り |
| `screw` (まとめる) | 外形 `Rect (−1.5, v₀ − 2) – (2.5, v₁ + 2)`、ピンごと (vᵢ) に `Circle 中心 (0.5, vᵢ) 半径 1.2` と斜線 `Line (1.3, vᵢ − 0.8) – (−0.3, vᵢ + 0.8)` |
| `jst_ph_f`・`jst_xh_f` (まとめる) | 外形 `Rect (−1.5, v₀ − 2) – (2.5, v₁ + 2)`、差し込み面 `Line (1.5, v₀ − 2) – (1.5, v₁ + 2)`、ピンごとに接点 `Rect (−0.6, vᵢ − 0.6) – (0.6, vᵢ + 0.6)` 塗り無し |

まとまりでは、E は最初のピンのふちの点、vᵢ = (Eᵢ − E)·V、v₀ = vᵢ の最小、v₁ = vᵢ の最大。1 本だけの種類は v₀ = v₁ = 0。中の細部は外形の線から 0.3 mm 以上離す (線の太さを引いて見た目 0.15 mm 以上)。PH と XH の違いは文字だけ。

承認されたモックと違う絵が 2 つある (黒の塗りしか無いため): DC プラグの「先の白い穴」は中空の筒 (塗った壁 2 枚の間に引き出し線が入る) にし、USB-C の角の丸い四角は弧 2 つで描く。モックの白い塗りの代わりに、箱の外形にすき間をあける (§2.3)。

#### 2.2 ピン・番号・名前

コネクタの**ある**ピン (種類は問わない):

- **ピンの点** `PinDef.at` = E + 5.0·U (`CONNECTOR_LEAD_MM`)。今の 2.5 から変わる。そのピンにつながっていた配線は触れなくなるので、付け直す (ユーザーが受け入れ済み)。
- **引き出し線** = `Line` E + 2.5·U → E + 5.0·U (ふちからではなく、絵の外の端から)。
- **番号**、w = `text_width(no, 2.0)`。`off` はピンの辺に沿った位置 (`offset(i, n)`)、hw / hh は箱の大きさの半分:
  - 左右: x は左で −(hw + 2.75 + w / 2)、右で hw + 2.75 + w / 2。y = off − 2.0 (線の上、今と同じ高さ)。番号の内側の端はいつも外形の 0.25 mm 外なので、長い番号は外へ伸び、自分の外形にかからない。
  - 上下: y は上で −(hh + 3.75)、下で hh + 3.75 (外形の先の 2.5 mm の引き出し線のところ)。x = off + 0.5 + w / 2 (用紙で線の右)。
- **名前**、nw = その幅:
  - 左右: ふちから `BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM` = 3.0 mm 中から書く (今は 1.5): x は左で −hw + 3.0 + nw / 2、右で hw − 3.0 − nw / 2。y = off。
  - 上下: 変えない (箱の中 4.5 mm、ピンの真上)。文字の箱 (ふちから 3.5〜5.5 mm) はもともと 1.5 mm の外形から 2 mm 離れていて、そのままにすると同じ辺の普通のピンと 1 列にそろう。

コネクタの**無い**ピンは、ほかのピンにコネクタのあるブロックでも、今と全く同じに描く。

**まとまりの文字** (`Text`、高さ 2.0、w = その幅):

- 左右、n ≥ 2 本のまとまり: まとまりの k 本目と k + 1 本目の間のすき間 (k = ⌊(n − 1) / 2⌋、0 から数える)。つまり y = y(最初のピン) + 2.5 + 5k。名前と同じそろえ方 (ふちから 3.0 mm 中から内へ: x は左で −hw + 3.0 + w / 2、右で hw − 3.0 − w / 2)。この行は空いている: 名前はピンの ±1 mm (0.5 mm 離れる)、番号は箱の外。
- 左右、1 本のまとまり: そのピンの行の、名前の後ろ: 書き始め = 3.0 + nw + 1.0 (名前が空なら 3.0)。
- 上下: まとまりのピンの真ん中に中央そろえで、ふちから 7.0 mm 中 (6.0〜8.0 の帯。名前の帯 3.5〜5.5 の下)。

#### 2.3 箱の外形

コネクタの**無い**ブロックは外形の `Rect` 1 つのまま (図形は今と同じ)。コネクタが**1 つでもある**ブロックは、外形を `Line` の区切りで描き、絵がふちを横切るところに**すき間**をあける (絵の v の範囲 [v₀ − 2, v₁ + 2] をふちの上に移したところ)。これでメスの口が白く見える。順: 上 (x の小さい順)、右 (y の小さい順)、下 (x の小さい順)、左 (y の小さい順)。区切りはそれぞれ 2 点の `Line`、長さ 0 の区切りは書かない。オスの箱の中の部分とまとまりの外形は、自分の線ですき間をふさぐ。

#### 2.4 箱の大きさ (`block_box` / `blockBox`)

コネクタのあるピンが無ければ変わらない。あるときは:

- 辺ごとの余白 = `BLOCK_TEXT_PAD_MM` + (その辺にコネクタのあるピンがあれば 1.5)。
- 辺ごとの文字の幅 (左右) = その辺の次のうちの最大: 名前の幅、文字付きの 1 本のまとまりは 名前の幅 + 1.0 + 文字の幅 (名前が空なら文字の幅だけ)、文字付きの n ≥ 2 のまとまりは文字の幅。
- 幅の項 = 余白(左) + 余白(右) + 文字(左) + 文字(右) + 5。
- `caption_inset(上・下)` = その辺がまとまりの文字を書くなら 10.0、それ以外は今の決まり。

5 mm の引き出し線は箱の外なので、箱の大きさは変えない (ピンの点を含むシンボルの範囲は、その辺で 2.5 mm 広がる。それで正しい)。指定寸法の下限 (`min_size`) は変えない。

#### 2.5 重ならない決まり

測り方: 文字は `tidy.rs:210-213` の箱 `at ± (w/2, h/2)`、絵はその図形の縦横にそろった外接の四角、引き出し線はその線分。辺が接するのは良い。面積のある重なりを重なりとする。1 つの辺の中と、左右の辺の間で次を守る: (a) 文字 (番号・名前・まとまりの文字) どうしが重ならない、(b) 文字が絵や引き出し線に重ならない、(c) コネクタのあるピンの番号とまとまりの文字は箱の外形をまたがない (2.5 mm より広い普通のピンの番号は、今もまたいでいて、それは変わらない)。左右はどんな番号でも成り立ち、上下は 4.0 mm (半角 3 文字) までの番号で成り立つ (上下の今の番号も 5 mm を超えると隣とぶつかる)。上下のまとまりの文字どうしは、それぞれが自分の外形 ((n − 1) × 5 + 4) より広くなければ重ならない。上下の辺と左右の辺が出会う角は今のまま (名前はもうそこでぶつかりうる)。手で最小の `block_size` を指定し、1 つの角の両側の辺にコネクタがあると、角で外形どうしが触れることがある。どちらも受け入れる。

「接する」場合が 1 つある: コネクタのピンの 5 mm 下にある普通のピンの番号は、文字の箱が外形の箱に接する。文字はベースラインに乗るので、用紙の上では外形から 0.4 mm ほど離れて見える。

### 3. 描く道筋

- **`madake-core/src/connector.rs`** (新規、`pub mod connector`): `ConnectorKind`、定数、`connector_groups`、`group_label`、`connector_primitives(kind, dir: PinDir, edges: &[Point]) -> Vec<Primitive>` (§2.1。まとまり 1 つか 1 本のピンについて、ピンのふちの点から作る。引き出し線と文字は含まない)、`outline_segments(pins: &BlockPins, hw, hh) -> Vec<Primitive>` (§2.3)。
- **`block.rs`**: `BlockPin` の欄、`check_connectors`、`block_box` (§2.4)、`block_symbol_from_spec`: コネクタが 1 つでもあれば §2.3 の外形の線。そのあと辺ごと・表の順のピンごとに、ピンがまとまりの始まりなら、そのまとまりの絵の図形、続けて文字の `Text`。そのあとそのピンの引き出し線・番号・名前 (今と同じ。コネクタのピンは §2.2 の位置)。`BlockSpec::has_connectors()`。
- **`src/canvas/connector.ts`** (新規) と **`dynamicSymbol.ts`** (`BlockPin`、`parseBlockSpec`、`blockBox`、`blockSymbol`): 同じことを同じ順で。
- **描画のコードは変えない。** SVG・PDF・PNG・`get_sheet_image`、DXF、KiCad、キャンバスは、今のコードで新しい図形を描く。受け入れる違い (どれも今の動き): DXF は塗りの円を円の中の正方形の `SOLID` で描く (`dxf.rs:420-436`)、キャンバスは塗りの図形を線無しで塗る (SVG より片側 0.15 mm 小さい)。

### 4. ファイル形式 19

- `FORMAT_VERSION` を 19 にし、`model.rs` の履歴に 1 行足す。形式 17・18 のビルドは番号で断る。コネクタの無いファイルは、版番号以外これまでと 1 バイトも変わらない (`block_pins` の文字は書かれたまま保存され、ほかに変わるものは無い)。
- `is_port_block` (`model.rs:613`) を、接点**かコネクタ**が 1 つでもあるブロックで真にする。コネクタのあるブロックもファイル専用の種類 `port_block` で書かれ、`v0.4.0` (新しい形式を断る仕組みが無く、`BlockPin` が `deny_unknown_fields`) はブロックを黙って消さずにファイルを断る。形式 18 のファイルと合わせるため、種類の名前は変えない。IPC・MCP は `symbol` のまま。
- 古いファイル (コネクタ無し) はそのまま開く。

### 5. 画面

- **`BlockPinColumn.vue`**: 1 行の「接点」の後ろに 2 つのマスを足す: **コネクタ** (`<select>`: 空の「なし」と、表の順の 13 種類。表示名は i18n) と **まとめ名** (`connector_ref`、文字、例の表示 `例: J3, TB1`)。まとめ名は、選んだ種類がまとめる種類のときだけ入力できる。見出しの行に 2 つの見出しを足す。ダイアログは収まるように広げる (CSS だけ)。
- **`blockEditor.ts`**: `BlockPinRow` に `connector?: ConnectorKind | ""` と `connectorRef?: string` を足す。`isFilled` はコネクタやまとめ名があるのも数える。`pinsOf` はコネクタがあれば `connector` を、まとめ名の前後の空白を落として空でなければ `connector_ref` を書く (キーは `contacts` の後)。`draftFromAttrs` はそれを読み戻すので、属性 → 下書き → 属性 が元に戻る。ストアの `updateRow` は、コネクタが「なし」かまとめない種類に変わったら `connectorRef` を消す。`draftIssues` に `connectorRefNotGrouped { side, index }` と `connectorRefSplit { ref }` を足す (§1 の 3・4 と 5。空のまとめ名は書かないので 2 は起きない)。
- **言語** (6 つすべて。キーのそろいは `i18n.test.ts` が調べる): `blockEditor.colConnector`、`blockEditor.colConnectorRef`、`blockEditor.connectorRefPlaceholder`、`blockEditor.connectorNone`、`blockEditor.connector.<種類>` × 13、`blockEditor.issue.connectorRefNotGrouped`、`blockEditor.issue.connectorRefSplit`。英語 / 日本語の表示名: USB-A socket / USB-A メス、USB-A plug / USB-A オス、USB-C socket / USB-C メス、USB-C plug / USB-C オス、micro-B socket / micro-B メス、micro-B plug / micro-B オス、3.5 mm jack / 3.5mm ジャック、3.5 mm plug / 3.5mm プラグ、DC jack / DC ジャック、DC plug / DC プラグ、Screw terminals / 端子台 (ネジ)、JST PH socket / JST PH メス、JST XH socket / JST XH メス。列の名前は Connector / コネクタ、Group name / まとめ名。zh・fr・de・es は同じものを訳す。
- **プロパティパネル**: 変えない (まとめ表示はピン数とポート数のまま。絵は図面そのものに出る)。
- エディタのプレビューは `blockSymbol` を使うので、何もしなくても絵が出る。

### 6. MCP・エージェント・文書

- `place_symbol`: attrs の説明 (`lib.rs:475-480`) とツールの説明 (`lib.rs:1242`) に足す:「ピンに `connector` (`usb_a_f usb_a_m usb_c_f usb_c_m micro_b_f micro_b_m jack35_f jack35_m dc_f dc_m screw jst_ph_f jst_xh_f` のどれか。`_f` = メス、`_m` = オス) を書くと、そのコネクタを箱のふちに描く。続けて並んだ `screw` / `jst_*` のピンで `connector_ref` (例 `J3`) が同じものは 1 つの外形になる。コネクタのピンの点は箱から 5 mm なので、ピンの位置は `get_pins` で読む」。`get_pins`・`connect_pins`・ネットリストのツールは変えない。
- `knowledge.rs`: ポートの決まりの隣に 1 項目:「ブロックのピンが実物のコネクタなら `connector` の種類を付ける (USB・3.5mm・DC: 機器側の差し口は `_f` メス、`_m` はプラグ)。端子台と JST の差し口は、続けて並んだピンに `screw` / `jst_ph_f` / `jst_xh_f` と 1 つの `connector_ref` を付ける。`connector` は絵だけなので、導体は `contacts` で書く。コネクタを付けたらピンの位置は `get_pins` で読み直す」。
- 文書 (英語・日本語): `docs/03-schematic-editor(.ja).md` (ブロックの項: コネクタの列、種類、まとまり)、`docs/10-automation-api(.ja).md` (`block_pins` の `connector` / `connector_ref`)、`docs/05-wire-management(.ja).md` (ポートのピンの段落: コネクタは接点と関係しない、形式 19、コネクタのあるブロックも `port_block` で保存)、`docs/internal/feature-inventory.md`。`docs/13-specification(.ja).md` は `python3 scripts/gen_spec.py` で作り直す。

## テスト

Rust のテストは `#[test]` の上に説明を 2 行 (英語、次に日本語)。Vitest のテストは `it(...)` の上に `// ja:` の行。`docs/13-specification*.md` は `python3 scripts/gen_spec.py` で作り直す。

- **双子の正解ファイル** `madake-core/tests/golden/connector-blocks.json`: 13 種類 × 4 辺について、そのコネクタを普通のピン 2 本ではさんだブロック (まとめる種類は 2 本と名前付き) の `primitives` と `pins`、それに AOGU のハブのブロック (「目的」)。Rust のテストは `block_symbol` の出力 (座標は 1e-6 に丸める) をこのファイルと比べ、`MADAKE_UPDATE_GOLDEN=1` のときは書き直す。`dynamicSymbol.test.ts` は同じファイルを読み込み、`blockSymbol` の出力を同じように比べる。これが全種類・全辺の Rust ↔ TS の一致の確認。
- `connector.rs`: 左の辺での各種類の図形が §2.1 とちょうど同じ (手で書いた期待値。正解ファイルが黙ってずれないように)。座標の回転 (点 (1, 0.5) が 4 つの辺で決まった用紙の点に行く、弧は回る量を保つ)。どの絵も u ∈ [−1.5, 2.5]、v ∈ [v₀ − 2, v₁ + 2] に収まる。
- `block.rs` の検査: 知らない種類を断る。空・コネクタ無しのまとめ名・`usb_a_f` のまとめ名・分かれたまとめ名 (2 つの辺、途切れ、違う種類) を §1 の文で断る。名前無しは何度でも良い。`connector` と `contacts` の両方を持てる。
- `block.rs` のまとめ方: 同じ種類 + 同じ名前はまとまる。無しと無しは同じ。名前が違う・種類が違う・間にほかのピンがあると分かれる。違う辺はまとまらない。
- `block.rs` の形: コネクタのピンの点は 5 mm 外、引き出し線は 2.5 → 5.0、番号の内側の端は 2.75 mm 外 (左右) か線の右 (上下)、左右の名前は 3.0 mm 中から。同じブロックの普通のピンは変わらない。外形は絵のところだけすき間があき、コネクタが無ければ `Rect` 1 つ。まとまりの文字 (`J3 PH`、`PH`、`TB1`、名前の無いネジは無し) が n = 1・2・3 と上の辺で §2.2 の位置にある。
- `block.rs` の大きさ: 今のテストのピン表では `block_box` は変わらない。コネクタのある辺ごとに幅が 1.5 増え、名前より広い文字でも増える。上の辺に文字があると `caption_inset` は 10.0。
- **重ならない** (`block.rs` と双子): 左右に全種類 (普通のピン、1〜4 文字の番号、名前と混ぜる)、上に `usb_a_f` が 4 つのハブ、1 本と 3 本の JST、ネジの端子台を持つブロックで、文字の箱どうし、文字の箱とほかのピンの絵・引き出し線・外形の区切りが重ならない (§2.5)。
- 描画: `usb_a_m` のブロックの SVG に黒で塗った多角形がある、DXF に `SOLID` がある、KiCad に `(fill (type outline))` がある。コネクタの無いブロックの出力は変わらない (今の正解ファイルとテストがそのまま通る)。
- `io.rs` / `model.rs`: 形式 19。コネクタだけ (接点無し) のブロックは `port_block` で保存され、読み直すとコネクタ付きに戻る。コネクタの無いファイルは版番号以外変わらない。形式 18 のファイルはそのまま開く。IPC の JSON は `kind: "symbol"` のまま。
- `command.rs`: コネクタの悪いピン表の `add_entity` / `update_entity` を断る。
- MCP (`drawing_tools.rs`): `connector: usb_a_f` の `place_symbol` はピンを箱から 5 mm に返す。知らない種類と `usb_a_f` のまとめ名を §1 の文で断る。そのブロックの `get_sheet_image` が成功する。
- エージェント (`madake-agent/tests/knowledge.rs`): 指示文に `connector` と `connector_ref` が出てくる。
- Vitest の `blockEditor.test.ts`: コネクタとまとめ名が往復する (属性 → 下書き → 属性 が同じ)。種類がまとめない種類に変わるとまとめ名が消える。`connectorRefNotGrouped`、`connectorRefSplit`。コネクタだけの行は埋まった行として数える (そして `emptyNo` を出す)。`dynamicSymbol.test.ts`: Rust と同じ検査、正解ファイルとの一致。`i18n.test.ts`: キーのそろい (今あるテスト)。

**レビューで見てほしいところの候補:** §2.5 の重ならない決まりとテストの測り方。外形のすき間で SVG とキャンバスのメスが白く見えるか。座標と弧の角度の移し方 (弧の双子の一致)。形式 19 と `port_block` の広げ方。コネクタの無いブロックの図形・SVG・ファイルが 1 バイトも変わらないこと。

## 進め方

`feature/connector-pictograms` から `master` への PR 1 本 (ユーザーの「1 つの変更に PR 1 本」の決まり)。コミットは分ける:

1. コア: `connector.rs`、`BlockPin` の欄・検査・まとめ方、形と外形のすき間、`block_box`、形式 19 と `port_block` の広げ方、正解ファイル、Rust のテスト。
2. TS の双子: `connector.ts`、`dynamicSymbol.ts`、正解ファイルとの一致と検査のテスト。
3. 画面: エディタの列、ストア、指摘、6 つの言語。
4. MCP の説明、`knowledge.rs`、文書 (英語・日本語)、仕様書の作り直し。

**やらないこと:** ライブラリの記号 (かみ合わせの `connector_plug` / `connector_socket` は今のまま)、`MadakeCAD.pen` のデザインファイル、色、種類から `contacts` を自動で埋めること、種類の電気的な意味 (プラグとソケットの食い違いの ERC、ネットリストや BOM の変更は無し)、ブロック以外の記号への絵。

**後でやること (この PR ではない):** AOGU の図面 (ハブ 39、ドングル、DCDC、バッテリー、33、44、49、51、53、X58) にコネクタを付け、動いたピンの点へ配線を付け直す。`drawing-madakecad-wiring` スキル (このリポジトリの外にある) にコネクタの決まりを足す。

## 決まっていないこと

無し。下の作者の判断はどれも、ユーザーが違う考えのときにかかる手間を書いてある。

## 決定の記録

| 日付 | 決定 | 理由 |
|---|---|---|
| 2026-10-25 | ユーザーの決定: ブロックのピンは、実物のコネクタの種類をブロックのふちに白黒の小さな絵で描ける | 図面が実物のように読めるように (ハブ、アンプの端子台、基板 33・44 の JST)。 |
| 2026-10-25 | ユーザーの決定: B の描き方。コネクタの外形が箱のふちをまたぐ。端子台と JST は続けて並んだピンをまとめて 1 つの外形で描く | モック A / B / C (style.html、kinds.html) から選んだ。 |
| 2026-10-25 | ユーザーの決定: メス = 白抜きの口がふちに入る、オス = 黒く塗った差し込み部が外へ出る | どの種類も同じ決まり (kinds.html、kinds-dc.html)。 |
| 2026-10-25 | ユーザーの決定: 決まった 13 種類、serde は snake_case、自由入力は無し (`usb_a_f` 〜 `jst_xh_f`) | AOGU の図面のコネクタを全部まかなう (機器ごとに使いどころを確認済み)。 |
| 2026-10-25 | ユーザーの決定: 省略できる `connector` と `connector_ref`。名前は `screw`・`jst_ph_f`・`jst_xh_f` だけで、ほかは断る | 自分の名前を持つのはまとめるコネクタだけ。 |
| 2026-10-25 | ユーザーの決定: まとまり = 1 つの辺で続けて並ぶ、種類と名前が同じピン (無しと無しは同じ)。文字は `<ref> PH/XH`、ネジは `<ref>`、名前無しは `PH`/`XH` だけ、名前の無いネジは何も書かない | 承認された 1 と 3。 |
| 2026-10-25 | ユーザーの決定: `connector` は絵だけ。`contacts` ともネットとも関係しない | USB の差し口は絵と 4 つの接点の両方を持つ。 |
| 2026-10-25 | ユーザーの決定: コネクタの無いピンは今と全く同じに描く | 今の図面をそのまま保つ。 |
| 2026-10-25 | ユーザーの決定: 形式 19。コネクタの無いファイルは版番号以外 1 バイトも変わらない | 古いビルドは知らないキーのブロックを黙って消す。はっきり断らせる。 |
| 2026-10-25 | ユーザーの決定: ピッチ 5 mm。外形はふちに沿って 4 mm、中に 1.5 mm・外に 2.5 mm。引き出し線 5 mm で、外形の先の 2.5 mm に番号。名前は中に入る深さぶん内へ。`block_box` はそれを含める | 承認された 2 (layout.html、dc-hub.html)。 |
| 2026-10-25 | ユーザーの決定: コネクタを付けるとそのピンの点は 2.5 mm 外へ動く。配線は手で付け直す (AOGU はとりまとめ役がする) | 長い引き出し線の手間として受け入れた。 |
| 2026-10-25 | ユーザーの決定: Rust のモジュール `connector.rs` 1 つと TS の双子 1 つ、双子のテスト。今ある図形だけ、色無し。描画はすべてそのまま付いてくる | 承認された 3。 |
| 2026-10-25 | ユーザーの決定: ブロックのエディタに「コネクタ」の選択と「まとめ名」の列、6 つの言語 | 承認された 3。 |
| 2026-10-25 | ユーザーの決定: MCP の説明とエージェントの決まりに種類と使い方を書く。文書は英語・日本語 | 承認された 3。 |
| 2026-10-25 | ユーザーの決定: やらないこと: ライブラリの記号、MadakeCAD.pen、色。AOGU の図面はマージの後 | 承認された 3。 |
| 2026-10-25 | ユーザーの決定: ハブの並び: 長い辺に USB-A メス 4 つ (U1〜U4)、前の短い辺に micro-B メスの電源、3 位置スイッチは図面の文字だけ、本体から出ている OTG ケーブルは右の辺の `micro_b_m` のピン | ユーザーのハブの写真から。 |
| 2026-10-25 | ユーザーの決定: PR 1 本。英語が正本の文書 + `.ja.md`。決定はこの表に書く | ユーザーの進め方の決まり。 |
| 2026-10-25 | 作者の判断: 文 (中に 1.5 mm・外に 2.5 mm、外形の先の引き出し線に番号) を、中 2.5 mm・外 1.5 mm で描いている layout.html / dc-hub.html より優先する | decisions.md が数をはっきり書いている。違ったときの手間: `CONNECTOR_IN_MM` と `CONNECTOR_OUT_MM` を入れ替え、細部を動かし、正解ファイルを作り直す。 |
| 2026-10-25 | 作者の判断: コネクタが 1 つでもあれば、箱の外形を絵のところにすき間をあけた線で描く | 白の塗りが無いので、すき間が無いとふちの線がメスの口を横切る。違ったときの手間: ブロックの `primitives[0]` を読むコードは見つからなかった。戻すなら `Rect` 1 つに戻すだけ。 |
| 2026-10-25 | 作者の判断: オスは、箱の中の塗り無しの部分 (u −1.5 〜 0) と、外の塗った差し込み部 (0 〜 2.5) | どの絵も承認された 4 × 4 mm に収まる。違ったときの手間: オス 5 種類を描き直す。 |
| 2026-10-25 | 作者の判断: `dc_m` は黒い筒に白い穴ではなく中空の筒 (塗った壁 2 枚)。USB-C の口は弧で描く | 白の塗りが無い。違ったときの手間: 1 種類を描き直す。 |
| 2026-10-25 | 作者の判断: `micro_b_f` の外形は箱の中はまっすぐ、外は台形 | ほかの種類と同じくふちと ±2 mm で交わるので、外形のすき間が同じ決まりで済む。違ったときの手間: 全部台形なら種類ごとのすき間の幅が要る。 |
| 2026-10-25 | 作者の判断: ローカル座標は右の辺の向きを回したもの (上下の辺では斜線「/」が「\」になる) | どの辺も同じ移し方で、弧の鏡写しが要らない。違ったときの手間: 2 つの辺に鏡写しを足す。 |
| 2026-10-25 | 作者の判断: 番号の内側の端は外形の 0.25 mm 外 (左右は線の上)、上下は線の右 | 長い番号でも自分の外形にかからない。違ったときの手間: 定数だけ動かす。 |
| 2026-10-25 | 作者の判断: 上下の辺のコネクタのピンの名前は、中 4.5 mm のまま | もともと 1.5 mm の外形から離れていて、動かすと普通のピンとの列がずれる。違ったときの手間: ずらし 1 つ。 |
| 2026-10-25 | 作者の判断: まとまりの文字は、空いている行 (n ≥ 2)、名前の後ろ (n = 1)、上下はふちから 7.0 mm 中で `caption_inset` 10.0。箱の幅は文字を数える | 5 mm ピッチで番号と名前から空いているのはそこだけ。違ったときの手間: 文字を動かし、重ならないテストはそのまま。 |
| 2026-10-25 | 作者の判断: 検査の文は `check_contacts` と同じく英語 | 指示は「check_contacts と同じ書き方の日本語」だったが、check_contacts は英語。エディタは訳した指摘を出す。違ったときの手間: Rust と TS の 5 つの文を訳す。 |
| 2026-10-25 | 作者の判断: 空のまとめ名は断る。1 つのまとめ名で 2 つのまとまりは作れない。名前無しは何度でも良い | 接点と同じ厳しさ。`J3` が分かれているのはたぶん間違い。違ったときの手間: 分かれたコネクタを 2 つの外形で描く必要があれば 5 を外す。 |
| 2026-10-25 | 作者の判断: コネクタのあるブロックも `port_block` で保存する | 形式 19 で止まるのは形式 17・18 のビルドだけ。そうしないと `v0.4.0` がブロックを黙って消す。違ったときの手間: 条件 1 つ。 |
| 2026-10-25 | 作者の判断: `PinDef` と `get_pins` は種類を持たない。プロパティのまとめ表示も変えない | 絵だけのデータは `block_pins` に置く。違ったときの手間: 形式を変えずに後から欄を足せる。 |
| 2026-10-25 | 作者の判断: 共有の正解 JSON ファイルで、全種類・全辺の Rust ↔ TS の一致を確かめる | 13 × 4 の期待値を手で写すとずれる。違ったときの手間: 無し。左の辺の手書きのテストも残す。 |
| 2026-10-25 | 作者の判断: 手で最小の `block_size` にした角の両側のコネクタ、広い上下のまとまりの文字、4 mm を超える上下の番号は、重ならないことを保証しない | 名前の今の限界と同じ。守るには `min_size` を大きくすることになる。違ったときの手間: `min_size` に余白を足す。 |
| 2026-10-25 | 作者の判断: DXF の塗りの円が正方形になること、キャンバスの塗りが線無しなことは受け入れる | 今の描画の動きで、どの記号でも同じ。違ったときの手間: この機能の外の描画の変更。 |
