# 多接点ポート: 1本のピンで複数の導体を運ぶ

日付: 2026-10-25 · 状態: 設計書は利用者が承認済み、計画に進める · English: [2026-10-25-port-contacts-design.md](2026-10-25-port-contacts-design.md)

## 目的

ブロックのピンを**ポート**として使えるようにする。ポートは図面上は1本のピン (ハブの USB-A ソケット、ドングルの USB-A プラグ) だが、名前の付いた複数の導体 (**接点**。例: `VBUS`・`D−`・`D+`・`GND`) を運ぶ。図面はピン1本のまま (必要なら線も1本のまま) にし、ネットリスト・ERC・クロスリファレンス・エージェントには接点ごとに1ネットとして見せる。

AOGU の図面でのできあがりの目安:

- ハブのブロック `39` にピン `U3` (名前 `USB-A メス`、接点 `VBUS, D−, D+, GND`) があり、LTE ドングルのブロックに同じ接点を持つピン (名前 `USB-A オス`) がある。2本のピンを普通の配線1本で結ぶ。
- `get_netlist` は1ネットではなく4ネット (`N003.VBUS`・`N003.D−`…) を返し、どのネットにも `39:U3/VBUS` とドングル側の同じ接点のピンが載る。
- LTE ハーネス 45/46 を `connect_cable` で「基板側 (JST XH) の4本が1本線にまとまり、**ポートのピン1本** `39.U3` で終わる」形に描ける。芯はそれぞれ同じ名前の接点に着く。
- 接点を持たない図面のネットリスト・帳票・ファイルは、形式番号を除いて今までと全く同じ。

## 今の困りごと

ピン1本はネット1つ。`extract_netlist` (`madake-core/src/netlist.rs`) は、シンボルのピンを乗っている配線のまとまりに入れる。`NetPin` が持つのは `reference`・`entity_id`・`pin` だけ。このため USB ポートを1本のピンで正しく描く方法が無い:

- ピン1本と、ドングルへの配線1本で描くと、ネットリストは**1つ**のネットになる。VBUS・D−・D+・GND の区別が無く、その先 (電線リスト・クロスリファレンス・今後のハーネス検査) はハブからドングルまで D+ を辿れない。
- 区間ケーブル (前回の機能) をそのピンに着けることはできない。`connect_cable` は芯1本にピン1本を求め (`plan_side`: 「ピンと芯は同じ数にする」)、`validate_run_cable` は1つのピンに芯2本を断る。仮に通しても、4本の芯が1つのピンのノードに繋がって短絡する。
- 逃げ道は、ポートをピン4本 (`U3.VBUS`・`U3.D−`…) で描くこと。ポートが何口もあるハブではピン数が4倍になり、ブロックが縦に大きく伸びる。利用者 (と AOGU のホワイトボード) の USB ポートの捉え方とも違う。

## 今の作り (コードで確認済み)

