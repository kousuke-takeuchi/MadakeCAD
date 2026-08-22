# Detailed Specification

**日本語: [13-specification.ja.md](13-specification.ja.md)** | ← [Roadmap](12-roadmap.md)

> **Generated from the test suite — do not edit by hand.**
> Every clause below is enforced by an automated test; the test id is shown in gray.
> Regenerate with `python3 scripts/gen_spec.py` after changing tests.

This document is the living, always-verified specification of MadakeCAD:
if a behavior is listed here, a test proves it on every run of the suite.


**570 specification clauses** across 5 areas.


## Core domain (madake-core)


### Command engine (undo/redo)

- Adding an entity can be undone (the entity disappears) and redone (it comes back). <sub>`add_undo_redo_entity`</sub>
- Moving entities shifts their coordinates; undo restores the original position exactly. <sub>`move_and_undo_restores_position`</sub>
- Deleting several entities at once is a single undo step that restores all of them. <sub>`delete_multiple_and_undo`</sub>
- Sheets can be added and removed; undoing a removal restores the sheet with its original id and position. <sub>`sheet_add_remove_undo`</sub>
- Commands serialize to JSON and back without loss, so any client can send them over the wire. <sub>`command_json_roundtrip`</sub>
- UpdateEntity replaces an entity wholesale; undo brings back the previous version. <sub>`update_entity_undo_restores_previous_version`</sub>
- SetTitleBlock updates the sheet's title block; undo restores the previous fields. <sub>`set_title_block_is_undoable`</sub>
- SetRevisions replaces the revision-table rows; undo restores the previous list. <sub>`set_revisions_is_undoable`</sub>
- A harness boundary is added, renamed and deleted with the ordinary entity commands, and every step can be undone. <sub>`harness_add_rename_delete_are_undoable`</sub>
- Moving a harness boundary shifts all of its corners, and undo puts them back exactly. <sub>`harness_move_and_undo_restores_every_corner`</sub>
- A harness boundary survives a JSON round trip with the kind tag "harness", so any client can send it. <sub>`harness_command_json_roundtrip_uses_the_harness_kind`</sub>
- Every execute/undo/redo increases the document revision, so clients can discard stale patches. <sub>`revision_increases_monotonically`</sub>
- Commands targeting a non-existent sheet fail with an error and change nothing. <sub>`unknown_sheet_is_rejected`</sub>
- Undo with an empty history returns None instead of an error. <sub>`undo_on_empty_history_returns_none`</sub>
- A new edit after undo clears the redo history (standard editor behavior). <sub>`new_edit_after_undo_clears_redo`</sub>

### Geometry & coordinates

- snapped() rounds a coordinate to the nearest grid pitch (default 2.5 mm), so everything lands on the pin grid. <sub>`snapped_rounds_to_grid_pitch`</sub>
- translated() returns a shifted copy and never mutates the original point. <sub>`translated_shifts_without_mutation`</sub>
- distance_to() is the Euclidean distance (a 3-4-5 triangle measures 5). <sub>`distance_is_euclidean`</sub>

### Harness boundaries

- A wire belongs to a harness only when every one of its points is inside the boundary. <sub>`a_fully_enclosed_wire_belongs_to_the_harness`</sub>
- A wire that touches the boundary line is still counted as inside (the line itself belongs to the harness). <sub>`a_wire_on_the_boundary_line_still_belongs`</sub>
- A wire that only partly overlaps the boundary does not belong to the harness. <sub>`a_partly_overlapping_wire_does_not_belong`</sub>
- A wire drawn completely outside the boundary does not belong to the harness. <sub>`a_wire_outside_the_boundary_does_not_belong`</sub>
- Looking up the harness of a wire returns the harness name, or an empty string when it belongs to none. <sub>`harness_name_lookup_is_empty_for_unassigned_wires`</sub>
- When harnesses are nested, a wire belongs to the smallest boundary that encloses it. <sub>`a_nested_harness_wins_over_the_outer_one`</sub>
- The wire count of a harness reports how many wires it currently encloses. <sub>`wire_count_reports_the_enclosed_wires`</sub>
- A new harness is named W1 on an empty sheet and takes the next free number after existing ones. <sub>`the_next_harness_name_continues_the_w_series`</sub>
- A rectangle drag produces four corner points regardless of the direction it was dragged in. <sub>`rect_points_normalize_the_drag_direction`</sub>

### Project file I/O (.mdkproj)

- A project saved to .mdkproj and loaded back is identical, including all entities. <sub>`project_file_roundtrip`</sub>
- The saved .mdkproj file is pretty-printed JSON with a format_version field, so it diffs well in git. <sub>`saved_file_is_pretty_json_with_format_version`</sub>
- Loading a missing file returns an error instead of panicking. <sub>`loading_missing_file_is_an_error`</sub>

### KiCad import

- KiCad import converts paper size, title block, wires, junctions, labels (power symbols become net labels) and text. <sub>`imports_paper_title_block_and_geometry`</sub>
- Known lib_ids map to our symbols (Device:R -> resistor, Conn_01x03 -> connector_3p) keeping designator/value/rotation/mirror; unknown symbols are skipped and itemized in the report. <sub>`maps_symbols_and_reports_skipped`</sub>
- A file that is not a kicad_sch document is rejected with a clear error. <sub>`rejects_non_schematic`</sub>
- The S-expression parser reads atoms, quoted strings, numbers and nested lists with typed accessors. <sub>`parses_atoms_strings_numbers_and_nesting`</sub>
- Escaped quotes/newlines and multibyte (Japanese) text inside strings parse correctly. <sub>`parses_escaped_strings_and_multibyte`</sub>
- children(name) iterates every child list with the given head symbol. <sub>`children_iterates_all_matches`</sub>
- Unbalanced parentheses and unterminated strings are reported as syntax errors with a position. <sub>`syntax_errors_are_reported`</sub>

### Document model

- Paper sizes follow ISO A-series dimensions, and portrait orientation swaps width and height. <sub>`paper_sizes_match_iso_and_orientation_swaps`</sub>
- A new project starts with one sheet named "Sheet1" and format_version 1. <sub>`new_project_has_one_default_sheet`</sub>
- Entity::translate moves every coordinate of the entity: all wire points, or the anchor of symbols/labels/text. <sub>`translate_moves_all_coordinates`</sub>
- Entity::id() returns the inner entity's UUID regardless of the entity kind. <sub>`entity_id_is_uniform_across_kinds`</sub>

### Netlist extraction

- A wire whose endpoints touch two symbol pins joins those pins into one net. <sub>`wires_connect_symbol_pins_into_one_net`</sub>
- Wires that merely cross do NOT connect; a net label attached to a wire names its net. <sub>`crossing_without_junction_stays_separate_and_label_names_net`</sub>
- A junction dot connects crossing wires, and same-named labels merge distant nets into one. <sub>`junction_connects_crossing_wires_and_same_labels_merge`</sub>
- Unnamed nets receive deterministic sequential names (N001, N002, ...). <sub>`unnamed_nets_get_deterministic_sequential_names`</sub>
- Two connection points of the same symbol sharing a pin number (a feed-through terminal) are internally shorted, and appear once in the net's pin list. <sub>`same_pin_number_points_short_internally`</sub>
- A wire number written on the wires becomes the net's displayed name when the net has no net label. <sub>`wire_number_names_a_net_without_a_label`</sub>
- A net label always wins over the wire number, which in turn wins over the automatic N001 name. <sub>`net_name_prefers_label_then_wire_number_then_auto_name`</sub>
- Pin positions honor the symbol's rotation (clockwise in the Y-down paper coordinate system) and placement. <sub>`pin_positions_apply_rotation_and_translation`</sub>
- Mirroring flips pins across the vertical axis before rotation is applied. <sub>`pin_positions_apply_mirror_before_rotation`</sub>

### ngspice runner

