# 詳細仕様設計書

**English (canonical): [13-specification.md](13-specification.md)** | ← [ロードマップ](12-roadmap.ja.md)

> **テストスイートから自動生成 — 手で編集しないこと。**
> 以下の各項目は自動テストで常に検証されている(灰色はテストID)。
> テスト変更後は `python3 scripts/gen_spec.py` で再生成する。

本書はMadakeCADの「生きた仕様書」である:
ここに載っている挙動は、テスト実行のたびに証明される。


全5領域・**480仕様項目**。


## コアドメイン (madake-core)


### Commandエンジン (undo/redo)

- エンティティの追加はundoで消え、redoで復活する。 <sub>`add_undo_redo_entity`</sub>
- 移動でエンティティの座標が動き、undoで元の位置に正確に戻る。 <sub>`move_and_undo_restores_position`</sub>
- 複数エンティティの一括削除は1回のundoで全て復元される。 <sub>`delete_multiple_and_undo`</sub>
- シートは追加・削除でき、削除のundoは元のid・位置のままシートを復元する。 <sub>`sheet_add_remove_undo`</sub>
- CommandはJSONに往復変換でき、どのクライアントからも送信できる。 <sub>`command_json_roundtrip`</sub>
- UpdateEntityはエンティティを丸ごと置換し、undoで置換前の状態に戻る。 <sub>`update_entity_undo_restores_previous_version`</sub>
- SetTitleBlockはシートの表題欄を更新し、undoで以前の内容に戻る。 <sub>`set_title_block_is_undoable`</sub>
- SetRevisionsは改訂欄の行を置き換え、undoで以前のリストに戻る。 <sub>`set_revisions_is_undoable`</sub>
- ハーネス境界は通常のエンティティ用コマンドで追加・改名・削除でき、どの操作もundoで戻せる。 <sub>`harness_add_rename_delete_are_undoable`</sub>
- ハーネス境界を移動すると4隅すべてが動き、undoで元の位置に正確に戻る。 <sub>`harness_move_and_undo_restores_every_corner`</sub>
- ハーネス境界は kind="harness" としてJSONに往復変換でき、どのクライアントからも送れる。 <sub>`harness_command_json_roundtrip_uses_the_harness_kind`</sub>
- execute/undo/redoのたびにドキュメントrevisionが増加し、クライアントは古いpatchを破棄できる。 <sub>`revision_increases_monotonically`</sub>
- 存在しないシートへのCommandはエラーになり、何も変更されない。 <sub>`unknown_sheet_is_rejected`</sub>
- 履歴が空のときのundoはエラーではなくNoneを返す。 <sub>`undo_on_empty_history_returns_none`</sub>
- undo後に新しい編集をするとredo履歴は消える(一般的なエディタと同じ挙動)。 <sub>`new_edit_after_undo_clears_redo`</sub>

### 座標・ジオメトリ

- snapped()は座標を最も近いグリッドピッチ(既定2.5mm)へ丸め、すべてがピングリッドに乗る。 <sub>`snapped_rounds_to_grid_pitch`</sub>
- translated()は平行移動したコピーを返し、元の点を変更しない。 <sub>`translated_shifts_without_mutation`</sub>
- distance_to()はユークリッド距離である(3-4-5の直角三角形で5になる)。 <sub>`distance_is_euclidean`</sub>

### ハーネス境界

- ワイヤは、その全ての点が囲みの内側にあるときだけハーネスに所属する。 <sub>`a_fully_enclosed_wire_belongs_to_the_harness`</sub>
- 境界線上に乗っているワイヤも内側として扱う (境界線そのものはハーネスに含まれる)。 <sub>`a_wire_on_the_boundary_line_still_belongs`</sub>
- 囲みに一部だけ入っているワイヤはハーネスに所属しない。 <sub>`a_partly_overlapping_wire_does_not_belong`</sub>
- 囲みの外に描かれたワイヤはハーネスに所属しない。 <sub>`a_wire_outside_the_boundary_does_not_belong`</sub>
- ワイヤの所属ハーネスを引くと名前が返り、どこにも属さないワイヤでは空文字になる。 <sub>`harness_name_lookup_is_empty_for_unassigned_wires`</sub>
- 囲みが入れ子になっているときは、そのワイヤを囲む最も小さいハーネスに所属する。 <sub>`a_nested_harness_wins_over_the_outer_one`</sub>
- ハーネスの「含む電線」本数は、その囲みが今いくつのワイヤを囲んでいるかを表す。 <sub>`wire_count_reports_the_enclosed_wires`</sub>
- 新しいハーネスの名前は、何も無いシートではW1、既にあるときはその次の番号になる。 <sub>`the_next_harness_name_continues_the_w_series`</sub>
- 矩形ドラッグはどの向きに引いても同じ4隅の頂点になる。 <sub>`rect_points_normalize_the_drag_direction`</sub>

### プロジェクトファイルI/O (.mdkproj)

- .mdkprojへ保存して読み直したプロジェクトは、全エンティティを含めて同一である。 <sub>`project_file_roundtrip`</sub>
- 保存された.mdkprojはformat_version付きの整形JSONで、gitの差分が読みやすい。 <sub>`saved_file_is_pretty_json_with_format_version`</sub>
- 存在しないファイルの読み込みはパニックせずエラーを返す。 <sub>`loading_missing_file_is_an_error`</sub>

### KiCadインポート

- KiCadインポートは用紙サイズ・表題欄・配線・ジャンクション・ラベル(電源シンボルはネットラベル化)・テキストを変換する。 <sub>`imports_paper_title_block_and_geometry`</sub>
- 既知のlib_idは本ライブラリへ対応付けられ(Device:R→抵抗、Conn_01x03→connector_3p)、参照記号・値・回転・ミラーが保たれる。未知のシンボルはスキップされレポートに列挙される。 <sub>`maps_symbols_and_reports_skipped`</sub>
- kicad_sch文書でないファイルは明確なエラーで拒否される。 <sub>`rejects_non_schematic`</sub>
- S式パーサはアトム・クォート文字列・数値・入れ子リストを読み、型付きアクセサで取り出せる。 <sub>`parses_atoms_strings_numbers_and_nesting`</sub>
- 文字列内のエスケープ(引用符・改行)と日本語などのマルチバイト文字を正しく解釈する。 <sub>`parses_escaped_strings_and_multibyte`</sub>
- children(name)は指定した先頭シンボルを持つ子リストをすべて列挙する。 <sub>`children_iterates_all_matches`</sub>
- 括弧の不整合や閉じていない文字列は、位置付きの構文エラーとして報告される。 <sub>`syntax_errors_are_reported`</sub>

### ドキュメントモデル

- 用紙サイズはISO A列の寸法に従い、縦置きでは幅と高さが入れ替わる。 <sub>`paper_sizes_match_iso_and_orientation_swaps`</sub>
- 新規プロジェクトは「Sheet1」という1枚のシートとformat_version 1で始まる。 <sub>`new_project_has_one_default_sheet`</sub>
- Entity::translateはエンティティの全座標を動かす: ワイヤは全頂点、シンボル/ラベル/テキストは基準点。 <sub>`translate_moves_all_coordinates`</sub>
- Entity::id()は種別によらず内側エンティティのUUIDを返す。 <sub>`entity_id_is_uniform_across_kinds`</sub>

### ネットリスト抽出

- 両端がシンボルのピンに一致するワイヤは、それらのピンを1つのネットに結合する。 <sub>`wires_connect_symbol_pins_into_one_net`</sub>
- 単に交差しただけのワイヤは接続されない。ワイヤに付いたネットラベルはそのネットの名前になる。 <sub>`crossing_without_junction_stays_separate_and_label_names_net`</sub>
- ジャンクションは交差ワイヤを接続し、同名ラベルは離れたネットを1つに統合する。 <sub>`junction_connects_crossing_wires_and_same_labels_merge`</sub>
- 無名ネットには決定的な連番名(N001, N002, …)が付く。 <sub>`unnamed_nets_get_deterministic_sequential_names`</sub>
- 同一シンボルで同じピン番号を持つ2つの接続点(貫通端子)は内部短絡され、ネットのピン一覧には1回だけ載る。 <sub>`same_pin_number_points_short_internally`</sub>
- ワイヤに書かれた線番は、ネットラベルが無いネットの表示名になる。 <sub>`wire_number_names_a_net_without_a_label`</sub>
- ネットラベルは常に線番より優先され、線番は自動名 (N001) より優先される。 <sub>`net_name_prefers_label_then_wire_number_then_auto_name`</sub>
- ピン座標はシンボルの回転(Y下向き座標系で時計回り)と配置位置を反映する。 <sub>`pin_positions_apply_rotation_and_translation`</sub>
- ミラーは回転より先に、縦軸に対してピンを反転する。 <sub>`pin_positions_apply_mirror_before_rotation`</sub>

### ngspiceランナー