- **ブロック** (`madake-core/src/block.rs`): `symbol_id = "block"`。形は `attrs["block_pins"]` の JSON **文字列** (`BlockPins { left, right, top, bottom }`、要素は `BlockPin { no, name }`。どちらの構造体も `#[serde(deny_unknown_fields)]`)。`parse_block_spec` が検証し (番号の空・重複、`BLOCK_PIN_MAX` = 200、外形)、`block_symbol_from_spec` が `BlockPin` ごとに `PinDef` を作る。定義は毎回属性から作り直す (`symbol::instance_symbol_def`、`DefIndex::get`)。属性が読めないブロックは `None` になり、**描かれず、ネットリストにも出ない**。`Engine` は追加・更新のときに同じ `parse_block_spec` を通す (`command.rs` の `validate_entity`)。
- **`PinDef`** (`symbol.rs`): `number`・`name` (`#[serde(default)]`)・`at`・`dir`。`deny_unknown_fields` は無い。構造体リテラルで作っている所が10か所ある。
- **`get_pins`** (MCP) は `pin_wiring::PartPins { entity_id, reference, symbol_id, bbox, pins: Vec<PinInfo { number, name, x, y, dir }> }` を返す。`resolve_pin` はピン指定を最後の `.` で分ける (`"39.U3"` → 部品 `39`・ピン `U3`。番号で探してから名前で探す)。
- **ネットリスト** (`netlist.rs`、`extract_netlist`): ノード `[配線][ピン][ジャンクション][ラベル][ケーブルの芯]` を1つの `DisjointSet` で結ぶ。配線同士は端点かジャンクションで、ピンとラベルは配線のどの点でも繋がる。同名ラベル同士、1つのシンボルの同じ番号の点同士 (貫通端子) も繋がる。区間ケーブルの芯は `CoreEnd` の行き先とだけ繋がる。配線か芯を含むまとまりが `Net` になる。並びは `(wire_ids が空か, 最小の配線id, 最小の (cable_id, core))`、名前はラベル (辞書順最小) > 線番 (`wire_no::is_generated_number` の決まり) > `N001…` の順。
- **かみ合わせは導通しない。** `connector_plug` / `connector_socket` (どちらもピン1本、`symbol.rs`) は `pin_wiring::mate_pair` が同じ参照記号で 5 mm 離して置く。外形は接するが、ピンは外側の端にある。`netlist.rs` には2つを繋ぐ処理が無い。組を知っているのは ERC (`mate.rs` の `mate_diagnostics`。`verify.rs` は参照記号の共有を許すために `connector_plug|connector_socket` を `MATE_HALF` として扱う) と帳票 (`嵌合相手` 列) だけ。つまり今、ハブ → ソケット | プラグ → ドングルの並びは**2つのネット**になる。
- **シートを跨ぐ統合** (`xref.rs`、`extract_netlist_project`): シートごとのネットを、そのネットの配線の上に**位置で**見つかったラベルの名前で統合する (`labels_on_net` が `wire_ids` を調べる)。`project_label_conflicts` (`verify.rs`) は、ラベル名を2つ以上持つ統合ネットを報告する。
- **`extract_netlist` の利用者** (テスト以外): ERC の `verify::erc` (未接続ピン・ラベル競合)、`verify::electrical` (`(部品, ピン)` → ネット番号**1つ**の表を作る)、`cable::check_cables` (芯の短絡)、`sim.rs` (`wire_ids` 経由のネット電圧)、`xref.rs`、MCP の `get_netlist`、Link API の `/netlist`、Tauri の `get_netlist`、エージェントの文脈 (`agent.rs`、ネット数)。配線を単位にする利用者として、`wire_no::numbering_nets` (ネットに番号を1つ振り、その `wire_ids` に書く)、`svg.rs` の `render_wire_numbers`・`dxf.rs`・`kicad.rs` (線番を KiCad のラベルにする) の線番の描画、`tidy.rs` (線番の文字枠)、`reports::wire_list_rows` (ラベル先の解決、G18)。KiCad の*ネットリスト*出力は無い。KiCad 出力は回路図 (`export_kicad_sch`)。SPICE (`spice.rs`) はネットリストを使わず、ノードは座標の一致で作り、配線1本ごとに抵抗素子にする。
- **区間ケーブル** (`model.rs`): `CoreEnd::Pin { part, pin, index }` | `CoreEnd::Wire { wire }`。`cable_run::plan_cable_run` / `plan_side` は芯をピン順に対応させる。`single` は着いたピンをまたぐ横棒を描く (`cable::bar_ends`)。`cable::validate_run_cable` は同じピンの鍵 `(part, pin, index)` を2本の芯で使うことを断る。
- **ファイル形式**は17 (`model.rs` の `FORMAT_VERSION`)。`io::load_project` は新しいファイルを断る (`check_format_version`)。エンティティは serde の代理型 `EntityRepr` (`kind` タグ) を通り、これは **IPC・MCP・Link API の形でもある**。`run_cable` は `EntityRepr` にも、フロントエンドの型 (`src/ipc.ts`、`src/canvas/cable.ts`) にもある。リリースのタグは `v0.4.0` まで (形式16以下。新しいファイルを断る処理は無い)。
- **古いビルドと `contacts` キー。** `BlockPin` は `deny_unknown_fields` なので (`v0.4.0` でも同じ)、古いビルドは `block_pins` に `contacts` があるブロックを読めない。そのブロックは**図面からもネットリストからも黙って消え** (繋いであった相手から見ると未接続になる)、属性の文字列はそのまま残って保存し直される。そのブロックの編集は `Engine` の検証で断られる。危ないのは「接点が黙って捨てられる」ことではなく「機器が丸ごと黙って消える」ことで、どちらにしてもネットリストが黙って間違う。
- **画面**: ブロックのピン表は `BlockEditorDialog.vue` で編集する (プロパティ画面から開く。`PropertiesPanel.vue` はピン数の要約と編集ボタンを出す)。辺ごとに `BlockPinColumn.vue` が1つあり、入力欄は `no` と `name`。TS 側は Rust の写し: `src/canvas/dynamicSymbol.ts` (`BlockPin { no, name? }`・`parseBlockSpec`・`blockSymbol`) と `src/canvas/blockEditor.ts` (`BlockPinRow`・`pinsOf`・`attrsFromDraft`・`draftFromAttrs`・`draftIssues`)。TS の `PinDef` は `src/ipc.ts`。フロントエンドは多くの所で `kind === "symbol"` を見ており、FreeCAD アドオン (`freecad-addon/madakecad_link/linking.py`) も `kind == "symbol"` でエンティティを選ぶ。
- **エージェントの指示** (`madake-agent/src/knowledge.rs`) には、「USB プラグ側を1本線、基板側を4本」のケーブルは `connect_cable` で引くよう既に書いてある。

先行例 (束線の分岐の設計書で確認済み): KiCad のグループバスはメンバを `USB1.DP`・`USB1.DM` と名付ける。この設計も接点のネットに同じ `<元の名前>.<メンバ>` の形を使う。

## 設計

### 1. データモデル

**ブロックのピンの JSON** (`block.rs`):

