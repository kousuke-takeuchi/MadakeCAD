# 図枠テンプレートの拡張: 余白・マーク・改訂欄・罫線の太さ・回転文字・自由な欄・文字の収め方・ロゴ画像

日付: 2026-11-14 · 状態: 設計はチャットで承認済み、仕様書はレビュー待ち · 英語(正本): [2026-11-14-frame-template-extensions-design.md](2026-11-14-frame-template-extensions-design.md)

## 目的

大判の産業図面の図枠を、実際に描かれているとおりに図枠テンプレートで記述できるようにする。A0参考図面の再現(`2026-11-14-a0-reference-reproduction-design.ja.md`)の領域1を参考図面に一致させるためである。`docs/internal/specs/reproduction-gaps.md`のG24〜G30と、コードを読んで見つけた1件(G31、ロゴ画像)を解消する。

この仕様書は**サブプロジェクトA: データと描画**である。新しい項目をUI(図枠エディタ、表題欄ダイアログ、改訂ダイアログ)で編集する機能はサブプロジェクトBとし、先に`MadakeCAD.pen`でデザインして別に仕様化する。Bはここで決めるデータ形式に依存する。

成功の条件:

- ユーザーの図枠テンプレートで次を指定できる: 辺ごとの余白、ゾーン目盛の非表示、裁断マークと中心マーク、独自の列・見出し・行高・固定の空行を持つ改訂欄、罫線の太さ、回転した文字と枠付きの文字、表題欄の自由な欄、大文字高さでの文字サイズ、セルに収める文字、ロゴ画像。
- SVG書き出し、PDF書き出し、画面のキャンバスが全て同じように描く。
- 新しい項目を1つも使わないテンプレートとプロジェクトは、**これまでと完全に同じ出力**になる(既存のSVG完全一致テストは変更せず、合格のまま)。
- 図枠エディタは、新しい項目を持つテンプレートを読み込んで保存しても、その項目を失わない。

対象外:

- 新しい項目を編集するUI(サブプロジェクトB)。
- 書体と、字幅の係数。
- 既存の数字・英字以外のゾーン目盛の文字種。

## 現状(コードで確認済み)

- `madake-core/src/frame_template.rs`: `FrameTemplate { id, name, name_ja, description, description_ja, paper, frame_margin, zones, title_block, revision_block, logo, builtin }`。`Zones { cols, rows, col_labels, row_labels }`。`TitleBlockDef { origin, w, h, row_h, grid_lines, cells }`。`GridLine { x1, y1, x2, y2 }`。`Cell { x, y, w, h, text, bind, size, weight, align, omit_if_empty }`。`RevisionBlockDef { rows_max, col_w }`で`REVISION_COLUMNS = 4`。`LogoFrame { x, y, w, h, path }`。`validate()`がテンプレートを検査する。`bind_value()`は`{project.*}`、`{sheet.*}`、`{rev.latest}`、`{title_block.*}`と別名4つを解決し、知らないバインドは空文字にする。
- `madake-core/src/svg.rs`: `frame_and_title_block()`は余白1値で外枠を描き、ゾーン目盛を必ず描き(`zone_cols.max(1)`)、罫線を全て`RULE_STROKE`(0.25)、外枠を`FRAME_STROKE`(0.5)で描き、セルを`text_weight_el`で描き、ロゴは空の矩形として描く(「画像の埋め込みは未対応」)。`revision_block()`は固定の4列を組み込みの見出し`REV_HEADERS`で描き、`title_block.row_h`を使い、改訂が無ければ何も描かず、文字を切り詰めも折り返しもしない。
- `madake-core/src/model.rs`: `Revision { mark, date, description, by }`。`TitleBlock`は文字列10欄。`FORMAT_VERSION = 19`。
- `madake-core/src/pdf.rs`はSVGを`svg2pdf`で変換する。SVGが唯一の描画ソース。クレート`image`、`png`、`zune-jpeg`、`base64`は既に`Cargo.lock`にある。
- `madake-core/src/tidy.rs`: `text_width(text, height)`が文字列の幅を見積もる。
- `src/canvas/frameTemplate.ts`がキャンバス側の対(`frameLayout`、`revisionLayout`、`bindValue`、`REV_HEADERS`)。`src/canvas/frameEditor.ts`と`src/stores/frameEditor.ts`がテンプレートを編集する(`REVISION_COLUMNS = 4`がここにも書かれている)。
- コマンド: `set_frame_template`、`set_title_block`、`set_revisions`。Link API: `GET /frame-templates`、`POST /frame-templates/save`。

