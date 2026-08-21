# 詳細仕様設計書

**English (canonical): [13-specification.md](13-specification.md)** | ← [ロードマップ](12-roadmap.ja.md)

> **テストスイートから自動生成 — 手で編集しないこと。**
> 以下の各項目は自動テストで常に検証されている(灰色はテストID)。
> テスト変更後は `python3 scripts/gen_spec.py` で再生成する。

本書はMadakeCADの「生きた仕様書」である:
ここに載っている挙動は、テスト実行のたびに証明される。


全5領域・**300仕様項目**。


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
- execute/undo/redoのたびにドキュメントrevisionが増加し、クライアントは古いpatchを破棄できる。 <sub>`revision_increases_monotonically`</sub>
- 存在しないシートへのCommandはエラーになり、何も変更されない。 <sub>`unknown_sheet_is_rejected`</sub>
- 履歴が空のときのundoはエラーではなくNoneを返す。 <sub>`undo_on_empty_history_returns_none`</sub>
- undo後に新しい編集をするとredo履歴は消える(一般的なエディタと同じ挙動)。 <sub>`new_edit_after_undo_clears_redo`</sub>

### 座標・ジオメトリ

- snapped()は座標を最も近いグリッドピッチ(既定2.5mm)へ丸め、すべてがピングリッドに乗る。 <sub>`snapped_rounds_to_grid_pitch`</sub>
- translated()は平行移動したコピーを返し、元の点を変更しない。 <sub>`translated_shifts_without_mutation`</sub>
- distance_to()はユークリッド距離である(3-4-5の直角三角形で5になる)。 <sub>`distance_is_euclidean`</sub>

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

### 帳票 (部品表 / 電線リスト)

- 部品表はシンボルを型番でまとめ、数量を集計する。 <sub>`bom_groups_by_value_and_counts`</sub>
- カンマを含む項目は引用符で囲まれ、CSVが壊れない。 <sub>`bom_escapes_fields_with_commas`</sub>
- 電線リストには各ワイヤの品番・線色・線径・長さが載る。 <sub>`wire_list_contains_attributes`</sub>

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
- 回転したシンボルは形状ごと回転して描かれる(90度で抵抗の本体が縦長になる)。 <sub>`svg_renders_rotated_symbol_primitives`</sub>

### シンボルライブラリ

- 同梱シンボルはすべて一意のidを持ち、最低1つのピンを持つ。 <sub>`builtin_symbols_have_unique_ids_and_pins`</sub>
- ピン数可変の端子台(例: 8極)は端子ごとに左右1点ずつの接続点を持ち、すべて2.5mmグリッド上・上下中央揃えになる。 <sub>`dynamic_terminal_block_has_through_pins_on_grid`</sub>
- 動的生成のconnector_2pは旧静的定義と完全に同じピン座標を持ち、既存図面に影響しない。 <sub>`dynamic_connector_2p_matches_legacy_static_def`</sub>
- resolve_symbolは同梱idを見つけ、不正・範囲外の動的ID(0極・51極・数値なし)は拒否する。 <sub>`resolve_symbol_rejects_invalid_ids_and_finds_builtins`</sub>
- sheet_symbol_defsは同梱ライブラリに加え、シートで実際に使われている動的シンボルの定義を返す。 <sub>`sheet_symbol_defs_includes_dynamic_ids_in_use`</sub>
- シンボル定義はJSONに往復変換しても失われない。 <sub>`symbol_json_roundtrip`</sub>

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

### viewClasses

- エンティティ種別を表示クラスへ対応付ける(ジャンクションは配線扱い) <sub>`entityViewClass`</sub>
- VIEW_CLASSESは全クラスを一意に列挙する <sub>`entityViewClass`</sub>

### viewport

- ワールド⇔スクリーン変換は往復可能で、ズームはアンカー点を固定する <sub>`Viewport`</sub>
- パンはスクリーンpx単位でビューを動かす <sub>`Viewport`</sub>
- スナップは既定で2.5mmグリッドへ丸める <sub>`Viewport`</sub>
- ズーム倍率は妥当な範囲に制限される <sub>`Viewport`</sub>

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