```rust
pub struct BlockPin {
    pub no: String,
    #[serde(default)]
    pub name: String,
    /// ポートの接点 (この1本のピンが運ぶ導体) を順に。空 = 普通のピン。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<String>,
}
```

```json
{"right":[{"no":"U3","name":"USB-A メス","contacts":["VBUS","D−","D+","GND"]}]}
```

`parse_block_spec` に足す検証: 接点は前後の空白を落として空でないこと、1本のピンの中で重ならないこと、接点を持つピンは**2つ以上**持つこと (導体1本なら普通のピン)、1本あたり `BLOCK_CONTACT_MAX` = 32 まで。別のピンと同じ接点名は使ってよい (USB ポートはどれも `VBUS` を持つ)。文字の制限は設けない。ネットは接点名を文字列の完全一致でだけ比べ、分解はしない。

**`PinDef`** (`symbol.rs`) に `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub contacts: Vec<String>` を足す。`block_symbol_from_spec` は空白を落とした接点を写す。ほかのシンボル (同梱・動的・ユーザーライブラリ) は空のまま。構造体リテラル10か所には `contacts: Vec::new()` を足す。ユーザーシンボルのファイルは変わらない (空なら書かない)。補助: `PinDef::is_port()` = `!contacts.is_empty()`。

ポートのピンは**ほかのブロックのピンと全く同じに描く** (番号は外、名前は内)。接点は描かない。

**`PinInfo`** (`pin_wiring.rs`、`get_pins`) に `contacts` を足す (空なら出さない)。

**`CoreEnd::Pin`** (`model.rs`) に `#[serde(default, skip_serializing_if = "Option::is_none")] contact: Option<String>` を足す。この芯がポートのピンのどの接点かを表す。普通のピンでは `None`。

**`NetPin`** (`netlist.rs`) に `#[serde(default, skip_serializing_if = "Option::is_none")] contact: Option<String>` を足す。ピンを文字で出す所 (`xref_table_rows`、`madake-cli` の `format::netlist`) では `39:U3/VBUS` と書く。

**`Net`** に2つのフィールドを足す。どちらも空なら書かないので、ポートの無い図面の JSON は1バイトも変わらない:

- `contact: Option<String>`: このネットがどの接点か (`"VBUS"`)。ポートのまとまりから分けたネットにだけ付く。
- `contact_labels: Vec<String>`: このネットがシートを跨いで呼ばれる名前 (2.6 節)。分けたネットにだけ入る。空なら `xref` は今までどおりシートの上でラベルを探す。

### 2. ネットリストの決まり (`netlist.rs`)

シートごとの用語:

- **ポートのピン**: `PinDef.contacts` が空でないシンボルの接続点。
- **素通しのピン**: かみ合わせの片側のピン。`symbol_id` が `connector_plug` か `connector_socket`。新しい `mate::is_mate_half(symbol_id)` で判定し、`verify.rs` の `MATE_HALF` の例外 (今はその場の `matches!`) もこれを使う。コネクタ一族全体 (`mate::is_connector_instance` = `connector_*`) にはしない。`connector_4p` のピンは導体1本 (JST) なので、素通しにすると USB と JST の短絡が隠れる。
- **普通のピン**: それ以外のピン。
- **素通しの要素**: 配線・ジャンクション・ネットラベル・素通しのピン。まとまりの全部の接点を運ぶ。

#### 2.1 手順

1. **かたまり。** 今の union-find を、**ケーブルの芯のノードを入れずに**回す (配線・ピン・ジャンクション・ラベル・同名ラベル・同じ番号の点)。根1つが*かたまり*1つ。
2. **ポートのかたまり。** ポートのピンを1本以上含むかたまりを、ポートのかたまりと呼ぶ。その**接点の並び**は、ポートのピンの接点を合わせたもので、順序を固定する: ポートのピンを `(参照記号, ピン番号, entity id)` で並べ、各ピンの接点を書いた順に見て、初めて出た名前を足していく。ほかのかたまりは導体1本のかたまり。
3. **枠。** 導体1本のかたまりは枠1つ。ポートのかたまりは接点ごとに枠1つと、**残りの枠**1つ。
4. **中身を枠へ** (ポートのかたまりだけ):
   - ポートのピン → 自分の接点それぞれの枠へ (どの枠でも `contact = Some(c)` の `NetPin`)
   - 素通しのピン・配線・ジャンクション・ラベル → 全部の接点の枠へ (素通しのピンは `contact = Some(c)` で載せる。配線の id は接点のネットそれぞれに重ねて入る)
   - 普通のピン → ピンの `PinDef.name` (空白を落として完全一致) と同じ名前の接点の枠へ。無ければ残りの枠へ入れ、ERC `erc.port_to_plain_pin` を出す。ピン番号は使わない (番号 `1` の JST のピンが、名前 `1` の接点にうっかり繋がらないように)。