## 設計

新しい項目は全て省略でき、省略時は今の動作になる。不正な値は`validate()`が項目名を示して拒否する。

### G24 — 辺ごとの余白

`frame_margins: { left, right, top, bottom }`(mm、それぞれ0以上)。指定があれば、外枠、ゾーンの目盛線、`title_block_origin`で`frame_margin`の代わりに使う。`frame_margin`は必須のまま残す(古い読み手が数値を得られるように)。`frame_margins`があるとき`frame_margin`は無視する。

### G25 — ゾーン目盛とマーク

- `zones.show: bool`、既定`true`。`false`なら目盛の文字も目盛線も描かない。クロスリファレンスが使うゾーンの住所はこれまでどおり計算する。
- `marks: { trim: bool, centre: bool }`、どちらも既定`false`。
  - `trim`: 用紙の四隅にL字。用紙の端から4mm内側、腕の長さ15mm、線幅0.5。
  - `centre`: 外枠の各辺の中央に、外枠から外向きの線。長さはその辺の余白と10mmの小さいほう、線幅0.5。

### G26 — 改訂欄

`revision_block`に次を足す:

- `columns: [{ field, header, w, align }]` — 1〜8列。`field`は`mark`、`count`、`description`、`rev_no`、`date`、`by`、`approved`のいずれかで、同じものは1回まで。`header`は見出し行に印字する文字。`w`はmmで、合計は`title_block.w`と一致させる(許容0.01)。`align`は`start`、`middle`、`end`(既定`start`)。`columns`が無ければ、`col_w`と`REV_HEADERS`による今の4列になる。
- `row_h`(mm): 改訂欄の行高。既定は`title_block.row_h`。
- `rows_fixed: n`: 改訂が少なくても(0件でも)、改訂行`n`行と見出し行を罫線付きで必ず描く。新しいほうから`n`件を表示する。既定は無し(行数は改訂の数に従い、`rows_max`で制限し、0件なら何も描かない)。
- `mark_shape: "none" | "triangle"`、既定`none`。`triangle`は記号の文字を、行高に収まる上向きの三角で囲む。

改訂欄のセルの文字は列幅に収め(G30の`fit`)、`\n`で2行目を書ける。

`Revision`に`rev_no`、`approved`、`count`を足す。全て文字列で既定は空。`count`は保存された文字をそのまま印字する(例: `x7`)。

### G27 — 罫線の太さ

`GridLine.weight: "thin" | "medium" | "heavy"`、既定`thin`。線幅は0.25、0.5、1.0mm。

### G28 — 回転した文字と枠付きの文字

- `Cell.rotation: 0 | 90 | 180 | 270`、既定0。文字はアンカー点(`x`, `y`)を中心に、画面上で時計回りに回転する。
- `Cell.boxed: bool`、既定`false`。見積もった文字の箱から0.8mm外側に細い矩形を描く。文字と一緒に回転する。

### G29 — 表題欄の自由な欄

`TitleBlock.fields: map<string, string>`(既定は空。空ならファイルに書かない)。バインド`{field.<名前>}`はその値に、名前が無ければ空文字になる。名前は`a-z`、`0-9`、`_`の1〜32文字。`set_title_block`が他の欄と同じようにこの対応表を運ぶ。

### G30 — 大文字高さのサイズと、収める処理