- The ngspice 'print all' output parses into node voltages (v(n1) -> n1) and branch currents (v1#branch), ignoring noise lines. <sub>`parse_print_all_reads_nodes_and_branches`</sub>
- The ngspice executable is located by priority: MADAKE_NGSPICE env var, then PATH, then OS default install paths; a missing env path falls through. <sub>`find_in_prefers_env_then_path_then_candidates`</sub>
- Running a DC operating point on a 24 V / 6+6 ohm divider yields 12 V at the midpoint and 2 A of source current (requires ngspice; skipped otherwise). <sub>`run_op_solves_a_divider_when_ngspice_is_installed`</sub>

### Parts database

- Opening a new database creates the schema and seeds sample parts exactly once; reopening never re-seeds. <sub>`open_creates_schema_and_seeds_samples_once`</sub>
- Parts can be inserted, updated by part number, fetched, and searched by partial name match or exact category. <sub>`upsert_get_and_search`</sub>
- Wire parts are registered and looked up by exact color + gauge combination. <sub>`wire_parts_crud_and_lookup`</sub>
- An old schema-v1 database migrates to v2 on open, preserving existing rows and gaining the spice_model column. <sub>`v1_database_migrates_to_v2_preserving_data`</sub>
- The database path defaults to the OS app-data folder and can be overridden with MADAKE_PARTS_DB. <sub>`default_path_respects_env_override`</sub>

### PDF output

- PDF export produces a valid PDF document (%PDF- header) of non-trivial size, including Japanese text. <sub>`sheet_to_pdf_produces_pdf_bytes`</sub>
- The PDF page is exactly the size of the paper (A3 landscape = 420x297mm), so printing at 100% is 1:1. <sub>`pdf_page_is_the_size_of_the_paper`</sub>
- A PDF book is ordered cover page, then every circuit sheet, then the selected report pages. <sub>`pdf_book_is_cover_then_sheets_then_reports`</sub>
- A PDF book can also carry the graphical terminal diagram, which lands among the report pages in the order it was listed. <sub>`pdf_book_can_include_the_terminal_diagram`</sub>
- With no reports selected a PDF book holds just the cover and the circuit sheets. <sub>`pdf_book_without_reports_is_cover_and_sheets_only`</sub>
- The cover page can be turned off, leaving the circuit sheets first. <sub>`pdf_book_can_omit_the_cover`</sub>
- Exporting the book writes one PDF document holding every page, with report pages on A4 even when the circuit is A3. <sub>`pdf_book_merges_every_page_into_one_document`</sub>
- A book of a project with no sheet at all still produces a valid one-page PDF (the cover). <sub>`pdf_book_of_an_empty_project_is_just_the_cover`</sub>

### Reports as drawing sheets

- A report page is an A4 landscape sheet with the JIS frame, the report title and a title block carrying project name, date and page number. <sub>`report_page_has_frame_title_and_title_block`</sub>
- Every column header and every cell of the data rows is drawn on the page. <sub>`report_page_draws_headers_and_all_cells`</sub>
- A table with no rows still produces exactly one page showing the column headers. <sub>`report_page_with_no_rows_still_shows_headers`</sub>
- Rows that do not fit on one page continue on further pages, each repeating the column headers and numbering pages n/N. <sub>`report_pages_split_and_repeat_headers`</sub>
- A cell longer than its column is cut off and ends with an ellipsis, so text never overruns the column. <sub>`report_page_truncates_cells_wider_than_the_column`</sub>
- Column widths follow the given ratios; without ratios the columns are equal in width. <sub>`report_page_column_widths_follow_ratios`</sub>
- Special XML characters in the data are escaped so the page stays valid SVG. <sub>`report_page_escapes_xml_special_characters`</sub>
- The terminal chart sheet carries the reference designator in its title and one table row per terminal, matching the chart data. <sub>`terminal_chart_sheet_matches_chart_rows`</sub>
- Asking for a terminal chart sheet of something that is not a terminal block yields no page at all. <sub>`terminal_chart_sheet_is_none_for_other_entities`</sub>
- The From-To wire list sheet uses the same columns and rows as the CSV report. <sub>`wire_list_sheet_matches_report_rows`</sub>
- The BOM sheet lists every part group with its reference designators and quantity. <sub>`bom_sheet_lists_parts_with_quantity`</sub>
- The cross-reference sheet lists each net with the pins it connects and the sheet it appears on. <sub>`xref_table_sheet_lists_nets_and_pins`</sub>
- The cover page shows the project name, every sheet with its drawing number, and the newest revision of the whole project. <sub>`cover_page_shows_project_sheets_and_latest_revision`</sub>
- A project without any revision says so on the cover instead of leaving the line blank. <sub>`cover_page_states_when_there_is_no_revision`</sub>
- Exporting a report as CSV returns the same table the drawing sheet shows, and reports how many body rows it wrote. <sub>`a_csv_export_returns_the_table_and_its_row_count`</sub>
- Asking for the terminal chart of the whole project puts the reference designator of each terminal block in the first column. <sub>`a_project_wide_terminal_chart_names_the_terminal_block_in_each_row`</sub>
- Exporting a report as PDF binds the framed drawing pages into one PDF file and reports the page count. <sub>`a_pdf_export_binds_the_framed_pages_into_one_file`</sub>
- The terminal connection diagram is a drawing, so it cannot be exported as CSV and says so. <sub>`the_terminal_connection_diagram_has_no_csv_form`</sub>
- Pointing a terminal report at something that is not a terminal block fails instead of writing an empty file. <sub>`a_terminal_report_of_an_unknown_block_fails`</sub>
- Every other report ignores the terminal selection and always covers the whole project. <sub>`other_reports_always_cover_the_whole_project`</sub>
- The cross-reference CSV carries the same column headings as its drawing sheet. <sub>`the_cross_reference_csv_has_the_same_columns_as_its_sheet`</sub>
- Report kinds are serialized with the same kebab-case names the CLI and Link API use. <sub>`report_kind_json_names_match_cli_spelling`</sub>

### Reports (BOM / wire list)

- The BOM groups symbols by part number and counts quantities per group. <sub>`bom_groups_by_value_and_counts`</sub>
- BOM fields containing commas are quoted so the CSV stays valid. <sub>`bom_escapes_fields_with_commas`</sub>
- The wire list is a From-To list: every row starts with the sheet name and the two ends of the wire, and the older column order (sheet, wire number, harness, ...) is no longer produced. <sub>`wire_list_is_a_from_to_list`</sub>
- An end of a wire that lands on a symbol pin is written as "reference:pin number" (for example K1:A1). <sub>`a_wire_end_on_a_pin_is_written_as_reference_and_pin_number`</sub>
- An end of a wire that carries a net label is written as the label name. <sub>`a_wire_end_with_a_net_label_is_written_as_the_label_name`</sub>
- An end of a wire that touches nothing is left empty in the list. <sub>`an_unconnected_wire_end_is_empty`</sub>
- Of the two ends, the one whose text sorts first alphabetically becomes From, so the same wire always yields the same From and To no matter which end was drawn first. <sub>`from_is_the_alphabetically_smaller_of_the_two_ends`</sub>
- When only one end is connected, that end is always From and the empty one is always To. <sub>`a_connected_end_becomes_from_and_the_unconnected_end_becomes_to`</sub>
- Both sides of a feed-through terminal share one terminal number, so wires on the inside and the outside of the same terminal are both written as "TB1:1". <sub>`both_sides_of_a_feed_through_terminal_use_the_same_terminal_number`</sub>
- The wire list keeps the wire-number column introduced with wire numbering: it holds the number written on the wire, or is empty when the wire is not numbered yet. <sub>`wire_list_has_a_wire_number_column`</sub>
- The wire list keeps the harness column: it carries the name of the harness boundary that fully encloses the wire, and stays empty for wires outside every harness. <sub>`wire_list_has_a_harness_column`</sub>
- The wire list contains each wire's color, gauge, length and part number. <sub>`wire_list_contains_attributes`</sub>
- A wire with no measured length leaves the length column empty (lengths come back from the 3D routing later). <sub>`wire_list_length_is_empty_until_it_is_known`</sub>
- Rows are sorted by From then To within each sheet, so exporting the same drawing twice gives byte-identical files. <sub>`rows_are_sorted_by_from_then_to`</sub>

### DC simulation

- Simulation with an invalid ngspice binary fails with an explicit error (no silent fallback). <sub>`missing_ngspice_is_an_explicit_error`</sub>
- The DC operating point reports per-net voltages (0-24 V here) and per-component current and power (a 2 A lamp shows ~2 A / ~48 W). <sub>`op_reports_net_voltages_and_component_currents`</sub>
- Declaring a switch open in the what-if analysis drops the load current to zero, and the result notes which switches were opened. <sub>`open_switch_cuts_the_current`</sub>

### SPICE netlist generation

- A series circuit converts to a SPICE deck: one voltage source, ground at the battery minus pin, one resistor per wire (rho*L/A) plus component bridges and the load's equivalent resistance. <sub>`series_circuit_builds_deck_with_source_ground_and_resistors`</sub>
- A junction resting mid-wire splits that wire's resistance proportionally to the geometric length on each side. <sub>`junction_splits_wire_resistance_proportionally`</sub>
- Same-named net labels in different places are bridged with a milliohm resistor, preserving their logical connection. <sub>`same_name_labels_are_bridged`</sub>
- A drawing without a power source cannot be converted (explicit NoSource error). <sub>`no_battery_is_an_error`</sub>
- Deck output is deterministic across runs, and the ground node 0 is the battery's minus pin. <sub>`node_names_are_deterministic_and_ground_is_battery_minus`</sub>

### SVG output (JIS frame)

- The exported SVG contains the JIS frame, the title block text, wires and reference designators. <sub>`svg_contains_frame_wire_and_symbol`</sub>
- Special characters in titles (<, >, &, quotes) are XML-escaped in the SVG. <sub>`svg_escapes_xml_special_chars`</sub>
- Two revisions draw a table right above the title block, oldest at the bottom and newest on top, with a column header row. <sub>`svg_draws_revision_table_above_title_block`</sub>
- A sheet with no revisions draws no revision table at all, not even an empty frame. <sub>`svg_omits_revision_table_when_no_revisions`</sub>
- The Rev field of the title block shows the mark of the newest revision, and falls back to the stored value when there are no revisions. <sub>`svg_title_block_rev_follows_latest_revision`</sub>
- With seven revisions only the newest six rows are drawn; the oldest row is dropped from the drawing while the data keeps it. <sub>`svg_revision_table_shows_only_newest_six_rows`</sub>
- The wire number of a horizontal wire is printed in a monospaced font 2.5 mm above the middle of the wire. <sub>`svg_draws_wire_number_above_a_horizontal_wire`</sub>
- The wire number of a vertical wire is printed 2.5 mm to the left of the middle of the wire. <sub>`svg_draws_wire_number_left_of_a_vertical_wire`</sub>
- One net is labelled once, at the middle of its longest segment, however many wires it is drawn with. <sub>`svg_draws_the_wire_number_once_on_the_longest_segment`</sub>
- A net without a wire number gets no number text at all. <sub>`svg_omits_wire_number_for_unnumbered_nets`</sub>
- A harness boundary is drawn as a dashed rectangle (IEC 61082-1 group enclosure) around the wires it holds. <sub>`svg_draws_a_harness_as_a_dashed_rectangle`</sub>
- The harness name is printed just outside the top-left corner of the boundary. <sub>`svg_labels_the_harness_at_its_top_left_corner`</sub>
- A harness with no name draws only its dashed boundary, without a label. <sub>`svg_omits_the_label_of_an_unnamed_harness`</sub>
- Rotated symbols are drawn with their shapes actually rotated (90 deg makes a resistor body vertical). <sub>`svg_renders_rotated_symbol_primitives`</sub>
- A sheet exported as part of its project shows, next to each net label, the "/sheet.zone" address of the same-named label on the other sheet. <sub>`svg_draws_cross_reference_address_next_to_net_label`</sub>
- A net label with no counterpart on another sheet gets no cross-reference text. <sub>`svg_omits_cross_reference_when_there_is_no_counterpart`</sub>
- Exporting a single sheet on its own (no project context) never draws cross-references. <sub>`svg_of_a_lone_sheet_has_no_cross_reference`</sub>

### Symbol library

- Every bundled symbol has a unique id and at least one pin. <sub>`builtin_symbols_have_unique_ids_and_pins`</sub>
- A parametric terminal block (e.g. 8 poles) has one left and one right connection point per terminal, all on the 2.5 mm grid and vertically centered. <sub>`dynamic_terminal_block_has_through_pins_on_grid`</sub>
- connector_2p generated dynamically has exactly the same pin coordinates as the old static definition, so existing drawings are unaffected. <sub>`dynamic_connector_2p_matches_legacy_static_def`</sub>
- resolve_symbol finds built-in ids, and rejects malformed or out-of-range dynamic ids (0 poles, 51 poles, missing count). <sub>`resolve_symbol_rejects_invalid_ids_and_finds_builtins`</sub>
- sheet_symbol_defs returns the built-in library plus definitions for every dynamic symbol actually used on the sheet. <sub>`sheet_symbol_defs_includes_dynamic_ids_in_use`</sub>
- Symbol definitions serialize to JSON and back without loss. <sub>`symbol_json_roundtrip`</sub>

### Terminal block charts

- The chart of a terminal block has exactly one row per terminal, listed in terminal-number order. <sub>`the_chart_has_one_row_per_terminal_in_number_order`</sub>
- What is wired to the left of a terminal is the inside (inside the panel) and what is wired to the right is the outside. <sub>`the_left_side_is_the_inside_and_the_right_side_is_the_outside`</sub>
- Turning a terminal block upside down (180 degrees) swaps its sides too: the inside is still whatever is drawn to the left of it on the paper. <sub>`a_terminal_block_rotated_180_degrees_still_takes_the_paper_left_as_the_inside`</sub>
- When a terminal block is laid sideways (90 degrees) its two connection points sit one above the other, and the upper one is taken as the inside. <sub>`a_sideways_terminal_block_takes_the_upper_connection_as_the_inside`</sub>
- A terminal with no wire on either side stays in the chart as a spare row with both sides empty. <sub>`an_unwired_terminal_stays_as_a_spare_row`</sub>
- A row shows the wire number written on the wire and the wire itself (color, gauge and part number). <sub>`a_row_shows_the_wire_number_and_the_wire_specification`</sub>
- When the wire on the inside differs from the wire on the outside, the row lists both. <sub>`both_wires_are_listed_when_the_inside_and_outside_differ`</sub>
- Besides the combined wire column, each row keeps the wire of the inside and of the outside separately. <sub>`a_row_keeps_the_wire_of_each_side_separately`</sub>
- Each row records the harness the wire of that side belongs to, and stays empty for a wire in no harness. <sub>`a_row_records_the_harness_of_each_side`</sub>
- Jumper text is normalized: each pair is written smaller-larger, duplicates are dropped and the pairs come out in ascending order. <sub>`jumpers_are_normalized`</sub>
- A jumper between terminals that are not next to each other is rejected, and the valid jumpers in the same text are still kept. <sub>`a_jumper_between_non_adjacent_terminals_is_rejected`</sub>
- Jumper text that is not a pair of terminal numbers is reported instead of crashing. <sub>`unreadable_jumper_text_is_reported`</sub>
- A jumper pointing at a terminal the block does not have is reported as such. <sub>`a_jumper_to_a_terminal_that_does_not_exist_is_reported`</sub>
- An empty jumper attribute simply means no jumpers. <sub>`no_jumper_text_means_no_jumpers`</sub>
- A jumper appears in the jumper column of both terminals it connects. <sub>`a_jumper_is_shown_on_both_of_its_terminals`</sub>
- The terminal chart CSV has the columns terminal, inside, wire number, wire, outside, jumper, and one line per terminal. <sub>`the_terminal_chart_csv_has_the_designed_columns`</sub>
- Asking for the chart of something that is not a terminal block gives nothing. <sub>`only_terminal_blocks_have_a_chart`</sub>
- The terminal block check reports every unwired terminal as information, so spares are visible without being treated as mistakes. <sub>`the_check_reports_unwired_terminals_as_information`</sub>
- The terminal block check reports a jumper between non-adjacent terminals as an error. <sub>`the_check_reports_an_invalid_jumper_as_an_error`</sub>
- The terminal block check reports a jumper to a terminal that does not exist as an error. <sub>`the_check_reports_a_jumper_to_a_missing_terminal_as_an_error`</sub>
- A terminal block with every terminal wired and correct jumpers passes the check with nothing to report. <sub>`a_fully_wired_terminal_block_passes_the_check`</sub>
- The editor lists every terminal block of the project with its sheet, its pole count and its jumpers, sheet by sheet and in reference-designator order. <sub>`the_editor_lists_every_terminal_block_with_its_sheet_poles_and_jumpers`</sub>
- Naming a sheet narrows the list to the terminal blocks drawn on that sheet. <sub>`naming_a_sheet_narrows_the_terminal_block_list_to_that_sheet`</sub>
- Symbols that are not terminal blocks never show up in the list. <sub>`other_symbols_never_show_up_in_the_terminal_block_list`</sub>
- The chart and the check of a terminal block can be looked up by entity id alone, without knowing which sheet it sits on. <sub>`a_terminal_block_can_be_looked_up_by_id_across_sheets`</sub>
- Jumpers are set with the ordinary update_entity command, so the chart follows the change and undo takes it back. <sub>`setting_jumpers_through_update_entity_is_undoable`</sub>

### Terminal connection diagrams

- The terminal strip is drawn as one numbered box per terminal, stacked from top to bottom in terminal order. <sub>`the_strip_stacks_one_numbered_box_per_terminal`</sub>
- The outside of the panel is drawn to the left of the strip and the inside to the right, each under its own caption. <sub>`the_outside_is_on_the_left_and_the_inside_on_the_right`</sub>
- A terminal with nothing wired to it stays in the strip as a lightly filled box marked as a spare. <sub>`a_spare_terminal_stays_in_the_strip_lightly_filled`</sub>
- Wires of the same harness are gathered into one bracket at the outer end of their lead lines, labelled with the harness name. <sub>`wires_of_one_harness_are_gathered_into_a_bracket`</sub>
- A wire that belongs to no harness gets no bracket at all. <sub>`a_wire_without_a_harness_gets_no_bracket`</sub>
- A saddle jumper between neighbouring terminals is drawn as a vertical link on the inside edge of the terminal boxes. <sub>`a_jumper_is_drawn_on_the_inside_edge_of_the_boxes`</sub>
- Each lead line carries the wire it stands for: colour, gauge in sq and part number, written small under the line. <sub>`each_lead_line_carries_the_wire_specification`</sub>
- Terminals that do not fit on one page continue on the next, and each page title states the range of terminals it holds. <sub>`terminals_that_do_not_fit_continue_on_the_next_page`</sub>
- Every terminal block gets exactly one page; anything that is not a terminal block gets none. <sub>`one_page_per_terminal_block_and_none_for_anything_else`</sub>
- A terminal diagram page is an A4 landscape sheet with the JIS frame and a title block naming the terminal block. <sub>`the_page_has_the_frame_and_a_title_block`</sub>
- The terminal diagram is offered as a report named terminal-diagram, spelled the same way in the CLI, Link API and MCP. <sub>`the_terminal_diagram_is_a_report_named_terminal_diagram`</sub>
- A project without any terminal block still yields one page, so the report is never empty. <sub>`a_project_without_terminal_blocks_still_yields_one_page`</sub>

### Verification (ERC & electrical)

- A fully wired circuit produces no ERC findings. <sub>`fully_wired_pair_has_no_erc_findings`</sub>
- Unconnected pins are reported as one warning per symbol, listing the affected pin numbers. <sub>`unconnected_pins_are_reported_per_symbol`</sub>
- A terminal-block terminal counts as connected if either its left or right side is wired. <sub>`terminal_block_terminal_counts_connected_if_either_side_wired`</sub>
- Missing and duplicate reference designators are flagged, but relay coil + contacts legitimately share one designator. <sub>`empty_and_duplicate_references_are_flagged_but_relays_allowed`</sub>
- Wire ends attached to nothing are flagged, including an endpoint resting mid-wire without a junction dot. <sub>`dangling_wire_end_is_flagged_including_missing_junction`</sub>
- A healthy series circuit (source, fuse, switch, lamp) raises no electrical findings. <sub>`healthy_series_circuit_has_no_elec_findings`</sub>
- A load cut off from the power source (broken wire or open path) is reported as unreachable. <sub>`load_cut_off_from_source_is_unreachable`</sub>
- A load current exceeding the wire's ampacity is an error, and exceeding the fuse rating is a warning. <sub>`overloaded_wire_and_fuse_are_flagged`</sub>
- Voltage drop above 3 % of the supply voltage on a long wire is flagged; a short wire passes. <sub>`excessive_voltage_drop_is_flagged`</sub>
- A load without a current_a attribute is excluded from current checks and reported as an Info note. <sub>`load_without_current_attr_gets_info_and_no_current_checks`</sub>
- With ngspice installed, electrical findings carry solver-measured values and no approximate-mode note appears. <sub>`simulation_mode_reports_measured_values_when_ngspice_installed`</sub>
- Without a solver result, checks fall back to a graph approximation and say so with an Info note. <sub>`fallback_mode_emits_approximate_info`</sub>
- Two different net labels on one net (a short between potentials) is an error. <sub>`conflicting_net_labels_on_one_net_are_an_error`</sub>
- A single-sheet label conflict is reported exactly once when the whole project is verified. <sub>`project_verification_reports_a_single_sheet_label_conflict_once`</sub>
- Nets joined across sheets by a shared label name are checked as one net, so a conflict spanning two sheets is reported once with all offending labels. <sub>`project_verification_merges_label_conflicts_across_sheets`</sub>
- A net continued onto another sheet with the same label name is not a conflict. <sub>`project_verification_accepts_a_net_continued_onto_another_sheet`</sub>

### Wire numbering

- Append mode numbers only the nets that have no number yet, leaving already numbered nets untouched. <sub>`append_numbers_only_unnumbered_nets`</sub>
- New numbers never collide with numbers that are already used on the drawing. <sub>`append_skips_numbers_already_in_use`</sub>
- Renumber mode throws away every numeric wire number and assigns fresh consecutive numbers from the start value. <sub>`renumber_reassigns_every_numeric_wire_number`</sub>
- A hand-written name that is not a plain number (for example "24V_1") survives a full renumber. <sub>`renumber_keeps_manual_non_numeric_names`</sub>
- A net that carries a net label keeps the label as its name and is never given a wire number. <sub>`nets_with_a_net_label_are_never_numbered`</sub>
- All wires of one net receive the very same wire number, even when they were drawn as separate segments. <sub>`every_wire_of_a_net_gets_the_same_number`</sub>
- Undo after an automatic renumbering restores every previous wire number, including wires that had none. <sub>`undo_restores_previous_wire_numbers`</sub>
- Numbering runs top to bottom, and left to right within the same height, so the same drawing always yields the same numbers. <sub>`numbering_order_is_top_to_bottom_then_left_to_right`</sub>
- Numbering can start from any number, for example 100 for a second panel. <sub>`numbering_starts_at_the_given_start_number`</sub>
- Without a sheet id every sheet of the project is numbered in sheet order with numbers unique across the whole project. <sub>`numbering_without_a_sheet_covers_the_whole_project`</sub>
- A single wire number can be edited directly, and undo puts the previous value back. <sub>`set_wire_numbers_edits_one_wire_and_is_undoable`</sub>
- The renumber command is plain JSON ({"type":"renumber_wires","mode":"append","start":1}), so AI agents and the CLI can send it. <sub>`renumber_command_is_plain_json`</sub>

### xref

- A zone address combines the row letter (top to bottom) with the column number (left to right), e.g. "B3". <sub>`zone_address_combines_row_letter_and_column_number`</sub>
- Points outside the drawing frame are rounded to the nearest zone instead of producing an invalid address. <sub>`zone_address_clamps_points_outside_the_frame`</sub>
- Net labels with the same name on different sheets are merged into a single project-wide net. <sub>`same_named_labels_merge_into_one_project_net`</sub>
- Nets with different label names stay separate across sheets. <sub>`differently_named_labels_stay_separate_nets`</sub>
- The cross-reference of a label is the address "/sheet.zone" of the same-named label on another sheet. <sub>`cross_reference_address_uses_slash_sheet_dot_zone`</sub>
- A label on the destination sheet points back to the source sheet, so both sides show the counterpart. <sub>`cross_reference_is_shown_on_both_sides`</sub>
- When the same net continues onto several sheets, every counterpart address is listed. <sub>`multiple_counterparts_are_all_listed`</sub>
- The label's own sheet is never listed as a counterpart, even when the same name appears twice on it. <sub>`own_sheet_is_excluded_from_counterparts`</sub>
- A label with no counterpart on another sheet shows no cross-reference at all. <sub>`label_without_counterpart_shows_nothing`</sub>
- The per-sheet cross-reference table maps each label entity to the text drawn beside it. <sub>`sheet_cross_reference_table_maps_labels_to_text`</sub>
- The cross-reference text sits to the right of the label text, on the same baseline. <sub>`cross_reference_text_sits_right_of_the_label`</sub>
- The cross-reference table has one row per project-wide net, listing the sheets it spans and the drawing addresses of its labels. <sub>`cross_reference_table_lists_one_row_per_net_with_its_sites`</sub>
- The columns of the cross-reference table are net / wire number / connected pins / sheets / sites. <sub>`cross_reference_table_columns_are_net_wire_pins_sheets_sites`</sub>


## Automation APIs (MCP / REST)


### Agent REST endpoints

- POST /agent/send runs an assistant turn and the conversation list reflects the new messages. <sub>`send_runs_a_turn_and_conversations_reflects_it`</sub>
- Sending to a non-existent conversation id returns 400 instead of creating garbage. <sub>`send_to_unknown_conversation_is_400`</sub>
- Cancel and undo-turn endpoints respond correctly for the latest applied turn. <sub>`cancel_and_undo_turn_respond`</sub>
- Undoing a turn rolls back exactly the document edits that the turn produced, via the same undo history as manual edits. <sub>`undo_turn_rolls_back_agent_edits_through_the_command_engine`</sub>
- GET /agent/events streams conversation events over SSE, in the same shape as the Tauri agent:event. <sub>`events_endpoint_streams_agent_events`</sub>
- Saving and loading a project carries the chat history alongside (.chat.json). <sub>`save_and_load_carry_the_chat_history`</sub>
- Loading a project cancels any running turn first, so the agent never edits the wrong document. <sub>`load_cancels_a_running_turn`</sub>
- Requests from external web origins are rejected; local origins and non-browser clients (no Origin header) pass. <sub>`external_origins_are_rejected_but_local_and_originless_pass`</sub>
- The origin guard treats only loopback hosts (localhost/127.0.0.1) as local. <sub>`local_origin_predicate_matches_only_loopback_hosts`</sub>
- The drawing context given to the agent summarizes the active sheet (name, nets, entity count). <sub>`drawing_context_summarizes_the_active_sheet`</sub>
- AI settings endpoints persist changes and apply them to the agent manager. <sub>`settings_endpoints_persist_and_apply`</sub>

### REST Link API

- The REST parts endpoints support searching (sample data included), upserting, category filtering, deleting, and listing wire parts. <sub>`parts_endpoints_search_upsert_delete`</sub>
- GET /api/v1/verify returns the drawing's diagnostics as JSON (e.g. empty reference and unconnected pins). <sub>`verify_returns_diagnostics`</sub>
- POST /api/v1/import/kicad replaces the open project with the converted schematic and returns a patch plus an import report. <sub>`import_kicad_replaces_project_and_reports`</sub>
- POST /api/v1/simulate/op solves the DC operating point and returns net voltages and component currents (requires ngspice; skipped otherwise). <sub>`simulate_op_returns_result`</sub>
- POST /api/v1/export/pdf writes a valid PDF file to the requested path. <sub>`export_pdf_writes_pdf_file`</sub>
- POST /api/v1/export/pdf-book writes one PDF holding the cover, every sheet and the requested reports. <sub>`export_pdf_book_writes_cover_sheets_and_reports`</sub>
- The terminal endpoints list the terminal blocks of the drawing and return one block's chart and check result. <sub>`terminal_endpoints_list_chart_and_check`</sub>
- POST /api/v1/export/report writes one report as CSV or as framed PDF pages, and refuses CSV for the graphical terminal diagram. <sub>`export_report_writes_csv_and_pdf_per_report`</sub>


## AI assistant (madake-agent)


### Claude CLI backend

- The Claude CLI is launched headless with the required flags (-p, stream-json output, partial messages, strict MCP config). <sub>`args_contain_required_flags`</sub>
- Resume session id, model choice and an appended system prompt are passed through when provided. <sub>`args_include_resume_model_and_system_prompt_when_given`</sub>
- The agent's MCP config points at MadakeCAD's own local MCP server, so it uses the same tools as any client. <sub>`mcp_config_points_at_local_mcp_server`</sub>
- A turn streams events (text deltas, tool use, completion) parsed from the CLI's stream-json output. <sub>`send_streams_events_from_fake_cli`</sub>
- A CLI exiting non-zero is reported as an error event instead of hanging. <sub>`send_reports_nonzero_exit_as_error_event`</sub>
- The prompt is passed via stdin, never via argv (avoids OS argument-length and quoting issues). <sub>`prompt_is_passed_through_stdin_not_argv`</sub>
- If the event receiver goes away, the turn finishes promptly instead of blocking forever. <sub>`send_returns_promptly_when_receiver_is_dropped`</sub>
- Output consisting only of unknown lines still terminates the turn promptly. <sub>`send_returns_promptly_when_only_non_event_lines_flow`</sub>
- An I/O error while reading CLI output becomes an error event. <sub>`io_error_while_reading_is_reported_as_error_event`</sub>
- Claude CLI detection reads the version from the configured executable. <sub>`detect_reads_version_from_configured_executable`</sub>
- Detection fails cleanly when the executable does not exist. <sub>`detect_fails_for_missing_executable`</sub>
- Detection falls back through the candidate path list until one works. <sub>`detect_falls_back_to_later_candidates`</sub>
- When no candidate works, detection reports an error listing what was tried. <sub>`detect_from_reports_error_when_no_candidate_works`</sub>
- Default candidates include the well-known Claude CLI install locations. <sub>`default_candidates_include_known_install_paths`</sub>

### Conversations & history

- Each turn records the engine revision range it spanned, for display and debugging. <sub>`turn_records_engine_revision_range`</sub>
- The turn's undo count is the undo-stack depth delta, not the revision delta (undo/redo also advance revisions). <sub>`undo_count_uses_undo_stack_depth_not_revision_delta`</sub>
- A turn whose edits were all undone during the turn counts as having no edits. <sub>`turn_whose_edits_were_all_undone_has_no_edits`</sub>
- When an undo rollback stops midway, only the completed undo count is recorded. <sub>`record_undone_applies_only_the_completed_count`</sub>
- A text-only turn (no document edits) has an undo count of zero. <sub>`turn_without_edits_has_zero_undo_count`</sub>
- A turn collects the streamed text, the tool calls, and the CLI session id. <sub>`turn_collects_text_tool_calls_and_session`</sub>
- A second turn appends to the conversation and resumes the same CLI session. <sub>`second_turn_appends_messages_and_reuses_session`</sub>
- An error event is recorded on the current turn's message. <sub>`error_event_is_recorded_on_current_turn`</sub>
- Chat history saves as pretty JSON and loads back identically. <sub>`chat_file_roundtrip_is_pretty_json`</sub>
- A chat file from a newer format version is rejected rather than silently mangled. <sub>`load_chat_rejects_newer_format_version`</sub>
- Chat saving writes atomically (temp file + rename) and leaves no temp file behind. <sub>`save_chat_writes_atomically_and_leaves_no_temp_file`</sub>
- Loading chat history when no file exists yields an empty history. <sub>`load_chat_of_missing_file_is_empty`</sub>
- Conversations get an updated_at timestamp on creation that advances with each turn. <sub>`updated_at_is_set_on_creation_and_advances_with_the_turn`</sub>
- Legacy chat files without updated_at load with a sensible default. <sub>`load_chat_defaults_updated_at_for_legacy_files`</sub>
- The chat file lives next to the project file as <name>.chat.json. <sub>`chat_path_sits_next_to_project_file`</sub>

### Agent manager (turns)

- Sending without a conversation id creates a conversation and broadcasts its events. <sub>`send_creates_conversation_and_broadcasts_events`</sub>
- A turn that edits the document records the applied revisions and emits a turn-applied event with the undo depth. <sub>`turn_records_applied_revisions_and_emits_turn_applied`</sub>
- Turn undo counts are based on undo-stack growth, so user undos during the turn don't corrupt the count. <sub>`undo_turn_counts_stack_growth_not_revision_delta`</sub>
- Only the latest applied turn may be reverted; older targets are rejected (safety guard). <sub>`undo_turn_rejects_targets_that_are_not_the_latest_applied_turn`</sub>
- A running turn cannot be reverted. <sub>`undo_turn_rejects_a_running_turn`</sub>
- If undo fails midway, the partial progress is recorded so the state stays truthful. <sub>`undo_turn_records_partial_progress_when_undo_fails_midway`</sub>
- Document errors and unknown turn targets are reported as distinct errors. <sub>`undo_turn_reports_doc_errors_and_unknown_targets`</sub>
- Sending to a conversation that is already running a turn is rejected. <sub>`second_send_while_running_is_rejected`</sub>
- Cancel kills the CLI process and marks the message as cancelled. <sub>`cancel_stops_the_turn_and_marks_the_message`</sub>
- Sending to an unknown conversation id fails cleanly. <sub>`send_to_unknown_conversation_fails`</sub>
- The drawing context and selected model are forwarded to the CLI invocation. <sub>`context_and_model_are_forwarded_to_the_cli`</sub>
- Turning off auto-read-drawing suppresses the drawing context. <sub>`auto_read_drawing_off_suppresses_the_drawing_context`</sub>
- The claude-path setting overrides which executable the backend runs. <sub>`claude_path_setting_becomes_the_backend_executable`</sub>
- Replacing the conversation history (project load) cancels any running turn first. <sub>`set_conversations_replaces_history_and_cancels_running_turn`</sub>

### stream-json parser

- The CLI's system/init line yields a session-started event carrying the session id. <sub>`init_line_yields_session_started`</sub>
- Only text deltas become text events; thinking deltas are ignored. <sub>`only_text_deltas_become_text_events`</sub>
- A completed assistant tool_use block (with full input) starts a tool-use event. <sub>`assistant_tool_use_block_yields_tool_use_started`</sub>
- A user tool_result line finishes the matching tool use, carrying the error flag. <sub>`tool_result_yields_tool_use_finished`</sub>
- The result line completes the turn with token usage attached. <sub>`result_line_yields_turn_completed_with_usage`</sub>
- Unknown message types and noise lines produce no events (forward compatible). <sub>`unknown_and_noise_lines_are_none`</sub>
- An error result becomes an error event with the message. <sub>`error_result_yields_error_event`</sub>
- A real recorded stream parses into the expected full event sequence. <sub>`full_event_sequence_of_tooluse_fixture`</sub>
- The parser fills in the tool name when a tool use finishes. <sub>`stream_parser_fills_tool_name_on_finish`</sub>
- Duplicate tool-use-started events for the same id are dropped. <sub>`stream_parser_drops_duplicate_tool_use_started`</sub>

### AI settings

- Default AI settings enable auto-apply and auto-read-drawing. <sub>`default_settings_are_auto_apply_and_auto_read`</sub>
- A missing settings file yields the defaults. <sub>`missing_file_yields_defaults`</sub>
- Saved settings load back identically. <sub>`saved_settings_round_trip`</sub>
- Unknown/missing fields in the settings file fall back to defaults (forward compatible). <sub>`missing_fields_fall_back_to_defaults`</sub>
- Saving creates the settings directory if needed. <sub>`save_creates_the_settings_directory`</sub>
- Blank executable paths are normalized away instead of being stored. <sub>`normalized_drops_blank_paths`</sub>
- The settings path honors its environment-variable override. <sub>`settings_path_honors_the_env_override`</sub>
- The default UI language is English. <sub>`default_language_is_english`</sub>
- A settings file saved before the language field existed loads with English. <sub>`old_settings_file_without_language_loads_as_english`</sub>
- Normalization lowercases the language tag and turns blank input into English. <sub>`language_is_normalized_to_lowercase_and_blank_becomes_english`</sub>
- The language choice survives a save/load round trip. <sub>`language_round_trips_through_save_and_load`</sub>


## madake CLI


### Argument parsing & dispatch

- The default port is 9310 and --json is off unless requested. <sub>`default_port_is_9310_and_json_is_off`</sub>
- --port and --json are global options and may appear after the subcommand. <sub>`port_and_json_are_global_options_after_subcommand`</sub>
- The export kind accepts the 'wire-list' spelling used in documentation. <sub>`export_kind_accepts_wire_list_spelling`</sub>
- madake export pdf-book sends the reports listed with --reports, in that order, and asks for a cover by default. <sub>`export_pdf_book_sends_the_requested_reports_in_order`</sub>
- --no-cover drops the cover page from the PDF book. <sub>`export_pdf_book_can_drop_the_cover`</sub>
- madake terminals lists the terminal blocks of the whole project, or of one sheet with --sheet. <sub>`terminals_lists_blocks_and_forwards_sheet`</sub>
- madake export terminal-chart writes the whole project's chart, choosing CSV from the .csv extension. <sub>`export_terminal_chart_defaults_to_csv_for_the_whole_project`</sub>
- A .pdf output path selects the PDF (framed drawing sheet) form of a report without asking for --format. <sub>`export_report_picks_pdf_from_the_extension`</sub>
- --format wins over the file extension. <sub>`export_report_format_option_overrides_the_extension`</sub>
- --terminal takes a reference designator (TB1) and looks its entity id up in the terminal list. <sub>`export_terminal_diagram_resolves_a_reference_designator`</sub>
- A --terminal value spelled as an entity id is sent as-is, with no lookup. <sub>`export_terminal_chart_accepts_an_entity_id_directly`</sub>
- An unknown terminal reference stops with the list of terminal blocks that do exist. <sub>`export_reports_unknown_terminal_with_candidates`</sub>
- --terminal is refused for reports that cover the whole project. <sub>`export_rejects_terminal_option_on_other_reports`</sub>
- Reports always cover the whole project, so --sheet alone is refused and points at --terminal. <sub>`export_rejects_sheet_only_narrowing_for_reports`</sub>
- --format is refused for schematic exports (svg/pdf/pdf-book), which are not reports. <sub>`export_rejects_format_option_on_schematic_exports`</sub>
- madake status calls the health endpoint and the project snapshot. <sub>`status_queries_health_and_project`</sub>
- --json prints raw pretty-printed JSON for piping into jq and similar tools. <sub>`json_flag_emits_raw_json`</sub>
- madake netlist forwards the --sheet option to the API. <sub>`netlist_forwards_sheet_option`</sub>
- madake export forwards kind, output path and optional sheet to the API. <sub>`export_forwards_kind_path_and_sheet`</sub>
- madake exec reads a JSON file containing a Command array and posts it to /commands. <sub>`exec_posts_command_array_from_file`</sub>
- madake exec rejects JSON that is not an array, with a clear message. <sub>`exec_rejects_non_array_json`</sub>
- A missing input file is reported as a file error, not a panic. <sub>`exec_reports_missing_file`</sub>
- madake renumber numbers every sheet from 1 in top-up mode unless told otherwise. <sub>`renumber_defaults_to_whole_project_append_from_one`</sub>
- madake renumber passes the target sheet, the numbering mode and the start number through. <sub>`renumber_forwards_sheet_mode_and_start`</sub>
- madake renumber says so when every net already had a wire number. <sub>`renumber_reports_when_nothing_changed`</sub>
- madake undo tells the user when there is nothing to undo. <sub>`undo_reports_empty_history`</sub>
- madake redo reports the new document revision on success. <sub>`redo_reports_revision`</sub>
- madake save/open report the file path they acted on. <sub>`save_and_open_report_path`</sub>

### Link API client

- The CLI talks to http://127.0.0.1:<port>/api/v1 (loopback only). <sub>`base_url_uses_loopback_and_api_v1`</sub>
- endpoint() joins the base URL with a relative path. <sub>`endpoint_appends_path`</sub>
- The netlist URL has no query string when no sheet is specified. <sub>`netlist_url_omits_query_without_sheet`</sub>
- Sheet selection uses the sheet_id query parameter, matching the server. <sub>`netlist_url_uses_sheet_id_query_name`</sub>
- Schematic exports (svg/pdf/pdf-book) each have their own REST route, while every report goes to the shared /export/report route. <sub>`export_kind_paths_match_link_api_routes`</sub>
- The five report kinds map to the report names the Link API knows; schematic exports are not reports. <sub>`export_kinds_map_to_the_five_reports`</sub>
- Only the two terminal reports can be narrowed to a single terminal block. <sub>`only_terminal_reports_take_a_terminal_option`</sub>
- Without --format the output format follows the file extension: .pdf writes a PDF, anything else writes CSV. <sub>`report_format_defaults_to_the_file_extension`</sub>
- The report format names sent to the Link API are "csv" and "pdf". <sub>`report_format_json_names`</sub>
- The terminals URL uses the same sheet_id query name as the other endpoints. <sub>`terminals_url_uses_sheet_id_query_name`</sub>
- A --terminal value is treated as an entity id only when it is spelled like a UUID; anything else is a reference designator. <sub>`uuid_shaped_values_are_recognized`</sub>
- The report names sent for a PDF book are spelled the same as in the Link API and MCP JSON. <sub>`report_kind_json_names_match_the_link_api`</sub>
- When the app is not running, the CLI explains it explicitly (with the port) instead of a cryptic error. <sub>`not_running_error_is_explicit`</sub>

### Human-readable output

- Display width counts full-width (Japanese) characters as two columns for correct table alignment. <sub>`disp_width_counts_fullwidth_as_two`</sub>
- Padding is based on display width, so Japanese and ASCII cells align. <sub>`pad_uses_display_width`</sub>
- Tables align columns to the widest cell. <sub>`table_aligns_columns`</sub>
- madake status output summarizes the endpoint and the document (sheets, entities). <sub>`status_summarizes_connection_and_document`</sub>
- madake project lists each sheet with its entity count. <sub>`project_lists_sheets_with_entity_counts`</sub>
- madake netlist renders a table of nets with their pin references (K1:2 style). <sub>`netlist_renders_table_with_pin_references`</sub>
- An empty netlist prints a friendly message instead of an empty table. <sub>`netlist_handles_empty`</sub>
- madake exec reports how many commands ran and the resulting revision. <sub>`exec_result_reports_revision_and_op_count`</sub>
- Undo/redo formatting handles the 'nothing to do' (null patch) case. <sub>`history_result_handles_null_patch`</sub>
- Undo/redo formatting reports the revision and change count. <sub>`history_result_reports_revision`</sub>
- Save/export messages include the written file path. <sub>`saved_and_exported_report_written_path`</sub>
- A report export message names the report, the format and how much was written (CSV rows / PDF pages). <sub>`exported_report_reports_format_and_count`</sub>
- madake terminals lists every terminal block with its pole count, jumpers, sheet and entity id. <sub>`terminals_lists_blocks_with_poles_and_jumpers`</sub>
- An empty terminal list prints a friendly message instead of an empty table. <sub>`terminals_handles_empty`</sub>
- Open messages include the loaded path and resulting revision. <sub>`opened_reports_path_and_revision`</sub>


## Frontend (editor UI)


### agentOverlay

- place_symbol's x/y produces a region around the approximate symbol size <sub>`toolBox`</sub>
- draw_wire's point list produces its bounding box <sub>`toolBox`</sub>
- execute_commands regions merge the add_entity commands inside <sub>`toolBox`</sub>
- tools without coordinates and malformed input produce no region <sub>`toolBox`</sub>
- wires use their vertex bbox; symbols use the anchor plus approximate size <sub>`entityBox`</sub>
- junctions, texts and net labels also get regions <sub>`entityBox`</sub>
- a tool start shows a region with margin included <sub>`AgentOverlay`</sub>
- tools that yield no region are ignored <sub>`AgentOverlay`</sub>
- re-notifying the same tool id does not duplicate the region <sub>`AgentOverlay`</sub>
- regions of unfinished tools persist <sub>`AgentOverlay`</sub>
- regions fade out HOLD_MS after completion <sub>`AgentOverlay`</sub>
- completions without an id are matched by tool name <sub>`AgentOverlay`</sub>
- entity upserts create regions that expire after HOLD_MS <sub>`AgentOverlay`</sub>
- re-upserting the same entity extends its display deadline <sub>`AgentOverlay`</sub>
- finishAll expires even in-progress regions <sub>`AgentOverlay`</sub>
- clear removes every region <sub>`AgentOverlay`</sub>
- the pulse alpha stays within 0.1-0.25 following a sine wave <sub>`pulseAlpha`</sub>
- margin is added without mutating the original box <sub>`expandBox`</sub>

### dynamicSymbol

- connector_2p has the same pin coordinates as the legacy static definition <sub>`dynamicSymbol`</sub>
- terminal_block_3p has 3 terminals with left/right points, centered on the 2.5 mm grid <sub>`dynamicSymbol`</sub>
- malformed dynamic ids return null <sub>`dynamicSymbol`</sub>
- static definitions win; unknown ids fall back to dynamic generation <sub>`resolveSymbolDef`</sub>

### Harness boundaries

- normalizes a rectangle drag into the same four corners in any direction <sub>`harness geometry`</sub>
- assigns a wire to a harness only when all of its points are inside (the boundary counts as inside) <sub>`harness geometry`</sub>
- prefers the smallest enclosing harness when boundaries are nested <sub>`harness geometry`</sub>
- counts how many wires a harness currently encloses <sub>`harness geometry`</sub>
- returns the bounding box of a harness and null when it has no points <sub>`harness geometry`</sub>
- places the name label just outside the top-left corner of the boundary <sub>`harness geometry`</sub>
- suggests W1 on an empty sheet and continues the W series afterwards <sub>`harness naming`</sub>
- ignores non-harness entities when suggesting the next name <sub>`harness naming`</sub>
- builds one add_entity command with the auto-suggested name <sub>`harnessAddCommand`</sub>
- creates nothing for a degenerate rectangle (a plain click) <sub>`harnessAddCommand`</sub>

### renderer

- places the revision table directly above the title block with the same width and row height <sub>`revisionLayout`</sub>
- stacks revisions oldest at the bottom and newest on top, with the column header row at the very bottom <sub>`revisionLayout`</sub>
- lays out nothing for a sheet without revisions <sub>`revisionLayout`</sub>
- keeps only the newest six revisions in the drawing and drops older rows <sub>`revisionLayout`</sub>
- splits the title block width into the mark, date, description and approver columns <sub>`revisionLayout`</sub>
- returns at most the newest six revisions, still in oldest-first order <sub>`visibleRevisions / effectiveRev`</sub>
- shows the newest revision mark in the title block Rev cell, falling back to the stored value or a dash <sub>`visibleRevisions / effectiveRev`</sub>

### viewClasses

- entity kinds map to view classes (junctions count as wires) <sub>`entityViewClass`</sub>
- VIEW_CLASSES enumerates every class exactly once <sub>`entityViewClass`</sub>
- shows every view class, including wire numbers and harnesses, until it is switched off <sub>`view class visibility`</sub>

### viewport

- world<->screen conversion round-trips and zoom keeps the anchor fixed <sub>`Viewport`</sub>
- panning moves the view in screen pixels <sub>`Viewport`</sub>
- snapping rounds to the 2.5 mm grid by default <sub>`Viewport`</sub>
- zoom is clamped to sane bounds <sub>`Viewport`</sub>

### wireNumbers

- puts the number of a horizontal wire 2.5mm above the midpoint, centred <sub>`wireNumberLabels`</sub>
- puts the number of a vertical wire 2.5mm to the left of the midpoint, right aligned <sub>`wireNumberLabels`</sub>
- draws one net's number only once, at the midpoint of its longest segment <sub>`wireNumberLabels`</sub>
- skips wires without a number and numbers that are only whitespace <sub>`wireNumberLabels`</sub>
- returns every number in a stable order <sub>`wireNumberLabels`</sub>
- places a diagonal wire's number above when it runs wide and to the left when it runs tall <sub>`wireNumberLabels`</sub>
- ignores entities that are not wires <sub>`wireNumberLabels`</sub>
- opens with sequential numbering from 1 over the current sheet, keeping existing numbers <sub>`wire numbering dialog store`</sub>
- numbers only the current sheet in append mode when keeping existing numbers <sub>`wire numbering dialog store`</sub>
- renumbers only the current sheet when all numbers are reassigned <sub>`wire numbering dialog store`</sub>
- omits the sheet id for the whole project so numbers stay unique across sheets <sub>`wire numbering dialog store`</sub>
- accepts only whole numbers of 1 or more as the start number <sub>`wire numbering dialog store`</sub>
- does not offer zone-based numbering yet <sub>`wire numbering dialog store`</sub>
- runs one command, reports how many nets were numbered and closes <sub>`wire numbering dialog store`</sub>
- refuses to run while the start number is invalid <sub>`wire numbering dialog store`</sub>
- sends nothing when the dialog is cancelled <sub>`wire numbering dialog store`</sub>
- keeps the dialog open and reports the error when numbering fails <sub>`wire numbering dialog store`</sub>

### xref

- combines the row letter (top to bottom) with the column number (left to right) <sub>`zoneAt`</sub>
- rounds points outside the drawing frame to the nearest zone <sub>`zoneAt`</sub>
- addresses a counterpart as /sheet.zone <sub>`cross references`</sub>
- shows the counterpart on both sides <sub>`cross references`</sub>
- lists every counterpart when the net continues onto several sheets <sub>`cross references`</sub>
- never lists the label's own sheet as a counterpart <sub>`cross references`</sub>
- shows nothing for a label without a counterpart <sub>`cross references`</sub>
- returns the target sheet and label id so the panel can jump to it <sub>`cross references`</sub>
- maps each label id to the text drawn beside it <sub>`cross references`</sub>
- names the sheet to switch to and the label to select and zoom <sub>`xrefJumpTarget`</sub>
- places the text right of the label text on the same baseline <sub>`xrefTextAt`</sub>

### propertyCommands

- builds a single set_wire_numbers command for an edited wire number <sub>`wireNumberCommand`</sub>
- clears the wire number when the field is emptied <sub>`wireNumberCommand`</sub>
- trims the entered number and treats whitespace as clearing it <sub>`wireNumberCommand`</sub>
- sends nothing when the wire number did not change <sub>`wireNumberCommand`</sub>
- builds nothing for entities other than wires <sub>`wireNumberCommand`</sub>
- builds an update_entity command for a renamed harness and keeps its shape <sub>`harnessUpdateCommand`</sub>
- builds a command when only the note changed <sub>`harnessUpdateCommand`</sub>
- trims the entered harness name <sub>`harnessUpdateCommand`</sub>
- sends nothing when neither the name nor the note changed <sub>`harnessUpdateCommand`</sub>
- builds nothing for entities other than harnesses <sub>`harnessUpdateCommand`</sub>

### drawingContext

- with no selection the context covers the whole sheet <sub>`drawingContextTag`</sub>
- with no sheet the string stays well-formed <sub>`drawingContextTag`</sub>
- a selection lists reference designators (falling back to entity kinds) <sub>`drawingContextTag`</sub>
- selections beyond 10 items collapse into +N more <sub>`drawingContextTag`</sub>
- selection ids absent from the active sheet are ignored <sub>`drawingContextTag`</sub>
- an empty draft receives the context as-is <sub>`appendContextTag`</sub>
- an existing draft is separated by a newline (without doubling) <sub>`appendContextTag`</sub>

### i18n

- defaults to English and falls back to English <sub>`i18n`</sub>
- ships English and Japanese catalogs <sub>`i18n`</sub>
- keeps the English and Japanese catalogs key-identical <sub>`i18n`</sub>
- rejects empty strings in either catalog <sub>`i18n`</sub>
- resolves language tags leniently and falls back to English for unknown values <sub>`i18n`</sub>

### chat

- text deltas append to the in-progress message of a known conversation <sub>`chat store: applyAgentEvent`</sub>
- turn_completed stops streaming and finalizes the token usage <sub>`chat store: applyAgentEvent`</sub>
- when no deltas arrived, the body is filled from turn_completed's result <sub>`chat store: applyAgentEvent`</sub>
- tool calls become chips, resolved to success/failure by id <sub>`chat store: applyAgentEvent`</sub>
- duplicate tool starts with the same id are ignored <sub>`chat store: applyAgentEvent`</sub>
- tool completions without an id match the running chip of the same name <sub>`chat store: applyAgentEvent`</sub>
- an error event marks the message and stops streaming <sub>`chat store: applyAgentEvent`</sub>
- turn_applied records the revision range and marks the turn applied <sub>`chat store: applyAgentEvent`</sub>
- an undo depth on turn_applied records the exact edit count <sub>`chat store: applyAgentEvent`</sub>
- folding events advances the conversation's updated_at (newest-first history) <sub>`chat store: applyAgentEvent`</sub>
- a delta after completion starts a new turn <sub>`chat store: applyAgentEvent`</sub>
- multiple conversations fold independently; streaming stays on while any is running <sub>`chat store: applyAgentEvent`</sub>
- an unknown conversation id refetches the list instead of creating a ghost <sub>`chat store: applyAgentEvent`</sub>
- while our own turn is running, unknown-conversation events do not trigger a refetch <sub>`chat store: applyAgentEvent`</sub>
- turn_completed/error for a conversation with no running turn is dropped <sub>`chat store: applyAgentEvent`</sub>
- send creates a conversation and adopts the server-assigned id <sub>`chat store: アクション`</sub>
- events arriving before send resolves still land in the same conversation <sub>`chat store: アクション`</sub>
- empty prompts and sends during streaming are ignored <sub>`chat store: アクション`</sub>
- a failed send marks the message with the error and stops streaming <sub>`chat store: アクション`</sub>
- after a failed first send, retrying sends as new without leaking the local id <sub>`chat store: アクション`</sub>
- cancel stops only the target conversation, leaving others streaming <sub>`chat store: アクション`</sub>
- cancel on a not-yet-assigned (local-) conversation skips the API and cleans up locally <sub>`chat store: アクション`</sub>
- a cancel issued before id assignment is sent to the server once the id arrives <sub>`chat store: アクション`</sub>
- streaming stops even if the cancel API fails <sub>`chat store: アクション`</sub>
- cancel with no conversation does nothing and never crashes <sub>`chat store: アクション`</sub>
- undoTurn calls the API and withdraws the applied badge <sub>`chat store: アクション`</sub>
- a server-rejected undoTurn propagates the error and keeps the applied badge <sub>`chat store: アクション`</sub>
- undoTurn never calls the API for a not-yet-assigned (local-) conversation <sub>`chat store: アクション`</sub>
- loadConversations normalizes the Rust representation into the display model <sub>`chat store: アクション`</sub>
- normalizeConversation treats unfinished tools as running <sub>`chat store: アクション`</sub>
- normalizeConversation carries updated_at through unchanged <sub>`chat store: アクション`</sub>
- concurrent subscribe calls result in a single subscription <sub>`chat store: アクション`</sub>
- unsubscribing before the subscription resolves still closes it cleanly <sub>`chat store: アクション`</sub>
- after a failed subscription, the next subscribe re-establishes it <sub>`chat store: アクション`</sub>
- newConversation does not disturb a pending local id <sub>`chat store: アクション`</sub>
- setModel / setPanel / newConversation update their state <sub>`chat store: アクション`</sub>
- the MCP tool-name prefix is stripped for display <sub>`summarizeToolUse`</sub>
- place_symbol calls summarize as symbol, reference and position <sub>`summarizeToolUse`</sub>
- draw_wire calls summarize as color, gauge and point count <sub>`summarizeToolUse`</sub>
- update_entity and execute_commands summarize by command content <sub>`summarizeToolUse`</sub>
- read and export tools get appropriate summaries <sub>`summarizeToolUse`</sub>
- tools without a summary show the tool name only <sub>`summarizeToolUse`</sub>
- the conversation title is the first 40 chars of the first user message <sub>`会話履歴ポップアップの表示ヘルパー`</sub>
- without any user message the title is '(empty conversation)' <sub>`会話履歴ポップアップの表示ヘルパー`</sub>
- relative time renders as just now / N min / N h / yesterday / M-D <sub>`会話履歴ポップアップの表示ヘルパー`</sub>
- unknown timestamps (legacy updated_at=0) show no relative time <sub>`会話履歴ポップアップの表示ヘルパー`</sub>
- the meta line joins time, message count and applied rev with a middle dot <sub>`会話履歴ポップアップの表示ヘルパー`</sub>
- conversations sort newest-first (unknown times last, in insertion order) <sub>`会話履歴ポップアップの表示ヘルパー`</sub>

### document

- entity_upserted / entity_removed patches update the mirrored sheet <sub>`document store`</sub>
- project_replaced swaps the whole mirrored project <sub>`document store`</sub>
- sheet add/remove patches keep sheet order <sub>`document store`</sub>
- sheet-meta patches never touch the entities <sub>`document store`</sub>
- patches with an older revision are discarded (duplicate delivery is safe) <sub>`document store`</sub>

### Parts database

- search stores the results from the parts API <sub>`parts store`</sub>
- a failed search clears the results and resets loading <sub>`parts store`</sub>

### Reports (BOM / wire list)

- offers the five report kinds of the reports tab <sub>`report generation dialog store`</sub>
- opens a project-wide report as a CSV export <sub>`report generation dialog store`</sub>
- switches the same report to a framed drawing-sheet PDF <sub>`report generation dialog store`</sub>
- lets the terminal chart target one terminal block as well as the whole project <sub>`report generation dialog store`</sub>
- offers PDF only for the graphical terminal connection diagram <sub>`report generation dialog store`</sub>
- opens with the drawing-sheet PDF already chosen when asked for it <sub>`report generation dialog store`</sub>
- falls back to a format the report actually has <sub>`report generation dialog store`</sub>
- swaps the targets and the formats when another report is picked in the dialog <sub>`report generation dialog store`</sub>
- suggests a file name from the report, the target and the format <sub>`report generation dialog store`</sub>
- cannot generate anything while the output path is empty <sub>`report generation dialog store`</sub>
- writes the report once, reports how much it wrote and closes <sub>`report generation dialog store`</sub>
- keeps the dialog open and shows the error when the export fails <sub>`report generation dialog store`</sub>
- includes the cover and every report by default <sub>`PDF book dialog store`</sub>
- keeps the chosen reports in the fixed report order <sub>`PDF book dialog store`</sub>
- can drop the cover and every report to export the schematics alone <sub>`PDF book dialog store`</sub>
- cannot export while the output path is empty <sub>`PDF book dialog store`</sub>
- writes one file, reports the page count and closes <sub>`PDF book dialog store`</sub>

### revisions

- adds the first row with mark A and today's date <sub>`revisions dialog store`</sub>
- numbers a new row with the alphabet letter after the highest existing mark <sub>`revisions dialog store`</sub>
- carries the mark from Z to AA <sub>`revisions dialog store`</sub>
- ignores blank or non-alphabetic marks when picking the next one <sub>`revisions dialog store`</sub>
- formats today's date as YYYY-MM-DD <sub>`revisions dialog store`</sub>
- lists the newest revision at the top of the table <sub>`revisions dialog store`</sub>
- edits a private draft and never touches the sheet itself <sub>`revisions dialog store`</sub>
- saves the edited list with a single set_revisions command <sub>`revisions dialog store`</sub>
- sends nothing when the dialog is cancelled <sub>`revisions dialog store`</sub>
- reflects row edits and deletions in the saved list <sub>`revisions dialog store`</sub>
- skips the command when nothing was edited <sub>`revisions dialog store`</sub>
- drops rows left completely blank and trims the remaining text <sub>`revisions dialog store`</sub>
- keeps the dialog open and reports the error when saving fails <sub>`revisions dialog store`</sub>

### AI settings

- defaults enable auto-apply and auto-read-drawing <sub>`settings store`</sub>
- load pulls the settings from the backend <sub>`settings store`</sub>
- save merges the changes, sends them, and adopts the normalized response <sub>`settings store`</sub>
- a failed save keeps the error and leaves the shown settings untouched <sub>`settings store`</sub>

### simulation

- run fetches the DC result and opens the panel <sub>`simulation store`</sub>
- toggled switches are passed as open_switches on the next run <sub>`simulation store`</sub>
- failures (e.g. ngspice missing) keep the error message and still open the panel <sub>`simulation store`</sub>

### terminals

- normalizes a jumper list to ascending, low-first, duplicate-free pairs <sub>`jumper specification`</sub>
- drops malformed entries and jumpers between terminals that are not neighbours <sub>`jumper specification`</sub>
- adds a jumper for adjacent terminals while keeping the existing ones <sub>`jumper specification`</sub>
- chains jumpers along a run of three or more adjacent terminals <sub>`jumper specification`</sub>
- refuses to jumper terminals that are not adjacent <sub>`jumper specification`</sub>
- removes only the jumpers that touch the selected terminals <sub>`jumper specification`</sub>
- loads the terminal blocks of the sheet and shows the first one <sub>`terminal strip editor store`</sub>
- shows one grid row per terminal with the wiring taken from the drawing <sub>`terminal strip editor store`</sub>
- marks terminals with no wire at all as spare <sub>`terminal strip editor store`</sub>
- writes a jumper into the symbol attributes with one update_entity command <sub>`terminal strip editor store`</sub>
- cannot generate a jumper while no terminal is selected <sub>`terminal strip editor store`</sub>
- cannot generate a jumper between terminals that are not adjacent <sub>`terminal strip editor store`</sub>
- clears only the jumpers on the selected terminals <sub>`terminal strip editor store`</sub>
- drops the jumper attribute entirely once no jumper is left <sub>`terminal strip editor store`</sub>
- reloads the chart after editing a jumper so the grid keeps matching the model <sub>`terminal strip editor store`</sub>
- reloads the grid when the drawing changes from outside the editor, e.g. an undo <sub>`terminal strip editor store`</sub>
- does not fetch anything while the editor is closed <sub>`terminal strip editor store`</sub>
- summarizes the terminal block check as counts per severity <sub>`terminal strip editor store`</sub>
- reports a clean terminal block when the check finds nothing <sub>`terminal strip editor store`</sub>
- clears the selection and the previous check result when another block is picked <sub>`terminal strip editor store`</sub>
- shows an empty grid on a sheet without any terminal block <sub>`terminal strip editor store`</sub>
- throws away the chart, the selection and the check result when closed <sub>`terminal strip editor store`</sub>

### ui

- defaults are the project tab with the chat collapsed <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- openAgentTab switches to the agent tab and expands the chat together <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- closeAgentTab returns to the project tab and collapses the chat <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- the chat draft is shared between the dock and the floating card <sub>`ui store: 左ドックのタブとチャット下書き`</sub>
- all view classes are visible by default <sub>`表示クラス (レイヤ)`</sub>
- toggleViewClass hides and re-shows a class <sub>`表示クラス (レイヤ)`</sub>

### verification

- run fetches diagnostics, opens the panel, and counts by severity <sub>`verification store`</sub>
- close hides the panel but keeps the diagnostics <sub>`verification store`</sub>
- a failed run resets the running flag and leaves diagnostics empty <sub>`verification store`</sub>