5. **芯。** 区間ケーブルの芯ごとに、両端の枠を union する。端の行き先: `Wire { wire }` → その配線のかたまりが導体1本ならその枠 (分岐の配線は芯そのもの)。普通のピンへの `Pin` → そのピンのかたまり。ポートのピンへの `Pin` で `contact = Some(c)` → ポートのかたまりの枠 `c`。ポートのピンへの `Pin` で `contact` が無い → 芯の名前 (`core.core`) と同じ名前の枠、無ければ残りの枠 (ERC、3節)。配線がポートのかたまりに入っている `Wire` の端 → 残りの枠 (ERC `erc.cable_core_unassigned`。分岐の配線はポートのネットに触れてはいけない)。
6. **点付きラベル。** 導体1本のかたまりに、ちょうど `<L>.<c>` という名前のラベルがあり、`<L>` が接点 `<c>` を持つポートのかたまりのラベルなら、その枠と union する。同じシートの上で、導体1本を名前で取り出せる (普通の配線に `USB_MIC.VBUS`)。シートを跨ぐときは同じ名前で `xref` が繋ぐ (2.6 節)。
7. **ネット。** union 後の枠のまとまりのうち、配線か芯を含むものをネットにする (今の決まり)。普通のピンも芯も入っていない残りの枠は出さない (共有の配線は接点のネット全部に既に入っている)。
8. **ポートが無ければ何も変わらない。** ポートのかたまりが1つも無ければ手順2〜6は何もせず、手順1と5は今と全く同じまとまりを作る (union-find は結ぶ順に依らない)。並びも名前も今と同じ。

今の動きは、2つ目の公開関数 `extract_wire_nets(sheet, symbols)` として残す。手順1と5だけを行い分けない (描いたまとまりごとに1ネット)。2つの関数はノードと union の処理を共有する。

#### 2.2 どの利用者がどちらを使うか

- **分ける方 (`extract_netlist`)**、電気的な意味を見るもの: `verify::erc`・`verify::electrical`・`cable::check_cables`・`sim.rs` のネット電圧・`xref.rs`・MCP の `get_netlist`・Link API・Tauri の `get_netlist`・エージェントの文脈。
- **分けない方 (`extract_wire_nets`)**、描いた配線のまとまりごとに1件を見るもの: `wire_no::numbering_nets` (分けると USB の配線1本に番号が4つ振られ、最後のものが残る)、`svg.rs` の `render_wire_numbers`、`dxf.rs` と `kicad.rs` の線番の文字、`tidy.rs` の線番の文字枠、`reports::wire_list_rows` (`Wire` 1本に1行のまま。G18 のラベル先は描いたまとまりで解決する)。
- `(部品, ピン)` でネットを引くコードは、ポートのピンが複数のネットに入ることを受け入れる: `verify::electrical` の `pin_net` は `(部品, ピン) → ネット番号の Vec` にする。`verify::erc` の `connected` は集合なので変えなくてよい。
- SPICE はそのまま。ノードは座標の一致で作るので、ポートのピンはノード1つになり、接点ごとには模擬しない。ポートはブロックの信号・電源の口で、直流の検証は今もそれを扱っていない。書き留めるだけで変えない。

#### 2.3 名前

接点のネットごとに、今と同じ優先順で候補を見る:

1. **ラベル**: ネットに入っているポートのかたまりのラベル `L` それぞれの `<L>.<c>` と、導体1本の部分のラベルそのもの (点付きラベルや、JST 側の `PWR5V` など)。名前は辞書順で最小のもの。`Net.label` もその名前。
2. **線番**: ポートのかたまりの線番 `W` (かたまりの配線に今の選び方を当てたもの) の `<W>.<c>` と、導体1本の部分の線番そのもの。今の決まり (手動の名前を自動採番より先に、その次は辞書順) で選ぶ。`Net.wire_no` は、そのネット自身の配線の生の線番 (全配線に今の決まりを当てたもの) のままにするので、線番を読む側には本物の番号が見える。
3. **自動**: `<Nxxx>.<c>`。ポートのかたまりは `Nxxx` の番号を**1つだけ**使い、下の並びで最初の接点のネットの位置で取る。接点のネットはその番号を共有する。残りのネットの名前は `<元の名前>` (同じ元の名前で、末尾を付けない)。

1つの接点のネットが芯を通して2つのポートのかたまりを含むときは、ネットの並びで先のかたまりの名前を元にする。

**名前の重なり。** 作った `<L>.<c>` と同じ名前のラベルは重なりではない (手順6で繋がる)。手動の線番が作った名前と同じ (`5.VBUS`) ときは繋がず、両方のネットに同じ名前が出る。名前は表示の文字で、ネットは中身で区別されるので受け入れる (検査しない)。`Nxxx.c` は `Nxxx` と重ならない。

#### 2.4 並び (安定性)

並べる鍵: `(wire_ids が空か, 最小の配線id, 最小の (cable_id, core), 接点の順位)`。接点の順位は、ポートのかたまりの接点の並びでの位置 (残りは最後)。分けないネットでは0。ポートが無ければ最後の要素はいつも0なので、並びも `N001…` の番号も今と全く同じ。ポートがあるときは、かたまりの接点のネットが、分けなかったときのネットの位置にまとまって並ぶので、ほかのネットの番号はずれない。