- テンプレートに`text_size_is: "font" | "cap"`、既定`font`。`cap`のとき、全ての`Cell.size`と改訂欄の文字サイズは大文字の高さで、描くフォントサイズは`size / 0.72`。
- `Cell.fit: bool`、既定`false`。`fit`と`Cell.w`があるとき、最も長い行の`tidy::text_width`が`w`以下になるようフォントサイズを下げる。大きくはしない。
- セルの値は`\n`を含められる。行はセルのベースラインから下へ、フォントサイズの1.25倍の間隔で描く(回転したセルでは回転した軸に沿う)。

### G31 — ロゴ画像

`logo.path`は、テンプレートを読み込んだフォルダからの相対パスでPNG、JPEG、SVGのファイルを指す。読めて2MB以下なら、ロゴ枠の中に縦横比を保って中央に描き、枠の矩形は描かない。そうでなければ今と同じく空の矩形を描く。テンプレートのフォルダの外へ出るパス(`..`、絶対パス)は`validate()`が拒否する。

- SVG: `data:`URIの`<image>`。
- PDF: `svg2pdf`経由。計画の最初のタスクでテストにより確認する。`svg2pdf`が埋め込みSVG画像を置けない場合、G31はPNGとJPEGに限定し、仕様書を更新する。
- キャンバス: テンプレート一覧(`GET /frame-templates`とIPCの対)が実行時の項目`logo.data_url`を運ぶ。テンプレートの読み込み時に埋め、テンプレートのファイルからは読まず、書きもしない(`builtin`と同じ扱い)。

### ファイル形式

`FORMAT_VERSION`を20にする: `Revision.rev_no / approved / count`と`TitleBlock.fields`。古いファイルはそのまま読める(serdeの既定値)。マイグレーションは番号を上げるだけ。形式19を読むビルドは、これまでの版上げと同じく、形式20のファイルを版番号で断る。図枠テンプレートのファイルには版番号が無く、新しい項目は省略可能なキーである。

### 描画

`svg.rs`と`src/canvas/frameTemplate.ts`が同じ規則を実装する。共有する数値(線幅、裁断マークの寸法、大文字高さの比、行間)は両側で名前付きの定数にし、それぞれの側に値を固定するテストを置く。

### エディタ(サブプロジェクトAの範囲)

TypeScriptの型に省略可能な項目を足す。図枠エディタの操作は、編集方法を知らない項目を保持する: 新しい項目を全て持つテンプレートを読み込んで保存すると、同じJSONが返る。エディタの`REVISION_COLUMNS`の検査は、`columns`を持たないテンプレートにだけ適用する。

## テスト

項目ごとに、RustとVitestの両方で、`CLAUDE.md`が求める英日の仕様文を付けて書く:

- 項目が仕様どおりに描かれる(SVGの文字列またはレイアウトのオブジェクトで、位置と線幅を確かめる)
- 項目が無いとき: 出力が今と同一(既存の完全一致テストは編集しない)
- 不正な値: `validate()`が拒否し、項目名を示す(負の余白、知らない改訂欄の項目、同じ項目の重複、列幅の合計が表題欄の幅と合わない、不正な回転、不正な欄の名前、フォルダの外へ出るロゴのパス、2MBを超える画像)
- 形式: 形式19のプロジェクトが、新しい項目が空の形式20として読める。形式20のプロジェクトが往復できる
- エディタ: 新しい項目を全て持つテンプレートが、読み込み、無関係な編集、保存を経ても残る

テストと一緒に`python3 scripts/gen_spec.py`を再生成してコミットする。

通しの確認: 再現の`make_frame.py`を新しい項目を使うよう更新し、`run.sh r1_frame`を実行して、重ね差分の表題欄の隅に赤や青の罫線が無いことを確かめる(字形の違いは除く)。

## リスク

- SVG完全一致テストを誤って壊す。対策: 新しい処理は新しい項目があるときだけ通る。古い処理は整形し直さない。
- 2つの描画が食い違う。対策: 両側に同じテストケースを、同じ名前で置く。
- `text_width`は見積もりなので、`fit`でわずかにはみ出す、または少し縮めすぎることがある。許容する。重なり検査が既に使っている見積もりである。