- ngspiceの'print all'出力はノード電圧(v(n1)→n1)と枝電流(v1#branch)に解釈され、無関係な行は無視される。 <sub>`parse_print_all_reads_nodes_and_branches`</sub>
- ngspiceの実行ファイルは MADAKE_NGSPICE環境変数→PATH→OS既定パス の優先順で探索され、存在しないenv指定は次へフォールバックする。 <sub>`find_in_prefers_env_then_path_then_candidates`</sub>
- 24V・6+6Ωの分圧回路のDC動作点は中点12V・電源電流2Aになる(ngspice必須。未導入時はスキップ)。 <sub>`run_op_solves_a_divider_when_ngspice_is_installed`</sub>

### 部品データベース

- 新規DBを開くとスキーマ作成とサンプル投入が一度だけ行われ、開き直しても再投入されない。 <sub>`open_creates_schema_and_seeds_samples_once`</sub>
- 部品は登録・型番キーでの更新・取得ができ、名称の部分一致やカテゴリ完全一致で検索できる。 <sub>`upsert_get_and_search`</sub>
- 電線品番は登録でき、線色+線径の完全一致で引き当てられる。 <sub>`wire_parts_crud_and_lookup`</sub>
- 旧スキーマv1のDBは開いた時点でv2へ移行され、既存データを保持したままspice_model列が使えるようになる。 <sub>`v1_database_migrates_to_v2_preserving_data`</sub>
- DBパスの既定はOSのアプリデータフォルダで、MADAKE_PARTS_DBで上書きできる。 <sub>`default_path_respects_env_override`</sub>

### PDF出力

- PDF出力は日本語を含む正しいPDF文書(%PDF-ヘッダ)を非自明なサイズで生成する。 <sub>`sheet_to_pdf_produces_pdf_bytes`</sub>
- PDFのページ寸法は用紙そのもの (A3横=420×297mm) になり、100%で印刷すると原寸になる。 <sub>`pdf_page_is_the_size_of_the_paper`</sub>
- PDF一括出力のページは 表紙 → 回路図の全シート → 選択した帳票 の順に並ぶ。 <sub>`pdf_book_is_cover_then_sheets_then_reports`</sub>
- 帳票を選ばなければPDF一括出力は表紙と回路図シートだけになる。 <sub>`pdf_book_without_reports_is_cover_and_sheets_only`</sub>
- 表紙は外すことができ、その場合は回路図シートが先頭になる。 <sub>`pdf_book_can_omit_the_cover`</sub>
- 一括出力は全ページを1つのPDF文書にまとめ、回路図がA3でも帳票ページはA4になる。 <sub>`pdf_book_merges_every_page_into_one_document`</sub>
- シートが1枚も無いプロジェクトでも、表紙だけの正しい1ページPDFになる。 <sub>`pdf_book_of_an_empty_project_is_just_the_cover`</sub>

### 帳票の図面シート化

- 帳票ページはA4横で、JIS図枠・帳票名・プロジェクト名/日付/ページ番号入りの表題欄を持つ。 <sub>`report_page_has_frame_title_and_title_block`</sub>
- 列見出しと全データ行のセルがページに描かれる。 <sub>`report_page_draws_headers_and_all_cells`</sub>
- 行が0件でも列見出しだけのページを1枚出す。 <sub>`report_page_with_no_rows_still_shows_headers`</sub>
- 1ページに収まらない行は次ページへ続き、各ページに列見出しを再掲してページ番号 n/N を振る。 <sub>`report_pages_split_and_repeat_headers`</sub>
- 列幅より長いセルは途中で切られ末尾が省略記号になる(文字が列からはみ出さない)。 <sub>`report_page_truncates_cells_wider_than_the_column`</sub>
- 列幅は指定した相対比に従い、指定が無ければ等分になる。 <sub>`report_page_column_widths_follow_ratios`</sub>
- データ中のXML特殊文字はエスケープされ、ページは正しいSVGのままになる。 <sub>`report_page_escapes_xml_special_characters`</sub>
- 端子台チャートのシートは帳票名に参照記号を含み、チャートのデータと同じ行を端子ごとに並べる。 <sub>`terminal_chart_sheet_matches_chart_rows`</sub>
- 端子台でないエンティティのチャートシートは1ページも出ない。 <sub>`terminal_chart_sheet_is_none_for_other_entities`</sub>
- From-To電線リストのシートはCSV帳票と同じ列・同じ行を使う。 <sub>`wire_list_sheet_matches_report_rows`</sub>
- 部品表のシートは部品グループごとに参照記号と数量を並べる。 <sub>`bom_sheet_lists_parts_with_quantity`</sub>
- クロスリファレンス表のシートはネットごとに接続先ピンと現れるシートを並べる。 <sub>`xref_table_sheet_lists_nets_and_pins`</sub>
- 表紙にはプロジェクト名・全シートと図番・プロジェクト全体で最新の改訂が出る。 <sub>`cover_page_shows_project_sheets_and_latest_revision`</sub>
- 改訂が1件も無いプロジェクトの表紙は空欄ではなく「なし」と記す。 <sub>`cover_page_states_when_there_is_no_revision`</sub>
- 帳票の種類はCLI・Link APIと同じケバブケース表記でJSONへ入る。 <sub>`report_kind_json_names_match_cli_spelling`</sub>

### 帳票 (部品表 / 電線リスト)

- 部品表はシンボルを型番でまとめ、数量を集計する。 <sub>`bom_groups_by_value_and_counts`</sub>
- カンマを含む項目は引用符で囲まれ、CSVが壊れない。 <sub>`bom_escapes_fields_with_commas`</sub>
- 電線リストはFrom-To形式で、各行はシート名とワイヤ両端の接続先から始まる。旧来の列順 (シート・線番・ハーネス…) はもう出力されない。 <sub>`wire_list_is_a_from_to_list`</sub>
- シンボルのピンに届いているワイヤの端は「参照記号:ピン番号」(例 K1:A1) と書かれる。 <sub>`a_wire_end_on_a_pin_is_written_as_reference_and_pin_number`</sub>
- ネットラベルが付いているワイヤの端は、そのラベル名で書かれる。 <sub>`a_wire_end_with_a_net_label_is_written_as_the_label_name`</sub>
- 何にも接続していないワイヤの端は、リストでは空欄になる。 <sub>`an_unconnected_wire_end_is_empty`</sub>
- 両端のうち表記の辞書順で小さい方がFromになるので、どちら向きに描いたワイヤでもFrom/Toは常に同じになる。 <sub>`from_is_the_alphabetically_smaller_of_the_two_ends`</sub>
- 片側だけ接続しているワイヤでは、接続している方が必ずFrom、空欄の方が必ずToになる。 <sub>`a_connected_end_becomes_from_and_the_unconnected_end_becomes_to`</sub>
- 貫通端子の左右は同じ端子番号なので、同じ端子の内側・外側につながる電線はどちらも「TB1:1」と書かれる。 <sub>`both_sides_of_a_feed_through_terminal_use_the_same_terminal_number`</sub>
- 電線リストには線番採番で入った線番の列があり、そのワイヤに書かれた線番が入る (未採番なら空欄)。 <sub>`wire_list_has_a_wire_number_column`</sub>
- 電線リストにはハーネス列があり、そのワイヤを完全に囲んでいるハーネス境界の名前が入る (どの囲みにも入らない線は空欄)。 <sub>`wire_list_has_a_harness_column`</sub>
- 電線リストには各ワイヤの線色・線径・長さ・品番が載る。 <sub>`wire_list_contains_attributes`</sub>
- 長さが決まっていないワイヤの長さ列は空欄になる (長さは後で3D配線から書き戻される)。 <sub>`wire_list_length_is_empty_until_it_is_known`</sub>
- 行はシートごとにFrom→Toの順に並ぶので、同じ図面を2回出力すると全く同じファイルになる。 <sub>`rows_are_sorted_by_from_then_to`</sub>

### DCシミュレーション

- 不正なngspice実行ファイルでのシミュレーションは明示的なエラーになる(暗黙のフォールバックはしない)。 <sub>`missing_ngspice_is_an_explicit_error`</sub>
- DC動作点はネットごとの電圧(この例では0〜24V)と部品ごとの電流・電力を報告する(2Aランプは約2A/約48W)。 <sub>`op_reports_net_voltages_and_component_currents`</sub>
- what-ifでスイッチを開路指定すると負荷電流はゼロになり、結果にどのスイッチを開いたかが明記される。 <sub>`open_switch_cuts_the_current`</sub>

### SPICEネットリスト生成

- 直列回路はSPICEデッキに変換される: 電圧源1つ・battery負極がGND・ワイヤごとの抵抗(ρL/A)・部品の橋・負荷の等価抵抗。 <sub>`series_circuit_builds_deck_with_source_ground_and_resistors`</sub>
- ワイヤ途中のジャンクションは、その位置の幾何学的長さ比でワイヤ抵抗を分割する。 <sub>`junction_splits_wire_resistance_proportionally`</sub>
- 離れた場所の同名ネットラベルはmΩ抵抗で橋渡しされ、論理接続が保たれる。 <sub>`same_name_labels_are_bridged`</sub>
- 電源の無い図面は変換できない(明示的なNoSourceエラー)。 <sub>`no_battery_is_an_error`</sub>
- デッキ出力は毎回同一(決定的)で、GNDノード0はbatteryの負極ピンになる。 <sub>`node_names_are_deterministic_and_ground_is_battery_minus`</sub>

### SVG出力 (JIS図枠)

- 出力SVGにはJIS図枠・表題欄の文字・配線・参照記号が含まれる。 <sub>`svg_contains_frame_wire_and_symbol`</sub>
- 品名などの特殊文字(<, >, &, 引用符)はSVG内でXMLエスケープされる。 <sub>`svg_escapes_xml_special_chars`</sub>
- 改訂が2件あると表題欄の真上に改訂表が描かれ、古い行が下・新しい行が上に積まれ、最下段に列見出し(記号/日付/内容/承認)が出る。 <sub>`svg_draws_revision_table_above_title_block`</sub>
- 改訂が0件のシートには改訂欄をまったく描かない(空の枠だけも描かない)。 <sub>`svg_omits_revision_table_when_no_revisions`</sub>
- 表題欄のRev欄には最新改訂の記号が出る。改訂が無いときは表題欄に保存された値がそのまま出る。 <sub>`svg_title_block_rev_follows_latest_revision`</sub>
- 改訂が7件あると新しい6行だけが描かれ、最も古い行は図面から省かれる(データとしては残る)。 <sub>`svg_revision_table_shows_only_newest_six_rows`</sub>
- 横向きの配線の線番は、配線の中点の2.5mm上に等幅フォントで描かれる。 <sub>`svg_draws_wire_number_above_a_horizontal_wire`</sub>
- 縦向きの配線の線番は、配線の中点の2.5mm左に描かれる。 <sub>`svg_draws_wire_number_left_of_a_vertical_wire`</sub>
- 1つのネットの線番は、何本のワイヤで描かれていても、最も長い線分の中点に1回だけ描かれる。 <sub>`svg_draws_the_wire_number_once_on_the_longest_segment`</sub>
- 線番の無いネットには線番テキストを一切描かない。 <sub>`svg_omits_wire_number_for_unnumbered_nets`</sub>
- ハーネス境界は、囲んだ配線のまわりに破線の矩形 (IEC 61082-1のグループ囲み) として描かれる。 <sub>`svg_draws_a_harness_as_a_dashed_rectangle`</sub>
- ハーネス名は囲みの左上角のすぐ外側に描かれる。 <sub>`svg_labels_the_harness_at_its_top_left_corner`</sub>
- 名前の無いハーネスは破線の囲みだけを描き、ラベルは出さない。 <sub>`svg_omits_the_label_of_an_unnamed_harness`</sub>
- 回転したシンボルは形状ごと回転して描かれる(90度で抵抗の本体が縦長になる)。 <sub>`svg_renders_rotated_symbol_primitives`</sub>
- プロジェクトの一部として書き出したシートには、各ネットラベルの脇に他シートの同名ラベルの住所「/シート.ゾーン」が出る。 <sub>`svg_draws_cross_reference_address_next_to_net_label`</sub>
- 他のシートに相手がいないネットラベルには、クロスリファレンスの文字が出ない。 <sub>`svg_omits_cross_reference_when_there_is_no_counterpart`</sub>
- プロジェクトの文脈なしにシート単体を書き出したときは、クロスリファレンスを描かない。 <sub>`svg_of_a_lone_sheet_has_no_cross_reference`</sub>

### シンボルライブラリ

- 同梱シンボルはすべて一意のidを持ち、最低1つのピンを持つ。 <sub>`builtin_symbols_have_unique_ids_and_pins`</sub>
- ピン数可変の端子台(例: 8極)は端子ごとに左右1点ずつの接続点を持ち、すべて2.5mmグリッド上・上下中央揃えになる。 <sub>`dynamic_terminal_block_has_through_pins_on_grid`</sub>
- 動的生成のconnector_2pは旧静的定義と完全に同じピン座標を持ち、既存図面に影響しない。 <sub>`dynamic_connector_2p_matches_legacy_static_def`</sub>
- resolve_symbolは同梱idを見つけ、不正・範囲外の動的ID(0極・51極・数値なし)は拒否する。 <sub>`resolve_symbol_rejects_invalid_ids_and_finds_builtins`</sub>
- sheet_symbol_defsは同梱ライブラリに加え、シートで実際に使われている動的シンボルの定義を返す。 <sub>`sheet_symbol_defs_includes_dynamic_ids_in_use`</sub>
- シンボル定義はJSONに往復変換しても失われない。 <sub>`symbol_json_roundtrip`</sub>

### 端子台チャート

- 端子台のチャートは端子1個につき1行で、端子番号の順に並ぶ。 <sub>`the_chart_has_one_row_per_terminal_in_number_order`</sub>
- 端子の左側に繋がっているものが内部側 (盤内)、右側に繋がっているものが外部側 (盤外) になる。 <sub>`the_left_side_is_the_inside_and_the_right_side_is_the_outside`</sub>
- 端子台を180度回転させると内外も入れ替わる。内部側はあくまで用紙上で左に描かれている側である。 <sub>`a_terminal_block_rotated_180_degrees_still_takes_the_paper_left_as_the_inside`</sub>
- 端子台を90度回して横向きに置くと接続点は上下に並び、上側が内部側として扱われる。 <sub>`a_sideways_terminal_block_takes_the_upper_connection_as_the_inside`</sub>
- どちら側にも電線が繋がっていない端子は、内部側・外部側とも空欄の予備端子として行が残る。 <sub>`an_unwired_terminal_stays_as_a_spare_row`</sub>
- 各行にはその電線に振られた線番と、電線の仕様 (線色・線径sq・品番) が載る。 <sub>`a_row_shows_the_wire_number_and_the_wire_specification`</sub>
- 内部側と外部側で電線が違うときは、行に両方の電線が並ぶ。 <sub>`both_wires_are_listed_when_the_inside_and_outside_differ`</sub>
- ジャンパの記述は正規化される。各組は小さい番号が先になり、重複は除かれ、昇順に並ぶ。 <sub>`jumpers_are_normalized`</sub>
- 隣り合っていない端子どうしのジャンパは受け付けられず、同じ記述内の正しいジャンパは残る。 <sub>`a_jumper_between_non_adjacent_terminals_is_rejected`</sub>
- 端子番号2つの組になっていないジャンパの記述は、異常終了せずエラーとして報告される。 <sub>`unreadable_jumper_text_is_reported`</sub>
- 端子台に無い端子番号を指すジャンパは、存在しない端子として報告される。 <sub>`a_jumper_to_a_terminal_that_does_not_exist_is_reported`</sub>
- ジャンパの記述が空なら、ジャンパは無いという意味になる。 <sub>`no_jumper_text_means_no_jumpers`</sub>
- ジャンパは、それが繋ぐ両方の端子のジャンパ欄に表示される。 <sub>`a_jumper_is_shown_on_both_of_its_terminals`</sub>
- 端子台チャートのCSVは 端子・内部側・線番・電線・外部側・ジャンパ の列を持ち、端子1個につき1行になる。 <sub>`the_terminal_chart_csv_has_the_designed_columns`</sub>
- 端子台ではないものにチャートを求めても何も返らない。 <sub>`only_terminal_blocks_have_a_chart`</sub>
- 端子台チェックは未結線の端子をすべて情報として報告するので、予備端子は間違い扱いされずに見える。 <sub>`the_check_reports_unwired_terminals_as_information`</sub>
- 端子台チェックは、隣り合わない端子に掛けられたジャンパをエラーとして報告する。 <sub>`the_check_reports_an_invalid_jumper_as_an_error`</sub>
- 端子台チェックは、存在しない端子へのジャンパをエラーとして報告する。 <sub>`the_check_reports_a_jumper_to_a_missing_terminal_as_an_error`</sub>
- 全端子が結線されジャンパも正しい端子台は、チェックで何も指摘されない。 <sub>`a_fully_wired_terminal_block_passes_the_check`</sub>
- ジャンパは通常のupdate_entityコマンドで設定するので、チャートに反映され、undoで元に戻る。 <sub>`setting_jumpers_through_update_entity_is_undoable`</sub>

### 検証 (ERC・電気検証)

- 完全に結線された回路ではERCの指摘は出ない。 <sub>`fully_wired_pair_has_no_erc_findings`</sub>
- 未接続ピンはシンボルごとに1件の警告として、該当ピン番号を列挙して報告される。 <sub>`unconnected_pins_are_reported_per_symbol`</sub>
- 端子台の端子は左右どちらか一方が結線されていれば接続済みとみなす。 <sub>`terminal_block_terminal_counts_connected_if_either_side_wired`</sub>
- 参照記号の未設定・重複は指摘されるが、リレーのコイル+接点が同じ記号を共有するのは正当として許容される。 <sub>`empty_and_duplicate_references_are_flagged_but_relays_allowed`</sub>
- どこにも接続されていないワイヤ端点は指摘される(ジャンクション無しで他ワイヤの途中に乗る端点も含む)。 <sub>`dangling_wire_end_is_flagged_including_missing_junction`</sub>
- 健全な直列回路(電源・ヒューズ・スイッチ・ランプ)では電気検証の指摘は出ない。 <sub>`healthy_series_circuit_has_no_elec_findings`</sub>
- 電源から切り離された負荷(断線・開路)は「到達不能」として報告される。 <sub>`load_cut_off_from_source_is_unreachable`</sub>
- 負荷電流がワイヤの許容電流を超えるとエラー、ヒューズ定格を超えると警告になる。 <sub>`overloaded_wire_and_fuse_are_flagged`</sub>
- 長い配線で電源電圧の3%を超える電圧降下は指摘され、短い配線では出ない。 <sub>`excessive_voltage_drop_is_flagged`</sub>
- current_a属性が無い負荷は電流計算から除外され、Infoとして通知される。 <sub>`load_without_current_attr_gets_info_and_no_current_checks`</sub>
- ngspiceがあれば電気検証はソルバの実測値で報告され、近似モードの通知は出ない。 <sub>`simulation_mode_reports_measured_values_when_ngspice_installed`</sub>
- ソルバ結果が無い場合はグラフ近似にフォールバックし、その旨をInfoで明示する。 <sub>`fallback_mode_emits_approximate_info`</sub>
- 1つのネットに異なるネットラベルが混在する状態(異電位の短絡)はエラーになる。 <sub>`conflicting_net_labels_on_one_net_are_an_error`</sub>
- シート1枚の中のラベル競合は、プロジェクト全体を検証しても1件だけ報告される。 <sub>`project_verification_reports_a_single_sheet_label_conflict_once`</sub>
- 同名ラベルでシートを跨いで繋がったネットは1ネットとして検査されるので、2枚に跨る競合も全ラベルを挙げた1件として報告される。 <sub>`project_verification_merges_label_conflicts_across_sheets`</sub>
- 同じラベル名でシートを跨いで続くネットは、競合ではない。 <sub>`project_verification_accepts_a_net_continued_onto_another_sheet`</sub>

### 線番採番

- 追い番モードは線番の無いネットにだけ番号を振り、すでに線番を持つネットはそのまま残す。 <sub>`append_numbers_only_unnumbered_nets`</sub>
- 新しく振る番号は、図面ですでに使われている番号とは決して衝突しない。 <sub>`append_skips_numbers_already_in_use`</sub>
- 振り直しモードは数字の線番を全て捨て、開始番号から順に新しい連番を振り直す。 <sub>`renumber_reassigns_every_numeric_wire_number`</sub>
- 数字ではない手書きの名前 (例: "24V_1") は、全振り直しをしても消えずに残る。 <sub>`renumber_keeps_manual_non_numeric_names`</sub>
- ネットラベルが付いたネットはラベル名がそのまま名前になり、線番は振られない。 <sub>`nets_with_a_net_label_are_never_numbered`</sub>
- 1つのネットに属する全ワイヤには、別々に描かれていても同じ線番が入る。 <sub>`every_wire_of_a_net_gets_the_same_number`</sub>
- 自動採番のundoは、線番が無かったワイヤも含めて全ての線番を元の状態へ戻す。 <sub>`undo_restores_previous_wire_numbers`</sub>
- 採番は上から下へ、同じ高さなら左から右へ進むため、同じ図面なら常に同じ線番になる。 <sub>`numbering_order_is_top_to_bottom_then_left_to_right`</sub>
- 開始番号は自由に指定でき、例えば2面目の盤を100番から始められる。 <sub>`numbering_starts_at_the_given_start_number`</sub>
- シートを指定しない採番はプロジェクトの全シートをシート順に処理し、番号は図面全体で重複しない。 <sub>`numbering_without_a_sheet_covers_the_whole_project`</sub>
- 線番は1本ずつ直接編集でき、undoで以前の値に戻る。 <sub>`set_wire_numbers_edits_one_wire_and_is_undoable`</sub>
- 採番コマンドは素のJSON ({"type":"renumber_wires","mode":"append","start":1}) で表現でき、AIやCLIから送れる。 <sub>`renumber_command_is_plain_json`</sub>

### xref

- ゾーンアドレスは行の英字(上から)と列の数字(左から)を組み合わせた「B3」形式になる。 <sub>`zone_address_combines_row_letter_and_column_number`</sub>
- 図枠の外にある点は、無効なアドレスにはならず最も近いゾーンに丸められる。 <sub>`zone_address_clamps_points_outside_the_frame`</sub>
- 別々のシートに置かれた同名のネットラベルは、プロジェクト全体では1つのネットに統合される。 <sub>`same_named_labels_merge_into_one_project_net`</sub>
- ラベル名が違うネットは、シートを跨いでも統合されず別のネットのままになる。 <sub>`differently_named_labels_stay_separate_nets`</sub>
- ラベルの相手先は、他のシートにある同名ラベルの住所「/シート.ゾーン」になる。 <sub>`cross_reference_address_uses_slash_sheet_dot_zone`</sub>
- 相手側のシートのラベルからも元のシートが見えるので、双方に相手先が表示される。 <sub>`cross_reference_is_shown_on_both_sides`</sub>
- 同じネットが複数のシートに続くときは、相手先の住所が全て列挙される。 <sub>`multiple_counterparts_are_all_listed`</sub>
- 自分のシート内の所在は、同名ラベルが2つあっても相手先には出ない。 <sub>`own_sheet_is_excluded_from_counterparts`</sub>
- 他のシートに相手がいないラベルには、クロスリファレンスが一切表示されない。 <sub>`label_without_counterpart_shows_nothing`</sub>
- シートごとのクロスリファレンス表は、各ラベルのentity idを脇に描くテキストへ対応付ける。 <sub>`sheet_cross_reference_table_maps_labels_to_text`</sub>
- クロスリファレンスのテキストは、ラベル本文の右側に同じベースラインで並ぶ。 <sub>`cross_reference_text_sits_right_of_the_label`</sub>
- クロスリファレンス表はプロジェクト全体のネット1本につき1行で、跨るシートとラベルの図面上の住所を並べる。 <sub>`cross_reference_table_lists_one_row_per_net_with_its_sites`</sub>
- クロスリファレンス表の列は ネット・線番・接続先・シート・所在 の5列。 <sub>`cross_reference_table_columns_are_net_wire_pins_sheets_sites`</sub>


## 自動化API (MCP / REST)


### エージェントRESTエンドポイント

- POST /agent/send はアシスタントのターンを実行し、会話一覧に新しいメッセージが反映される。 <sub>`send_runs_a_turn_and_conversations_reflects_it`</sub>
- 存在しない会話IDへの送信は400を返し、不正なデータを作らない。 <sub>`send_to_unknown_conversation_is_400`</sub>
- キャンセルとターン巻き戻しのエンドポイントは、最新の適用済みターンに対して正しく応答する。 <sub>`cancel_and_undo_turn_respond`</sub>
- ターンの巻き戻しは、そのターンが行った編集だけを手動編集と同じundo履歴経由で戻す。 <sub>`undo_turn_rolls_back_agent_edits_through_the_command_engine`</sub>
- GET /agent/events は会話イベントをTauriのagent:eventと同じ形でSSE配信する。 <sub>`events_endpoint_streams_agent_events`</sub>
- プロジェクトの保存・読込はチャット履歴(.chat.json)を一緒に運ぶ。 <sub>`save_and_load_carry_the_chat_history`</sub>
- プロジェクト読込は実行中のターンを先に中断し、エージェントが古い図面を編集し続けないようにする。 <sub>`load_cancels_a_running_turn`</sub>
- 外部Webオリジンからのリクエストは拒否され、ローカルオリジンとブラウザ以外(Originヘッダなし)は通る。 <sub>`external_origins_are_rejected_but_local_and_originless_pass`</sub>
- オリジンガードはループバックホスト(localhost/127.0.0.1)のみをローカル扱いする。 <sub>`local_origin_predicate_matches_only_loopback_hosts`</sub>
- エージェントへ渡す図面コンテキストはアクティブシートの要約(名前・ネット数・要素数)を含む。 <sub>`drawing_context_summarizes_the_active_sheet`</sub>
- AI設定のエンドポイントは変更を永続化し、エージェントマネージャへ適用する。 <sub>`settings_endpoints_persist_and_apply`</sub>

### REST Link API

- RESTの部品エンドポイントは検索(サンプル含む)・登録更新・カテゴリ絞り込み・削除・電線品番一覧に対応する。 <sub>`parts_endpoints_search_upsert_delete`</sub>
- GET /api/v1/verify は図面の診断(空参照・未接続ピンなど)をJSONで返す。 <sub>`verify_returns_diagnostics`</sub>
- POST /api/v1/import/kicad は開いているプロジェクトを変換結果で置き換え、patchとインポートレポートを返す。 <sub>`import_kicad_replaces_project_and_reports`</sub>
- POST /api/v1/simulate/op はDC動作点を解き、ネット電圧と部品電流を返す(ngspice必須。未導入時はスキップ)。 <sub>`simulate_op_returns_result`</sub>
- POST /api/v1/export/pdf は指定パスへ正しいPDFファイルを書き出す。 <sub>`export_pdf_writes_pdf_file`</sub>


## AIアシスタント (madake-agent)


### Claude CLIバックエンド

- Claude CLIは必須フラグ(-p・stream-json出力・部分メッセージ・strict MCP設定)付きでヘッドレス起動される。 <sub>`args_contain_required_flags`</sub>
- セッション再開ID・モデル指定・追加システムプロンプトは、指定時にCLIへ引き渡される。 <sub>`args_include_resume_model_and_system_prompt_when_given`</sub>
- エージェントのMCP設定はMadakeCAD自身のローカルMCPサーバーを指し、他クライアントと同じツールを使う。 <sub>`mcp_config_points_at_local_mcp_server`</sub>
- ターンはCLIのstream-json出力から解釈したイベント(テキスト差分・ツール実行・完了)を流す。 <sub>`send_streams_events_from_fake_cli`</sub>
- CLIが非ゼロ終了した場合はハングせずエラーイベントとして報告される。 <sub>`send_reports_nonzero_exit_as_error_event`</sub>
- プロンプトはargvではなくstdin経由で渡される(OSの引数長・クォート問題を避ける)。 <sub>`prompt_is_passed_through_stdin_not_argv`</sub>
- イベント受信側が消えてもターンは永久にブロックせず速やかに終了する。 <sub>`send_returns_promptly_when_receiver_is_dropped`</sub>
- 未知の行しか流れない出力でもターンは速やかに終了する。 <sub>`send_returns_promptly_when_only_non_event_lines_flow`</sub>
- CLI出力読み取り中のI/Oエラーはエラーイベントになる。 <sub>`io_error_while_reading_is_reported_as_error_event`</sub>
- Claude CLI検出は設定された実行ファイルからバージョンを読む。 <sub>`detect_reads_version_from_configured_executable`</sub>
- 実行ファイルが存在しない場合、検出は明確に失敗する。 <sub>`detect_fails_for_missing_executable`</sub>
- 検出は候補パスを順に試し、動くものが見つかるまでフォールバックする。 <sub>`detect_falls_back_to_later_candidates`</sub>
- どの候補も動かない場合、試した内容が分かるエラーを報告する。 <sub>`detect_from_reports_error_when_no_candidate_works`</sub>
- 既定の候補にはClaude CLIの既知のインストール先が含まれる。 <sub>`default_candidates_include_known_install_paths`</sub>

### 会話・履歴

- 各ターンは跨いだエンジンrevision範囲を記録する(表示・デバッグ用)。 <sub>`turn_records_engine_revision_range`</sub>
- ターンのundo回数はrevision差分ではなくundoスタック深さの増分で数える(undo/redoでもrevisionは進むため)。 <sub>`undo_count_uses_undo_stack_depth_not_revision_delta`</sub>
- ターン中に編集がすべてundoされた場合、そのターンは編集なしとして扱われる。 <sub>`turn_whose_edits_were_all_undone_has_no_edits`</sub>
- 巻き戻しが途中で止まった場合、完了したundo回数だけが記録される。 <sub>`record_undone_applies_only_the_completed_count`</sub>
- テキストのみのターン(図面編集なし)のundo回数はゼロ。 <sub>`turn_without_edits_has_zero_undo_count`</sub>
- ターンはストリームされたテキスト・ツール実行・CLIセッションIDを収集する。 <sub>`turn_collects_text_tool_calls_and_session`</sub>
- 2回目のターンは会話に追記され、同じCLIセッションを再開する。 <sub>`second_turn_appends_messages_and_reuses_session`</sub>
- エラーイベントは現在ターンのメッセージに記録される。 <sub>`error_event_is_recorded_on_current_turn`</sub>
- チャット履歴は整形JSONで保存され、同一内容で読み戻せる。 <sub>`chat_file_roundtrip_is_pretty_json`</sub>
- 新しいformat_versionのチャットファイルは黙って壊さず拒否する。 <sub>`load_chat_rejects_newer_format_version`</sub>
- チャット保存はアトミック(一時ファイル+リネーム)で、一時ファイルを残さない。 <sub>`save_chat_writes_atomically_and_leaves_no_temp_file`</sub>
- チャットファイルが無い場合は空の履歴として読み込まれる。 <sub>`load_chat_of_missing_file_is_empty`</sub>
- 会話は作成時にupdated_atを持ち、ターンごとに進む。 <sub>`updated_at_is_set_on_creation_and_advances_with_the_turn`</sub>
- updated_atの無い旧チャットファイルは妥当な既定値で読み込まれる。 <sub>`load_chat_defaults_updated_at_for_legacy_files`</sub>
- チャットファイルはプロジェクトの隣に<名前>.chat.jsonとして置かれる。 <sub>`chat_path_sits_next_to_project_file`</sub>

### エージェントマネージャ (ターン)

- 会話IDなしの送信は会話を新規作成し、そのイベントを配信する。 <sub>`send_creates_conversation_and_broadcasts_events`</sub>
- 図面を編集したターンは適用revisionを記録し、undo深さ付きのターン適用イベントを発行する。 <sub>`turn_records_applied_revisions_and_emits_turn_applied`</sub>
- ターンのundo回数はundoスタックの増分基準で、ターン中のユーザーundoが数を狂わせない。 <sub>`undo_turn_counts_stack_growth_not_revision_delta`</sub>
- 巻き戻せるのは最新の適用済みターンのみで、古い対象は拒否される(安全ガード)。 <sub>`undo_turn_rejects_targets_that_are_not_the_latest_applied_turn`</sub>
- 実行中のターンは巻き戻せない。 <sub>`undo_turn_rejects_a_running_turn`</sub>
- undoが途中で失敗した場合も進んだ分だけ記録し、状態を正直に保つ。 <sub>`undo_turn_records_partial_progress_when_undo_fails_midway`</sub>
- ドキュメントエラーと不明なターン指定は区別されたエラーとして報告される。 <sub>`undo_turn_reports_doc_errors_and_unknown_targets`</sub>
- ターン実行中の会話への追加送信は拒否される。 <sub>`second_send_while_running_is_rejected`</sub>
- キャンセルはCLIプロセスを停止し、メッセージをキャンセル済みにする。 <sub>`cancel_stops_the_turn_and_marks_the_message`</sub>
- 不明な会話IDへの送信は明確に失敗する。 <sub>`send_to_unknown_conversation_fails`</sub>
- 図面コンテキストと選択モデルはCLI起動へ引き渡される。 <sub>`context_and_model_are_forwarded_to_the_cli`</sub>
- 図面自動読み取りをオフにすると図面コンテキストは付かない。 <sub>`auto_read_drawing_off_suppresses_the_drawing_context`</sub>
- claude実行ファイルパス設定がバックエンドの実行ファイルを上書きする。 <sub>`claude_path_setting_becomes_the_backend_executable`</sub>
- 会話履歴の置き換え(プロジェクト読込)は実行中ターンを先にキャンセルする。 <sub>`set_conversations_replaces_history_and_cancels_running_turn`</sub>

### stream-jsonパーサ

- CLIのsystem/init行はセッションIDを載せたセッション開始イベントになる。 <sub>`init_line_yields_session_started`</sub>
- テキスト差分のみがテキストイベントになり、thinking差分は無視される。 <sub>`only_text_deltas_become_text_events`</sub>
- assistantのtool_useブロック(完全な入力付き)がツール実行開始イベントになる。 <sub>`assistant_tool_use_block_yields_tool_use_started`</sub>
- userのtool_result行が対応するツール実行を完了させ、エラーフラグを伝える。 <sub>`tool_result_yields_tool_use_finished`</sub>
- result行はトークン使用量付きでターンを完了させる。 <sub>`result_line_yields_turn_completed_with_usage`</sub>
- 未知のメッセージ種別やノイズ行はイベントを生まない(前方互換)。 <sub>`unknown_and_noise_lines_are_none`</sub>
- エラーのresultはメッセージ付きのエラーイベントになる。 <sub>`error_result_yields_error_event`</sub>
- 実際に採取したストリームが期待どおりの完全なイベント列に解釈される。 <sub>`full_event_sequence_of_tooluse_fixture`</sub>
- パーサはツール実行の完了時にツール名を補完する。 <sub>`stream_parser_fills_tool_name_on_finish`</sub>
- 同じIDの重複したツール開始イベントは破棄される。 <sub>`stream_parser_drops_duplicate_tool_use_started`</sub>

### AI設定

- AI設定の既定は自動適用と図面自動読み取りが有効。 <sub>`default_settings_are_auto_apply_and_auto_read`</sub>
- 設定ファイルが無ければ既定値になる。 <sub>`missing_file_yields_defaults`</sub>
- 保存した設定は同一内容で読み戻せる。 <sub>`saved_settings_round_trip`</sub>
- 設定ファイルに無い項目は既定値へフォールバックする(前方互換)。 <sub>`missing_fields_fall_back_to_defaults`</sub>
- 保存時に設定ディレクトリが無ければ作成される。 <sub>`save_creates_the_settings_directory`</sub>
- 空白の実行ファイルパスは保存されず正規化で除去される。 <sub>`normalized_drops_blank_paths`</sub>
- 設定ファイルパスは環境変数の上書きに従う。 <sub>`settings_path_honors_the_env_override`</sub>
- UIの既定言語は英語 (en)。 <sub>`default_language_is_english`</sub>
- 言語フィールド追加前に保存された設定ファイルは英語 (en) として読み込まれる。 <sub>`old_settings_file_without_language_loads_as_english`</sub>
- 言語タグは正規化で小文字になり、空白だけの入力は英語 (en) に戻る。 <sub>`language_is_normalized_to_lowercase_and_blank_becomes_english`</sub>
- 言語の選択は保存して読み直しても保持される。 <sub>`language_round_trips_through_save_and_load`</sub>


## madake CLI


### 引数解釈・ディスパッチ

- 既定ポートは9310で、--jsonは指定しない限り無効。 <sub>`default_port_is_9310_and_json_is_off`</sub>
- --portと--jsonはグローバルオプションで、サブコマンドの後にも書ける。 <sub>`port_and_json_are_global_options_after_subcommand`</sub>
- エクスポート種別はドキュメント表記どおりの'wire-list'を受け付ける。 <sub>`export_kind_accepts_wire_list_spelling`</sub>
- madake statusはヘルスチェックとプロジェクト概要を取得する。 <sub>`status_queries_health_and_project`</sub>
- --jsonはjq等へ渡せる整形JSONをそのまま出力する。 <sub>`json_flag_emits_raw_json`</sub>
- madake netlistは--sheetオプションをAPIへ引き渡す。 <sub>`netlist_forwards_sheet_option`</sub>
- madake exportは種別・出力パス・シート指定をAPIへ引き渡す。 <sub>`export_forwards_kind_path_and_sheet`</sub>
- madake execはCommand配列のJSONファイルを読み、/commandsへ送信する。 <sub>`exec_posts_command_array_from_file`</sub>
- madake execは配列でないJSONを明確なメッセージで拒否する。 <sub>`exec_rejects_non_array_json`</sub>
- 入力ファイルが無い場合はパニックせずファイルエラーとして報告する。 <sub>`exec_reports_missing_file`</sub>
- madake renumberは既定でプロジェクト全体を1から追い番で採番する。 <sub>`renumber_defaults_to_whole_project_append_from_one`</sub>
- madake renumberは対象シート・採番方式・開始番号をそのままコマンドへ載せる。 <sub>`renumber_forwards_sheet_mode_and_start`</sub>
- madake renumberは全ネットが採番済みで変更が無かったことを伝える。 <sub>`renumber_reports_when_nothing_changed`</sub>
- madake undoは戻す操作が無いことをユーザーに伝える。 <sub>`undo_reports_empty_history`</sub>
- madake redoは成功時に新しいドキュメントrevisionを報告する。 <sub>`redo_reports_revision`</sub>
- madake save/openは対象のファイルパスを報告する。 <sub>`save_and_open_report_path`</sub>

### Link APIクライアント

- CLIは http://127.0.0.1:<ポート>/api/v1(ループバックのみ)へ接続する。 <sub>`base_url_uses_loopback_and_api_v1`</sub>
- endpoint()はベースURLに相対パスを連結する。 <sub>`endpoint_appends_path`</sub>
- シート未指定のときnetlist URLにクエリは付かない。 <sub>`netlist_url_omits_query_without_sheet`</sub>
- シート指定はサーバー側と同じsheet_idクエリ名を使う。 <sub>`netlist_url_uses_sheet_id_query_name`</sub>
- 各エクスポート種別(svg/pdf/bom/wire-list)は対応するRESTルートへ対応付く。 <sub>`export_kind_paths_match_link_api_routes`</sub>
- アプリ未起動時は不可解なエラーではなく、ポート付きの明快なメッセージを出す。 <sub>`not_running_error_is_explicit`</sub>

### 人間向け整形出力

- 表示幅は全角文字を2桁として数え、表の桁揃えを正しくする。 <sub>`disp_width_counts_fullwidth_as_two`</sub>
- パディングは表示幅基準で、日本語とASCIIのセルが揃う。 <sub>`pad_uses_display_width`</sub>
- 表は最も広いセルに合わせて列を揃える。 <sub>`table_aligns_columns`</sub>
- madake statusの出力は接続先とドキュメント(シート・要素数)を要約する。 <sub>`status_summarizes_connection_and_document`</sub>
- madake projectは各シートを要素数付きで一覧する。 <sub>`project_lists_sheets_with_entity_counts`</sub>
- madake netlistはネットの表をピン参照(K1:2形式)付きで描画する。 <sub>`netlist_renders_table_with_pin_references`</sub>
- ネットが無い場合は空の表ではなく分かりやすいメッセージを出す。 <sub>`netlist_handles_empty`</sub>
- madake execは実行したコマンド数と結果のrevisionを報告する。 <sub>`exec_result_reports_revision_and_op_count`</sub>
- undo/redoの整形は「対象なし」(nullパッチ)の場合を扱う。 <sub>`history_result_handles_null_patch`</sub>
- undo/redoの整形はrevisionと変更件数を報告する。 <sub>`history_result_reports_revision`</sub>
- 保存・エクスポートのメッセージには書き出したパスが含まれる。 <sub>`saved_and_exported_report_written_path`</sub>
- openのメッセージには読み込んだパスと結果のrevisionが含まれる。 <sub>`opened_reports_path_and_revision`</sub>


## フロントエンド (エディタUI)


### agentOverlay

- place_symbolのx/yからシンボル概寸の領域を作る <sub>`toolBox`</sub>
- draw_wireのpoints列のバウンディングボックスを作る <sub>`toolBox`</sub>
- execute_commands内のadd_entityを合成した領域になる <sub>`toolBox`</sub>
- 座標を持たないツールや不正なinputは領域を作らない <sub>`toolBox`</sub>
- ワイヤは頂点列のbbox、シンボルは基準点±概寸で領域を作る <sub>`entityBox`</sub>
- ジャンクション・テキスト・ネットラベルも領域を持つ <sub>`entityBox`</sub>
- ツール開始でマージン込みの領域が表示される <sub>`AgentOverlay`</sub>
- 領域を作らないツールは無視される <sub>`AgentOverlay`</sub>
- 同一idの再通知では領域が重複しない <sub>`AgentOverlay`</sub>
- 完了していないツールの領域は残り続ける <sub>`AgentOverlay`</sub>
- 完了後HOLD_MS経過で領域は消える <sub>`AgentOverlay`</sub>
- idの無い完了通知はツール名で対応付けて畳む <sub>`AgentOverlay`</sub>
- エンティティのupsertから領域を作り、HOLD_MSで消える <sub>`AgentOverlay`</sub>
- 同一エンティティの再upsertで表示期限が延びる <sub>`AgentOverlay`</sub>
- finishAllで進行中の領域も期限切れになる <sub>`AgentOverlay`</sub>
- clearで全領域が消える <sub>`AgentOverlay`</sub>
- パルスの透明度は常に0.1〜0.25のsin波に収まる <sub>`pulseAlpha`</sub>
- 元の矩形を変更せずマージンを足す <sub>`expandBox`</sub>

### dynamicSymbol

- connector_2pは旧静的定義と同一のピン座標を持つ <sub>`dynamicSymbol`</sub>
- terminal_block_3pは3端子×左右2接続点で、中央揃え・2.5mmグリッド上にある <sub>`dynamicSymbol`</sub>
- 不正な動的IDはnullになる <sub>`dynamicSymbol`</sub>
- 静的定義を優先し、無ければ動的生成にフォールバックする <sub>`resolveSymbolDef`</sub>

### ハーネス境界

- 矩形ドラッグはどの向きに引いても同じ4隅の頂点になる <sub>`harness geometry`</sub>
- 全ての点が囲みの内側にある配線だけがハーネスに所属する(境界線上は内側) <sub>`harness geometry`</sub>
- 入れ子の囲みでは、その配線を囲む最も小さいハーネスに所属する <sub>`harness geometry`</sub>
- ハーネスの「含む電線」本数は、その囲みが今いくつの配線を囲んでいるかを表す <sub>`harness geometry`</sub>
- 囲みの外接矩形は左上と右下の点を返し、頂点が無ければnullになる <sub>`harness geometry`</sub>
- 名前ラベルは囲みの左上角の外側(右へ1mm・上へ1mm)に置く <sub>`harness geometry`</sub>
- 新しいハーネスの名前は、何も無いシートではW1、既にあるときはその次の番号になる <sub>`harness naming`</sub>
- ハーネス以外のエンティティ(参照記号など)は名前の採番に影響しない <sub>`harness naming`</sub>
- 矩形ドラッグはadd_entityコマンド1回(kind=harness・自動採番した名前)になる <sub>`harnessAddCommand`</sub>
- つぶれた矩形(クリックしただけ)ではハーネスを作らない <sub>`harnessAddCommand`</sub>

### renderer

- 改訂欄は表題欄の真上に同じ右端・同じ幅で置かれ、行高は表題欄と同じ8mmになる <sub>`revisionLayout`</sub>
- 改訂行は古い行が下・新しい行が上に積まれ、列見出しは最下段(表題欄側)に置かれる <sub>`revisionLayout`</sub>
- 改訂が無いシートには改訂欄を作らない(空の枠も描かない) <sub>`revisionLayout`</sub>
- 改訂が7件あると新しい6件だけが描かれ、最も古い行は省略される(データは残る) <sub>`revisionLayout`</sub>
- 列は記号・日付・内容・承認の4つで、合計幅は表題欄の幅と一致する <sub>`revisionLayout`</sub>
- 表示対象の改訂は新しい方から6件までで、古い順のまま返る <sub>`visibleRevisions / effectiveRev`</sub>
- 表題欄のRev欄は最新改訂の記号を出し、改訂が無ければ表題欄の値、それも空ならハイフンを出す <sub>`visibleRevisions / effectiveRev`</sub>

### viewClasses

- エンティティ種別を表示クラスへ対応付ける(ジャンクションは配線扱い) <sub>`entityViewClass`</sub>
- VIEW_CLASSESは全クラスを一意に列挙する(線番・ハーネスを加えて9クラス) <sub>`entityViewClass`</sub>
- 表示クラスは初期状態で全て表示ONになっている(線番・ハーネスも最初から見える) <sub>`view class visibility`</sub>

### viewport

- ワールド⇔スクリーン変換は往復可能で、ズームはアンカー点を固定する <sub>`Viewport`</sub>
- パンはスクリーンpx単位でビューを動かす <sub>`Viewport`</sub>
- スナップは既定で2.5mmグリッドへ丸める <sub>`Viewport`</sub>
- ズーム倍率は妥当な範囲に制限される <sub>`Viewport`</sub>

### wireNumbers

- 横向きの配線の線番は、配線の中点の2.5mm上に中央揃えで置かれる <sub>`wireNumberLabels`</sub>
- 縦向きの配線の線番は、配線の中点の2.5mm左に右揃えで置かれる <sub>`wireNumberLabels`</sub>
- 1つの線番は、何本のワイヤに分かれていても最も長い線分の中点に1回だけ置かれる <sub>`wireNumberLabels`</sub>
- 線番の無い配線、空白だけの線番は描かない <sub>`wireNumberLabels`</sub>
- 複数の線番があると線番順に並んで返り、同じ図面なら常に同じ並びになる <sub>`wireNumberLabels`</sub>
- 斜めの配線は、横に長ければ上、縦に長ければ左に線番を置く <sub>`wireNumberLabels`</sub>
- 配線以外のエンティティは線番の計算に影響しない <sub>`wireNumberLabels`</sub>
- 既定は「連番・開始1・現在のシート・既存の線番は保持」で開く <sub>`wire numbering dialog store`</sub>
- 「現在のシート」+「保持する」は、そのシートだけを追い番で採番するコマンドになる <sub>`wire numbering dialog store`</sub>
- 「現在のシート」+「すべて振り直す」は、そのシートを振り直すコマンドになる <sub>`wire numbering dialog store`</sub>
- 「プロジェクト全体」はシート指定を省いたコマンドになり、図面全体で一意の線番が振られる <sub>`wire numbering dialog store`</sub>
- 開始番号は1以上の整数だけを受け付ける(0・負数・小数・文字は入力エラー) <sub>`wire numbering dialog store`</sub>
- 参照ベースの「ゾーン基準」はまだ選べない(M4予定) <sub>`wire numbering dialog store`</sub>
- 採番実行はコマンドを1回だけ送り、採番したネット数を返して閉じる <sub>`wire numbering dialog store`</sub>
- 開始番号が不正なままでは採番せず、ダイアログは開いたまま残る <sub>`wire numbering dialog store`</sub>
- キャンセルするとコマンドは送られずに閉じる <sub>`wire numbering dialog store`</sub>
- 採番に失敗したらダイアログは開いたままエラーを表示する <sub>`wire numbering dialog store`</sub>

### xref

- ゾーンアドレスは行の英字(上から)と列の数字(左から)を組み合わせた「B3」形式になる <sub>`zoneAt`</sub>
- 図枠の外にある点は、無効なアドレスにはならず最も近いゾーンに丸められる <sub>`zoneAt`</sub>
- ラベルの相手先は、他のシートにある同名ラベルの住所「/シート.ゾーン」になる <sub>`cross references`</sub>
- 相手側のシートのラベルからも元のシートが見えるので、双方に相手先が表示される <sub>`cross references`</sub>
- 同じネットが複数のシートに続くときは、相手先の住所が全て列挙される <sub>`cross references`</sub>
- 自分のシート内の所在は、同名ラベルが2つあっても相手先には出ない <sub>`cross references`</sub>
- 他のシートに相手がいないラベルには、クロスリファレンスが一切表示されない <sub>`cross references`</sub>
- 相手先の一覧はジャンプ先のシートidとラベルidを持つので、クリックで飛べる <sub>`cross references`</sub>
- シートごとのクロスリファレンス表は、各ラベルのidを脇に描くテキストへ対応付ける <sub>`cross references`</sub>
- 相手先をクリックすると、相手のシートへ切り替えて相手のラベルを選択・ズームする指示になる <sub>`xrefJumpTarget`</sub>
- クロスリファレンスのテキストは、ラベル本文の右側に同じベースラインで並ぶ <sub>`xrefTextAt`</sub>

### propertyCommands

- 線番を書き換えて確定するとset_wire_numbersコマンドが1件だけ組み立てられる(undoで戻せる) <sub>`wireNumberCommand`</sub>
- 線番を空にして確定すると、その配線の線番を消すコマンドになる <sub>`wireNumberCommand`</sub>
- 前後の空白は落とされ、空白だけの入力は線番を消す扱いになる <sub>`wireNumberCommand`</sub>
- 線番が変わっていなければコマンドを送らない(無駄なundo履歴を作らない) <sub>`wireNumberCommand`</sub>
- 配線以外を選んでいるときは線番コマンドを作らない <sub>`wireNumberCommand`</sub>
- ハーネス名を書き換えるとupdate_entityコマンドになり、囲みの形はそのまま残る <sub>`harnessUpdateCommand`</sub>
- 備考だけを変えたときもコマンドになる <sub>`harnessUpdateCommand`</sub>
- 名前の前後の空白は落とされる <sub>`harnessUpdateCommand`</sub>
- 名前も備考も変わっていなければコマンドを送らない(無駄なundo履歴を作らない) <sub>`harnessUpdateCommand`</sub>
- ハーネス以外を選んでいるときはハーネスコマンドを作らない <sub>`harnessUpdateCommand`</sub>

### drawingContext

- 選択が無ければコンテキストはシート全体になる <sub>`drawingContextTag`</sub>
- シートが無くても文字列は壊れない <sub>`drawingContextTag`</sub>
- 選択があれば参照記号を列挙する(無ければ種別名) <sub>`drawingContextTag`</sub>
- 10件を超える選択は「+N件」に畳まれる <sub>`drawingContextTag`</sub>
- アクティブシートに無い選択idは無視される(全て外れれば全体扱い) <sub>`drawingContextTag`</sub>
- 空の下書きにはそのまま挿入される <sub>`appendContextTag`</sub>
- 既存の下書きとは改行で区切る(改行済みなら重ねない) <sub>`appendContextTag`</sub>

### i18n

- UIの既定言語は英語で、フォールバックも英語 <sub>`i18n`</sub>
- 実装済みロケールは英語と日本語 <sub>`i18n`</sub>
- 英語と日本語のカタログはキーが完全に一致する(訳し漏れをCIで検出) <sub>`i18n`</sub>
- カタログの文字列は空にできない(キーだけ足して訳し忘れることを防ぐ) <sub>`i18n`</sub>
- 言語タグは大文字・余白があっても解決でき、未知・空の値は英語になる <sub>`i18n`</sub>

### chat

- 既知の会話ではtext_deltaが進行中メッセージへ連結される <sub>`chat store: applyAgentEvent`</sub>
- turn_completedでストリーミングを解除し、トークン使用量を確定する <sub>`chat store: applyAgentEvent`</sub>
- デルタが来なかった場合はturn_completedのresultで本文を埋める <sub>`chat store: applyAgentEvent`</sub>
- ツール呼び出しはチップになり、idで成功/失敗が確定する <sub>`chat store: applyAgentEvent`</sub>
- 同じtool_use_idの重複開始は無視される <sub>`chat store: applyAgentEvent`</sub>
- id無しのtool_use_finishedは同名の実行中チップへ対応付けられる <sub>`chat store: applyAgentEvent`</sub>
- errorイベントはメッセージにエラーを付与し、ストリーミングを解除する <sub>`chat store: applyAgentEvent`</sub>
- turn_appliedでrevision範囲を記録し、適用済みとして扱う <sub>`chat store: applyAgentEvent`</sub>
- turn_appliedにundo深さがあれば正確な編集件数を記録する <sub>`chat store: applyAgentEvent`</sub>
- イベントを畳み込んだ会話はupdated_atが進む(履歴の最新順に反映) <sub>`chat store: applyAgentEvent`</sub>
- 完了後の新しいデルタは新しいターンを開始する <sub>`chat store: applyAgentEvent`</sub>
- 複数会話は独立に畳み込まれ、片方が進行中ならstreamingを保つ <sub>`chat store: applyAgentEvent`</sub>
- 未知のconversation_idでは幽霊会話を作らず一覧を取り直す <sub>`chat store: applyAgentEvent`</sub>
- 自分のターンが進行中の間は、未知会話のイベントで一覧を取り直さない <sub>`chat store: applyAgentEvent`</sub>
- 進行中ターンの無い会話へのturn_completed/errorは捨てられる <sub>`chat store: applyAgentEvent`</sub>
- sendは会話を新規作成し、サーバー採番のidを引き取る <sub>`chat store: アクション`</sub>
- send解決前に届いたイベントも同じ会話へ入る <sub>`chat store: アクション`</sub>
- 空プロンプトとストリーミング中の送信は無視される <sub>`chat store: アクション`</sub>
- send失敗時はメッセージにエラーを載せてストリーミングを解除する <sub>`chat store: アクション`</sub>
- 初回送信に失敗した会話でも、再送はローカルidを渡さず新規として送れる <sub>`chat store: アクション`</sub>
- cancelは対象の会話だけを止め、他会話のストリーミングは残す <sub>`chat store: アクション`</sub>
- 採番前(local-)の会話ではcancel APIを呼ばずローカル整理だけ行う <sub>`chat store: アクション`</sub>
- 採番前のcancelは採番後にサーバーへ中断を送る <sub>`chat store: アクション`</sub>
- cancel APIが失敗してもストリーミング解除は完了する <sub>`chat store: アクション`</sub>
- 会話が無い状態のcancelは何もせず落ちない <sub>`chat store: アクション`</sub>
- undoTurnはAPIを呼び、適用済み表示を取り下げる <sub>`chat store: アクション`</sub>
- undoTurnのサーバー拒否は呼び出し元へ投げられ、適用済み表示は変わらない <sub>`chat store: アクション`</sub>
- 採番前(local-)の会話ではundoTurn APIを呼ばない <sub>`chat store: アクション`</sub>
- loadConversationsはRust表現を表示用モデルへ正規化する <sub>`chat store: アクション`</sub>
- normalizeConversationは未完了ツールをrunningとして扱う <sub>`chat store: アクション`</sub>
- normalizeConversationはupdated_atをそのまま引き継ぐ <sub>`chat store: アクション`</sub>
- subscribeを同時に呼んでも購読は1本だけになる <sub>`chat store: アクション`</sub>
- 購読の解決前にunsubscribeしても取りこぼさず閉じられる <sub>`chat store: アクション`</sub>
- 購読に失敗しても次のsubscribeで張り直せる <sub>`chat store: アクション`</sub>
- newConversationは採番待ち(pendingLocalId)を巻き込まない <sub>`chat store: アクション`</sub>
- setModel / setPanel / newConversationがそれぞれの状態を更新する <sub>`chat store: アクション`</sub>
- 表示時にMCPツール名のプレフィックスを外す <sub>`summarizeToolUse`</sub>
- place_symbolはシンボル・参照記号・位置で要約される <sub>`summarizeToolUse`</sub>
- draw_wireは線色・線径・頂点数で要約される <sub>`summarizeToolUse`</sub>
- update_entity / execute_commandsはコマンド内容で要約される <sub>`summarizeToolUse`</sub>
- 読み取り系・書き出し系ツールも適切に要約される <sub>`summarizeToolUse`</sub>
- 要約を作れないツールは空文字になり、表示はツール名のみ <sub>`summarizeToolUse`</sub>
- 会話タイトルは最初のユーザー発話の先頭40字 <sub>`会話履歴ポップアップの表示ヘルパー`</sub>
- ユーザー発話が無ければ「(空の会話)」になる <sub>`会話履歴ポップアップの表示ヘルパー`</sub>
- 相対時刻は たった今/N分前/N時間前/昨日/M-D で表示される <sub>`会話履歴ポップアップの表示ヘルパー`</sub>
- 時刻不明(旧履歴のupdated_at=0)は相対時刻を出さない <sub>`会話履歴ポップアップの表示ヘルパー`</sub>
- メタ行は時刻・件数・適用済みrevを中黒で連ねる <sub>`会話履歴ポップアップの表示ヘルパー`</sub>
- 会話は更新の新しい順に並ぶ(時刻不明は後ろに登録順) <sub>`会話履歴ポップアップの表示ヘルパー`</sub>

### document

- entity_upserted / entity_removed パッチがミラーのシートを更新する <sub>`document store`</sub>
- project_replacedでミラーのプロジェクト全体が置き換わる <sub>`document store`</sub>
- シート追加・削除のパッチは並び順を保つ <sub>`document store`</sub>
- シートメタ更新のパッチはエンティティに触れない <sub>`document store`</sub>
- 古いrevisionのパッチは破棄される(二重配信しても安全) <sub>`document store`</sub>

### 部品データベース

- searchは部品APIの結果を保持する <sub>`parts store`</sub>
- 検索失敗時は結果を空にしloadingを戻す <sub>`parts store`</sub>

### revisions

- 改訂が1件も無いシートで行を追加すると、記号Aと今日の日付が入る <sub>`revisions dialog store`</sub>
- 既に改訂がある場合、追加行の記号は既存の最大記号の次のアルファベットになる <sub>`revisions dialog store`</sub>
- 記号がZまで進んだらAAへ繰り上がる <sub>`revisions dialog store`</sub>
- 記号が空欄や数字だけの行があっても採番は壊れず、アルファベットの続きが入る <sub>`revisions dialog store`</sub>
- 今日の日付はYYYY-MM-DD形式で入る <sub>`revisions dialog store`</sub>
- 表は最新の改訂が一番上に並ぶ(図枠の改訂欄と同じ並び) <sub>`revisions dialog store`</sub>
- ダイアログを開いてもシートの改訂は書き換わらない(編集は下書きの上だけ) <sub>`revisions dialog store`</sub>
- 保存すると編集後の一覧を積んだset_revisionsコマンドが1回だけ送られる <sub>`revisions dialog store`</sub>
- キャンセルするとコマンドは送られず、編集内容は捨てられる <sub>`revisions dialog store`</sub>
- 行の削除と内容の書き換えは保存する一覧に反映される <sub>`revisions dialog store`</sub>
- 何も編集せずに保存したときはコマンドを送らない(無駄なundo履歴を作らない) <sub>`revisions dialog store`</sub>
- 全欄が空のまま残った行は保存時に取り除かれ、前後の空白も落とされる <sub>`revisions dialog store`</sub>
- 保存に失敗したらダイアログは開いたままエラーを表示する <sub>`revisions dialog store`</sub>

### AI設定

- 既定値は自動適用・図面自動読み取りがON <sub>`settings store`</sub>
- loadでバックエンドの設定を取り込む <sub>`settings store`</sub>
- saveは変更分をマージして送り、正規化後の戻り値を採用する <sub>`settings store`</sub>
- save失敗時はエラーを保持し、表示中の設定を変えない <sub>`settings store`</sub>

### simulation

- runでDC解析結果を取得してパネルを開く <sub>`simulation store`</sub>
- 開閉トグルは再実行時にopen_switchesとして渡される <sub>`simulation store`</sub>
- 失敗時(ngspice未導入等)はエラーメッセージを保持したままパネルを開く <sub>`simulation store`</sub>

### ui

- 既定はプロジェクトタブ+チャット折りたたみ <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- openAgentTabでエージェントタブ切替とチャット展開が同時に行われる <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- closeAgentTabでプロジェクトタブへ戻り、チャットも畳まれる <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- チャット下書きはドックと浮きカードで共有される <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- 既定では全表示クラスが表示される <sub>`表示クラス (レイヤ)`</sub>
- toggleViewClassで表示クラスの非表示・再表示を切り替える <sub>`表示クラス (レイヤ)`</sub>

### verification

- runで診断を取得してパネルを開き、severity別に件数を数える <sub>`verification store`</sub>
- closeはパネルを閉じるが診断は保持する <sub>`verification store`</sub>
- 取得失敗時はrunningが戻り、診断は空のまま <sub>`verification store`</sub>