#### 2.5 かみ合わせ

かみ合わせは今も導通しない (「今の作り」)。ハブ → ソケット | プラグ → ドングルは、描いたまとまり2つのまま。それぞれがポートのかたまり (ポートのピン1本と素通しのピン) なので接点ごとに分かれ、かみ合わせの片側は接点のネット全部に載る。両側を接点ごとに繋ぐことはしない。今も全く繋がっていないのと同じ。かみ合わせを導通させるのは別の変更にする (2026-10-25 に決定、決定の記録を参照)。

よくある描き方への利用者の選択 (案1): 2本のポートのピンの間を普通の配線 (か素通しの要素の並び) で結ぶと、**同じ名前の接点同士が繋がる**。1つのかたまりの中の手順2〜4がそれに当たる。

#### 2.6 シートを跨ぐとき (`xref.rs`)

`extract_netlist_project` は、`Net.contact_labels` が空でないときは `labels_on_net` の名前の代わりにそれを使う。シート1の `USB_MIC.VBUS` は、シート2の `USB_MIC.VBUS` (向こうでも分けたネット、または点付きラベルの普通の配線) と統合され、`USB_MIC.D−` とは決して統合されない。`NetMember.label_ids` と `sites` は今までどおり位置で探すので、描いたラベルに付くクロスリファレンスの番地は変わらない。`ProjectNet.label_names` には名前を付け足した形が入るので、`project_label_conflicts` は正しいもの同士を比べる。

あるシートでラベル `L` がポートの無いネットに載り、別のシートでは `L` がポートのラベルである場合、前者の鍵は `L` のままで、どの接点とも繋がない (全部に繋ぐと接点同士が短絡する)。ERC で知らせる (3節)。

### 3. ERC

新しいコードは新しいモジュール `madake-core/src/port.rs` に置く (`check_ports(sheet)`。`verify_sheet_with` から `check_cables` と並べて呼ぶ。シートを跨ぐ検査は `verify_project` から)。

| コード | 重さ | 出るとき |
|---|---|---|
| `erc.port_to_plain_pin` | 警告 | 普通のピンがポートのかたまりに入っていて、名前がどの接点とも一致しない。文は接点を並べる。例: `J45:1 は 39:U3 (VBUS, D−, D+, GND) につながっていますが、どの接点か分かりません。ピン名を接点名にするか、connect_cable で芯ごとに引く`。そのピンは残りのネットに入る。 |
| `erc.port_contact_unmatched` | 情報 | ポートのピンを2本以上含むかたまりで、ある接点が1本にしか無い (例: ハブには `SHIELD` があるがドングルには無い)。ポートのピンが1本だけのかたまり (かみ合わせに繋いだポート) では出さない。 |
| `erc.port_label_unsplit` | 警告 | プロジェクト: ラベル `L` があるシートではポートのネットに、別のシートではポートの無いネットに載っている。それらのシートは繋がらない。文は、そちらにもポートを置くか、ラベル `L.<接点>` を使うよう勧める。 |
| `erc.cable_core_unassigned` (既存) | エラー | 追加: 芯の `contact` がピンの接点に無い (後からブロックを直した)、ポートのピンへの芯に `contact` が無く芯の名前もどの接点でもない、分岐の配線の端がポートのネットに触れている。 |
| `erc.label_conflict` (変更) | エラー | 接点のネットでは、名前を付け足した形 (`USB_MIC.VBUS`) とラベルそのものを比べる。`USB_MIC` と `USB_MIC.VBUS` は同じ名前として扱い、競合にしない。 |
| `erc.unconnected_pin` | (変更なし) | ポートのピンはどれかのネットに載っていれば接続済み。 |

### 4. 区間ケーブルと `connect_cable` (`cable_run.rs`、`cable.rs`)

片側を**ポートのピン1本**に着けられる: `"b": { "pins": ["39.U3"], "style": "single" }` (または `"part": "39", "pins": ["U3"]`)。

- その側のピンがちょうど1本に決まり、そのピンが接点を持つときに当てはまる。`style` は `single` にする (`multi` は断る。ポートは接続点1つ)。
- 対応: 芯の名前が全部そのピンの接点なら名前で (芯は一部だけでもよい。例: 接点9つの USB 3 ポートに芯4本)。芯の名前がどれも接点でなく、数が同じなら順番で (芯 i → 接点 i)。それ以外は接点を並べて断る (一部だけ一致するのは打ち間違いの見込みが高い)。
- 芯には `CoreEnd::Pin { part, pin, index, contact: Some(c) }` が付く。結果の表記は `"39.U3/VBUS"`。
- 形: 1本線はピンの接続点で終わり、ピンの向きに出ていく。片側の `Pin` の端が全部同じ接続点なら**横棒は描かない** (SVG の `render_run_cable` と `src/canvas/cable.ts` は、`bar_ends` が1点を返したら横棒を飛ばす)。`erc.cable_run_end_detached` は今までどおり。
- `validate_run_cable`: ピンの鍵を `(part, pin, index, contact)` にする。同じ接点に芯2本は断り、1本のピンの別々の接点なら認める。接点がブロックにあるかはここでは調べない (束線の分岐の決定どおり形だけ)。古くなった接点は ERC が知らせる。
- 区間ケーブルの芯の電線リストの行: ピンの端は `39:U3/VBUS` と書く。ケーブル一覧の「使っている芯」と部品表は変わらない。

