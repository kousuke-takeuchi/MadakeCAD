# 詳細仕様設計書

**English (canonical): [13-specification.md](13-specification.md)** | ← [ロードマップ](12-roadmap.ja.md)

> **テストスイートから自動生成 — 手で編集しないこと。**
> 以下の各項目は自動テストで常に検証されている(灰色はテストID)。
> テスト変更後は `python3 scripts/gen_spec.py` で再生成する。

本書はMadakeCADの「生きた仕様書」である:
ここに載っている挙動は、テスト実行のたびに証明される。


全5領域・**1144仕様項目**。


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
- 存在しないエンティティの更新は失敗し、その要素がシートへ紛れ込むこともない。 <sub>`updating_a_missing_entity_fails_without_inserting_it`</sub>
- 履歴の各エントリは編集の由来(誰の編集か)を記録し、通常のexecute()はユーザー編集として扱う。 <sub>`execute_as_records_the_edit_origin`</sub>
- 区間の巻き戻しはその中のエージェント編集だけを戻し、間に挟まったユーザー編集はそのまま残す。 <sub>`revert_range_rolls_back_agent_edits_and_keeps_user_edits`</sub>
- 巻き戻しも通常の編集として履歴に乗るため、undoすればエージェントの編集が戻ってくる。 <sub>`revert_range_is_itself_undoable`</sub>
- エージェントの編集を現在の図面へ逆適用できない場合(対象をユーザーが消した等)、巻き戻し全体を拒否し何も変更しない。 <sub>`revert_range_refuses_conflicting_reverts_without_partial_changes`</sub>
- バッチとしてまとめて実行したコマンド列は履歴1件になり、undo一発で全部消え、redo一発で全部戻る。 <sub>`execute_batch_is_undone_in_one_step`</sub>
- バッチ内の1つでも失敗したらバッチ全体を拒否し、図面は変わらず履歴にも残らない。 <sub>`execute_batch_refuses_everything_when_one_command_fails`</sub>
- 空のバッチは何も変えない(履歴も増えず、ドキュメントrevisionも進まない)。 <sub>`empty_batch_changes_nothing`</sub>
- その由来の編集が1件も無い区間の巻き戻しは、図面に触れず「戻すものが無い」と報告する。 <sub>`revert_range_without_matching_edits_changes_nothing`</sub>

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

### 回路マクロ

- マクロの保存では全座標が基準点からの相対座標になり、基準点は選択範囲の左下に最も近いピンになる。 <sub>`saving_a_macro_uses_the_bottom_left_pin_as_the_base_point`</sub>
- 選択範囲にシンボルのピンが1つも無いときは、基準点はバウンディングボックスの左下角になる。 <sub>`the_base_point_falls_back_to_the_bounding_box_corner_without_pins`</sub>
- マクロの保存では線番は捨てられ(図面ごとに振り直すため)、ネットラベルは回路の一部としてそのまま残る。 <sub>`saving_a_macro_drops_wire_numbers_but_keeps_net_labels`</sub>
- 選択が空のマクロ保存は拒否され、そのシートに無いエンティティidは黙って飛ばさずエラーになる。 <sub>`saving_a_macro_refuses_an_empty_or_unknown_selection`</sub>
- idを指定せずに保存したマクロは名前から安定したidが作られる(保存ダイアログは名前だけ聞けばよい)。 <sub>`a_macro_without_an_id_derives_one_from_its_name`</sub>
- マクロを挿入すると、基準点がちょうど指定した位置(カーソル位置)へ来る。 <sub>`inserting_a_macro_lands_the_base_point_on_the_cursor`</sub>
- 回転を指定して挿入すると、回路全体が挿入点を中心に回り、各シンボルの向きも一緒に回る。 <sub>`inserting_a_macro_rotated_turns_the_whole_circuit`</sub>
- マクロの挿入では参照記号が図面で使用済みの最大値の次から振り直され、マクロ内の関係は保たれる(K1/K2は別のリレーのまま、コイルとその接点は同じ記号を共有し続ける)。 <sub>`inserting_a_macro_renumbers_references_after_the_existing_ones`</sub>
- 手書きのマクロファイルに線番が残っていても挿入時に消される(線番は挿入先の図面のものだから)。 <sub>`wire_numbers_in_a_macro_file_are_cleared_on_insert`</sub>
- マクロは代替バリアントを持てる。既定のcommandsがバリアント「A」で、他のキーを指定するとそのバリアントの回路が入る。 <sub>`a_variant_can_be_chosen_when_inserting`</sub>
- 知らないバリアントキーはキーを添えたエラーで拒否され、図面は変わらない。 <sub>`an_unknown_variant_key_is_refused`</sub>
- マクロの挿入は1回の編集なので、undo一発で回路全体が消え、redo一発で戻ってくる。 <sub>`inserting_a_macro_is_undone_in_one_step`</sub>
- 回路をマクロとして保存し基準点の位置へ挿入し直すと、図面がそのまま再現される(ピン・配線・接続点・ラベルが同じ座標に来る)。 <sub>`a_saved_macro_reproduces_the_drawing_when_inserted_back`</sub>
- モータ回路のマクロは別シートへ2回挿入でき、2つのコピーが参照記号を取り合うことはなく、図面は検証でエラー0・ERC警告0のまま通る。 <sub>`a_macro_inserted_twice_keeps_references_unique_and_passes_verification`</sub>
- マクロはユーザーのマクロフォルダへ書き出され、そこから一覧される。マクロとして読めないファイルはパスと理由を添えて報告され、他のマクロはそのまま使える。 <sub>`macros_round_trip_through_the_user_folder_and_broken_files_are_reported`</sub>
- 値セットが無かった頃のマクロファイルもそのまま読めて挿入できる(プレースホルダも値セットも無いマクロとして扱われる)。 <sub>`a_macro_file_without_placeholders_still_loads_and_inserts`</sub>
- 挿入時に値セットを選ぶと、プレースホルダが指すシンボルの型番・値の欄へその値が書き込まれる。 <sub>`a_chosen_value_set_fills_in_the_value_field_of_its_targets`</sub>
- プレースホルダは型番欄の代わりに属性 (attrs.<名前>) も指せるので、定格や部品番号をそれぞれの欄へ入れられる。 <sub>`a_placeholder_can_write_into_a_named_attribute`</sub>
- 値セットの1つの値はプレースホルダの全対象へ一度に届き(複数のシンボル・複数の欄)、値セットは複数のキーを持てる。 <sub>`one_value_set_updates_every_target_of_every_key_at_once`</sub>
- 値セットを選ばずに挿入すると、各欄は保存したときのままになる。 <sub>`inserting_without_a_value_set_keeps_the_saved_values`</sub>
- 知らない値セットidはidを添えたエラーで拒否され、図面には何も置かれない。 <sub>`an_unknown_value_set_id_is_refused_and_places_nothing`</sub>
- マクロに無いエンティティを指す値セットは拒否され、他の値も一切適用されない(中途半端に埋まった回路は作らない)。 <sub>`a_target_that_is_not_in_the_macro_is_refused_without_applying_anything`</sub>
- マクロが宣言していないキーを持つ値セットは、そのキーを添えたエラーで拒否される。 <sub>`a_value_set_key_without_a_placeholder_is_refused`</sub>
- 選択範囲の外を指すプレースホルダや、存在しない欄へ書こうとするプレースホルダは保存時に拒否される(届かない対象を持つマクロは作れない)。 <sub>`saving_refuses_a_placeholder_target_outside_the_selection_or_an_unknown_field`</sub>
- 値セットを選んで挿入しても編集は1回のままなので、undo一発で値ごと回路全体が戻る。 <sub>`inserting_with_a_value_set_is_still_undone_in_one_step`</sub>
- プレースホルダと値セットはマクロフォルダへの往復でも残るので、保存したマクロは後からどの値セットでも挿入できる。 <sub>`placeholders_and_value_sets_round_trip_through_the_user_folder`</sub>
- プロジェクトに無いシートへの挿入は拒否され、知らないマクロidは名前を添えて拒否される。 <sub>`inserting_into_an_unknown_sheet_or_by_an_unknown_id_is_refused`</sub>

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
- シンボル内の弧はシンボルと一緒に回るので、電磁接触器の半円はどの回転角でも可動接点の側を向き続け、ミラーでは描画方向を保ったまま左右が入れ替わる。 <sub>`arc_angles_follow_symbol_rotation_and_mirror`</sub>

### ngspiceランナー

