# 詳細仕様設計書

**English (canonical): [13-specification.md](13-specification.md)** | ← [ロードマップ](12-roadmap.ja.md)

> **テストスイートから自動生成 — 手で編集しないこと。**
> 以下の各項目は自動テストで常に検証されている(灰色はテストID)。
> テスト変更後は `python3 scripts/gen_spec.py` で再生成する。

本書はMadakeCADの「生きた仕様書」である:
ここに載っている挙動は、テスト実行のたびに証明される。


全6領域・**2897仕様項目**。


## コアドメイン (madake-core)


### block

- ブロックシンボルは名前付きピンを箱の左右に5mmピッチ・2.5mmグリッド上で中央揃えに並べ、ピンはそれぞれ外向きになる。 <sub>`block_pins_sit_on_both_sides_five_millimetres_apart`</sub>
- ピン番号は外形の外側(リードの脇)に、ピン名は外形の内側に描かれる。 <sub>`pin_numbers_are_drawn_outside_and_pin_names_inside`</sub>
- 外形はピン数とピン名の長さに応じて自動で大きくなり、寸法は必ず5mm刻みになる。 <sub>`auto_size_grows_with_pin_count_and_name_length`</sub>
- ピン表がまったく無いブロックも、最小サイズの空の箱として有効に作られる。 <sub>`a_block_without_a_pin_table_is_an_empty_box`</sub>
- 上辺・下辺のピンは左から右へ並び、文字を縦向きに描けないので番号・名前は箱の内側に置かれる。 <sub>`top_and_bottom_pins_run_left_to_right`</sub>
- 外形寸法を明示しても5mm刻みへ切り上げられ、外形はグリッド上に載ったままになる。 <sub>`an_explicit_size_is_rounded_up_to_the_grid`</sub>
- 同じピン番号が2つある、またはピン番号が空のブロックは、黙って描かれずにエラーになる。 <sub>`duplicate_or_empty_pin_numbers_are_rejected`</sub>
- ピンが収まらない寸法は拒否され、JSONとして壊れたピン表や知らない辺名も同じく拒否される。 <sub>`a_size_too_small_or_a_malformed_pin_table_is_rejected`</sub>
- ブロックのピン数には上限があり、桁違いのピン表ははっきりエラーになる。 <sub>`too_many_pins_are_rejected`</sub>
- 属性スロット(参照記号・型番・機能・定格)は箱の中央に縦に積まれる。 <sub>`attribute_slots_stack_at_the_centre_of_the_box`</sub>
- ブロックを回転・ミラーしてもピンは本体と一緒に動くだけで、番号は付け替わらない。 <sub>`rotating_or_mirroring_keeps_the_pin_numbers`</sub>

### cable

- ケーブルを置くと、定義線が横切った配線が始点に近い順に芯1・2・3として割り当たり、芯の色は配線の線色になる。undo一発でケーブルが消える。 <sub>`placing_a_cable_assigns_the_crossed_wires_as_cores_in_order`</sub>
- 芯数を指定せずに置いたケーブルは、横切った配線の本数を芯数にする。定義線が届かない配線は入らない。 <sub>`a_cable_without_a_core_count_takes_the_number_of_crossed_wires`</sub>
- ケーブルの注記は参照記号・型番・芯数×線径・長さの順に並ぶ。 <sub>`the_cable_label_reads_designator_part_cores_gauge_and_length`</sub>
- ケーブルの芯数より多い配線を割り当てるとERCのErrorになり、そのケーブルを指す。 <sub>`more_assigned_wires_than_cores_is_an_erc_error`</sub>
- ケーブル一覧は、ケーブルごとに型番・芯数・線径・長さ・配線両端の機器・使用芯/予備芯・シールド・シートを並べる。 <sub>`the_cable_list_shows_ends_used_and_spare_cores`</sub>
- 部品表ではケーブルを型番ごとに数え、全部に長さがあれば合計の長さ、1本でも無ければ本数にする。型番の無いケーブルは載せない。 <sub>`the_bom_counts_cables_per_part_number`</sub>
- 印刷される図面には、ケーブルの定義線と、シールドがあればシールドの楕円、注記が描かれる。 <sub>`the_printed_sheet_draws_the_definition_line_shield_and_label`</sub>
- 定義線が2点でない・芯数が0のケーブルは拒否される。芯の配線がシートに無い (先に消された) だけなら受け付けてERCに任せる (ケーブルの削除の取り消しが失敗しないように)。 <sub>`a_malformed_cable_is_refused`</sub>
- 芯の配線が消えた定義線ケーブルでも、ネットリスト・ERC・ケーブル一覧・電線リスト・部品表・印刷図面は落ちずに作れる。名前も変えられ、ケーブルを消してから2回元に戻すとケーブルも配線も戻る。 <sub>`a_definition_cable_whose_wire_is_gone_still_works`</sub>
- 区間ケーブルは、端の無い芯・同じ配線やピンの使い回し・配線の端が部品を指す/ピンの端が配線を指す芯・定義線用の欄を使った芯・2点未満の区間があれば断る。芯の行き先を持つ定義線ケーブルも断る。行き先の配線・部品・ピンがシートに無いだけなら受け付ける (ERCが知らせる)。 <sub>`a_run_cable_is_checked_when_placed`</sub>
- 区間ケーブルの芯2本が同じ呼び名 (前後の空白を除いて) なら置くときに断り、重なった呼び名を示す。 <sub>`a_run_cable_with_duplicate_core_names_is_refused`</sub>
- 分岐の配線を1本消し、別の編集でケーブルを消してから2回元に戻すと、ケーブルも配線も戻り、やり直すとまた消える。 <sub>`deleting_a_breakout_wire_then_the_cable_can_be_undone`</sub>
- 区間ケーブルが着く部品を消し、別の編集でケーブルを消してから2回元に戻すと両方が戻り、やり直すとまた消える。 <sub>`deleting_the_landing_part_then_the_cable_can_be_undone`</sub>
- 分岐の配線が消えた区間ケーブルも更新 (名前の変更) ができ、ERCは行き先の無い芯を知らせ続ける。 <sub>`a_run_cable_whose_wire_is_gone_can_be_updated`</sub>
- 区間ケーブルを分岐の配線と着く先の部品と一緒に消してから元に戻すと全部が戻り、やり直すとまた消える (ケーブルは指す相手の後に戻す)。 <sub>`deleting_a_run_cable_with_its_wires_can_be_undone`</sub>
- 1本線の区間で描いたLTEハーネスはERCを通る: 短絡も、未接続のプラグのピンも、宙ぶらりんの分岐の配線も、ケーブルの指摘も出ない。 <sub>`the_lte_harness_passes_erc`</sub>
- 1本のケーブルの芯2本が同じネットにあるとERCのエラーになり、ケーブルと両方の芯を示す (区間ケーブルでも定義線ケーブルでも)。 <sub>`two_cores_in_one_net_is_a_short`</sub>
- 区間ケーブルの芯が指す配線や部品が消えているとERCのエラーになり、その芯を示す。 <sub>`a_core_whose_target_is_gone_is_unassigned`</sub>
- 芯が着く部品から1本線の端が離れていると警告になり、部品に触れていれば警告しない。 <sub>`a_run_end_away_from_its_part_is_warned`</sub>
- 区間ケーブルの芯の配線の分岐側の端は宙ぶらりんにならない (ケーブルが無ければ宙ぶらりん)。 <sub>`breakout_ends_are_not_dangling`</sub>
- 芯数より多い芯を持つ区間ケーブルは、定義線ケーブルと同じ芯数超過のエラーになる。 <sub>`a_run_cable_with_too_many_cores_overflows`</sub>
- 両端とも未割当の区間ケーブルの芯は「行き先なし」で報告し、浮いたネットや短絡など別の指摘を重ねない。 <sub>`a_core_with_both_ends_unassigned_is_only_unassigned`</sub>
- ケーブル一覧は1本線のケーブルの両端の機器 (分岐の配線が行く基板、1本線が着くプラグ) を示し、両端のある芯を使用芯と数える。 <sub>`the_cable_list_shows_the_ends_of_a_run_cable`</sub>
- 印刷される図面は、1本線のケーブルを太い線と斜線、着いたプラグのピンを横切る棒、分岐の配線への扇と芯の呼び名、注記で描く。 <sub>`the_printed_sheet_draws_a_run_cable`</sub>

### cable_run

- LTEハーネスを計画すると、基板のピンから10mm先の分岐点まで芯ごとに1本ずつ配線を引き、そこからプラグまで1本線を通し、芯をピン順にピンへ対応させる。結果は4ネットになる。 <sub>`plans_the_lte_harness`</sub>
- 全長1本線で結ぶ2つのコネクタ (3.5mm AUX) は、ピン列の間に1本線が通り、配線は無い。 <sub>`plans_a_single_line_between_two_connectors`</sub>
- ピンは番号でも名前でも指定でき、ピンの数が芯の数と違う端は、その端と両方の数を示して断る。 <sub>`pins_by_name_and_a_count_mismatch`</sub>
- 同じ呼び名 (前後の空白を除いて) の芯が2本あるケーブルの計画は断り、重なった呼び名を示す。 <sub>`duplicate_core_names_are_refused`</sub>
- 貫通端子台に着く1本線は、芯を端子番号1つずつに対応させ、ふだんは左の接続点、「#2」を付ければ右の接続点に着く。 <sub>`a_single_line_on_a_through_terminal_block`</sub>
- ピンより後ろに置いた分岐点や、芯の配線の端が別の配線に乗る分岐点は、breakout_at をずらすよう伝えて断る。 <sub>`a_bad_breakout_is_refused`</sub>

### Commandエンジン (undo/redo)

- ライブラリに無いシンボルidは、描かれない幽霊エンティティとして保存されるのではなく拒否され、図面は何も変わらない。 <sub>`an_unknown_symbol_id_is_refused_and_changes_nothing`</sub>
- 極数可変シンボルを範囲外の極数 (0極や上限超え) で頼むと、使える範囲を書いたメッセージで拒否される。 <sub>`a_pole_count_outside_the_range_is_refused`</sub>
- ふつうの配置はこれまでどおり通る: 同梱シンボル・範囲内の極数可変シンボル・属性で形が決まるブロック。 <sub>`valid_symbol_ids_are_still_accepted`</sub>
- 既にある部品を知らないシンボルidへ変更するのも拒否されるので、図面の部品は常に描ける。 <sub>`changing_a_symbol_to_an_unknown_id_is_refused`</sub>
- ピン表が壊れている(番号が重複した)ブロックは追加時に拒否され、図面は何も変わらない。 <sub>`a_block_with_a_broken_pin_table_is_refused_and_changes_nothing`</sub>
- 正しいブロックを壊れた内容へ編集するのも拒否されるので、図面上のブロックは常に描けるピン表を持つ。 <sub>`editing_a_block_into_a_broken_one_is_refused`</sub>
- 図面が知らない色名 (打ち間違いや生の色コード) を付けた注記は、黙って黒で描かれるのではなく拒否される。 <sub>`a_note_with_an_unknown_colour_is_refused`</sub>
- 図面の色トークンを使った注記 (色を指定しない注記も) は受け付けられる。 <sub>`a_note_with_a_known_colour_token_is_accepted`</sub>
- 既にある注記を知らない色へ変える更新も拒否され、注記は元の色のままになる。 <sub>`recolouring_a_note_to_an_unknown_colour_is_refused`</sub>
- シート上に存在しないIDへ引き出し線を向けた注記は、いつまでも描かれない引き出し線を持ったまま保存されるのではなく拒否される。 <sub>`a_note_with_an_unknown_leader_target_is_refused`</sub>
- 注記はシート上の他のエンティティへ引き出し先を設定でき、引き出し先を外す更新 (Noneへ戻す) も他の更新と同じくundoできる。 <sub>`a_note_can_target_an_existing_entity_and_the_leader_can_be_cleared`</sub>
- エンティティの追加はundoで消え、redoで復活する。 <sub>`add_undo_redo_entity`</sub>
- 移動でエンティティの座標が動き、undoで元の位置に正確に戻る。 <sub>`move_and_undo_restores_position`</sub>
- 複数エンティティの一括削除は1回のundoで全て復元される。 <sub>`delete_multiple_and_undo`</sub>
- シートは追加・削除でき、削除のundoは元のid・位置のままシートを復元する。 <sub>`sheet_add_remove_undo`</sub>
- 種別を指定しないシート追加は通常の回路図シートになり、盤レイアウトシートは種別を指定して追加する(どちらもundoで消える)。 <sub>`add_sheet_defaults_to_schematic_and_can_add_a_panel_sheet`</sub>
- 盤シートが無かった頃に書かれたシート追加コマンド(種別の欄が無い)もそのまま動き、回路図シートを追加する。 <sub>`a_sheet_add_command_without_a_kind_still_adds_a_schematic_sheet`</sub>
- 盤エンティティ4種(盤外形・DINレール・ダクト・フットプリント)は通常のエンティティ用コマンドで追加・移動・削除でき、どの操作もundoで戻せる。 <sub>`panel_entities_are_added_moved_and_deleted_with_undo`</sub>
- フットプリントの編集(参照記号・型番・回転)は丸ごと置換で、undoで編集前に戻る。 <sub>`a_footprint_can_be_edited_and_undone`</sub>
- CommandはJSONに往復変換でき、どのクライアントからも送信できる。 <sub>`command_json_roundtrip`</sub>
- UpdateEntityはエンティティを丸ごと置換し、undoで置換前の状態に戻る。 <sub>`update_entity_undo_restores_previous_version`</sub>
- SetTitleBlockはシートの表題欄を更新し、undoで以前の内容に戻る。 <sub>`set_title_block_is_undoable`</sub>
- SetRevisionsは改訂欄の行を置き換え、undoで以前のリストに戻る。 <sub>`set_revisions_is_undoable`</sub>
- シートの注記文字高はコマンドで設定し、undoで以前の高さに戻る。 <sub>`set_sheet_annotation_text_is_undoable`</sub>
- 注記文字高をnullで消すと、シートは既定の2.0mmに戻る。 <sub>`clearing_the_annotation_text_height_restores_the_default`</sub>
- 1.0〜3.5mmの範囲を外れた注記文字高は理由付きで拒否され、シートは書き換わらない。 <sub>`an_out_of_range_annotation_text_height_is_rejected`</sub>
- 注記文字高のコマンドは素のJSONで表現でき、AIやCLIから送れる。 <sub>`set_sheet_annotation_text_is_plain_json`</sub>
- SetPlcSheetOptionsはPLC I/O図面の生成設定をプロジェクトに保存し、undoで保存前の状態に戻る。 <sub>`set_plc_sheet_options_saves_the_settings_and_undo_restores_them`</sub>
- SetPlcAssignmentsはプロジェクトのPLC I/O割付表を丸ごと置き換え、undoで以前の表に戻る。 <sub>`set_plc_assignments_is_undoable`</sub>
- SetFrameTemplateはプロジェクト全体の図枠テンプレートを1回の編集で差し替え、undoで元の図枠へ戻る(最初の差し替えをundoすると未指定=既定の図枠へ戻る)。 <sub>`set_frame_template_is_undoable`</sub>
- SetFrameTemplateは知らないidも受け付ける(図面は既定の図枠で描かれ続けるので、他所で作ったプロジェクトも開ける)。 <sub>`set_frame_template_accepts_an_unknown_id`</sub>
- ハーネス境界は通常のエンティティ用コマンドで追加・改名・削除でき、どの操作もundoで戻せる。 <sub>`harness_add_rename_delete_are_undoable`</sub>
- ハーネス境界を移動すると4隅すべてが動き、undoで元の位置に正確に戻る。 <sub>`harness_move_and_undo_restores_every_corner`</sub>
- ハーネス境界は kind="harness" としてJSONに往復変換でき、どのクライアントからも送れる。 <sub>`harness_command_json_roundtrip_uses_the_harness_kind`</sub>
- 4種類すべての図形が通常のエンティティ用コマンドで追加・編集・削除でき、どの操作もundoで戻せる。 <sub>`shape_add_edit_delete_are_undoable_for_every_kind`</sub>
- 図形を移動すると全ての頂点が動き、undoで元の位置に正確に戻る。 <sub>`shape_move_and_undo_restores_every_point`</sub>
- 色トークンではなく生の色コードを入れた図形は拒否され、図面は何も変わらない。 <sub>`a_shape_with_a_raw_colour_code_is_refused`</sub>
- 印刷できないほど細い線幅・グリッドを潰すほど太い線幅の図形は拒否される。 <sub>`a_shape_with_an_unprintable_stroke_width_is_refused`</sub>
- 頂点の数が合っていない矩形は拒否されるので、描きかけの図形が図面へ入ることはない。 <sub>`a_shape_with_the_wrong_number_of_points_is_refused`</sub>
- 図形は kind="shape" としてJSONに往復変換でき、どのクライアントからも送れる。 <sub>`shape_command_json_roundtrip_uses_the_shape_kind`</sub>
- execute/undo/redoのたびにドキュメントrevisionが増加し、クライアントは古いpatchを破棄できる。 <sub>`revision_increases_monotonically`</sub>
- 存在しないシートへのCommandはエラーになり、何も変更されない。 <sub>`unknown_sheet_is_rejected`</sub>
- 履歴が空のときのundoはエラーではなくNoneを返す。 <sub>`undo_on_empty_history_returns_none`</sub>
- 適用できないundoは図面を何も変えず、その編集をundo履歴に残す (半分だけ戻ることも、失われることもない)。 <sub>`a_failed_undo_keeps_the_edit_in_the_history`</sub>
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
- 盤の採番は、そのシートのフットプリント全てに読み順のアイテム番号を振る。 <sub>`renumbering_a_panel_numbers_every_footprint`</sub>
- 採番のundoは元のアイテム番号を戻す(番号が無かった部品は番号の無い状態へ戻る)。 <sub>`undoing_the_numbering_restores_the_previous_numbers`</sub>
- 同じ盤をもう一度採番しても図面は何も変わらない(すでに読み順どおりの番号が入っているため)。 <sub>`numbering_an_already_numbered_panel_changes_nothing`</sub>
- 採番コマンドは素のJSONで表現できるので、AIやCLIから送れる。 <sub>`the_panel_numbering_command_is_plain_json`</sub>
- 存在しないシートの採番は、黙って何もせずに断られる。 <sub>`numbering_an_unknown_sheet_is_refused`</sub>
- 8極の端子台を開始番号から採番すると、全ピンの新しい番号がピン順に書き込まれる。 <sub>`renumbering_writes_every_terminal_number_from_the_start_value`</sub>
- 多段端子台の採番は各極の段記号を保ったまま極番号だけを変えるので、同じ極の全段が同じ新番号を共有する。 <sub>`renumbering_a_multilevel_block_keeps_the_level_letters`</sub>
- 接頭辞と1より大きい増分はどちらも効く: 増分が極番号を進め、接頭辞は全ラベルの前に付く。 <sub>`renumbering_applies_the_step_and_prefix`</sub>
- 採番は同じコマンド内でジャンパの指定も新しい番号へ書き換えるので、ジャンパは実際に繋ぐ端子を指したままになる。 <sub>`renumbering_rewrites_the_jumpers_to_the_new_labels`</sub>
- 採番はピングループの範囲の両端も新しい番号へ書き換える。範囲は端子番号で指しているため。 <sub>`renumbering_rewrites_pin_group_ranges_too`</sub>
- 採番したあとでもピングループの括弧は図面に描かれ続け、図面検証も無指摘のままになる: 保存された範囲はその端子台の表示番号と突き合わせられるため。 <sub>`a_pin_group_bracket_still_draws_after_renumbering_the_terminals`</sub>
- 多段端子台で1段ぶんの端子を指すピングループの範囲も、段記号が保たれるので採番後に括弧を描き続ける。 <sub>`a_multilevel_pin_group_bracket_survives_a_renumbering`</sub>
- 読めないジャンパの項目 (や端子台に無い端子を指す項目) は採番をそのまま通り抜けるので、図面検証はそれを指し続け、undoで属性は1バイトも変わらず戻る (ここでは旧番号と新番号のラベル集合が重ならないため。重なる場合の例外は`undoing_a_renumbering_of_an_unreadable_item_is_not_byte_identical_when_old_and_new_labels_overlap`を参照)。 <sub>`a_broken_jumper_item_survives_a_renumbering_and_comes_back_unchanged_on_undo`</sub>
- 採番のundoは、読めないジャンパの項目を必ずしも1バイトも変えずに戻すとは限らない: 旧番号と新番号のラベル集合が重なると、順方向では旧番号として解決できなかったトークン(旧番号には存在しなかったため)が、逆方向では新番号として解決できてしまうことがあり、順方向では触れなかった項目に逆方向だけが触れてしまう。これはドキュメント化済みの境界事例 (docs/05-wire-management) であり、このタスクで直す不具合ではない。 <sub>`undoing_a_renumbering_of_an_unreadable_item_is_not_byte_identical_when_old_and_new_labels_overlap`</sub>
- 採番のundoはピングループの指定を空白ごと1バイトも変えずに戻す: 書き換えは認識できた端子番号のトークンだけを置き換えるため。 <sub>`undoing_a_renumbering_restores_the_pin_group_spec_byte_for_byte`</sub>
- 3端子にまたがるジャンパの連鎖は増分2の採番にも追従する: 各リンクの両端が新しい番号になり、連鎖のままになる。 <sub>`a_jumper_chain_follows_a_renumbering_with_a_step`</sub>
- 端子台ではないシンボルの採番は拒否される: 端子番号の上書きはそのシンボルのピン番号テキストを図面上で書き換えてしまうため。 <sub>`renumbering_a_symbol_that_is_not_a_terminal_block_is_refused`</sub>
- 端子番号は前後の空白を落として保存されるので、コマンドが検証した値とチャートが読み戻す値が必ず一致する。 <sub>`terminal_numbers_are_stored_without_their_surrounding_spaces`</sub>
- 前後の空白しか違わない2つのラベルは重複として拒否される: 保存されると同じ端子番号になってしまうため。 <sub>`terminal_numbers_that_are_duplicates_once_trimmed_are_refused`</sub>
- カンマを含む端子番号は拒否される: カンマは保存形の区切り記号なので、読み戻すと端子の個数そのものが変わってしまうため。 <sub>`a_terminal_number_containing_a_comma_is_refused`</sub>
- 採番のundoは直前の端子番号を正確に戻す: 直前に直接指定していればそれが戻り、既定値から採番していれば上書き無しの状態へ戻る。 <sub>`undoing_a_renumbering_restores_the_previous_numbers`</sub>
- 明示的な上書きの値がたまたま既定のラベルと同じでも、採番のundoはそのキーを復元する: キーがあった事実は「上書き無し」として消されない (Task 3 ルーリング3a)。 <sub>`undoing_a_renumbering_restores_an_override_that_matches_the_default_value`</sub>
- ピン数と個数が合わない番号列の指定は拒否され、図面は変わらない。 <sub>`a_terminal_number_count_mismatch_is_refused_without_changing_the_drawing`</sub>
- 番号列に重複したラベルがあると、図面を変えずに拒否される。 <sub>`duplicate_terminal_numbers_are_refused`</sub>
- 番号列に空要素があると、図面を変えずに拒否される。 <sub>`an_empty_terminal_number_is_refused`</sub>
- 直接指定のあとに番号を`None`にすると、シンボルの既定番号へ戻る (`terminal_numbers`属性自体が消える)。 <sub>`setting_numbers_to_none_returns_to_the_defaults`</sub>
- シートに無いentity idの採番は、黙って何もせずに断られる。 <sub>`renumbering_an_unknown_entity_is_refused`</sub>
- 採番コマンドは既定の開始番号・増分を持つ素のJSONで表現できるので、AIやCLIからentity idだけでも送れる。 <sub>`the_terminal_renumbering_command_is_plain_json`</sub>
- 4極の単段端子台を2段化すると、symbol_idが書き換わり、terminal_numbers属性は未設定のまま (明示的な上書きが無かったため)。 <sub>`setting_levels_turns_a_single_level_block_into_two_levels`</sub>
- 段数を増やしたあとの解除 (直接levels=1を指定しても) は、単段のsymbol_idそのものに戻る (`_1l`という別名にはならない)。 <sub>`setting_levels_back_to_one_restores_the_plain_single_level_id`</sub>
- 段を増やすと、明示的なterminal_numbers上書きが極→段の順で段記号付きのラベルへ展開される (計画の例と同じ "11,12,…" → "11a,11b,12a,12b,…")。 <sub>`increasing_levels_expands_an_explicit_terminal_numbers_override`</sub>
- 段記号の無い単段のジャンパ指定は、2段化すると段aへ移る (既存の結線指定が黙って消えない)。 <sub>`increasing_levels_moves_a_levelless_jumper_to_level_a`</sub>
- 消える段のピンに配線が繋がったままだと、段を減らす操作は拒否され、図面は一切変わらない。 <sub>`decreasing_levels_is_refused_when_a_wire_is_connected_to_a_vanishing_level`</sub>
- 消える段の端子を指すジャンパが残っていると、段を減らす操作は拒否される。 <sub>`decreasing_levels_is_refused_when_a_jumper_names_a_vanishing_level`</sub>
- 消える段の端子を範囲に含むピングループが残っていると、段を減らす操作は拒否される。 <sub>`decreasing_levels_is_refused_when_a_pin_group_spans_a_vanishing_level`</sub>
- 消える段の端子を指す付属品指定が残っていると、段を減らす操作は拒否される (spec §1.1 付属品の書式。専用コマンドが入る前から検証する)。 <sub>`decreasing_levels_is_refused_when_an_accessory_names_a_vanishing_level`</sub>
- 消える段に何も参照が残っていなければ、段を減らす操作は成功し、symbol_idと端子番号の上書きの両方から消える段のラベルが落ちる。 <sub>`decreasing_levels_succeeds_and_drops_the_vanishing_levels_labels`</sub>
- 段数を変えても段aの端子の用紙上の座標は1mmも動かない (端子台の回転・反転にかかわらず) ので、その端子に描かれていたものは動かさずに済む。 <sub>`a_level_change_keeps_level_a_terminals_at_the_same_sheet_coordinates`</sub>
- 単段の端子台の端子1に引いてあった配線は、2段化したあとも電気的に同じ端子 (名前は`1a`になる) に繋がったままで、新しくできた段の端子へ繋ぎ変わったりしない。 <sub>`a_wire_on_terminal_one_stays_on_level_a_after_adding_a_level`</sub>
- 消える段のピンに載っているのがネットラベルだけ (配線は1本も無い) のときも、ネットリストと同じ座標一致の規則で段を減らす操作は拒否される。 <sub>`decreasing_levels_is_refused_when_a_net_label_sits_on_a_vanishing_pin`</sub>
- 3段から2段へ減らす操作は、段cの端子を指すジャンパが残っている間は拒否され、外せば通る (そのとき落ちるのは段cのラベルだけで、残ったジャンパはそのまま)。 <sub>`decreasing_three_levels_to_two_is_refused_only_while_level_c_is_referenced`</sub>
- 段を増やすと、付属品指定の端子番号もジャンパと同じように書き換わる (端子1に付けた付属品は`1a`へ追従する)。種別と型番は1バイトも変わらない。 <sub>`increasing_levels_rewrites_accessory_terminal_labels`</sub>
- `terminal_numbers`キーがあるのに値が無効 (個数がピン数と合わない) なとき、その値は段数変更でも書き換わらず、undoで1バイトも変わらず戻る (コマンドは自分で戻せないキーを消さない)。 <sub>`a_present_but_invalid_terminal_numbers_value_survives_a_level_change`</sub>
- 段数変更で土台の取り違えにより重複が起きる指定 (一方の極の段aラベルは段aの記号で終わって剥がされ、もう一方の極の段aラベルはもともと記号を持たず剥がされない、その結果同じ土台になる指定) は、何も書き込む前に拒否される (重複したラベルを持つterminal_numbersを黙って保存しない)。 <sub>`a_level_change_that_would_produce_duplicate_labels_is_refused_without_writing_anything`</sub>
- 段数変更のundoは、変更前のsymbol_idと配置座標を**1回のundoでそろって**戻すので、端子は元の座標へ帰る。 <sub>`undoing_a_level_change_restores_the_symbol_id_and_the_position_together`</sub>
- 1〜3の範囲外の段数指定は、図面を変えずに拒否される。 <sub>`setting_levels_outside_one_to_three_is_refused`</sub>
- 端子台以外のシンボルの段数変更は拒否される (RenumberTerminals/SetTerminalNumbersと同じ門)。 <sub>`setting_levels_on_a_symbol_that_is_not_a_terminal_block_is_refused`</sub>
- `_1l`という段数1の別名で綴ったsymbol_idは配置時に拒否される: 単段端子台のsymbol_idの綴りは1通りだけが有効。 <sub>`a_one_level_alias_symbol_id_is_refused_when_placed`</sub>
- 端子台の極数を増やしても (予備の挿入がそうする)、既存ピンの用紙上の座標は動かない — インスタンスが逆向きにずれて打ち消すため — ので、繋がっていた配線は外れない。 <sub>`inserting_a_spare_terminal_keeps_every_existing_pins_coordinates`</sub>
- 指定した端子の後ろに予備を挿入すると、繰り下がった番号列が書き込まれ、動いたラベルを使っていたジャンパ指定も書き換わるので、ジャンパは同じ物理的な端子の組を指したままになる。 <sub>`inserting_a_spare_after_a_terminal_renumbers_labels_and_follows_the_jumper`</sub>
- 0個の指定は、図面を変えずに拒否される。 <sub>`inserting_zero_spares_is_refused_without_touching_the_drawing`</sub>
- 端子台に無い端子番号を挿入位置に指定すると拒否される。 <sub>`inserting_after_an_unknown_terminal_is_refused`</sub>
- 動的シンボルの極数上限を超える挿入は拒否される。 <sub>`inserting_past_the_pole_cap_is_refused`</sub>
- 端子台以外のシンボルへの予備挿入は拒否される (他の端子台系コマンドと同じ門)。 <sub>`inserting_a_spare_on_a_symbol_that_is_not_a_terminal_block_is_refused`</sub>
- 挿入した予備をundoすると、symbol_id・端子番号・位置がまとめて1回で元に戻る。 <sub>`undoing_an_inserted_spare_restores_symbol_id_numbers_and_position_together`</sub>
- 多段端子台への予備挿入は、1極につき段数ぶんの端子が増える (2段なら"9a","9b")。 <sub>`inserting_a_spare_on_a_multilevel_block_adds_one_terminal_per_level`</sub>
- `after`を省略した挿入コマンドのJSON (MCP/Link API/CLIが送れる最小形) は、末尾挿入として通る。 <sub>`the_insert_spare_terminals_command_accepts_after_omitted`</sub>
- 予備を入れた端子台へもう1つ予備を挿入する操作は、末尾でもどの端子の後ろでも通る。挿入位置を物理的なピン順ではなくチャートに出る順で数えるため。 <sub>`inserting_a_second_spare_into_the_same_block_succeeds`</sub>
- 接頭辞を付けて採番した端子台への予備挿入は、どのラベルも接頭辞を保ったまま通り、パニックしない (接頭辞付きのラベルは素の極番号として読めず、以前は番号の計算が桁溢れしていた)。 <sub>`inserting_a_spare_into_a_prefixed_block_keeps_the_prefix`</sub>
- 「予備を空き番号のまま残す」扱いは、予備を空き番号に入れ、後ろの端子の番号 (とそれを指すジャンパ) を一切変えない。 <sub>`inserting_a_spare_without_renumbering_leaves_the_following_numbers_alone`</sub>
- 「空き番号のまま残す」扱いで予備の番号が既に使われている挿入は、図面を変えずに拒否される。 <sub>`inserting_a_spare_without_renumbering_is_refused_when_the_number_is_taken`</sub>
- 予備を挿入しても既存の端子の用紙上の座標は1mmも動かない (端子台の回転・反転にかかわらず) ので、その端子に描かれていたものは動かさずに済む。 <sub>`inserting_a_spare_keeps_pin_coordinates_at_every_rotation_and_mirroring`</sub>
- シートの用紙サイズ・向きを変えても (G15)、既存のエンティティは元の座標のまま動かず、消えもしない。 <sub>`changing_sheet_paper_keeps_entity_coordinates`</sub>
- 用紙サイズの変更をundoすると、サイズと向きの両方が元へ戻る。 <sub>`undoing_a_paper_change_restores_size_and_orientation`</sub>
- 用紙を変えてもシート独自のゾーン分割はそのまま: 独自の分割数を持つシートはその分割を保つ。 <sub>`changing_paper_keeps_the_sheets_own_zone_division`</sub>
- 用紙変更をundoするとシートは元どおりになるので、そこで求めたクロスリファレンスのゾーン住所も変更前と同じゾーンを指す。 <sub>`undoing_a_paper_change_restores_the_zone_addresses`</sub>
- 存在しないシートの用紙変更は断られる。 <sub>`changing_paper_on_an_unknown_sheet_is_refused`</sub>
- `set_project_meta`はプロジェクト名と番号を1コマンドで設定し、その欄を省略すれば全シートの表題欄には触れない。 <sub>`set_project_meta_sets_name_and_number`</sub>
- `set_project_meta`は表題欄の欄(会社名・図番・表題)をプロジェクトの全シートへ1つのundo可能なコマンドで書き込む。 <sub>`set_project_meta_writes_title_block_to_every_sheet`</sub>
- `set_project_meta`のundoは、各シートがそれぞれ異なる元の値を持っていた場合でも、シートごとの元の表題欄を正しく復元する(共通の1値には戻さない)。 <sub>`undoing_set_project_meta_restores_each_sheets_own_title_block`</sub>
- プロジェクト番号のコマンドは素のJSONで表現できるので、AI・CLI・Link APIのどこからでも送れる。 <sub>`set_project_meta_command_is_plain_json`</sub>
- 番号が無かったプロジェクトに初めて番号を付けてundoすると、空の番号ではなく「番号なし」に戻る。 <sub>`undoing_the_first_project_number_restores_the_unset_state`</sub>
- プロジェクト番号にnullを送ると未設定へ戻る(間違って付けた番号を消す手段)。 <sub>`a_null_project_number_clears_it`</sub>
- 消したプロジェクト番号をundoすると、元の番号がそのまま戻る。 <sub>`undoing_a_cleared_project_number_brings_it_back`</sub>
- コマンドからプロジェクト番号の欄を省くと、ほかの欄を変えても既にある番号はそのまま残る。 <sub>`omitting_the_project_number_leaves_it_untouched`</sub>
- FreeCAD対応付けを登録するとentity idをキーに保存され、再登録で置き換わり、解除で消える。各操作はundoで直前の一覧に戻る。 <sub>`mech_links_are_upserted_removed_and_undone`</sub>
- 計測した電線長の書き戻しは、各配線の長さを設定して出所を「FreeCAD計測」にする1回の編集になる。何も変わらない書き戻しはundoの段を作らない。 <sub>`wire_lengths_write_back_with_their_source`</sub>
- シートに無い配線が混ざった書き戻しは何も変えない。同じ一括分の他の配線も元のままで、undoの段もできない。 <sub>`wire_lengths_write_back_is_all_or_nothing`</sub>

### DXF連携 (AutoCAD Electrical / EPLAN)

- 書き出したDXFはmm単位のAutoCAD 2000形式(AC1015)で、HEADER/TABLES/BLOCKS/ENTITIES/OBJECTSの各セクションと重複しないハンドルを持つ。 <sub>`export_writes_a_well_formed_ac1015_file`</sub>
- 配線はセグメントごとに1本のLINEとしてWIRESレイヤに書かれ、Y座標はDXFの上向き座標へ反転される。 <sub>`wires_become_lines_on_the_wires_layer_with_y_flipped`</sub>
- シンボルはMDK_<symbol_id>ブロック(1回だけ定義)のINSERTになり、ミラー(X倍率-1)・TAG1/CAT/DESC1/RATING1/TERMnnの属性を持つ。 <sub>`symbols_become_block_inserts_with_attributes`</sub>
- 線番はWIRENOレイヤ、ネットラベルはLABELS、注記はMISCに書かれ、ハーネス境界はHARNESSレイヤの閉じた破線多角形と名前になる。 <sub>`texts_and_harness_go_to_their_layers`</sub>
- 非ASCII文字は\U+XXXXで書かれ読み込み時に戻る。MTEXTの段落区切り(\P)や書式コードも読み込みで処理する。 <sub>`unicode_is_escaped_and_decoded`</sub>
- シートをDXFへ書き出して読み戻すと、用紙・シンボル(種類・位置・回転・ミラー・参照記号・型番・属性)・配線・ジャンクション・線番・ラベル・注記・ハーネス名が保たれる。 <sub>`export_then_import_round_trips_the_sheet`</sub>
- AutoCAD Electrical流のDXFを読むと、配線レイヤ(WIRES・_MULTI_WIRE_*)の線分が配線になり、WIRENOの文字は最寄りの配線の線番になり、他の線分と未知のブロックはスキップ項目として報告され、文字はデコードされる。 <sub>`imports_acade_style_wires_and_reports_unknown_blocks`</sub>
- 読み込みオプションで配線レイヤを明示すると、そのレイヤだけが配線になる。 <sub>`explicit_wire_layers_limit_what_becomes_a_wire`</sub>
- 指定した配線レイヤに線分が1本も無いときは配線を1本も読まず、警告がそのDXFに実際にあるレイヤ名を挙げる(明示した絞り込みが「全ての線分を配線」に反転しない)。 <sub>`explicit_wire_layers_that_match_nothing_import_no_wire`</sub>
- 小さい円がジャンクションになるのは配線レイヤの上だけ。全線分を配線として読む救済の下では、他社図面のピン円はスキップされ、ネットが勝手に繋がらない。 <sub>`small_circles_become_junctions_only_on_wire_layers`</sub>
- 配線レイヤの無いDXFは全ての線分を配線として読み、その旨を警告する。近くに配線の無い線番の文字は注記になる。 <sub>`without_wire_layers_all_lines_become_wires`</sub>
- ヘッダの図面限界 ($LIMMIN/$LIMMAX、無ければ$EXTMIN/$EXTMAX) がある図面は用紙サイズと原点をそこから決めるので、内容が隅にしか無いA3図面もA3のまま同じ座標で読める。 <sub>`header_limits_decide_the_paper_and_origin`</sub>
- 揃え指定のあるTEXT(群72か73が0以外)は、読み捨てられる群10/20ではなく群11/21の揃え基準点に置かれる。 <sub>`justified_text_is_placed_at_its_alignment_point`</sub>
- インチ単位のDXFはmmへ換算され、用紙サイズは図面の外形が収まる最小のA判になる。 <sub>`inch_files_are_scaled_and_paper_fits_extents`</sub>
- 90度の倍数でない角度で置かれたブロックは0度で配置され、警告が出る。 <sub>`non_right_angle_rotation_is_rounded_with_a_warning`</sub>
- ENTITIESセクションの無いファイルはDXFではないとして拒否され、数値でないグループコードは行番号付きの構文エラーになる。 <sub>`rejects_non_dxf_and_reports_syntax_errors`</sub>
- シンボルブロックの図形はY反転したローカル座標で書かれ、弧は用紙座標の時計回りがDXFでは反時計回りになる分だけ角度を入れ替えて向きを保つ。 <sub>`block_graphics_are_y_flipped_and_arcs_keep_direction`</sub>
- 図枠はFRAMEレイヤに出る: 外枠・ゾーン目盛の英字と数字・バインドを解決した表題欄のセル(図番・品名・改訂)。 <sub>`the_frame_and_title_block_go_to_the_frame_layer`</sub>
- 図形・電線品番凡例・盤レイアウトのエンティティもそれぞれのレイヤ(SHAPES / ANNOTATION / PANEL)へ書き出され、書き出し結果の内訳が種別ごとの件数を返す。 <sub>`shapes_legends_and_panel_layout_are_exported_on_their_own_layers`</sub>
- シンボル定義が見つからないシンボルは、黙って消えるのではなく書き出し結果の内訳で報告される。 <sub>`an_unresolvable_symbol_is_reported_as_skipped`</sub>
- 回転・ミラーしたシンボルのピンはモデルどおりの用紙座標に着地するので、書き出したDXFでもピンがそのピンへ引いた配線の端と一致し、読み戻すと同じ回転角・ミラーになる。 <sub>`rotated_symbol_pins_meet_their_wires_in_the_exported_dxf`</sub>

### 図枠テンプレート (プロットフレーム)

- 同梱の図枠テンプレートは「JIS A3 横」「JIS A4 縦」の2種がこの順で並び、英語名と日本語名を持ち、どちらも定義として正しい。 <sub>`the_two_bundled_frame_templates_are_listed_in_both_languages`</sub>
- リソースのディレクトリが無い環境でも、ビルドへ埋め込んだ同梱図枠が使える。 <sub>`frame_templates_are_available_without_the_resource_directory`</sub>
- jis-a3テンプレートは現行のJIS図枠をそのまま表す: マージン10mm、ゾーンは4×6で列が数字・行が英字、表題欄は右下基準の120×32mm。 <sub>`the_jis_a3_template_describes_the_current_jis_frame`</sub>
- jis-a4-portraitテンプレートは210×297の図枠で、表題欄のセル構成はjis-a3と同じまま幅だけ110mmへ詰めてあり、改訂欄の列幅の合計も表題欄の幅と一致する。 <sub>`the_a4_portrait_template_narrows_the_same_title_block`</sub>
- 説明書きのバインドはすべて値になる: プロジェクト名・シート番号/総数・シート名・最新改訂・表題欄の全欄・{date}/{scale}/{designer}/{approver}の別名。 <sub>`every_documented_bind_resolves_to_its_value`</sub>
- {rev.latest}は保存されたRev欄ではなく、最新の改訂行の記号に追従する。 <sub>`the_latest_revision_bind_follows_the_revision_table`</sub>
- 知らないバインドはエラーではなく空文字になる(テンプレートに書き間違いがあっても図面は出せる)。 <sub>`an_unknown_bind_becomes_an_empty_string`</sub>
- バインドの無い文字はそのまま出て、閉じ括弧の無い「{」は行の残りを飲み込まずそのまま残る。 <sub>`plain_text_and_unclosed_braces_are_left_alone`</sub>
- シート単体では「1枚中1枚目・プロジェクト名は空」として解決され、プロジェクトの中では実際の位置が入る。 <sub>`a_lone_sheet_is_bound_as_sheet_one_of_one`</sub>
- プロジェクトの外 (シート単体) ではプロジェクト番号のバインドは名前として知られているが、読み取り元のプロジェクトが無いので空になる。 <sub>`the_project_number_bind_is_empty_outside_a_project`</sub>
- プロジェクトの中ではプロジェクト番号のバインドが`Project::no` (G16) を読む。`set_project_meta`コマンドが書き込む欄と同じ。 <sub>`the_project_number_bind_reads_the_project_number`</sub>
- 固定文字のセルは中括弧も含めてそのまま出し、バインドのセルだけが値へ置き換わる。 <sub>`a_fixed_text_cell_is_printed_verbatim`</sub>
- セルに text と bind を両方書いた(またはどちらも書かなかった)テンプレートは、セル番号を添えて拒否される。 <sub>`a_cell_must_have_either_text_or_bind`</sub>
- 改訂欄の列はJISの4列で、幅の合計が表題欄の幅と一致していなければならない(ずれた表は表題欄と揃わない)。 <sub>`the_revision_columns_must_add_up_to_the_title_block_width`</sub>
- ゾーンが0列・0行、マージンが負、表題欄の高さが0といった定義は拒否される。 <sub>`degenerate_frame_definitions_are_rejected`</sub>
- 同じバインドを2つのセルに書くことも、表題欄の外にセルを置くことも許される(会社名は表題欄の左外に出す)。 <sub>`duplicate_binds_and_cells_outside_the_block_are_allowed`</sub>
- ユーザーが`~/MadakeCAD/templates/frames`へ置いた図枠は同梱の後に並び、同梱と同じidを使えば差し替えられる。 <sub>`user_frame_templates_are_listed_after_the_bundled_ones`</sub>
- JSONとして壊れた図枠ファイルや、読めても定義が不正な図枠ファイルは、パスと理由を添えて報告され、他の図枠はそのまま使える。 <sub>`a_broken_frame_template_file_is_reported_with_its_path`</sub>
- 図枠を指定していないときも、誰も知らないidのときも、同梱のjis-a3へフォールバックするので図面は必ず出せる。 <sub>`an_unknown_frame_template_id_falls_back_to_jis_a3`</sub>
- 図枠テンプレートは用紙に依存しない定義で、シートの用紙にそのまま描かれる(図枠を替えても用紙サイズは変わらない)。 <sub>`a_frame_template_does_not_change_the_paper_size`</sub>
- ゾーンの目盛文字はテンプレートが決める: 数字は1,2,3…、英字はA,B,C…でZの次はAへ戻る。 <sub>`zone_labels_follow_the_configured_style`</sub>
- 図枠エディタはユーザー図枠をユーザーの置き場へ{id}.jsonとして保存し、一覧にはその編集内容がユーザー図枠として並ぶ。 <sub>`the_frame_editor_saves_a_user_frame_that_lists_back`</sub>
- 同梱の図枠と同じidでの保存は拒否して何も書かない(別名で保存させる)。 <sub>`saving_over_a_bundled_frame_id_is_refused`</sub>
- 壊れた図枠(改訂欄の列幅の合計が表題欄の幅と違う)や、ファイル名にできないidは拒否して何も書かない。 <sub>`an_invalid_frame_or_id_is_not_saved`</sub>

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
- PLC割付表が無かった頃の古いプロジェクトファイルもそのまま開ける (割付表は空になり、現行のファイル形式へ更新される)。 <sub>`an_old_project_file_opens_with_an_empty_plc_assignment_table`</sub>
- 図枠テンプレートが無かった頃のプロジェクトファイルは図枠未指定で開き、従来どおり既定のJIS A3図枠で描かれる。 <sub>`an_old_project_file_opens_with_the_default_frame_template`</sub>
- 盤レイアウトが無かった頃のプロジェクトファイルは全シートが回路図シートとして開き、現行のファイル形式へ更新される。 <sub>`an_old_project_file_opens_with_schematic_sheets`</sub>
- 図形が無かった頃のプロジェクトファイルはそのままの姿で開き (シートに図形は無い)、現行のファイル形式へ更新される。 <sub>`an_old_project_file_opens_without_any_shapes`</sub>
- 図形が導入された版のプロジェクトファイルは、図形を保ったまま今でも開く (当時描いた図面の見た目が変わらない)。 <sub>`a_project_file_from_the_version_that_introduced_shapes_keeps_them`</sub>
- 注記の引き出し線が無かった頃のプロジェクトファイルは、全ての注記の引き出し先が未設定として開く (どの注記も引き出し線を描かない)。現行のファイル形式へ更新される。 <sub>`an_old_project_file_opens_with_no_leader_targets`</sub>
- シートに描いた図形は、種類・頂点・見た目を保ったまま保存して読み直せる。 <sub>`a_shape_survives_a_save_and_reload`</sub>
- 盤レイアウトシートと、その上の盤エンティティは保存して読み直しても変わらない。 <sub>`a_panel_sheet_survives_a_save_and_reload`</sub>
- テキスト装飾が無かった頃のプロジェクトファイルは、全ての注記が装飾なし (左揃え・既定色・太字でない) で開き、現行のファイル形式へ更新される。 <sub>`an_old_project_file_opens_with_unstyled_notes`</sub>
- 現行のファイル形式バージョンは17 (1本線の区間を持つケーブルを追加し、種類「run_cable」で保存する版) である。 <sub>`the_current_format_version_is_seventeen`</sub>
- 1本線の区間を持つケーブルは種類「run_cable」で保存され、読み直すと区間と芯の端が戻る。そのファイルに種類「cable」のエンティティは無い。 <sub>`a_run_cable_is_saved_as_run_cable_and_round_trips`</sub>
- 区間が空の「run_cable」(ファイルを手で直したときなど) は普通の定義線ケーブルとして開き、書き戻すと種類「cable」になる。 <sub>`a_run_cable_without_a_run_opens_as_a_plain_cable`</sub>
- 新しいMadakeCADで保存されたファイル (形式の版が大きい) は、一部が欠けたまま開くのではなく、両方の版を示す理由付きで断られる。 <sub>`a_file_from_a_newer_app_is_refused`</sub>
- 定義線・芯線割当・シールド・長さを持つケーブルは、保存して読み直しても変わらない。 <sub>`a_cable_round_trips_through_a_saved_file`</sub>
- PLC図面の生成設定を保存できなかった頃 (format_version 13) のファイルは、保存なしの状態で開く。 <sub>`a_version_13_file_opens_without_saved_plc_settings`</sub>
- 整えバリアントが無かった頃のプロジェクトファイルは、全シートが印のない普通のシートとして開き、現行のファイル形式へ更新される。 <sub>`an_old_project_file_opens_with_no_variant_marks`</sub>
- 電線品番凡例ブロックが無かった頃のプロジェクトファイルは凡例ブロックの無い状態で開き、現行のファイル形式へ更新される。 <sub>`an_old_project_file_opens_without_any_wire_legends`</sub>
- シートに置いた電線品番凡例ブロックは、置き場所を保ったまま保存して読み直せる。 <sub>`a_wire_legend_survives_a_save_and_reload`</sub>
- プロジェクト番号が無かった頃のプロジェクトファイルはプロジェクト番号が未設定として開き、表題欄バインド`{project.no}`はこれまでどおり空欄のままになる。現行のファイル形式へ更新される。 <sub>`an_old_project_file_opens_with_no_project_number`</sub>
- 注記文字高が無かった頃の図面は既定の2.0mmで開くので、注記の見た目はこれまでと変わらない。 <sub>`an_old_project_file_opens_with_the_default_annotation_text_height`</sub>
- シートに設定した注記文字高はファイルへ保存され、読み直しても残っている。 <sub>`the_annotation_text_height_survives_a_save_and_reload`</sub>
- アイテム番号が無かった頃の盤図面はバルーンの無い状態で開く。フットプリントの位置はそのままで、番号はいつでも振り直せる。 <sub>`an_old_panel_file_opens_without_item_numbers`</sub>
- プロジェクトに選んだ図枠テンプレートはファイルへ保存され、読み直しても残っている。 <sub>`the_frame_template_survives_a_save_and_reload`</sub>
- 形式2のファイル(FreeCAD対応付けが無く、配線に長さの出所が無い)は、対応付けが空で全ての配線の長さが手入力扱いとして開ける。 <sub>`a_format_2_file_opens_with_empty_mech_links_and_manual_lengths`</sub>
- 形式3のファイル(FreeCAD対応付けに配置が無い)は配置なしの対応付けとして開け、配置付きの対応付けは保存→読み込みで保たれる。 <sub>`a_format_3_file_opens_with_unplaced_links_and_placements_round_trip`</sub>
- クラウド側のビルドが書いたファイル(独自の形式4。FreeCAD対応付けとFreeCAD計測の電線長を含む)は、それらの内容を保ったまま開き、統合後の現行形式で書き直される。 <sub>`a_cloud_format_4_file_keeps_its_mech_links_and_length_source`</sub>

### KiCadインポート / エクスポート

- KiCadインポートは用紙サイズ・表題欄・配線・ジャンクション・ラベル(電源シンボルはネットラベル化)・テキストを変換する。 <sub>`imports_paper_title_block_and_geometry`</sub>
- 既知のlib_idは本ライブラリへ対応付けられ(Device:R→抵抗、Conn_01x03→connector_3p)、参照記号・値・回転・ミラーが保たれる。未知のシンボルはスキップされレポートに列挙される。 <sub>`maps_symbols_and_reports_skipped`</sub>
- kicad_sch文書でないファイルは明確なエラーで拒否される。 <sub>`rejects_non_schematic`</sub>
- S式パーサはアトム・クォート文字列・数値・入れ子リストを読み、型付きアクセサで取り出せる。 <sub>`parses_atoms_strings_numbers_and_nesting`</sub>
- 文字列内のエスケープ(引用符・改行)と日本語などのマルチバイト文字を正しく解釈する。 <sub>`parses_escaped_strings_and_multibyte`</sub>
- children(name)は指定した先頭シンボルを持つ子リストをすべて列挙する。 <sub>`children_iterates_all_matches`</sub>
- 括弧の不整合や閉じていない文字列は、位置付きの構文エラーとして報告される。 <sub>`syntax_errors_are_reported`</sub>
- KiCad書き出しの内訳は、書いた電気要素の件数と、まったく書かない種別(図形・電線品番凡例・盤レイアウト)を挙げるので、黙って落ちるものが無い。 <sub>`the_kicad_export_report_names_the_kinds_it_cannot_write`</sub>
- シートを.kicad_schへ書き出して読み戻すと、用紙・表題欄・シンボル(種類・参照記号・型番・回転・ミラー)・配線・ジャンクション・ラベル・注記が保たれる。 <sub>`export_then_import_round_trips_the_sheet`</sub>
- 折れ線で描いた配線は、セグメントごとに2点のKiCad wireとして書き出される。 <sub>`polyline_wires_are_split_into_segments`</sub>
- シートで使うシンボルは図形とピンごとにlib_symbolsへ1回だけ埋め込まれ、KiCad側にライブラリが無くても開ける。 <sub>`used_symbols_are_embedded_in_lib_symbols`</sub>
- 線番はそのネット上のKiCadラベルになり、KiCadでも再インポート後もネット名として残る。 <sub>`wire_numbers_become_labels`</sub>
- ハーネス境界は破線の閉じた多角形と、その名前の注記として書き出される。 <sub>`harness_becomes_dashed_polyline_with_name`</sub>
- 注記内の引用符・バックスラッシュ・改行はエスケープされ、書き出したファイルは正しく解釈されて同じ文字列に戻る。 <sub>`strings_are_escaped_and_read_back`</sub>
- MadakeCADが書き出した回路図の"MadakeCAD:<id>"ライブラリidは、端子台・コネクタの動的シンボルも含めそのまま同じシンボルに戻る。 <sub>`madakecad_lib_ids_resolve_directly_on_import`</sub>

### leader

- 対象の真上にある注記から引く引き出し線は、注記の下辺の中点から始まり、対象の上辺のいちばん近い点で終わる。 <sub>`a_leader_above_its_target_runs_from_the_bottom_edge_to_the_top`</sub>
- 対象の真横にある注記から引く引き出し線は、注記の近いほうの縦辺から対象の外接矩形まで水平に伸びる。 <sub>`a_leader_beside_its_target_runs_horizontally`</sub>
- leader_forは注記の移動に追従する: 位置を変えて求め直すとモデルを書き換えずに違うジオメトリが返る。 <sub>`leader_for_recomputes_after_the_note_moves`</sub>
- leader_forはleader_to未設定ならNoneを返す。普通の注記は引き出し線を描かない。 <sub>`leader_for_is_none_without_a_leader_target`</sub>
- シンボルを指す引き出し線は、図面に描かれている外形ちょうどで止まる: 配置基準点は必ず外形の内側に入り、シンボルの中の文字は文字の箱ではなく基準点1点として数える。 <sub>`a_leader_targets_exactly_the_outline_the_drawing_shows`</sub>
- ピングループ括弧の付いたシンボルを指す引き出し線は、シンボル本体ではなく括弧の脇のキャプションの外側で止まる。括弧もキャプションもそのシンボルと一緒に描かれるので、その上に重ねて終わると読みにくいため。 <sub>`a_leader_stops_outside_a_symbols_pin_group_caption`</sub>
- entity_boundsはシンボルの外形をその定義から求めるので、引き出し線はシンボルも対象にできる。 <sub>`entity_bounds_resolves_a_symbol_instance`</sub>

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
- 既存のマクロへ新しいバリアントキーで保存すると、そのバリアントが足され、既定と他のバリアントはそのまま残る。 <sub>`saving_with_a_new_variant_key_adds_a_variant_to_the_existing_macro`</sub>
- 登録済みのバリアントキーで保存すると、そのバリアントだけが置き換わる。キーAは既定の回路を置き換える。 <sub>`saving_over_a_registered_variant_replaces_only_that_variant`</sub>
- バリアントキーはAからZの1文字でなければならず、空・2文字・数字は拒否される。 <sub>`a_variant_key_must_be_one_letter`</sub>
- プレースホルダは新しいバリアントでも参照記号が同じシンボルへ結び付くので、そのバリアントを値セット付きで挿入しても値が入る。 <sub>`placeholders_follow_a_new_variant_by_designator`</sub>
- バリアントキーが無いとき、または既存マクロが無いときは、従来どおり新しいマクロとして保存される。 <sub>`saving_without_a_variant_or_existing_macro_makes_a_new_macro`</sub>
- ケーブルを配線や部品より先に選んで写したハーネスは、ケーブルを最後にして保存され、1回の編集で貼り付けられ、貼ったケーブルの芯は貼った配線と部品を指す。 <sub>`a_cable_selected_first_pastes_after_its_wires`</sub>

### mate

- 互いを嵌合相手に指定し、極数も一致している2つのコネクタは、ERCの指摘を1つも出さない。 <sub>`a_reciprocated_matching_pair_has_no_erc_findings`</sub>
- 極数の違う相手を嵌合相手に指定したコネクタには、極数不一致の警告が出る。 <sub>`a_pole_count_mismatch_is_a_warning`</sub>
- 相手側からは指定されていない嵌合相手を宣言したコネクタには、片側宣言の参考情報が出る。 <sub>`a_one_sided_declaration_is_info`</sub>
- 存在しない参照記号を嵌合相手に指定すると警告になり、自分自身を指定した場合も同様。 <sub>`an_unknown_or_self_reference_is_a_warning`</sub>
- `mates_with`属性を持たないコネクタは、嵌合関連の指摘を一切出さない。 <sub>`a_connector_with_no_declaration_is_silent`</sub>
- 端子台は「connector」カテゴリを共有していても嵌合コネクタとしては扱わない (属性を付けても嵌合の指摘は出ない)。 <sub>`a_terminal_block_is_not_a_mating_connector`</sub>
- 同一シート上で50mm以内にある嵌合対は、図面に描く相手先ペアがちょうど1組できる。 <sub>`a_close_pair_on_one_sheet_produces_one_mate_link`</sub>
- 同一シート上でも50mmを超えて離れている嵌合対は、図面に線を描かない(対は帳票だけが伝える)。 <sub>`a_far_pair_produces_no_mate_link`</sub>
- コネクタインスタンスに宣言された嵌合相手が、電線リスト・From-To帳票の「嵌合相手」列にそのまま読まれる。 <sub>`declared_mate_reads_the_attribute`</sub>

### ドキュメントモデル

- 用紙サイズはISO A列の寸法に従い、縦置きでは幅と高さが入れ替わる。 <sub>`paper_sizes_match_iso_and_orientation_swaps`</sub>
- 新規プロジェクトは「Sheet1」という1枚のシート・現行のファイル形式バージョン・空のPLC割付表で始まり、図枠テンプレートは未指定(=既定のJIS A3図枠)である。 <sub>`new_project_has_one_default_sheet`</sub>
- Entity::translateはエンティティの全座標を動かす: ワイヤは全頂点、シンボル/ラベル/テキストは基準点。 <sub>`translate_moves_all_coordinates`</sub>
- 新しいシートは回路図シートで、盤レイアウトシートはファイルに記録される別のシート種別である。 <sub>`a_new_sheet_is_schematic_and_a_panel_sheet_is_a_separate_kind`</sub>
- 盤レイアウトが無かった頃に保存されたシートは種別を持たないが、通常の回路図シートとして開く。 <sub>`a_sheet_without_a_kind_opens_as_a_schematic_sheet`</sub>
- シート種別は snake_case ("schematic" / "panel") のJSONで往復する。 <sub>`the_sheet_kind_is_serialized_in_snake_case`</sub>
- 盤シートを動かすと盤エンティティも動く: 盤外形・ダクト・フットプリントは左上の角が動き、DINレールは左右にも上下にも動く。 <sub>`translate_moves_panel_entities`</sub>
- 盤エンティティはそれぞれの種別タグでJSONを往復でき、どのクライアントからも送れる。 <sub>`panel_entities_have_their_own_json_kind_tags`</sub>
- テキスト装飾が無かった頃に書かれた注記は、従来どおりの見た目 (左揃え・黒・太字でない) で開く。 <sub>`an_old_note_opens_with_the_previous_look`</sub>
- テキストの基準位置は snake_case ("start" / "middle" / "end") のJSONで往復する。 <sub>`the_text_anchor_is_serialized_in_snake_case`</sub>
- 装飾を付けた注記は、基準位置・色トークン・太字の指定をJSONの往復でも保つ。 <sub>`a_styled_note_survives_a_json_round_trip`</sub>
- 図形を動かすと全ての頂点が動くので、矩形は大きさを保ち、折れ線は折れ方を保つ。 <sub>`translate_moves_every_point_of_a_shape`</sub>
- 図形は kind="shape" のJSONで往復し、図形の種類は snake_case で入るので、どのクライアントからも送れる。 <sub>`a_shape_is_serialized_under_the_shape_kind_tag`</sub>
- 4種類の図形はいずれも snake_case でJSONを往復する。 <sub>`the_shape_kinds_are_serialized_in_snake_case`</sub>
- 見た目の欄を省いて送られた図形は、既定色・細線・塗りなし・実線という素の形で開く。 <sub>`a_shape_without_look_fields_opens_plain`</sub>
- Entity::id()は種別によらず内側エンティティのUUIDを返す。 <sub>`entity_id_is_uniform_across_kinds`</sub>
- 区間ケーブルは芯が着く配線と部品をシート内参照として挙げ、写し替え (シート複製) で芯の端がすべて複製先を指す。複製先の無い端は落とす。 <sub>`a_run_cable_remaps_its_core_ends`</sub>
- 区間ケーブルを動かすと、1本線の区間の全頂点が動く。 <sub>`translating_a_run_cable_moves_its_run`</sub>

### net_tag

- タグ「右」のネットラベルは、置いた点から右へ先端が飛び出した五角形の旗として描かれる。 <sub>`tag_right_points_to_the_right`</sub>
- タグ「左」のネットラベルは、先端が左へ飛び出した左右反転した旗として描かれる。 <sub>`tag_left_points_to_the_left`</sub>
- 旗の本体はラベルを置いた点を覆わず、先端と反対側へ伸びる。ラベルが載っている配線が隠れない。 <sub>`the_flag_body_grows_away_from_the_point`</sub>
- ラベルを置いた点は先端の付け根にあたるので、左向きの旗と右向きの旗はちょうど鏡像になる。 <sub>`the_label_position_is_the_root_of_the_point`</sub>
- タグの文字は必ず旗の内側に収まり、旗の幅はネット名の長さに合わせて伸びる。 <sub>`tag_box_follows_the_text_width`</sub>
- 左向きと右向きのタグは同じ大きさになるので、左右のラベルが並んでも高さと幅がそろう。 <sub>`both_directions_have_the_same_size`</sub>
- シート間クロスリファレンスは旗の本体側の外へ逃がされるので、先端や配線の上には出ない。 <sub>`the_cross_reference_escapes_on_the_body_side`</sub>
- 注記文字高を小さくすると、文字・余白・先端がまとめて縮み、旗の形はそのままで小さくなる。 <sub>`a_smaller_annotation_height_shrinks_the_whole_flag`</sub>
- タグ属性の無いネットラベルは、これまでどおり文字だけの見た目のままになる。 <sub>`label_without_tag_attribute_stays_plain_text`</sub>
- 知らないタグの値(打ち間違いや "up")は無視され、失敗せずに文字だけの表示に戻る。 <sub>`unknown_tag_value_is_ignored`</sub>

### ネットリスト抽出

- 両端がシンボルのピンに一致するワイヤは、それらのピンを1つのネットに結合する。 <sub>`wires_connect_symbol_pins_into_one_net`</sub>
- ブロックのピンに届いた配線は、そのピンを番号ではなくピン名(例 "OUT1")でネットに参加させる。 <sub>`block_pins_join_nets_under_their_own_names`</sub>
- 単に交差しただけのワイヤは接続されない。ワイヤに付いたネットラベルはそのネットの名前になる。 <sub>`crossing_without_junction_stays_separate_and_label_names_net`</sub>
- ジャンクションは交差ワイヤを接続し、同名ラベルは離れたネットを1つに統合する。 <sub>`junction_connects_crossing_wires_and_same_labels_merge`</sub>
- シンボルのピンにぴったり置いたネットラベルは、配線を引かなくても同じ名前のネットへそのピンを繋ぐ。 <sub>`a_label_on_a_pin_joins_that_pin_to_the_net_of_the_same_name`</sub>
- ネットリストが使う載り判定は、配線の頂点上・線分の途中・シンボルのピン上のラベルを載っていると認め、わずかでも浮いたラベルは認めない。 <sub>`the_label_attachment_test_accepts_vertices_segments_and_pins`</sub>
- 無名ネットには決定的な連番名(N001, N002, …)が付く。 <sub>`unnamed_nets_get_deterministic_sequential_names`</sub>
- 同一シンボルで同じピン番号を持つ2つの接続点(貫通端子)は内部短絡され、ネットのピン一覧には1回だけ載る。 <sub>`same_pin_number_points_short_internally`</sub>
- 多段端子台は段どうしを決して短絡しない: 同じ極でも段aと段bは別々のネットになる(各段自身の左右接続点は貫通端子として今までどおり内部短絡される)。 <sub>`each_level_of_a_multilevel_terminal_block_is_an_independent_net`</sub>
- ワイヤに書かれた線番は、ネットラベルが無いネットの表示名になる。 <sub>`wire_number_names_a_net_without_a_label`</sub>
- ネットラベルは常に線番より優先され、線番は自動名 (N001) より優先される。 <sub>`net_name_prefers_label_then_wire_number_then_auto_name`</sub>
- ピン座標はシンボルの回転(Y下向き座標系で時計回り)と配置位置を反映する。 <sub>`pin_positions_apply_rotation_and_translation`</sub>
- ミラーは回転より先に、縦軸に対してピンを反転する。 <sub>`pin_positions_apply_mirror_before_rotation`</sub>
- シンボル内の弧はシンボルと一緒に回るので、電磁接触器の半円はどの回転角でも可動接点の側を向き続け、ミラーでは描画方向を保ったまま左右が入れ替わる。 <sub>`arc_angles_follow_symbol_rotation_and_mirror`</sub>
- LTEハーネス (基板側の配線4本が1本線に分岐し、その線がUSBプラグに着く) は別々の4ネットになり、各ネットは芯を通して基板のピン1本とプラグの同じ番号のピンを結ぶ。 <sub>`a_run_cable_keeps_each_core_its_own_net`</sub>
- コネクタの全ピンの上を通る1本線でも、どのピンも繋がない (ケーブルの区間は位置では繋がらない)。 <sub>`a_run_over_a_pin_column_joins_nothing`</sub>
- 2つのコネクタを全長1本線で結ぶケーブル (3.5mm AUX の L・R・GND) は、配線が1本も無くても芯ごとに1ネットになる。 <sub>`a_connector_to_connector_run_gives_one_net_per_core`</sub>
- 芯だけのネットは配線のあるネットの後ろに名前が付くので、そういうケーブルを足しても今あるネットの名前は変わらない。 <sub>`core_only_nets_are_named_after_wire_nets`</sub>
- 1本線が着く部品を動かしても、どの芯も同じピンに繋がったまま (接続は割当てで決まり、描いた位置では決まらない)。 <sub>`moving_the_landing_part_keeps_core_connectivity`</sub>

### ngspiceランナー

- ngspiceの'print all'出力はノード電圧(v(n1)→n1)と枝電流(v1#branch)に解釈され、無関係な行は無視される。 <sub>`parse_print_all_reads_nodes_and_branches`</sub>
- ngspiceの実行ファイルは MADAKE_NGSPICE環境変数→PATH→OS既定パス の優先順で探索され、存在しないenv指定は次へフォールバックする。 <sub>`find_in_prefers_env_then_path_then_candidates`</sub>
- 24V・6+6Ωの分圧回路のDC動作点は中点12V・電源電流2Aになる(ngspice必須。未導入時はスキップ)。 <sub>`run_op_solves_a_divider_when_ngspice_is_installed`</sub>

### panel

- フットプリントは自分の寸法どおりの矩形を占め、90度回すと幅と高さが入れ替わる。 <sub>`a_rotated_footprint_swaps_width_and_height`</sub>
- 盤外形は筐体と、その内側に一回り小さい取付板(部品を留める板)を描く。 <sub>`the_panel_outline_carries_a_mounting_plate_inside_it`</sub>
- インセットより小さい盤外形には取付板が無い(描く余地が残らない)。 <sub>`a_tiny_panel_outline_has_no_mounting_plate`</sub>
- DINレールは取付線を中心とした横帯として描かれるので、取付線に合わせた部品はレールに乗る。 <sub>`a_din_rail_is_a_band_centred_on_its_mounting_line`</sub>
- 右から左へ引いたレールも、左から右へ引いたものと同じ範囲を覆う。 <sub>`a_rail_drawn_backwards_covers_the_same_span`</sub>
- ダクトは45度のハッチ(斜線)で埋まる。斜線はダクトの内側に収まり、一定間隔なので隣り合うダクトでも線がそろう。 <sub>`a_duct_is_filled_with_45_degree_hatch_lines`</sub>
- 幅や高さの無いダクトには描く斜線が無い。 <sub>`a_flat_duct_has_no_hatch_lines`</sub>
- 回路図にあって盤シートにまだフットプリントが無い部品は、型番と外形寸法つきで「未配置」として並ぶ。 <sub>`parts_without_a_footprint_are_listed_as_unplaced`</sub>
- 型番の寸法が部品DBに無い部品も配置できる。既定の矩形の大きさで並び、置いたあとで直せる。 <sub>`an_unknown_type_number_falls_back_to_the_default_footprint_size`</sub>
- 盤シートがまだ1枚も無いプロジェクトでは、回路図の部品は全て未配置になる。 <sub>`without_a_panel_sheet_every_part_is_unplaced`</sub>
- 同じ部品が回路図に何度も出てきても未配置の一覧には1件だけ載る(リレーのコイルと接点は同じ参照記号の1台)。 <sub>`a_device_drawn_several_times_is_listed_once`</sub>
- まだ参照記号が付いていないシンボル(盤へ置くものが決まっていない)は未配置の一覧に出ない。 <sub>`symbols_without_a_reference_are_not_listed`</sub>
- 回路図に無い参照記号のフットプリントは「みなしご」として報告される(参照記号を変えた・部品を消したときに盤へ置き去りにしないため)。 <sub>`a_footprint_with_no_matching_part_is_reported`</sub>
- 参照記号を結び付けていないフットプリント(盤だけの部品)はみなしごとして報告しない。 <sub>`a_footprint_without_a_reference_is_not_reported`</sub>
- アイテム番号は読み順(上の段から先に、段の中は左から右へ)に振られる。 <sub>`item_numbers_follow_reading_order`</sub>
- 同じDINレールに乗った部品は、外形の高さが違っても同じ段として左から右へ数える。 <sub>`parts_on_the_same_rail_form_one_row`</sub>
- 採番は読み順で 1..N を振るので、図面のバルーンは左上から順に読める。 <sub>`renumbering_hands_out_one_to_n`</sub>
- アイテム番号がまだ無いフットプリントにはバルーンが出ない(採番するまで何も描かない)。 <sub>`an_unnumbered_footprint_has_no_balloon`</sub>
- バルーンはフットプリントの左上角のすぐ外の小さな円で、短い引出線で角と結ばれる。 <sub>`the_balloon_sits_outside_the_top_left_corner`</sub>
- 盤に置いた参照記号ごとのアイテム番号が引けるので、部品表にも同じ番号を出せる。 <sub>`item_numbers_can_be_looked_up_by_reference`</sub>
- 凡例表は同じシートの採番済み部品を番号順に「アイテム番号・参照記号・型番」で並べる。 <sub>`the_legend_lists_the_numbered_parts_in_order`</sub>
- まだ採番していない部品も番号欄を空けて末尾に並ぶので、盤の上の部品が黙って抜け落ちない。 <sub>`unnumbered_parts_come_last_in_the_legend`</sub>
- 凡例表は行が増えるぶんだけ下へ伸び、幅は列幅で決まった固定値になる。 <sub>`the_legend_table_grows_with_its_rows`</sub>
- 盤内部品配置表は、フットプリントをアイテム番号・参照記号・型番・位置・レールで並べる。 <sub>`the_placement_table_lists_position_and_rail`</sub>
- レールに乗せずに吊った部品は、配置表のレール欄が空欄になる。 <sub>`a_part_off_the_rail_has_an_empty_rail_column`</sub>
- レールは盤の上から順に番号を振るので、「レール1」は配置表でもプロパティでも同じレールを指す。 <sub>`rails_are_numbered_from_the_top_down`</sub>
- 取付板の上で重なっている2つのフットプリントは、両方を留められないので指摘される。 <sub>`overlapping_footprints_are_reported`</sub>
- 辺が触れているだけのフットプリントは指摘しない(レールに並べた部品どうしは普通に接する)。 <sub>`footprints_that_only_touch_are_not_reported`</sub>
- どのDINレールにも乗っていない部品は「お知らせ」として出るので、レールの間に浮いた部品に気づける。 <sub>`a_part_hanging_off_the_rails_is_noted`</sub>
- DINレールを1本も描いていない盤では吊り配置を報告しない(その盤は取付板へ直付けする設計のため)。 <sub>`a_panel_without_rails_reports_nothing_as_hanging`</sub>
- DINレールごとに、長さのどれだけが部品で埋まっているか(充填率)が出る。 <sub>`each_rail_reports_how_full_it_is`</sub>
- 充填率が80%を超えたレールは注意になる(部品を足す・触る余地が残っていないため)。 <sub>`a_rail_packed_over_eighty_percent_becomes_a_warning`</sub>
- これらの検査は盤シートだけを見るので、回路図シートから盤の指摘は出ない。 <sub>`the_panel_checks_ignore_schematic_sheets`</sub>
- フットプリントに結び付いた銘板はその下に少し隙間を空けて中央揃えで付き、指定すれば上に付く。単独の銘板は置いた位置のまま。 <sub>`a_tied_nameplate_sits_centred_under_its_footprint`</sub>
- 説明に追従する銘板は、1行目が回路図の同じ参照記号の説明になる。追従しなければ自分の文字のまま。 <sub>`a_nameplate_can_follow_the_schematic_description`</sub>
- 2つのフットプリントの間の寸法線は、軸方向の中心間距離を測り、部品を動かすと追従する。 <sub>`a_dimension_measures_between_footprint_centres_and_follows_them`</sub>
- 寸法線を動かすと線の位置が動く (測っている端は部品に付いたまま)。 <sub>`moving_a_dimension_moves_its_line`</sub>
- 型番の付いた銘板は、部品表に型番ごとの枚数で載る。 <sub>`nameplates_are_counted_in_the_bom`</sub>
- 印刷される盤シートには、銘板の文字と寸法の値が描かれる。 <sub>`the_printed_panel_sheet_draws_nameplates_and_dimensions`</sub>
- シートに無いフットプリントに結び付いた銘板や、無い部品を指す寸法線の端は拒否される。 <sub>`nameplates_and_dimensions_must_point_at_footprints_on_the_sheet`</sub>
- 揃えると、選んだ他のフットプリントの端または中心が最初に選んだ部品に揃い、1回のコマンドでundoできる。 <sub>`aligning_lines_footprints_up_with_the_first_one`</sub>
- 上下方向の揃えでは、DINレールに乗っている部品は動かさない (レールから外さない)。 <sub>`vertical_alignment_keeps_rail_mounted_footprints_on_their_rail`</sub>
- 等間隔にすると、両端の部品の間を等分して並べるか、指定の中心間隔で先頭から並べる。 <sub>`distributing_spaces_footprints_evenly_or_at_a_pitch`</sub>
- 2個未満の揃え・ピッチ無しで3個未満の等間隔・0以下のピッチは拒否される。 <sub>`too_few_footprints_or_a_bad_pitch_is_refused`</sub>

### panel_wiring

- 盤内配線図は、盤に置いた部品ごとに、回路図でそのピンに繋がる電線を端子・線番・行き先・線色・線径で並べる。 <sub>`the_panel_wiring_lists_each_pin_wire_with_its_destination`</sub>
- 盤内配線図の図面ページは部品ごとに端子の行を描き、盤に部品が無ければ見出しだけのページになる。 <sub>`the_panel_wiring_sheet_draws_parts_or_an_empty_page`</sub>

### 部品データベース

- 新規DBを開くとスキーマ作成とサンプル投入が一度だけ行われ、開き直しても再投入されない。 <sub>`open_creates_schema_and_seeds_samples_once`</sub>
- 部品は登録・型番キーでの更新・取得ができ、名称の部分一致やカテゴリ完全一致で検索できる。 <sub>`upsert_get_and_search`</sub>
- 電線品番は登録でき、線色+線径の完全一致で引き当てられる。 <sub>`wire_parts_crud_and_lookup`</sub>
- 旧スキーマv1のDBは開いた時点でv2へ移行され、既存データを保持したままspice_model列が使えるようになる。 <sub>`v1_database_migrates_to_v2_preserving_data`</sub>
- 旧スキーマv2のDBは開いた時点でv3へ移行され、既存データを保持したままcontact_config列が使えるようになる。 <sub>`v2_database_migrates_to_v3_preserving_data`</sub>
- 同梱のサンプルリレーは接点構成を持っているので、配置直後から接点数超過の検証ができる。 <sub>`sample_relay_part_has_a_contact_configuration`</sub>
- 旧スキーマv3のDBは開いた時点でv4へ移行され、既存データを保持したままplc_module列が使えるようになる。 <sub>`v3_database_migrates_to_v4_preserving_data`</sub>
- 前のバージョンで作られた部品DBを開くと同梱のPLCモジュールのサンプルが追補されるので、モジュールを手で登録しなくてもPLC I/O図面を作れる。 <sub>`an_older_database_gains_the_bundled_plc_module_samples`</sub>
- 同梱サンプルには三菱・Siemens・Allen-Bradleyのアドレス体系をひと通り含むPLCモジュールが入っている。 <sub>`sample_plc_modules_cover_the_three_address_styles`</sub>
- PLCモジュールの一覧はモジュール定義を持つ部品だけを返すので、普通の部品がモジュールライブラリに紛れ込まない。 <sub>`the_plc_module_list_contains_only_parts_with_a_module_definition`</sub>
- 旧スキーマv5のDBは開いた時点でv6へ移行され、既存データを保持したまま盤レイアウト用のフットプリント寸法の列が使えるようになる。 <sub>`v5_database_migrates_to_v6_preserving_data`</sub>
- 同梱のサンプル部品はフットプリント寸法を持っているので、盤シートへ置くと既定の矩形ではなくその部品の外形で入る。 <sub>`sample_parts_have_footprint_sizes`</sub>
- 前のバージョンで作られた部品DBを開くと、寸法の無い同梱サンプルにフットプリント寸法が追補される(自分で入れた寸法は書き換えない)。 <sub>`an_older_database_gains_the_bundled_footprint_sizes`</sub>
- 幅か高さの片方しか入っていない部品はフットプリント寸法を持たない扱いになる(外形を描くには両方必要なため)。 <sub>`a_half_filled_footprint_size_is_not_used`</sub>
- DBパスの既定はOSのアプリデータフォルダで、MADAKE_PARTS_DBで上書きできる。 <sub>`default_path_respects_env_override`</sub>
- 旧スキーマv6のDBは開いた時点でv7へ移行され、既存データを保持したまま端子台の種別列が使えるようになり、同梱の端子台サンプルに種別が入り、端子台付属品のサンプルが足される。 <sub>`v6_database_migrates_to_v7_with_terminal_kinds_and_accessory_samples`</sub>
- 部品DBから作る部品表の引き当て表には、登録のある型番ごとにメーカ・品名・価格・通貨・購入先が入り、登録の無い型番は入らない。 <sub>`the_bom_catalog_holds_only_registered_part_numbers`</sub>
- 新しく作った部品DBには、種別入りの端子台サンプルと端子台付属品3種のサンプルが最初から入っている。 <sub>`a_new_database_has_terminal_kinds_and_accessory_samples`</sub>

### PDF出力

- PDFの文字はOSごとに日本語グリフを持つ標準書体へ割り当てる: macOS=Hiragino Sans/Menlo、Windows=Yu Gothic UI/Consolas、その他(Linux)=Noto Sans CJK JP/DejaVu Sans Mono。 <sub>`preferred_families_follow_the_operating_system`</sub>
- PDF出力は日本語を含む正しいPDF文書(%PDF-ヘッダ)を非自明なサイズで生成する。 <sub>`sheet_to_pdf_produces_pdf_bytes`</sub>
- PDFのページ寸法は用紙そのもの (A3横=420×297mm) になり、100%で印刷すると原寸になる。 <sub>`pdf_page_is_the_size_of_the_paper`</sub>
- PDF一括出力のページは 表紙 → 回路図の全シート → 選択した帳票 の順に並ぶ。 <sub>`pdf_book_is_cover_then_sheets_then_reports`</sub>
- PDF一括出力には端子接続図も入れられ、指定した順で帳票ページとして並ぶ。 <sub>`pdf_book_can_include_the_terminal_diagram`</sub>
- 帳票を選ばなければPDF一括出力は表紙と回路図シートだけになる。 <sub>`pdf_book_without_reports_is_cover_and_sheets_only`</sub>
- 表紙は外すことができ、その場合は回路図シートが先頭になる。 <sub>`pdf_book_can_omit_the_cover`</sub>
- 一括出力は全ページを1つのPDF文書にまとめ、回路図がA3でも帳票ページはA4になる。 <sub>`pdf_book_merges_every_page_into_one_document`</sub>
- シートが1枚も無いプロジェクトでも、表紙だけの正しい1ページPDFになる。 <sub>`pdf_book_of_an_empty_project_is_just_the_cover`</sub>
- 盤レイアウトシートも一括出力に含まれ、回路図シートに続けてPDFへ入る。 <sub>`the_pdf_book_includes_the_panel_sheet`</sub>
- 盤内部品配置表も他の帳票と同じようにPDF一括出力へ綴じられる。 <sub>`the_pdf_book_can_include_the_panel_placement_table`</sub>

### pin_groups

- 「範囲:名前」1項目と、ダッシュの無い単ピン指定は、それぞれ1つのグループとして解析される。 <sub>`a_range_and_a_single_pin_each_parse_to_one_group`</sub>
- チャート順で終点が始点より前に書かれた範囲は不正として報告され、グループにならない。 <sub>`a_backwards_range_is_reported_as_invalid`</sub>
- そのインスタンスに無いピンを指す範囲は不明ピンの診断コードで報告され、グループにならない。 <sub>`a_range_with_an_unknown_pin_is_reported`</sub>
- コロンが無い・範囲が空といった読めない記述はパニックせず不正として報告され、残りの項目は解析される。 <sub>`unparseable_text_is_reported_without_panicking`</sub>
- チャート順の範囲が重なる2つのグループはどちらも報告されグループにならないが、重ならない3つ目は残る。 <sub>`overlapping_groups_are_both_rejected`</sub>
- 多段端子台のチャート順は段ごとに並ぶ (1a,2a,…,1b,2b,…) ので、"1a-4a"のような範囲は1段の中で連続する。 <sub>`a_multilevel_range_within_one_level_parses`</sub>
- キャプションの折り返しは4文字ごとに順番どおり分割し、横書きの行を積んで縦書きを近似する。 <sub>`wrap_caption_splits_into_chunks_of_four`</sub>
- シンボル左側のピンにかける括弧は、いちばん上から下のピンまで縦棒でつながり、キャプションはさらに左に置かれる。 <sub>`a_bracket_on_left_pins_is_a_vertical_bar_further_left`</sub>
- ピン番号そのものに「-」が入っている端子はピングループに指定できず、「存在しないピン」ではなく使えない理由がそのまま報告される。 <sub>`a_pin_number_containing_a_hyphen_is_refused_with_its_own_reason`</sub>
- 多段端子台で2段目の端子にかけたグループの括弧は、その段の列の外側に出る (列と列の間には入らない)。 <sub>`a_multilevel_group_brackets_the_outside_of_its_own_column`</sub>
- 端子台の端子番号が上書きされていても、表示ラベルで書かれたピングループの範囲はピンを見つけて括弧を描き、「存在しないピン」も報告しない。 <sub>`a_range_written_with_the_displayed_labels_still_draws_its_bracket`</sub>
- 端子番号を上書きした多段端子台でも、1段ぶんの端子を指す範囲は上書きが無いときと同じようにその段の列に括弧を掛ける。 <sub>`a_multilevel_range_still_brackets_its_own_column_after_an_override`</sub>
- 多段端子台の段をまたぐ範囲は、黙って何も描かれるのではなく報告される: 段ごとに外側の辺が逆なので、1本の括弧では囲めないため。 <sub>`a_range_across_two_levels_is_reported_instead_of_silently_skipped`</sub>
- シンボルの2つの辺にまたがるピン (向きが揃わない) への括弧は拒否される: まっすぐな括弧にならないため。 <sub>`a_bracket_across_mixed_pin_directions_is_refused`</sub>

### pin_wiring

- プラグのピンは用紙上の位置で返り、向きは回転と反転に合わせて変わる。 <sub>`pin_direction_follows_rotation_and_mirror`</sub>
- ブロックの名前付きピンは番号・名前・外側の位置で返り、外形は丸めた後の大きさで返る。 <sub>`block_pins_and_outline_are_reported`</sub>
- 参照記号で引くとその参照記号の部品を全部返し、entity id で引くとその部品だけを返す。 <sub>`parts_are_found_by_reference_or_entity_id`</sub>
- ピン指定は最後の「.」で分け、ピン番号で探してから名前で探す。 <sub>`a_pin_is_resolved_by_number_then_by_name`</sub>
- 誤ったピン指定は、直すのに要るもの(書き方・似た参照記号・部品のピン・選ぶentity id)を添えて断られる。 <sub>`a_wrong_specifier_is_refused_with_candidates`</sub>
- 左右向きのピンどうしは Z 字で結ばれ、縦の区間は2本の中央の2.5mmグリッド上に来る。 <sub>`horizontal_pins_get_a_z_route`</sub>
- 上下向きのピンどうしは Z 字で結ばれ、横の区間は中央に来る。 <sub>`vertical_pins_get_a_z_route`</sub>
- 左右向きと上下向きのピンは L 字で結ばれ、各端は自分の向きへ出る。 <sub>`mixed_pins_get_an_l_route`</sub>
- すでに一直線に並んだピンは、向きにかかわらず直線1本で結ばれる。 <sub>`aligned_pins_get_a_straight_wire`</sub>
- グリッドから外れたピンでも端点はピンの位置のままで、曲がる位置だけグリッドにそろう。 <sub>`off_grid_pins_keep_their_ends`</sub>
- 経由点があると順に通り、そろっていない区間には水平を先にした角を1つ足す。 <sub>`via_points_are_passed_in_order`</sub>
- かみ合わせの組は、どちらの向きでもオスがどちら側でも、プラグとソケットを5mm離して向かい合わせに置く。参照記号は両方に、値はソケットに付く。 <sub>`a_mate_pair_faces_each_other_in_every_layout`</sub>
- 経路が別のピンの上を通るとネットリスト上そのピンもつながってしまうので断り、どのピンかを知らせる。 <sub>`a_route_that_passes_over_another_pin_is_refused`</sub>
- 同じピンどうしの配線は、番号で書いても名前で書いても断られる。 <sub>`wiring_a_pin_to_itself_is_refused`</sub>
- 1つの部品に同じピン番号が2つある(貫通端子)ときは、番号だけの指定を両方の候補つきで断り、「#n」で片方を選べる。 <sub>`a_through_terminal_side_is_chosen_with_an_occurrence`</sub>
- かみ合わせの組は2.5mmグリッドにのり、2つは接するだけなので、整頓指標は組を重なりにもグリッド外にも数えない。 <sub>`a_mate_pair_is_tidy`</sub>
- かみ合わせの組のプラグ側は参照記号を印字しないので、共有する記号はソケット側に1回だけ描かれる。 <sub>`the_plug_hides_the_shared_designator`</sub>

### PLC I/O

- 三菱式のアドレスは8進で数えるので、Xモジュールの8点目はX8ではなくX10になる。 <sub>`mitsubishi_addresses_count_up_in_octal`</sub>
- Siemens式のアドレスは「バイト.ビット」で1バイト8点なので、9点目は%I1.0になる。 <sub>`siemens_addresses_are_written_as_byte_and_bit`</sub>
- Allen-Bradley式のアドレスは「ワード/ビット」で1ワード16点なので、17点目はI:1/0になる。 <sub>`allen_bradley_addresses_are_written_as_word_and_bit`</sub>
- アドレスの自動採番は開始点をずらせるので、2枚目のモジュールは1枚目の続きから振れる。 <sub>`auto_addresses_can_start_from_a_point_offset`</sub>
- モジュール定義は置くべき動的シンボルを知っている (入力=plc_di_{n}p、出力=plc_do_{n}p)。 <sub>`a_module_definition_names_its_drawing_symbol`</sub>
- モジュール定義は部品DBのJSON列を往復しても内容が変わらない。 <sub>`a_module_definition_round_trips_through_json`</sub>
- 割付表はアドレス・信号名・コメントの3列でCSVになり、読み直すと同じ行に戻る。 <sub>`the_assignment_table_round_trips_through_csv`</sub>
- 見出し行の無いCSVもデータとして読めるので、見出しを付けずに書き出した表もそのまま取り込める。 <sub>`a_csv_without_a_header_row_is_still_read`</sub>
- CSVの取り込みは対象モジュールの行だけを置き換え、他のモジュールの割付はそのまま残る。undo一発で元の表に戻る。 <sub>`importing_a_csv_replaces_only_the_target_module`</sub>
- 3列に足りない行があるCSVは拒否され、図面の割付表は元のまま変わらない。 <sub>`a_malformed_csv_row_is_refused_without_changing_the_table`</sub>
- 割付表の各点は、図面の結線から接続先の機器と線番を拾ってくる。 <sub>`each_point_picks_up_its_target_and_wire_number_from_the_drawing`</sub>
- 図面に置かれたPLCモジュールは点数と入出力の種別つきで一覧でき、エディタの選択肢にできる。 <sub>`placed_modules_are_listed_with_their_point_count_and_kind`</sub>
- I/O図面の生成はシートを1枚増やし、そのラダーにはモジュールの点数ぶんのラングが並ぶ。 <sub>`generating_an_io_drawing_makes_one_rung_per_point`</sub>
- 生成したラングへ現場機器を結線すると、その機器が割付表の対応する点の接続先になる。 <sub>`a_device_wired_onto_a_generated_rung_becomes_that_points_target`</sub>
- ラングは指定したラング間隔で並び、先頭の指定本数ぶんの位置は空けられる。 <sub>`rungs_follow_the_requested_spacing_and_leading_skip`</sub>
- 生成したページ全体が1回の編集なので、undo一発でシートごと消える。 <sub>`a_generated_io_page_is_undone_in_one_step`</sub>
- 2回生成すると1枚目を上書きせず、2枚目のシートが増える。 <sub>`generating_twice_adds_a_second_sheet`</sub>
- 生成したI/Oページには ERC の指摘が出ない (全ての点が結線され、電線の端はどこかに繋がっている)。 <sub>`a_generated_io_page_passes_the_electrical_rule_check`</sub>
- 割付表が空のモジュールを生成すると、同じ1回の編集の中で自動採番したアドレスが表に入る。 <sub>`generating_fills_an_empty_assignment_table_with_auto_addresses`</sub>
- 生成しても既存の信号名は保たれる (信号名・コメントは割付表が正)。 <sub>`generating_keeps_the_signal_names_already_in_the_table`</sub>
- 横バス形式は上にバスを横に引き、点ごとに縦のラングを下のモジュールへ下ろす (点は左から右へ並ぶ)。ページはERCを通る。 <sub>`the_horizontal_bus_style_drops_vertical_rungs_left_to_right`</sub>
- 「収まる場合のみ同居」では、最後に生成したページの残りのラングに丸ごと入るモジュールは、新しいページではなくそのページへ入る。 <sub>`share_if_fits_puts_a_small_module_on_the_last_page`</sub>
- 「収まる場合のみ同居」で残りのラングに入らないモジュールは、丸ごと新しいページへ行く。 <sub>`share_if_fits_starts_a_new_page_when_the_module_does_not_fit`</sub>
- 「同居し必要なら分割」では、最後のページの残りのラングを分割の単位ごとに埋め、残りは新しいページへ続ける。各片は分割片のシンボルで描かれ続き参照が付き、undo一発ですべて消える。 <sub>`share_or_split_fills_the_last_page_and_continues_on_a_new_one`</sub>
- 「同居し必要なら分割」では、1ページに入らない大きなモジュールは必要な枚数のページに分かれ、どの片も1ページに収まる。 <sub>`share_or_split_spreads_a_big_module_over_several_pages`</sub>
- 分割の単位0や、ページにラングが1つも残らない先頭スキップは断られ、図面は変わらない。 <sub>`a_zero_split_unit_or_an_oversized_start_skip_is_refused`</sub>
- 同居できるのは同じラダー形式・同じラング間隔で生成したページだけで、違えば次のモジュールは新しいページから始まる。 <sub>`a_page_of_another_style_or_spacing_is_not_shared`</sub>
- 2.5mmグリッドの正の倍数でないラング間隔は断られるので、生成したページは必ずグリッドに乗る。 <sub>`an_off_grid_rung_spacing_is_refused`</sub>
- I/Oレポートは各点をアドレス・信号名・接続先・線番・コメントで並べ、プロジェクト全体版では先頭にモジュールの参照記号が付く。 <sub>`the_io_report_lists_address_signal_target_wire_number_and_comment`</sub>

### raster

- シートは用紙寸法×指定した1mmあたりのピクセル数の大きさのPNGになる。 <sub>`a_sheet_becomes_a_png_of_the_paper_size`</sub>
- mm で指定した範囲で切り抜けるので、エージェントはシートの一部を拡大して見られる。 <sub>`a_region_crops_the_image`</sub>
- 上限より大きい画像は収まるように縮め、実際に使った倍率を返す。 <sub>`a_large_image_is_scaled_down_to_the_limit`</sub>
- 用紙の外や面積の無い範囲は断る。 <sub>`an_empty_region_is_refused`</sub>

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
- c接点(切換接点)も、a接点・b接点と同じようにそのリレーの接点として数えられる。 <sub>`a_changeover_contact_belongs_to_its_relay`</sub>
- c接点で組んだリレーは、「接点を動かしていないコイル」として誤って指摘されなくなる。 <sub>`a_relay_with_changeover_contacts_is_not_reported_as_contactless`</sub>
- c接点の端子は「11-12-14」のように、共通・b側・a側の3つを並べて書かれる。 <sub>`changeover_terminals_are_written_common_break_make`</sub>
- コイル下の接点マップには、c接点も3つの端子と所在つきで並ぶ。 <sub>`contact_map_lists_changeover_contacts`</sub>
- 「2CO」のような接点構成は、c接点2極として読み取られる。 <sub>`contact_config_parses_changeover_poles`</sub>
- 「2CO」のリレーに接点を1つだけ描いたときは、残り1極がc接点の未使用行として並ぶ。 <sub>`contact_map_shows_the_remaining_changeover_poles`</sub>
- c接点の極はa接点・b接点としても描けるので、「2CO」のリレーはa接点2個の使い方も受け付ける。 <sub>`a_changeover_pole_can_be_used_as_a_make_or_break_contact`</sub>
- 部品の極数を超えて使えばやはりエラーになり、メッセージにc接点と示される。 <sub>`using_more_changeover_contacts_than_the_part_has_is_an_error`</sub>
- c接点の図記号が使っている極は、a接点が借りることはできない。 <sub>`a_make_contact_cannot_borrow_a_pole_already_in_use`</sub>
- 接点マップの表はコイルの真下に中央揃えで置かれ、接点1個につき1行ずつ下へ伸びる。 <sub>`contact_map_table_is_centred_under_the_coil`</sub>
- 接点マップはコイルの外形の下にぶら下がり、コイル所在の文字は接点シンボルの右脇に置かれる。 <sub>`annotations_are_anchored_to_the_symbol_outline`</sub>
- 切換接点のコイル所在は、上の配線に付く注記と重ならないよう逃げる (G22の衝突の再現)。 <sub>`coil_location_moves_off_a_colliding_wire_note`</sub>
- 上も塞がっているときは、コイル所在は外形の下へ逃げる。 <sub>`coil_location_falls_back_below_when_above_is_also_occupied`</sub>
- 接点自身に型番が書かれているときは、コイル所在はその文字の上へは逃げず、さらに先の外形の反対側へ回る (G22)。 <sub>`coil_location_does_not_escape_onto_the_instances_own_value_label`</sub>
- 逃がしの判定はそのシートが実際に描く文字高で配線注記を測るので、注記文字を大きくしたシートでもコイル所在はきちんと逃げる (G20とG22)。 <sub>`coil_location_measures_the_wire_note_at_the_sheets_own_text_height`</sub>
- コイルは`attrs["device_block"]`がちょうど`"true"`のときだけ実機ブロック表示になり、それ以外(空文字含む)は従来どおりの分散配置になる。 <sub>`device_block_attribute_selects_the_display_mode`</sub>
- 実機ブロックの接点イメージには、接点マップと同じ並びで端子対と所在が並ぶ。 <sub>`device_block_lists_the_same_contacts_as_the_contact_map`</sub>
- 型番が空のときは、空の型番行ではなく型番行そのものが無い実機ブロックになる。 <sub>`device_block_has_no_part_number_line_when_blank`</sub>
- シートの実機ブロック一覧に入るのは`device_block`が付いたコイルだけで、従来どおりの分散配置のコイルは入らない。 <sub>`sheet_device_blocks_only_includes_flagged_coils`</sub>
- 実機ブロックで表示するコイルには、コイルの下にぶら下げる別表としての接点マップはもう付かない(接点はブロックの中に既に並んでいるので)。 <sub>`device_block_coils_are_excluded_from_the_separate_contact_map`</sub>
- 実機ブロックの外形はコイルのピン間隔を下回らないので、幅が最小のときはA1/A2ピンがそのまま外形の左右端になり、引き出し線は要らない。 <sub>`device_block_layout_is_no_narrower_than_the_pin_span`</sub>
- 実機ブロックの最小幅はリレーコイルのシンボル定義のピン間隔そのものなので、いちばん細いブロックでもコイルの両ピンに届く。 <sub>`device_block_min_width_is_the_coil_symbols_pin_span`</sub>
- 型番があるとヘッダは2行になり、内容で外形がピン間隔より広がっても引き出し線でピンは外形まで届く。 <sub>`device_block_layout_grows_for_the_part_number_and_keeps_the_pins_reachable`</sub>
- 行の仕切り線はコイルの引き出し線が付く高さには来ないので、2本の引き出し線がブロックを横切る1本の線で繋がって見えることはない。 <sub>`device_block_rows_keep_clear_of_the_lead_height`</sub>
- 外形がどれだけ横に広がっても、回していないコイルの引き出し線は水平のまま、配線が来るピンの高さで外形の左右の辺に届く (G7)。 <sub>`device_block_leads_run_along_the_pin_axis_however_wide_the_block`</sub>
- 90度・270度に回したコイルの引き出し線は真上・真下へ伸びて外形の上端・下端で止まり、接点が何行あっても接点イメージの文字を横切らない (G7)。 <sub>`a_turned_device_blocks_leads_run_vertically_without_crossing_the_rows`</sub>
- 実機ブロックはどの回転でも正立のまま: 外形の大きさも位置も変わらず、回るのはピンだけなので、引き出し線はピン自身の軸に沿って外形へ届く。 <sub>`device_block_bounds_stay_upright_when_the_instance_turns`</sub>
- 実機ブロックの接点の行はヘッダ行のすぐ下に続き、外形全体の高さは隙間なく全ての行をちょうど覆う。 <sub>`device_block_layout_stacks_contact_rows_under_the_header`</sub>

### 帳票の図面シート化

- 帳票ページはA4横で、JIS図枠・帳票名・プロジェクト名/日付/ページ番号入りの表題欄を持つ。 <sub>`report_page_has_frame_title_and_title_block`</sub>
- 列見出しと全データ行のセルがページに描かれる。 <sub>`report_page_draws_headers_and_all_cells`</sub>
- 行が0件でも列見出しだけのページを1枚出す。 <sub>`report_page_with_no_rows_still_shows_headers`</sub>
- 1ページに収まらない行は次ページへ続き、各ページに列見出しを再掲してページ番号 n/N を振る。 <sub>`report_pages_split_and_repeat_headers`</sub>
- 列幅より長いセルは途中で切られ末尾が省略記号になる(文字が列からはみ出さない)。 <sub>`report_page_truncates_cells_wider_than_the_column`</sub>
- 列幅は指定した相対比に従い、指定が無ければ等分になる。 <sub>`report_page_column_widths_follow_ratios`</sub>
- データ中のXML特殊文字はエスケープされ、ページは正しいSVGのままになる。 <sub>`report_page_escapes_xml_special_characters`</sub>
- 端子台チャートのシートは帳票名に参照記号を含み、チャートのデータと同じ行を端子ごとに並べる。 <sub>`terminal_chart_sheet_matches_chart_rows`</sub>
- 図枠付きの端子台チャートシートは、チャートと同じ行順でセルを描く。`row_order`を変えると図面シート上の行の位置そのものが動く (見出しだけ変わるのではない)。 <sub>`the_terminal_chart_sheet_draws_the_rows_in_the_chart_row_order`</sub>
- 端子台でないエンティティのチャートシートは1ページも出ない。 <sub>`terminal_chart_sheet_is_none_for_other_entities`</sub>
- From-To電線リストのシートはCSV帳票と同じ列・同じ行を使う。 <sub>`wire_list_sheet_matches_report_rows`</sub>
- 部品表のシートは部品グループごとに参照記号と数量を並べる。 <sub>`bom_sheet_lists_parts_with_quantity`</sub>
- 部品カタログを渡して部品表を書き出すと、CSVでもPDFでもカタログ由来の欄が埋まり、CSVの件数には合計行も含まれる。 <sub>`exporting_the_bom_with_a_catalog_fills_the_catalog_columns`</sub>
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
- PLC I/Oレポートは割付済みの点ごとに1行で、CSVでも図枠付きの図面シートでも出せる。 <sub>`the_plc_io_report_has_one_row_per_assigned_point`</sub>
- 帳票の種類はCLI・Link APIと同じケバブケース表記でJSONへ入る。 <sub>`report_kind_json_names_match_cli_spelling`</sub>
- 盤内部品配置表はCSVで出せる。フットプリント1つにつき1行で、アイテム番号・位置・レールが並ぶ。 <sub>`the_panel_placement_table_is_available_as_csv`</sub>
- 盤内部品配置表は図枠付きのA4図面ページとしても出せるので、図面と一緒に綴じられる。 <sub>`the_panel_placement_table_is_available_as_a_drawing_sheet`</sub>
- 盤シートが1枚も無いプロジェクトでも配置表は出る(列見出しだけで本文が空になる)。 <sub>`the_panel_placement_table_of_a_project_without_panels_is_empty`</sub>

### 帳票 (部品表 / 電線リスト)

- 部品表はシンボルを型番でまとめ、数量を集計する。 <sub>`bom_groups_by_value_and_counts`</sub>
- 部品表の先頭列は盤のアイテム番号なので、盤図面のバルーンと部品表が突き合わせられる。 <sub>`bom_starts_with_the_panel_item_number`</sub>
- まだ盤に置いていない部品はアイテム番号の欄が空欄になり、部品表の他の欄はそのまま出る。 <sub>`bom_leaves_the_item_number_empty_for_unplaced_parts`</sub>
- 配線の無いプロジェクトには電線集約行が1行も付かない (部品だけの部品表はそのまま)。 <sub>`bom_has_no_wire_rollup_rows_without_any_wires`</sub>
- 部品行の後ろに、電線品番ごとの集約行が付き、数量欄にプロジェクト全体の使用長合計 (m単位・小数第2位まで) が入る。 <sub>`bom_rolls_up_wires_by_part_number_after_the_component_rows`</sub>
- 品番を引けない配線は「(未割当)」の1行にまとまり、勝手に品番を作ったり黙って消えたりしない。 <sub>`bom_rolls_up_unresolved_wires_under_one_unassigned_row`</sub>
- 部品表の各行は型番を部品カタログで引き当て、メーカ・品名・単価・通貨・金額 (数量×単価)・購入先を表示する。 <sub>`bom_fills_maker_name_price_and_amount_from_the_catalog`</sub>
- 部品カタログに無い型番も部品表の行は残り、カタログ由来の欄だけが空欄になる。 <sub>`a_part_missing_from_the_catalog_keeps_its_row_with_empty_catalog_columns`</sub>
- 電線の単価は1mあたりとして扱い、電線集約行の金額は合計の長さ (m) × 単価になる。 <sub>`a_wire_roll_up_row_is_priced_per_metre`</sub>
- 単価のある行が1つでもあれば、部品表の末尾に通貨ごとの合計行が付き、単価が無くて合計から除いた行の数も書かれる。 <sub>`the_bom_ends_with_a_total_per_currency_noting_rows_without_a_price`</sub>
- 部品カタログで引き当てる型番は、部品表に出る空でない型番を1回ずつで、電線の「(未割当)」行は含まない。 <sub>`the_part_numbers_to_look_up_skip_blanks_and_the_unassigned_wire_row`</sub>
- 部品表CSVは部品表の図面シートと同じ11列で、カタログ由来の欄が埋まる。 <sub>`the_bom_csv_carries_the_catalog_columns`</sub>
- 端子台の付属品は部品表に型番ごとの1行として載り、数量欄に使用個数、参照記号欄に使っている端子台が入り、電線集約行の後ろに来る。 <sub>`bom_counts_terminal_accessories_by_part_number`</sub>
- 型番の無い付属品や、端子台チェックが読めない付属品の項は、部品表に行を作らない。 <sub>`bom_skips_accessories_without_a_part_number_or_that_cannot_be_read`</sub>
- 品番を引けない配線に使用長が1本も入っていないときは、「(未割当) 0.00 m」の行を出さない。 <sub>`bom_adds_no_unassigned_row_when_those_wires_have_no_length`</sub>
- カンマを含む項目は引用符で囲まれ、CSVが壊れない。 <sub>`bom_escapes_fields_with_commas`</sub>
- 電線リストはFrom-To形式で、各行はシート名とワイヤ両端の接続先、続いて嵌合相手・備考列 (G14/G18) が並ぶ。旧来の列順 (シート・線番・ハーネス…) はもう出力されない。 <sub>`wire_list_is_a_from_to_list`</sub>
- シンボルのピンに届いているワイヤの端は「参照記号:ピン番号」(例 K1:A1) と書かれる。 <sub>`a_wire_end_on_a_pin_is_written_as_reference_and_pin_number`</sub>
- ブロックのピンに届いたワイヤの端はピン名つき(例 CTRL1:OUT1)で書かれ、ブロック自体も部品表に数えられる。 <sub>`a_wire_end_on_a_block_pin_uses_the_pin_name_and_the_block_is_in_the_parts_list`</sub>
- ネットラベルが付いているワイヤの端は、そのラベル名で書かれる。 <sub>`a_wire_end_with_a_net_label_is_written_as_the_label_name`</sub>
- ネットラベルに届いたワイヤの端は、そのネットに他のピンがちょうど1つだけあれば、そのデバイスのピンまで辿って解決される (G18)。 <sub>`a_label_end_resolves_to_the_one_to_one_device`</sub>
- 直接の配線が無くても同名のネットラベル2つだけで繋がったネット (2つのラベルを介した鎖) でも正しく解決され、鎖の両側のワイヤがそれぞれ反対側のデバイスへ解決される。 <sub>`the_net_chains_through_two_same_named_labels`</sub>
- ネットラベルのネットに他のピンが複数あるときは、ラベル端は1つのデバイスを選ばずネット名のまま残り、備考列がその行を多端点として印す (G18)。 <sub>`a_label_end_with_multiple_other_pins_keeps_the_label_and_marks_the_remark`</sub>
- どこにもデバイスのピンが無い、ラベル同士だけのネットは両端ともラベル名のまま残り、備考は多端点とは別の「none」(辿る先が無い) になる。 <sub>`a_label_to_label_net_with_no_device_keeps_the_label_name`</sub>
- 嵌合相手を宣言したコネクタに届くワイヤは、電線リストの嵌合相手列にその参照記号を載せる (G14)。 <sub>`wire_list_carries_the_declared_mate`</sub>
- 両端とも嵌合相手を宣言したコネクタに届くワイヤは、どちら向きに描いても嵌合相手列に同じ並びで両方が載る (G14)。 <sub>`the_mate_column_does_not_depend_on_the_direction_the_wire_was_drawn`</sub>
- どちらも嵌合相手を宣言したコネクタではない2つのデバイス間のワイヤは、嵌合相手列が空欄のままになる。 <sub>`wire_list_mate_column_is_empty_without_a_declaration`</sub>
- 何にも接続していないワイヤの端は、リストでは空欄になる。 <sub>`an_unconnected_wire_end_is_empty`</sub>
- 両端のうち表記の辞書順で小さい方がFromになるので、どちら向きに描いたワイヤでもFrom/Toは常に同じになる。 <sub>`from_is_the_alphabetically_smaller_of_the_two_ends`</sub>
- 片側だけ接続しているワイヤでは、接続している方が必ずFrom、空欄の方が必ずToになる。 <sub>`a_connected_end_becomes_from_and_the_unconnected_end_becomes_to`</sub>
- 貫通端子の左右は同じ端子番号なので、同じ端子の内側・外側につながる電線はどちらも「TB1:1」と書かれる。 <sub>`both_sides_of_a_feed_through_terminal_use_the_same_terminal_number`</sub>
- 電線リストには線番採番で入った線番の列があり、そのワイヤに書かれた線番が入る (未採番なら空欄)。 <sub>`wire_list_has_a_wire_number_column`</sub>
- 電線リストにはハーネス列があり、そのワイヤを完全に囲んでいるハーネス境界の名前が入る (どの囲みにも入らない線は空欄)。 <sub>`wire_list_has_a_harness_column`</sub>
- 電線リストには各ワイヤの線色・線径・長さ・品番が載る。 <sub>`wire_list_contains_attributes`</sub>
- 電線リストの品番列はプロジェクトの電線品番表 (線色+線径) から埋まるので、ワイヤ1本ずつ品番を書く必要はない。 <sub>`wire_list_part_number_comes_from_the_wire_part_table`</sub>
- 電線品番表に無い線色+線径の組み合わせは、品番の列が空欄のままになる。 <sub>`wire_list_part_number_is_empty_when_the_table_has_no_match`</sub>
- ワイヤ自身に書かれた品番は、表に別の品番があってもそのまま電線リストに載る。 <sub>`wire_list_keeps_a_part_number_written_on_the_wire`</sub>
- 長さが決まっていないワイヤの長さ列は空欄になる (長さは後で3D配線から書き戻される)。 <sub>`wire_list_length_is_empty_until_it_is_known`</sub>
- 行はシートごとにFrom→Toの順に並ぶので、同じ図面を2回出力すると全く同じファイルになる。 <sub>`rows_are_sorted_by_from_then_to`</sub>
- 1本線のケーブルは芯ごとに端から端まで1行になる: 分岐の配線の先の基板のピン、芯が着くプラグのピン、芯の色、ケーブルの線径・長さ・型番、ハーネス列にケーブル、「ケーブル/芯」の列。分岐の配線だけの行は出ない。 <sub>`a_run_cable_gives_one_wire_list_row_per_core`</sub>
- 分岐の配線の先がネットラベルで、シート上にそのラベルを持つ他の相手が無いとき (続きは別シート)、芯の行はラベル名と備考「none」を出す。 <sub>`a_breakout_wire_ending_on_a_label_shows_the_label`</sub>
- ケーブルの芯になっている配線 (1本線のケーブルの分岐の配線と、定義線が横切った配線) は、ケーブル行が数えているので部品表の電線集約に重ねて足さない。 <sub>`cable_core_wires_are_not_counted_as_loose_wire`</sub>

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
- リレーは、接点構成のうちまだ図面に描いていない接点を未配置機能として並べる。端子の呼び名は接点マップの未使用行と同じで、置くときのシンボルも持つ。 <sub>`a_relay_lists_its_undrawn_contacts_as_unplaced`</sub>
- 接点構成が無い・読めないリレーと、リレー以外のデバイスには未配置機能が無い。 <sub>`devices_without_a_contact_configuration_have_nothing_unplaced`</sub>
- 端子台は全極をまとめた「端子」1行として並び、極数はデバイス側に持つ。 <sub>`terminal_block_device_shows_its_terminal_range`</sub>
- 多段端子台は端子の数ではなく極数で数える: 4極2段の端子台は「4極2段」で、端子の行は最初から最後の端子番号まで (1a-4b) になる。 <sub>`multilevel_terminal_block_device_counts_poles_not_terminals`</sub>
- 機能に分かれない普通の部品は、「本体」1行だけのデバイスとして、シンボルの名前つきで出る。 <sub>`plain_part_is_a_single_body_row`</sub>
- デバイスは参照記号でまとまるので、複数シートに散っていても1デバイスになる。参照記号の無いシンボルはデバイスにならない。 <sub>`devices_group_by_reference_across_sheets`</sub>
- 空のプロジェクトにデバイスは無く、ツリーは参照記号の順に並ぶ。 <sub>`device_tree_is_ordered_and_can_be_empty`</sub>
- 機能の行はジャンプ先のシートとゾーンを持つ。ナビゲータと参照サーフィンはこれを使って図面を表示する。 <sub>`every_function_knows_where_to_jump`</sub>
- フィルタ名はAPI上ではsnake_caseの文字列で、未知の名前は推測せずに拒否する。 <sub>`filter_names_parse_from_the_api`</sub>
- 1本線のケーブル (定義線を持たない) のヒットは、用紙の原点ではなく区間の始点の位置になる。 <sub>`a_run_cable_hit_is_at_its_run`</sub>

### shape

- 新しい図形は既定色・細線・塗りなし・実線で始まるので、プロパティを何も触らずに引いた矩形は図枠の線と同じ見た目になる。 <sub>`a_new_shape_starts_plain`</sub>
- 矩形は対角の2点で表し、どの向きにドラッグしても同じ矩形へ正規化される。 <sub>`a_rectangle_is_two_diagonal_corners`</sub>
- 円は中心の点と円周上の点で表すので、半径はその2点の距離になる。 <sub>`a_circle_is_a_centre_and_a_point_on_its_edge`</sub>
- 折れ線は描いた順に全ての点をそのまま持つ。 <sub>`a_polyline_keeps_every_point`</sub>
- 矩形・線・円は2点ちょうどでなければ拒否され、折れ線は2点以上が必要である。 <sub>`the_number_of_points_must_match_the_shape_kind`</sub>
- 図形の色は名前付きトークンだけで生の色コードは使えないので、紙に出せない色の図形が生まれることはない。 <sub>`a_shape_colour_must_be_a_token`</sub>
- 線幅は、A3に印刷できる最も細い線と、2.5mmグリッドを潰さない最も太い線の間でなければならない。 <sub>`the_stroke_width_has_a_printable_range`</sub>
- 実数でない座標は拒否されるので、壊れた図形が図面を描けなくすることはない。 <sub>`a_non_finite_coordinate_is_rejected`</sub>
- 図形はトークンが表す色で刷られ、塗りを指定していない図形は塗られない。 <sub>`shapes_print_in_their_token_colour`</sub>
- 図形は電気的な意味を持たない: 配線の真上に重ねて描いても、ネットリストも検証結果も変わらない。 <sub>`shapes_are_invisible_to_the_netlist_and_the_checks`</sub>
- 図形は部品表にも電線リストにも出ず、図形の中に入った配線のハーネス列は空のままである (筐体の囲みをハーネスではなく図形で描く理由)。 <sub>`shapes_never_reach_the_parts_list_or_the_wire_list`</sub>
- 図形は整頓メトリクスの対象外なので、注記の枠をグリッドから外して描いても図面の点数は下がらない。 <sub>`shapes_are_left_out_of_the_tidy_metrics`</sub>
- シートの図形を数えると図形だけが返り、配線・注記・ハーネス境界は混ざらない。 <sub>`only_shapes_are_listed`</sub>

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
- ブロックシンボルは、インスタンスの属性から外形・ピン番号・ピン名まで描かれる。 <sub>`svg_draws_block_outline_with_pin_numbers_and_names`</sub>
- jis-a3図枠テンプレートで描いた図枠は、これまで出力してきたJIS図枠とSVGの1文字まで同一である(外枠・ゾーン目盛・改訂欄・表題欄の全セル)。 <sub>`the_jis_a3_template_reproduces_the_previous_frame_exactly`</sub>
- 改訂の無い空のA4シートも従来どおりの図枠を出す(改訂欄は描かれず、表題欄のセルは空欄のまま残る)。 <sub>`an_empty_sheet_reproduces_the_previous_frame_exactly`</sub>
- A4縦テンプレートでは190×277の図枠が描かれ、110mm幅の表題欄が右下に、ゾーン目盛は同じ4×6で出る。 <sub>`the_a4_portrait_template_draws_a_narrower_frame_and_title_block`</sub>
- 図面は図枠テンプレートに従う: プロジェクトをA4縦へ切り替えると全シートの図枠が変わり、undoで元へ戻る。 <sub>`switching_the_project_frame_template_changes_the_output`</sub>
- 誰も知らない図枠テンプレートを指しているプロジェクトも、既定のJIS A3図枠で問題なく出力できる。 <sub>`an_unknown_frame_template_still_prints_with_the_default_frame`</sub>
- 表題欄のセルにはバインドの解決結果が出る: 「何枚中の何枚目」・プロジェクト名・最新改訂がそのまま欄に入る。 <sub>`title_block_cells_print_resolved_binds`</sub>
- ロゴ枠を持つ図枠テンプレートは、ロゴの位置に空の矩形を描く(画像を入れる前でも場所が確保される)。 <sub>`a_logo_frame_is_drawn_as_an_empty_rectangle`</sub>
- 品名などの特殊文字(<, >, &, 引用符)はSVG内でXMLエスケープされる。 <sub>`svg_escapes_xml_special_chars`</sub>
- 改訂が2件あると表題欄の真上に改訂表が描かれ、古い行が下・新しい行が上に積まれ、最下段に列見出し(記号/日付/内容/承認)が出る。 <sub>`svg_draws_revision_table_above_title_block`</sub>
- 改訂が0件のシートには改訂欄をまったく描かない(空の枠だけも描かない)。 <sub>`svg_omits_revision_table_when_no_revisions`</sub>
- 表題欄のRev欄には最新改訂の記号が出る。改訂が無いときは表題欄に保存された値がそのまま出る。 <sub>`svg_title_block_rev_follows_latest_revision`</sub>
- 改訂が7件あると新しい6行だけが描かれ、最も古い行は図面から省かれる(データとしては残る)。 <sub>`svg_revision_table_shows_only_newest_six_rows`</sub>
- 装飾を付けていない注記は、これまでと寸分違わない形で出力される (左揃え・黒・普通の太さ)。 <sub>`an_unstyled_note_is_exported_exactly_as_before`</sub>
- 既定の色トークンを明示的に選んでも、出力される図面は何も変わらない。 <sub>`the_default_colour_token_changes_nothing_in_the_export`</sub>
- 注記の基準位置・色トークン・太字は、そのまま出力図面に反映される。 <sub>`a_styled_note_is_exported_with_its_anchor_colour_and_weight`</sub>
- 引き出し先の無い注記は、図枠・表題欄だけが描く線の本数と変わらない (引き出し線が足されない)。この機能が無かった頃の図面と同じ。 <sub>`a_note_without_a_leader_target_draws_no_leader_line`</sub>
- 対象の真上に置いた注記は、下辺の中点から対象の外接矩形の上辺まで直線を引き、対象側の端には配線の分岐点と同じ丸印を描く。 <sub>`a_note_with_a_leader_target_draws_a_leader_line_to_it`</sub>
- シンボルの機能ラベル (第3のテキスト欄) は、上に並ぶ参照記号・型番とは反対に、シンボル外形の下へ中央揃えで描かれる。 <sub>`the_function_label_is_printed_under_the_symbol`</sub>
- 機能ラベルの無いシンボルは従来どおり、参照記号と型番だけを添えて描かれる。 <sub>`a_symbol_without_a_function_label_is_drawn_as_before`</sub>
- 横向きの配線の線番は、配線の中点の2.5mm上に等幅フォントで描かれる。 <sub>`svg_draws_wire_number_above_a_horizontal_wire`</sub>
- 縦向きの配線の線番は、配線の中点の2.5mm左に描かれる。 <sub>`svg_draws_wire_number_left_of_a_vertical_wire`</sub>
- 1つのネットの線番は、何本のワイヤで描かれていても、最も長い線分の中点に1回だけ描かれる。 <sub>`svg_draws_the_wire_number_once_on_the_longest_segment`</sub>
- 線番の無いネットには線番テキストを一切描かない。 <sub>`svg_omits_wire_number_for_unnumbered_nets`</sub>
- ハーネス境界は、囲んだ配線のまわりに破線の矩形 (IEC 61082-1のグループ囲み) として描かれる。 <sub>`svg_draws_a_harness_as_a_dashed_rectangle`</sub>
- ハーネス名は囲みの左上角のすぐ外側に描かれる。 <sub>`svg_labels_the_harness_at_its_top_left_corner`</sub>
- 同一シート上で50mm以内にある嵌合対は、外形どうしを結ぶ短い破線で結ばれる (G14)。 <sub>`svg_draws_a_dashed_line_between_a_mated_pair_on_one_sheet`</sub>
- 30mm離れた3極コネクタ2つの嵌合線は、双方の外形の向かい合う辺の上、配置基準点の高さを結ぶ (G14)。 <sub>`the_mate_link_runs_between_the_facing_outline_edges`</sub>
- ちょうど50mm離れた嵌合対は描かれる: 距離の上限は50mmちょうどを含む。 <sub>`the_mate_link_distance_limit_includes_exactly_fifty_millimetres`</sub>
- 外形の内側にある点は、その外形のいちばん近い辺の上へ押し出される (嵌合線がシンボルの内側から始まらない)。 <sub>`a_point_inside_the_outline_is_pushed_out_to_the_nearest_edge`</sub>
- 50mmを超えて離れた嵌合対は線を一切描かない (対は帳票が伝える)。 <sub>`svg_draws_no_line_for_a_mated_pair_more_than_50mm_apart`</sub>
- 名前の無いハーネスは破線の囲みだけを描き、ラベルは出さない。 <sub>`svg_omits_the_label_of_an_unnamed_harness`</sub>
- 矩形の図形は、与えた2つの角の位置にそのまま矩形として描かれる。 <sub>`svg_draws_a_rectangle_shape`</sub>
- 線の図形は2点を結ぶ直線として、折れ線の図形は与えた点を順にたどる線として描かれる。 <sub>`svg_draws_line_and_polyline_shapes`</sub>
- 円の図形は中心の位置に、円周上の点までの距離を半径にして描かれる。 <sub>`svg_draws_a_circle_shape`</sub>
- 図形はトークンが表す色・指定した線幅で刷られ、破線に設定すれば破線に、塗りを指定すれば薄く塗られる。 <sub>`svg_honours_the_colour_width_dash_and_fill_of_a_shape`</sub>
- 図形は配線より背面に描かれるので、塗りつぶした強調枠がその上の回路を隠すことはない。 <sub>`svg_draws_shapes_behind_the_wires`</sub>
- 回転したシンボルは形状ごと回転して描かれる(90度で抵抗の本体が縦長になる)。 <sub>`svg_renders_rotated_symbol_primitives`</sub>
- プロジェクトの一部として書き出したシートには、各ネットラベルの脇に他シートの同名ラベルの住所「/シート.ゾーン」が出る。 <sub>`svg_draws_cross_reference_address_next_to_net_label`</sub>
- 他のシートに相手がいないネットラベルには、クロスリファレンスの文字が出ない。 <sub>`svg_omits_cross_reference_when_there_is_no_counterpart`</sub>
- プロジェクトの文脈なしにシート単体を書き出したときは、クロスリファレンスを描かない。 <sub>`svg_of_a_lone_sheet_has_no_cross_reference`</sub>
- コイルのあるシートには、コイルの下に接点マップ(端子対と所在の行)が描かれる。 <sub>`svg_draws_the_contact_map_under_the_coil`</sub>
- 接点のあるシートには、その脇にコイルの住所が丸括弧付きで描かれる。 <sub>`svg_draws_the_coil_location_beside_the_contact`</sub>
- シート単体の書き出しでは相手のシートが分からないので、接点マップは描かれない。 <sub>`svg_of_a_lone_sheet_has_no_contact_map`</sub>
- `hide_reference`属性が`"true"`のシンボルは、参照記号をデータには持ったまま印字しないので、嵌合するプラグ/ソケットの片方が同じ記号を二重に描かない。 <sub>`svg_does_not_print_a_hidden_designator`</sub>
- `device_block`属性が`"true"`のコイルは、小さなコイル図形と別表の接点マップの代わりに、参照記号・型番・接点イメージをまとめた1つの外形として描かれる (G7)。 <sub>`svg_draws_a_device_block_for_a_flagged_coil`</sub>
- 実機ブロックのコイルをどの向きに回しても反転しても、引き出し線は必ず実際のA1/A2ピンの点から出るので、そこに繋いだ配線は繋がったまま (G7)。 <sub>`a_turned_device_block_keeps_its_leads_on_the_coil_pins`</sub>
- 実機ブロックのコイルをどの向きに回しても反転しても、表は正立したまま描かれる: どの行の文字も外形の中に収まり、行どうしが重なることはない (G7)。 <sub>`a_turned_device_block_keeps_its_lettering_upright_and_inside_the_outline`</sub>
- 実機ブロックで表示するコイルでも、機能ラベルは従来の分散配置と同じく外形の下に描かれる (G4)。 <sub>`a_device_block_keeps_the_function_label_under_its_outline`</sub>
- `device_block`を付けなければ従来どおりの分散配置のまま: 小さなコイル図形の下に別表の接点マップが付く。 <sub>`svg_keeps_the_distributed_placement_without_the_attribute`</sub>
- リレー以外の参照記号が2枚のシートに描かれていると、両方のシートに「/2.B3」形式のクロスリファレンスが自動で添えられる (G13)。 <sub>`svg_draws_a_device_cross_reference_for_a_designator_on_two_sheets`</sub>
- 盤レイアウトシートは回路図と同じ図枠つきで書き出され、そこに盤外形と取付板が描かれる。 <sub>`svg_of_a_panel_sheet_draws_the_enclosure_and_its_mounting_plate`</sub>
- ダクトは45度のハッチ(斜線)で埋めた矩形として描かれる。 <sub>`svg_of_a_panel_sheet_hatches_the_duct`</sub>
- DINレールは取付線を中心にした横帯として描かれる。 <sub>`svg_of_a_panel_sheet_draws_the_din_rail`</sub>
- フットプリントは外形の矩形として描かれ、その中に参照記号と型番が入る。 <sub>`svg_of_a_panel_sheet_draws_the_footprint_with_its_reference`</sub>
- 90度回したフットプリントは横倒しに描かれる(幅と高さが入れ替わる)。 <sub>`svg_draws_a_rotated_footprint_lying_down`</sub>
- 採番したフットプリントにはバルーンが付く(アイテム番号を入れた白丸と、部品へ伸びる短い引出線)。 <sub>`svg_draws_a_balloon_on_a_numbered_footprint`</sub>
- まだ採番していないフットプリントには図面にバルーンが出ない。 <sub>`svg_draws_no_balloon_before_numbering`</sub>
- 凡例表は置いた場所に描かれ、そのシートの部品のアイテム番号・参照記号・型番が並ぶ。 <sub>`svg_draws_the_legend_table`</sub>
- 電線品番凡例ブロックは置いた場所に描かれ、そのシートの配線の線色・線径・品番・使用長合計が並ぶ。 <sub>`svg_draws_the_wire_legend_block`</sub>
- 配線の無いシートでも、電線品番凡例ブロックの列見出しは描かれる。 <sub>`svg_draws_the_wire_legend_header_even_with_no_wires`</sub>
- 電線品番凡例ブロックは、電線リストと同じ規則でプロジェクトの電線品番表から品番を引く。 <sub>`svg_wire_legend_resolves_part_number_from_the_project_table`</sub>
- シート単体の書き出しでも、呼び出し元が電線品番表を渡せば凡例の品番欄は埋まる。 <sub>`svg_wire_legend_resolves_part_number_on_the_sheet_only_export`</sub>
- 全ての配線は線径・線色・電線長を配線の下に印字する(現場が電線を作るのに必要な情報)。 <sub>`svg_annotates_each_wire_with_gauge_colour_and_length`</sub>
- 配線注記は図面そのものの一部なので、何も設定しなくてもSVG・PDFに出る。 <sub>`wire_notes_are_exported_by_default`</sub>
- 画面で表示クラスを消しても出力ファイルは変わらない(出力は常に図面の全部を描く)。 <sub>`export_ignores_screen_only_view_classes`</sub>
- 線径も色も長さも無い配線には、注記が何も付かない。 <sub>`svg_omits_the_note_of_a_bare_wire`</sub>
- タグ「右」のネットラベルは右を向いた旗の枠として描かれ、ネット名は置いた点の左へ伸びる本体の中に入る。 <sub>`svg_draws_a_directional_flag_around_a_tagged_net_label`</sub>
- 注記文字高を小さくしたシートは、線番・配線注記・ネットタグだけが小さくなって出力される。 <sub>`svg_follows_the_sheet_annotation_text_height`</sub>
- タグの無いネットラベルは従来どおり文字だけで、周りに枠は描かれない。 <sub>`svg_draws_no_outline_for_an_untagged_net_label`</sub>
- 知らないタグの値は無視され、出力が壊れるのではなく文字だけの表示に戻る。 <sub>`svg_ignores_an_unknown_tag_value`</sub>
- ジャンパバーは、ジャンパの掛け方や端子台の向き・反転にかかわらず、どの端子番号にも重ならない場所に描かれるので、番号はいつでも読める。 <sub>`a_jumper_bar_never_covers_a_terminal_number`</sub>
- 隣り合う2端子間のジャンパは、図面上でその2つの印(丸)の隙間を埋める短い太いバー1本として描かれ、ほかの図形は足されない。 <sub>`svg_draws_a_jumper_bar_between_adjacent_terminals`</sub>
- 多段端子台で段記号付きに書かれたジャンパ ("1a-1b") は、その極の行の上、2つの段の列の印の隙間にバーとして描かれる。 <sub>`svg_draws_a_jumper_bar_across_the_levels_of_a_multilevel_block`</sub>
- ジャンパバーは端子台シンボルの一部なので、シンボルと一緒に出たり隠れたりし、単独で消えることはない: 表示クラスの概念を持たない書き出しでは、端子台の向きにかかわらず常に描かれる。 <sub>`svg_export_always_draws_the_jumper_bar_with_its_symbol`</sub>
- ジャンパバーは端子台と一緒に回るので、90度回した端子台では、バーも回転後の端子の間に横向きに描かれる。 <sub>`svg_draws_the_jumper_bar_turned_with_a_rotated_terminal_block`</sub>
- シンボルの中の文字 (モータの「M」など) はシンボル定義が決めた場所に描かれ、シンボルを回しても反転してもその場所から動かない (文字そのものは常に水平に書くため)。 <sub>`the_lettering_inside_a_symbol_stays_put_when_the_symbol_is_turned`</sub>
- 採番後のジャンパバーは、単に幾何的に近いというだけでなく、実際に繋ぐ端子の印刷ラベルのテキストと紐付けて帰属確認する。最寄りマークのテストでは端子番号の入れ替わりを検出できなかったフェーズ6の積み残しを閉じる。 <sub>`a_jumper_bar_is_attributed_to_its_terminals_by_label_text_after_renumbering`</sub>
- 1〜2文字の端子番号は、端子台をどの向きに回しても反転しても、自分の印(丸)の中に丸ごと収まり、その中央に載る。だから回した端子台でも端子ごとに番号が読める。 <sub>`a_turned_terminal_block_keeps_a_short_number_inside_its_own_mark`</sub>
- 出力図面(SVG/PDF)では、回していない端子台のどの端子番号も、自分の印の中心のちょうど文字高の半分(端子番号の文字高2.0mmに対して1.0mm)だけ下にベースラインが来る。単段でも多段でも同じ(画面のキャンバスはこれより小さい独自の下げ幅を使う)。 <sub>`a_terminal_number_sits_half_a_character_below_its_mark_centre`</sub>
- 3文字の端子番号 (多段端子台の`10a`など) は自分の印と同じ幅なので、印の中央に載せたうえで、印から出るのは最大0.3mmまでに収まる。そのうえでジャンパバーにも隣の端子の印にも掛からない。 <sub>`a_three_character_terminal_number_hangs_at_most_a_little_over_its_own_mark`</sub>
- `attrs["terminal_numbers"]`は各印に描かれるラベルを上書きする: シンボル定義の生のピン番号は一切現れず、上書きラベルはそれぞれちょうど1回だけ現れる。 <sub>`svg_draws_the_overridden_terminal_number_instead_of_the_raw_pin_number`</sub>
- 採番などで生じた3文字の上書きラベル (多段の"11a"や接頭辞付き"P12") も、シンボル既定の3文字番号で確かめた可読性の範囲に収まる: 自分の印の中央に載り、はみ出しは同じ許容量まで、ジャンパバーにも隣の印にも掛からない。 <sub>`an_overridden_three_character_label_hangs_at_most_a_little_over_its_own_mark`</sub>
- ジャンパバーはシンボルの普通の線の2倍以上の太さで描かれるので、端子の印どうしの狭い隙間に入る短いバーでも、印刷したときにショートバーだと分かる。 <sub>`the_jumper_bar_is_at_least_twice_as_thick_as_a_symbol_line`</sub>
- 隣り合わない2端子間のジャンパは(端子台チャートと同じ規則で)不正なので、バーはまったく描かれない。 <sub>`svg_draws_no_bar_for_a_jumper_between_non_adjacent_terminals`</sub>
- 複数のジャンパを持つ端子台には、ジャンパの数だけバーが描かれる。 <sub>`svg_draws_one_bar_per_jumper_when_there_are_several`</sub>
- ジャンパの無い端子台にはバーが描かれず、端子台以外のシンボルは"jumpers"属性を持っていてもバーが描かれない。 <sub>`svg_draws_no_bar_without_jumpers_or_on_a_non_terminal_block`</sub>
- pin_groups の範囲は、角括弧のポリライン1本とそのキャプション文字をシートに描く。 <sub>`svg_draws_a_bracket_and_caption_for_a_pin_group`</sub>
- 端子番号を採番したあとでもピングループの括弧は描かれ続ける: 範囲はその端子台の表示番号と突き合わせられ、採番はその表示番号へ範囲を書き換えているため。 <sub>`svg_still_draws_the_bracket_after_the_terminals_are_renumbered`</sub>
- pin_groups属性が無い・空のシンボルには括弧のポリラインが描かれない。 <sub>`svg_draws_no_bracket_without_pin_groups`</sub>
- 重なるpin_groups項目は (verify.rsで検証済みのとおり) 括弧を一切描かず、同じインスタンスの関係ない有効な項目は変わらず描かれる。 <sub>`svg_draws_nothing_for_overlapping_pin_groups_but_keeps_the_rest`</sub>
- ピングループの括弧とキャプションは、ブロック・端子台・コネクタのいずれでも、全ての回転とミラーで、ピン番号・ピン名・参照記号・型番・機能ラベルのどれにも重ならない。 <sub>`svg_pin_group_boxes_never_land_on_any_label_at_any_rotation`</sub>
- ピングループの部品どうし (括弧と、折り返したキャプションの各行) は決して重ならないので、複数行に割れたキャプションは全ての回転・ミラーで読める。 <sub>`svg_pin_group_pieces_never_overlap_each_other_at_any_rotation`</sub>
- 複数行に割れたキャプションは、シンボルをどう回しても文字は横書きのままなので、用紙の上では必ず縦に積まれる (各行は同じ中心線に並び、順に下へ下がる)。 <sub>`svg_pin_group_caption_lines_stack_downwards_on_paper`</sub>
- ピングループのキャプションは塊ごと括弧の外側に出る: どの行も括弧に触れず、シンボルから見て括弧の棒より遠くにある。回転・ミラーでも変わらない。 <sub>`svg_pin_group_caption_block_sits_wholly_beyond_its_bracket`</sub>
- 実機ブロック表示のコイルは、ピングループの属性を持っていても括弧のための余白を取らない (そこには括弧を描かないため)。外形も、それに従う参照記号・接点マップの位置も、属性が無いときと寸分違わない。 <sub>`a_device_block_coil_reserves_no_room_for_brackets_it_never_draws`</sub>
- 注記の引き出し線の対象端に描く丸印は、配線の分岐点の丸印とぴったり同じ大きさで描かれる (図面上で同じ印に見える)。 <sub>`svg_leader_dot_is_the_size_of_a_wire_junction_dot`</sub>

### シンボルライブラリ

- 同梱シンボルはすべて一意のidを持ち、最低1つのピンを持つ。 <sub>`builtin_symbols_have_unique_ids_and_pins`</sub>
- ピン数可変の端子台(例: 8極)は端子ごとに左右1点ずつの接続点を持ち、すべて2.5mmグリッド上・上下中央揃えになる。 <sub>`dynamic_terminal_block_has_through_pins_on_grid`</sub>
- 多段端子台(8極2段)は段記号付きのピン番号(1a,1b,2a,…8b)を持ち、単段と同じく1端子=1ピン番号・左右2接続点になる。 <sub>`dynamic_multilevel_terminal_block_numbers_pins_with_level_letters`</sub>
- 多段端子台は段ごとに1列の端子が横に並ぶ形で描かれ、3段なら3列が左から等間隔に並ぶ。 <sub>`a_multilevel_terminal_block_draws_one_terminal_column_per_level`</sub>
- 同じ極の端子は段が違っても同じ行に並ぶので、1aと1bは同じ高さになり、行のピッチは単段と同じ5mmのままになる。 <sub>`every_level_of_the_same_pole_sits_on_one_row`</sub>
- 多段端子台の接続点はすべて2.5mmグリッド上にあり、別の端子どうしが同じ点を共有することはないので、どの端子にも別々に配線できる。 <sub>`multilevel_terminal_block_pins_are_on_the_grid_and_never_coincide`</sub>
- 段数が0や4(1〜3の範囲外)の端子台は、極数が範囲外のときと同様に未知のシンボルとして拒否される。 <sub>`multilevel_terminal_block_rejects_level_count_outside_one_to_three`</sub>
- `terminal_block_8p_1l`は`terminal_block_8p`の別名として受け付けず、未知のシンボルとして拒否する: 単段端子台のsymbol_idの綴りは1通りだけにする (Task 3 多段端子の生成・解除)。 <sub>`multilevel_terminal_block_rejects_the_one_level_alias_spelling`</sub>
- 単段の端子台id(`terminal_block_{n}p`)は多段対応の追加後もそのままの数字だけのピン番号(1,2,…)を持ち、既存プロジェクトは有効なまま使える。 <sub>`single_level_terminal_block_id_is_unaffected_by_multilevel_support`</sub>
- 動的生成のconnector_2pは旧静的定義と完全に同じピン座標を持ち、既存図面に影響しない。 <sub>`dynamic_connector_2p_matches_legacy_static_def`</sub>
- resolve_symbolは同梱idを見つけ、不正・範囲外の動的ID(0極・51極・数値なし)は拒否する。 <sub>`resolve_symbol_rejects_invalid_ids_and_finds_builtins`</sub>
- sheet_symbol_defsは同梱ライブラリに加え、シートで実際に使われている動的シンボルの定義を返す。 <sub>`sheet_symbol_defs_includes_dynamic_ids_in_use`</sub>
- PLC入力モジュールのシンボル (例: 16点) は縦長の箱で、左側に点数ぶんの接続点が2.5mmグリッド上に並ぶ。 <sub>`a_plc_input_module_has_one_connection_point_per_io_point`</sub>
- PLCモジュールの分割片 plc_di_{n}p_s{start} は点番号を開始点から振るので、ピンにはモジュール本来の点番号が付く。 <sub>`a_split_plc_module_piece_numbers_its_points_from_the_start`</sub>
- PLCモジュールには入力用と出力用があり、どちらも参照記号の接頭辞はPLCになる。 <sub>`plc_modules_come_in_input_and_output_flavours`</sub>
- PLCモジュールのシンボルは1〜64点まで作れ、その外の点数はシンボルとして存在しない。 <sub>`plc_module_point_counts_are_limited_to_one_through_sixty_four`</sub>
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
- 拡声器はコイルの箱と開いたコーンをつないだ形で、接続点は平らな (コイル側の) 辺に2つある。 <sub>`loudspeaker_is_a_coil_box_joined_to_a_cone`</sub>
- ブザーは接続点の側に平らな辺を向けた半円で、ベルのドーム (底辺が平ら) とは向きが違う。 <sub>`buzzer_is_a_half_circle_facing_its_terminals`</sub>
- サイレンは直角三角形で、接続点は三角形の底辺の両角の下にある。 <sub>`siren_is_a_right_triangle`</sub>
- 送風機は3枚羽根の風車を正方形の外囲で囲んだ形で、接続点は左右にある。 <sub>`fan_is_a_pinwheel_in_a_square_housing`</sub>
- 電圧計・電流計は「V」「A」を書いた円で、2端子の計器として回路に入れる。 <sub>`voltmeter_and_ammeter_are_circles_marked_v_and_a`</sub>
- 変流器は一次導体が鉄心を貫き、二次側にS1・S2の2端子を持つ。 <sub>`current_transformer_has_primary_through_and_secondary_terminals`</sub>
- 差込接続器のプラグとソケットは向かい合う形で、プラグはくさび形、ソケットはそれを受ける半円形になる。 <sub>`connector_plug_and_socket_face_each_other`</sub>
- 多極機器は極ごと(端子1-2・3-4・5-6)に導通するため、相どうしがつながっている扱いにはならない。 <sub>`multipole_devices_conduct_pole_by_pole`</sub>

### ユーザーシンボルライブラリ

- シンボルのファイルが1つも無いフォルダではユーザーシンボルは0件になり、「JSONをここへ置く」置き場のパスだけが返る。 <sub>`an_empty_symbols_folder_lists_nothing_but_its_path`</sub>
- 保存したユーザーシンボルはidの名前のJSONファイル1つになり、図形・接続点・属性スロットがそのまま一覧に戻ってくる。 <sub>`a_saved_user_symbol_round_trips_through_the_folder`</sub>
- ファイルにどの分類が書かれていても、ユーザーシンボルの分類は常に「user」になり、部品挿入ダイアログでは独立した「ユーザー」グループに並ぶ。 <sub>`a_user_symbol_is_always_filed_under_the_user_category`</sub>
- 部品挿入の一覧は「同梱が先・ユーザーが後」の並びになり、ユーザーの記号が同梱の分類の並びに割り込まない。 <sub>`the_insert_list_puts_user_symbols_after_the_bundled_ones`</sub>
- JSONとして壊れているファイルがあっても一覧は止まらず、残りのシンボルは読み込まれ、壊れたファイルはパスと理由つきで報告される。 <sub>`a_broken_json_file_is_reported_with_its_path_and_reason`</sub>
- JSONとしては読めても使えないシンボル(ここでは2.5mmグリッドから外れた接続点)は、一覧に載らず理由つきで報告される。 <sub>`a_symbol_file_that_fails_validation_is_reported_instead_of_listed`</sub>
- ユーザーシンボルは同梱シンボルと同じidを名乗れない。許すと、ファイルを置いただけで、そのシンボルを使っている既存図面の見た目が黙って変わってしまうため。 <sub>`a_user_symbol_may_not_take_the_id_of_a_bundled_symbol`</sub>
- ピン数可変シンボル(例 terminal_block_8p)のidも名乗れない。その名前は必要に応じて自動生成されるものだから。 <sub>`a_user_symbol_may_not_take_a_parametric_symbol_id`</sub>
- 同じidのファイルが2つあるときは先のファイル(ファイル名順)を採用し、後のファイルは理由つきで報告する。どちらが勝つかがフォルダの読み順で変わらないようにするため。 <sub>`two_files_with_the_same_id_keep_the_first_and_report_the_second`</sub>
- 保存は問題を一度に全部返し、ファイルは1つも書かない。直しかけの定義がライブラリに残らないようにするため。 <sub>`saving_an_invalid_symbol_reports_every_problem_and_writes_nothing`</sub>
- 接続点は2.5mmグリッドの上に無ければならない。グリッドに沿って引いた配線がぴったり接続するため。 <sub>`pins_must_sit_on_the_grid`</sub>
- 同じ番号の接続点を離れた2点に置くのは正しい。貫通端子(左右が内部で短絡している端子)はこう描くため。 <sub>`the_same_pin_number_at_two_points_is_a_through_terminal`</sub>
- 同じ番号の接続点が同じ位置に2つあるのは拒否する(2つ目には決して接続できないため)。 <sub>`the_same_pin_number_at_the_same_point_is_rejected`</sub>
- 違う番号の接続点が同じ位置に重なっているのは拒否する(そこへ引いた配線がどちらの端子に繋がったのか決まらないため)。 <sub>`two_different_pin_numbers_at_the_same_point_are_rejected`</sub>
- 接続点には番号が必要(ネットリストや端子台チャートに出るのはこの番号)。名前は空でもよい。 <sub>`a_pin_needs_a_number_but_not_a_name`</sub>
- シンボルには参照記号のスロット(TAG)が必須。無いと、配置した部品にK1・PB2といった記号を表示できないため。 <sub>`a_symbol_without_a_tag_slot_is_rejected`</sub>
- 描画側が知らない属性スロットのキー(打ち間違いなど)は、黙って何も描かれるより先に拒否する。 <sub>`an_unknown_attribute_slot_key_is_rejected`</sub>
- シンボルには図形が最低1つ・接続点が最低1つ必要。無ければ何も見えないし、配線もできないため。 <sub>`a_symbol_needs_at_least_one_shape_and_one_pin`</sub>
- idはそのままファイル名になるので、使えるのは英数字・アンダースコア・ハイフンだけ。 <sub>`an_id_with_unusable_characters_is_rejected`</sub>
- 日本語だけで名前を付けたシンボルも保存でき、英語名には日本語名がコピーされる(英語UIでも名前が出るようにするため)。 <sub>`a_japanese_only_name_fills_both_name_fields`</sub>
- 同梱シンボルを複製すると、空いている新しいid・「コピー」の名前・ユーザー分類を持った下書きが返る(この時点ではファイルには書かない)。 <sub>`duplicating_a_bundled_symbol_gives_a_free_id_and_a_copy_name`</sub>
- 同じシンボルを2回複製してもidは重複しない(2つ目には番号が付く)。 <sub>`duplicating_twice_never_reuses_an_id`</sub>
- ユーザーシンボルも複製できる(これで似た記号の一族を作る)。ピン数可変シンボルからも始められる。 <sub>`a_user_symbol_and_a_parametric_symbol_can_both_be_duplicated`</sub>
- 存在しないシンボルの複製は、空のシンボルを作らずにエラーになる。 <sub>`duplicating_an_unknown_symbol_fails`</sub>
- ユーザーシンボルを削除するとフォルダからファイルが消える。知らないidの削除はエラーになり、何も消えない。 <sub>`deleting_a_user_symbol_removes_its_file`</sub>
- 使用数は、そのシンボルがプロジェクト全体(全シート)で何個置かれているかを返す。削除の前に警告を出すために使う。 <sub>`the_usage_count_counts_placements_across_every_sheet`</sub>

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
- 端子台チャートのCSVは 端子・Lv・内部側・線番・電線・外部側・ジャンパ の列を持ち、端子1個につき1行になる。 <sub>`the_terminal_chart_csv_has_the_designed_columns`</sub>
- 多段端子台はチャートのLv列に段記号が入り、端子列は完全なピン番号 ("1a") になるので、2つの段が同じ端子に見えることはない。 <sub>`a_multilevel_terminal_block_shows_its_level_letter_in_the_lv_column`</sub>
- `attrs["terminal_numbers"]`が既定の番号を上書きしていると、チャートの行・ジャンパ欄・CSVはどれもシンボル既定のピン番号ではなく上書きラベルを表示する。 <sub>`an_overridden_terminal_number_shows_up_in_the_chart_and_csv`</sub>
- 端子番号の指定は前後の空白を落とした値で判定される: ラベル前後の空白は無視され、空白しか違わない2つのラベルは重複と数え、保存形の区切り記号 (カンマ) を含むラベルは拒否される。 <sub>`a_number_list_is_validated_on_its_trimmed_values`</sub>
- ラベル対応表は生のピン番号を表示ラベルへ、表示ラベルを生のピン番号へ相互変換する。上書きの無い端子台では恒等変換になるので、呼び出し側は分岐なしで素通しできる。 <sub>`the_label_map_converts_a_raw_pin_number_both_ways`</sub>
- 端子台ではないシンボルには、手編集のファイルに`terminal_numbers`が残っていてもラベル対応表は作られない。上書きは端子台だけの仕組みであるため。 <sub>`a_symbol_that_is_not_a_terminal_block_has_no_label_map`</sub>
- ジャンパ指定の書き換えは認識できた端子番号だけを置き換え、それ以外 (空白・区切り記号・読めない項目) は元のまま残すので、同じ書き換えを逆向きに走らせれば元の文字列へ戻る。 <sub>`rewriting_a_jumper_spec_only_touches_the_numbers_it_recognises`</sub>
- 端子数と合わない上書き (ラベルが多い・少ない) は無効なので、チャートは行を失ったり異常終了したりせず、シンボル既定の番号へフォールバックする。 <sub>`an_invalid_override_falls_back_to_the_default_numbers`</sub>
- 多段端子台で極だけを増分2以上で変える上書き (例 "11a"と"13a") でも、ジャンパの隣接判定はシンボルの生のピン配置から決めるので、表示ラベルの数値差では判定しない。だから増分2の採番後もジャンパは有効なまま。 <sub>`adjacency_after_an_override_is_based_on_the_raw_pin_layout_not_the_label_arithmetic`</sub>
- 多段端子台のジャンパは段記号まで書く。"1a-1b" は同じ極の2つの段を繋ぐ指定として受け付けられる。 <sub>`a_jumper_can_link_the_two_levels_of_one_pole`</sub>
- 多段端子台でも、同じ段の隣り合う極どうしのジャンパ ("1a-2a") は単段の端子台と同じように受け付けられる。 <sub>`a_jumper_can_link_neighbouring_poles_of_one_level`</sub>
- 多段端子台に段記号なしのジャンパ ("1-2") を書くと、どの段に掛けるのか分からないので受け付けられない。 <sub>`a_jumper_without_a_level_letter_is_refused_on_a_multilevel_block`</sub>
- 多段端子台に無い段を指すジャンパ (2段の端子台への "1a-1c") は、存在しない端子として受け付けられない。 <sub>`a_jumper_to_a_level_the_block_does_not_have_is_refused`</sub>
- 斜めのジャンパ ("1a-2b"、極も段も違う組) は、その2端子が隣り合っていないので受け付けられない。 <sub>`a_diagonal_jumper_across_a_pole_and_a_level_is_refused`</sub>
- 多段端子台のチャートのジャンパ欄には、そのジャンパが実際に繋ぐ2行にだけ表示され、同じ極の別の段には出ない。 <sub>`a_multilevel_jumper_is_shown_only_on_the_rows_it_links`</sub>
- 端子台チェックは、多段端子台に段記号なしで書かれたジャンパをエラーとして報告するので、図面と帳票が食い違うことはない。 <sub>`the_check_reports_a_jumper_without_a_level_letter_as_an_error`</sub>
- 端子台ではないものにチャートを求めても何も返らない。 <sub>`only_terminal_blocks_have_a_chart`</sub>
- 端子台チェックは未結線の端子をすべて情報として報告するので、予備端子は間違い扱いされずに見える。 <sub>`the_check_reports_unwired_terminals_as_information`</sub>
- 端子台チェックは、隣り合わない端子に掛けられたジャンパをエラーとして報告する。 <sub>`the_check_reports_an_invalid_jumper_as_an_error`</sub>
- 端子台チェックは、存在しない端子へのジャンパをエラーとして報告する。 <sub>`the_check_reports_a_jumper_to_a_missing_terminal_as_an_error`</sub>
- 全端子が結線されジャンパも正しい端子台は、チェックで何も指摘されない。 <sub>`a_fully_wired_terminal_block_passes_the_check`</sub>
- 端子台エディタの一覧には、プロジェクトの全端子台がシート順・参照記号順に、所在シート・極数・ジャンパ付きで並ぶ。 <sub>`the_editor_lists_every_terminal_block_with_its_sheet_poles_and_jumpers`</sub>
- 多段端子台の概要は極数・段数を端子数と区別して持つ: 4極2段の端子台は極数4・段数2・端子数8になる。 <sub>`a_multilevel_terminal_block_summary_separates_poles_levels_and_terminals`</sub>
- 単段の端子台は段数1として報告されるので、極数と端子数は同じ数になる。 <sub>`a_single_level_terminal_block_summary_has_one_level`</sub>
- 8極の単段端子台を2段化すると、symbol_idが多段の動的idに変わる。 <sub>`multilevel_id_turns_a_single_level_block_into_two_levels`</sub>
- 多段を解除して1段に戻すと、`_1l`という別名ではなく元の単段idそのものに戻る。 <sub>`multilevel_id_normalises_one_level_to_the_plain_single_level_id`</sub>
- 0段や上限を超える段数は、勝手に丸められず拒否される。 <sub>`multilevel_id_rejects_zero_and_too_many_levels`</sub>
- 端子台以外のsymbol_idはそもそも拒否する (段数は端子台にしか意味が無い)。 <sub>`multilevel_id_rejects_a_symbol_that_is_not_a_terminal_block`</sub>
- 2段から3段へ増やすときも、極数は現在のsymbol_idから読み取ったものを使う。 <sub>`multilevel_id_keeps_the_pole_count_when_changing_between_multilevel_counts`</sub>
- 段を増やすと、極ごとのプレーンなラベルが段数ぶんに展開される。極→段の順で段記号を付け足す ("11,12" → "11a,11b,12a,12b")。 <sub>`relabel_for_levels_expands_plain_labels_into_lettered_levels_pole_major`</sub>
- 段を減らして単段へ戻すと、消える段のラベルは落ち、残る段記号は取り除かれる。 <sub>`relabel_for_levels_drops_vanishing_levels_and_strips_the_letter_at_one_level`</sub>
- 2段から3段に増やすと、既存の段a・段bのラベルはそのまま残り、極ごとに新しい段cのラベルが増える。 <sub>`relabel_for_levels_adds_a_new_level_on_top_of_existing_ones`</sub>
- 段aのラベルの末尾が段aの記号と一致するときだけ土台から取り除く。段aのラベルがたまたま別の文字で終わる上書き(この仕組みが付け足した段記号ではなく、ユーザーが書いた表記の一部)ではその文字を残すので、別の文字で終わる上書きをした2つの極の土台が同じになって潰れることがない(Task 3再レビューの持ち越し: 以前は末尾がアルファベットなら何でも取り除いており、手で書いたラベルの一部を壊したり土台が重複したりし得た)。 <sub>`relabel_for_levels_only_strips_a_trailing_letter_that_matches_level_as_own_letter`</sub>
- 指定した端子の後ろに予備を1個挿入すると、シンボルの極は末尾に足される (既存ピンは動かない) が、返る番号列はそれより後ろの極の番号がすべて1つ繰り下がり、空いた番号が新しい予備に渡る。 <sub>`inserting_a_spare_pushes_the_numbers_after_the_chosen_terminal`</sub>
- 挿入位置を指定しない (末尾挿入) と、番号もそのまま続く: 新しい極は物理的にも番号的にも最後になるので、既存のどの端子の番号も変わらない。 <sub>`inserting_a_spare_at_the_end_does_not_renumber_anything`</sub>
- 純関数の番号列で新しい予備の位置に来る行は、チャートではふつうの予備行として出る: 両側未結線で`spare`が立ち、シンボル既定の端子と見分けが付く特別扱いは何も無い。 <sub>`the_inserted_spares_row_is_unconnected_in_the_chart`</sub>
- 0個の指定は、何もしない変更として素通しせず拒否される。 <sub>`inserting_zero_spares_is_refused`</sub>
- 端子台に無い端子番号を挿入位置に指定すると、「末尾扱い」に読み替えず拒否される。 <sub>`inserting_after_an_unknown_terminal_is_refused`</sub>
- 動的シンボルの極数上限を超える挿入は拒否される。 <sub>`inserting_past_the_pole_cap_is_refused`</sub>
- 多段端子台では、挿入した1つの予備極が段の数だけの端子になる (2段なら新しいラベルは"3a"/"3b"の2つ)。各段の記号はそのまま保たれる。 <sub>`inserting_a_spare_on_a_multilevel_block_adds_one_terminal_per_level`</sub>
- 同じ端子台への2回目・3回目の予備挿入もそのまま通る。挿入位置をチャートに出る順 (番号順) で数えるので、1回目の挿入がわざと食い違わせた物理的なピン順には引きずられない。 <sub>`inserting_a_second_spare_counts_the_position_in_the_charts_order`</sub>
- 1回目の途中挿入のあとに選べる挿入位置はどれも通る (ポップオーバー既定の末尾も含む)。指定した位置は表示番号の中から探すので、1回目の予備が取った番号とぶつからない。 <sub>`every_insertion_point_still_works_after_a_mid_block_insertion`</sub>
- 多段端子台への2回目の挿入も通り、やはり1極につき段数ぶんの端子が増える。番号はチャートの順で最後の極の次になる。 <sub>`inserting_a_second_spare_on_a_multilevel_block_adds_one_terminal_per_level`</sub>
- 予備のラベルは隣の端子の表記の数値部だけを増やして作るので、接頭辞を付けて採番した端子台は接頭辞を保つ (「読めない番号」の番兵で桁溢れすることもない)。 <sub>`inserting_a_spare_keeps_a_prefixed_labels_prefix`</sub>
- 数字を1つも含まないラベルからは番号を決められないので、パニックしたりラベルを黙って壊したりせず、理由を添えて拒否する。 <sub>`inserting_a_spare_next_to_a_label_without_digits_is_refused`</sub>
- 後ろの番号を動かさない扱い (「予備を空き番号のまま残す」) では既存のラベルが1つも変わらず、選んだ端子の次の空き番号が予備に渡る。 <sub>`leaving_the_following_numbers_alone_gives_the_spare_the_vacant_number`</sub>
- 「空き番号のまま残す」扱いで、予備の番号が既にある端子とぶつかる挿入は、重複したラベルを書かずに理由を添えて拒否する。 <sub>`leaving_the_following_numbers_alone_is_refused_when_the_number_is_taken`</sub>
- チャートは物理的な極の位置ではなく、番号が示す位置に予備を表示する: 既定 (数値順) は解析した極番号で並べ直すので、ブロックの途中への挿入もチャートの途中への挿入として読める。 <sub>`the_chart_shows_the_spare_after_its_chosen_terminal_not_at_the_physical_end`</sub>
- 付属品指定の書き換えは各項の先頭の端子番号トークンだけに効き、種別・型番・末尾指定の`end`・知らない端子を指す項は1バイトも変わらない。 <sub>`rewrite_accessory_labels_only_replaces_the_terminal_number_token`</sub>
- 既定値と同じラベルを明示的に書いたterminal_numbersも、キーがあり値が妥当なので上書きとして扱われる (展開結果が既定と同じかどうかだけでは判定しない)。 <sub>`terminal_numbers_override_reports_a_key_that_matches_the_default_value`</sub>
- 個数が合わない無効なterminal_numbersは上書きとして扱わない (表示は既定へフォールバック済みで、復元できる妥当な値が無い)。 <sub>`terminal_numbers_override_is_none_for_an_invalid_value`</sub>
- terminal_numbersキー自体が無ければ上書きは無い。 <sub>`terminal_numbers_override_is_none_when_the_key_is_absent`</sub>
- シートを指定すると、そのシートに描かれている端子台だけの一覧になる。 <sub>`naming_a_sheet_narrows_the_terminal_block_list_to_that_sheet`</sub>
- 端子台ではないシンボルは一覧に出ない。 <sub>`other_symbols_never_show_up_in_the_terminal_block_list`</sub>
- 端子台のチャートとチェックは、どのシートにあるかを知らなくてもentity idだけで引ける。 <sub>`a_terminal_block_can_be_looked_up_by_id_across_sheets`</sub>
- 同じ極の2つの段を繋ぐジャンパバーは、その極の行の上、2つの段の列の印の隙間に引かれる。 <sub>`a_cross_level_jumper_bar_lies_on_the_row_between_the_two_marks`</sub>
- ジャンパバーの両端はちょうど自分の端子の印の縁の上にあり、ほかのどの端子よりも自分の端子に近いので、どの2端子を繋いでいるか読み違えようがない。 <sub>`each_jumper_bar_end_sits_on_the_edge_of_its_own_terminal_mark`</sub>
- ジャンパバーの長さは、繋ぐ2つの端子の印の隙間ちょうど、つまりその端子台の端子の間隔から両端の印の半径を引いた長さになる。 <sub>`a_jumper_bar_is_exactly_as_long_as_the_gap_between_the_two_marks`</sub>
- 同じ列の2端子を繋ぐジャンパは、2つの印の間の隙間だけに描かれるので、端子台の配線接続点の上を通ることはない。 <sub>`a_column_jumper_stays_in_the_gap_between_the_two_marks`</sub>
- 1つの端子台に複数のジャンパがあれば、それぞれ独立したバーの線分になる。 <sub>`multiple_jumpers_each_produce_their_own_bar_segment`</sub>
- 端子台に無い端子番号を指すジャンパは、パニックせず線分を作らない。 <sub>`a_jumper_to_a_missing_terminal_produces_no_bar_segment`</sub>
- ジャンパは通常のupdate_entityコマンドで設定するので、チャートに反映され、undoで元に戻る。 <sub>`setting_jumpers_through_update_entity_is_undoable`</sub>
- `attrs["row_order"]`が未設定・空・未知語なら数値順 (既定) に落ちる。 <sub>`an_unset_or_unknown_row_order_falls_back_to_number_order`</sub>
- 数値順 (既定) は極番号を数として昇順に並べる。10極以上でも桁の大小どおりになる。 <sub>`number_order_sorts_poles_as_integers`</sub>
- 数値順は表示ラベルを解析した極番号で並べる。行が最初に持っていた物理的な(生ピンの)並びでは決まらないので、予備端子挿入 (Task 4) が作るような物理順と食い違う上書きでも数値の昇順で出る。 <sub>`number_order_sorts_by_the_parsed_label_even_when_the_physical_order_disagrees`</sub>
- 英数順は端子番号を文字列として辞書順に並べるので、10極以上では数値順と違う結果になる ("10"が"2"より前に来る)。 <sub>`alnum_order_compares_terminal_numbers_as_text`</sub>
- ページ順は、その極の配線が図面上で読み順(上→下、左→右)に最初に現れる位置で並べる。番号は逆でも、読み順で先に来る方が先になる。配線の無い極(予備)は最後に極番号順で並ぶ。 <sub>`page_order_follows_where_the_wiring_first_appears_on_the_sheet`</sub>
- 外部ケーブル順は、外部ハーネス名が最初に現れた極の順にその名前の極をまとめる (名前の辞書順ではなく初出順)。ハーネス名の無い極は最後に極番号順で並ぶ。 <sub>`cable_order_groups_poles_by_their_external_harness_in_first_seen_order`</sub>
- ジャンパ順は、ジャンパが1本でも掛かっている極を極番号順でまとめて先頭に出し、ジャンパの無い極をその後ろに極番号順で並べる。 <sub>`jumper_order_brings_every_jumpered_pole_to_the_front`</sub>
- 3段の端子台では、どの並べ替え順を選んでも同じ極の3段 (a/b/c) が並びの中で必ず隣り合う (段がバラバラの位置に散らばらない)。 <sub>`every_row_order_keeps_the_three_levels_of_one_pole_adjacent`</sub>
- チャートCSVの行順は、常にチャート(エディタ)の行順と一致する (row_orderで並べ替えても、CSVは`chart.rows`をそのまま使うので食い違わない)。 <sub>`the_csv_row_order_always_matches_the_chart_row_order`</sub>
- 付属品の指定は「端子番号:種別:型番」を並べたもので、`end`は最後の端子の後ろを指し、型番は省略できる。 <sub>`accessories_are_read_in_order_with_end_and_optional_part_number`</sub>
- 種別が3種以外・端子台に無い端子番号・書式が壊れた付属品の項は、その項だけが飛ばされて報告され、ほかの項は生きる。 <sub>`a_bad_accessory_item_is_skipped_alone_and_reported`</sub>
- 端子台チャートでは、付属品は指定した端子の直前に自分の行を持ち、`end`の付属品は最後の端子の後ろに来る。 <sub>`accessory_rows_sit_before_their_terminal_and_end_rows_come_last`</sub>
- 端子台チェックは、読めない付属品の項をWarningとして報告し、どの項かを名指しする。 <sub>`the_terminal_check_warns_about_a_broken_accessory_item`</sub>
- 端子の内外をスワップすると、チャートではその端子の内部側と外部側だけが入れ替わり、ほかの端子はそのまま。 <sub>`swapping_a_terminal_exchanges_only_its_inside_and_outside`</sub>
- 内外スワップの指定が空なら、どの端子も入れ替わらない。 <sub>`an_empty_swap_setting_swaps_nothing`</sub>
- 内外スワップの指定に端子台に無い端子番号があると、その項目だけを無視し、端子台チェックがWarningで知らせる。 <sub>`an_unknown_swapped_terminal_is_ignored_and_warned_about`</sub>
- 3段端子台で内外スワップと並べ替えを同時に指定しても、行は並べ替えどおりに並び、入れ替わるのは指定した端子だけ。 <sub>`swapping_and_sorting_a_three_level_block_work_together`</sub>

### 端子接続図

- 端子ストリップは端子1個につき1つの番号入りの箱として、端子番号の順に上から下へ縦に積まれる。 <sub>`the_strip_stacks_one_numbered_box_per_terminal`</sub>
- 端子接続図はチャート(エディタ)と同じ並び順に従う: `row_order`でジャンパの掛かった極をまとめると、ストリップの箱もその順に積まれ、単純な端子番号順にはならない。 <sub>`the_strip_follows_the_chart_row_order_not_plain_terminal_order`</sub>
- 盤外 (外部側) はストリップの左、盤内 (内部側) は右に描かれ、それぞれ見出しが付く。 <sub>`the_outside_is_on_the_left_and_the_inside_on_the_right`</sub>
- 何も繋がっていない端子は、薄く塗った箱に予備の注記を付けてストリップに残る。 <sub>`a_spare_terminal_stays_in_the_strip_lightly_filled`</sub>
- 同じハーネスに属する電線は引出線の外端で1つのブラケットにまとめられ、ハーネス名が添えられる。 <sub>`wires_of_one_harness_are_gathered_into_a_bracket`</sub>
- どのハーネスにも属さない電線にはブラケットが付かない。 <sub>`a_wire_without_a_harness_gets_no_bracket`</sub>
- 隣り合う端子のサドルジャンパは、端子箱の内部側の縁に縦の連結線として描かれる。 <sub>`a_jumper_is_drawn_on_the_inside_edge_of_the_boxes`</sub>
- 各引出線には、その電線の線色・線径sq・品番が線の下に小さく書かれる。 <sub>`each_lead_line_carries_the_wire_specification`</sub>
- 内外をスワップした端子は、2本の電線が反対側に描かれる。ほかの端子は元の側のまま。 <sub>`a_swapped_terminal_draws_its_wires_on_the_opposite_sides`</sub>
- 多段端子台の接続図は、各箱に段記号付きの完全な端子番号(1a,1b,2a,2b,…)を書くので、2つの段が同じ端子として描かれることはない。 <sub>`a_multilevel_terminal_block_labels_boxes_with_the_level_letter`</sub>
- 多段端子台の接続図は段を列で表す。端子ストリップが段の数だけの列に分かれ(段aが左)、各端子の箱は自分の段の列に置かれる。 <sub>`a_multilevel_terminal_diagram_draws_each_level_as_a_column`</sub>
- 多段端子台の接続図は、段の列それぞれの上に段記号を書くので、箱の番号を読まなくてもどの列がどの段か分かる。 <sub>`a_multilevel_terminal_diagram_heads_each_level_column_with_its_letter`</sub>
- 同じ段の2極を繋ぐジャンパ("1a-2a")は、間に挟まる別の段の行("1b")を飛び越えるので、どの行の引出線よりも外側のレーンを通る — 間に挟まる行自身の引出線が走る帯を横切らない。 <sub>`a_same_level_jumper_that_skips_a_row_never_crosses_the_skipped_rows_lead_line`</sub>
- 1つの極の2つの段を繋ぐジャンパ("1a-1b")は、必ず物理的に隣り合う行を繋ぐ (段はクラスタでまとまる) ので、単段の端子台と同じ、箱のすぐそばの近いレーンを使う。 <sub>`a_cross_level_jumper_between_adjacent_rows_uses_the_same_near_lane_as_single_level`</sub>
- 段の違う2つの同段ジャンパ("1a-2a"と"1b-2b")はそれぞれ別の段の行を飛び越えるので、それぞれ専用のレーンを使う: 2つのレーンは別のXになり、どちらもどこかの行の引出線が走る帯には入らない。 <sub>`each_level_gets_its_own_distinct_jumper_lane`</sub>
- 段ごとのジャンパレーンはどれも、ブラケットとハーネス名が入る20mmの帯の外側で、かつ印刷できる紙の内側に収まる (2段でも3段でも) ので、レーンがハーネス名の上を走ることも紙からはみ出すこともない。 <sub>`every_jumper_lane_clears_the_harness_zone_and_stays_inside_the_page`</sub>
- 内部側の接続先表記は段レーンの帯の幅だけ短く収められるので、文字がレーンに届くことはない。単段の端子台では帯は0で、表記の幅は変わらない。 <sub>`the_inside_label_width_reserves_the_lane_band`</sub>
- 行を飛び越える同段ジャンパがあっても、千鳥配置 (行ごとに1箱、段は列) の外部側左・内部側右は崩れない: 箱の列や引出線の向きは、ジャンパ線がどのレーンを使うかに影響されない。 <sub>`the_staggered_layout_keeps_outside_left_and_inside_right_with_skipping_jumpers`</sub>
- 単段の端子台の接続図には段の見出しが付かない (端子の列が1本しかないため)。 <sub>`a_single_level_terminal_diagram_has_no_level_heading`</sub>
- 1ページに収まらない端子は次のページへ続き、各ページの表題にそのページの端子の範囲が入る。 <sub>`terminals_that_do_not_fit_continue_on_the_next_page`</sub>
- ページ表題の端子の範囲は、そのページに載る端子の最小と最大を書く。ジャンパ順のように行が端子番号順に並んでいない並べ替えでも、範囲として読める表記のままになる。 <sub>`the_page_title_range_names_the_smallest_and_largest_terminal_on_the_page`</sub>
- 端子台1つにつき1ページが出て、端子台でないものには1ページも出ない。 <sub>`one_page_per_terminal_block_and_none_for_anything_else`</sub>
- 端子接続図のページはA4横で、JIS図枠と、端子台の参照記号が入った表題欄を持つ。 <sub>`the_page_has_the_frame_and_a_title_block`</sub>
- 端子接続図はterminal-diagramという名前の帳票として選べ、CLI・Link API・MCPで同じ綴りになる。 <sub>`the_terminal_diagram_is_a_report_named_terminal_diagram`</sub>
- 端子台が1つも無いプロジェクトでも1ページは出るので、帳票が空になることはない。 <sub>`a_project_without_terminal_blocks_still_yields_one_page`</sub>

### text_style

- 注記の色はどれも印刷色を持つ名前付きトークンで、既定のトークンは黒で刷られる。 <sub>`every_colour_token_has_a_print_colour`</sub>
- 生の色コードは色トークンではないので、図面に紛れ込むことはない。 <sub>`a_raw_colour_code_is_not_a_colour_token`</sub>
- 色を指定していない注記は既定色で描かれ、トークンを指定した注記はそのトークンの印刷色で描かれる。 <sub>`a_note_without_a_colour_uses_the_default_colour`</sub>
- 3つの基準位置はSVGの text-anchor の3つの値に対応し、既定は従来の図面が使ってきた左揃えである。 <sub>`anchors_map_to_svg_text_anchor`</sub>
- 機能ラベルの属性が無いシンボルには、余分に描くものが無い。 <sub>`a_symbol_without_a_function_label_draws_nothing_extra`</sub>
- 空白だけの機能ラベルは「無し」と同じ扱いになるので、うっかりの空白で空行が刷られることはない。 <sub>`a_blank_function_label_counts_as_no_label`</sub>
- 同梱シンボル (機能スロットを持たない) では、機能ラベルはシンボル外形のすぐ下に、配置基準点を中心にして置かれる。 <sub>`the_function_label_sits_below_a_symbol_that_defines_no_slot`</sub>
- シンボル定義がFUNCTIONスロットを持つ場合、機能ラベルは自動位置ではなくそのスロット (回転も込み) に従う。 <sub>`the_function_label_follows_a_slot_defined_by_the_symbol`</sub>

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
- ピングループのキャプション(G8)もシートの文字として測られるので、他の文字と重なっていれば重なりの指標が拾う。 <sub>`a_pin_group_caption_is_measured_as_a_text_box`</sub>
- タグ付きネットラベルの文字は旗の中として測られるので、重なりの数は実際に印字される見た目と一致する。 <sub>`a_tagged_net_label_is_measured_inside_its_flag`</sub>
- 注記文字高を小さくすると、測られるネットラベル・線番の大きさも一緒に小さくなる。 <sub>`measured_annotation_text_follows_the_sheet_setting`</sub>
- シンボル原点も配線頂点も2.5mmグリッドに乗っていれば、グリッド外は0件になる。 <sub>`entities_on_the_grid_report_no_off_grid_points`</sub>
- グリッドから0.1mmずれた点は数えられ、ずれた頂点1つにつき1件になる。 <sub>`a_point_shifted_a_tenth_of_a_millimetre_is_off_grid`</sub>
- 印字しない参照記号は、整えメトリクスの文字の当たり判定に入らない。 <sub>`a_hidden_designator_has_no_text_box`</sub>
- 何も置かれていないシートは完全に整っており、全てのメトリクスが0になる。 <sub>`an_empty_sheet_scores_zero_on_every_metric`</sub>
- tidy_metricsは1枚のシートの4つの数値をまとめて1つの値で返す。 <sub>`tidy_metrics_gathers_the_four_counts`</sub>
- 同じシートを2回測ると全く同じ数値になるので、エージェントは編集の前後を比べられる。 <sub>`measuring_the_same_sheet_twice_gives_the_same_numbers`</sub>
- tidy_issuesは数えた問題を1件ずつ、種類・関係するエンティティ・文字・場所つきで並べ、件数と一致するので、エージェントは1件ずつ見つけて直せる。 <sub>`tidy_issues_name_each_counted_problem`</sub>

### 整えバリアント (シート複製 / 採用)

- 案の複製は用紙・表題欄・改訂欄・シート種別・注記文字高と全エンティティを元のまま持ち、シートidとエンティティidだけ新しく、名前にラベルが付き、どのシートの複製かの印が付き、元id→新idの対応表が全要素分できる。 <sub>`duplicate_keeps_content_with_fresh_ids_and_a_full_map`</sub>
- 複製の引き出し線つき注記は「複製側の対象」を指すので、複製だけで成り立つ正しいシートになる。対象は複製の中に実在し、シートの外を指す参照は残らず、図面検証も引き出し線の対象欠落を出さない。 <sub>`duplicate_repoints_leader_lines_inside_the_copy`</sub>
- 案の複製がある間も、元シートと各案のシート間クロスリファレンス住所 (`/2.B3`) は増えない。複製は「同じ参照記号を持つ別シート」として数えないため。 <sub>`variant_copies_do_not_grow_cross_reference_addresses`</sub>
- N案で開始すると既存シートの後ろへ入るRestoreSheetがN個でき、案ごとにラベルと対応表が付く。1〜4以外の数は拒否される。 <sub>`start_builds_one_restore_sheet_per_variant`</sub>
- 採用すると、動かした要素は元idのまま更新、案で消えた要素は削除、案で増えた要素は新idで追加、変わらない要素は触らず、最後に全ての案シートが消える。 <sub>`adopt_maps_changes_back_to_original_ids`</sub>
- 何も変わっていない案を採用すると、案シートを消すだけになる。引き出し線つき注記は複製側と元で対象idが違っても「変わっていない」と見なす。 <sub>`adopting_an_unchanged_variant_only_removes_sheets`</sub>
- 案が引き出し線を張り替えた場合と、引き出し線つき注記を増やした場合の採用は、対象を元シート側のidへ写してエンジンを通る。案が消した要素を指していた引き出し線は宙ぶらりんにせず外す。 <sub>`adopt_repoints_leaders_to_original_side_ids`</sub>
- 案が引き出し線の対象を消した場合、採用は「残らない対象」を書き戻さずに引き出し線を外す。 <sub>`adopt_drops_a_leader_whose_target_the_variant_deleted`</sub>
- 終了は、元シートや案シートが無い実行・その元シートの案ではないシート・写し先が元シートのidになっている対応表・案に含まれない採用先・比較開始後に元シートが手編集された採用を、理由つきで断る。手編集後でも破棄はできる。 <sub>`finish_validates_the_run_against_the_current_drawing`</sub>
- エンジンで開始→案を編集→採用すると、元シートが案の配置になり案シートは残らない。開始・編集・採用はそれぞれundo1回で戻る。 <sub>`engine_round_trip_is_one_undo_per_step`</sub>

### 検証 (ERC・電気検証)

- 完全に結線された回路ではERCの指摘は出ない。 <sub>`fully_wired_pair_has_no_erc_findings`</sub>
- 用紙の内側に収まっているエンティティは`erc.outside_paper`警告を出さない。 <sub>`an_entity_inside_the_paper_has_no_outside_paper_warning`</sub>
- 用紙の縁ちょうどに収まるエンティティは指摘されない (縁を越えたときだけ指摘する)。 <sub>`an_entity_exactly_on_the_paper_edge_has_no_warning`</sub>
- シートの用紙の外に出ているエンティティは、動かしたり消したりせずに`erc.outside_paper`として報告される。 <sub>`an_entity_outside_the_paper_is_reported`</sub>
- 未接続ピンはシンボルごとに1件の警告として、該当ピン番号を列挙して報告される。 <sub>`unconnected_pins_are_reported_per_symbol`</sub>
- ブロックの未接続ピンはピン名で列挙されるので、コントローラの未配線入力が番号ではなく名前で分かる。 <sub>`unconnected_block_pins_are_listed_by_pin_name`</sub>
- 端子台の端子は左右どちらか一方が結線されていれば接続済みとみなす。 <sub>`terminal_block_terminal_counts_connected_if_either_side_wired`</sub>
- 参照記号の未設定・重複は指摘されるが、リレーのコイル+接点が同じ記号を共有するのは正当として許容される。 <sub>`empty_and_duplicate_references_are_flagged_but_relays_allowed`</sub>
- 同じ参照記号を持つプラグとソケットは1つの嵌合コネクタなので重複エラーにならないが、無関係な部品と同じ記号のプラグは指摘される。 <sub>`a_plug_and_socket_may_share_a_designator`</sub>
- 他のグループと重なる・端子台に無いピンを指すpin_groups属性は、黙って無視されるのではなく端子台とその項目名を添えたERCエラーとして報告される。 <sub>`a_bad_pin_groups_item_is_reported_as_an_erc_error`</sub>
- 実機ブロック表示のコイルにはピングループ括弧を描けないので、黙って消えるのではなくERCエラーとして理由が報告される。 <sub>`pin_groups_on_a_device_block_coil_are_reported_instead_of_silently_dropped`</sub>
- 引き出し先を後から削除された注記は指摘される。行き先の無い引き出し先を黙って抱えたままにしない。 <sub>`a_note_whose_leader_target_was_deleted_is_reported`</sub>
- どこにも接続されていないワイヤ端点は指摘される(ジャンクション無しで他ワイヤの途中に乗る端点も含む)。 <sub>`dangling_wire_end_is_flagged_including_missing_junction`</sub>
- 健全な直列回路(電源・ヒューズ・スイッチ・ランプ)では電気検証の指摘は出ない。 <sub>`healthy_series_circuit_has_no_elec_findings`</sub>
- 電源から切り離された負荷(断線・開路)は「到達不能」として報告される。 <sub>`load_cut_off_from_source_is_unreachable`</sub>
- 負荷電流がワイヤの許容電流を超えるとエラー、ヒューズ定格を超えると警告になる。 <sub>`overloaded_wire_and_fuse_are_flagged`</sub>
- 長い配線で電源電圧の3%を超える電圧降下は指摘され、短い配線では出ない。 <sub>`excessive_voltage_drop_is_flagged`</sub>
- current_a属性が無い負荷は電流計算から除外され、Infoとして通知される。 <sub>`load_without_current_attr_gets_info_and_no_current_checks`</sub>
- ngspiceがあれば電気検証はソルバの実測値で報告され、近似モードの通知は出ない。 <sub>`simulation_mode_reports_measured_values_when_ngspice_installed`</sub>
- ソルバ結果が無い場合はグラフ近似にフォールバックし、その旨をInfoで明示する。 <sub>`fallback_mode_emits_approximate_info`</sub>
- 1つのネットに異なるネットラベルが混在する状態(異電位の短絡)はエラーになる。 <sub>`conflicting_net_labels_on_one_net_are_an_error`</sub>
- 配線の頂点上・配線の途中・シンボルのピン上にあるネットラベルはきちんと載っているので、指摘されない。 <sub>`a_net_label_on_a_wire_or_a_pin_is_not_reported`</sub>
- 配線から0.5mmでもずれたネットラベルは、黙ってネットから外れてしまうので、ラベル名を挙げた警告として報告される。 <sub>`a_net_label_that_floats_off_the_wire_is_reported`</sub>
- 接続の許容誤差 (0.01mm) の内側にあるネットラベルは載っている扱いなので、ふつうの丸め誤差で警告が出ることはない。 <sub>`a_net_label_within_the_connection_tolerance_is_still_attached`</sub>
- シート1枚の中のラベル競合は、プロジェクト全体を検証しても1件だけ報告される。 <sub>`project_verification_reports_a_single_sheet_label_conflict_once`</sub>
- 同名ラベルでシートを跨いで繋がったネットは1ネットとして検査されるので、2枚に跨る競合も全ラベルを挙げた1件として報告される。 <sub>`project_verification_merges_label_conflicts_across_sheets`</sub>
- 同じラベル名でシートを跨いで続くネットは、競合ではない。 <sub>`project_verification_accepts_a_net_continued_onto_another_sheet`</sub>
- 多極の遮断器は極ごとに導通するため、結線されていない極につながる負荷は電源から到達できないと報告される。 <sub>`multipole_breaker_does_not_connect_its_poles_to_each_other`</sub>
- 盤レイアウトシートは回路図の規則で検査しない。配線を持たないシートなので、未接続ピンや宙ぶらりんの配線として指摘されるものは無い。 <sub>`a_panel_sheet_is_not_checked_against_the_schematic_rules`</sub>
- 回路図のどこにも無い参照記号のフットプリントが盤に置かれていると注意として報告され、プロジェクト全体の検証では回路図の指摘と一緒に出る。 <sub>`an_orphan_footprint_is_reported_by_the_project_check`</sub>

### wire_legend

- 凡例ブロックを置いてもシートのネットリストは変わらない (電気的な意味を持たない作図要素だから)。 <sub>`a_legend_block_does_not_appear_in_the_netlist`</sub>
- 凡例ブロックを置いても、そのブロックについての検証指摘は出ず、他の指摘も増減しない。 <sub>`a_legend_block_raises_no_verification_diagnostic`</sub>
- 凡例ブロックを置いても部品表は変わらない (部品でもなく、電線の使用長も増やさない)。 <sub>`a_legend_block_does_not_change_the_bom`</sub>
- 凡例ブロックを置いても電線リストは変わらない (導体ではないから)。 <sub>`a_legend_block_does_not_change_the_wire_list`</sub>
- 配線が1本も無いシートでも、凡例ブロックは列見出しだけの表として出る。 <sub>`an_empty_sheet_has_only_the_header_row`</sub>
- 線色・線径・引き当てた品番が同じ配線は1行にまとまり、使用長は合算される。 <sub>`wires_of_the_same_colour_gauge_and_part_number_are_summed_into_one_row`</sub>
- 英語名と日本語名で書かれた同じ線色は同じ電線なので、1行にまとまり使用長も合算される。 <sub>`a_colour_written_in_english_and_in_japanese_makes_one_row`</sub>
- 凡例に出る色名は、同じシートの配線注記が印字する色名と同じになる。 <sub>`the_legend_prints_the_same_colour_name_as_the_wire_annotation`</sub>
- 行は線色順、同じ線色の中では線径順に並ぶ。 <sub>`rows_are_sorted_by_colour_then_gauge`</sub>
- 足し合わせると二進小数で誠実に表せない使用長も、桁の並んだ長い数字ではなく短い小数で表示される。 <sub>`a_summed_length_is_rounded_instead_of_showing_floating_point_noise`</sub>
- 電線品番表に無い組み合わせの配線は品番欄が空欄のままで、勝手に品番が作られることはない。 <sub>`an_unresolved_part_number_shows_as_blank`</sub>

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
- コネクタ同士を結ぶ1本線ケーブルがあるシートで線番を振り直しても、配線は1, 2と飛ばずに番号が付く (配線の無い芯のネットは番号を使わない)。 <sub>`core_only_nets_take_no_wire_number`</sub>

### wire_note

- 線径・線色・電線長がそろったワイヤには「0.75sq 白 0.4m」という注記が付く。 <sub>`note_shows_gauge_colour_and_length`</sub>
- 欠けている属性は注記から省かれ、長さの無いワイヤは「0.75sq 白」と書かれる。 <sub>`note_omits_missing_attributes`</sub>
- 線径も長さも無い黒の配線には注記を付けない (黒は既定の色で、全ての線に「黒」と書くと図面が埋まるため)。線径か長さのある黒の配線は色名も書く。 <sub>`a_plain_black_wire_gets_no_note`</sub>
- 線径も色も長さも無いワイヤには、空の注記ではなく注記そのものが付かない。 <sub>`wire_without_attributes_has_no_note`</sub>
- 色名は英字の大小と前後の空白を無視して照合される。知っている色は表示用の名前で、知らない色は入力のまま (ただし小文字にそろえて) 書かれる。 <sub>`colour_names_are_matched_ignoring_capitals_and_space`</sub>
- 横向きのワイヤの注記は配線の下に置かれ、上に出る線番とは反対側になる。 <sub>`horizontal_wire_note_sits_below_the_wire`</sub>
- 縦向きのワイヤの注記は配線の右に置かれ、左に出る線番とは反対側になる。 <sub>`vertical_wire_note_sits_right_of_the_wire`</sub>
- 注記はワイヤ1本ごとに付く(ネットごとに1回だけ描かれる線番とは違う)。 <sub>`each_wire_gets_its_own_note`</sub>
- 注記文字高を設定していないシートは、これまでどおり2.0mmの注記で描かれる。 <sub>`a_sheet_without_a_setting_keeps_the_default_note_height`</sub>
- シートの注記文字高を1.4mmにすると、配線注記はちょうどその高さで描かれる。 <sub>`the_sheet_setting_is_the_wire_note_height`</sub>
- 注記文字を小さくすると注記が配線に寄るので、5mm間隔で並ぶ配線でも読める余白が残る。 <sub>`smaller_annotation_text_moves_the_note_closer_to_the_wire`</sub>
- 折れ曲がったワイヤの注記は、いちばん長い直線部分に置かれて読みやすくなる。 <sub>`note_is_placed_on_the_longest_straight_run`</sub>

### 電線品番 (線色+線径の表)

- ワイヤの電線品番は、線色と線径からプロジェクトの電線品番表を引いて決まる。 <sub>`a_wire_takes_its_part_number_from_the_colour_and_gauge_table`</sub>
- 表に無い線色+線径の組み合わせは品番が空のままで、勝手に品番が作られることはない。 <sub>`an_unregistered_combination_stays_empty`</sub>
- ワイヤ自身に書かれた品番は表より優先されるので、手で決めた品番が表で上書きされることはない。 <sub>`a_part_number_written_on_the_wire_wins_over_the_table`</sub>
- 引き当ては線色の前後の空白と英字の大小を無視する。 <sub>`the_lookup_ignores_spaces_and_letter_case_in_the_colour`</sub>
- ワイヤの英語の色名と表の日本語の色名は同じ電線を指すので、日本語で書いた表がそのまま英語の色名の図面に効く。 <sub>`the_english_and_japanese_names_of_a_colour_mean_the_same_wire`</sub>
- 線色が空・線径が0のワイヤは引き当てない。 <sub>`a_wire_without_a_colour_or_a_gauge_is_not_looked_up`</sub>
- 同じ線色+線径の行が表に2つあるときは、先に書いた行が勝つ。 <sub>`the_first_row_wins_when_the_table_has_duplicates`</sub>
- 電線品番表はCSVで往復でき、線色・線径・品番・備考がそのまま戻る。 <sub>`the_wire_part_table_round_trips_through_csv`</sub>
- 見出し行の無いCSVもそのままデータとして読め、備考の列は省略できる。 <sub>`a_csv_without_a_header_row_and_without_the_note_column_is_read`</sub>
- 壊れたCSV (列数違い・線径が数値でない・品番が空) は拒否され、1行も取り込まれない。 <sub>`a_malformed_csv_is_refused`</sub>
- CSVの取り込みはCommandエンジンを通して表全体を置き換え、undo一発で元の表に戻る。 <sub>`importing_a_csv_replaces_the_table_and_undo_restores_it`</sub>
- 読めないCSVを取り込もうとしても、表は元のままである。 <sub>`a_csv_that_cannot_be_read_leaves_the_table_unchanged`</sub>

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
- 注記文字高を小さくすると、クロスリファレンスも同じ比率でラベルに寄る。 <sub>`cross_reference_follows_the_annotation_text_height`</sub>
- クロスリファレンス表はプロジェクト全体のネット1本につき1行で、跨るシートとラベルの図面上の住所を並べる。 <sub>`cross_reference_table_lists_one_row_per_net_with_its_sites`</sub>
- クロスリファレンス表の列は ネット・線番・接続先・シート・所在 の5列。 <sub>`cross_reference_table_columns_are_net_wire_pins_sheets_sites`</sub>
- 2枚の違うシートに描かれた参照記号は相互に参照し合う: それぞれのインスタンスにもう一方のシートの住所が出る。 <sub>`a_designator_on_two_sheets_cross_references_each_other`</sub>
- 1枚のシートにしか描かれていない参照記号には、クロスリファレンスが一切付かない。 <sub>`a_designator_on_a_single_sheet_has_no_cross_reference`</sub>
- 接点を従えたリレーのコイルと、その接点は、デバイス単位のクロスリファレンスの対象から外れる(コイル⇔接点クロスリファレンスが既に互いの所在を書いているため)。 <sub>`a_relay_coil_and_its_contact_are_skipped_by_the_device_cross_reference`</sub>
- 接点をどこにも持たないコイルにはコイル⇔接点クロスリファレンスが何も描かないので、そういうコイルだけの参照記号には普通のデバイス単位クロスリファレンスが付く (G13)。 <sub>`a_contactless_coil_on_two_sheets_still_gets_a_device_cross_reference`</sub>
- シートごとのデバイスXRef表は、各シンボルのentity idを脇に描くテキストへ対応付ける。 <sub>`sheet_device_cross_reference_table_maps_symbols_to_text`</sub>
- 3枚のシートに跨る参照記号は、シート順で並んだ他シート全部の住所を1行に並べる。 <sub>`a_designator_on_three_sheets_lists_every_other_sheet`</sub>
- デバイスXRefのテキストはシンボル外形の右側、参照記号のラベルと同じ高さに置かれる。 <sub>`device_cross_reference_text_sits_right_of_the_outline`</sub>
- 参照記号のラベルがシンボルの外形より横に長いときは、デバイスXRefはその右まで下がるので、ラベルと重ならない。 <sub>`device_cross_reference_clears_a_designator_label_wider_than_the_symbol`</sub>


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

### drawing_tools

- place_symbol は置いた部品のピンを用紙上の位置つきで返すので、エージェントは座標を計算せずに配線できる。 <sub>`place_symbol_returns_the_pins_of_the_new_part`</sub>
- get_pins は参照記号か entity id で部品を探し、ピンと外形を返す。 <sub>`get_pins_finds_parts_by_reference_or_entity_id`</sub>
- ピン操作ツールの説明には、ピン座標を計算せずピン名で配線するよう書かれている。 <sub>`the_pin_tools_tell_the_agent_not_to_compute_coordinates`</sub>
- connect_pins はピン名を指定した2本のピンを直角の線分で結び、その後の検証で未接続ピンが出ない。 <sub>`connect_pins_wires_two_pins_by_name`</sub>
- connect_pins は渡した経由点を順に通る。 <sub>`connect_pins_passes_via_points`</sub>
- connect_pins は不明な部品・不明なピン・どれか決まらない参照記号・同じピンどうしを断り、何も作らない。 <sub>`connect_pins_refuses_bad_specifiers_and_creates_nothing`</sub>
- place_mate はプラグとソケットの組を1回で置き、両方に参照記号が付いて参照記号の警告は出ず、外側の2つの接続点を返し、1回の取り消しで組ごと消える。 <sub>`place_mate_puts_a_pair_that_one_undo_removes`</sub>
- connect_pins は別のピンの上を通る経路を断って何も作らないので、ピンが誤って回路に入ることがない。 <sub>`connect_pins_refuses_a_route_over_another_pin`</sub>
- connect_pins は配線のリストを受け取って1回で全部引き、1回の取り消しで全部消えるので、エージェントは配線1本ごとに呼ばずにシートを配線できる。 <sub>`connect_pins_draws_a_list_of_connections_in_one_edit`</sub>
- リストの配線のどれか1本でも誤っていれば、connect_pins はその番号を知らせて1本も作らない。 <sub>`connect_pins_list_is_all_or_nothing`</sub>
- place_mate は組のリストを受け取って1回で全部置き、1回の取り消しで全部消える。 <sub>`place_mate_places_a_list_of_pairs_in_one_edit`</sub>
- connect_cable は LTE ハーネスを1回で描く: 基板側の配線4本と分岐、USBプラグまでの1本線、ピン順の芯の対応。ネットリストは4ネット、ERC は未接続ピンを出さず、undo 1回で全部消える。 <sub>`connect_cable_draws_a_breakout_harness_in_one_edit`</sub>
- リストのケーブルのどれか1本でも誤っていれば、connect_cable はその番号を知らせて1本も描かない。正しいリストは1回の編集で描く。 <sub>`connect_cable_list_is_all_or_nothing`</sub>
- connect_cable の説明は、芯がピン順に対応することと、偽の分岐ブロックの代わりに使うことをエージェントに伝える。 <sub>`connect_cable_is_described_for_the_agent`</sub>

### 図枠テンプレート (REST)

- GET /frame-templates は同梱の図枠テンプレートを英日の名前つきで返し、ユーザーが自分の図枠を置くフォルダの場所も添える。 <sub>`the_link_api_lists_the_bundled_frame_templates`</sub>
- 一覧には図枠の定義一式(用紙・マージン・ゾーン・表題欄のセル・改訂欄の列幅)が入っているので、クライアントはファイルを読まずにプレビューできる。 <sub>`a_listed_frame_template_carries_its_whole_definition`</sub>
- POST /commands の set_frame_template でプロジェクト全体の図枠が切り替わり、undo一発で元の図枠へ戻る。 <sub>`setting_the_frame_template_over_the_link_api_is_one_undo_step`</sub>
- 書き出したSVGはプロジェクトの図枠テンプレートに従う: A4縦へ切り替えると表題欄が120mmではなく110mm幅で描かれる。 <sub>`the_exported_svg_follows_the_projects_frame_template`</sub>
- 新規プロジェクトは、アプリ設定で「グローバル既定」に選んだ図枠テンプレートで始まる。 <sub>`a_new_project_starts_with_the_default_frame_template_from_the_settings`</sub>
- 設定にグローバル既定が無ければ、新規プロジェクトの図枠は未指定のまま(同梱のJIS A3図枠で描かれる)。 <sub>`a_new_project_without_a_default_setting_keeps_the_frame_unset`</sub>
- 設定の既定図枠が空欄のときは、未知のidではなく「既定なし」として扱う。 <sub>`a_blank_default_frame_template_setting_is_ignored`</sub>
- POST /frame-templates/save は同梱の図枠と同じidや、ファイル名にできないidの図枠を400で拒否して何も書かない(エディタは別名を求める)。 <sub>`the_link_api_refuses_to_save_over_a_bundled_frame`</sub>

### instance_guard

- 別の起動のインスタンスidを持つリクエストは図面に届く前に断られるので、落ちた・再起動したアプリから残ったエージェントCLIは新しいセッションを編集できない。 <sub>`a_request_from_an_earlier_app_run_is_refused`</sub>
- この起動のidを持つエージェント自身のリクエストと、ヘッダの無い外部クライアントは今までどおり通る。 <sub>`this_run_and_external_clients_are_served`</sub>
- アプリを起動するたびに別のインスタンスidになる。 <sub>`each_app_run_gets_its_own_instance_id`</sub>

### REST Link API

- RESTの部品エンドポイントは検索(サンプル含む)・登録更新・カテゴリ絞り込み・削除・電線品番一覧に対応する。 <sub>`parts_endpoints_search_upsert_delete`</sub>
- GET /api/v1/verify は図面の診断(空参照・未接続ピンなど)をJSONで返す。 <sub>`verify_returns_diagnostics`</sub>
- POST /api/v1/import/kicad は開いているプロジェクトを変換結果で置き換え、patchとインポートレポートを返す。 <sub>`import_kicad_replaces_project_and_reports`</sub>
- POST /api/v1/simulate/op はDC動作点を解き、ネット電圧と部品電流を返す(ngspice必須。未導入時はスキップ)。 <sub>`simulate_op_returns_result`</sub>
- POST /api/v1/export/pdf は指定パスへ正しいPDFファイルを書き出す。 <sub>`export_pdf_writes_pdf_file`</sub>
- POST /api/v1/export/dxf と /export/kicad はシートをDXFと.kicad_schへ書き出し、POST /api/v1/import/dxf はそのDXFをインポートレポート付きでプロジェクトへ読み戻す。 <sub>`export_dxf_kicad_and_import_dxf_round_trip`</sub>
- POST /api/v1/variants/start はシートを案の数だけ複製し(undo1回)、案ごとのid対応表を返す。/variants/finish は選んだ案を元idのまま元シートへ写し戻して複製を消し、nullなら複製を消すだけ。1〜4以外の数は拒否される。 <sub>`variants_start_and_finish_round_trip`</sub>
- POST /api/v1/variants/finish は、複製がもう無い実行・案に含まれない採用先・比較中に元シートを手編集した後の採用を断る。破棄はそれでも通り、開始の案の数0も拒否される。 <sub>`variants_finish_refuses_a_run_that_no_longer_matches_the_drawing`</sub>
- POST /api/v1/export/pdf-book は表紙・全シート・指定した帳票を1つのPDFにまとめて書き出す。 <sub>`export_pdf_book_writes_cover_sheets_and_reports`</sub>
- 端子台エンドポイントは図面の端子台を一覧し、1台のチャートとチェック結果を返す。 <sub>`terminal_endpoints_list_chart_and_check`</sub>
- POST /api/v1/export/report は帳票1種をCSVまたは図枠付きPDFで書き出し、図面である端子接続図のCSVは拒否する。 <sub>`export_report_writes_csv_and_pdf_per_report`</sub>
- POST /api/v1/export/report の部品表は、部品DBからメーカ・品名・単価・金額・購入先の欄を埋め、部品DBに無い型番ではそれらを空欄にする。 <sub>`export_report_bom_looks_part_numbers_up_in_the_parts_database`</sub>

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

### PLC I/O (MCPツール / REST)

- PUT /plc/assignments はI/O割付表を丸ごと置き換え、GETはモジュールごとの表として読み出す。 <sub>`the_assignment_table_can_be_written_and_read_over_the_link_api`</sub>
- POST /plc/assignments/import はアドレス・信号名・コメントのCSVを1モジュールぶん取り込み、undo一発で取り込み前へ戻る。 <sub>`a_csv_of_signal_names_can_be_imported_and_undone`</sub>
- 3列に足りない行のあるCSVは400で断られ、割付表は元のまま変わらない。 <sub>`a_malformed_csv_import_is_refused_with_a_bad_request`</sub>
- POST /plc/generate はI/Oラダーページを1回の編集で作り、新しいシートにはモジュールのシンボルと点数ぶんのラングが載る。GET /plc/modules はその置かれたモジュールを一覧に返す。 <sub>`generating_an_io_page_over_the_link_api_is_one_undo_step`</sub>
- 図面にできない生成設定 (分割の単位0点) は400で断られ、図面には何も起きない。 <sub>`an_impossible_generation_setting_is_refused_with_a_bad_request`</sub>
- PLC I/Oレポートは他の帳票と同じ書き出し口から`plc-io`として出力され、割付済みの点ごとに1行が並ぶ。 <sub>`the_plc_io_report_is_exported_like_any_other_report`</sub>
- PLCのMCPツールの説明には、割付表が何のためのものか・ページ生成がundo一発であることが書かれている。 <sub>`the_plc_tools_explain_the_assignment_table_and_the_generated_page`</sub>

### project_tools

- save_project で保存したプロジェクトは open_project で開き直せ、開いていた図面と置き換わる。 <sub>`a_saved_project_opens_again_and_replaces_the_open_drawing`</sub>
- 無いファイルや MadakeCAD のプロジェクトでないファイルを開こうとすると理由つきで失敗し、開いている図面はそのまま残る。 <sub>`opening_a_missing_or_foreign_file_fails_and_keeps_the_drawing`</sub>
- アプリ内エージェント用のツール窓口 (エージェント管理が付いていない) からはプロジェクトを開けない。自分の応答中に図面が差し替わってしまうため。 <sub>`the_in_app_tool_window_cannot_open_a_project`</sub>
- エージェントの応答中は、open_project はそのターンを中断せずに断る。MCP でつながったエージェントが自分の応答を断ち切らないようにするため。 <sub>`opening_is_refused_while_an_agent_turn_is_running`</sub>
- アプリ内のツール窓口から保存すると図面だけを書く。チャット履歴はエージェント管理が付いているときだけ保存する。 <sub>`saving_without_the_agent_manager_writes_the_drawing_only`</sub>
- MCP サーバーは open_project と save_project を公開し、説明には扱うファイルの種類と「開くと図面が置き換わる」ことが書かれている。 <sub>`the_server_offers_open_and_save_with_clear_descriptions`</sub>

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

### sheet_image

- シート画像は既定で 4 px/mm のシート全体のPNGで、mm で範囲を指定すると一部を拡大できる。 <sub>`the_sheet_image_is_a_png_of_the_sheet_or_a_region`</sub>
- ツール結果の画像を表示できない経路 (ツール窓口を通るAPI系) には、巨大なbase64の代わりに短い説明を返す。 <sub>`the_bridge_replaces_the_image_with_a_short_note`</sub>
- ツールの説明には、編集した後に図面を目で確かめるよう書かれている。 <sub>`the_image_tool_tells_the_agent_to_look_after_editing`</sub>

### ユーザーシンボルライブラリ (REST)

- GET /symbols は同梱ライブラリとユーザーが作ったシンボルをまとめて返す。ユーザーのシンボルは最後に並び、分類は常に「user」になる。 <sub>`the_link_api_lists_bundled_and_user_symbols_together`</sub>
- GET /symbols/user はユーザーのシンボルだけを返し、置き場のパスも添える(クライアントが「シンボルのフォルダを開く」を出せるように)。 <sub>`the_link_api_lists_the_user_symbols_with_their_folder`</sub>
- Link APIで保存したユーザーシンボルは、すぐに図面へ配置でき、そのピンも同梱シンボルと同じように検証の対象になる。 <sub>`a_saved_user_symbol_can_be_placed_like_a_bundled_one`</sub>
- ユーザーシンボルを編集すると、すでに図面に置いてある部品も変わる。図面はシンボル定義のスナップショットを持たず、シンボルidだけを持っているため。 <sub>`editing_a_user_symbol_changes_the_parts_already_placed`</sub>
- おかしなシンボルの保存は理由を一度に全部返し、ファイルは書かない。直しかけの定義がライブラリに残らないようにするため。 <sub>`saving_an_invalid_user_symbol_returns_every_reason_and_writes_nothing`</sub>
- 同梱シンボルと同じidのシンボルは保存を拒否する。ファイルを置いただけで、そのシンボルを使っている図面が黙って変わることが起きないようにするため。 <sub>`a_user_symbol_may_not_take_a_bundled_symbol_id`</sub>
- シンボルライブラリはundo履歴の外にある。保存したあとにundoすると、戻るのは直前の図面の編集で、保存したシンボルは消えない。 <sub>`the_symbol_library_is_outside_the_undo_history`</sub>
- まだ図面で使われているユーザーシンボルの削除は、使用数を添えて拒否される(強制指定したときだけ消える)。 <sub>`deleting_a_user_symbol_in_use_is_refused_unless_forced`</sub>
- 使われていないユーザーシンボルはそのまま削除できる。 <sub>`deleting_an_unused_user_symbol_succeeds`</sub>
- シンボルの複製は、空いているidと「コピー」の名前を持った下書きを返すだけで、保存するまでファイルは書かれない。 <sub>`duplicating_a_symbol_returns_a_draft_that_is_not_saved_yet`</sub>
- 存在しないシンボルの複製は、空のシンボルではなくエラーを返す。 <sub>`duplicating_an_unknown_symbol_is_an_error`</sub>
- シンボルのフォルダに壊れたファイルがあっても一覧は止まらない。正しいシンボルは読み込まれ、壊れたファイルはパスと理由つきで報告される。 <sub>`a_broken_file_in_the_symbols_folder_is_reported_by_the_api`</sub>

### 開始テンプレート (REST)

- GET /templates は同梱の開始テンプレートを英日の名前つきで返す。 <sub>`the_link_api_lists_the_bundled_templates`</sub>
- POST /templates/apply はテンプレートを1回の編集としてシートへ入れ、undo一発で戻せる。 <sub>`applying_a_template_over_the_link_api_is_one_undo_step`</sub>
- シートidを省くと先頭シートが対象になり、知らないテンプレートidは400で拒否される。 <sub>`the_link_api_defaults_to_the_first_sheet_and_refuses_unknown_templates`</sub>

### 整えメトリクス (MCPツール / REST)

- 整えメトリクスのツール説明には「整えループの目標値として使う」ことと、数える3つの対象が書かれている。 <sub>`the_tidy_metrics_tool_is_advertised_as_the_tidy_loop_target`</sub>
- GET /api/v1/tidy-metrics はシートの交差数・ラベル重なり数・シンボル重なり数・グリッド外数をJSONで返す。 <sub>`tidy_metrics_endpoint_returns_the_four_counts`</sub>
- 整えメトリクスは数えた問題を1件ずつ (種類・エンティティ・文字・場所) 並べるので、エージェントは直す場所を見つけられる。一覧には上限があり、省いた件数も返す。 <sub>`tidy_metrics_list_each_problem_with_its_place`</sub>

### tool_bridge

- API経由で見えるツールはCLI経由とまったく同じ顔ぶれで、片方だけ機能が欠けることがない。 <sub>`the_api_route_sees_the_same_tools_as_the_cli_route`</sub>
- 窓口から見えるツールには全て説明が付き、入力スキーマはオブジェクト型になっている。 <sub>`every_bridged_tool_has_a_description_and_an_object_schema`</sub>
- 窓口経由の編集は図面に入り、他の編集と同じように元に戻せる(Commandエンジンを通っている)。 <sub>`an_edit_through_the_bridge_lands_in_the_document_and_can_be_undone`</sub>
- 名前付きピンを持つ多ピン機器は、ピン表をattrsで渡すだけの1回の呼び出しで配置できる。 <sub>`a_block_with_named_pins_is_placed_in_one_call`</sub>
- ピン表が壊れているブロックは、図面に空の箱が置かれるのではなくエラーとして返る。 <sub>`a_block_with_a_broken_pin_table_is_refused`</sub>
- 読み取り系のツールも窓口経由で動く(エージェントは編集の前に図面を見られる)。 <sub>`reading_tools_work_through_the_bridge`</sub>
- 存在しないツール名は、ターンを落とさずにエラーの結果として返る(モデルが言い直せる)。 <sub>`an_unknown_tool_name_comes_back_as_an_error_result`</sub>
- 引数が間違っているときも理由つきのエラーの結果として返り、モデルが自分で直せる。 <sub>`bad_arguments_come_back_with_a_reason`</sub>
- execute_commands の入力スキーマに run_cable の種類が載り、エージェントが区間のあるケーブルの書き方を知れる。 <sub>`execute_commands_schema_lists_the_run_cable_kind`</sub>

### tools

- 部品検索ツールの説明には選定・比較・代替品の用途が書かれており、「これの代替は?」と聞かれたエージェントがこれを使う。 <sub>`the_parts_search_tool_advertises_selection_and_comparison`</sub>
- テンプレートのツール説明には「作図の雛形であること」「ERC指摘ゼロで入ること」「undo一発で戻せること」が書かれている。 <sub>`the_template_tools_explain_what_a_template_is`</sub>
- 公開ツールには全て説明文が付いており、説明の無いツールをエージェントへ見せない。 <sub>`every_published_tool_has_a_description`</sub>
- 編集ツールの説明には多段端子台のジャンパの書き方 (段記号つき) が書かれているので、エージェントが拒否される数字だけの指定を送らずに済む。 <sub>`the_edit_tool_explains_the_multilevel_jumper_form`</sub>

### 電線品番・コマンドエラー (REST)

- エンジンが拒否したコマンドは、黙って200で返るのではなく、理由と何番目のコマンドが落ちたかを添えたHTTP 400で返る。 <sub>`a_refused_command_returns_400_with_the_reason_and_the_command_index`</sub>
- 極数の上限を超えた極数可変シンボルも同じように拒否され、使える範囲がメッセージに書かれる。 <sub>`a_pole_count_beyond_the_limit_is_refused_over_the_api`</sub>
- 正しいコマンド列はこれまでどおり200で、コマンドごとのpatchが返る。 <sub>`a_valid_command_list_still_succeeds`</sub>
- 一括実行(`?batch=true`)を指定すると、コマンド列全体が1回の編集になる。patchは1件返り、undo一発で全部が元に戻る。 <sub>`a_batch_command_list_is_one_undo_step`</sub>
- 一括実行の最後のコマンドが拒否されると何も適用されない。図面は手つかずで、返答は適用0件と伝える。 <sub>`a_refused_batch_applies_nothing`</sub>
- 電線品番表のCSVを投げるとプロジェクトの線色+線径の表が埋まり、品番を持たないワイヤでも電線リストに品番が出る。 <sub>`importing_a_wire_part_csv_fills_the_part_number_column_of_the_wire_list`</sub>
- 読めないCSVは400で拒否され、表は元のままになる。 <sub>`a_malformed_wire_part_csv_is_refused_and_changes_nothing`</sub>


## AIアシスタント (madake-agent)


### anthropic_api

- APIから届いた本文はそのまま文字の差分として流れ、ターンの終わりに組み立てた返答とトークン使用量が付く。 <sub>`streamed_text_becomes_deltas_and_a_completed_turn`</sub>
- APIへ送るリクエストには設定したモデル・ストリーミング指定・システムプロンプト・ユーザーの発言が載る。 <sub>`the_request_carries_the_model_system_prompt_and_user_message`</sub>
- ブリッジしたMCPツールは毎回のリクエストでAPIへ渡る(エージェントが図面を編集できる)。 <sub>`the_bridged_mcp_tools_are_offered_to_the_api`</sub>
- APIが送ってくる思考は、返答の本文とは別の思考イベントとして順に届く (空の差分と署名は飛ばす)。 <sub>`streamed_thinking_becomes_thinking_events`</sub>
- モデルが要求したツールはその場で実行され、結果を返して会話を続ける(1ターンで完結する)。完了イベントにはツールの結果が載る。 <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
- 失敗したツールはターンを中断せず、エラーとしてモデルへ返す(モデルが直せる)。 <sub>`a_failing_tool_is_reported_to_the_model_as_an_error_result`</sub>
- ツール実行の往復には上限があり、堂々巡りになったモデルが延々と動き続けない。 <sub>`the_tool_loop_stops_after_a_bounded_number_of_rounds`</sub>
- APIキーが弾かれたときは、生のHTTPコードではなく「キーの問題」と分かる文言を表示する。 <sub>`a_rejected_api_key_is_explained_as_a_key_problem`</sub>
- APIが混み合っているときは「混雑しているので時間をおいて」と読める文言を表示する。 <sub>`an_overloaded_api_is_explained_as_a_busy_service`</sub>
- ストリームの途中で届いたエラーもユーザーへ伝える(黙って途切れさせない)。 <sub>`an_error_event_inside_the_stream_is_surfaced`</sub>
- 会話の前のやりとりも一緒に送るので、モデルは前に話した内容を覚えている。 <sub>`earlier_turns_of_the_conversation_are_replayed`</sub>
- 接続テストは、使えるキーなら成功を、駄目なキーなら読める理由を返す。 <sub>`the_connection_test_reports_success_or_a_readable_reason`</sub>
- 発言に付けた画像とPDFは、画像ブロックと文書ブロックとして本文の前に載ってAPIへ届く。 <sub>`attachments_reach_the_api_as_image_and_document_blocks`</sub>

### Claude CLIバックエンド

- Claude CLIは必須フラグ(-p・stream-json出力・部分メッセージ・strict MCP設定)付きでヘッドレス起動される。 <sub>`args_contain_required_flags`</sub>
- 同梱ドキュメントのフォルダは読み取り可能なディレクトリとしてエージェントへ開かれ、マニュアルを引用できる。 <sub>`args_open_the_bundled_documentation_for_reading`</sub>
- セッション再開ID・モデル指定・追加システムプロンプトは、指定時にCLIへ引き渡される。 <sub>`args_include_resume_model_and_system_prompt_when_given`</sub>
- エージェントのMCP設定はMadakeCAD自身のローカルMCPサーバーを指し、他クライアントと同じツールを使う。 <sub>`mcp_config_points_at_local_mcp_server`</sub>
- エージェントのMCP設定にはこの起動のインスタンスidがヘッダで入るので、前の起動から残ったCLIはサーバーに断られる。idが無ければヘッダは送らない。 <sub>`mcp_config_carries_the_instance_header`</sub>
- ターンはCLIのstream-json出力から解釈したイベント(思考・テキスト差分・ツール実行・完了)を流す。 <sub>`send_streams_events_from_fake_cli`</sub>
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
- 添付があるとCLIをstream-json入力で起動し、画像・PDFのブロックを本文より前に並べたユーザーメッセージを1行で渡す。 <sub>`attachments_reach_the_cli_as_stream_json_content_blocks`</sub>
- 添付が無ければ引数は従来どおり (stream-json入力にしない) で、ふつうのターンは何も変わらない。 <sub>`plain_turns_keep_the_text_input`</sub>

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
- 応答は起きた順のブロックとして記録される。同じ種類が続く間は1つのブロックにまとまり、種類が変わると新しいブロックになる。本文とツールの一覧も今までどおり埋まる。 <sub>`replies_are_recorded_as_blocks_in_the_order_they_happened`</sub>
- 8 KBより長いツールの結果は先頭だけを残し、上限をまたぐ文字の手前で切る。省いた量を画面が示せるよう、全体の大きさも残す。 <sub>`a_long_tool_result_is_cut_on_a_character_boundary`</sub>
- idの無いツール完了 (Copilotの一部の行) でも、今までどおり同じツールの未完了の呼び出しに結果が付く。 <sub>`a_finish_without_an_id_puts_the_result_on_the_open_call_of_that_tool`</sub>
- ブロックとツールの結果が無かった頃に保存した会話ファイルも読み込め、メッセージのブロックは空になる。保存し直しても空のブロックや結果は書き出さない。 <sub>`an_old_chat_file_without_blocks_still_loads`</sub>
- このビルドが知らない種類のブロックを含む会話ファイルも読み込める。知らないブロックは「不明」として受け止め、残りの履歴はそのまま残る。 <sub>`a_chat_file_with_an_unknown_block_kind_still_loads`</sub>

### copilot_cli

- Copilot CLIは、人が見ていなくても働ける設定(JSONL出力・ツール自動許可・質問しない)で非対話起動される。 <sub>`args_contain_required_flags`</sub>
- 会話の最初のターンは新しいセッションIDを固定し、以降のターンが同じセッションを続けられるようにする。 <sub>`the_first_turn_pins_a_new_session_id`</sub>
- 2ターン目以降は新しいセッションを作らず同じCopilotセッションを再開するので、エージェントは会話を覚えている。 <sub>`a_follow_up_turn_resumes_the_same_session`</sub>
- 設定したCopilotのモデルはCLIへ渡される(`auto`ならCopilotが自動で選ぶ)。 <sub>`the_configured_model_is_passed_to_the_cli`</sub>
- CopilotにはMadakeCAD自身のMCPサーバーを渡すので、他のクライアントと同じコマンド経由で図面を編集する。 <sub>`the_mcp_config_points_at_the_local_madakecad_server`</sub>
- CopilotのMCP設定にもこの起動のインスタンスidがヘッダで入るので、前の起動から残ったCopilotは断られる。 <sub>`the_mcp_config_carries_the_instance_header`</sub>
- Copilot CLIにはシステムプロンプト用のフラグが無いため、MadakeCADの作図ルールは見出し付きでプロンプトの先頭へ前置される。 <sub>`the_system_prompt_is_prepended_to_the_user_prompt`</sub>
- ターンはCopilotのJSONLから解釈したイベント(セッション・本文・ツール実行・完了)を流す。 <sub>`a_turn_streams_events_from_the_fake_cli`</sub>
- MadakeCADが決めたセッションIDはCLIがそのまま返してくるので、通知は一度だけになる。 <sub>`the_pinned_session_id_is_announced_once`</sub>
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
- Copilotへは添付を読める場所 (--add-dir) のフォルダへ保存し、本文の後ろにパスを並べて渡す。フォルダはターンの後に消す。 <sub>`copilot_reads_attachments_from_a_temporary_allowed_folder`</sub>
- Copilot CLI向けにターンのフォルダへ保存するのは画像とPDFだけ。表計算は本文にテキストで入るので保存せず、保存するものが無ければフォルダも作らない。 <sub>`only_images_and_pdfs_are_saved_for_the_copilot_cli`</sub>

### copilot_events

- アシスタントの回答は差分(message_delta)で届き、書かれるそばからチャットに表示される。 <sub>`assistant_message_deltas_are_shown_as_they_arrive`</sub>
- モデルの思考(reasoning)は回答と同じ形で流れてくるが、チャットには表示しない。 <sub>`reasoning_deltas_are_not_shown_in_the_chat`</sub>
- 完成したアシスタントメッセージには、モデルが頼んだツール呼び出しと引数が入っている。 <sub>`a_completed_message_carries_the_requested_tool_calls`</sub>
- ツールの実行開始はツール名と引数を伝えるので、チャットには今なにをしているかが出る。 <sub>`a_tool_execution_start_reports_the_tool_and_its_arguments`</sub>
- ツールの実行完了は対応する呼び出しを閉じ、成功したかどうかと、CLIが出した結果の文字を伝える。 <sub>`a_tool_execution_complete_closes_the_call_with_its_outcome`</sub>
- 最終行はターンを終わらせ、そのターンが使ったCopilotのプレミアムリクエスト数を伝える。 <sub>`the_final_line_completes_the_turn_with_the_premium_requests_used`</sub>
- 最終行の終了コードが0でなければ、通常の回答ではなくエラーとしてチャットに出る。 <sub>`a_failed_final_line_becomes_an_error`</sub>
- CLIが実際に使ったセッションIDが伝わるので、次のターンは同じ会話を再開できる。 <sub>`the_session_line_reports_the_id_to_resume`</sub>
- 準備や記録のための行(MCPの状態・スキル一覧・ここまでの消費・ターンの区切り)はチャットに出ない。 <sub>`setup_and_bookkeeping_lines_are_ignored`</sub>
- 解釈できない行はターンを壊さず黙って無視される。 <sub>`unknown_and_broken_lines_are_ignored`</sub>
- ツール名は呼び出しから結果へ補完され、チャットには何が終わったのかが表示される。 <sub>`the_tool_name_is_carried_from_the_call_to_its_result`</sub>
- 同じツール呼び出しはメッセージと実行開始の両方に現れるが、チャットには一度だけ表示される。 <sub>`a_tool_call_announced_twice_is_shown_only_once`</sub>
- ターンの回答はアシスタントの最後のメッセージになる(最終行には回答が載らないため)。 <sub>`the_turn_result_comes_from_the_last_assistant_message`</sub>
- 差分では届かず完成メッセージにだけ載った本文も、取りこぼさずチャットに表示される。 <sub>`text_missing_from_the_deltas_is_still_delivered`</sub>
- 差分が一切来ない場合は、完成した回答がまるごと一度だけ届く。 <sub>`the_whole_answer_is_delivered_when_no_deltas_arrive`</sub>
- MadakeCADが決めたセッションIDが最終行で返ってきただけのときは、二重に通知しない。 <sub>`a_confirmed_session_id_is_not_announced_twice`</sub>
- CLIが別のセッションIDで答えた場合はそのIDが通知され、次のターンは正しい会話を再開する。 <sub>`a_different_session_id_from_the_cli_wins`</sub>
- アシスタントの本文は、CLIがどのフィールド名で出してもチャットへ届く。 <sub>`assistant_text_is_delivered_under_any_of_the_known_field_names`</sub>
- ツール呼び出しは別のフィールド名(tool/input、function/parameters)でも認識される。 <sub>`tool_calls_are_recognised_under_alternative_field_names`</sub>
- ツールの失敗は、知られているどの言い方でも失敗として認識される。 <sub>`a_failed_tool_result_is_recognised_under_any_wording`</sub>
- トークン使用量はOpenAI流のフィールド名(prompt/completion tokens)でも読み取れる。 <sub>`token_usage_is_also_read_from_openai_style_names`</sub>
- CLIのエラー行は、チャットにエラーとして表示される。 <sub>`an_error_line_becomes_an_error_event`</sub>
- エラー扱いのresult行は、通常の完了ではなくエラーになる。 <sub>`a_failed_result_line_becomes_an_error`</sub>

### gemini

- Geminiから届いた本文はそのまま文字の差分として流れ、ターンの終わりに組み立てた返答とトークン使用量が付く。 <sub>`streamed_text_becomes_deltas_and_a_completed_turn`</sub>
- Geminiが毎チャンクに載せてくる累計の使用量を二重に足し込まない。 <sub>`repeated_running_token_totals_are_not_counted_twice`</sub>
- リクエストは設定したモデルのストリーミング用エンドポイントへ行き、システム指示とユーザーの発言を載せる。 <sub>`the_request_goes_to_the_streaming_endpoint_with_the_system_instruction_and_message`</sub>
- ブリッジしたMCPツールはGeminiのfunctionDeclarations形式で渡る(エージェントが図面を編集できる)。 <sub>`the_bridged_mcp_tools_are_offered_as_function_declarations`</sub>
- Geminiが受け付けないスキーマの項目はツール定義から落とし、型が無くなったプロパティにも型を補う。 <sub>`schema_keywords_gemini_rejects_are_dropped_from_tool_definitions`</sub>
- Geminiには思考を求めないので、返答から思考イベントは出ない。 <sub>`gemini_replies_produce_no_thinking_events`</sub>
- モデルが要求したツールはその場で実行され、結果をfunctionResponseとして返して会話を続ける(1ターンで完結する)。完了イベントにはツールの結果が載る。 <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
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
- 発言に付けた画像とPDFは、inline_dataの部品として本文の部品の前に載ってGeminiへ届く。 <sub>`attachments_reach_gemini_as_inline_data_parts`</sub>

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
- システムプロンプトは、配線図はピン操作ツールでピン名を指定し、まとめて1回で描き、ピン座標を手で計算しないよう指示する。 <sub>`system_prompt_tells_the_agent_to_wire_by_pin_name`</sub>
- システムプロンプトは、描いた後にシート画像を見て確かめるよう指示する(数値だけで見た目を判断しない)。 <sub>`system_prompt_tells_the_agent_to_look_at_the_sheet`</sub>

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
- 添付つきの発言は会話に名前・種類・大きさだけを残し (中身は残さない)、ターンには添付が渡る。 <sub>`a_message_keeps_the_attachment_names_but_not_their_content`</sub>
- 添付が条件 (種類・大きさ・件数) から外れた発言は、会話に何も積まずに断る。 <sub>`invalid_attachments_are_refused_before_the_turn_starts`</sub>

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
- モデルがreasoning_contentかreasoningで送ってくる思考 (Ollama・DeepSeekなど) は、返答の本文とは別の思考イベントとして届く。空の差分は飛ばす。 <sub>`streamed_reasoning_becomes_thinking_events`</sub>
- 同じ思考をreasoning_contentとreasoningの両方に載せる差分 (ミラーするプロキシがある) は、思考イベントを2回ではなく1回だけ出す。 <sub>`mirrored_reasoning_fields_are_not_doubled`</sub>
- モデルが要求したツールはその場で実行され、結果をツールメッセージとして返して会話を続ける(1ターンで完結する)。完了イベントにはツールの結果が載る。 <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
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
- 添付した画像は、OpenAI互換サーバーへdata URLの部品として本文の前に載って届く。 <sub>`an_attached_image_reaches_the_server_as_a_data_url`</sub>
- OpenAI互換のモデルはPDFを読めないので、PDFを添付すると何も送らずに理由つきで断る。 <sub>`an_attached_pdf_is_refused_before_sending`</sub>

### stream-jsonパーサ

- CLIのsystem/init行はセッションIDを載せたセッション開始イベントになる。 <sub>`init_line_yields_session_started`</sub>
- テキスト差分はテキストイベント、thinking差分は思考イベントになり、それぞれ順につながる。signature差分は無視される。 <sub>`text_and_thinking_deltas_become_their_own_events`</sub>
- ツールの完了には、ツールが返した内容が載る。CLIが文字列で送っても、テキストブロックの配列で送っても同じ。 <sub>`a_tool_result_carries_what_the_tool_returned`</sub>
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
- 既定では「グローバル既定の図枠テンプレート」は未設定で、新しい図面は同梱のJIS A3図枠で始まる。 <sub>`default_frame_template_is_unset`</sub>
- 既定図枠の項目が無かった頃の設定ファイルもそのまま読め、「既定なし」として扱われる。 <sub>`old_settings_file_without_default_frame_template_loads_as_unset`</sub>
- 既定図枠のidは前後の空白が取り除かれ、空欄は空のidではなく「既定なし」になる。 <sub>`default_frame_template_is_trimmed_and_blank_means_unset`</sub>
- 選んだ既定の図枠テンプレートは、設定を保存して読み直しても残っている。 <sub>`default_frame_template_round_trips_through_save_and_load`</sub>
- 既定では起動時に新しい版を確認する。この項目が無かった頃の設定ファイルも「確認する」として読む。 <sub>`update_checks_are_on_by_default_and_for_old_settings_files`</sub>

### tool_bridge

- MCPツール1つがAPIのツール1つになり、名前・説明・入力スキーマがそのまま引き継がれる。 <sub>`every_mcp_tool_becomes_one_api_tool`</sub>
- JSON Schemaの`$schema`宣言は落とす(APIが必要とするのは形そのものだけ)。 <sub>`the_schema_marker_is_dropped_from_the_input_schema`</sub>
- スキーマの無いツールにも空のオブジェクトスキーマを与える(APIが受け付ける形にする)。 <sub>`a_tool_without_a_schema_gets_an_empty_object_schema`</sub>
- `"type": "object"`が抜けたスキーマは、そのまま送らずに補って直す。 <sub>`a_schema_missing_its_object_type_is_repaired`</sub>
- ブリッジしたツール名は全てAPIが許す文字種・長さに収まる(ツールが弾かれない)。 <sub>`bridged_tool_names_fit_the_api_name_rules`</sub>
- 説明が空のツールには説明の代わりを入れる(説明無しのツールをAPIへ渡さない)。 <sub>`a_tool_without_a_description_still_carries_some_text`</sub>

### attachments

- Excelのブックはテキストになる: シートごとに見出しを付けて順に並べ、1行1行・列はタブ区切り。空の行は省き、整数は「.0」を付けず、日付のセルは日付で書く。 <sub>`an_excel_workbook_becomes_tab_separated_text_per_sheet`</sub>
- 長すぎて全部は送れないブックは行の切れ目で切り、省いた行数を書き添える。 <sub>`a_long_workbook_is_cut_and_says_how_many_rows_were_left_out`</sub>
- ブックとして読めないファイルはファイル名を添えて断り、何も送らない。 <sub>`a_broken_workbook_is_refused_with_its_name`</sub>
- CSVは先頭のBOMを除いた本文をそのまま送る。UTF-8でないCSV (Shift_JISなど) は「UTF-8で保存し直す」案内を付けて断る。 <sub>`a_csv_is_sent_as_text_and_must_be_utf8`</sub>
- 表計算はどのプロバイダにも本文より前のテキストとして渡す: Anthropic・Claude Code CLIはテキストブロック、Gemini・OpenAI互換はテキスト部品 (PDFを読めないモデルでもExcelは読める)、Copilot CLIは本文に直接入れる。 <sub>`every_provider_gets_a_spreadsheet_as_text_before_the_message`</sub>
- 画像 (PNG/JPEG/GIF/WebP)・PDF・Excel (.xlsx)・CSVは添付でき、それ以外の種類・6件以上・20MBを超えるファイルは全体を断る。 <sub>`only_images_pdfs_and_spreadsheets_within_the_limits_are_accepted`</sub>
- 選んだファイルの種類は拡張子 (大文字小文字は問わない) で決まり、知らない拡張子は添付できない。 <sub>`the_kind_of_a_picked_file_comes_from_its_extension`</sub>
- Anthropic APIへは画像を画像ブロック、PDFを文書ブロックにして、本文より前に置く。 <sub>`anthropic_gets_image_and_document_blocks_before_the_text`</sub>
- Geminiへは添付をinline_dataの部品にして、本文の部品の前に置く。 <sub>`gemini_gets_inline_data_parts`</sub>
- OpenAI互換のモデルへは画像をdata URLで渡し、PDFは何も送る前に断る。 <sub>`openai_gets_data_url_images_and_refuses_pdfs`</sub>
- Claude Code CLIへは同じ内容ブロックを持つstream-jsonのユーザーメッセージを1行で渡す。 <sub>`claude_cli_gets_one_stream_json_line`</sub>
- CLI向けの保存はファイル名を保ち、区切り文字を無害にし、同名には番号を足す。保存先のパスは本文の後ろに並ぶ。 <sub>`saving_for_a_cli_keeps_safe_unique_names`</sub>
- 選んだファイルは拡張子の種類で読み込み、貼り付けのbase64は復号する。壊れたbase64は断る。 <sub>`files_and_pasted_content_become_attachments`</sub>
- 選んだファイルの下見は名前・種類・大きさを返し、添付できないとき (種類違い・ファイルが無い) はその理由を返す。 <sub>`previewing_a_file_reports_its_kind_size_and_problems`</sub>
- 画面から届く添付はファイルのパスから読むかbase64を復号し、1件でも読めなければ発言全体を断る。 <sub>`inputs_from_the_screen_resolve_from_a_path_or_base64`</sub>


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
- `madake export cable-list` はケーブル一覧の帳票を書き出す (既定はCSV)。 <sub>`export_cable_list_writes_the_cable_list_report`</sub>
- `madake export panel-wiring` は盤内配線図の帳票を書き出す。 <sub>`export_panel_wiring_writes_the_panel_wiring_report`</sub>
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
- madake export dxf / kicadはDXF・KiCadのエンドポイントでシート1枚を書き出し(シート指定可)、書き出した種別の件数と出せなかったものを表示する。 <sub>`export_dxf_and_kicad_forward_to_their_endpoints`</sub>
- madake openに.dxfを渡すと/import/dxfで読み込み(--wire-layerを引き渡す)、変換結果の要約を表示する。拡張子の大文字小文字はKiCadでも区別しない。他のファイルで--wire-layerを付けるとエラー。 <sub>`open_dxf_imports_with_wire_layers`</sub>
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
- madake terminalsは多段端子台を極数で数えて段数を併記するので、4極2段の端子台が8極の端子台に見えることはない。 <sub>`terminals_shows_a_multilevel_block_as_poles_and_levels`</sub>
- 端子台が無い場合は空の表ではなく分かりやすいメッセージを出す。 <sub>`terminals_handles_empty`</sub>
- openのメッセージには読み込んだパスと結果のrevisionが含まれる。 <sub>`opened_reports_path_and_revision`</sub>


## フロントエンド (エディタUI)


### agentOverlay

- place_symbolのx/yからシンボル概寸の領域を作る <sub>`toolBox`</sub>
- GitHub Copilotが付ける「サーバー名-」の接頭辞でも同じ領域が光る <sub>`toolBox`</sub>
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
- 表示中のシート以外の編集は拾わない(案ごとに別シートを整える整えバリアントで、他の案の編集が表示中のシートへ幽霊のように出るのを防ぐ) <sub>`patchOverlayTargets`</sub>
- 走っている会話が1本だけならその会話の色、2本以上あってどの会話の編集か分からなければ既定色 <sub>`patchOverlayTargets`</sub>
- 表示中のシートが整え案の複製なら、並列に何本走っていてもその案の会話色で描く(どの会話の編集かがシートで分かるため) <sub>`patchOverlayTargets`</sub>

### annotationText

- 注記文字高を設定していないシートは既定の2.0mmで、注記の大きさはこれまでと変わらない <sub>`annotationText`</sub>
- 注記文字高1.4mmのシートでは、注記の層がまとめて0.7倍の大きさになる <sub>`annotationText`</sub>
- 壊れた値 (0・負・数値でない) が入っていても既定に落ちるので、図面が消えることはない <sub>`annotationText`</sub>
- 設定できる注記文字高は1.0〜3.5mmで、その外はエンジンが受け付けない <sub>`annotationText`</sub>

### blockEditor

- 新しいブロックの下書きは左右に1本ずつピンがあり、番号は1と2で外形は自動 <sub>`block pin table draft`</sub>
- ピンを追加すると、いま使っている一番大きい番号の次の番号が振られる <sub>`block pin table draft`</sub>
- 番号が数字でないピンばかりのときは、ピンの本数+1を新しい番号にする <sub>`block pin table draft`</sub>
- ピンを削除すると、その行だけが表から消える <sub>`block pin table draft`</sub>
- ピンを上下に動かすと隣の行と入れ替わり、端の行はそれ以上動かない <sub>`block pin table draft`</sub>
- 配置に使う属性はピンのある辺だけを書き、ピン名が空の行は番号だけを書く <sub>`block attributes built from the draft`</sub>
- 番号もピン名も空の行は書き出さない(表の空行はピンにならない) <sub>`block attributes built from the draft`</sub>
- 外形を手動にしたときだけ、寸法が属性に書かれる <sub>`block attributes built from the draft`</sub>
- 配置済みブロックの属性を読み戻すと、同じピン表と外形の下書きに戻る <sub>`block attributes built from the draft`</sub>
- 読めない属性のブロックは空のピン表として開く(その場で作り直せる) <sub>`block attributes built from the draft`</sub>
- 外形が自動のときの寸法はピンの数で決まり、5mm刻みに丸められる <sub>`block attributes built from the draft`</sub>
- プレビューは実際に描かれるシンボル定義そのもの(ピンの数だけ接続点がある) <sub>`block attributes built from the draft`</sub>
- 正しく埋めたピン表には指摘が出ない <sub>`checks on the pin table`</sub>
- 同じピン番号が2つあると指摘される(番号は図面内で一意) <sub>`checks on the pin table`</sub>
- ピン名だけ書いて番号が空の行は指摘される <sub>`checks on the pin table`</sub>
- 手動の外形がピンの並びより小さいと指摘される(必要な寸法つき) <sub>`checks on the pin table`</sub>
- ピンが多すぎる(200本超)ブロックは指摘される <sub>`checks on the pin table`</sub>
- 配線が接続点に届いているピンは「配線済み」として拾われる <sub>`pins that are already wired`</sub>
- 回転して置いたブロックでも、回転後の接続点で配線済みかを見る <sub>`pins that are already wired`</sub>
- 配線が1本も届いていないブロックには配線済みピンが無い <sub>`pins that are already wired`</sub>
- 配線済みのピンを消す・番号を変えると、そのピンが「接続が外れる」として挙がる <sub>`pins that are already wired`</sub>
- 配線済みのピンをそのまま残す編集では警告は出ない <sub>`pins that are already wired`</sub>
- 部品挿入で「ブロック(汎用)」を選ぶと、左右1本ずつのピン表が開く <sub>`starting a new block from the insert dialog`</sub>
- 編集したピン表はそのまま配置に使う属性になる(配置後もこの形で描かれる) <sub>`starting a new block from the insert dialog`</sub>
- 「手動」に切り替えると、そのときの自動寸法が編集できる初期値として入る <sub>`starting a new block from the insert dialog`</sub>
- ピン番号が重なっているあいだは配置できない(指摘が出る) <sub>`starting a new block from the insert dialog`</sub>
- プロパティの「ピン表を編集…」は、そのブロックのピン表を読み込んで開く <sub>`editing a block that is already on the drawing`</sub>
- 適用するとupdate_entityコマンドが1回だけ実行され、参照記号・型番・位置は変わらない <sub>`editing a block that is already on the drawing`</sub>
- ブロック以外の属性(定格など)は編集しても残る <sub>`editing a block that is already on the drawing`</sub>
- 外形を自動へ戻して適用すると、手動寸法の属性が消える <sub>`editing a block that is already on the drawing`</sub>
- 何も変えずに適用したときはコマンドを送らない(履歴を汚さない) <sub>`editing a block that is already on the drawing`</sub>
- 閉じると下書きを捨てる(図面は変わらない) <sub>`editing a block that is already on the drawing`</sub>
- 配線済みのピンの番号を変えると「接続が外れる」警告が出る <sub>`warning about wired pins`</sub>
- 配線済みのピンを消しても同じ警告が出る <sub>`warning about wired pins`</sub>
- 配線されていないピンを消しても警告は出ない <sub>`warning about wired pins`</sub>
- 警告が出ていても適用はできる(承知のうえで直せる) <sub>`warning about wired pins`</sub>

### cable

- ケーブルの注記は参照記号・型番・芯数×線径・長さの順に並び、空の項は飛ばす (Rust側と同じ) <sub>`多芯ケーブル`</sub>
- シールドの楕円は定義線の中点を中心に、定義線より少し長く囲む <sub>`多芯ケーブル`</sub>
- 新しいケーブルの参照記号は、シート上のケーブルとハーネスのW番号の続きになる <sub>`多芯ケーブル`</sub>
- 定義線のドラッグは、芯数0 (横切った本数にする) のadd_cableコマンドになり、クリックしただけでは何も作らない <sub>`多芯ケーブル`</sub>
- 定義線の近くをクリックするとケーブルを掴める <sub>`多芯ケーブル`</sub>
- isCable は区間の有無にかかわらずケーブル (cable と run_cable) を見分け、cablePoints は定義線と区間の点を両方返す <sub>`多芯ケーブル`</sub>
- 芯の行き先がピンなら「参照記号.ピン」(部品が消えていれば「?.ピン」)、分岐の配線ならnull <sub>`多芯ケーブル`</sub>
- 区間ケーブルの印は区間のいちばん長い線分の中点を通る45度の斜線で、芯数の文字・注記・シールドの楕円もそこを基準にする (Rust側と同じ) <sub>`多芯ケーブル`</sub>
- 部品に着く端は着いたピンの先を結ぶ棒、分岐の端は区間の端から各芯の配線の端への扇になる <sub>`多芯ケーブル`</sub>
- 区間ケーブルは区間の線の近くをクリックすると掴める <sub>`多芯ケーブル`</sub>

### deviceTree

- 展開中のデバイスは、見出しの下に機能の行が続く <sub>`デバイスツリーの整形`</sub>
- 折りたたんだデバイスは見出しだけになり、他のデバイスの展開には影響しない <sub>`デバイスツリーの整形`</sub>
- デバイスの並びも機能の並びもRust側が決めた順のまま変えない <sub>`デバイスツリーの整形`</sub>
- 空のプロジェクトのツリーは1行も出ない <sub>`デバイスツリーの整形`</sub>
- 多段端子台は端子の数ではなく「何極何段」で呼ぶ <sub>`見出しの文言`</sub>
- リレーは種別と型番で呼ぶ(型番が無ければ種別だけ) <sub>`見出しの文言`</sub>
- 端子台は極数つきで呼ぶ <sub>`見出しの文言`</sub>
- その他の部品はシンボルの名前で呼び、UI言語に合わせて英語名と日本語名を使い分ける <sub>`見出しの文言`</sub>
- 機能の行の名前と所在バッジは「接点 13-14」「/2.B3」の形になる <sub>`見出しの文言`</sub>
- 機能の行をクリックすると、その機能のシートへ切り替えてエンティティを選ぶ <sub>`ツリーからの操作`</sub>
- デバイスの削除は、その参照記号の全機能のエンティティを重複なく消す <sub>`ツリーからの操作`</sub>
- 未配置の接点は、配置済みの機能の後ろに1行ずつ続く <sub>`未配置機能 (予約接点)`</sub>
- 折りたたんだデバイスは未配置行も出さない <sub>`未配置機能 (予約接点)`</sub>
- 絞り込み「未配置あり」は未配置機能のあるデバイスだけ、「配置済み」は未配置機能の無いデバイスだけを出す <sub>`未配置機能 (予約接点)`</sub>
- 未配置行の名前は接点の種別と端子番号で読める (例「接点 23-24 NO」) <sub>`未配置機能 (予約接点)`</sub>

### dynamicSymbol

- connector_2pは旧静的定義と同一のピン座標を持つ <sub>`dynamicSymbol`</sub>
- terminal_block_3pは3端子×左右2接続点で、中央揃え・2.5mmグリッド上にある <sub>`dynamicSymbol`</sub>
- PLC入力モジュールは点数ぶんの接続点が左側に5mmピッチで並ぶ縦長の箱になる <sub>`dynamicSymbol`</sub>
- PLCモジュールには入力用と出力用があり、名前で見分けられる <sub>`dynamicSymbol`</sub>
- 不正な動的IDはnullになる <sub>`dynamicSymbol`</sub>
- PLCモジュールのシンボルは1〜64点までで、その外の点数は存在しない <sub>`dynamicSymbol`</sub>
- PLCモジュールの分割片 plc_di_{n}p_s{start} は点番号を開始点から振り、形は同じ点数のモジュールと同じ (Rust側と一致) <sub>`dynamicSymbol`</sub>
- 多段端子台(4極2段)は段記号付きのピン番号(1a,1b,…4b)を持ち、1端子=左右2接続点になる <sub>`dynamicSymbol`</sub>
- 多段端子台は段ごとに1列の端子が横に並び、同じ極の端子(1aと1b)は同じ行に載る(Rust側と同じ座標) <sub>`dynamicSymbol`</sub>
- 多段端子台の接続点はすべて2.5mmグリッド上にあり、別の端子どうしが同じ点を共有しない (段数1は`_1l`という別名idを作らないので対象外) <sub>`dynamicSymbol`</sub>
- 段数が0や4(1〜3の範囲外)の端子台は、極数が範囲外のときと同様にnullになる <sub>`dynamicSymbol`</sub>
- `terminal_block_8p_1l`は`terminal_block_8p`の別名として受け付けずnullになる (Task 3。単段端子台のsymbol_idの綴りは1通りだけ、Rust側resolve_symbolと同じ規則) <sub>`dynamicSymbol`</sub>
- 単段の端子台id(terminal_block_{n}p)は多段対応の追加後も数字だけのピン番号のままで、既存プロジェクトとの互換を保つ <sub>`dynamicSymbol`</sub>
- 静的定義を優先し、無ければ動的生成にフォールバックする <sub>`resolveSymbolDef`</sub>
- ブロックのピンは箱の左右に5mmピッチ・2.5mmグリッド上で中央揃えに並ぶ(Rust側と同じ座標) <sub>`blockSymbol`</sub>
- ピン番号は外形の外側に、ピン名は内側に描かれる <sub>`blockSymbol`</sub>
- 外形はピン数とピン名の長さから自動で決まり、必ず5mm刻みになる <sub>`blockSymbol`</sub>
- 外形寸法を明示すると5mm刻みへ切り上げられる <sub>`blockSymbol`</sub>
- 番号の重複・空番号・小さすぎる寸法・壊れたピン表はエラーになる(黙って描かない) <sub>`blockSymbol`</sub>
- ピン表の無いブロックは最小サイズの空の箱になる <sub>`blockSymbol`</sub>
- 配置済みブロックの定義はインスタンスの属性から作られる(同じidでも中身が違う) <sub>`resolveInstanceDef`</sub>
- 属性が壊れているブロックは描かれない(undefinedになる) <sub>`resolveInstanceDef`</sub>
- ピングループ括弧はシンボルと一緒に描かれるので、参照記号・型番の位置決めに使う外形にはその箱も入る (Rust側 svg::symbol_bounds とparity) <sub>`symbolBounds`</sub>
- 端子番号を採番したあとでも、ピングループの箱は外形に入ったままになる (範囲は表示番号と突き合わせられるため。Rust側 pin_group_paper_boxes とparity) <sub>`symbolBounds`</sub>
- 実機ブロック表示のコイルは、ピングループの属性を持っていても括弧のための余白を取らない (そこには括弧を描かないため。Rust側 a_device_block_coil_reserves_no_room_for_brackets_it_never_draws とparity) <sub>`symbolBounds`</sub>
- 回転しても同じで、90度回すと括弧は上に出るので外形は上へ広がる (回転したシンボルで参照記号が括弧の文字に重ならない) <sub>`symbolBounds`</sub>
- ピングループの部品どうし (括弧と、折り返したキャプションの各行) は全ての回転・ミラーで決して重ならない (Rust側 svg_pin_group_pieces_never_overlap_each_other_at_any_rotation とparity) <sub>`captionLineAnchors`</sub>
- 文字はシンボルを回しても横書きのままなので、キャプションの行は用紙の上で必ず縦に積まれる (同じ中心線に並び、順に下へ下がる。Rust側 svg_pin_group_caption_lines_stack_downwards_on_paper とparity) <sub>`captionLineAnchors`</sub>

### frameEditor

- 図枠エディタは用紙座標で見せ、表題欄の左上角からの相対座標で保存する (A3の既定図枠なら表題欄は(290,255)) <sub>`frame editor geometry`</sub>
- 線ツールは表題欄の罫線を1本足し、元のテンプレートは変えない <sub>`frame editor geometry`</sub>
- 長さ0の線や幅0の矩形は何も足さない <sub>`frame editor geometry`</sub>
- 矩形ツールは対角の2点から4本の罫線を足す (どちら向きに引いても同じ) <sub>`frame editor geometry`</sub>
- Shiftの水平/垂直拘束は、変化の大きいほうの軸だけを残す <sub>`frame editor geometry`</sub>
- 文字ツールは固定文字のセル、プレースホルダツールはバインドのセルを足す <sub>`frame editor geometry`</sub>
- 表題欄セルツールは大きさ付きのバインドセルと外周の罫線4本を足す <sub>`frame editor geometry`</sub>
- ロゴ枠ツールは矩形をロゴ枠にし、もう一度引けば置き換わる <sub>`frame editor geometry`</sub>
- クリックは罫線の上なら罫線、セルの中ならセルを拾い、何も無い所ではnull <sub>`frame editor selection and editing`</sub>
- 移動は要素を(dx,dy)だけずらし、消去は要素を取り除く <sub>`frame editor selection and editing`</sub>
- セルに固定文字を入れるとバインドは消え、バインドを入れると固定文字は消える <sub>`frame editor selection and editing`</sub>
- プレースホルダは名前の部分一致で絞れ、空の検索語なら全部出る <sub>`frame editor placeholders and pre-save checks`</sub>
- バインド文字列から{名前}を出てくる順に取り出す <sub>`frame editor placeholders and pre-save checks`</sub>
- 同梱の図枠は保存前チェックがすべて0件 <sub>`frame editor placeholders and pre-save checks`</sub>
- 知らないプレースホルダと枠外の要素は数えるが、保存は止めない <sub>`frame editor placeholders and pre-save checks`</sub>
- 改訂欄の列幅の合計が表題欄の幅と違う・文字高が0のセルは保存できない誤りになる <sub>`frame editor placeholders and pre-save checks`</sub>
- 別名で保存の既定idは元のid+-copyで、使われていれば番号を足す。idは英数字・-・_だけ <sub>`frame editor placeholders and pre-save checks`</sub>
- 同梱の図枠を開くと、-copyのidを付けた新しい下書きになる (同梱はそのまま上書きできない) <sub>`frame editor store`</sub>
- エディタ内の編集は図面のコマンドを送らず、⌘Zで1段ずつ戻り⇧⌘Zでやり直せる <sub>`frame editor store`</sub>
- 何も変えない編集は段を積まず、新しい編集はやり直しの段を捨てる。段数には上限がある <sub>`frame editor store`</sub>
- ユーザー図枠の保存は同じidで書き、一覧を読み直して未保存の印を消す <sub>`frame editor store`</sub>
- まだファイルの無い下書きの保存は、書かずに別名で保存の欄を開く <sub>`frame editor store`</sub>
- 別名で保存は、ファイル名にできないidと既にあるidを断り、通れば新しいidと入力した名前で書く <sub>`frame editor store`</sub>
- 保存前チェックに誤りがあれば保存しない <sub>`frame editor store`</sub>
- 未保存の変更があるときに閉じると確認を出し、破棄を選べば閉じる <sub>`frame editor store`</sub>

### 図枠テンプレートの配置計算 (キャンバス)

- 既定の図枠テンプレートは現行のJIS図枠そのもので、マージン10mm・ゾーン4×6・右下の120×32mm表題欄を持つ <sub>`図枠テンプレート (jis-a3の既定値)`</sub>
- キャンバスが持つ既定の図枠は、同梱のjis-a3.jsonと一字一句同じ内容である(印刷とキャンバスで図枠がずれない) <sub>`図枠テンプレート (jis-a3の既定値)`</sub>
- ゾーンの目盛文字は数字なら1,2,3…、英字ならA,B,C…でZの次はAへ戻る <sub>`図枠テンプレート (jis-a3の既定値)`</sub>
- 説明書きのバインドはすべて値になる (プロジェクト名・シート番号/総数・シート名・最新改訂・表題欄の全欄) <sub>`表題欄のバインド解決`</sub>
- {date} {scale} {designer} {approver} は表題欄の日付・尺度・設計・承認の別名として同じ値になる <sub>`表題欄のバインド解決`</sub>
- 知らないバインドはエラーにならず空文字になる (テンプレートに書き間違いがあっても図面は出せる) <sub>`表題欄のバインド解決`</sub>
- バインドの無い文字はそのまま出て、閉じ括弧の無い「{」は行の残りを飲み込まずそのまま残る <sub>`表題欄のバインド解決`</sub>
- {rev.latest}は保存されたRev欄ではなく最新の改訂行に追従する <sub>`表題欄のバインド解決`</sub>
- シート単体では「1枚中1枚目・プロジェクト名は空」として解決される <sub>`表題欄のバインド解決`</sub>
- 固定文字のセルは中括弧も含めてそのまま出し、バインドのセルだけが値へ置き換わる <sub>`表題欄のバインド解決`</sub>
- 既定の図枠をA3横へ当てると、外枠は10mmマージンの400×277、表題欄は右下(290,255)の120×32mmになる <sub>`frameLayout`</sub>
- ゾーンの分割数はシートの設定に従い、目盛文字の種類はテンプレートが決める(図枠を替えても図面の住所は変わらない) <sub>`frameLayout`</sub>
- 表題欄の罫線とセルは用紙座標へ移され、セルには解決済みの文字が入る <sub>`frameLayout`</sub>
- 会社名のセルは表題欄の左外に右揃えで置かれ、会社名が空のシートでは描かれない <sub>`frameLayout`</sub>
- A4縦の図枠へ差し替えると表題欄の幅が110mmへ縮み、改訂欄の列も同じ幅に収まる <sub>`frameLayout`</sub>
- ロゴ枠を持つ図枠テンプレートでは、ロゴの位置がそのまま配置に出る(画像が無くても場所が決まる) <sub>`frameLayout`</sub>
- 改訂欄は表題欄の真上に同じ右端・同じ幅で置かれ、行高は表題欄と同じ8mmになる <sub>`revisionLayout (図枠テンプレート版)`</sub>
- 改訂欄の最大行数はテンプレートが決め、上限を超えた分は古い行から省かれる <sub>`revisionLayout (図枠テンプレート版)`</sub>
- 表題欄の中身が空のプロジェクトでは、どのバインドがそこへ入るかを`{...}`のまま重ねて見せる <sub>`bindOverlays (図枠プレビューの青いプレースホルダ)`</sub>
- 実際の値が入っている欄にはプレースホルダを重ねない(図面の値が読めなくならない) <sub>`bindOverlays (図枠プレビューの青いプレースホルダ)`</sub>
- 固定文字のセル(「品名」などの見出し)はバインドではないのでプレースホルダを出さない <sub>`bindOverlays (図枠プレビューの青いプレースホルダ)`</sub>
- 空欄なら描画されない会社名のセルも、プレビューではどのバインドが入るかを見せる <sub>`bindOverlays (図枠プレビューの青いプレースホルダ)`</sub>
- 図枠テンプレートの名前は日本語UIでは日本語名、それ以外では英語名で出す <sub>`frameTemplateName (UI言語ごとの図枠名)`</sub>
- 日本語名を持たないユーザー図枠は、日本語UIでも英語名をそのまま出す <sub>`frameTemplateName (UI言語ごとの図枠名)`</sub>
- 情報行は用紙・ゾーン分割・表題欄の大きさ・改訂欄の有無をまとめて示す <sub>`frameTemplateSummary (一覧・情報行の要約)`</sub>
- 表題欄が使っているバインドを、出てくる順に重複なく並べる(1つのセルに複数書いてもよい) <sub>`bindNames (フィールドバインド表)`</sub>
- バインドを1つも使わない図枠では表は空になる(見出しだけの図枠も作れる) <sub>`bindNames (フィールドバインド表)`</sub>

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

### leader

- 対象の真上にある注記から引く引き出し線は、注記の下辺の中点から始まり、対象の上辺のいちばん近い点で終わる (Rust側 a_leader_above_its_target_runs_from_the_bottom_edge_to_the_top とparity) <sub>`leaderLine`</sub>
- 対象の真横にある注記から引く引き出し線は、注記の近いほうの縦辺から対象の外接矩形まで水平に伸びる (Rust側 a_leader_beside_its_target_runs_horizontally とparity) <sub>`leaderLine`</sub>
- leader_toが未設定の普通の注記はnull (引き出し線を描かない) <sub>`leaderFor`</sub>
- 対象がシート上に実在すれば線が引け、注記を動かすと引き出し線も追従して変わる <sub>`leaderFor`</sub>
- 対象のエンティティidが存在しない・削除されていればnull (パニックせず何も描かない) <sub>`leaderFor`</sub>
- シンボルの外形はその定義から求まるので、引き出し線はシンボルも対象にできる <sub>`entityBounds`</sub>
- シンボルを指す引き出し線は図面に描かれている外形ちょうどで止まる: 配置基準点は必ず外形の内側に入り、シンボルの中の文字は文字の箱ではなく基準点1点として数える (Rust側 a_leader_targets_exactly_the_outline_the_drawing_shows とparity) <sub>`entityBounds`</sub>
- ピングループ括弧の付いたシンボルを指す引き出し線は、シンボル本体ではなく括弧の脇のキャプションの外側で止まる (括弧もキャプションもそのシンボルと一緒に描かれるため。Rust側 a_leader_stops_outside_a_symbols_pin_group_caption とparity) <sub>`entityBounds`</sub>
- 図形・ハーネス・配線・ジャンクションも対象にでき、盤レイアウト専用のエンティティはnull <sub>`entityBounds`</sub>

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
- 電線品番凡例ブロックの置き場所も、他のエンティティと同じ変換で移動する <sub>`placeMacroEntities`</sub>
- 配置は複製に対して行われ、元のマクロ定義は書き換えない(何度でも同じ形で置ける) <sub>`placeMacroEntities`</sub>
- プレビューは選んだバリアントのエンティティだけを載せた仮のシートを描く <sub>`macroSheet`</sub>
- マクロ名はUI言語が日本語なら日本語名、それ以外は英語名を出す <sub>`macroName`</sub>
- 日本語名が空のマクロはどちらの言語でも英語名を出す(空欄にしない) <sub>`macroName`</sub>

### mate

- connector_で始まるsymbol_idだけが嵌合対の対象で、端子台は除外される <sub>`mate: connector family gate`</sub>
- 宣言した嵌合相手の参照記号がそのまま読める <sub>`mate: declaredMate`</sub>
- 宣言が無い参照記号は空文字を返す <sub>`mate: declaredMate`</sub>
- 同一シート上で50mm以内にある嵌合対は、描くべきペアがちょうど1組できる <sub>`mate: mateLinks`</sub>
- 50mmを超えて離れた嵌合対は描くべきペアに入らない (帳票だけが伝える) <sub>`mate: mateLinks`</sub>
- 距離の上限は50mmちょうどを含む (50.0mmは描き、50mmを超えると描かない。Rust側 the_mate_link_distance_limit_includes_exactly_fifty_millimetres とparity) <sub>`mate: mateLinks`</sub>
- 宣言していないコネクタはペアに入らない <sub>`mate: mateLinks`</sub>
- 矩形の外側にある点へは、その点へ向かってクランプした周上の点を返す <sub>`mate: rectBoundaryPointToward`</sub>
- 矩形の内側にある点は、いちばん近い辺の上へ押し出される (Rust側 a_point_inside_the_outline_is_pushed_out_to_the_nearest_edge とparity) <sub>`mate: rectBoundaryPointToward`</sub>
- 30mm離れた3極コネクタ2つの嵌合線は、双方の外形の向かい合う辺の上、配置基準点の高さを結ぶ (Rust側 the_mate_link_runs_between_the_facing_outline_edges と同じ配置・同じ座標) <sub>`mate: mateLinkSegment (SVG/canvas parity)`</sub>

### netTag

- タグ「右」のネットラベルは、置いた点から右へ先端が飛び出した五角形の旗として描かれる <sub>`netTag`</sub>
- タグ「左」のネットラベルは、先端が左へ飛び出した左右反転した旗として描かれる <sub>`netTag`</sub>
- 旗の本体はラベルを置いた点を覆わず先端と反対側へ伸びるので、載っている配線が隠れない <sub>`netTag`</sub>
- ラベルを置いた点は先端の付け根にあたるので、左向きの旗と右向きの旗はちょうど鏡像になる <sub>`netTag`</sub>
- タグの文字は必ず旗の内側に収まり、旗の幅はネット名の長さに合わせて伸びる <sub>`netTag`</sub>
- 左向きと右向きのタグは同じ大きさになるので、左右のラベルが並んでも高さと幅がそろう <sub>`netTag`</sub>
- シート間クロスリファレンスは旗の本体側の外へ逃がされるので、先端や配線の上には出ない <sub>`netTag`</sub>
- 注記文字高を小さくすると、文字・余白・先端がまとめて縮み、旗の形はそのままで小さくなる <sub>`netTag`</sub>
- タグ属性の無いネットラベルは、これまでどおり文字だけの見た目のままになる <sub>`netTag`</sub>
- 知らないタグの値(打ち間違いや "up")は無視され、文字だけの表示に戻る <sub>`netTag`</sub>

### panel

- フットプリントは自分の寸法どおりの矩形を占める。 <sub>`panel layout geometry`</sub>
- 90度回したフットプリントは幅と高さが入れ替わる。 <sub>`panel layout geometry`</sub>
- 部品DBに寸法が無い型番でも既定の矩形で置ける。 <sub>`panel layout geometry`</sub>
- 盤外形の内側には一回り小さい取付板がある。 <sub>`panel layout geometry`</sub>
- インセットより小さい盤外形には取付板が無い。 <sub>`panel layout geometry`</sub>
- DINレールは取付線を中心とした横帯として描かれる。 <sub>`panel layout geometry`</sub>
- 右から左へ引いたレールも同じ範囲を覆う。 <sub>`panel layout geometry`</sub>
- ダクトは45度の斜線で埋まり、斜線はダクトの内側に収まる。 <sub>`panel layout geometry`</sub>
- 幅や高さの無いダクトには描く斜線が無い。 <sub>`panel layout geometry`</sub>
- レールの近くへ置いた部品は、レールの取付線に吸い付く。 <sub>`panel layout geometry`</sub>
- アイテム番号は読み順(上の段から先に、段の中は左から右へ)に並ぶ。 <sub>`panel item numbers and legend`</sub>
- 同じレールに乗った部品は、外形の高さが違っても同じ段として左から右へ数える。 <sub>`panel item numbers and legend`</sub>
- アイテム番号がまだ無いフットプリントにはバルーンが出ない。 <sub>`panel item numbers and legend`</sub>
- バルーンは左上角のすぐ外の丸で、短い引出線で角と結ばれる。 <sub>`panel item numbers and legend`</sub>
- 凡例表は採番済みの部品を番号順に並べ、未採番の部品は番号欄を空けて末尾に置く。 <sub>`panel item numbers and legend`</sub>
- 凡例表は行が増えるぶんだけ下へ伸び、幅は列幅の合計になる。 <sub>`panel item numbers and legend`</sub>
- レールは盤の上から順に番号を振るので、配置表とプロパティが同じレールを指す。 <sub>`panel item numbers and legend`</sub>
- 未配置部品の一覧はRust側 (回路図と盤シートの突き合わせ) から読み込む。 <sub>`unplaced parts store`</sub>
- 図面が変わっていなければ一覧を読み直さない (置くたびの往復を減らす)。 <sub>`unplaced parts store`</sub>
- 部品を盤へ置いて図面が変わると、次の読み込みで一覧から消える。 <sub>`unplaced parts store`</sub>
- 一覧が読めなくても画面は開いたまま (理由を控えて空一覧にする)。 <sub>`unplaced parts store`</sub>
- 配置する部品を選ぶと、その行だけが選択表示になる。 <sub>`unplaced parts store`</sub>

### panelMarks

- 部品に結び付いた銘板は部品の下に中央揃えで付き、指定すれば上に付く。単独の銘板は置いた位置のまま (Rust側と同じ) <sub>`盤の銘板`</sub>
- 説明に追従する銘板は1行目が回路図の説明になる (説明は回路図シンボルの機能ラベルから集める) <sub>`盤の銘板`</sub>
- 2つの部品の間の寸法は中心間の水平距離を測り、線は指定の高さに引く <sub>`盤の寸法線`</sub>
- 寸法ツールは2点の距離が大きい向きの寸法を、端点の外側 (水平なら上) 10mmに置き、同じ点なら何も作らない <sub>`盤の寸法線`</sub>
- クリックした位置の部品を見つけ、寸法線は線の近く、銘板は外形の中で掴める <sub>`盤の寸法線`</sub>

### panelTools

- 盤枠ツールの矩形ドラッグで、盤の外形を1つ足すコマンドを送る (undo一発で消える)。 <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- 逆向き(右下から左上へ)にドラッグしても同じ盤ができる。 <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- 盤の名前は自動で採番する (CP-1, CP-2 …)。 <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- クリックしただけ(つぶれた矩形)ではコマンドを送らない。 <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- DINレールは水平ドラッグで作る。取付線の高さは押し始めた点で決まる。 <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- ダクトツールの矩形ドラッグで配線ダクトを1つ足す。 <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- 未配置部品の行には参照記号・型番・外形寸法が並ぶ。 <sub>`unplaced parts panel`</sub>
- 部品DBに寸法が無い型番は既定の矩形で置ける (行には「寸法から自動」と出る)。 <sub>`unplaced parts panel`</sub>
- 型番が入っていない部品は「未登録」として注意付きで並ぶ。 <sub>`unplaced parts panel`</sub>
- フットプリントはカーソルを中心に置かれる (外形の左上ではなく真ん中を持つ)。 <sub>`footprint placement and rail snapping`</sub>
- 90度回すと幅と高さが入れ替わり、置かれる位置も回した外形の中心になる。 <sub>`footprint placement and rail snapping`</sub>
- DINレールの近くへ落とすと、部品の中心がレールの取付線へ吸い付く。 <sub>`footprint placement and rail snapping`</sub>
- レールから離れた場所ではカーソルの位置のまま置かれる (勝手に吸い付かない)。 <sub>`footprint placement and rail snapping`</sub>
- 未配置部品を置くと、参照記号と型番を持つフットプリントが1つ足される。 <sub>`footprint placement and rail snapping`</sub>
- 置いたフットプリントをレールの近くへ動かすと、移動量がレールへ吸い付くように直る。 <sub>`footprint placement and rail snapping`</sub>
- フットプリントが乗っているレールが分かる (プロパティの「所属レール」)。 <sub>`footprint placement and rail snapping`</sub>
- プロパティの編集は update_entity コマンドになる (モデルを直接書き換えない)。 <sub>`panel entity properties`</sub>
- 何も変わっていなければコマンドを送らない (無駄なundo段を作らない)。 <sub>`panel entity properties`</sub>
- 幅・高さに0や文字を入れても図面は壊れない (その欄は変えない)。 <sub>`panel entity properties`</sub>
- 回転は0度と90度だけ (v1の描画がこの2つしか持たない)。 <sub>`panel entity properties`</sub>
- 盤枠は名前と寸法を、レールは取付線と長さを編集できる。 <sub>`panel entity properties`</sub>
- 回路図の要素(配線など)はこの編集の対象外。 <sub>`panel entity properties`</sub>
- 盤レイアウトシートでは回路図のツール(配線・部品挿入など)を使えない。 <sub>`tools allowed per sheet kind`</sub>
- 回路図シートでは盤のツール(盤枠・レール・ダクト)を使えない。 <sub>`tools allowed per sheet kind`</sub>
- シートが1枚も開いていなければどちらのツールも使えない。 <sub>`tools allowed per sheet kind`</sub>
- バルーン採番は renumber_panel_items コマンド1回 (undo一発で採番前へ戻せる)。 <sub>`balloon numbering and the legend table`</sub>
- 置く部品が1つも無い盤では採番のコマンドを送らない。 <sub>`balloon numbering and the legend table`</sub>
- 凡例表の挿入は add_entity コマンド1つで、決めるのは置き場所だけ。 <sub>`balloon numbering and the legend table`</sub>

### pinGroups

- 「範囲:名前」1項目と、ダッシュの無い単ピン指定は、それぞれ1つのグループとして解析される (Rust側とparity) <sub>`parsePinGroups`</sub>
- チャート順で終点が始点より前に書かれた範囲は不正として報告される <sub>`parsePinGroups`</sub>
- そのインスタンスに無いピンを指す範囲は不明ピンの診断コードで報告される <sub>`parsePinGroups`</sub>
- コロンが無い・範囲が空の読めない記述はパニックせず不正として報告され、残りの項目は解析される <sub>`parsePinGroups`</sub>
- チャート順の範囲が重なる2つのグループはどちらも報告されグループにならないが、重ならない3つ目は残る <sub>`parsePinGroups`</sub>
- 多段端子台のチャート順は段ごとに並ぶ (1a,2a,…,1b,2b,…) ので、"1a-4a"は1段の中で連続する <sub>`parsePinGroups`</sub>
- ピン番号そのものに「-」が入っている端子はピングループに指定できず、使えない理由がそのまま報告される (Rust側とparity) <sub>`parsePinGroups`</sub>
- キャプションの折り返しは4文字ごとに順番どおり分割する <sub>`wrapCaption`</sub>
- シンボル左側のピンにかける括弧は、いちばん上から下のピンまで縦棒でつながり、キャプションはさらに左に置かれる <sub>`pinGroupBracket`</sub>
- ブロックでも括弧は左のピンの足先より外に出て、キャプションはさらに外に置かれる (Rust側の3一族テストとparity) <sub>`pinGroupBracket`</sub>
- コネクタは右側にピンが出るので、括弧とキャプションは足先より右に出る (Rust側の3一族テストとparity) <sub>`pinGroupBracket`</sub>
- 多段端子台で2段目の端子にかけたグループの括弧は、その段の列の外側に出る (列と列の間には入らない、Rust側とparity) <sub>`pinGroupBracket`</sub>
- 多段端子台の段をまたぐ範囲は、黙って何も描かれるのではなく報告される (段ごとに外側の辺が逆で1本の括弧では囲めないため、Rust側とparity) <sub>`pinGroupBracket`</sub>
- 端子番号が上書きされた端子台では、表示ラベルで書かれた範囲がピンを見つけて括弧を描き、古い生のピン番号は「存在しないピン」になる (Rust側とparity) <sub>`pinGroupBracket`</sub>
- 端子番号を上書きした多段端子台でも、1段ぶんの範囲はその段の列の外側に括弧を掛ける <sub>`pinGroupBracket`</sub>
- シンボルの2つの辺にまたがるピン (向きが揃わない) への括弧は拒否される <sub>`pinGroupBracket`</sub>

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
- c接点(切換接点)も、a接点・b接点と同じようにそのリレーの接点として数えられる <sub>`relayXref`</sub>
- c接点の端子は「11-12-14」のように、共通・b側・a側の3つを並べて書かれる <sub>`relayXref`</sub>
- 「2CO」のリレーに接点を1つだけ描いたときは、残り1極がc接点の未使用行として並ぶ <sub>`relayXref`</sub>
- c接点の極はa接点・b接点としても描けるので、「2CO」のリレーはa接点2個の使い方も受け付ける <sub>`relayXref`</sub>
- 接点マップの表はコイルの真下に中央揃えで置かれ、接点1個につき1行ずつ下へ伸びる。 <sub>`relayXref`</sub>
- 接点マップはコイルの外形の下にぶら下がり、コイル所在の文字は接点シンボルの右脇に置かれる。 <sub>`relayXref`</sub>
- 切換接点のコイル所在は、上の配線に付く注記と重ならないよう逃げる (G22の衝突の再現)。 <sub>`relayXref`</sub>
- 上も塞がっているときは、コイル所在は外形の下へ逃げる。 <sub>`relayXref`</sub>
- 接点自身に型番が書かれているときは、コイル所在はその文字の上へは逃げず、さらに先の外形の反対側へ回る (G22)。 <sub>`relayXref`</sub>
- 逃がしの判定はそのシートが実際に描く文字高で配線注記を測るので、注記文字を大きくしたシートでもコイル所在はきちんと逃げる (G20とG22) <sub>`relayXref`</sub>
- コイルはattrs.device_blockがちょうど"true"のときだけ実機ブロック表示になり、それ以外(空文字含む)は従来どおりの分散配置になる。 <sub>`relayXref`</sub>
- シートの実機ブロック一覧に入るのはdevice_blockが付いたコイルだけで、従来どおりの分散配置のコイルは入らない。 <sub>`relayXref`</sub>
- 実機ブロックで表示するコイルには、コイルの下にぶら下げる別表としての接点マップはもう付かない。 <sub>`relayXref`</sub>
- 実機ブロックの外形はコイルのピン間隔を下回らないので、幅が最小のときはA1/A2ピンがそのまま外形の左右端になり、引き出し線は要らない。 <sub>`relayXref`</sub>
- 型番があるとヘッダは2行になり、内容で外形がピン間隔より広がっても引き出し線でピンは外形まで届く。 <sub>`relayXref`</sub>
- 行の仕切り線はコイルの引き出し線が付く高さには来ないので、2本の引き出し線が1本の線で繋がって見えることはない <sub>`relayXref`</sub>
- 実機ブロックの接点の行はヘッダ行のすぐ下に続き、外形全体の高さは隙間なく全ての行をちょうど覆う。 <sub>`relayXref`</sub>

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
- ズームアウトしても文字だけ大きいまま残らない。図形と同じ割合で縮み、読めない大きさになったら描かない <sub>`drawSymbol text placement`</sub>
- シンボルの中の文字は、シンボルを回しても反転しても定義どおりの場所に描かれる <sub>`drawSymbol text placement`</sub>
- 1〜2文字の端子番号は、どの向きに回しても反転しても自分の印(丸)の中に丸ごと収まる <sub>`drawSymbol text placement`</sub>
- 画面のキャンバスでは、回していない端子台の番号は印の中心から文字高の約1/3(1/2.8)だけ下にベースラインが来る(出力図面の1/2より小さい) <sub>`drawSymbol text placement`</sub>
- ジャンパバーは太い線だけで描かれ、両端に丸(円)は描かれない <sub>`drawSymbol text placement`</sub>
- attrs["terminal_numbers"]の上書きは、シンボル定義の生のピン番号ではなく上書きラベルを描く (Rust側 svg_draws_the_overridden_terminal_number_instead_of_the_raw_pin_number とparity) <sub>`drawSymbol text placement`</sub>
- 採番後のジャンパバーは、単なる幾何的な近さではなく実際に繋ぐ端子の印刷ラベルへ紐付けて帰属確認できる (Rust側 a_jumper_bar_is_attributed_to_its_terminals_by_label_text_after_renumbering とparity。フェーズ6の「最寄りマーク帰属のテストが端子番号の入れ替わりを検出しない」を閉じる) <sub>`drawSymbol text placement`</sub>
- 実機ブロックをどの向きに回しても反転しても、引き出し線は必ず実際のA1/A2ピンの点から出るので、そこに繋いだ配線は繋がったまま (G7) <sub>`drawDeviceBlock`</sub>
- 実機ブロックの表はどの回転でも正立のままで、外形の位置も大きさも変わらない (回るのはピンだけ) <sub>`drawDeviceBlock`</sub>
- 外形がどれだけ横に広がっても、回していないコイルの引き出し線は水平のまま、配線が来るピンの高さで外形の左右の辺に届く <sub>`drawDeviceBlock`</sub>
- 90度・270度に回したコイルの引き出し線は真上・真下へ伸びて外形の上端・下端で止まり、接点が何行あっても接点イメージの文字を横切らない <sub>`drawDeviceBlock`</sub>
- 実機ブロックの文字はどの回転でも自分のセルの中に収まり、ほかの行と重ならない <sub>`drawDeviceBlock`</sub>
- 選択中の実機ブロックは、ほかのシンボルと同じように選択色の外形で描かれる <sub>`drawDeviceBlock`</sub>
- 実機ブロックのコイルにも機能ラベルは外形の下に描かれる (G4) <sub>`drawDeviceBlock`</sub>
- 表示クラス「参照記号」を隠すと、実機ブロックの接点イメージ(端子対と所在)も消える(外形とヘッダは残る) <sub>`drawDeviceBlock`</sub>
- 嵌合対の破線は表示クラス「シンボル」に属し、シンボルを隠すと一緒に消える (結ぶ相手が見えないため) <sub>`renderSheet mate links`</sub>
- 参照記号を隠しても嵌合対の破線は残る (参照記号の表示クラスには属さない) <sub>`renderSheet mate links`</sub>
- ジャンパバー(G5)は他のシンボル付随描画(嵌合対の破線など)と同じ表示クラス「シンボル」に属し、シンボルを隠すと一緒に消える (フェーズ6積み残しの解消) <sub>`jumper bar view-class gating (Task 3, フェーズ6積み残し)`</sub>
- 参照記号を隠してもジャンパバーは残る (参照記号の表示クラスには属さない) <sub>`jumper bar view-class gating (Task 3, フェーズ6積み残し)`</sub>
- 注記の引き出し線の対象端に描く丸印は、配線の分岐点の丸印とぴったり同じ大きさで描かれる (図面上で同じ印に見える。Rust側 svg_leader_dot_is_the_size_of_a_wire_junction_dot とparity) <sub>`renderSheet dots`</sub>
- 凡例ブロックは列見出しと、シートの配線から導出した行 (色・線径・品番・使用長) を表として描く <sub>`wire legend block drawing`</sub>
- 選択中の凡例ブロックは、外形の矩形を選択色で描く <sub>`wire legend block drawing`</sub>
- hide_reference="true" のシンボルは参照記号を画面に描かない (嵌合の組の片方が同じ記号を二重に描かない。svg.rsと同ルール) <sub>`hidden designator`</sub>

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

### shapes

- 新しい図形は既定色・細線・塗りなし・実線で始まる(何も設定せずに引いた線が図枠と同じ見た目になる) <sub>`shape geometry`</sub>
- 矩形は対角の2点で、どの向きにドラッグしても同じ矩形になる <sub>`shape geometry`</sub>
- 円は中心と円周上の点で表すので、半径はその2点の距離になる <sub>`shape geometry`</sub>
- 折れ線は描いた点をそのまま順に持ち、外接矩形は全ての点を含む <sub>`shape geometry`</sub>
- 矩形・線・円は2点ちょうど、折れ線は何点でもよい(Rust側と同じ規則) <sub>`shape geometry`</sub>
- 線幅は0.1〜2.0mmの範囲だけを受け付ける(Commandエンジンと同じ範囲) <sub>`shape geometry`</sub>
- 図形は色トークンの画面色で描かれ、選択中は選択色が優先する <sub>`shape colours on screen`</sub>
- 塗りは下の配線・記号が透けて読める薄さで塗り、塗り無しは塗らない <sub>`shape colours on screen`</sub>
- 破線の刻みはハーネス境界と同じ3mm/2mm <sub>`shape colours on screen`</sub>
- 矩形は輪郭の線で掴む(塗りの下にある配線を選べるように、面の中では掴まない) <sub>`shape hit testing`</sub>
- 円は円周の上で掴む(中心も外側も掴まない) <sub>`shape hit testing`</sub>
- 線と折れ線はどの区間の上でも掴める <sub>`shape hit testing`</sub>
- 作図の確定は add_entity コマンド1本になる(undo一発で消える) <sub>`shape commands`</sub>
- クリックしただけのつぶれた図形はコマンドを送らない(誤操作で点だけの図形が生まれない) <sub>`shape commands`</sub>
- 折れ線をダブルクリックで確定したときに重なる最後の点は1つにまとめられる <sub>`shape commands`</sub>
- 見た目の編集は update_entity コマンドになり、何も変わっていなければ送らない <sub>`shape commands`</sub>
- 範囲外の線幅は送らない(エンジンも同じ範囲で拒否する) <sub>`shape commands`</sub>
- 図形以外のエンティティにはコマンドを作らない <sub>`shape commands`</sub>
- シートの図形を数えると図形だけが返る(ハーネス境界は混ざらない) <sub>`shape commands`</sub>
- 作図ツールのidは4種の図形にひとつずつ対応する <sub>`shape commands`</sub>
- 図形は電線リストのハーネス列を汚さない(筐体をハーネスで囲まなくてよくなる理由) <sub>`shapes and the reports`</sub>

### sheets

- シートを追加するときに種別を選べる。盤レイアウトを選ぶと盤シートが1枚増える。 <sub>`sheet kind (schematic / panel)`</sub>
- 種別に回路図を選ぶと、これまでどおりの回路図シートが増える。 <sub>`sheet kind (schematic / panel)`</sub>
- 名前は種別ごとに数える (回路図が3枚あっても最初の盤は「Panel1」になる)。 <sub>`sheet kind (schematic / panel)`</sub>
- すでに同じ名前のシートがあるときは、空いている番号まで数を進める。 <sub>`sheet kind (schematic / panel)`</sub>
- 用紙は表示中のシートに合わせる (盤シートだけA4になってしまわない)。 <sub>`sheet kind (schematic / panel)`</sub>
- 種別を書いていない古いシートは回路図シートとして扱う。 <sub>`sheet kind (schematic / panel)`</sub>
- 図面タブの「シート種別」の表示は種別ごとの文言になる。 <sub>`sheet kind (schematic / panel)`</sub>

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

### symbolEditor

- キャンバスのクリック位置は必ず2.5mmグリッドの交点へ吸い付く <sub>`symbol editor canvas geometry`</sub>
- 原点より左上(マイナス側)をクリックしてもグリッドに吸い付く <sub>`symbol editor canvas geometry`</sub>
- 図形の上に乗っている接続点をクリックすると、図形ではなく接続点が選ばれる <sub>`symbol editor canvas geometry`</sub>
- 図形が重なっている場所では、あとから描いた図形が選ばれる <sub>`symbol editor canvas geometry`</sub>
- 何も無いところをクリックすると選択は外れる <sub>`symbol editor canvas geometry`</sub>
- 属性テキストは配置位置をクリックして選べる <sub>`symbol editor canvas geometry`</sub>
- 図形をドラッグすると、その図形の全ての点が同じ量だけ動く <sub>`symbol editor canvas geometry`</sub>
- 新しい接続点の番号は、いま一番大きい番号の次になる(最初は1番) <sub>`symbol editor canvas geometry`</sub>
- 方向ボタンで足した接続点は、図形の外側1グリッドの位置にグリッドに乗って置かれる <sub>`symbol editor canvas geometry`</sub>
- 同じ方向にもう1本足しても、接続点が同じ点に重なることはない <sub>`symbol editor canvas geometry`</sub>
- 過不足のないシンボルは指摘ゼロで保存できる <sub>`symbol editor validation`</sub>
- 2.5mmグリッドから外れた接続点は保存前に指摘される <sub>`symbol editor validation`</sub>
- 同じ位置に同じ番号の接続点が2つあると保存前に指摘される <sub>`symbol editor validation`</sub>
- 同じ位置に別番号の接続点が重なっていると保存前に指摘される <sub>`symbol editor validation`</sub>
- 離れた2点に同じ番号を置くのは正しい(端子台の貫通端子)ので指摘しない <sub>`symbol editor validation`</sub>
- 参照記号(TAG)の属性スロットが無いと保存できない <sub>`symbol editor validation`</sub>
- 同梱シンボルと同じidは、既存図面の記号が変わってしまうので使えない <sub>`symbol editor validation`</sub>
- idが空・英数字以外・名前や接頭辞が空・図形0個・接続点0個はいずれも保存できない <sub>`symbol editor validation`</sub>
- 指摘は見つかった分を全部返すので、直しては保存し直す回数が減る <sub>`symbol editor validation`</sub>
- 保存前チェックは「グリッド」「番号の重複」「必須属性」「任意属性」の4行を必ず出す <sub>`symbol editor pre-save checklist`</sub>
- 過不足のないシンボルでは、任意属性が未配置なら注意、それ以外は合格になる <sub>`symbol editor pre-save checklist`</sub>
- 任意属性をすべて配置すると4行すべてが合格になる <sub>`symbol editor pre-save checklist`</sub>
- 問題のある行はエラーになり、残りの理由は5行目以降に並ぶ <sub>`symbol editor pre-save checklist`</sub>
- エラーが1つでもあるあいだは保存ボタンを押せない <sub>`symbol editor pre-save checklist`</sub>
- 空から始めると、参照記号(TAG)の置き場だけが入った白紙の下書きになる <sub>`symbol editor store`</sub>
- 既存シンボルを複製して開くと、その図形と接続点がそのまま下書きに入る <sub>`symbol editor store`</sub>
- 図形を描き足すと、その図形が選択された状態になる <sub>`symbol editor store`</sub>
- 選択した図形はドラッグで動かせる <sub>`symbol editor store`</sub>
- 選択した図形・接続点は削除できる <sub>`symbol editor store`</sub>
- 方向ボタンで足した接続点には次の番号が付き、2.5mmグリッドに乗る <sub>`symbol editor store`</sub>
- キャンバスで置いた接続点も、クリック位置に関わらずグリッドの交点に乗る <sub>`symbol editor store`</sub>
- 接続点の番号・名前・方向は表から書き換えられる <sub>`symbol editor store`</sub>
- 属性テキストは置いたり外したりでき、外した属性はチェックで注意になる <sub>`symbol editor store`</sub>
- シンボル情報 (id・名前・接頭辞・検索キー) は右のフォームから書き換えられる <sub>`symbol editor store`</sub>
- 保存先のパスはidに追従する (フッタに出るパス) <sub>`symbol editor store`</sub>
- 同梱シンボルと同じidにすると、保存前チェックがエラーになり保存できない <sub>`symbol editor store`</sub>
- グリッド外の接続点・番号の重複・必須属性の欠落は保存前チェックに出る <sub>`symbol editor store`</sub>
- 保存すると下書きがそのままユーザーライブラリへ書かれ、保存先のパスが返る <sub>`symbol editor store`</sub>
- 保存に成功すると未保存の変更が消え、部品挿入の一覧も読み直される <sub>`symbol editor store`</sub>
- 保存が拒否されたら何も書かれず、理由が全部そのまま出る <sub>`symbol editor store`</sub>
- 未保存の変更があるまま閉じようとすると、いったん確認を出して閉じない <sub>`symbol editor store`</sub>
- 「複製から新規」のピッカーは、同梱もユーザーも合わせた全シンボルから選べる <sub>`symbol editor store`</sub>

### symbolLibrary

- 検索語は英語名・日本語名・シンボルidのどれに当たっても一致する <sub>`symbolMatchesQuery`</sub>
- 名称に無い現場の呼び方 (NFB・マグネットスイッチ等) も検索キーワードで見つかる <sub>`symbolMatchesQuery`</sub>
- 空の検索語はすべてのシンボルに一致する <sub>`symbolMatchesQuery`</sub>
- シンボルはライブラリの並び順のままカテゴリごとにまとまる <sub>`groupSymbolsByCategory`</sub>
- 検索語を渡すと一致するシンボルだけが残り、空になったカテゴリは消える <sub>`groupSymbolsByCategory`</sub>
- 日本語で使っているときは日本語名、英語のときは英語名が一覧に出る <sub>`symbolName`</sub>
- 片方の名前しか無いシンボルは、あるほうの名前が出る <sub>`symbolName`</sub>
- 名前がどちらも空ならシンボルidが出る(一覧から行が消えない) <sub>`symbolName`</sub>

### terminalJumpers

- 隣り合う端子どうしのジャンパは正規化され、隣り合わない組は捨てられる <sub>`parseJumperSpec`</sub>
- 複数のジャンパはそれぞれ組として残り、端子番号順に並ぶ <sub>`parseJumperSpec`</sub>
- 空文字や読めない記述はジャンパ無しとして扱う <sub>`parseJumperSpec`</sub>
- 多段端子台では段記号付きのジャンパ(同じ極の段をまたぐ組・同じ段の隣の極)だけが通る <sub>`parseJumperSpec`</sub>
- 多段端子台に段記号なしの指定("1-2")を書いても、どの段か決まらないので捨てられる <sub>`parseJumperSpec`</sub>
- 端子台に無い段を指すジャンパ("1a-1c")と斜めの組("1a-2b")は捨てられる <sub>`parseJumperSpec`</sub>
- ジャンパバーの寸法(線幅・端子の印の半径・端子番号の文字高さ)はRust側と同じ数値になる <sub>`jumper bar dimensions`</sub>
- 同じ列の2端子を繋ぐジャンパは、列の中心線上の2つの印の隙間だけに描かれる <sub>`jumperBars`</sub>
- ジャンパバーの長さは、その端子台の端子の間隔から両端の印の半径を引いた長さちょうどになる <sub>`jumperBars`</sub>
- 段をまたぐジャンパは、極の行の上、2つの段の列の印の隙間に描かれる <sub>`jumperBars`</sub>
- バーの両端はちょうど自分の端子の印の縁の上にあり、ほかのどの端子よりも自分の端子に近い <sub>`jumperBars`</sub>
- ジャンパバーは、どの端子番号の文字にも重ならない <sub>`jumperBars`</sub>
- 複数のジャンパはそれぞれ独立した線分になる <sub>`jumperBars`</sub>
- 端子台に無い端子番号を指すジャンパは線分を作らない <sub>`jumperBars`</sub>
- 個数が合い、重複も空要素も無ければ妥当 <sub>`validTerminalNumbers`</sub>
- ピン数と合わない個数は無効 <sub>`validTerminalNumbers`</sub>
- 重複したラベルは無効 <sub>`validTerminalNumbers`</sub>
- 空要素は無効 <sub>`validTerminalNumbers`</sub>
- 判定はtrim後の値で行う: ラベル前後の空白は無視し、空白しか違わない2つは重複と数え、空白だけのラベルも無効 (Rust側とparity) <sub>`validTerminalNumbers`</sub>
- 保存形の区切り記号 (カンマ) を含むラベルは無効 (読み戻すと端子の個数が変わってしまう) <sub>`validTerminalNumbers`</sub>
- 対応表は生のピン番号と表示ラベルを相互変換し、知らない番号はそのまま返す (Rust側 TerminalLabels とparity) <sub>`terminalLabels / rewriteJumperLabels`</sub>
- 端子台以外のシンボルには対応表を作らない (上書きは端子台だけの仕組み) <sub>`terminalLabels / rewriteJumperLabels`</sub>
- ジャンパ指定の書き換えは認識できた端子番号だけを置き換え、空白・読めない項目・存在しない端子を含む項目はそのまま残す (逆向きで元の文字列へ戻る。Rust側 rewrite_jumper_labels とparity) <sub>`terminalLabels / rewriteJumperLabels`</sub>
- attrs["terminal_numbers"]が無ければシンボル既定の番号のまま <sub>`terminalNumbersOfInstance / terminalLabels`</sub>
- 妥当な上書きはピン順のラベルとして使われ、生の番号→上書きラベルの対応が引ける <sub>`terminalNumbersOfInstance / terminalLabels`</sub>
- 個数が合わない上書きは既定の番号へフォールバックする <sub>`terminalNumbersOfInstance / terminalLabels`</sub>
- 開始番号から刻み1でピン順に振る (単段) <sub>`terminalRenumberPlan`</sub>
- 多段端子台は段記号を保ったまま極だけ採番するので、同じ極の全段が同じ番号を共有する <sub>`terminalRenumberPlan`</sub>
- 増分と接頭辞はどちらも効く <sub>`terminalRenumberPlan`</sub>
- 極数と段数は端子番号から数え、単段の端子台は段数1になる <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- 段数を変えたsymbol_idは、段数1のとき`_1l`の別名ではなく単段のidへ正規化される <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- 端子台以外のシンボルと範囲外の段数はnull (プレビューを出さない) <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- 段を増やすと極ごとのラベルに段記号が付き、減らすと残る段のラベルだけになる (段数1では段記号が落ちる) <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- 末尾が英字でない上書きラベル (接頭辞付きの`X11`) は段記号を外す対象にならず、そのまま土台になる <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- 段aのラベルの末尾が段aの記号以外の文字 (別の文字で終わる手書きの上書き) では土台から取り除かず、2極の土台が同じ文字列に潰れて重複することもない (Task 3再レビューの持ち越し) <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- 指定した端子の後ろに挿入すると、symbol_idは末尾に1極増え、番号列はそれより後ろが繰り下がって空いた番号を新しい予備が引き継ぐ <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- 挿入位置を指定しない (末尾) と、番号は繰り下がらずそのまま続く <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- 接頭辞付きのラベル (A1..A8) でも数字の部分だけを繰り下げ、接頭辞は保たれる (Rust側 inserting_a_spare_keeps_a_prefixed_labels_prefix と同じ値) <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- 2回目の挿入は番号順 (チャートの並び) で位置を数えるので、1回目で末尾に足した極があっても末尾・途中のどちらへも挿入できる (Rust側 inserting_a_second_spare_counts_the_position_in_the_charts_order と同じ値) <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- 「空き番号のまま残す」(renumberFollowing=false) では後ろの番号を変えず、空いていた番号を予備が取る (Rust側 leaving_the_following_numbers_alone_gives_the_spare_the_vacant_number と同じ値) <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- 「空き番号のまま残す」で予備の番号が既存の端子とぶつかるときはnull (Rust側は理由付きで拒否) <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- 挿入位置のラベルに数字が無い ("予備") ときはnull (Rust側は理由付きで拒否) <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- 0個以下の指定はnull (プレビューを出さない) <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- 端子台に無い端子番号を挿入位置に指定するとnull <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- 多段端子台では、挿入した1つの予備極が段の数だけの端子になる <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- 増分2で採番したような表示ラベル(11,13,15,17)でも、隣接判定は生のピン配置で決まるので、物理的に隣り合う極どうしのジャンパは通る (Rust側 adjacency_after_an_override_is_based_on_the_raw_pin_layout_not_the_label_arithmetic とparity) <sub>`parseJumperSpec with an overridden display order`</sub>

### textSize

- 図面の文字は図形と同じ割合で拡大・縮小し、画面上で小さすぎて読めない大きさになったら描かない (大きさ 0) <sub>`drawing text size on screen`</sub>
- 読めないほど小さい文字はフォントを変えずに描かない。0px を指定すると WebKit は無視して直前の大きさで描くため <sub>`drawing text size on screen`</sub>

### textStyle

- 注記の色はどれも名前付きトークンで、既定のトークンも必ずその中にある <sub>`note styling`</sub>
- 生の色コードは色トークンではないので、図面に紛れ込むことはない <sub>`note styling`</sub>
- 色を指定していない注記は画面でも既定色で描かれ、トークンを指定すればその色になる <sub>`note styling`</sub>
- 選択中の注記は色トークンより選択ハイライトが優先されるので、選んだものが分かる <sub>`note styling`</sub>
- 3つの基準位置はCanvas2Dの3つの寄せ方に対応し、既定は従来どおりの左揃えになる <sub>`note styling`</sub>
- 左揃えの注記は基準点から右へ広がる箱で掴め、箱の高さは文字高さぶんある <sub>`note hit box`</sub>
- 中央揃え・右揃えの注記は、基準点の左側にも箱が広がるので同じように掴める <sub>`note hit box`</sub>
- 長い文字の注記ほど掴める箱も広くなるので、書いた文字のどこを押しても選べる <sub>`note hit box`</sub>
- 機能ラベルの属性が無いシンボルには、余分に描くものが無い <sub>`function label`</sub>
- 空白だけの機能ラベルは「無し」と同じ扱いになり、前後の空白は取り除かれる <sub>`function label`</sub>
- スロットを持たない同梱シンボルでは、機能ラベルは外形のすぐ下・基準点を中心に置かれる <sub>`function label`</sub>
- シンボル定義がFUNCTIONスロットを持つ場合は、そのスロットの位置と文字高さに従う <sub>`function label`</sub>

### viewClasses

- エンティティ種別を表示クラスへ対応付ける(ジャンクションは配線扱い) <sub>`entityViewClass`</sub>
- VIEW_CLASSESは全クラスを一意に列挙する(線番・配線注記・ハーネス・図形・盤レイアウトを加えて12クラス) <sub>`entityViewClass`</sub>
- 表示クラスは初期状態で全て表示ONになっている(線番・配線注記・ハーネスも最初から見える) <sub>`view class visibility`</sub>

### viewport

- ワールド⇔スクリーン変換は往復可能で、ズームはアンカー点を固定する <sub>`Viewport`</sub>
- パンはスクリーンpx単位でビューを動かす <sub>`Viewport`</sub>
- スナップは既定で2.5mmグリッドへ丸める <sub>`Viewport`</sub>
- ズーム倍率は妥当な範囲に制限される <sub>`Viewport`</sub>

### wireLegend

- 配線が1本も無いシートでも、凡例ブロックは列見出しだけの表として出る。 <sub>`wire legend block geometry`</sub>
- 線色・線径・引き当てた品番が同じ配線は1行にまとまり、使用長は合算される。 <sub>`wire legend block geometry`</sub>
- 行は線色順、同じ線色の中では線径順に並ぶ。 <sub>`wire legend block geometry`</sub>
- 足し合わせると二進小数で誠実に表せない使用長も、桁の並んだ長い数字ではなく短い小数で表示される。 <sub>`wire legend block geometry`</sub>
- 英語名と日本語名で書かれた同じ線色は同じ電線なので、1行にまとまり使用長も合算される。 <sub>`wire legend block geometry`</sub>
- 凡例に出る色名は、同じシートの配線注記が印字する色名と同じになる。 <sub>`wire legend block geometry`</sub>
- 電線品番表に無い組み合わせの配線は品番欄が空欄のままで、勝手に品番が作られることはない。 <sub>`wire legend block geometry`</sub>
- 凡例ブロックを置くコマンドはadd_entityでkind="wire_legend"、置き場所だけを決める。 <sub>`wire legend block geometry`</sub>
- シート上の凡例ブロックはentityのまま取り出せる。 <sub>`wire legend block geometry`</sub>

### wireNotes

- 線径・線色・電線長がそろったワイヤには「0.75sq 白 0.4m」という注記が付く <sub>`wireNoteText`</sub>
- 欠けている属性は注記から省かれ、長さの無いワイヤは「0.75sq 白」と書かれる <sub>`wireNoteText`</sub>
- 線径も色も長さも無いワイヤには、空の注記ではなく注記そのものが付かない <sub>`wireNoteText`</sub>
- 線径も長さも無い黒の配線には注記を付けない (黒は既定の色)。線径か長さのある黒の配線は色名も書く (wire_note.rsと同ルール) <sub>`wireNoteText`</sub>
- 色名は英字の大小と前後の空白を無視して照合される。知っている色は表示用の名前で、知らない色は入力のまま (ただし小文字にそろえて) 書かれる <sub>`wireNoteText`</sub>
- 横向きのワイヤの注記は配線の下に置かれ、上に出る線番とは反対側になる <sub>`wireNoteLabels`</sub>
- 縦向きのワイヤの注記は配線の右に置かれ、左に出る線番とは反対側になる <sub>`wireNoteLabels`</sub>
- 注記はワイヤ1本ごとに付く(ネットごとに1回だけ描かれる線番とは違う) <sub>`wireNoteLabels`</sub>
- 折れ曲がったワイヤの注記は、いちばん長い直線部分に置かれて読みやすくなる <sub>`wireNoteLabels`</sub>
- 属性の無いワイヤは注記の一覧に現れない <sub>`wireNoteLabels`</sub>
- 注記文字高を設定していないシートは、これまでどおり2.0mmの注記で描かれる <sub>`annotation text height`</sub>
- シートの注記文字高を1.4mmにすると、配線注記はちょうどその高さで描かれ配線にも寄る <sub>`annotation text height`</sub>

### wireNumbers

- 横向きの配線の線番は、配線の中点の2.5mm上に中央揃えで置かれる <sub>`wireNumberLabels`</sub>
- 縦向きの配線の線番は、配線の中点の2.5mm左に右揃えで置かれる <sub>`wireNumberLabels`</sub>
- 1つの線番は、何本のワイヤに分かれていても最も長い線分の中点に1回だけ置かれる <sub>`wireNumberLabels`</sub>
- 線番の無い配線、空白だけの線番は描かない <sub>`wireNumberLabels`</sub>
- 複数の線番があると線番順に並んで返り、同じ図面なら常に同じ並びになる <sub>`wireNumberLabels`</sub>
- 斜めの配線は、横に長ければ上、縦に長ければ左に線番を置く <sub>`wireNumberLabels`</sub>
- 配線以外のエンティティは線番の計算に影響しない <sub>`wireNumberLabels`</sub>
- 注記文字高を設定していないシートの線番は、これまでどおり2.5mm・配線から2.5mmで描かれる <sub>`annotation text height`</sub>
- シートの注記文字高を1.4mmにすると、線番も同じ比率(0.7倍)で小さくなり配線に寄る <sub>`annotation text height`</sub>
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

### wireParts

- ワイヤの品番は線色と線径から電線品番表を引いて決まる <sub>`wirePartNo`</sub>
- 表に無い線色+線径の組み合わせは空のまま <sub>`wirePartNo`</sub>
- ワイヤ自身に書かれた品番は表より優先される <sub>`wirePartNo`</sub>
- 引き当ては線色の前後の空白と英字の大小を無視する <sub>`wirePartNo`</sub>
- ワイヤの英語の色名と表の日本語の色名は同じ電線を指す <sub>`wirePartNo`</sub>
- 線色が空・線径が0のワイヤは引き当てない <sub>`wirePartNo`</sub>
- 同じ線色+線径の行が2つあるときは先に書いた行が勝つ <sub>`wirePartNo`</sub>

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
- 2枚の違うシートに描かれた参照記号は相互に参照し合う: それぞれのインスタンスにもう一方のシートの住所が出る <sub>`G13 device cross-reference`</sub>
- 1枚のシートにしか描かれていない参照記号には、クロスリファレンスが一切付かない <sub>`G13 device cross-reference`</sub>
- 接点を従えたリレーのコイルと、その接点は、デバイス単位のクロスリファレンスの対象から外れる(コイル⇔接点クロスリファレンスが既に互いの所在を書いているため) <sub>`G13 device cross-reference`</sub>
- 接点をどこにも持たないコイルにはコイル⇔接点クロスリファレンスが何も描かないので、そういうコイルだけの参照記号には普通のデバイス単位クロスリファレンスが付く (G13) <sub>`G13 device cross-reference`</sub>
- 整えバリアントの複製シートがあっても、元シートにも案シートにも住所注記は生えない (複製は「同じ参照記号を持つ別シート」として数えない) <sub>`G13 device cross-reference`</sub>
- シートごとのデバイスXRef表は、各シンボルのentity idを脇に描くテキストへ対応付ける <sub>`G13 device cross-reference`</sub>
- 3枚のシートに跨る参照記号は、シート順で並んだ他シート全部の住所を1行に並べる <sub>`G13 device cross-reference`</sub>
- デバイスXRefのテキストはシンボル外形の右側、参照記号のラベルと同じ高さに置かれる <sub>`G13 device cross-reference`</sub>
- 参照記号のラベルがシンボルの外形より横に長いときは、デバイスXRefはその右まで下がるので、ラベルと重ならない <sub>`G13 device cross-reference`</sub>

### blocks

- 続けて並んだツールは1つのまとまりになり、本文か思考が来るとまとまりが切れる。失敗や実行中の手順があればまとまりにその印が付く <sub>`agent reply display`</sub>
- ブロックの無い古いメッセージは、本文のあとにツールを1つのまとまりで並べる <sub>`agent reply display`</sub>
- 応答中は最後の行だけが開き、履歴の会話や終わったターンではどの行も閉じている。利用者が開閉した行はその状態を保つ <sub>`agent reply display`</sub>
- 思考の時間は秒に丸め、1秒未満や不明なら秒数を出さない <sub>`agent reply display`</sub>
- 切り詰めた結果は省いた量をKB (切り上げ) で示し、全部あるときは何も示さない <sub>`agent reply display`</sub>
- ツールの入力は2字下げのJSONで表示し、文字列はそのまま出す <sub>`agent reply display`</sub>

### markdown

- 見出し・箇条書き・表・強調・インラインコード・引用・区切り線を描画する <sub>`reply markdown`</sub>
- コードブロックには言語名とコピーボタンが付き、閉じていないコードブロックも届いた分までをコードとして描く <sub>`reply markdown`</sub>
- 返答の生のHTMLは動かさず文字として表示する <sub>`reply markdown`</sub>
- リンクはhttp(s)だけをリンクにし、javascript:・file:・data:はただの文字にする <sub>`reply markdown`</sub>
- 画像は読み込まず、代替文字を表示したリンクにする <sub>`reply markdown`</sub>
- 外部で開いてよいURLはhttp(s)だけ <sub>`reply markdown`</sub>

### ファイル操作のショートカット

- ⌘N=新規 / ⌘O=開く / ⌘S=保存 / ⇧⌘S=名前を付けて保存 <sub>`editor shortcuts: ファイル操作`</sub>
- WindowsとLinuxではCtrlでも同じショートカットが効く <sub>`editor shortcuts: ファイル操作`</sub>
- 修飾キー無しのN/O/Sはファイル操作にならない (図面のショートカットのまま) <sub>`editor shortcuts: ファイル操作`</sub>
- モーダル (シンボルエディタ等) を開いている間はファイル操作のショートカットが効かない <sub>`editor shortcuts: ファイル操作`</sub>
- ⌥を足した組み合わせ (⌥⌘Sなど) はファイル操作にしない <sub>`editor shortcuts: ファイル操作`</sub>
- ⇧⌘Nと⇧⌘Oはファイル操作に割り当てない (別のショートカットに残す) <sub>`editor shortcuts: ファイル操作`</sub>
- 判定は物理キー (`code`) で行うので、非ラテン配列でも同じキーで効く <sub>`editor shortcuts: ファイル操作`</sub>
- ⌘Z=元に戻す / ⇧⌘Z=やり直し で、WindowsとLinuxではCtrlでも同じ <sub>`editor shortcuts: 元に戻す・やり直し`</sub>
- 修飾キー無しのZや⌥を足した組み合わせ、Z以外のキーは元に戻す・やり直しにならない <sub>`editor shortcuts: 元に戻す・やり直し`</sub>

### entityKindLabels

- 図面に置ける全てのエンティティ種別に「種類」行のラベルがある (電線品番凡例ブロックも盤の凡例表も含む)。 <sub>`entity kind labels`</sub>
- ラベルが引くi18nキーは英語・日本語の両方に実体があり、空文字にならない。 <sub>`entity kind labels`</sub>
- 電線品番凡例ブロックを選ぶと、種別の名前が出る (英語・日本語それぞれの短い名前)。 <sub>`entity kind labels`</sub>

### FreeCAD対応付けの表示 (プロパティパネル)

- 配置は「(120, 45.5, 0) mm / 90°」の形で、FreeCADアドオンのパネルと同じ文面になる。 <sub>`mechLink`</sub>
- 計算で出た端数 (45.500000000000004 のような値) は有効6桁に丸めて表示する。 <sub>`mechLink`</sub>
- 未同期の配置は空文字になり、「3D対応付け」行はオブジェクト名だけを出す。 <sub>`mechLink`</sub>

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
- 注記文字高を入力するとシート設定のコマンドになり、注記・線番・タグがまとめて縮む <sub>`annotationTextCommand`</sub>
- 空欄にすると既定 (2.0mm) へ戻すコマンドになる <sub>`annotationTextCommand`</sub>
- 範囲 (1.0〜3.5mm) を外れた値・数値でない入力は送らない (エンジンも同じ範囲で拒否する) <sub>`annotationTextCommand`</sub>
- 今と同じ高さならコマンドを送らない (無駄なundo履歴を作らない) <sub>`annotationTextCommand`</sub>
- コネクタでないシンボルを保存しても、パネルに欄の無い嵌合相手の属性は残る (MCP・CLIが書いた値を落とさない) <sub>`symbolAttrs`</sub>
- コネクタの嵌合相手欄を空にして保存すると、その属性はキーごと消える <sub>`symbolAttrs`</sub>
- パネルが持たない属性 (端子台のジャンパなど) は、どのシンボルでもそのまま残る <sub>`symbolAttrs`</sub>
- 色・線径・品番・長さのどれも変わっていなければコマンドを送らない <sub>`wireUpdateCommand`</sub>
- 長さを手で変えると更新コマンドになり、出所は「手入力」になる <sub>`wireUpdateCommand`</sub>
- FreeCADで計測した長さを手で上書きすると、その旨のフラグが立つ (呼び出し側が警告する) <sub>`wireUpdateCommand`</sub>
- 長さ以外だけを変えたときはFreeCAD計測の出所を保つ <sub>`wireUpdateCommand`</sub>
- ケーブルのプロパティは型番・芯数・線径・シールド・長さを1回のupdate_entityで書き、芯線割当はそのまま残す <sub>`cableUpdateCommand`</sub>
- 芯数が1以上の整数でない、線径や長さが数値でない入力は図面を変えない <sub>`cableUpdateCommand`</sub>
- 何も変えていなければコマンドを送らない <sub>`cableUpdateCommand`</sub>
- 1本線のケーブル (run_cable) のプロパティを書き換えても、種類・区間・芯の行き先はそのまま残る <sub>`cableUpdateCommand`</sub>
- 銘板の文字・文字高・位置・追従・型番を1回のupdate_entityで書き、空の2行目は付けない <sub>`nameplateUpdateCommand`</sub>
- 文字高が正の数でなければ、また何も変えていなければ図面を変えない <sub>`nameplateUpdateCommand`</sub>

### ribbonLayout

- 1列に入る数ちょうどの小ボタンは、1列のままで列が増えない。 <sub>`ribbon small-button columns`</sub>
- 1列に入る数を超えた小ボタンは、リボンからはみ出さないよう次の列へ送られる。 <sub>`ribbon small-button columns`</sub>
- 小ボタンが1つも無いグループは列も作らない (空の列が幅だけ取らない)。 <sub>`ribbon small-button columns`</sub>
- 2列に収まらない数の小ボタンは、必要なだけ列を増やして全部が押せる位置に並ぶ。 <sub>`ribbon small-button columns`</sub>

### symbolPickerDynamic

- 段数ステッパーは1〜3段に収める(1より小さい値は1段、3より大きい値は3段になる) <sub>`clampLevels`</sub>
- 段数の入力欄に数字以外や小数を入れても、1〜3段の整数に直される <sub>`clampLevels`</sub>
- 極数ステッパーは1極から50極までに収める <sub>`clampPins`</sub>
- 段数ステッパーは端子台を選んだときだけ出る(コネクタには段という概念が無い) <sub>`supportsLevels`</sub>
- 段数が既定の1段のままなら、これまでどおりの単段の端子台 (terminal_block_8p) が置かれる <sub>`dynamicSymbolId`</sub>
- 段数を2段にすると2段の端子台 (terminal_block_8p_2l) が置かれる <sub>`dynamicSymbolId`</sub>
- コネクタは段数を上げても段は付かない (コネクタに段は無い) <sub>`dynamicSymbolId`</sub>
- 段数ステッパーのラベルと範囲ヒントは英語カタログにも日本語カタログにも入っている <sub>`段数ステッパーの表示文字列`</sub>

### updateStatus

- 最新のときは最終確認の時刻つきで「最新です」を緑で出す <sub>`update status line`</sub>
- 新しい版があるときは版と公開日を出し、公開日が無ければ版だけを出す <sub>`update status line`</sub>
- ダウンロード中はMB単位の進み具合を出し、全体が分からなければ受け取った量だけを出す <sub>`update status line`</sub>
- 失敗は理由つきで赤く出す <sub>`update status line`</sub>
- 開発ビルドとブラウザ検証モードでは、確認しないことを淡く出す <sub>`update status line`</sub>
- リリースノートは空行を除いた先頭3行を抜粋し、公開日は日付の部分だけを使う <sub>`update status line`</sub>

### chatComposer

- 会話のトークンはターンをまたいで足し合わされる <sub>`sumUsage`</sub>
- GitHub Copilotのプレミアムリクエストは小数のままトークンと別枠で数える <sub>`sumUsage`</sub>
- 使用量がまだ無い会話は全て0(バッジは「1x」表示になる) <sub>`sumUsage`</sub>
- Cmd+Enter (macOS) で送信する <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- Ctrl+Enter で送信する <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- ただのEnterは送信せず、入力欄の改行になる (日本語変換の確定で誤送信しない) <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- Shift+Enterも送信しない (改行) <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- IME変換中 (isComposing) は、Cmd/Ctrlを押していても送信しない <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- 変換確定のEnterがkeyCode 229で届く環境 (WebKit) でも送信しない <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- Enter以外のキーはCmd/Ctrlと一緒でも送信しない <sub>`isSendShortcut (チャット入力の送信キー)`</sub>

### 閉じる・リロード時の未保存ガード

- 未保存の編集を捨てないと答えると、ウィンドウは閉じない <sub>`close guard: アプリ (Tauri)`</sub>
- 未保存の編集を捨ててよいと答えると、ウィンドウはそのまま閉じる <sub>`close guard: アプリ (Tauri)`</sub>
- 未保存の編集が無ければ確認は出ず、すぐ閉じる <sub>`close guard: アプリ (Tauri)`</sub>
- ガードを取り外すと閉じる要求に割り込まなくなる <sub>`close guard: アプリ (Tauri)`</sub>
- ブラウザ検証では未保存の編集があるときだけ離脱確認を出す <sub>`close guard: ブラウザ検証`</sub>
- ブラウザ検証でもガードを取り外すと離脱確認は出なくなる <sub>`close guard: ブラウザ検証`</sub>

### drawingContext

- 選択が無ければコンテキストはシート全体になる <sub>`drawingContextTag`</sub>
- シートが無くても文字列は壊れない <sub>`drawingContextTag`</sub>
- 選択があれば参照記号を列挙する(無ければ種別名) <sub>`drawingContextTag`</sub>
- 10件を超える選択は「+N件」に畳まれる <sub>`drawingContextTag`</sub>
- アクティブシートに無い選択idは無視される(全て外れれば全体扱い) <sub>`drawingContextTag`</sub>
- 空の下書きにはそのまま挿入される <sub>`appendContextTag`</sub>
- 既存の下書きとは改行で区切る(改行済みなら重ねない) <sub>`appendContextTag`</sub>

### ファイルメニュー (新規/開く/保存/名前を付けて保存)

- 保存先が決まっていれば「保存」はダイアログを出さずに上書きする <sub>`file menu: 保存`</sub>
- 保存先が未定なら「保存」は保存ダイアログを出す (名前を付けて保存と同じ) <sub>`file menu: 保存`</sub>
- 保存ダイアログをキャンセルすると何も書かない <sub>`file menu: 保存`</sub>
- 「名前を付けて保存」は開いているファイルのパスを既定にする <sub>`file menu: 保存`</sub>
- 上書き保存に失敗したら、書こうとしたパスと理由をログに出し、未保存のまま残る <sub>`file menu: 保存`</sub>
- 保存に失敗したら理由をログに出し、保存先は変えない <sub>`file menu: 保存`</sub>
- 「新規」は無題のプロジェクトを作り、保存先は空になる <sub>`file menu: 新規と開く`</sub>
- 未保存の編集があるときの「新規」は確認し、キャンセルすると図面はそのまま <sub>`file menu: 新規と開く`</sub>
- 新規に失敗したら理由をログに出し、図面はそのまま <sub>`file menu: 新規と開く`</sub>
- 未保存の編集が無ければ確認なしで新規・開くへ進む <sub>`file menu: 新規と開く`</sub>
- 「開く」はダイアログで選んだ.mdkprojを読み込み、そのパスが保存先になる <sub>`file menu: 新規と開く`</sub>
- 開くダイアログをキャンセルすると何も読み込まない <sub>`file menu: 新規と開く`</sub>
- 未保存の編集があるときの「開く」は確認し、キャンセルするとダイアログも出ない <sub>`file menu: 新規と開く`</sub>
- .kicad_schを選ぶとKiCad回路図として読み込み、件数をログに出す <sub>`file menu: 新規と開く`</sub>
- .dxfを選ぶとDXF図面として読み込み、件数と警告をログに出す <sub>`file menu: 新規と開く`</sub>
- 「読み込み」ボタンは選んだ形式のフィルタだけで開くダイアログを出し、未保存の編集があれば先に確認する <sub>`file menu: 新規と開く`</sub>
- 最近使ったファイルはダイアログ無しで開く <sub>`file menu: 新規と開く`</sub>
- 無くなっていた最近使ったファイルは一覧から外し、理由をログに出す <sub>`file menu: 新規と開く`</sub>
- 一時的に開けなかっただけ (権限・共有ドライブ未マウント) のファイルは一覧に残す <sub>`file menu: 新規と開く`</sub>
- 「ファイルが無い」という失敗だけを見分ける (最近使ったファイルから外すかの判断に使う) <sub>`fileActions helpers`</sub>
- DXF書き出しは「<シート名>.dxf」を既定にDXFフィルタで保存先を聞き、表示中のシートを書き出す <sub>`file menu: シートの書き出し`</sub>
- KiCad書き出しは「<シート名>.kicad_sch」を既定にし、SVG/PDFは従来どおり拡張子のフィルタで保存する <sub>`file menu: シートの書き出し`</sub>
- 保存ダイアログをキャンセルすると何も書き出さない <sub>`file menu: シートの書き出し`</sub>
- シートが無ければダイアログを出さずにログで案内する <sub>`file menu: シートの書き出し`</sub>
- 書き出しに失敗したら理由をログに出す <sub>`file menu: シートの書き出し`</sub>
- DXF / KiCad書き出しは「何を何件書いたか」と「書けなかったもの」をコマンドログに出す <sub>`file menu: シートの書き出し`</sub>
- 配線レイヤが決まらなかったDXF読み込みは、候補のレイヤ名をログで知らせて覚えておく <sub>`file menu: DXFの配線レイヤ`</sub>
- 選んだレイヤで読み直すと、同じファイルをそのレイヤだけを配線として読み込む <sub>`file menu: DXFの配線レイヤ`</sub>
- 直前のDXF読み込みが無ければ読み直しは何もしない <sub>`file menu: DXFの配線レイヤ`</sub>

### ネイティブダイアログ (確認/通知/ファイル選択)

- アプリ本体の確認ダイアログはプラグインの`ask`を「警告」として出し、「はい」でtrueになる <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- 確認ダイアログで「いいえ」を選ぶとfalseになり、続行しない <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- 確認ダイアログのタイトルと重大度は呼び出し側で上書きできる <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- 通知はプラグインの`message`で出す (押せるのはOKだけ) <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- 保存先はプラグインの保存ダイアログで選び、既定のファイル名とフィルタを渡す <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- 開くファイルは1つだけ選べるダイアログで選ぶ <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- 開くダイアログをキャンセルするとnullになる (複数選択の結果もnull扱い) <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- ブラウザ検証では確認は`window.confirm`で出す <sub>`native dialogs: Tauri外 (ブラウザ検証) のフォールバック`</sub>
- ブラウザ検証では通知は`window.alert`で出す <sub>`native dialogs: Tauri外 (ブラウザ検証) のフォールバック`</sub>
- ブラウザ検証ではネイティブのファイルダイアログが無いので、保存先はファイル名を既定にして入力してもらう <sub>`native dialogs: Tauri外 (ブラウザ検証) のフォールバック`</sub>
- ブラウザ検証で開くパスも入力してもらい、キャンセルするとnullになる <sub>`native dialogs: Tauri外 (ブラウザ検証) のフォールバック`</sub>
- ダイアログを持たない実行環境 (テストランナー等) では確認はfalse・入力はnullになり、例外は出さない <sub>`native dialogs: Tauri外 (ブラウザ検証) のフォールバック`</sub>

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
- 実装済みロケールは英語・日本語・中国語・スペイン語・フランス語・ドイツ語の6つ <sub>`i18n`</sub>
- 全ロケールのカタログは英語とキーが完全に一致する(訳し漏れ・余分なキーをCIで検出) <sub>`i18n`</sub>
- どのカタログでも文字列は空にできない(キーだけ足して訳し忘れることを防ぐ) <sub>`i18n`</sub>
- カタログの文字列はすべて表示できる(`@`などの記号は書き方を誤ると画面が真っ白になる) <sub>`i18n`</sub>
- 翻訳は英語と同じ名前付きプレースホルダを持つ(`{count}`の欠落・綴り違いを検出) <sub>`i18n`</sub>
- 複数形の文(`|`区切り)は英語と同じ数の形を持つ <sub>`i18n`</sub>
- 言語名は各言語の自称で表示される(英語に翻訳しない) <sub>`i18n`</sub>
- 言語タグは大文字・余白があっても解決でき、未知・空の値は英語になる <sub>`i18n`</sub>

### chat

- 既知の会話ではtext_deltaが進行中メッセージへ連結される <sub>`chat store: applyAgentEvent`</sub>
- turn_completedでストリーミングを解除し、トークン使用量を確定する <sub>`chat store: applyAgentEvent`</sub>
- デルタが来なかった場合はturn_completedのresultで本文を埋める <sub>`chat store: applyAgentEvent`</sub>
- ツール呼び出しはチップになり、idで成功/失敗が確定する <sub>`chat store: applyAgentEvent`</sub>
- 応答は起きた順のブロックとして組み立てられ、同じ種類が続く間は1つにまとまり、思考には最初から最後の差分までの時間が付く <sub>`chat store: applyAgentEvent`</sub>
- 会話一覧から読み込んだメッセージはブロックとツールの結果を持ち、知らない種類のブロックは捨てる <sub>`chat store: applyAgentEvent`</sub>
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
- 添付つきの送信は添付の指定をバックエンドへ渡し、発言には添付の名前・種類・大きさを残す <sub>`chat store: アクション`</sub>
- 保存された会話の発言にも添付の情報が載り、添付の無い発言は空になる <sub>`chat store: アクション`</sub>
- sendは会話を新規作成し、サーバー採番のidを引き取る <sub>`chat store: アクション`</sub>
- send解決前に届いたイベントも同じ会話へ入る <sub>`chat store: アクション`</sub>
- 空プロンプトとストリーミング中の送信は無視される <sub>`chat store: アクション`</sub>
- 答えている最中の割り込み送信は、進行中のターンを中断してから同じ会話へ新しい指示を送る <sub>`chat store: アクション`</sub>
- 答えていないときの割り込み送信は普通の送信と同じで、中断APIは呼ばない <sub>`chat store: アクション`</sub>
- 会話idの採番を待っている間 (最初の送信が返る前) は割り込めず、何も送らない <sub>`chat store: アクション`</sub>
- 空の割り込み送信は無視され、進行中のターンも中断しない <sub>`chat store: アクション`</sub>
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
- GitHub Copilotが付ける「サーバー名-」の接頭辞も外して同じ名前で表示する <sub>`summarizeToolUse`</sub>
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

### chatAttachments

- 画像 (PNG/JPEG/GIF/WebP)・PDF・Excel (.xlsx)・CSVだけを添付でき、種類は拡張子でも判断する (大文字小文字は問わない) <sub>`pending chat attachments`</sub>
- ブラウザが別の種類を名乗っても、知っている拡張子なら拡張子で決める (WindowsはCSVをExcelの種類と名乗る) <sub>`pending chat attachments`</sub>
- 表計算はPDFを読めないOpenAI互換のモデルにも送れる (テキストにして渡すため) <sub>`pending chat attachments`</sub>
- 20MBを超えるファイルは大きさつきで「大きすぎます」になり、大きさはMB/KBで表示する <sub>`pending chat attachments`</sub>
- 貼り付け・ブラウザのファイルは中身をbase64で持ち、送信する添付と発言に出す情報になる <sub>`pending chat attachments`</sub>
- ⌘Vの画像には「貼り付け画像 N.png」の名前を順に付ける (クリップボードに名前が無いため) <sub>`pending chat attachments`</sub>
- アプリで選んだ・落としたファイルはパスで持ち、名前・種類・大きさと読めない理由はRustの下見から取る <sub>`pending chat attachments`</sub>
- 6件目以降は件数オーバーで送れない状態になり、1件外すと5件以内に戻ったものは送れるようになる <sub>`pending chat attachments`</sub>
- OpenAI互換のモデルを選んでいるとPDFは送れない状態になり (注意帯)、他のプロバイダでは送れる <sub>`pending chat attachments`</sub>
- 送れない添付は送信する添付に入れない <sub>`pending chat attachments`</sub>
- 貼り付けで添付するのは画像だけで、ドラッグはファイルを含むときだけ受ける <sub>`pending chat attachments`</sub>

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

- 自動注記 (シート間XRef・リレー注記・デバイス単位XRef) はドキュメントが変わったときだけ計算し直す (描画のたびに全プロジェクトを走査しない) <sub>`document store`</sub>
- entity_upserted / entity_removed パッチがミラーのシートを更新する <sub>`document store`</sub>
- project_replacedでミラーのプロジェクト全体が置き換わる <sub>`document store`</sub>
- シート追加・削除のパッチは並び順を保つ <sub>`document store`</sub>
- シートメタ更新のパッチはエンティティに触れない <sub>`document store`</sub>
- 古いrevisionのパッチは破棄される(二重配信しても安全) <sub>`document store`</sub>
- mech_links_replaced パッチでFreeCAD対応付けの一覧が置き換わる <sub>`document store`</sub>
- 図枠テンプレートを差し替えるパッチはミラーへ反映され、undoでも即座に元へ戻る <sub>`document store`</sub>
- 起動時に図枠テンプレートの一覧を読み込み、以降の描画はその定義で行う <sub>`document store: 図枠テンプレートの解決`</sub>
- 図枠テンプレートの一覧が読めなくても図面は開ける(同梱のjis-a3で描く) <sub>`document store: 図枠テンプレートの解決`</sub>
- プロジェクトが選んでいる図枠テンプレートの定義が、図面の描画に使われる <sub>`document store: 図枠テンプレートの解決`</sub>
- 図枠テンプレートが未指定なら同梱のjis-a3で描く(従来どおりの見た目) <sub>`document store: 図枠テンプレートの解決`</sub>
- 他所で作った図面が知らない図枠idを持っていても、jis-a3で開けて図面は失われない <sub>`document store: 図枠テンプレートの解決`</sub>
- 図枠を差し替えるとキャンバスへ渡す図枠定義もその場で入れ替わる(全シートに効く) <sub>`document store: 図枠テンプレートの解決`</sub>
- 表題欄のバインドはプロジェクト文脈で解決される(「3枚中2枚目」がシートごとに入る) <sub>`document store: 図枠テンプレートの解決`</sub>
- シートが1枚も無ければ図枠の描画情報は無い(空の図面でも落ちない) <sub>`document store: 図枠テンプレートの解決`</sub>

### frameTemplates

- ダイアログを開くと同梱とユーザーの図枠が一覧され、いま図面が使っている図枠が選ばれた状態で出る <sub>`frame template dialog store`</sub>
- 図枠が未指定の図面では、実際に描かれている同梱のjis-a3が選ばれた状態で開く <sub>`frame template dialog store`</sub>
- 一覧の各行は同梱かユーザーかの区別を持つ(どこから来た図枠かが分かる) <sub>`frame template dialog store`</sub>
- 行を選ぶとプレビューと情報行がその図枠の内容に切り替わる <sub>`frame template dialog store`</sub>
- 適用範囲の既定は「このプロジェクト」で、適用すると set_frame_template を1回だけ実行する <sub>`frame template dialog store`</sub>
- いま使っている図枠をそのまま適用してもコマンドは送らない(空のundo段を積まない) <sub>`frame template dialog store`</sub>
- 適用しても一覧の「使用中」は選んだ図枠へ移り、続けて適用を押しても二重にコマンドは飛ばない <sub>`frame template dialog store`</sub>
- 適用範囲を「グローバル既定」にすると図面は編集せず、新規プロジェクトの既定として設定に保存する <sub>`frame template dialog store`</sub>
- 図面の図枠を替えたあとに適用範囲を切り替えても、それぞれの適用は1回ずつしか起きない <sub>`frame template dialog store`</sub>
- OKは適用してからダイアログを閉じる <sub>`frame template dialog store`</sub>
- 適用は閉じずに反映する(続けて別の図枠を試せる) <sub>`frame template dialog store`</sub>
- キャンセルは何も適用せずに閉じる <sub>`frame template dialog store`</sub>
- 適用が失敗したらダイアログは開いたままで理由を表示する <sub>`frame template dialog store`</sub>
- 読み込めなかった図枠JSONは一覧の代わりに理由として並べる(壊れたファイルで一覧が空にならない) <sub>`frame template dialog store`</sub>
- 「+ JSONを読み込み…」はユーザー図枠の置き場を開き、戻ってきたときに一覧を読み直す <sub>`frame template dialog store`</sub>
- フォルダを開けない環境(ブラウザ検証)では置き場のパスだけを案内する <sub>`frame template dialog store`</sub>

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
- 次の空きバリアントは登録済みでない最初の文字で、26文字すべて使っていれば無い <sub>`adding a variant to an existing macro from the save dialog`</sub>
- バリアントチップは登録済みのキーと次の空きキー1つを並べ、選んだキーが登録済みなら上書きの印が付く <sub>`adding a variant to an existing macro from the save dialog`</sub>
- 名前欄に入力すると、その文字を含む既存マクロが候補に出る <sub>`adding a variant to an existing macro from the save dialog`</sub>
- 候補から既存マクロを選ぶと、名前と分類がそのマクロのものになり、次の空きバリアントが選ばれる <sub>`adding a variant to an existing macro from the save dialog`</sub>
- 既存マクロへの保存は、そのマクロのidと選んだバリアントキーを渡す <sub>`adding a variant to an existing macro from the save dialog`</sub>
- 登録済みのバリアントを選んで保存すると、まず上書き確認を出し、確認してから保存する <sub>`adding a variant to an existing macro from the save dialog`</sub>
- 上書き確認で「別のバリアントに保存」を選ぶと、次の空きキーで保存する <sub>`adding a variant to an existing macro from the save dialog`</sub>
- 既存マクロの選択をやめると、従来どおり新しいマクロ (バリアントAだけ) として保存する <sub>`adding a variant to an existing macro from the save dialog`</sub>

### nameplates

- 部品をクリックして開くと、1行目は回路図の説明から始まり、説明への追従が付く <sub>`銘板の挿入`</sub>
- 挿入は部品に結び付いた銘板をadd_entity 1回で置き、何も無い所で開けば単独の銘板になる <sub>`銘板の挿入`</sub>

### 部品データベース

- searchは部品APIの結果を保持する <sub>`parts store`</sub>
- 検索失敗時は結果を空にしloadingを戻す <sub>`parts store`</sub>

### plcIo

- 三菱のアドレスは8進で数えるので X7 の次は X10 になる <sub>`PLC address numbering`</sub>
- Siemensのアドレスは「バイト.ビット」(1バイト8点) で数える <sub>`PLC address numbering`</sub>
- Allen-Bradleyのアドレスは「ワード/ビット」(1ワード16点) で数える <sub>`PLC address numbering`</sub>
- 自動採番の開始アドレスは何点目から始めるかを表す (2枚目のユニットは続き番号から振れる) <sub>`PLC address numbering`</sub>
- アドレス体系に合わない開始アドレスは受け付けない (採番できないため) <sub>`PLC address numbering`</sub>
- 開くと図面に置かれているPLCモジュールを読み込み、先頭のモジュールを表示する <sub>`PLC I/O assignment editor`</sub>
- グリッドはモジュールの点数ぶんの行になり、未割付の行にも自動採番の候補アドレスが出る <sub>`PLC I/O assignment editor`</sub>
- 割付表に保存済みの行はアドレス・信号名・コメントがそのまま表示される <sub>`PLC I/O assignment editor`</sub>
- 接続先と線番は図面から読んだ値で、編集はできない (読み取り専用の列) <sub>`PLC I/O assignment editor`</sub>
- 「自動採番で埋める」は開始アドレスから全行のアドレスを埋め、信号名とコメントは触らない <sub>`PLC I/O assignment editor`</sub>
- 開始アドレスを変えて自動採番すると、その番号から続けて振られる <sub>`PLC I/O assignment editor`</sub>
- 保存はset_plc_assignmentsコマンド1回で、編集した表がまとめてプロジェクトへ入る <sub>`PLC I/O assignment editor`</sub>
- 保存しても他のモジュールの行は順番も中身もそのまま残る <sub>`PLC I/O assignment editor`</sub>
- すでにある行はidを引き継ぐので、保存し直しても表の同一性が保たれる <sub>`PLC I/O assignment editor`</sub>
- 何も編集していなければ保存してもコマンドは送らない <sub>`PLC I/O assignment editor`</sub>
- 末尾の空行は保存しない (途中の空行は点番号がずれないよう残す) <sub>`PLC I/O assignment editor`</sub>
- CSVを読み込むと、そのモジュールの行だけがCSVの中身で置き換わる (undo一発で戻る) <sub>`PLC I/O assignment editor`</sub>
- 列数の合わないCSVはエラーになり、表は一切変わらない <sub>`PLC I/O assignment editor`</sub>
- 図面が外から変わったら (結線の追加やundo) 接続先・線番の列を取り直す <sub>`PLC I/O assignment editor`</sub>
- 編集の途中で図面が変わっても、まだ保存していない信号名は消えない <sub>`PLC I/O assignment editor`</sub>
- 編集していないときの外からの変更は、表そのものを読み直す <sub>`PLC I/O assignment editor`</sub>
- エディタを閉じている間は図面が変わっても読み込みに行かない <sub>`PLC I/O assignment editor`</sub>
- PLCモジュールが1つも置かれていなければ表は空になり、保存も生成もできない <sub>`PLC I/O assignment editor`</sub>
- 閉じると下書きと選択を捨てる <sub>`PLC I/O assignment editor`</sub>
- 生成設定の既定はJIS慣行の縦バス+横ラング・ラング間隔10mm・先頭スキップ0・新しいページ・分割の単位8点 <sub>`PLC I/O sheet generation settings`</sub>
- 生成は選んだ設定と、部品DBから引いたモジュール定義でI/O図面を作る。設定はその前にプロジェクトへ保存する <sub>`PLC I/O sheet generation settings`</sub>
- 横バス+縦ラングと「同居し必要なら分割」(分割の単位つき) でも生成できる <sub>`PLC I/O sheet generation settings`</sub>
- プロジェクトに保存した設定は、次に生成ダイアログを開いたときの初期値になり、変えずに生成すれば保存し直さない <sub>`PLC I/O sheet generation settings`</sub>
- 「プロジェクトに保存」を外して生成すると、設定は保存しない <sub>`PLC I/O sheet generation settings`</sub>
- 未保存の編集があれば、生成の前に割付表を保存する (図面のラベルが下書きと一致する) <sub>`PLC I/O sheet generation settings`</sub>
- 部品DBに無い型番のモジュールは、種別どおりの既定のアドレス体系 (三菱) で扱う <sub>`PLC I/O sheet generation settings`</sub>

### プロジェクトファイル (開く/保存/最近使ったファイル)

- 起動直後は読み込み済みの図面が「保存済み」の基準になり、未保存の編集は無い <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- 図面を編集すると未保存の編集ありになる <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- undoで保存時と同じ内容に戻しても未保存の印は消えない (revisionは戻らない) <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- ファイルを開くとそのパスが保存先になり、未保存の編集は無くなる <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- ファイルを開くとundo/redoの履歴は空になる (前の図面には戻れない) <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- 保存先が決まっていれば「保存」はダイアログ無しで同じファイルへ上書きする <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- 保存先が無ければ「保存」は何も書かずnullを返す (名前を付けて保存へ回す) <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- 「名前を付けて保存」は新しいパスへ書き、以後そのパスが保存先になる <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- 保存中に届いた編集はディスクに無いので、保存後も未保存の編集ありのまま <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- 新規プロジェクトは保存先が無く、未保存の編集も無い状態から始まる <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- KiCad回路図の読み込みは保存先が無く、内容は未保存の編集として扱う <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- DXFの読み込みも保存先が無く、内容は未保存の編集として扱う <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- 起動時に保管先から最近使ったファイルを読む <sub>`projectFile store: 最近使ったファイル`</sub>
- 開いたファイル・保存したファイルは一覧の先頭に入り、保管先へ書かれる <sub>`projectFile store: 最近使ったファイル`</sub>
- 同じファイルを開き直すと重複せず先頭へ移る <sub>`projectFile store: 最近使ったファイル`</sub>
- 一覧は上限件数までで、古いものから落ちる <sub>`projectFile store: 最近使ったファイル`</sub>
- 一覧から外したファイルは保管先からも消える <sub>`projectFile store: 最近使ったファイル`</sub>
- 保管データが重複していても起動時の一覧は重複なしになる <sub>`projectFile store: 最近使ったファイル`</sub>
- 保管データが上限を超えていても起動時の一覧は上限件数までになる <sub>`projectFile store: 最近使ったファイル`</sub>
- 書いた一覧はlocalStorageから読み戻せる (アプリを閉じても残る) <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- 保管が空なら一覧も空から始まる <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- 壊れたJSONが保管されていても空の一覧になる (起動を止めない) <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- 配列でない値が保管されていても空の一覧になる <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- 文字列でない要素は一覧から落とす <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- 保管できない環境 (プライベートモード・容量超過) でも例外にせず、一覧はセッション内だけ生きる <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- localStorageが無い実行環境でも一覧は空として扱う <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- パスからファイル名を取り出す (`/`区切りと`\`区切りの両方) <sub>`projectFile helpers`</sub>
- 拡張子が.dxf(大文字小文字を問わず)ならDXF図面とみなす <sub>`projectFile helpers`</sub>
- 拡張子が.kicad_sch(大文字小文字を問わず)ならKiCad回路図とみなす <sub>`projectFile helpers`</sub>

### projectInfo

- 開くと、プロジェクト名・番号と現在のシートの表題欄が下書きへ複製される <sub>`project info dialog store`</sub>
- 何も変えずに保存してもコマンドは送られない <sub>`project info dialog store`</sub>
- 変えた欄だけがset_project_metaコマンドに載り、他の欄はundefinedのまま(触っていないシートの値を壊さない) <sub>`project info dialog store`</sub>
- 名前・会社名・図番・表題をすべて変えれば、すべての欄がコマンドに載る <sub>`project info dialog store`</sub>
- プロジェクト番号を空にして保存すると、空文字ではなくnullを送って番号を未設定へ戻す <sub>`project info dialog store`</sub>
- キャンセルすると下書きが捨てられ、コマンドは送られない <sub>`project info dialog store`</sub>

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

- 帳票の種類は From-To電線リスト・端子台チャート・端子接続図・部品表・ケーブル一覧・XRef表・PLC I/Oレポート・盤内部品配置表・盤内配線図 の9つ <sub>`report generation dialog store`</sub>
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
- 図枠テンプレートのグローバル既定は初期状態では未設定(新しい図面は同梱のJIS A3図枠で始まる) <sub>`settings store`</sub>
- 図枠選択ダイアログの「グローバル既定」は設定として保存され、以降の表示にも残る <sub>`settings store`</sub>
- loadでバックエンドの設定を取り込む <sub>`settings store`</sub>
- saveは変更分をマージして送り、正規化後の戻り値を採用する <sub>`settings store`</sub>
- 知識ファイルのパスを保存でき、空文字はバックエンドが未設定へ正規化する <sub>`settings store`</sub>
- save失敗時はエラーを保持し、表示中の設定を変えない <sub>`settings store`</sub>

### sheetPaper

- 開くと、シートの現在の用紙サイズ・向きが下書きへ複製される <sub>`sheet paper dialog store`</sub>
- 何も変えずに保存してもコマンドは送られない <sub>`sheet paper dialog store`</sub>
- サイズ・向きを変えて保存すると、set_sheet_paperコマンドが1回送られる <sub>`sheet paper dialog store`</sub>
- キャンセルすると下書きが捨てられ、コマンドは送られない <sub>`sheet paper dialog store`</sub>

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
- 5つの並べ替え順はRust側`RowOrder`のバリアントと同じ5つ、同じ表記 <sub>`row order (parity with madake-core terminal_chart::RowOrder)`</sub>
- 未設定・未知語は数値順に落ちる(Rust側 row_order_of と同じ既定) <sub>`row order (parity with madake-core terminal_chart::RowOrder)`</sub>
- 5つの既知の語はそのまま対応する並びに解決される <sub>`row order (parity with madake-core terminal_chart::RowOrder)`</sub>
- 開くとそのシートの端子台一覧を読み込み、先頭の端子台のチャートを表示する <sub>`terminal strip editor store`</sub>
- グリッドは端子番号ごとに1行で、外部側・内部側・線番・電線が図面どおりに並ぶ <sub>`terminal strip editor store`</sub>
- グリッドの行は段記号(Lv)も運ぶ(単段は空欄、多段端子台は"a"/"b"のような文字) <sub>`terminal strip editor store`</sub>
- 端子台の一覧より先にチャートだけが読めているときは、行の段記号からLv列の要否を判定する <sub>`terminal strip editor store`</sub>
- 単段の端子台ではLv列は空欄で、多段端子台とは判定される <sub>`terminal strip editor store`</sub>
- 電線が1本も繋がっていない端子は予備端子として印が付く <sub>`terminal strip editor store`</sub>
- 隣り合う2端子を選んでジャンパを生成すると、update_entityコマンド1回で属性に書き込まれる <sub>`terminal strip editor store`</sub>
- 多段端子台の見出しには端子数ではなく極数を使う(4極2段は「8極」ではなく「4極」) <sub>`terminal strip editor store`</sub>
- 端子台の一覧より先にチャートだけが読めているときは、極数をチャートの行から数える(4極2段のチャートなら「4極」) <sub>`terminal strip editor store`</sub>
- チャートも端子台の一覧も読めていなければ極数は0極になる <sub>`terminal strip editor store`</sub>
- 端子を選んでいないとジャンパは生成できない <sub>`terminal strip editor store`</sub>
- 隣り合わない端子を選んでもジャンパは生成できない <sub>`terminal strip editor store`</sub>
- ジャンパ削除は選んだ端子に掛かるジャンパだけを外したコマンドになる <sub>`terminal strip editor store`</sub>
- ジャンパが1本も残らないときは属性ごと消す <sub>`terminal strip editor store`</sub>
- ジャンパを編集したらチャートを読み直し、グリッドが図面と一致し続ける <sub>`terminal strip editor store`</sub>
- 並べ替え順を変えると、update_entityコマンド1回で属性(row_order)に書き込まれる <sub>`terminal strip editor store`</sub>
- 既定の数値順を選び直すと、row_order属性ごと消す(未設定の端子台と同じ状態に戻す) <sub>`terminal strip editor store`</sub>
- 現在のチャートの並べ替え順をそのまま見せる(サーバ側で並べ替え済み) <sub>`terminal strip editor store`</sub>
- 「図面へジャンプ」は表示中の端子台のシート・entity idをrevealへそのまま渡す <sub>`terminal strip editor store`</sub>
- 「図面へジャンプ」はエディタを閉じてから図面を出す(全画面モーダルの下にジャンプ先が隠れないように) <sub>`terminal strip editor store`</sub>
- ジャンプで閉じたエディタを開き直すと、同じ端子台と同じ行選択に戻る <sub>`terminal strip editor store`</sub>
- ×やスクリムで閉じたときは覚え書きを残さない(次に開くと先頭の端子台・選択なし) <sub>`terminal strip editor store`</sub>
- 並べ替えを変えたときの読み直しは1回だけ(自前の読み直しと図面のrevision通知で二重に取りに行かない) <sub>`terminal strip editor store`</sub>
- 端子台が無ければ「図面へジャンプ」は何もしない <sub>`terminal strip editor store`</sub>
- 図面が外から変わったら (undoやAIの編集) グリッドを取り直し、古いチェック結果は捨てる <sub>`terminal strip editor store`</sub>
- 端子台エディタを閉じている間は図面が変わっても読み込みに行かない <sub>`terminal strip editor store`</sub>
- 端子台チェックの結果は重大度ごとの件数に整形される <sub>`terminal strip editor store`</sub>
- 問題が1件も無ければチェックは「問題なし」になる <sub>`terminal strip editor store`</sub>
- 端子台を切り替えると選択と直前のチェック結果は消える <sub>`terminal strip editor store`</sub>
- 端子台が1つも無いシートでは行が空になり、生成もチェックもできない <sub>`terminal strip editor store`</sub>
- 閉じると選択・チャート・チェック結果を捨てる <sub>`terminal strip editor store`</sub>
- プレビューは旧番号→新番号の対応として並び、多段端子台では段記号を保ったまま出る <sub>`renumber popover`</sub>
- 増分0の採番は全端子を同じ番号にしてしまうので、重複するラベルと件数を理由として返す <sub>`renumber popover`</sub>
- 適用できない案はコマンドを1本も送らずに断られる (図面は変わらない) <sub>`renumber popover`</sub>
- 負の開始番号・小数の増分は0以上の整数へ丸めてから送る (JSONの型エラーにしない) <sub>`renumber popover`</sub>
- 端子台が1つも無ければ採番の対象が無いことを理由として返す <sub>`renumber popover`</sub>
- プレビューは採番に追従したあとのジャンパ指定も見せる (適用と同じ書き換え規則) <sub>`renumber popover`</sub>
- ジャンパが1本も無ければ追従の行は出さない <sub>`renumber popover`</sub>
- 段数は一覧の`levels`をそのまま見せ、単段は1になる <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- 一覧より先にチャートだけが読めているときは、行の段記号の種類数から段数を数える <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- 段数を変えると、set_terminal_levelsコマンド1回だけが送られる <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- 今と同じ段数を指定したら何もコマンドを送らない (無駄なundoの段を作らない) <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- 1〜TERMINAL_LEVEL_MAXの範囲外は、コマンドを送らずに断る (ステッパーの+/-ボタンが外へ出さないための門と同じ判定) <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- Rust側が段数変更を拒否したら (消える段に参照が残っている等)、その理由を`error`へ残す (図面は変えない) <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- プレビューは適用と同じ純関数で symbol_id・端子番号・ジャンパの追従を見せる (デザイン「P 段数 (既定)」の3行) <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- 今と同じ段数のプレビューは出さない (何も変わらないので) <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- 段を減らせないときは、消える段の端子とその結線相手を理由付きで並べる (デザイン「P 段数 (解除不可)」の囲み) <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- 消える段を指すジャンパも、結線と同じように解除を止める理由として並ぶ <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- 段を増やすときは消える段が無いので、理由の一覧は常に空 (適用ボタンは有効のまま) <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- 理由が1件でもある段数はコマンドを送らずに断る (ポップオーバーが適用ボタンを無効にしている門と同じ判定) <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- 予備を挿入すると、insert_spare_terminalsコマンド1回だけが送られる <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- 末尾挿入は`after`がnullのコマンドになる <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- 「空き番号のまま残す」で末尾に挿入すると、後ろの番号を変えずに次の空き番号 (9) が予備に付き、renumber_following=false で送られる <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- 「空き番号のまま残す」で途中に挿入して番号がぶつかるときは keepDuplicate の指摘になり、コマンドは送られない <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- 0個以下の指定は、コマンドを送らずに断る <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- フロントの検証を通っても、Rust側が挿入を拒否したら (エンティティが図面から消えていた等) その理由をerrorへ残す (図面は変えない) <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- プレビューは適用と同じ純関数で symbol_id と新しく増える予備のラベルを見せる (デザイン「P 予備端子の挿入 (既定)」の2行) <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- 0個以下の指定は指摘 (適用ボタンを無効にする) <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- 極数の上限を超える指定も指摘になる <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- 挿入位置の選択肢は極ごとに1つ、多段端子台では段aのラベルを代表にする <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- 個数ステッパーの入力は1以上の整数へ丸める <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- 付属品の行は「端子番号:種別:型番」のカンマ列になり、型番の前後の空白と書式を壊す「,」「:」は取り除かれる <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- 付属品が1つも無いときは空文字になる (ストアは属性ごと消す) <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- チャートの付属品は、末尾 (before=null) を "end" にしてポップオーバーの行へ写る <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- 部品割当は update_entity 1回で、端子台の型番 (value) と種別 (attrs.terminal_kind) を同時に書く <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- 種別の無い部品を割り当てると、古い種別の属性は消える <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- 付属品を全部消して適用すると、accessories属性そのものが消える <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- 付属品の行を適用すると、書式どおりの accessories 属性を update_entity 1回で書く <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- 選んだ端子のうち、入れ替わっていなかった端子は内外スワップの指定に加わり、入れ替わっていた端子は外れる <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>
- 内外スワップの指定にある端子台に無い番号や余分な空白・空の項は、反転しても残るか取り除かれるだけで壊れない <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>
- 端子を選んでいないと内外スワップはできず、コマンドも送られない <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>
- 内外スワップは選んだ端子の番号を swapped 属性へ書く update_entity 1回で、選択はそのまま残る <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>
- 入れ替え済みの端子をすべて戻すと、swapped 属性そのものが消える <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>
- グリッドの行は、チャートで内外を入れ替えた端子かどうかを運ぶ (古いチャートで欠けていれば入れ替えなし) <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>

### ui

- 既定はプロジェクトタブ+チャット折りたたみ <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- openAgentTabでエージェントタブ切替とチャット展開が同時に行われる <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- closeAgentTabでプロジェクトタブへ戻り、チャットも畳まれる <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- チャット下書きはドックと浮きカードで共有される <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- 既定では全表示クラスが表示される <sub>`表示クラス (レイヤ)`</sub>
- toggleViewClassで表示クラスの非表示・再表示を切り替える <sub>`表示クラス (レイヤ)`</sub>
- 起動直後のリボンは回路図タブ <sub>`シート種別に合わせた画面の切り替え`</sub>
- 盤レイアウトシートを開くとリボンのパネルタブと左ドックの未配置部品が自動で開く <sub>`シート種別に合わせた画面の切り替え`</sub>
- 回路図シートへ戻すと回路図タブとプロジェクトツリーへ戻る <sub>`シート種別に合わせた画面の切り替え`</sub>
- 自分で開いた表示タブは盤シートを開いても勝手に切り替わらない <sub>`シート種別に合わせた画面の切り替え`</sub>
- エージェントと会話中は左ドックのタブを取り上げない <sub>`シート種別に合わせた画面の切り替え`</sub>

### updates

- 起動時の確認で新しい版が見つかると、その版と公開日・リリースノートを持って右下の通知を出す <sub>`updates store`</sub>
- 設定で起動時の確認をオフにしていれば確認しない。開発ビルドとブラウザ検証モードでも確認しない <sub>`updates store`</sub>
- 起動時の確認が通信できずに失敗しても、何も表示しない <sub>`updates store`</sub>
- 「今すぐ確認」は、最新なら「最新です」、失敗なら理由を出す <sub>`updates store`</sub>
- 「更新して再起動」はダウンロードの進み具合を数え、入れ替えが済んだらアプリを再起動する <sub>`updates store`</sub>
- 未保存の図面があるときは先に確認し、断られたらダウンロードしない <sub>`updates store`</sub>
- ダウンロードや入れ替えに失敗したら理由を出し、再起動しない <sub>`updates store`</sub>
- 「後で」を押した版は、この起動中は通知を出さない <sub>`updates store`</sub>
- Linuxの.deb/.rpmでは入れ替えず、リリースページを開くだけにする <sub>`updates store`</sub>

### 整えバリアント (シート複製 / 採用)

- 開始すると表示中のシートが案の数だけ複製され、案ごとに別の会話で同じ整え指示が送られ、比較パネルが開く <sub>`variants store: 整え案の開始`</sub>
- 選択があれば、その要素の対応する複製側の要素だけを整える範囲として指示する <sub>`variants store: 整え案の開始`</sub>
- 開始直後に元の図面と各案の指標が取られ、案ごとに自分のシートの数値になる (元シートの数値にすり替わらない) <sub>`variants store: 整え案の開始`</sub>
- 案ごとのプロンプトは、測るときも検証するときも自分の複製シートのsheet_idを指定させる <sub>`variants store: 整え案の開始`</sub>
- シートが無い・案の数が2〜4以外・比較中は開始しない <sub>`variants store: 整え案の開始`</sub>
- 複製に失敗したら理由をログに出し、何も始まらない <sub>`variants store: 整え案の開始`</sub>
- 案の会話が答えている間は「実行中」で、全部終わると採用できる <sub>`variants store: 比較・採用・破棄`</sub>
- 採用すると選んだ案が終了IPCへ渡り、元の図面へ戻って比較は終わる (undo一発で戻せる旨をログ) <sub>`variants store: 比較・採用・破棄`</sub>
- 破棄は実行中の案の会話を止めてから全複製を消す <sub>`variants store: 比較・採用・破棄`</sub>
- 終了に失敗したら理由をログに出し、比較はそのまま続く <sub>`variants store: 比較・採用・破棄`</sub>
- 開始をCmd+Zで戻して複製シートが消えたら、比較は理由つきで片付けられ、採用も破棄もしない <sub>`variants store: 比較・採用・破棄`</sub>
- 表示中のシートが案の複製なら、その案を走らせている会話が分かる (編集オーバーレイの色に使う) <sub>`variants store: 比較・採用・破棄`</sub>
- パネルは閉じても比較は続き、開き直せる。比較が無ければ開かない <sub>`variants store: 比較・採用・破棄`</sub>
- 指標の取り直しで取れなかった案は「指標なし」になる <sub>`variants store: 比較・採用・破棄`</sub>

### verification

- runで診断を取得してパネルを開き、severity別に件数を数える <sub>`verification store`</sub>
- closeはパネルを閉じるが診断は保持する <sub>`verification store`</sub>
- 取得失敗時はrunningが戻り、診断は空のまま <sub>`verification store`</sub>

### blockPlacement

- ブロックの配置ゴーストは、ピン表どおりの形(接続点の数)で出る <sub>`placing a block with a pin table`</sub>
- 配置を確定すると、ピン表の属性を持ったブロックが1回のadd_entityで追加される <sub>`placing a block with a pin table`</sub>
- 縦長のブロックは外形の中ならどこをクリックしても選べる(固定半径では届かない下端も) <sub>`picking a symbol on the canvas`</sub>
- 外形の外をクリックしてもブロックは選ばれない <sub>`picking a symbol on the canvas`</sub>
- 小さなシンボルもその外形の中で選べる <sub>`picking a symbol on the canvas`</sub>
- 小さなシンボルから離れたところをクリックしても選ばれない(外形で判定するため) <sub>`picking a symbol on the canvas`</sub>
- 実機ブロックで表示したコイルは、描かれているブロックの外形の中ならどこをクリックしても選べる (G7) <sub>`picking a symbol on the canvas`</sub>
- 属性が壊れていて描けないブロックも、置いた位置の近くをクリックすれば選べる(直せる) <sub>`picking a symbol on the canvas`</sub>

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

### panelPlacement

- 盤枠ツールは矩形ドラッグで盤の外形を1つ作る (undo一発で消える)。 <sub>`panel drawing tools on a panel sheet`</sub>
- DINレールツールは水平ドラッグでレールを1本作る。 <sub>`panel drawing tools on a panel sheet`</sub>
- ダクトツールは矩形ドラッグでダクトを1つ作る。 <sub>`panel drawing tools on a panel sheet`</sub>
- 作図をやめる (Esc) と選択ツールへ戻り、コマンドは送られない。 <sub>`panel drawing tools on a panel sheet`</sub>
- 未配置部品を選ぶとフットプリントの配置モードに入る (ゴーストがカーソルに付く)。 <sub>`placing an unplaced part`</sub>
- クリックでフットプリントが確定し、レールの近くなら取付線へ吸い付く。 <sub>`placing an unplaced part`</sub>
- 配置中のRは向きを0度と90度で入れ替える (盤の描画はこの2つだけを持つ)。 <sub>`placing an unplaced part`</sub>
- 置いたあとは選択ツールへ戻る (同じ部品を2台置いてしまわない)。 <sub>`placing an unplaced part`</sub>
- 盤の要素 (フットプリント・レール・ダクト・盤枠) はクリックで選べる。 <sub>`placing an unplaced part`</sub>
- 置いたフットプリントをレールの近くへ動かすと、移動もレールへ吸い付く。 <sub>`placing an unplaced part`</sub>
- 盤レイアウトシートでは回路図のツール(配線・部品挿入)へ切り替わらず、理由をログに出す。 <sub>`tools that do not belong to the sheet`</sub>
- 回路図シートでは盤のツール(盤枠・レール・ダクト)へ切り替わらない。 <sub>`tools that do not belong to the sheet`</sub>
- 回路図シートでは未配置部品の配置も始まらない (置き場所が無い)。 <sub>`tools that do not belong to the sheet`</sub>
- 盤シートでも選択・移動・削除はこれまでどおり使える。 <sub>`tools that do not belong to the sheet`</sub>

### shapePlacement

- 矩形ツールは1回のドラッグで矩形を1つ作る(undo一発で消える) <sub>`drawing shapes`</sub>
- 線ツールと円ツールも1回のドラッグで1つ作る(円は押した点が中心) <sub>`drawing shapes`</sub>
- クリックしただけ(ドラッグしていない)ときは何も作らない <sub>`drawing shapes`</sub>
- 折れ線はクリックを重ねて打ち、ダブルクリックで確定する <sub>`drawing shapes`</sub>
- 折れ線はEnterでも確定でき、点が1つだけなら何も作らずに捨てる <sub>`drawing shapes`</sub>
- 図形ツールは盤レイアウトシートでは使えない(回路図の作図要素なので) <sub>`drawing shapes`</sub>
- 図形は輪郭で掴む(塗りの下の配線を選びたいので、面の中では掴まない) <sub>`selecting shapes`</sub>
- 図形を移動すると全ての頂点が動く(move_entitiesコマンド1本) <sub>`selecting shapes`</sub>

### wireLegendPlacement

- 電線品番凡例ツールは、クリックした位置を表の左上にしてブロックを1つ置く (置いたらツールは選択へ戻る)。 <sub>`wire legend block on the canvas`</sub>
- 表の矩形の中をクリックすると凡例ブロックが選べ、外をクリックすると選べない。 <sub>`wire legend block on the canvas`</sub>
- 選んだ凡例ブロックはドラッグで動かせる (置き場所だけが動き、中身は動かした先のシートから導出し直される)。 <sub>`wire legend block on the canvas`</sub>


## FreeCADアドオン (MadakeCAD Link)


### Link APIクライアント (Python)

- クライアントはローカルのMadakeCAD Link API(既定ポート9310)へ接続する。 <sub>`test_base_url_points_at_the_local_link_api`</sub>
- 接続確認とプロジェクトのスナップショットはJSONを解釈した辞書で返る。 <sub>`test_health_and_project_are_decoded_from_json`</sub>
- ネットリストと部品の要求は、シートid・検索語・カテゴリをクエリパラメータで渡す。 <sub>`test_netlist_and_parts_pass_their_filters_as_query_parameters`</sub>
- コマンドはJSON配列として送られ、コマンドごとにpatchが1つ返る(全ての書き込みがMadakeCADのCommandエンジンを通る)。 <sub>`test_commands_are_posted_as_a_json_array_and_return_one_patch_each`</sub>
- batch指定で送るとコマンド配列全体を1回の編集として実行させる(?batch=true)ので、パネルの1操作はMadakeCADでundo1回で戻る。 <sub>`test_a_batch_post_asks_madakecad_for_a_single_undo_step`</sub>
- 読み取り中に切れたストリーム(送信途中の終了・ソケットのリセット・無音)は、ポートと再接続の案内が入った文面で報告され、生のソケットエラーは出さない。 <sub>`test_errors_while_reading_the_event_stream_become_readable_link_errors`</sub>
- 接続拒否・HTTPエラー・不正なJSONはいずれも、原因が読める文面のLinkErrorになる。 <sub>`test_errors_are_reported_as_link_errors_with_a_readable_message`</sub>
- イベントストリームはpatchごとに("patch", {revision, ops})を返し、複数行のdataは結合し、keep-aliveのコメント行は読み飛ばす。 <sub>`test_events_yield_patches_from_the_sse_stream`</sub>
- イベント名の無い行は"message"イベントになり、dataは空行で確定し、末尾の空行が無いイベントも届く。 <sub>`test_parse_sse_defaults_the_event_name_and_dispatches_on_blank_lines`</sub>

### ライブ追従・設定

- patchは前回より新しいrevisionのときだけ受け入れられ、重複や古いpatchは無視される。 <sub>`test_only_newer_revisions_are_accepted`</sub>
- 表示中シートの要素が変わったとき、またはシートの追加・削除・改名やプロジェクト置換のときにパネルは再読込し、別シートの編集では再読込しない。 <sub>`test_refresh_is_needed_for_the_shown_sheet_and_structural_changes`</sub>
- シートをまだ選んでいない間は、どの要素の変更でも再読込して最初の表示が最新になるようにする。 <sub>`test_with_no_sheet_shown_yet_any_entity_change_reloads`</sub>
- MadakeCAD側で3D対応付けを追加・置換・undoすると、シート表示中でもパネルを再読込するので、対応付け列・配置列に消えた対応付けが残らない。 <sub>`test_a_change_to_the_freecad_links_reloads_whatever_sheet_is_shown`</sub>
- opの範囲はsheet_idか持っているsheetのidで決まり、シートを示さないopはプロジェクト全体として常に再読込する(新しい全体opを取りこぼさない)。 <sub>`test_an_op_that_names_no_sheet_counts_as_project_wide`</sub>
- ポート設定は1〜65535の整数を保持し、それ以外(文字列・0・大きすぎる値)は既定の9310へ戻る。 <sub>`test_port_setting_accepts_valid_ports_and_falls_back_to_the_default`</sub>

### 部品対応付け・電線長書き戻し

- 部品リストはシンボルごとに1行(シート・参照記号順)で、型番の3Dモデルと対応付け済みのFreeCADオブジェクト名を持つ。 <sub>`test_part_rows_list_symbols_with_their_3d_model_and_linked_object`</sub>
- 配線リストは配線ごとに1行で、ネット/線番・現在の長さ・長さの出所・対応付け済みの経路オブジェクトを持つ。 <sub>`test_wire_rows_show_net_length_source_and_linked_route`</sub>
- STEP/IGES/BREPはシェイプとして挿入、.FCStdはドキュメントとして取り込み、それ以外は挿入できない。 <sub>`test_model_kind_recognises_shape_files_and_freecad_documents`</sub>
- 対応付けはentity id・FreeCADドキュメントのパス・オブジェクト名・同期時刻を持つset_mech_linkコマンドを送り、解除はremove_mech_linkを送る。 <sub>`test_link_commands_carry_the_entity_id_document_path_object_name_and_time`</sub>
- 対応付けをやり直しても、すでにFreeCADから同期済みの配置は保たれるので、モデルの挿入や再リンクで盤レイアウトの位置が捨てられない。 <sub>`test_relinking_keeps_the_placement_freecad_already_synced`</sub>
- 経路オブジェクトはmadake_idで配線と突き合わせ、長さはmmからm(1mm単位)へ換算し、配線でないオブジェクトは無視する。 <sub>`test_measured_route_lengths_are_converted_to_metres_and_matched_to_wires`</sub>
- 書き戻しはシートごとに1つのset_wire_lengthsコマンドで、各項目はm単位の長さと出所"freecad"を持つ(MadakeCADが手入力の上書きを警告できる)。 <sub>`test_write_back_is_one_command_per_sheet_with_the_freecad_source`</sub>
- 計測時に経路オブジェクトを配線の対応付けとして登録するが、すでに同じオブジェクトに対応付いている配線は登録し直さない。 <sub>`test_routes_are_linked_only_when_not_already_linked_to_that_object`</sub>
- 配線の経路オブジェクトを別のものに付け替えても、その配線に同期済みの配置は保たれるので、計測でFreeCADがマスタのジオメトリが消えない。 <sub>`test_relinking_a_route_to_another_object_keeps_its_synced_placement`</sub>

### プロジェクト概要・ネットリスト表示

- プロジェクト概要にはプロジェクト名・revisionと、シートごとのシンボル・配線などの件数が入る。 <sub>`test_summary_carries_name_revision_and_per_sheet_entity_counts`</sub>
- 空または欠けたスナップショットは、失敗せずにシート無しの無名プロジェクトとして要約される。 <sub>`test_an_empty_snapshot_summarizes_to_zero`</sub>
- ネットリストの各行には、ネット名・線番・ラベル・「K1:A1, TB1:3」形式のピン一覧・配線本数が並ぶ。 <sub>`test_rows_show_net_name_wire_number_label_pins_and_wire_count`</sub>
- 参照記号の無いシンボルのピンは「?:<ピン番号>」と表示され、行はそのまま読める。 <sub>`test_pins_without_a_designator_show_a_placeholder`</sub>

### 経路同期・ネットハイライト・配置

- 両端の部品が対応付け済みの配線には、一方の部品の配置から他方の配置へ向かう経路スタブが1本でき、配線idとネット名を持つ。 <sub>`test_a_wire_between_two_linked_parts_gets_one_stub_between_their_placements`</sub>
- 部品が対応付けられていないネットや、対応付け先のオブジェクトがFreeCADドキュメントに無いネットにはスタブができない。 <sub>`test_parts_without_a_link_or_not_in_the_document_produce_no_stub`</sub>
- 1ネットに対応付け済み部品が3つあると名前順に2区間の鎖になり、ネットの配線へ順に割り当てる。余った配線にはスタブを作らず、配線が足りなければ余った区間は捨てる。 <sub>`test_a_net_with_three_parts_is_chained_and_segments_follow_the_wire_ids`</sub>
- すでに経路オブジェクトのある配線は飛ばすので、スタブ生成を再実行しても重複しない。 <sub>`test_rerunning_skips_wires_that_already_have_a_route_object`</sub>
- 両端の部品が同じ位置にあるスタブは線にできないので、利用者へ報告する側へ分け、残りのスタブは作る。 <sub>`test_stubs_between_parts_at_the_same_position_are_reported_instead_of_drawn`</sub>
- 両端が1/1000mmより近ければ同じ点として扱い(FreeCADが線を作れない)、それより離れていれば線を作る。 <sub>`test_ends_a_thousandth_of_a_millimetre_apart_still_count_as_the_same_point`</sub>
- ネットのハイライトは、対応付け済み部品と経路の各オブジェクトを1回ずつ選び、未対応付けのピンは無視する。 <sub>`test_highlighting_a_net_selects_its_linked_parts_and_routes_once_each`</sub>
- 配置の同期はset_mech_linkで基点をmm(1µm単位)・回転を度で送り、既存の対応付け情報は保つ。 <sub>`test_placement_command_carries_the_base_in_mm_and_the_rotation_in_degrees`</sub>
- 配置の同期はドキュメントにある対応付け済みオブジェクトごとに1コマンド送り、保存済みの配置と同じものは飛ばす。 <sub>`test_sync_placements_writes_only_linked_objects_whose_placement_changed`</sub>
- Z軸まわりの回転は軸の向きの符号つきで度で保存し、別の軸に傾いた部品は盤の回転を持たないので、その角度として保存せず報告する。 <sub>`test_only_a_rotation_about_the_z_axis_is_stored_as_a_panel_rotation`</sub>