ポートに着けないほうの側は今の決まり (芯1本にピン1本、順番に対応) のまま。

### 5. 電線リストと部品表

変えない。2つのポートの間の普通の配線は `Wire` 1本で、電線リストは1行、電線の集約も1件、線番も1つ (分けない方、2.2 節)。区間ケーブルもケーブルの行1つのまま。

### 6. MCP とエージェント

- `place_symbol`: `block_pins` の説明に `contacts` を足す (`{"no":"U3","name":"USB-A メス","contacts":["VBUS","D−","D+","GND"]}`)。「USB・オーディオ・DC ジャックのポートは、導体ごとのピンではなく接点を持つピン1本で描く」も書く。
- `get_pins`: ピンに `contacts` が出る。
- `connect_cable`: 説明にポートへの着け方 (4節) を足す。
- `get_netlist`: 説明に、ポートは `<名前>.<接点>` のネットに分かれ、ピンに `contact` が付くことがあると書く。
- `connect_pins` は変えない。2本のポートのピンを配線1本で結ぶのが、同じ名前の接点同士を繋ぐふつうのやり方。
- `knowledge.rs`: 配線の決まりに1項目足す: 「複数の導体を運ぶコネクタ (USB・3.5 mm オーディオ・DC ジャック) は**`contacts` を持つブロックのピン1本**にする。2つのポートは `connect_pins` の配線1本で結ぶ (同じ名前の接点同士が繋がる)。ケーブルをポートに着けるときは `connect_cable` で `pins: [\"<参照記号>.<ポートのピン>\"]`。導体1本のピンは接点と同じ名前 (`VBUS`・`GND`) にすればその接点に繋がる」。
- 文書: `docs/10-automation-api(.ja).md` (block_pins・get_pins・connect_cable・get_netlist)、`docs/05-wire-management(.ja).md` (ポートのピン)、`drawing-madakecad-wiring` スキル。

### 7. 画面

- `BlockPinColumn.vue`: 行ごとに3つ目の入力欄 **接点 / Contacts** を足す。区切りはカンマ (`,`・`、`・`，` のどれでもよい)、見本の文字は `VBUS, D−, D+, GND`。空 = 普通のピン。
- `blockEditor.ts`: `BlockPinRow` に `contacts: string` (入力の文字) を足す。`pinsOf` は区切って空白を落とし、空のものを捨て、空でないときだけ `contacts` を書く。`draftFromAttrs` は `, ` でつなぐ。`draftIssues` に `contactsTooFew`・`duplicateContact`・`tooManyContacts` を足す (Rust と同じ決まり)。
- `dynamicSymbol.ts`: `BlockPin.contacts?: string[]`。`parseBlockSpec` に同じ検証を足し、`blockSymbol` は接点を `PinDef` に写す。`src/ipc.ts` に `PinDef.contacts?: string[]`・`NetPin.contact?`・`Net.contact?`・`Net.contact_labels?`・`CoreEnd` のピンの `contact?` を足す。
- `PropertiesPanel.vue`: ブロックの要約に、0でなければポートの数を足す (`ポート 2`)。図面の見た目は変わらない。
- i18n のキーは両方の言語に。MadakeCAD.pen のブロックのピン表の画面にも列を足す。

### 8. ファイル形式と互換性

- `FORMAT_VERSION` は18。`model.rs` の版の履歴に v18 の行を足す。古いファイルはそのまま開く。ポートのピンが無いファイルは、版番号を除いて今までと1バイトも違わずに書かれる。
- **ファイルの上の種類 `port_block`。** `symbol_id == "block"` で、`block_pins` のどれかのピンが接点を持つシンボルは、`"kind": "port_block"` (中身は同じ `SymbolInstance`) で書く。`v0.4.0` 以前はそのファイルを読み込めず (知らない種類のエンティティ)、ブロックをネットリストから黙って落とすことが無い。形式17のビルドは、形式18のファイルを版番号で既に断る。
- **ファイルの中だけで変え、IPC・MCP では変えない。** `EntityRepr` は IPC・MCP・Link API の形でもあり、フロントエンドのあちこちと FreeCAD アドオン (`linking.py`) が `kind == "symbol"` を見ている。フロントエンドに覚えさせた新しい種類 `run_cable` と違い、ポートのブロックはそこでは `symbol` のままにする。serde_json は `preserve_order` 無しで作られているので、`serde_json::Value` の上で `kind` を書き換えると、保存するファイルすべてでキーの並びが変わる。そこで `EntityRepr` に `PortBlock(SymbolInstance)` を足し (JSON スキーマには出さない)、`From<Entity> for EntityRepr` はスレッドごとの印が立っている間だけそれを選ぶ。印を立てるのは `io::save_project` だけ (`FileWriteGuard`)。読むときはどこでも `port_block` を受け取り、`Entity::Symbol` にする。接点の無い `port_block` (手で直したファイル) は普通のシンボルとして開き、`symbol` で書き戻す。区間の無い `run_cable` と同じ。
- ファイルへの書き込みは全部 `io::save_project` を通る (Tauri の `save_project`、MCP と Link API は `agent.rs` 経由)。マクロのファイル (`macros.rs` の `write_macro_to`) は書き換えない。古いビルドは、知らない `contacts` キーで `Engine` の検証が失敗するので、そのマクロの挿入を断る。黙ってではなく、目に見えて失敗する。
- `CoreEnd::Pin.contact` には種類の工夫は要らない。区間ケーブルは既に `v0.4.0` の知らない `run_cable` で、形式18は形式17のビルドを止める。