- ngspiceの'print all'出力はノード電圧(v(n1)→n1)と枝電流(v1#branch)に解釈され、無関係な行は無視される。 <sub>`parse_print_all_reads_nodes_and_branches`</sub>
- ngspiceの実行ファイルは MADAKE_NGSPICE環境変数→PATH→OS既定パス の優先順で探索され、存在しないenv指定は次へフォールバックする。 <sub>`find_in_prefers_env_then_path_then_candidates`</sub>
- 24V・6+6Ωの分圧回路のDC動作点は中点12V・電源電流2Aになる(ngspice必須。未導入時はスキップ)。 <sub>`run_op_solves_a_divider_when_ngspice_is_installed`</sub>

### 部品データベース

- 新規DBを開くとスキーマ作成とサンプル投入が一度だけ行われ、開き直しても再投入されない。 <sub>`open_creates_schema_and_seeds_samples_once`</sub>
- 部品は登録・型番キーでの更新・取得ができ、名称の部分一致やカテゴリ完全一致で検索できる。 <sub>`upsert_get_and_search`</sub>
- 電線品番は登録でき、線色+線径の完全一致で引き当てられる。 <sub>`wire_parts_crud_and_lookup`</sub>
- 旧スキーマv1のDBは開いた時点でv2へ移行され、既存データを保持したままspice_model列が使えるようになる。 <sub>`v1_database_migrates_to_v2_preserving_data`</sub>
- 旧スキーマv2のDBは開いた時点でv3へ移行され、既存データを保持したままcontact_config列が使えるようになる。 <sub>`v2_database_migrates_to_v3_preserving_data`</sub>
- 同梱のサンプルリレーは接点構成を持っているので、配置直後から接点数超過の検証ができる。 <sub>`sample_relay_part_has_a_contact_configuration`</sub>
- DBパスの既定はOSのアプリデータフォルダで、MADAKE_PARTS_DBで上書きできる。 <sub>`default_path_respects_env_override`</sub>

### PDF出力

- PDF出力は日本語を含む正しいPDF文書(%PDF-ヘッダ)を非自明なサイズで生成する。 <sub>`sheet_to_pdf_produces_pdf_bytes`</sub>
- PDFのページ寸法は用紙そのもの (A3横=420×297mm) になり、100%で印刷すると原寸になる。 <sub>`pdf_page_is_the_size_of_the_paper`</sub>
- PDF一括出力のページは 表紙 → 回路図の全シート → 選択した帳票 の順に並ぶ。 <sub>`pdf_book_is_cover_then_sheets_then_reports`</sub>
- PDF一括出力には端子接続図も入れられ、指定した順で帳票ページとして並ぶ。 <sub>`pdf_book_can_include_the_terminal_diagram`</sub>
- 帳票を選ばなければPDF一括出力は表紙と回路図シートだけになる。 <sub>`pdf_book_without_reports_is_cover_and_sheets_only`</sub>
- 表紙は外すことができ、その場合は回路図シートが先頭になる。 <sub>`pdf_book_can_omit_the_cover`</sub>
- 一括出力は全ページを1つのPDF文書にまとめ、回路図がA3でも帳票ページはA4になる。 <sub>`pdf_book_merges_every_page_into_one_document`</sub>
- シートが1枚も無いプロジェクトでも、表紙だけの正しい1ページPDFになる。 <sub>`pdf_book_of_an_empty_project_is_just_the_cover`</sub>

### relay_xref

- 同じ参照記号を持つコイルと接点は、1つのリレーデバイスとしてまとめられる。 <sub>`same_reference_groups_coil_and_contacts_into_one_device`</sub>
- リレーのコイル・接点以外のシンボルは、リレーデバイスには含まれない。 <sub>`non_relay_symbols_are_not_relay_devices`</sub>
- デバイスの接点はシート順→ゾーン順に並ぶので、端子対の連番は図面の読み順どおりになる。 <sub>`contacts_are_ordered_by_sheet_then_zone`</sub>
- 端子対はIEC 60947-1に従い、先頭の数字が接点の連番、末尾がa接点なら3/4・b接点なら1/2になる。 <sub>`terminal_pairs_follow_iec_position_and_function_digits`</sub>
- コイル下の接点マップには、配置済みの接点が端子対と図面上の住所とともに並ぶ。 <sub>`contact_map_lists_used_contacts_with_their_addresses`</sub>
- 部品の接点構成が分かっているときは、まだ使っていない接点も行として並び、所在の代わりに「—」が入る。 <sub>`contact_map_shows_dash_for_unused_contacts`</sub>
- 接点構成が分からないときは実際に描かれた接点だけが並び、空の行は作られない。 <sub>`contact_map_omits_unused_rows_without_contact_config`</sub>
- それぞれの接点の脇には、その接点を動かすコイルの住所が丸括弧付きで出る。 <sub>`contact_shows_the_location_of_its_coil`</sub>
- コイルが見つからない接点には、コイル所在が一切表示されない。 <sub>`contact_without_a_coil_shows_no_location`</sub>
- シートごとの接点マップはコイルのentity idで引くので、そのシートに居るコイルだけが表を持つ。 <sub>`sheet_contact_maps_are_keyed_by_the_coil_on_that_sheet`</sub>
- 「2NO+2NC」のような接点構成は、その部品が実際に持つa接点・b接点の数として読み取られる。 <sub>`contact_config_parses_make_and_break_counts`</sub>
- 読み取れない接点構成は、接点数を推測せずに無視される。 <sub>`unreadable_contact_config_is_ignored`</sub>
- 割り当てた部品が持つ数より多くの接点を使うとエラーになり、足りない接点の種別が示される。 <sub>`using_more_contacts_than_the_part_has_is_an_error`</sub>
- プロジェクトのどこにも同じ参照記号のコイルが無い接点はエラーになる。 <sub>`a_contact_without_a_coil_is_an_error`</sub>
- 接点を1つも動かしていないコイルは、これから足す可能性があるので警告にとどまる。 <sub>`a_coil_without_contacts_is_a_warning`</sub>
- 読み取れない接点構成は、接点数の検証が黙って効かなくなるため警告として報告される。 <sub>`an_unreadable_contact_config_is_a_warning`</sub>
- 正しく組まれたリレー(コイル+接点構成の範囲内の接点)には、クロスリファレンスの指摘が一切出ない。 <sub>`a_correct_relay_produces_no_diagnostics`</sub>
- プロジェクト全体の検証には、コイル⇔接点クロスリファレンスの検査が含まれる。 <sub>`project_verification_includes_relay_checks`</sub>
- 接点マップの表はコイルの真下に中央揃えで置かれ、接点1個につき1行ずつ下へ伸びる。 <sub>`contact_map_table_is_centred_under_the_coil`</sub>
- 接点マップはコイルの外形の下にぶら下がり、コイル所在の文字は接点シンボルの右脇に置かれる。 <sub>`annotations_are_anchored_to_the_symbol_outline`</sub>

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
- 帳票をCSVで書き出すと図面シートと同じ表が返り、書き出した本文の行数も分かる。 <sub>`a_csv_export_returns_the_table_and_its_row_count`</sub>
- 対象を絞らずに端子台チャートを出すと、どの端子台の行かが分かるよう先頭列に参照記号が入る。 <sub>`a_project_wide_terminal_chart_names_the_terminal_block_in_each_row`</sub>
- 帳票をPDFで書き出すと図枠付きの図面ページが1つのPDFにまとまり、ページ数が分かる。 <sub>`a_pdf_export_binds_the_framed_pages_into_one_file`</sub>
- 端子接続図は図面なのでCSVでは出せず、その旨を返す。 <sub>`the_terminal_connection_diagram_has_no_csv_form`</sub>
- 端子台ではないものを対象にすると、空のファイルを書かずにエラーになる。 <sub>`a_terminal_report_of_an_unknown_block_fails`</sub>
- 端子台以外の帳票は端子台の指定を無視し、常にプロジェクト全体を対象にする。 <sub>`other_reports_always_cover_the_whole_project`</sub>
- クロスリファレンス表のCSVの見出しは、図面シート版の列見出しと同じ。 <sub>`the_cross_reference_csv_has_the_same_columns_as_its_sheet`</sub>
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

### search

- 1回の検索で5種すべて(参照記号・型番・ネット名・線番・テキスト)を対象にする。 <sub>`search_covers_all_five_targets`</sub>
- クエリは語の一部にも当たり、大文字小文字も区別しない。「my2」で型番「MY2N」が見つかる。 <sub>`search_is_case_insensitive_and_partial`</sub>
- フィルタチップで対象を絞ると、その種別だけが残る。「K1」を参照記号だけで探すと注記のヒットは消える。 <sub>`kinds_filter_narrows_the_targets`</sub>
- 空のクエリは何も返さない(図面全体を並べたりしない)。空欄のフィールドもヒットしない。 <sub>`empty_query_finds_nothing`</sub>
- 結果は図面の読み順(シート番号→ゾーン→エンティティ)で返るので、同じ図面なら並びは常に同じになる。 <sub>`results_are_in_reading_order`</sub>
- ヒットは所在(シート・ゾーン・選択するエンティティ)を持ち、住所を「/シート.ゾーン」の形で表す。 <sub>`hit_carries_its_location`</sub>
- 型番のヒットはどの部品のものかを、参照記号のヒットはその部品の型番を、それぞれ添えて返す。 <sub>`hits_carry_the_partner_field_as_detail`</sub>
- シンボルのヒットは、そのシンボルがデバイスの中で果たす機能も返すので、結果一覧に「リレー 接点 13-14」と出せる。 <sub>`symbol_hits_say_what_the_symbol_does`</sub>
- 接点構成のような属性の値も、型番と一緒に検索できる。 <sub>`attribute_values_are_searched_as_part_numbers`</sub>
- リレーのデバイスはコイルを先に、続けて接点を並べ、端子の呼び名は接点マップと同じになる。 <sub>`relay_device_lists_coil_then_contacts`</sub>
- 端子台は全極をまとめた「端子」1行として並び、極数はデバイス側に持つ。 <sub>`terminal_block_device_shows_its_terminal_range`</sub>
- 機能に分かれない普通の部品は、「本体」1行だけのデバイスとして、シンボルの名前つきで出る。 <sub>`plain_part_is_a_single_body_row`</sub>
- デバイスは参照記号でまとまるので、複数シートに散っていても1デバイスになる。参照記号の無いシンボルはデバイスにならない。 <sub>`devices_group_by_reference_across_sheets`</sub>
- 空のプロジェクトにデバイスは無く、ツリーは参照記号の順に並ぶ。 <sub>`device_tree_is_ordered_and_can_be_empty`</sub>
- 機能の行はジャンプ先のシートとゾーンを持つ。ナビゲータと参照サーフィンはこれを使って図面を表示する。 <sub>`every_function_knows_where_to_jump`</sub>
- フィルタ名はAPI上ではsnake_caseの文字列で、未知の名前は推測せずに拒否する。 <sub>`filter_names_parse_from_the_api`</sub>

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
- コイルのあるシートには、コイルの下に接点マップ(端子対と所在の行)が描かれる。 <sub>`svg_draws_the_contact_map_under_the_coil`</sub>
- 接点のあるシートには、その脇にコイルの住所が丸括弧付きで描かれる。 <sub>`svg_draws_the_coil_location_beside_the_contact`</sub>
- シート単体の書き出しでは相手のシートが分からないので、接点マップは描かれない。 <sub>`svg_of_a_lone_sheet_has_no_contact_map`</sub>

### シンボルライブラリ

- 同梱シンボルはすべて一意のidを持ち、最低1つのピンを持つ。 <sub>`builtin_symbols_have_unique_ids_and_pins`</sub>
- ピン数可変の端子台(例: 8極)は端子ごとに左右1点ずつの接続点を持ち、すべて2.5mmグリッド上・上下中央揃えになる。 <sub>`dynamic_terminal_block_has_through_pins_on_grid`</sub>
- 動的生成のconnector_2pは旧静的定義と完全に同じピン座標を持ち、既存図面に影響しない。 <sub>`dynamic_connector_2p_matches_legacy_static_def`</sub>
- resolve_symbolは同梱idを見つけ、不正・範囲外の動的ID(0極・51極・数値なし)は拒否する。 <sub>`resolve_symbol_rejects_invalid_ids_and_finds_builtins`</sub>
- sheet_symbol_defsは同梱ライブラリに加え、シートで実際に使われている動的シンボルの定義を返す。 <sub>`sheet_symbol_defs_includes_dynamic_ids_in_use`</sub>
- リレーコイルのシンボルは、JISのコイル端子記号A1・A2と参照記号の接頭辞Kを持つ。 <sub>`relay_coil_has_a1_a2_terminals`</sub>
- リレー接点はa接点・b接点の2種類があり、コイルと同じ参照記号の接頭辞を持ち、2つの接続点が2.5mmグリッド上にある。 <sub>`relay_contacts_come_in_make_and_break_types`</sub>
- シンボル定義はJSONに往復変換しても失われない。 <sub>`symbol_json_roundtrip`</sub>
- 同梱ライブラリはJIS C 0617の主要記号を50種規模で網羅し、idはすべて一意である。 <sub>`library_covers_the_main_jis_symbols`</sub>
- すべてのピンは2.5mmグリッド上にあり、接続点ごとに番号が決まっていて、シンボル本体の外側を向いている。 <sub>`every_pin_is_on_the_grid_and_points_outwards`</sub>
- すべてのシンボルは4つの属性スロット(参照記号・型番・説明・定格)を持ち、外形に重ならない位置に置かれる。 <sub>`every_symbol_exposes_the_standard_attribute_slots`</sub>
- シンボルは部品挿入ダイアログの表示順にカテゴリごとまとまって並び、英語・日本語のキーワードで検索できる。 <sub>`symbols_are_grouped_by_category_in_display_order`</sub>
- 初版から同梱している記号はid・カテゴリ・参照記号の接頭辞・ピン番号・ピン位置が変わらないため、ライブラリ拡充前に描いた図面もそのまま同じに描画される。 <sub>`legacy_symbols_keep_their_definition`</sub>
- 電磁接触器(3極)は極ごとにコンタクタの半円付きa接点を描き、端子は1/2・3/4・5/6、極をつなぐ連動線は破線で表す。 <sub>`contactor_3p_has_three_ganged_main_contacts`</sub>
- 単極の配線用遮断器(MCB)は固定接点に遮断器の×印を付けたa接点で、端子は1と2である。 <sub>`circuit_breaker_1p_marks_the_fixed_contact_with_a_cross`</sub>
- 2極・3極の配線用遮断器は極を5mmピッチで並べ、端子を1/2・3/4(・5/6)と振る。 <sub>`circuit_breaker_multipole_numbers_terminals_by_pole`</sub>
- 断路器は固定接点を、遮断器の×印ではなく可動接点に直交する短い棒で表す。 <sub>`disconnector_marks_the_fixed_contact_with_a_bar`</sub>
- 3極の断路器は3つの可動接点を連動させ、端子は1/2・3/4・5/6になる。 <sub>`disconnector_3p_gangs_three_blades`</sub>
- b接点の押しボタンは、a接点と同じ2つの接続点を持ち、押しボタンの操作子が付く。 <sub>`pushbutton_nc_is_a_break_contact_with_a_button_actuator`</sub>
- 非常停止は、きのこ形の頭部を持つb接点で、叩くと回路が開く。 <sub>`emergency_stop_is_a_break_contact_with_a_mushroom_head`</sub>
- 切替スイッチは共通端子1つと固定接点2つを持ち、操作していない状態では可動接点がb接点側に載っている。 <sub>`switch_spdt_has_a_common_and_two_fixed_contacts`</sub>
- 3位置切替スイッチは可動接点が中立位置にあり、どちらの固定接点にも接触していない状態で描く。 <sub>`switch_3pos_shows_the_blade_in_neutral`</sub>
- リミットスイッチはa接点・b接点の2種類があり、どちらも位置スイッチの操作子(ロッド先端の塗りつぶし四角)で動く。 <sub>`limit_switch_comes_in_make_and_break_types`</sub>
- リレーの切替接点(c接点)は、IECの端子番号11(共通)・12(b接点)・14(a接点)を使う。 <sub>`relay_contact_co_uses_iec_terminal_numbers`</sub>
- 変圧器(2巻線)は鉄心を挟んで両側に巻線を半円で描き、一次側が端子1/2、二次側が端子3/4になる。 <sub>`transformer_has_two_windings_around_a_core`</sub>
- 交流電源は円の中に正弦波を描いた記号で、接続点は2つである。 <sub>`ac_source_is_a_circle_with_a_sine_wave`</sub>
- 整流器(ブリッジ)は菱形の中にダイオードを描いた記号で、交流側が左右、直流側が上(+)と下(-)の端子になる。 <sub>`rectifier_bridge_has_ac_and_dc_terminals`</sub>
- 保護接地は接地記号を円で囲み、機能接地(フレーム接地)はシャーシ記号で表す。どちらも接続点は1つ。 <sub>`earth_symbols_distinguish_protective_and_frame_earth`</sub>
- ツェナーダイオードはダイオードの形を保ちつつ、陰極バーの両端を折り曲げた形で描く。 <sub>`zener_diode_bends_the_cathode_bar`</sub>
- 有極性コンデンサは片方の極板を塗りつぶし、プラス側の端子を「+」で示す。 <sub>`capacitor_polarized_marks_the_positive_plate`</sub>
- インダクタ(コイル)は導線の上に並んだ半円で描く。 <sub>`inductor_is_a_row_of_half_circles`</sub>
- 可変抵抗器は抵抗器の外形を斜めに貫く矢印を加えた形で描く。 <sub>`resistor_variable_adds_an_arrow_across_the_body`</sub>
- バリスタは抵抗器を斜線が貫き「U」を添えた形で、電圧に依存する抵抗であることを示す。 <sub>`varistor_is_a_voltage_dependent_resistor`</sub>
- 三相電動機は円の上側にU・V・Wの3つの相端子を持つ。 <sub>`motor_3ph_has_u_v_w_terminals`</sub>
- 単相電動機と直流電動機は電動機の円を共有し、中に書く「1~」と直流記号で区別する。 <sub>`motor_1ph_and_dc_are_told_apart_by_the_mark_inside`</sub>
- ベルは底辺の上に半円を載せたドーム形で、接続点は2つである。 <sub>`bell_is_a_dome_with_two_terminals`</sub>
- 電圧計・電流計は「V」「A」を書いた円で、2端子の計器として回路に入れる。 <sub>`voltmeter_and_ammeter_are_circles_marked_v_and_a`</sub>
- 変流器は一次導体が鉄心を貫き、二次側にS1・S2の2端子を持つ。 <sub>`current_transformer_has_primary_through_and_secondary_terminals`</sub>
- 差込接続器のプラグとソケットは向かい合う形で、プラグはくさび形、ソケットはそれを受ける半円形になる。 <sub>`connector_plug_and_socket_face_each_other`</sub>
- 多極機器は極ごと(端子1-2・3-4・5-6)に導通するため、相どうしがつながっている扱いにはならない。 <sub>`multipole_devices_conduct_pole_by_pole`</sub>

### 開始テンプレート

- 同梱テンプレートは「24V制御基本・モータ起動回路・非常停止回路」の3種がこの順で並び、それぞれ英語と日本語の名前・説明を持つ。 <sub>`the_three_bundled_templates_are_listed_in_both_languages`</sub>
- リソースのディレクトリが無い環境でも、ビルドへ埋め込んだ同梱テンプレートが使える。 <sub>`templates_are_available_without_the_resource_directory`</sub>
- 「24V制御基本」を適用すると、直流電源・ヒューズ・4極端子台と24V/0Vのネットラベルが図面に入る。 <sub>`applying_the_24v_template_places_its_parts_on_the_sheet`</sub>
- テンプレートの適用は1回の編集なので、undo一発で図面が空に戻り、redo一発で全部戻ってくる。 <sub>`applying_a_template_is_undone_in_one_step`</sub>
- 同梱テンプレートはどれも検証でエラー0・ERC警告0になる(浮いたピンの無い、そのまま使える出発点)。 <sub>`every_bundled_template_verifies_without_errors_or_erc_warnings`</sub>
- 同じテンプレートは2回適用できる(2回目は新しいidが振られ、1回目と衝突しない)。 <sub>`the_same_template_can_be_applied_twice`</sub>
- テンプレートは複数シートのプロジェクトでも、指定したシートにだけ入る。 <sub>`a_template_lands_on_the_requested_sheet`</sub>
- 知らないテンプレートidはidを添えたエラーで拒否され、図面は変わらない。 <sub>`an_unknown_template_id_is_refused`</sub>
- ユーザーが自分のテンプレートフォルダに置いたテンプレートは同梱テンプレートの後に並び、同じように適用できる。 <sub>`user_templates_are_listed_after_the_bundled_ones`</sub>
- JSONとして壊れている(または知らないコマンドを含む)テンプレートは、パスと理由を添えて報告され、他のテンプレートはそのまま使える。 <sub>`a_broken_template_file_is_reported_with_its_path`</sub>
- ユーザーテンプレートは同梱テンプレートと同じidを使うことで差し替えられる(自社版が優先される)。 <sub>`a_user_template_replaces_the_bundled_one_with_the_same_id`</sub>

### 端子台チャート

- 端子台のチャートは端子1個につき1行で、端子番号の順に並ぶ。 <sub>`the_chart_has_one_row_per_terminal_in_number_order`</sub>
- 端子の左側に繋がっているものが内部側 (盤内)、右側に繋がっているものが外部側 (盤外) になる。 <sub>`the_left_side_is_the_inside_and_the_right_side_is_the_outside`</sub>
- 端子台を180度回転させると内外も入れ替わる。内部側はあくまで用紙上で左に描かれている側である。 <sub>`a_terminal_block_rotated_180_degrees_still_takes_the_paper_left_as_the_inside`</sub>
- 端子台を90度回して横向きに置くと接続点は上下に並び、上側が内部側として扱われる。 <sub>`a_sideways_terminal_block_takes_the_upper_connection_as_the_inside`</sub>
- どちら側にも電線が繋がっていない端子は、内部側・外部側とも空欄の予備端子として行が残る。 <sub>`an_unwired_terminal_stays_as_a_spare_row`</sub>
- 各行にはその電線に振られた線番と、電線の仕様 (線色・線径sq・品番) が載る。 <sub>`a_row_shows_the_wire_number_and_the_wire_specification`</sub>
- 内部側と外部側で電線が違うときは、行に両方の電線が並ぶ。 <sub>`both_wires_are_listed_when_the_inside_and_outside_differ`</sub>
- 行はまとめた電線欄とは別に、内部側の電線と外部側の電線を分けて持つ。 <sub>`a_row_keeps_the_wire_of_each_side_separately`</sub>
- 行は各側の電線が属するハーネス名を持ち、どのハーネスにも属さない電線では空欄になる。 <sub>`a_row_records_the_harness_of_each_side`</sub>
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
- 端子台エディタの一覧には、プロジェクトの全端子台がシート順・参照記号順に、所在シート・極数・ジャンパ付きで並ぶ。 <sub>`the_editor_lists_every_terminal_block_with_its_sheet_poles_and_jumpers`</sub>
- シートを指定すると、そのシートに描かれている端子台だけの一覧になる。 <sub>`naming_a_sheet_narrows_the_terminal_block_list_to_that_sheet`</sub>
- 端子台ではないシンボルは一覧に出ない。 <sub>`other_symbols_never_show_up_in_the_terminal_block_list`</sub>
- 端子台のチャートとチェックは、どのシートにあるかを知らなくてもentity idだけで引ける。 <sub>`a_terminal_block_can_be_looked_up_by_id_across_sheets`</sub>
- ジャンパは通常のupdate_entityコマンドで設定するので、チャートに反映され、undoで元に戻る。 <sub>`setting_jumpers_through_update_entity_is_undoable`</sub>

### 端子接続図

- 端子ストリップは端子1個につき1つの番号入りの箱として、端子番号の順に上から下へ縦に積まれる。 <sub>`the_strip_stacks_one_numbered_box_per_terminal`</sub>
- 盤外 (外部側) はストリップの左、盤内 (内部側) は右に描かれ、それぞれ見出しが付く。 <sub>`the_outside_is_on_the_left_and_the_inside_on_the_right`</sub>
- 何も繋がっていない端子は、薄く塗った箱に予備の注記を付けてストリップに残る。 <sub>`a_spare_terminal_stays_in_the_strip_lightly_filled`</sub>
- 同じハーネスに属する電線は引出線の外端で1つのブラケットにまとめられ、ハーネス名が添えられる。 <sub>`wires_of_one_harness_are_gathered_into_a_bracket`</sub>
- どのハーネスにも属さない電線にはブラケットが付かない。 <sub>`a_wire_without_a_harness_gets_no_bracket`</sub>
- 隣り合う端子のサドルジャンパは、端子箱の内部側の縁に縦の連結線として描かれる。 <sub>`a_jumper_is_drawn_on_the_inside_edge_of_the_boxes`</sub>
- 各引出線には、その電線の線色・線径sq・品番が線の下に小さく書かれる。 <sub>`each_lead_line_carries_the_wire_specification`</sub>
- 1ページに収まらない端子は次のページへ続き、各ページの表題にそのページの端子の範囲が入る。 <sub>`terminals_that_do_not_fit_continue_on_the_next_page`</sub>
- 端子台1つにつき1ページが出て、端子台でないものには1ページも出ない。 <sub>`one_page_per_terminal_block_and_none_for_anything_else`</sub>
- 端子接続図のページはA4横で、JIS図枠と、端子台の参照記号が入った表題欄を持つ。 <sub>`the_page_has_the_frame_and_a_title_block`</sub>
- 端子接続図はterminal-diagramという名前の帳票として選べ、CLI・Link API・MCPで同じ綴りになる。 <sub>`the_terminal_diagram_is_a_report_named_terminal_diagram`</sub>
- 端子台が1つも無いプロジェクトでも1ページは出るので、帳票が空になることはない。 <sub>`a_project_without_terminal_blocks_still_yields_one_page`</sub>

### 整えメトリクス (交差・重なり・グリッド)

- 2本の配線がX字に横切っていると、配線交差数は1になる。 <sub>`two_wires_laid_across_each_other_count_as_one_crossing`</sub>
- 触れ合わない配線どうしには交差が無い。 <sub>`wires_that_never_meet_have_no_crossings`</sub>
- 端点を共有してつながっている2本の配線は接続であって交差ではない。 <sub>`wires_joined_at_a_shared_endpoint_are_not_a_crossing`</sub>
- 片方の端点が相手の途中に乗るT分岐は接続点であり、交差には数えない。 <sub>`a_t_branch_landing_on_another_wire_is_not_a_crossing`</sub>
- 1本の折れ線配線の曲がり角は交差ではない: 同じ配線の連続する線分どうしは数えない。 <sub>`the_corner_of_a_polyline_wire_is_not_a_crossing`</sub>
- 自分自身の上を横切るように引き回した配線は、離れた線分どうしが交差するので交差に数える。 <sub>`a_wire_crossing_its_own_route_counts_as_a_crossing`</sub>
- 一部が重なって二重に引かれた配線は交差1件として数える (直すべき箇所が1つだから)。 <sub>`wires_drawn_on_top_of_each_other_count_as_one_crossing`</sub>
- 同一直線上で端どうしが突き合わさっているだけの配線は接続であり、重なりではない。 <sub>`wires_meeting_end_to_end_on_one_line_are_not_a_crossing`</sub>
- 重なって印字される2つの注記は、重なり1件として数える。 <sub>`two_notes_printed_over_each_other_overlap`</sub>
- 外枠が辺で接しているだけの2つの注記は、重なりに数えない。 <sub>`notes_that_only_touch_along_an_edge_do_not_overlap`</sub>
- 十分に離して置かれた注記は重ならない。 <sub>`notes_placed_well_apart_do_not_overlap`</sub>
- シンボルの外形の上に印字された注記は、重なりとして数える。 <sub>`a_note_printed_over_a_symbol_outline_overlaps`</sub>
- シンボルの外形の上端でぴたりと止まる注記は、接しているだけで重なっていない。 <sub>`a_note_stopping_at_the_symbol_edge_does_not_overlap`</sub>
- 重ねて置かれたシンボルどうしの重なりは、ラベルの重なりとは別に数える。 <sub>`symbols_placed_on_top_of_each_other_are_counted_separately`</sub>
- シンボル自身の参照記号は、その持ち主のシンボルとの重なりには数えない。 <sub>`a_reference_designator_never_overlaps_its_own_symbol`</sub>
- シンボル原点も配線頂点も2.5mmグリッドに乗っていれば、グリッド外は0件になる。 <sub>`entities_on_the_grid_report_no_off_grid_points`</sub>
- グリッドから0.1mmずれた点は数えられ、ずれた頂点1つにつき1件になる。 <sub>`a_point_shifted_a_tenth_of_a_millimetre_is_off_grid`</sub>
- 何も置かれていないシートは完全に整っており、全てのメトリクスが0になる。 <sub>`an_empty_sheet_scores_zero_on_every_metric`</sub>
- tidy_metricsは1枚のシートの4つの数値をまとめて1つの値で返す。 <sub>`tidy_metrics_gathers_the_four_counts`</sub>
- 同じシートを2回測ると全く同じ数値になるので、エージェントは編集の前後を比べられる。 <sub>`measuring_the_same_sheet_twice_gives_the_same_numbers`</sub>

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
- 多極の遮断器は極ごとに導通するため、結線されていない極につながる負荷は電源から到達できないと報告される。 <sub>`multipole_breaker_does_not_connect_its_poles_to_each_other`</sub>

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
- キャンセルとターン巻き戻しのエンドポイントはターン安定IDを受け取り、戻せる編集が無いターンは拒否する。 <sub>`cancel_and_undo_turn_respond`</sub>
- ターンの巻き戻しはCommandエンジン経由でエージェントの編集だけを戻し、ターン中にユーザーが手で入れた編集は残す。 <sub>`undo_turn_rolls_back_agent_edits_and_keeps_the_manual_edit`</sub>
- ターンが触った要素を手編集で消していた場合、巻き戻しは衝突エラーで拒否され、図面は一切変更されない。 <sub>`undo_turn_refuses_a_conflicting_rollback`</sub>
- GET /agent/events は会話イベントをTauriのagent:eventと同じ形でSSE配信する。 <sub>`events_endpoint_streams_agent_events`</sub>
- プロジェクトの保存・読込はチャット履歴(.chat.json)を一緒に運ぶ。 <sub>`save_and_load_carry_the_chat_history`</sub>
- プロジェクト読込は実行中のターンを先に中断し、エージェントが古い図面を編集し続けないようにする。 <sub>`load_cancels_a_running_turn`</sub>
- 外部Webオリジンからのリクエストは拒否され、ローカルオリジンとブラウザ以外(Originヘッダなし)は通る。 <sub>`external_origins_are_rejected_but_local_and_originless_pass`</sub>
- オリジンガードはループバックホスト(localhost/127.0.0.1)のみをローカル扱いする。 <sub>`local_origin_predicate_matches_only_loopback_hosts`</sub>
- エージェントへ渡す図面コンテキストはアクティブシートの要約(名前・ネット数・要素数)を含む。 <sub>`drawing_context_summarizes_the_active_sheet`</sub>
- 指摘の無い図面では、図面コンテキストの検証サマリがエラー0・警告0になる。 <sub>`drawing_context_reports_a_clean_drawing_as_no_diagnostics`</sub>
- 図面コンテキストは検証サマリ(重要度ごとの件数と先頭数件のcode・メッセージ)を含む。 <sub>`drawing_context_summarizes_the_verification_result`</sub>
- AI設定のエンドポイントは変更を永続化し、エージェントマネージャへ適用する。 <sub>`settings_endpoints_persist_and_apply`</sub>

### REST Link API

- RESTの部品エンドポイントは検索(サンプル含む)・登録更新・カテゴリ絞り込み・削除・電線品番一覧に対応する。 <sub>`parts_endpoints_search_upsert_delete`</sub>
- GET /api/v1/verify は図面の診断(空参照・未接続ピンなど)をJSONで返す。 <sub>`verify_returns_diagnostics`</sub>
- POST /api/v1/import/kicad は開いているプロジェクトを変換結果で置き換え、patchとインポートレポートを返す。 <sub>`import_kicad_replaces_project_and_reports`</sub>
- POST /api/v1/simulate/op はDC動作点を解き、ネット電圧と部品電流を返す(ngspice必須。未導入時はスキップ)。 <sub>`simulate_op_returns_result`</sub>
- POST /api/v1/export/pdf は指定パスへ正しいPDFファイルを書き出す。 <sub>`export_pdf_writes_pdf_file`</sub>
- POST /api/v1/export/pdf-book は表紙・全シート・指定した帳票を1つのPDFにまとめて書き出す。 <sub>`export_pdf_book_writes_cover_sheets_and_reports`</sub>
- 端子台エンドポイントは図面の端子台を一覧し、1台のチャートとチェック結果を返す。 <sub>`terminal_endpoints_list_chart_and_check`</sub>
- POST /api/v1/export/report は帳票1種をCSVまたは図枠付きPDFで書き出し、図面である端子接続図のCSVは拒否する。 <sub>`export_report_writes_csv_and_pdf_per_report`</sub>

### 回路マクロ (REST)

- POST /macros/save は選択したエンティティをユーザーのマクロフォルダのファイルにし、GET /macros がそれを基準点つきで一覧に返す。 <sub>`saving_a_selection_over_the_link_api_stores_a_macro_in_the_user_folder`</sub>
- POST /macros/apply はマクロを指定位置へ1回の編集として入れ(undo一発で戻る)、参照記号は衝突しないよう振り直される。 <sub>`applying_a_macro_over_the_link_api_is_one_undo_step`</sub>
- プレースホルダと値セットを付けて保存したマクロは、Link APIから値セットを選んで挿入でき、値も同じ1回の編集で図面へ入る。 <sub>`applying_a_macro_with_a_value_set_over_the_link_api_fills_in_the_values`</sub>
- POST /macros/build は選択範囲をファイルに書かずにマクロへ組み立てる(保存ダイアログのプレビューと⌘Cの無名マクロが使う)。 <sub>`building_a_macro_does_not_write_a_file`</sub>
- POST /macros/apply-inline は値で渡したマクロ(⌘Cのクリップボード)を1回の編集として入れ、参照記号も保存済みマクロと同じように振り直す。 <sub>`applying_an_inline_macro_behaves_like_a_stored_one`</sub>
- 知らないマクロid・知らないバリアントキー・空の選択はいずれも400で拒否され、図面は変わらない。 <sub>`the_link_api_refuses_unknown_macros_variants_and_empty_selections`</sub>

### 編集origin (ユーザー/エージェント/MCP)

- 編集は入口ごとに由来が残る: UIはユーザー編集、エージェントのターン中はエージェント編集、外部クライアントはmcp編集。 <sub>`every_edit_path_records_who_made_the_change`</sub>
- ターン実行中の印は入れ子でも数えられ、必ず解除されるため、ターン後の編集はまたユーザー編集になる。 <sub>`the_agent_turn_marker_nests_and_always_clears`</sub>
- エージェントマネージャが使う窓口はエージェント編集だけを巻き戻し、戻した件数を報告する。 <sub>`the_agent_bridge_reverts_only_agent_edits`</sub>

### provider_api

- 設定画面は「どのプロバイダか」「キーが保存済みか」を知るが、キーそのものは受け取らない。 <sub>`the_settings_screen_never_receives_the_api_key`</sub>
- 保存済みのキーを削除すると「保存済み」表示が戻る(設定画面から消えたことが分かる)。 <sub>`removing_the_saved_key_flips_the_saved_flag_back`</sub>
- キー欄が空のまま保存しようとするとエラーになる(役に立たない空の登録を作らない)。 <sub>`an_empty_key_box_is_refused`</sub>
- 接続テストはリクエスト自体を失敗させず、読める理由を答えとして返す。 <sub>`the_connection_test_answers_with_a_readable_reason`</sub>
- 設定エンドポイントでAnthropic APIを選ぶと、プロバイダの状態にも反映される。 <sub>`choosing_the_anthropic_api_is_reflected_in_the_provider_status`</sub>
- OSキーチェーンが返事をしないときも設定画面は開き、理由を表示する(固まらない)。 <sub>`a_keychain_that_never_answers_does_not_freeze_the_settings_screen`</sub>

### search_api

- GET /search はプロジェクト全体から全種別を探し、ジャンプ先のシートとゾーンつきで各ヒットを返す。 <sub>`searching_over_the_link_api_returns_located_hits`</sub>
- kindsパラメータは選んだフィルタチップだけに絞る。未知のフィルタ名は推測せずエラーにする。 <sub>`the_kinds_parameter_narrows_the_search`</sub>
- 空のクエリは1件も返さないので、検索バーが図面全体を並べてしまうことはない。 <sub>`an_empty_query_returns_nothing`</sub>
- GET /devices はデバイスナビゲータが描く参照記号ツリーを、機能ごとの端子と所在つきで返す。 <sub>`the_device_tree_comes_back_over_the_link_api`</sub>
- 何も置いていない図面のデバイスツリーは、エラーではなく空になる。 <sub>`an_empty_drawing_has_an_empty_device_tree`</sub>

### 開始テンプレート (REST)

- GET /templates は同梱の開始テンプレートを英日の名前つきで返す。 <sub>`the_link_api_lists_the_bundled_templates`</sub>
- POST /templates/apply はテンプレートを1回の編集としてシートへ入れ、undo一発で戻せる。 <sub>`applying_a_template_over_the_link_api_is_one_undo_step`</sub>
- シートidを省くと先頭シートが対象になり、知らないテンプレートidは400で拒否される。 <sub>`the_link_api_defaults_to_the_first_sheet_and_refuses_unknown_templates`</sub>

### 整えメトリクス (MCPツール / REST)

- 整えメトリクスのツール説明には「整えループの目標値として使う」ことと、数える3つの対象が書かれている。 <sub>`the_tidy_metrics_tool_is_advertised_as_the_tidy_loop_target`</sub>
- GET /api/v1/tidy-metrics はシートの交差数・ラベル重なり数・シンボル重なり数・グリッド外数をJSONで返す。 <sub>`tidy_metrics_endpoint_returns_the_four_counts`</sub>

### tool_bridge

- API経由で見えるツールはCLI経由とまったく同じ顔ぶれで、片方だけ機能が欠けることがない。 <sub>`the_api_route_sees_the_same_tools_as_the_cli_route`</sub>
- 窓口から見えるツールには全て説明が付き、入力スキーマはオブジェクト型になっている。 <sub>`every_bridged_tool_has_a_description_and_an_object_schema`</sub>
- 窓口経由の編集は図面に入り、他の編集と同じように元に戻せる(Commandエンジンを通っている)。 <sub>`an_edit_through_the_bridge_lands_in_the_document_and_can_be_undone`</sub>
- 読み取り系のツールも窓口経由で動く(エージェントは編集の前に図面を見られる)。 <sub>`reading_tools_work_through_the_bridge`</sub>
- 存在しないツール名は、ターンを落とさずにエラーの結果として返る(モデルが言い直せる)。 <sub>`an_unknown_tool_name_comes_back_as_an_error_result`</sub>
- 引数が間違っているときも理由つきのエラーの結果として返り、モデルが自分で直せる。 <sub>`bad_arguments_come_back_with_a_reason`</sub>

### tools

- 部品検索ツールの説明には選定・比較・代替品の用途が書かれており、「これの代替は?」と聞かれたエージェントがこれを使う。 <sub>`the_parts_search_tool_advertises_selection_and_comparison`</sub>
- テンプレートのツール説明には「作図の雛形であること」「ERC指摘ゼロで入ること」「undo一発で戻せること」が書かれている。 <sub>`the_template_tools_explain_what_a_template_is`</sub>
- 公開ツールには全て説明文が付いており、説明の無いツールをエージェントへ見せない。 <sub>`every_published_tool_has_a_description`</sub>


## AIアシスタント (madake-agent)


### anthropic_api

- APIから届いた本文はそのまま文字の差分として流れ、ターンの終わりに組み立てた返答とトークン使用量が付く。 <sub>`streamed_text_becomes_deltas_and_a_completed_turn`</sub>
- APIへ送るリクエストには設定したモデル・ストリーミング指定・システムプロンプト・ユーザーの発言が載る。 <sub>`the_request_carries_the_model_system_prompt_and_user_message`</sub>
- ブリッジしたMCPツールは毎回のリクエストでAPIへ渡る(エージェントが図面を編集できる)。 <sub>`the_bridged_mcp_tools_are_offered_to_the_api`</sub>
- モデルが要求したツールはその場で実行され、結果を返して会話を続ける(1ターンで完結する)。 <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
- 失敗したツールはターンを中断せず、エラーとしてモデルへ返す(モデルが直せる)。 <sub>`a_failing_tool_is_reported_to_the_model_as_an_error_result`</sub>
- ツール実行の往復には上限があり、堂々巡りになったモデルが延々と動き続けない。 <sub>`the_tool_loop_stops_after_a_bounded_number_of_rounds`</sub>
- APIキーが弾かれたときは、生のHTTPコードではなく「キーの問題」と分かる文言を表示する。 <sub>`a_rejected_api_key_is_explained_as_a_key_problem`</sub>
- APIが混み合っているときは「混雑しているので時間をおいて」と読める文言を表示する。 <sub>`an_overloaded_api_is_explained_as_a_busy_service`</sub>
- ストリームの途中で届いたエラーもユーザーへ伝える(黙って途切れさせない)。 <sub>`an_error_event_inside_the_stream_is_surfaced`</sub>
- 会話の前のやりとりも一緒に送るので、モデルは前に話した内容を覚えている。 <sub>`earlier_turns_of_the_conversation_are_replayed`</sub>
- 接続テストは、使えるキーなら成功を、駄目なキーなら読める理由を返す。 <sub>`the_connection_test_reports_success_or_a_readable_reason`</sub>

### Claude CLIバックエンド

- Claude CLIは必須フラグ(-p・stream-json出力・部分メッセージ・strict MCP設定)付きでヘッドレス起動される。 <sub>`args_contain_required_flags`</sub>
- 同梱ドキュメントのフォルダは読み取り可能なディレクトリとしてエージェントへ開かれ、マニュアルを引用できる。 <sub>`args_open_the_bundled_documentation_for_reading`</sub>
- セッション再開ID・モデル指定・追加システムプロンプトは、指定時にCLIへ引き渡される。 <sub>`args_include_resume_model_and_system_prompt_when_given`</sub>
- エージェントのMCP設定はMadakeCAD自身のローカルMCPサーバーを指し、他クライアントと同じツールを使う。 <sub>`mcp_config_points_at_local_mcp_server`</sub>
- ターンはCLIのstream-json出力から解釈したイベント(テキスト差分・ツール実行・完了)を流す。 <sub>`send_streams_events_from_fake_cli`</sub>
- CLIが非ゼロ終了した場合はハングせずエラーイベントとして報告される。 <sub>`send_reports_nonzero_exit_as_error_event`</sub>
- CLIが理由(利用上限など)をstdoutの素のテキストで出して異常終了したとき、その理由がチャットのエラーに載る。 <sub>`a_plain_stdout_reason_reaches_the_error_message`</sub>
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
- 巻き戻しを記録するとターンの適用範囲が畳まれ、以降は「元に戻す」の対象にならない。 <sub>`record_reverted_clears_the_applied_range`</sub>
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
- 1ターンのユーザー発話とエージェント応答には同じターンID(巻き戻しの目印)が付く。 <sub>`the_two_messages_of_a_turn_share_one_turn_id`</sub>
- ターンごとに別のターンIDが振られ、後続ターンが積まれても対象を指定できる。 <sub>`each_turn_gets_its_own_turn_id`</sub>
- 旧フォーマット(ターンID無し)のチャット履歴は、ターン境界からIDを採番して読み込まれる。 <sub>`load_chat_migrates_legacy_files_by_assigning_turn_ids`</sub>
- チャット履歴の保存は現在のフォーマット版を記録する(移行済みファイルを再移行しない)。 <sub>`save_chat_stamps_the_current_format_version`</sub>
- チャットファイルはプロジェクトの隣に<名前>.chat.jsonとして置かれる。 <sub>`chat_path_sits_next_to_project_file`</sub>

### copilot_cli

- Copilot CLIは、人が見ていなくても働ける設定(JSONL出力・ツール自動許可・質問しない)で非対話起動される。 <sub>`args_contain_required_flags`</sub>
- 会話の最初のターンは新しいセッションIDを固定し、以降のターンが同じセッションを続けられるようにする。 <sub>`the_first_turn_pins_a_new_session_id`</sub>
- 2ターン目以降は新しいセッションを作らず同じCopilotセッションを再開するので、エージェントは会話を覚えている。 <sub>`a_follow_up_turn_resumes_the_same_session`</sub>
- 設定したCopilotのモデルはCLIへ渡される(`auto`ならCopilotが自動で選ぶ)。 <sub>`the_configured_model_is_passed_to_the_cli`</sub>
- CopilotにはMadakeCAD自身のMCPサーバーを渡すので、他のクライアントと同じコマンド経由で図面を編集する。 <sub>`the_mcp_config_points_at_the_local_madakecad_server`</sub>
- Copilot CLIにはシステムプロンプト用のフラグが無いため、MadakeCADの作図ルールは見出し付きでプロンプトの先頭へ前置される。 <sub>`the_system_prompt_is_prepended_to_the_user_prompt`</sub>
- ターンはCopilotのJSONLから解釈したイベント(セッション・本文・ツール実行・完了)を流す。 <sub>`a_turn_streams_events_from_the_fake_cli`</sub>
- 合成したプロンプトは実際に-p引数としてCLIへ届く。 <sub>`the_composed_prompt_reaches_the_cli`</sub>
- Copilotへ渡すMCP設定ファイルはターン実行中に実在し、MadakeCADのサーバーを指している。 <sub>`the_mcp_config_file_exists_while_the_turn_runs`</sub>
- GitHubへサインインしていない場合、チャットにその旨とサインイン手順が表示される。 <sub>`a_signed_out_cli_is_reported_with_login_guidance`</sub>
- 実機CLIの未認証メッセージは、どこに出ても未認証として認識される。 <sub>`the_real_unauthenticated_message_is_recognised`</sub>
- Copilotが理由(クレジット切れなど)を素のテキストで出して異常終了したとき、その理由がチャットのエラーに載る。 <sub>`a_plain_stdout_reason_reaches_the_error_message`</sub>
- イベント受信側が消えた(ユーザーが中断した)場合、ターンは永久にブロックせず速やかに終わる。 <sub>`a_cancelled_turn_stops_promptly`</sub>
- Copilot CLIの検出は、設定された実行ファイルからバージョンを読む。 <sub>`detect_reads_the_version_from_the_configured_executable`</sub>
- Copilotの実行ファイルが入っていない場合、検出は明確に失敗する。 <sub>`detect_fails_when_copilot_is_not_installed`</sub>
- 検出はPATHと、npmの一般的なインストール先からcopilotを探す。 <sub>`default_candidates_include_the_known_install_paths`</sub>
- 接続テストは、CLIが応答すれば成功を、サインインしていなければサインイン案内を返す。 <sub>`the_connection_test_distinguishes_success_from_being_signed_out`</sub>

### copilot_events

- Copilotのセッション行は、同じ会話を続けるためのセッションイベントになる。 <sub>`session_line_becomes_a_session_event`</sub>
- アシスタントの本文は、CLIがどのフィールド名で出してもチャットへ届く。 <sub>`assistant_text_is_delivered_under_any_of_the_known_field_names`</sub>
- ツール呼び出しの行は、ツール名と引数を持つ「ツール開始」イベントになる。 <sub>`a_tool_call_line_becomes_a_tool_start_event`</sub>
- ツール呼び出しは別のフィールド名(tool/input、function/parameters)でも認識される。 <sub>`tool_calls_are_recognised_under_alternative_field_names`</sub>
- ツール結果の行は対応するツール呼び出しを閉じ、失敗したかどうかを伝える。 <sub>`a_tool_result_line_closes_the_call_and_reports_failure`</sub>
- ツール名は呼び出しから結果へ補完され、チャットには何が終わったのかが表示される。 <sub>`the_tool_name_is_carried_from_the_call_to_its_result`</sub>
- 最後の行は、回答とトークン使用量を伴ってターンを完了させる。 <sub>`the_final_line_completes_the_turn_with_usage`</sub>
- トークン使用量はOpenAI流のフィールド名(prompt/completion tokens)でも読み取れる。 <sub>`token_usage_is_also_read_from_openai_style_names`</sub>
- CLIのエラー行は、チャットにエラーとして表示される。 <sub>`an_error_line_becomes_an_error_event`</sub>
- エラー扱いのresult行は、通常の完了ではなくエラーになる。 <sub>`a_failed_result_line_becomes_an_error`</sub>
- 解釈できない行はターンを壊さず黙って無視される。 <sub>`unknown_and_broken_lines_are_ignored`</sub>
- 同じツール呼び出しが二度流れても、チャットには一度だけ表示される。 <sub>`a_repeated_tool_call_is_shown_only_once`</sub>

### gemini

- Geminiから届いた本文はそのまま文字の差分として流れ、ターンの終わりに組み立てた返答とトークン使用量が付く。 <sub>`streamed_text_becomes_deltas_and_a_completed_turn`</sub>
- Geminiが毎チャンクに載せてくる累計の使用量を二重に足し込まない。 <sub>`repeated_running_token_totals_are_not_counted_twice`</sub>
- リクエストは設定したモデルのストリーミング用エンドポイントへ行き、システム指示とユーザーの発言を載せる。 <sub>`the_request_goes_to_the_streaming_endpoint_with_the_system_instruction_and_message`</sub>
- ブリッジしたMCPツールはGeminiのfunctionDeclarations形式で渡る(エージェントが図面を編集できる)。 <sub>`the_bridged_mcp_tools_are_offered_as_function_declarations`</sub>
- Geminiが受け付けないスキーマの項目はツール定義から落とし、型が無くなったプロパティにも型を補う。 <sub>`schema_keywords_gemini_rejects_are_dropped_from_tool_definitions`</sub>
- モデルが要求したツールはその場で実行され、結果をfunctionResponseとして返して会話を続ける(1ターンで完結する)。 <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
- 呼び出しIDを付けてこないモデルにも、関数名で対応づけて結果を返す。 <sub>`a_call_without_an_id_is_answered_by_function_name`</sub>
- 失敗したツールはターンを中断せず、エラーの欄に理由を入れてモデルへ返す(モデルが直せる)。 <sub>`a_failing_tool_is_reported_to_the_model_as_an_error_result`</sub>
- 関数呼び出しの往復には上限があり、堂々巡りになったモデルが延々と動き続けない。 <sub>`the_tool_loop_stops_after_a_bounded_number_of_rounds`</sub>
- APIキーはx-goog-api-keyヘッダだけで送られ、URLにも本文にもユーザーに見える文言にも現れない。 <sub>`the_api_key_travels_only_in_the_header`</sub>
- APIキーが弾かれたときは、生のHTTPコードではなく「キーの問題」と分かる文言を表示する。 <sub>`a_rejected_api_key_is_explained_as_a_key_problem`</sub>
- Geminiの利用上限に当たったときは「利用上限に達したので時間をおいて」と読める文言を表示する。 <sub>`a_rate_limited_service_is_explained_as_a_usage_limit`</sub>
- モデル名が見つからないときは「モデル名の問題」と分かる文言で、試したモデル名を添えて表示する。 <sub>`an_unknown_model_name_is_explained_as_a_model_problem`</sub>
- サーバーへ接続できないときは、黙って失敗せずに試したURLを示す。 <sub>`an_unreachable_server_names_the_url_that_was_tried`</sub>
- 会話の前のやりとりも一緒に送るので、モデルは前に話した内容を覚えている。 <sub>`earlier_turns_of_the_conversation_are_replayed`</sub>
- 接続テストは、使える設定なら成功を、駄目な設定なら区分つきの読める理由を返す。 <sub>`the_connection_test_reports_success_or_a_readable_reason`</sub>
- 接続テストは設定したモデルの通常(非ストリーミング)エンドポイントを叩く。 <sub>`the_connection_test_uses_the_non_streaming_endpoint`</sub>
- 接続先と既定のモデル名はGoogleが公開しているGemini APIのものになっている。 <sub>`the_defaults_match_the_published_gemini_api`</sub>
- 「models/」付きで貼り付けたモデル名でも、正しいエンドポイントへ1回だけつながる。 <sub>`a_model_name_with_the_models_prefix_still_works`</sub>
- モデル名が無いときは通信する前にターンを止め、どこを埋めればよいかを伝える。 <sub>`an_empty_model_name_stops_before_any_request`</sub>

### 規格知識の注入

- 同梱の規格知識には図記号・参照記号・線色/sq・線番・配置の決まりが書かれている。 <sub>`bundled_standards_cover_the_drawing_conventions`</sub>
- システムプロンプトには同梱の規格知識と検証ループの指示が載る。 <sub>`system_prompt_carries_the_standards_and_the_verification_loop`</sub>
- システムプロンプトは、白紙から作り始めるときはまず開始テンプレートを使うよう指示する。 <sub>`system_prompt_points_at_the_start_templates`</sub>
- システムプロンプトは、シェルが使えないこととエンティティidを自分で書くことを伝える。 <sub>`system_prompt_tells_the_agent_no_shell_is_available`</sub>
- システムプロンプトは図面コンテキストが先頭で、規格知識はその後ろに続く。 <sub>`system_prompt_puts_the_drawing_context_first`</sub>
- 設定の知識ファイルは同梱ノートの後ろへ追記される。 <sub>`user_knowledge_file_is_appended_to_the_prompt`</sub>
- 知識ファイル未設定なら、プロンプトには同梱ノートだけが載る。 <sub>`without_a_knowledge_file_only_the_bundled_note_is_used`</sub>
- プロンプトには同梱マニュアル(01〜13)の目次が載り、操作方法の質問へ出典つきで答えられる。 <sub>`system_prompt_lists_the_bundled_documentation`</sub>
- 未対応機能の質問には、ロードマップと機能インベントリを見てマイルストーンを答える。 <sub>`system_prompt_points_unsupported_features_at_the_roadmap`</sub>
- ドキュメントが同梱されていない環境では、存在しないファイルを案内せずガイドごと省く。 <sub>`the_documentation_guide_is_dropped_when_the_docs_are_missing`</sub>
- レビュー依頼ではまず`run_verification`を実行し、その後で慣行の5観点を点検する。 <sub>`system_prompt_carries_the_review_checklist`</sub>
- レビューの指摘は「重要度|対象|指摘|提案」で並べ、修正はユーザー承認を待つ。 <sub>`review_findings_use_the_severity_table_and_wait_for_approval`</sub>
- 動作確認手順の依頼には「手順|操作|期待結果」の表で答え、図面へ勝手に注記を書き込まない。 <sub>`system_prompt_carries_the_test_plan_table`</sub>
- 部品選定では部品DBを検索し、比較表で答える。 <sub>`system_prompt_carries_the_parts_comparison_table`</sub>
- 読めない知識ファイルは読み飛ばされ、同梱ノートは失われない。 <sub>`an_unreadable_knowledge_file_is_skipped`</sub>
- システムプロンプトは、整えるときは`get_tidy_metrics`で数値を測り、改善が止まったらやめるよう指示する。 <sub>`system_prompt_carries_the_tidy_loop`</sub>

### knowledge_docs

- エージェントにはアプリが解決したドキュメントの場所が伝わり、どこにインストールされていてもマニュアルを読める。 <sub>`the_documentation_folder_of_the_installed_app_is_handed_to_the_agent`</sub>

### 規格知識 (リソースファイル)

- 同梱の規格知識は、実ファイル(Tauriのリソース・開発時のパス)があればそちらから読む。 <sub>`the_standards_note_is_read_from_the_resource_file_when_present`</sub>

### エージェントマネージャ (ターン)

- 会話IDなしの送信は会話を新規作成し、そのイベントを配信する。 <sub>`send_creates_conversation_and_broadcasts_events`</sub>
- 図面を編集したターンは適用revisionを記録し、undo深さ付きのターン適用イベントを発行する。 <sub>`turn_records_applied_revisions_and_emits_turn_applied`</sub>
- ターン実行中に入った編集はエージェント編集として記録され、ターン外の編集はユーザー編集のままになる。 <sub>`edits_during_a_turn_are_recorded_as_agent_edits`</sub>
- ターンの巻き戻しはエージェントの編集だけを戻し、ターン中に入れたものも含めてユーザーの手編集は残す。 <sub>`undo_turn_reverts_only_the_agent_edits_of_the_turn`</sub>
- 古いターンも後から巻き戻せて、そのあとに入った編集は保持される。 <sub>`undo_turn_can_revert_an_older_turn_and_keeps_later_edits`</sub>
- 手編集と衝突する巻き戻しは拒否され、ターンは適用済みのまま残るので、後から再試行できる。 <sub>`undo_turn_reports_a_conflict_and_keeps_the_turn_rollbackable`</sub>
- ターンIDは後続ターンが積まれても同じターンを指し続ける(添字と違いズレない)。 <sub>`a_turn_id_keeps_addressing_the_same_turn_after_more_turns`</sub>
- 既に巻き戻したターンは二重に巻き戻せない(明示エラー)。 <sub>`undo_turn_rejects_an_already_undone_turn`</sub>
- 実行中のターンは巻き戻せない。 <sub>`undo_turn_rejects_a_running_turn`</sub>
- ドキュメントエラーと不明なターン指定は区別されたエラーとして報告される。 <sub>`undo_turn_reports_doc_errors_and_unknown_targets`</sub>
- 配信される全イベントに、それを生んだターンの通し番号が付き、送信のたびに増える。 <sub>`turn_events_carry_a_monotonic_turn_seq`</sub>
- キャンセルは中断したターンの通し番号を伝え、次のターンはより大きい番号になるため、遅れて届くイベントを見分けられる。 <sub>`cancel_reports_the_cancelled_turn_seq`</sub>
- ターン実行中の会話への追加送信は拒否される。 <sub>`second_send_while_running_is_rejected`</sub>
- キャンセルはCLIプロセスを停止し、メッセージをキャンセル済みにする。 <sub>`cancel_stops_the_turn_and_marks_the_message`</sub>
- 不明な会話IDへの送信は明確に失敗する。 <sub>`send_to_unknown_conversation_fails`</sub>
- 図面コンテキストと選択モデルはCLI起動へ引き渡される。 <sub>`context_and_model_are_forwarded_to_the_cli`</sub>
- 送信のたびに、同梱の規格知識と検証ループの指示がシステムプロンプトへ載る。 <sub>`every_turn_injects_the_standards_knowledge`</sub>
- 設定の知識ファイルの内容もCLIへ渡る。 <sub>`the_knowledge_file_setting_reaches_the_cli`</sub>
- 図面自動読み取りをオフにすると図面コンテキストは付かないが、規格知識は残る。 <sub>`auto_read_drawing_off_suppresses_the_drawing_context`</sub>
- claude実行ファイルパス設定がバックエンドの実行ファイルを上書きする。 <sub>`claude_path_setting_becomes_the_backend_executable`</sub>
- 会話履歴の置き換え(プロジェクト読込)は実行中ターンを先にキャンセルする。 <sub>`set_conversations_replaces_history_and_cancels_running_turn`</sub>
- 別々の会話のターンは順番待ちにならず、同時に走る。 <sub>`turns_in_two_conversations_run_at_the_same_time`</sub>
- 2つの会話が同時に入れた編集は、どれも失われずrevisionの順に図面へ収まる。 <sub>`parallel_turns_keep_every_edit_in_revision_order`</sub>
- ターン通し番号は会話ごとに独立しているので、片方を中断してももう片方のイベントは捨てられない。 <sub>`each_conversation_keeps_its_own_turn_seq`</sub>
- 並行していたターンの編集ごと巻き戻したときは、そのターンも巻き戻し済みになる(適用済みのまま残らない)。 <sub>`undo_turn_marks_the_parallel_turn_whose_edits_it_swept`</sub>

### manager_api_provider

- Anthropic APIを選んでおけば、Claude Code CLIが入っていない環境でもチャットのやりとりができる。 <sub>`a_turn_runs_on_the_api_without_any_claude_cli`</sub>
- APIを選んだのにキーを保存していないと、設定画面へ促す文言で送信を断る。 <sub>`sending_without_a_saved_key_points_at_the_settings_screen`</sub>
- キーを保存すればプロバイダは「使える」状態になり、消せば「使えない」状態に戻る(バッジ表示用)。 <sub>`the_provider_is_ready_only_while_a_key_is_saved`</sub>
- 接続テストはキーを保存する前は実行せず、「キーが無い」ことを理由として返す。 <sub>`the_connection_test_refuses_before_a_key_is_saved`</sub>
- APIキーはUIへ流れるイベントに一切載らない。 <sub>`the_api_key_never_appears_in_the_event_stream`</sub>

### manager_copilot_provider

- GitHub Copilotを選んでおけば、Anthropicのキーが無くてもCopilot CLI経由でチャットのやりとりができる。 <sub>`a_turn_runs_through_the_copilot_cli`</sub>
- 最初のターンで決めたセッションIDは記憶され、次のターンは同じCopilotセッションの続きになる。 <sub>`the_session_is_remembered_for_the_next_turn`</sub>
- プロバイダのバッジは、Copilot CLIが実際に見つかるときだけ「使える」状態になる。 <sub>`the_provider_is_ready_only_while_copilot_is_found`</sub>
- 設定したCopilotの実行ファイルが無い場合、黙って失敗せず理由がチャットに表示される。 <sub>`a_missing_copilot_executable_is_explained_in_the_chat`</sub>
- Copilotを選んでいる間、接続テストはAnthropic APIではなくCopilotへ行く。 <sub>`the_connection_test_follows_the_chosen_provider`</sub>

### manager_gemini_provider

- Google Geminiを選んでおけば、Claude Code CLIが入っていない環境でもチャットのやりとりができる。 <sub>`a_turn_runs_on_google_gemini`</sub>
- Geminiを選んだのにキーを保存していないと、設定画面へ促す文言で送信を断る。 <sub>`sending_without_a_key_points_at_the_settings_screen`</sub>
- GeminiのキーはAnthropic・OpenAI互換とは別の入れ物に入るため、他社のキーがあってもGeminiは「使える」にならない。 <sub>`the_gemini_key_is_stored_separately_from_the_other_providers`</sub>
- モデル欄を空にすると推奨の既定モデルへ戻るため、そのまま使える状態が保たれる。 <sub>`a_blank_model_box_falls_back_to_the_default_model`</sub>
- Geminiを選んでいる間、接続テストはAnthropic APIではなくGeminiへ行く。 <sub>`the_connection_test_follows_the_chosen_provider`</sub>
- GeminiのAPIキーはUIへ流れるイベントにも会話履歴にも一切載らない。 <sub>`the_api_key_never_appears_in_the_event_stream`</sub>

### プロバイダ: OpenAI互換 / Ollama

- OpenAI互換APIを選んでおけば、Claude Code CLIが入っていない環境でもチャットのやりとりができる。 <sub>`a_turn_runs_on_an_openai_compatible_endpoint`</sub>
- ローカルのOllamaのURLならAPIキーは要らず、キーチェーンに何も保存していなくてもやりとりできる。 <sub>`a_local_ollama_url_needs_no_api_key`</sub>
- 社外のOpenAI互換APIを選んだのにキーを保存していないと、設定画面へ促す文言で送信を断る。 <sub>`sending_to_a_remote_endpoint_without_a_key_points_at_the_settings_screen`</sub>
- モデル名を空のままにしていると送信を断り、どこを埋めればよいかを伝える。 <sub>`sending_without_a_model_name_says_which_box_to_fill_in`</sub>
- プロバイダのバッジは、キーを保存済み(またはURLがローカル)でモデル名が入っているときだけ「使える」状態になる。 <sub>`the_provider_is_ready_with_a_key_or_a_local_url_and_a_model`</sub>
- OpenAI互換APIを選んでいる間、接続テストはAnthropic APIではなくそのURLへ行く。 <sub>`the_connection_test_follows_the_chosen_provider`</sub>
- OpenAI互換APIのキーはUIへ流れるイベントにも会話履歴にも一切載らない。 <sub>`the_api_key_never_appears_in_the_event_stream`</sub>

### OpenAI互換バックエンド

- OpenAI互換サーバーから届いた本文はそのまま文字の差分として流れ、ターンの終わりに組み立てた返答とトークン使用量が付く。 <sub>`streamed_text_becomes_deltas_and_a_completed_turn`</sub>
- リクエストは設定したベースURL配下の /chat/completions へ行き、モデル・ストリーミング指定・システムプロンプト・ユーザーの発言を載せる。 <sub>`the_request_goes_to_chat_completions_with_the_model_system_prompt_and_message`</sub>
- ブリッジしたMCPツールはOpenAIのfunction形式で渡る(エージェントが図面を編集できる)。 <sub>`the_bridged_mcp_tools_are_offered_as_openai_functions`</sub>
- モデルが要求したツールはその場で実行され、結果をツールメッセージとして返して会話を続ける(1ターンで完結する)。 <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
- 失敗したツールはターンを中断せず、エラーと分かる印をつけてモデルへ返す(モデルが直せる)。 <sub>`a_failing_tool_is_reported_to_the_model_as_an_error_result`</sub>
- ツール実行の往復には上限があり、堂々巡りになったモデルが延々と動き続けない。 <sub>`the_tool_loop_stops_after_a_bounded_number_of_rounds`</sub>
- 保存したキーはBearerトークンとして送られ、ユーザーに見える文言には一切現れない。 <sub>`a_saved_key_is_sent_as_a_bearer_token`</sub>
- Ollamaのようなローカルサーバーは、キーを保存していなくてもAuthorizationヘッダ無しでそのまま呼べる。 <sub>`a_local_server_is_called_without_an_authorization_header`</sub>
- APIキーが弾かれたときは、生のHTTPコードではなく「キーの問題」と分かる文言を表示する。 <sub>`a_rejected_api_key_is_explained_as_a_key_problem`</sub>
- レート制限に当たったときは「利用上限に達したので時間をおいて」と読める文言を表示する。 <sub>`a_rate_limited_service_is_explained_as_a_usage_limit`</sub>
- モデル名が見つからないときは「モデル名の問題」と分かる文言で、試したモデル名を添えて表示する。 <sub>`an_unknown_model_name_is_explained_as_a_model_problem`</sub>
- サーバーへ接続できないときは試したURLを示し、ローカルURLならOllamaの起動を促す。 <sub>`an_unreachable_server_names_the_url_and_suggests_starting_ollama`</sub>
- 会話の前のやりとりも一緒に送るので、モデルは前に話した内容を覚えている。 <sub>`earlier_turns_of_the_conversation_are_replayed`</sub>
- 接続テストは、使える設定なら成功を、駄目な設定なら区分つきの読める理由を返す。 <sub>`the_connection_test_reports_success_or_a_readable_reason`</sub>
- Ollamaプリセットはローカルのollamaが持つOpenAI互換エンドポイントを指し、既定の接続先はOpenAI本体である。 <sub>`the_ollama_preset_points_at_the_local_ollama_server`</sub>
- 末尾に「/」を付けて貼り付けたベースURLでも、/chat/completions へ正しく1回だけつながる。 <sub>`a_base_url_with_a_trailing_slash_still_works`</sub>

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

### secrets

- 保存したAPIキーは読み戻せて、削除すると消える。 <sub>`a_saved_api_key_can_be_read_back_and_deleted`</sub>
- 保存していないキーを削除してもエラーにしない(UIの「削除」がいつでも押せる)。 <sub>`deleting_a_key_that_was_never_stored_is_not_an_error`</sub>
- 空の入力は保存せずに断る(空のキーを持って後から分かりにくい失敗をしないため)。 <sub>`a_blank_api_key_is_refused`</sub>
- 前後の空白は取り除いて保存する(改行ごと貼り付けたキーでも使える)。 <sub>`a_pasted_key_is_trimmed_before_it_is_stored`</sub>
- 保存したキーは設定ファイルに一切現れない(設定ファイルに秘密は書かない)。 <sub>`the_settings_file_never_contains_the_api_key`</sub>
- 何らかの理由でAPIキーが書かれた設定ファイルを読んでも、次の保存でその項目は消える。 <sub>`an_api_key_smuggled_into_the_settings_file_is_dropped_on_save`</sub>
- キーチェーンの保管先はアプリ名と決まった名前で、次に起動しても同じキーが見つかる。 <sub>`the_keychain_entry_is_addressed_by_the_app_name`</sub>
- 実際のOSキーチェーンでも、キーは保存して読み戻して削除できる(環境変数で明示的に有効化したときだけ実行)。 <sub>`the_real_os_keychain_round_trips_a_key`</sub>
- キーチェーンを読むのはアプリ起動につき1回だけで、OSの許可確認が何度も出ることがない。 <sub>`the_keychain_is_read_only_once_per_app_run`</sub>
- OSキーチェーンが読めないときは、黙って「キー未設定」にせず理由を伝える。 <sub>`a_keychain_that_cannot_be_read_reports_the_reason`</sub>
- OpenAI互換APIのキーはキーチェーンの別の名前で保管され、片方を保存してももう片方のキーは影響を受けない。 <sub>`the_openai_key_is_stored_beside_the_anthropic_one_without_disturbing_it`</sub>
- 貼り付けたOpenAIのキーは前後の空白を落として保存し、空の入力は保存せずに断る。 <sub>`a_pasted_openai_key_is_trimmed_and_a_blank_one_is_refused`</sub>
- OpenAI互換APIのキーは設定ファイルに一切現れない(設定ファイルに残るのはURLとモデル名だけ)。 <sub>`the_settings_file_never_contains_the_openai_key`</sub>

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
- 既定では追加の知識ファイルは未設定(同梱の規格ノートだけを使う)。 <sub>`default_knowledge_path_is_unset`</sub>
- 知識ファイルの項目が無い旧い設定ファイルは、未設定として読み込まれる。 <sub>`old_settings_file_without_knowledge_path_loads_unset`</sub>
- 空白だけの知識ファイルパスは正規化で未設定になり、前後の空白は取り除かれる。 <sub>`normalized_drops_a_blank_knowledge_path`</sub>
- 既定のエージェントはClaude Code CLI経由で、API経由に切り替えたときのモデルはClaude Sonnet 5。 <sub>`the_default_provider_is_the_claude_code_cli`</sub>
- Anthropic APIを選んだ設定は、モデル名と一緒に保存して読み直しても保持される。 <sub>`the_anthropic_api_choice_round_trips_through_save_and_load`</sub>
- プロバイダは読める名前で保存される(設定ファイルを手で書き換えられる)。 <sub>`the_provider_is_stored_under_a_readable_name`</sub>
- プロバイダの項目が無い旧い設定ファイルもそのまま動き、Claude Code CLIのままになる。 <sub>`an_old_settings_file_without_a_provider_stays_on_the_cli`</sub>
- このビルドが知らないプロバイダ名は、設定ファイル全体を読めなくせずにCLIへ戻す。 <sub>`an_unknown_provider_name_falls_back_to_the_cli`</sub>
- モデル名を空にすると既定のモデルへ戻り、前後の空白は取り除かれる。 <sub>`a_blank_api_model_falls_back_to_the_default`</sub>
- GitHub Copilotの既定はモデルをCopilotに選ばせる設定で、実行ファイルはPATHから探す。 <sub>`the_copilot_defaults_let_copilot_pick_the_model`</sub>
- GitHub Copilotを選んだ設定は、パスとモデルと一緒に保存して読み直しても保持される。 <sub>`the_copilot_choice_round_trips_through_save_and_load`</sub>
- GitHub Copilotの項目が無い旧い設定ファイルもそのまま動き、Copilotの項目は既定値になる。 <sub>`an_old_settings_file_without_copilot_fields_keeps_working`</sub>
- Copilotのモデル欄を空にすると`auto`へ戻り、パス欄を空にすると自動検出へ戻る。 <sub>`blank_copilot_boxes_return_to_the_defaults`</sub>
- GitHubの資格情報は設定ファイルへ一切書かれない(Copilotは自身のサインインを使う)。 <sub>`no_github_credential_is_written_to_the_settings_file`</sub>
- OpenAI互換APIの既定の接続先はOpenAI本体で、モデル名はあえて空(利用者が必ず選ぶ項目)。 <sub>`the_openai_defaults_point_at_openai_and_ask_for_a_model`</sub>
- OpenAI互換APIを選んだ設定は、URLとモデル名と一緒に保存して読み直しても保持される。 <sub>`the_openai_choice_round_trips_through_save_and_load`</sub>
- OpenAI互換APIの項目が無い旧い設定ファイルもそのまま動き、その項目は既定値になる。 <sub>`an_old_settings_file_without_openai_fields_keeps_working`</sub>
- URL欄を空にするとOpenAI本体へ戻り、末尾の「/」は取り除かれ、モデル名は入力どおり(前後の空白だけ除去)保たれる。 <sub>`blank_openai_boxes_return_to_the_defaults`</sub>
- OpenAI互換APIのキーは設定ファイルへ一切書かれない(そこに置くのはURLとモデル名だけ)。 <sub>`no_openai_key_is_written_to_the_settings_file`</sub>
- Geminiの経路は、はじめから推奨の高速モデルが入った状態になっている。 <sub>`the_gemini_default_model_is_the_recommended_fast_one`</sub>
- Geminiを選んだ設定は、モデル名と一緒に保存して読み直しても保持される。 <sub>`the_gemini_choice_round_trips_through_save_and_load`</sub>
- Geminiの項目が無い旧い設定ファイルもそのまま動き、その項目は既定値になる。 <sub>`an_old_settings_file_without_gemini_fields_keeps_working`</sub>
- Geminiのモデル欄を空にすると既定モデルへ戻り、前後の余分な空白は取り除かれる。 <sub>`a_blank_gemini_model_box_returns_to_the_default`</sub>
- GeminiのAPIキーは設定ファイルへ一切書かれない(そこに置くのはモデル名だけ)。 <sub>`no_gemini_key_is_written_to_the_settings_file`</sub>

### tool_bridge

- MCPツール1つがAPIのツール1つになり、名前・説明・入力スキーマがそのまま引き継がれる。 <sub>`every_mcp_tool_becomes_one_api_tool`</sub>
- JSON Schemaの`$schema`宣言は落とす(APIが必要とするのは形そのものだけ)。 <sub>`the_schema_marker_is_dropped_from_the_input_schema`</sub>
- スキーマの無いツールにも空のオブジェクトスキーマを与える(APIが受け付ける形にする)。 <sub>`a_tool_without_a_schema_gets_an_empty_object_schema`</sub>
- `"type": "object"`が抜けたスキーマは、そのまま送らずに補って直す。 <sub>`a_schema_missing_its_object_type_is_repaired`</sub>
- ブリッジしたツール名は全てAPIが許す文字種・長さに収まる(ツールが弾かれない)。 <sub>`bridged_tool_names_fit_the_api_name_rules`</sub>
- 説明が空のツールには説明の代わりを入れる(説明無しのツールをAPIへ渡さない)。 <sub>`a_tool_without_a_description_still_carries_some_text`</sub>


## madake CLI


### 引数解釈・ディスパッチ

- 既定ポートは9310で、--jsonは指定しない限り無効。 <sub>`default_port_is_9310_and_json_is_off`</sub>
- --portと--jsonはグローバルオプションで、サブコマンドの後にも書ける。 <sub>`port_and_json_are_global_options_after_subcommand`</sub>
- エクスポート種別はドキュメント表記どおりの'wire-list'を受け付ける。 <sub>`export_kind_accepts_wire_list_spelling`</sub>
- madake export pdf-book は --reports に並べた帳票をその順で送り、既定では表紙付きで依頼する。 <sub>`export_pdf_book_sends_the_requested_reports_in_order`</sub>
- --no-cover を付けると一括PDFから表紙が外れる。 <sub>`export_pdf_book_can_drop_the_cover`</sub>
- madake terminalsは図面全体の端子台を一覧し、--sheetでシートを絞れる。 <sub>`terminals_lists_blocks_and_forwards_sheet`</sub>
- madake export terminal-chart は図面全体のチャートを、拡張子.csvからCSVと判断して書き出す。 <sub>`export_terminal_chart_defaults_to_csv_for_the_whole_project`</sub>
- 出力先が.pdfなら--format無しでも帳票のPDF(図枠付き図面シート)形式になる。 <sub>`export_report_picks_pdf_from_the_extension`</sub>
- --formatは拡張子より優先される。 <sub>`export_report_format_option_overrides_the_extension`</sub>
- --terminalは参照記号(TB1)を受け取り、端子台一覧からentity idを引く。 <sub>`export_terminal_diagram_resolves_a_reference_designator`</sub>
- --terminalにentity idを渡した場合は一覧を引かずそのまま送る。 <sub>`export_terminal_chart_accepts_an_entity_id_directly`</sub>
- 存在しない端子台を指定した場合は、図面にある端子台の一覧を添えて中断する。 <sub>`export_reports_unknown_terminal_with_candidates`</sub>
- 図面全体が対象の帳票に--terminalを付けると拒否する。 <sub>`export_rejects_terminal_option_on_other_reports`</sub>
- 帳票は常に図面全体が対象のため、--sheetだけの指定は--terminalを案内して拒否する。 <sub>`export_rejects_sheet_only_narrowing_for_reports`</sub>
- 帳票ではない回路図の出力(svg/pdf/pdf-book)に--formatを付けると拒否する。 <sub>`export_rejects_format_option_on_schematic_exports`</sub>
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
- 回路図の出力(svg/pdf/pdf-book)は専用ルートを持ち、帳票は共通の/export/reportへまとまる。 <sub>`export_kind_paths_match_link_api_routes`</sub>
- 帳票5種はLink APIの帳票名へ対応付き、回路図の出力は帳票ではない。 <sub>`export_kinds_map_to_the_five_reports`</sub>
- 対象を1つの端子台に絞れるのは端子台チャートと端子接続図だけ。 <sub>`only_terminal_reports_take_a_terminal_option`</sub>
- --format省略時は出力先の拡張子に従い、.pdfならPDF、それ以外はCSVになる。 <sub>`report_format_defaults_to_the_file_extension`</sub>
- Link APIへ送る出力形式の綴りは "csv" と "pdf"。 <sub>`report_format_json_names`</sub>
- 端子台一覧のURLも他のエンドポイントと同じsheet_idクエリ名を使う。 <sub>`terminals_url_uses_sheet_id_query_name`</sub>
- --terminalの値はUUID表記のときだけentity idとして扱い、それ以外は参照記号とみなす。 <sub>`uuid_shaped_values_are_recognized`</sub>
- PDF一括出力で送る帳票名の綴りは、Link API・MCPのJSON表記と同じになる。 <sub>`report_kind_json_names_match_the_link_api`</sub>
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
- 帳票の書き出しメッセージは帳票名・形式・分量(CSVは行数、PDFはページ数)を伝える。 <sub>`exported_report_reports_format_and_count`</sub>
- madake terminalsは端子台を極数・ジャンパ・シート・entity id付きで一覧する。 <sub>`terminals_lists_blocks_with_poles_and_jumpers`</sub>
- 端子台が無い場合は空の表ではなく分かりやすいメッセージを出す。 <sub>`terminals_handles_empty`</sub>
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
- 会話の色を指定すると、その会話の編集領域はその色で描かれる <sub>`並列エージェント: 会話ごとの色`</sub>
- 色を指定しない編集領域は既定色(会話1本目の色)になる <sub>`並列エージェント: 会話ごとの色`</sub>
- 同時に走る2会話の編集領域は、それぞれの色を保ったまま並ぶ <sub>`並列エージェント: 会話ごとの色`</sub>
- 会話の色は開始順(0始まり)で決まり、5本目からは先頭の色へ戻る <sub>`並列エージェント: 会話ごとの色`</sub>

### deviceTree

- 展開中のデバイスは、見出しの下に機能の行が続く <sub>`デバイスツリーの整形`</sub>
- 折りたたんだデバイスは見出しだけになり、他のデバイスの展開には影響しない <sub>`デバイスツリーの整形`</sub>
- デバイスの並びも機能の並びもRust側が決めた順のまま変えない <sub>`デバイスツリーの整形`</sub>
- 空のプロジェクトのツリーは1行も出ない <sub>`デバイスツリーの整形`</sub>
- リレーは種別と型番で呼ぶ(型番が無ければ種別だけ) <sub>`見出しの文言`</sub>
- 端子台は極数つきで呼ぶ <sub>`見出しの文言`</sub>
- その他の部品はシンボルの名前で呼び、UI言語に合わせて英語名と日本語名を使い分ける <sub>`見出しの文言`</sub>
- 機能の行の名前と所在バッジは「接点 13-14」「/2.B3」の形になる <sub>`見出しの文言`</sub>
- 機能の行をクリックすると、その機能のシートへ切り替えてエンティティを選ぶ <sub>`ツリーからの操作`</sub>
- デバイスの削除は、その参照記号の全機能のエンティティを重複なく消す <sub>`ツリーからの操作`</sub>

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

### macroPreview

- バリアントキーは既定の「A」が先頭で、そのあとにマクロが持つバリアントが並ぶ <sub>`macroVariantKeys`</sub>
- バリアントを持たないマクロのキーは「A」の1つだけ <sub>`macroVariantKeys`</sub>
- バリアントの表示名は「キー+名前」で、UI言語が日本語なら日本語名を使う <sub>`macroVariantLabel`</sub>
- 名前の無いバリアント(既定のA)はキーだけを表示する <sub>`macroVariantLabel`</sub>
- 値セットの表示名はUI言語に合わせ、日本語名が無ければ英語名を使う <sub>`macroValueSetLabel`</sub>
- 名前の無い値セットはidをそのまま表示する <sub>`macroValueSetLabel`</sub>
- 「A」と未指定は既定のコマンド列、それ以外のキーはそのバリアントのコマンド列を読む <sub>`macroCommands / macroEntities`</sub>
- 知らないバリアントキーを指定したときは既定のコマンド列に戻る(空表示にしない) <sub>`macroCommands / macroEntities`</sub>
- マクロが置くエンティティだけを取り出す(add_entity以外のコマンドは描かない) <sub>`macroCommands / macroEntities`</sub>
- 回転0のときは基準点からの相対座標をそのまま挿入位置へ足す <sub>`placePoint`</sub>
- 90度回転は用紙座標系(Y下向き)で (x,y) → (-y,x) に写る <sub>`placePoint`</sub>
- 配置後のシンボルはカーソル位置へ移り、シンボル自身の向きにも回転が加わる <sub>`placeMacroEntities`</sub>
- 配線の頂点もシンボルと同じ変換で動くので、回転しても回路の形は崩れない <sub>`placeMacroEntities`</sub>
- 配置は複製に対して行われ、元のマクロ定義は書き換えない(何度でも同じ形で置ける) <sub>`placeMacroEntities`</sub>
- プレビューは選んだバリアントのエンティティだけを載せた仮のシートを描く <sub>`macroSheet`</sub>
- マクロ名はUI言語が日本語なら日本語名、それ以外は英語名を出す <sub>`macroName`</sub>
- 日本語名が空のマクロはどちらの言語でも英語名を出す(空欄にしない) <sub>`macroName`</sub>

### relayXref

- 同じ参照記号を持つコイルと接点は、1つのリレーデバイスとしてまとめられる。 <sub>`relayXref`</sub>
- リレーのコイル・接点以外のシンボルは、リレーデバイスには含まれない。 <sub>`relayXref`</sub>
- デバイスの接点はシート順→ゾーン順に並ぶので、端子対の連番は図面の読み順どおりになる。 <sub>`relayXref`</sub>
- 端子対はIEC 60947-1に従い、先頭の数字が接点の連番、末尾がa接点なら3/4・b接点なら1/2になる。 <sub>`relayXref`</sub>
- コイル下の接点マップには、配置済みの接点が端子対と図面上の住所とともに並ぶ。 <sub>`relayXref`</sub>
- 部品の接点構成が分かっているときは、まだ使っていない接点も行として並び、所在の代わりに「—」が入る。 <sub>`relayXref`</sub>
- 接点構成が分からないときは実際に描かれた接点だけが並び、空の行は作られない。 <sub>`relayXref`</sub>
- シートごとの接点マップはコイルのentity idで引くので、そのシートに居るコイルだけが表を持つ。 <sub>`relayXref`</sub>
- それぞれの接点の脇には、その接点を動かすコイルの住所が丸括弧付きで出る。 <sub>`relayXref`</sub>
- コイルが見つからない接点には、コイル所在が一切表示されない。 <sub>`relayXref`</sub>
- 「2NO+2NC」のような接点構成は、a接点・b接点の数として読み取られ、読めない値は無視される。 <sub>`relayXref`</sub>
- 接点マップの表はコイルの真下に中央揃えで置かれ、接点1個につき1行ずつ下へ伸びる。 <sub>`relayXref`</sub>
- 接点マップはコイルの外形の下にぶら下がり、コイル所在の文字は接点シンボルの右脇に置かれる。 <sub>`relayXref`</sub>

### renderer

- 改訂欄は表題欄の真上に同じ右端・同じ幅で置かれ、行高は表題欄と同じ8mmになる <sub>`revisionLayout`</sub>
- 改訂行は古い行が下・新しい行が上に積まれ、列見出しは最下段(表題欄側)に置かれる <sub>`revisionLayout`</sub>
- 改訂が無いシートには改訂欄を作らない(空の枠も描かない) <sub>`revisionLayout`</sub>
- 改訂が7件あると新しい6件だけが描かれ、最も古い行は省略される(データは残る) <sub>`revisionLayout`</sub>
- 列は記号・日付・内容・承認の4つで、合計幅は表題欄の幅と一致する <sub>`revisionLayout`</sub>
- 表示対象の改訂は新しい方から6件までで、古い順のまま返る <sub>`visibleRevisions / effectiveRev`</sub>
- 表題欄のRev欄は最新改訂の記号を出し、改訂が無ければ表題欄の値、それも空ならハイフンを出す <sub>`visibleRevisions / effectiveRev`</sub>
- シンボル内の弧はシンボルと一緒に回るので、電磁接触器の半円はどの回転角でも可動接点の側を向く <sub>`rotateArcAngles`</sub>
- ミラーは弧を左右反転させるが、描画方向(開始→終了を時計回り)は保つ <sub>`rotateArcAngles`</sub>

### search

- 「すべて」チップは対象を絞らない <sub>`検索バーのフィルタ`</sub>
- 「ネット」チップはネット名と線番の両方を探す(図面上はどちらもネットの名前) <sub>`検索バーのフィルタ`</sub>
- 参照記号・型番・テキストのチップはそれぞれ1種類だけに絞る <sub>`検索バーのフィルタ`</sub>
- チップはデザインどおり「すべて/参照記号/型番/ネット/テキスト」の順に並ぶ <sub>`検索バーのフィルタ`</sub>
- Enterは次の結果へ進み、末尾まで行くと先頭へ回り込む <sub>`Enter巡回`</sub>
- Shift+Enterは前の結果へ戻り、先頭からは末尾へ回り込む <sub>`Enter巡回`</sub>
- まだ何も選んでいなければ、Enterで先頭、Shift+Enterで末尾を選ぶ <sub>`Enter巡回`</sub>
- 結果が0件なら選択は無いまま(巡回しても何も起きない) <sub>`Enter巡回`</sub>
- 行クリックのジャンプ先は「そのヒットのシート」と「選択する1エンティティ」 <sub>`結果行`</sub>
- 所在の列はシート名とゾーンを中黒でつないで出す <sub>`結果行`</sub>
- シンボルのヒットの種別欄は、そのシンボルがデバイスで果たす機能で説明する <sub>`結果行`</sub>
- 機能を持たないヒット(ネット名・線番・注記)は検索の対象種別そのもので説明する <sub>`結果行`</sub>
- ツリーとSurferでは、デバイス名の下に並ぶので種別を略した呼び名を使う <sub>`結果行`</sub>
- ⌘Fで検索バーが開く <sub>`検索バー`</sub>
- Escで検索バーと結果パネルが閉じ、検索語は次に開いたときのために残る <sub>`検索バー`</sub>
- 検索すると件数が出て、結果パネルが開く <sub>`検索バー`</sub>
- 検索語が空なら検索そのものを行わず、結果を捨ててパネルを閉じる <sub>`検索バー`</sub>
- 「すべて」のときは対象を絞らずに検索する <sub>`フィルタチップ`</sub>
- チップを切り替えると、その対象で検索し直す <sub>`フィルタチップ`</sub>
- 前後の空白は検索語から落とす <sub>`フィルタチップ`</sub>
- Enterで次のヒットへ進み、末尾からは先頭へ回り込む <sub>`Enter巡回と行クリック`</sub>
- Shift+Enterで前のヒットへ戻る <sub>`Enter巡回と行クリック`</sub>
- 結果が0件なら巡回しても何も選ばれない <sub>`Enter巡回と行クリック`</sub>
- 結果パネルの行をクリックすると、その行が巡回位置になる <sub>`Enter巡回と行クリック`</sub>
- 検索し直すと巡回位置は先頭より前(未選択)に戻る <sub>`Enter巡回と行クリック`</sub>
- パネルの✕は結果パネルだけ閉じ、検索バーは開いたままにする <sub>`Enter巡回と行クリック`</sub>

### surfer

- 参照記号のあるシンボルをAlt+クリックすると、同じデバイスの全機能が所在つきで並ぶ <sub>`参照サーフィン (Surfer)`</sub>
- ネットラベルをAlt+クリックすると、同名ラベルの所在が自分のシートも含めて並ぶ <sub>`参照サーフィン (Surfer)`</sub>
- 線番のついたワイヤをAlt+クリックすると、同じ線番のワイヤが全部並ぶ <sub>`参照サーフィン (Surfer)`</sub>
- 所在の並びはシート順→ゾーン順→id順で決まり、同じ図面なら常に同じ順になる <sub>`参照サーフィン (Surfer)`</sub>
- 名前も番号も付いていない要素ではポップアップを出さない <sub>`参照サーフィン (Surfer)`</sub>
- 図面に置かれていない参照記号(デバイスが見つからない)ではポップアップを出さない <sub>`参照サーフィン (Surfer)`</sub>
- 行を選ぶと、その所在のシートへ切り替えてエンティティを選択+ズームする <sub>`参照サーフィン (Surfer)`</sub>
- ↑↓の巡回は検索のEnter巡回と同じ規則で、端まで行くと回り込む <sub>`参照サーフィン (Surfer)`</sub>
- 参照記号のあるシンボルをAlt+クリックすると、同じデバイスの所在一覧が開く <sub>`Surferポップアップ`</sub>
- 巡回はクリックした要素そのものから始まる <sub>`Surferポップアップ`</sub>
- 巡回先の無い要素ではポップアップを出さない <sub>`Surferポップアップ`</sub>
- ↑↓で所在を巡回し、端まで行くと回り込む <sub>`Surferポップアップ`</sub>
- 行をクリックするとその所在が選ばれ、範囲外の行は無視される <sub>`Surferポップアップ`</sub>
- Escで閉じると巡回位置も先頭へ戻る <sub>`Surferポップアップ`</sub>

### symbolLibrary

- 検索語は英語名・日本語名・シンボルidのどれに当たっても一致する <sub>`symbolMatchesQuery`</sub>
- 名称に無い現場の呼び方 (NFB・マグネットスイッチ等) も検索キーワードで見つかる <sub>`symbolMatchesQuery`</sub>
- 空の検索語はすべてのシンボルに一致する <sub>`symbolMatchesQuery`</sub>
- シンボルはライブラリの並び順のままカテゴリごとにまとまる <sub>`groupSymbolsByCategory`</sub>
- 検索語を渡すと一致するシンボルだけが残り、空になったカテゴリは消える <sub>`groupSymbolsByCategory`</sub>

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

### 整えメトリクス (交差・重なり・グリッド)

- 整えモードは配置整理・配線整理・ラベル整頓の3種類 <sub>`tidyPrompt`</sub>
- 配置整理は2.5mmグリッドへ載せる・シンボルの重なりを解消する・列を揃えることを指示する <sub>`tidyPrompt`</sub>
- 配線整理は交差を減らし直交を保ち、余分な曲がりを減らすことを指示する <sub>`tidyPrompt`</sub>
- ラベル整頓は位置を動かすだけで、ラベルや線番を消したり書き換えたりしないよう指示する <sub>`tidyPrompt`</sub>
- どの整えも接続関係(どのピンとどのピンが繋がるか)は変えない <sub>`tidyPrompt`</sub>
- どの整えも「計測 → 編集 → 再計測」をget_tidy_metricsで行うよう指示する <sub>`tidyPrompt`</sub>
- どの整えも改善が止まったら終わり、繰り返しは最大3回まで <sub>`tidyPrompt`</sub>
- どの整えも最終応答にビフォー/アフターの数値を書かせる <sub>`tidyPrompt`</sub>
- 選択が無ければ整える対象はシート全体になる <sub>`tidyPrompt`</sub>
- 選択があれば選択したエンティティのidを並べ、それ以外は動かさないよう指示する <sub>`tidyPrompt`</sub>
- 別シートの選択が残っていても、このシートに無いidは対象にしない(シート全体扱いに戻る) <sub>`tidyPrompt`</sub>
- シートが無くてもプロンプトは壊れない <sub>`tidyPrompt`</sub>
- 整えの実行は普通のチャット送信1回(=undo一発で戻せる1ターン)になる <sub>`runTidy`</sub>
- 整えを実行するとエージェントタブが開いて経過が見える <sub>`runTidy`</sub>
- 選択中のエンティティがあれば、送るプロンプトにそのidが入る <sub>`runTidy`</sub>
- 開いている会話が答えている途中は整えを二重に投げない <sub>`runTidy`</sub>

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
- turn_appliedはターンの安定IDを載せ、「元に戻す」の対象指定に使える <sub>`chat store: applyAgentEvent`</sub>
- turn_appliedにundo深さがあれば正確な編集件数を記録する <sub>`chat store: applyAgentEvent`</sub>
- イベントを畳み込んだ会話はupdated_atが進む(履歴の最新順に反映) <sub>`chat store: applyAgentEvent`</sub>
- 完了後の新しいデルタは新しいターンを開始する <sub>`chat store: applyAgentEvent`</sub>
- 複数会話は独立に畳み込まれ、streamingは開いている会話だけを表す <sub>`chat store: applyAgentEvent`</sub>
- 未知のconversation_idでは幽霊会話を作らず一覧を取り直す <sub>`chat store: applyAgentEvent`</sub>
- 自分のターンが進行中の間は、未知会話のイベントで一覧を取り直さない <sub>`chat store: applyAgentEvent`</sub>
- 進行中ターンの無い会話へのturn_completed/errorは捨てられる <sub>`chat store: applyAgentEvent`</sub>
- 送信に失敗したら、その理由が最初の1通目からチャットに表示される <sub>`chat store: アクション`</sub>
- sendは会話を新規作成し、サーバー採番のidを引き取る <sub>`chat store: アクション`</sub>
- send解決前に届いたイベントも同じ会話へ入る <sub>`chat store: アクション`</sub>
- 空プロンプトとストリーミング中の送信は無視される <sub>`chat store: アクション`</sub>
- send失敗時はメッセージにエラーを載せてストリーミングを解除する <sub>`chat store: アクション`</sub>
- 初回送信に失敗した会話でも、再送はローカルidを渡さず新規として送れる <sub>`chat store: アクション`</sub>
- cancelは対象の会話だけを止め、他会話のストリーミングは残す <sub>`chat store: アクション`</sub>
- 採番前(local-)の会話ではcancel APIを呼ばずローカル整理だけ行う <sub>`chat store: アクション`</sub>
- 採番前のcancelは採番後にサーバーへ中断を送る <sub>`chat store: アクション`</sub>
- キャンセル後に遅れて届いた同じターンのイベントは捨てられる <sub>`chat store: アクション`</sub>
- キャンセル後に始めた新しいターン(より大きい通し番号)のイベントは通る <sub>`chat store: アクション`</sub>
- 通し番号の無いイベント(旧サーバー)はキャンセル後でも捨てない <sub>`chat store: アクション`</sub>
- cancel APIが失敗してもストリーミング解除は完了する <sub>`chat store: アクション`</sub>
- 会話が無い状態のcancelは何もせず落ちない <sub>`chat store: アクション`</sub>
- undoTurnはターン安定IDでAPIを呼び、適用済み表示を取り下げる <sub>`chat store: アクション`</sub>
- 会話に無いターンIDのundoTurnはAPIを呼ばない <sub>`chat store: アクション`</sub>
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
- 会話には開始順で編集オーバーレイ色が割り当てられ、表示順を変えても揺れない <sub>`chat store: 並列エージェント`</sub>
- 別の会話が答えている最中でも、開いている会話からは送信できる <sub>`chat store: 並列エージェント`</sub>
- 開いている会話が答えている間は、その会話への追加送信をしない <sub>`chat store: 並列エージェント`</sub>
- 会話一覧を取り直しても、実行中の会話は実行中のまま表示できる <sub>`chat store: 並列エージェント`</sub>
- ターンが終わった会話は実行中の一覧から外れる <sub>`chat store: 並列エージェント`</sub>

### devices

- タブを開くとツリーを読み込み、デバイスの件数が分かる <sub>`デバイスツリーの読み込み`</sub>
- 図面が変わっていなければ読み直さない(タブを行き来しても無駄に問い合わせない) <sub>`デバイスツリーの読み込み`</sub>
- 図面が変われば読み直す <sub>`デバイスツリーの読み込み`</sub>
- デバイス行をたたむと機能の行が隠れ、もう一度たたむと戻る <sub>`折りたたみと選択`</sub>
- 行を選ぶと選択が保持される <sub>`折りたたみと選択`</sub>
- デバイスの削除はCommand経由なのでCmd+Zで戻せる。シートを跨ぐぶんはシートごとに送る <sub>`デバイスの削除`</sub>
- 1つのシンボルが複数の機能を持つ端子台でも、削除は1回だけ送る <sub>`デバイスの削除`</sub>
- 消したデバイスの行が選ばれていたら、選択も外してツリーを読み直す <sub>`デバイスの削除`</sub>

### document

- entity_upserted / entity_removed パッチがミラーのシートを更新する <sub>`document store`</sub>
- project_replacedでミラーのプロジェクト全体が置き換わる <sub>`document store`</sub>
- シート追加・削除のパッチは並び順を保つ <sub>`document store`</sub>
- シートメタ更新のパッチはエンティティに触れない <sub>`document store`</sub>
- 古いrevisionのパッチは破棄される(二重配信しても安全) <sub>`document store`</sub>

### 回路マクロ

- 一覧を読み込むとマクロ・読めなかったファイル・置き場のパスがそろう <sub>`macro library listing`</sub>
- カテゴリツリーは「すべて」が先頭で、分類の無いマクロは「ユーザー」に入る <sub>`macro library listing`</sub>
- タイルにはバリアント数(既定のA+持っているバリアント)のバッジが付く <sub>`macro library listing`</sub>
- カテゴリを選ぶとそのカテゴリのタイルだけが並ぶ <sub>`macro library listing`</sub>
- 検索語は名前(英語・日本語)とidに部分一致し、大文字小文字は区別しない <sub>`macro library listing`</sub>
- タイルを選ぶとバリアントは既定の「A」に戻り、右のプレビューが切り替わる <sub>`macro library listing`</sub>
- 一覧の読み込みに失敗しても画面は壊れず、理由がエラーとして残る <sub>`macro library listing`</sub>
- 選択範囲があるときだけ保存ダイアログが開き、選択範囲のプレビューを組み立てる <sub>`macro save dialog`</sub>
- 何も選択していないときは保存ダイアログを開かない(マクロにする回路が無い) <sub>`macro save dialog`</sub>
- 名前が空のあいだは保存できない(名前がマクロのidになるため) <sub>`macro save dialog`</sub>
- 保存は選択範囲・名前・カテゴリをそのまま渡し、保存後は一覧を読み直して閉じる <sub>`macro save dialog`</sub>
- 保存に失敗したらダイアログは開いたまま理由を出す(入力をやり直せる) <sub>`macro save dialog`</sub>
- 基準点は選択範囲から自動で決まり(左下ピン)、保存ダイアログには表示だけする <sub>`macro save dialog`</sub>
- プレースホルダの候補は、選択したシンボルの型番欄と属性の一覧から作られる <sub>`placeholders and value sets in the save dialog`</sub>
- 同じキー名を付けた複数の欄は、行き先を複数持つ1つのプレースホルダにまとまる <sub>`placeholders and value sets in the save dialog`</sub>
- 値セットは行を足して名前とキーごとの値を入れると組み立てられ、idは名前から作られる <sub>`placeholders and value sets in the save dialog`</sub>
- 名前を入れていない値セットの行は保存されない(空の値セットは作らない) <sub>`placeholders and value sets in the save dialog`</sub>
- プレースホルダと値セットは保存時にマクロの情報として一緒に渡される <sub>`placeholders and value sets in the save dialog`</sub>
- 何も指定しなければ保存の中身は今までどおり(プレースホルダも値セットも付かない) <sub>`placeholders and value sets in the save dialog`</sub>
- 保存ダイアログを開き直すと、前回のプレースホルダと値セットは残らない <sub>`placeholders and value sets in the save dialog`</sub>
- 値セットを持たないマクロでは選ぶものが無い(挿入ダイアログのドロップダウンを出さない) <sub>`choosing a value set when inserting`</sub>
- 値セットを持つマクロでは一覧が並び、選んだ値セットを覚える <sub>`choosing a value set when inserting`</sub>
- 別のマクロを選び直すと、値セットの選択は外れる(そのマクロには無い値セットのため) <sub>`choosing a value set when inserting`</sub>
- ⌘Cは選択範囲を無名マクロとしてメモリに持ち、ファイルには書き出さない <sub>`unnamed clipboard macro (Cmd+C / Cmd+V)`</sub>
- 何も選択していない状態の⌘Cは何もしない(前のコピー内容も消さない) <sub>`unnamed clipboard macro (Cmd+C / Cmd+V)`</sub>
- コピーした無名マクロは何度でも貼り付けられる(貼り付けても消えない) <sub>`unnamed clipboard macro (Cmd+C / Cmd+V)`</sub>
- 何もコピーしていないときの⌘Vは何も起こさない <sub>`unnamed clipboard macro (Cmd+C / Cmd+V)`</sub>

### 部品データベース

- searchは部品APIの結果を保持する <sub>`parts store`</sub>
- 検索失敗時は結果を空にしloadingを戻す <sub>`parts store`</sub>

### provider

- 既定のプロバイダはClaude Code CLIで、API経由に切り替えたときのモデルはClaude Sonnet 5 <sub>`AI provider settings`</sub>
- プロバイダを選び直すと設定として保存される <sub>`AI provider settings`</sub>
- プロバイダの状態を読み込むと「キーが保存済みか」が分かる <sub>`AI provider settings`</sub>
- 保存したAPIキーの文字列はフロントの状態に一切残らない <sub>`AI provider settings`</sub>
- キー欄が空のままなら送信せず、入力を促すエラーになる <sub>`AI provider settings`</sub>
- キーを削除すると「保存済み」表示が消える <sub>`AI provider settings`</sub>
- 接続テストは成功すると確かめたモデル名を表示する <sub>`AI provider settings`</sub>
- 接続テストが失敗すると理由をそのまま表示する <sub>`AI provider settings`</sub>
- 接続テストの実行中はtestingが立ち、前回の結果は消える <sub>`AI provider settings`</sub>
- 接続バッジはClaude Code CLIならCLIの検出、Anthropic APIならキーの保存状況を見る <sub>`AI provider settings`</sub>
- 保存済みのキーは伏せ字で表す(値そのものは画面に出さない) <sub>`AI provider settings`</sub>
- プロバイダの選択肢にGitHub Copilot CLIがある <sub>`GitHub Copilot CLI provider`</sub>
- Copilotの既定はモデルをCopilotに任せる`auto`で、実行ファイルはPATHから探す <sub>`GitHub Copilot CLI provider`</sub>
- GitHub Copilot CLIを選ぶと設定として保存される <sub>`GitHub Copilot CLI provider`</sub>
- Copilotのモデルと実行ファイルのパスは設定として保存できる <sub>`GitHub Copilot CLI provider`</sub>
- プロバイダの状態からCopilot CLIが見つかったか(とバージョン)が分かる <sub>`GitHub Copilot CLI provider`</sub>
- Copilotを選んでいるときの接続バッジは、Copilot CLIが見つかったかどうかを見る <sub>`GitHub Copilot CLI provider`</sub>
- Copilotが未認証のときは、接続テストがサインイン手順つきで失敗を返す <sub>`GitHub Copilot CLI provider`</sub>
- Copilotの認証情報(GitHubトークン)はフロントの状態に一切持たない <sub>`GitHub Copilot CLI provider`</sub>
- プロバイダの選択肢にOpenAI互換APIがある <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- 既定の接続先はOpenAI本体で、モデル名は空(接続先ごとに違うので必ず選ばせる) <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- 「Ollama (ローカル)」プリセットはローカルの11434番へ向ける <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- OpenAI互換APIのURLとモデル名は設定として保存できる <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- OpenAI互換APIのキーはAnthropicのキーとは別枠で保存され、値は状態に残らない <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- OpenAI互換APIのキーを削除すると、そのプロバイダ指定で削除が呼ばれる <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- 接続バッジは、キー保存済み(またはローカルURL)でモデル名が入っているときだけ「接続済み」 <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- プロバイダの状態からOpenAI互換APIのURL・モデル・キーの保存状況が分かる <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- キーが弾かれたときは、接続テストがOpenAI互換API用の区分つきで失敗を返す <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- プロバイダの選択肢にGoogle Geminiがあり、既定のモデル名が入っている <sub>`Google Gemini provider`</sub>
- Geminiのモデル名は設定として保存できる <sub>`Google Gemini provider`</sub>
- GeminiのキーはAnthropic・OpenAI互換とは別枠で保存され、値は状態に残らない <sub>`Google Gemini provider`</sub>
- Geminiのキーを削除すると、そのプロバイダ指定で削除が呼ばれる <sub>`Google Gemini provider`</sub>
- 接続バッジは、キーが保存済みでモデル名が入っているときだけ「使えます」 <sub>`Google Gemini provider`</sub>
- プロバイダの状態からGeminiのモデルとキーの保存状況が分かる <sub>`Google Gemini provider`</sub>
- キーが弾かれたときは、接続テストがGemini用の区分つきで失敗を返す <sub>`Google Gemini provider`</sub>

### 帳票 (部品表 / 電線リスト)

- 帳票の種類は From-To電線リスト・端子台チャート・端子接続図・部品表・XRef表 の5つ <sub>`report generation dialog store`</sub>
- 部品表はプロジェクト全体が対象で、既定はCSV出力 <sub>`report generation dialog store`</sub>
- 出力形式を図面シートPDFにすると、同じ帳票がPDF要求になる <sub>`report generation dialog store`</sub>
- 端子台チャートは「プロジェクト全体」に加えて端子台1つを対象に選べる <sub>`report generation dialog store`</sub>
- 端子接続図はグラフィカルな図面なのでPDFだけを選べる <sub>`report generation dialog store`</sub>
- 「帳票のシート化」から開くと図面シートPDFを選んだ状態で開く <sub>`report generation dialog store`</sub>
- 帳票にその出力形式が無ければ、選べる形式に読み替えて開く <sub>`report generation dialog store`</sub>
- ダイアログを開いたまま帳票を変えると、対象と出力形式もその帳票のものに入れ替わる <sub>`report generation dialog store`</sub>
- 既定のファイル名は帳票名と対象と出力形式から決まる <sub>`report generation dialog store`</sub>
- 出力先パスが空のままでは生成できない <sub>`report generation dialog store`</sub>
- 生成すると帳票を1回だけ書き出し、書き出した件数を返して閉じる <sub>`report generation dialog store`</sub>
- 書き出しに失敗したらダイアログは開いたままエラーを表示する <sub>`report generation dialog store`</sub>
- 既定では表紙付きで、全ての帳票が回路図の後ろに付く <sub>`PDF book dialog store`</sub>
- 選んだ帳票は種類の並び順どおりに並ぶ(選んだ順ではない) <sub>`PDF book dialog store`</sub>
- 表紙は外せる。帳票を1つも選ばなければ回路図だけのPDFになる <sub>`PDF book dialog store`</sub>
- 出力先パスが空のままでは出力できない <sub>`PDF book dialog store`</sub>
- 出力すると1ファイルに書き出し、ページ数を返して閉じる <sub>`PDF book dialog store`</sub>

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
- 知識ファイルのパスを保存でき、空文字はバックエンドが未設定へ正規化する <sub>`settings store`</sub>
- save失敗時はエラーを保持し、表示中の設定を変えない <sub>`settings store`</sub>

### simulation

- runでDC解析結果を取得してパネルを開く <sub>`simulation store`</sub>
- 開閉トグルは再実行時にopen_switchesとして渡される <sub>`simulation store`</sub>
- 失敗時(ngspice未導入等)はエラーメッセージを保持したままパネルを開く <sub>`simulation store`</sub>

### 開始テンプレート

- ダイアログを開くとテンプレート一覧を読み込み、先頭を選んだ状態で表示する <sub>`template picker store`</sub>
- タイルを選ぶと選択中のテンプレートが切り替わる (右のプレビューが変わる) <sub>`template picker store`</sub>
- 「このテンプレートで開始」で選択中のテンプレートが現在のシートへ適用され、ダイアログが閉じる <sub>`template picker store`</sub>
- 適用したテンプレートは図面ミラーにも反映され、「元に戻す」が使える状態になる <sub>`template picker store`</sub>
- キャンセルではダイアログが閉じるだけで、図面には何も適用されない <sub>`template picker store`</sub>
- 適用に失敗したときは理由を表示し、ダイアログは開いたままにする (選択をやり直せる) <sub>`template picker store`</sub>
- 読み込めなかったテンプレートファイルはパスと理由つきで持ち帰り、残りのテンプレートは選べる <sub>`template picker store`</sub>
- テンプレートが1つも無いときは選択なしで開き、適用しても何も起きない <sub>`template picker store`</sub>
- 「+ ユーザーテンプレートを追加...」はテンプレートフォルダをファイラで開く <sub>`template picker store`</sub>
- ファイラを開けない環境では、テンプレートを置くフォルダのパスだけを案内する <sub>`template picker store`</sub>
- 名前と説明はUI言語に従い、日本語以外では英語表記になる <sub>`template picker store`</sub>
- プレビューはテンプレートが置くエンティティを仮のシートに組み立てて描く <sub>`template preview`</sub>
- プレビューは図面の描画範囲を求め、その中心がキャンバスの中心に来るよう合わせる <sub>`template preview`</sub>
- 空のテンプレートでもプレビューは既定のビューポートを返す (描画で落ちない) <sub>`template preview`</sub>

### terminals

- ジャンパ指定は「小さい端子番号が先・昇順・重複なし」に正規化される <sub>`jumper specification`</sub>
- 壊れた記述と隣り合わない端子のジャンパは読み飛ばす <sub>`jumper specification`</sub>
- 隣り合う端子を選んでジャンパを掛けると既存のジャンパは残る <sub>`jumper specification`</sub>
- 3つ以上の端子を選ぶと隣どうしを順につないだジャンパになる <sub>`jumper specification`</sub>
- 隣り合わない端子どうしにはジャンパを掛けられない <sub>`jumper specification`</sub>
- ジャンパ削除は選んだ端子に掛かっているものだけを外す <sub>`jumper specification`</sub>
- 開くとそのシートの端子台一覧を読み込み、先頭の端子台のチャートを表示する <sub>`terminal strip editor store`</sub>
- グリッドは端子番号ごとに1行で、外部側・内部側・線番・電線が図面どおりに並ぶ <sub>`terminal strip editor store`</sub>
- 電線が1本も繋がっていない端子は予備端子として印が付く <sub>`terminal strip editor store`</sub>
- 隣り合う2端子を選んでジャンパを生成すると、update_entityコマンド1回で属性に書き込まれる <sub>`terminal strip editor store`</sub>
- 端子を選んでいないとジャンパは生成できない <sub>`terminal strip editor store`</sub>
- 隣り合わない端子を選んでもジャンパは生成できない <sub>`terminal strip editor store`</sub>
- ジャンパ削除は選んだ端子に掛かるジャンパだけを外したコマンドになる <sub>`terminal strip editor store`</sub>
- ジャンパが1本も残らないときは属性ごと消す <sub>`terminal strip editor store`</sub>
- ジャンパを編集したらチャートを読み直し、グリッドが図面と一致し続ける <sub>`terminal strip editor store`</sub>
- 図面が外から変わったら (undoやAIの編集) グリッドを取り直し、古いチェック結果は捨てる <sub>`terminal strip editor store`</sub>
- 端子台エディタを閉じている間は図面が変わっても読み込みに行かない <sub>`terminal strip editor store`</sub>
- 端子台チェックの結果は重大度ごとの件数に整形される <sub>`terminal strip editor store`</sub>
- 問題が1件も無ければチェックは「問題なし」になる <sub>`terminal strip editor store`</sub>
- 端子台を切り替えると選択と直前のチェック結果は消える <sub>`terminal strip editor store`</sub>
- 端子台が1つも無いシートでは行が空になり、生成もチェックもできない <sub>`terminal strip editor store`</sub>
- 閉じると選択・チャート・チェック結果を捨てる <sub>`terminal strip editor store`</sub>

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

### macroPlacement

- マクロを配置し始めるとマクロツールになり、バリアントAで回転0のゴーストが出る <sub>`starting macro placement`</sub>
- Escでマクロ配置をやめると選択ツールへ戻り、ゴーストが消える <sub>`starting macro placement`</sub>
- 配置中のRは90度ずつ回転し、4回で元の向きへ戻る <sub>`R and Tab during macro placement`</sub>
- 配置中のTabはバリアントをA→B→C→Aと巡回し、ゴーストが差し替わる <sub>`R and Tab during macro placement`</sub>
- バリアントが1つしか無いマクロではTabを押しても「A」のまま <sub>`R and Tab during macro placement`</sub>
- マクロを配置していないときのTabは横取りしない(キャンバス外の操作を邪魔しない) <sub>`R and Tab during macro placement`</sub>
- ライブラリのマクロはid・バリアント・クリック位置・回転を渡して挿入される <sub>`confirming a macro placement`</sub>
- 貼り付けた無名マクロはidではなくマクロそのものを渡して挿入される(ファイルが無いため) <sub>`confirming a macro placement`</sub>
- 確定してもマクロツールのままなので、同じマクロを続けて何個でも置ける <sub>`confirming a macro placement`</sub>
- 挿入ダイアログで選んだ値セットは、挿入の引数にそのまま乗る(定格が一括で入る) <sub>`placing a macro with a value set`</sub>
- 値セットを選ばずに置いたマクロは、保存時の値のまま入る <sub>`placing a macro with a value set`</sub>
- 選択範囲を⌘Cすると無名マクロとして覚え、⌘Vでそのまま配置モードに入る <sub>`Cmd+C / Cmd+V`</sub>
- 何も選択していない⌘C・何もコピーしていない⌘Vはキー入力を横取りしない <sub>`Cmd+C / Cmd+V`</sub>