## テスト

Rust のテストは `#[test]` の上に2行の説明 (英語、次に日本語) を付ける。Vitest のテストは `it(...)` の上に `// ja:` の行を付ける。PR ごとに `python3 scripts/gen_spec.py` で `docs/13-specification*.md` を作り直す (CI で `--check`)。

- `block.rs`: 接点が読めて `PinDef` まで届く。空・重複・1つだけ・多すぎる接点は分かりやすい文で断る。接点の無いピンは変わらない。
- `netlist.rs`:
  - 2本のポートのピンを配線1本で結ぶと、接点ごとに1ネットになり、どれにも両方のピンが `contact` 付きで載り、名前は `Nxxx.<c>`、接点の順に並ぶ
  - ポート → 配線 → ソケット | プラグ → 配線 → ラベル → 配線 → ポート: 描いたまとまり2つで、それぞれ接点ごとに分かれ、かみ合わせの片側は接点のネット全部に載り、かみ合わせを越えては繋がらない
  - ポートのかたまりの中の、名前 `GND` の普通のピンは `.GND` に入る。名前の無い普通のピンは残りのネットに入る
  - LTE ハーネス: JST のピン4本 → 分岐の配線 → 1本線 → ポートのピン `39.U3` で4ネットになり、どれも JST のピン1本と `39:U3/<c>`
  - 普通の配線の上の点付きラベル `USB_MIC.VBUS` は `USB_MIC.VBUS` に入る
  - ポートの無い図面は、`extract_wire_nets` とも今までとも同じ `Vec<Net>` (名前・並び・JSON) になる (`netlist.rs` の既存のテストはそのまま通る)
  - `extract_wire_nets` では USB の配線は1ネットのまま
- `xref.rs`: 分けたネットはシートを跨いで接点ごとに統合され、別の接点とは統合されない。別のシートの点付きラベルはその接点に繋がる。
- `port.rs` / `verify.rs`: `erc.port_to_plain_pin`、`erc.port_contact_unmatched` (ポートのピン1本では出ない)、`erc.port_label_unsplit`、古くなった接点が `erc.cable_core_unassigned` になる、`USB_MIC` と `USB_MIC.VBUS` は `erc.label_conflict` にならない、ポートのピンは未接続にならない。
- `wire_no.rs` / `reports.rs`: USB の配線に線番は1つ。電線リストはそれに1行。区間ケーブルの行は `/接点` を書く。
- `cable_run.rs` / `cable.rs`: ポートへ名前で着く、順番で着く、一部一致は断る、ポートへの `multi` は断る、接点の一部だけでもよい、同じ接点に芯2本は断る、着く点が1つなら横棒を描かない。
- `io.rs`: 形式18。ポートのブロックは `port_block` で保存され、読み直すと接点付きの `Entity::Symbol` に戻る。ポートの無いファイルには `port_block` が無く、版番号以外は変わらない。接点の無い `port_block` はシンボルとして開く。v17 のファイルはそのまま開く。ポートのブロックの IPC・MCP の JSON は `kind: "symbol"`。
- MCP (`madake-mcp/tests`): 接点付きの `place_symbol`、`get_pins` が接点を返す、`connect_cable` が `39.U3` に着く、`get_netlist` が分けたネットを返す。
- Vitest: `blockEditor.test.ts` (接点の文字の往復、区切り、指摘)、`dynamicSymbol.test.ts` (Rust と同じ検証、`PinDef.contacts`)、`cable.test.ts` (着く点が1つなら横棒なし)。

## 進め方

PR はどれも `master` 向け。依存する PR は積み重ねず、「先に #N をマージ」と書く。

1. 中核: `BlockPin` / `PinDef` の接点、ネットの分割と `extract_wire_nets` と利用者の切り替え、`NetPin` / `Net` のフィールド、xref の鍵、`port.rs` の ERC、`port_block` 付きの形式18。単独でマージしても安全 (接点を作れるのはまだ `execute_commands` と `place_symbol` の属性だけ)。
2. 区間ケーブル: `CoreEnd::Pin.contact`、`connect_cable` のポートへの着け方、検証の鍵、横棒なしの描画 (SVG と作図領域)、電線リストの `/接点`。
3. 画面: ブロックエディタの接点の列、プロパティの要約、TS の型、i18n、.pen。
4. MCP の説明、`knowledge.rs`、文書、スキル。その後で AOGU の図面のハブ・ドングルのポートと LTE 45/46 を描き直す。

## 未決事項

無し。かみ合わせの導通 (プラグとソケットの組が2つのネットを繋ぐこと) は、2026-10-25 に別の変更にすると決めた。この設計書では、かみ合わせの片側を自分のネットの中で素通しにするだけにする。

## 決定の記録

| 日付 | 決めたこと | 理由 |
|---|---|---|
| 2026-10-25 | ピンは名前付きの接点を持てる。描くのはピン1本で、ネットは接点ごとに分ける | 利用者は USB ポートをピン1本で描きつつ、導体ごとの接続を保ちたい |
| 2026-10-25 | 利用者の選択 (案A): 今回接点を持てるのはブロックのピンだけ (ライブラリ・ユーザーシンボルは後で) | ポートが出てくるのはブロック (ハブ・ドングル)。`PinDef.contacts` は一般的な形なので、シンボルも形式を変えずに後から続ける |
| 2026-10-25 | 利用者の選択 (案1): 2つのポートの間の普通の配線 (か素通しの要素の並び) は、同じ名前の接点同士を繋ぐ | USB ケーブルを1本線で描くやり方に合う。接点ごとの配線が要らない |
| 2026-10-25 | 分けたネットの名前は `<元の名前>.<接点>`。ポートのかたまりは自動の番号を1つ使う | KiCad のグループバスの名付けに倣う。ほかのネットの名前と並びがずれない |
| 2026-10-25 | 素通しにするのは `connector_plug` / `connector_socket` だけ (`mate::is_mate_half`)。`connector_*` 全部ではない | `connector_{n}p` のピンは導体1本。素通しにすると短絡が隠れる |
| 2026-10-25 | 普通のピンはピンの**名前**だけで接点に入る。番号は使わない | JST のピン `1` と名前 `1` の接点のような、うっかりした接続を避ける |
| 2026-10-25 | 分けない方のネットリストを `extract_wire_nets` として残し、線番の採番・線番の描画・電線リストに使う | 分けたネットは配線を共有する。ネットごとに採番すると配線1本に番号が4つ付く。利用者は電線リストと部品表を1行のままにするよう求めた |
| 2026-10-25 | 分割はケーブルの芯を入れずに作ったかたまりの上で行い、そのあとで芯と点付きラベルが枠を繋ぐ | 分岐の配線は芯1本を運ぶ。芯を入れた後で分けると、その配線がポートのまとまりに入り、多接点に見える |
| 2026-10-25 | シートを跨ぐ統合は `Net.contact_labels` の名前を付け足した形 (`USB_MIC.VBUS`) で行う | 分けたネットは配線を共有するので、位置でラベルを探すと全部の接点がまた1つにまとまる |
| 2026-10-25 | 点付きラベル `L.c` で接点1つを取り出せる (同じシートでも、シートを跨いでも) | シートを跨ぐときの一貫性に要る。導体1本を名前で取り出せる。この設計書が出した案で、2026-10-25 に利用者が承認 |
| 2026-10-25 | ファイル形式18。ポートのブロックはファイルの中だけ `port_block`、IPC・MCP では `symbol` | 利用者は古いビルドにそのファイルを開かせない方を選んだ。古いビルドでは (deny_unknown_fields のため) 接点が捨てられるのではなくブロックが消える。ファイルの中だけの種類にすれば、フロントエンドと FreeCAD アドオンにたくさんある `kind == "symbol"` の判定に手を入れずに済む |
| 2026-10-25 | 接点は1本のピンに2つ以上32まで、ピンの中で重ならない | 導体1本なら普通のピン。上限でエディタとネットを扱える大きさに保つ |
| 2026-10-25 | SPICE は変えない | ノードはネットではなく座標の一致で作る。ポートは今も模擬していない |
| 2026-10-25 | `port_block` は `serde_json::Value` の書き換えではなく、`EntityRepr` の「ファイルへ書いている」印 (スレッドごと) で作る | 計画のレビュー: serde_json に `preserve_order` が無いので、`Value` を経由すると保存するファイルすべてのキーの並びが変わり、「ポートの無いファイルは変わらない」が守れない |
| 2026-10-25 | 今回は設計書の下書きだけで、製品のコードはまだ書かない | 利用者は先に調査と設計を求めた |
| 2026-10-25 | 設計書が出した案を利用者が承認: 点付きラベル `L.c` で接点1つを取り出す、接点は1本のピンに2〜32、ポートへの `connect_cable` で芯の名前が一部だけ一致するときは断る、普通のピンはピンの名前だけで接点に入る | 設計書のレビューへの回答 |
| 2026-10-25 | 利用者の決定: かみ合わせの導通 (プラグのピン n とソケットのピン n) は別の変更にし、今回は入れない | かみ合わせのある既存の図面すべてのネットが変わるため |
