# Detailed Specification

**日本語: [13-specification.ja.md](13-specification.ja.md)** | ← [Roadmap](12-roadmap.md)

> **Generated from the test suite — do not edit by hand.**
> Every clause below is enforced by an automated test; the test id is shown in gray.
> Regenerate with `python3 scripts/gen_spec.py` after changing tests.

This document is the living, always-verified specification of MadakeCAD:
if a behavior is listed here, a test proves it on every run of the suite.


**1339 specification clauses** across 6 areas.


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
- SetPlcAssignments replaces the whole PLC I/O assignment table of the project; undo restores the previous table. <sub>`set_plc_assignments_is_undoable`</sub>
- A harness boundary is added, renamed and deleted with the ordinary entity commands, and every step can be undone. <sub>`harness_add_rename_delete_are_undoable`</sub>
- Moving a harness boundary shifts all of its corners, and undo puts them back exactly. <sub>`harness_move_and_undo_restores_every_corner`</sub>
- A harness boundary survives a JSON round trip with the kind tag "harness", so any client can send it. <sub>`harness_command_json_roundtrip_uses_the_harness_kind`</sub>
- Every execute/undo/redo increases the document revision, so clients can discard stale patches. <sub>`revision_increases_monotonically`</sub>
- Commands targeting a non-existent sheet fail with an error and change nothing. <sub>`unknown_sheet_is_rejected`</sub>
- Undo with an empty history returns None instead of an error. <sub>`undo_on_empty_history_returns_none`</sub>
- A new edit after undo clears the redo history (standard editor behavior). <sub>`new_edit_after_undo_clears_redo`</sub>
- Updating an entity that does not exist fails and does not sneak the new entity into the sheet. <sub>`updating_a_missing_entity_fails_without_inserting_it`</sub>
- Every history entry records who made the edit; plain execute() counts as a user edit. <sub>`execute_as_records_the_edit_origin`</sub>
- Reverting a range rolls back only the agent's edits in it and keeps the user's own edits, even when they were interleaved. <sub>`revert_range_rolls_back_agent_edits_and_keeps_user_edits`</sub>
- A revert is a normal edit in the history, so undoing it brings the agent's work back. <sub>`revert_range_is_itself_undoable`</sub>
- If the agent's edit cannot be undone against the current drawing (the user deleted the target), the whole revert is refused and nothing changes. <sub>`revert_range_refuses_conflicting_reverts_without_partial_changes`</sub>
- Commands run as one batch become a single history entry, so one undo removes all of them at once (and one redo brings them all back). <sub>`execute_batch_is_undone_in_one_step`</sub>
- If any command in a batch fails, the whole batch is refused: the drawing is unchanged and nothing lands in the history. <sub>`execute_batch_refuses_everything_when_one_command_fails`</sub>
- An empty batch changes nothing at all: no history entry and no new document revision. <sub>`empty_batch_changes_nothing`</sub>
- Reverting a range with no edits of that origin reports "nothing to do" instead of touching the drawing. <sub>`revert_range_without_matching_edits_changes_nothing`</sub>
- Registering a FreeCAD link stores it under the entity id, registering again replaces it, removing it takes it away, and each step undoes back to the previous list. <sub>`mech_links_are_upserted_removed_and_undone`</sub>
- Writing measured wire lengths back sets each wire's length and marks it as measured by FreeCAD in one undo step; unchanged wires are skipped and an unknown wire is an error. <sub>`wire_lengths_write_back_with_their_source`</sub>

### DXF interop (AutoCAD Electrical / EPLAN)

- The exported DXF is an AutoCAD 2000 (AC1015) file in millimetres with HEADER, TABLES, BLOCKS, ENTITIES and OBJECTS sections and unique entity handles. <sub>`export_writes_a_well_formed_ac1015_file`</sub>
- Wires are written as one LINE per segment on the WIRES layer with the Y axis flipped to DXF's upward convention. <sub>`wires_become_lines_on_the_wires_layer_with_y_flipped`</sub>
- Each symbol becomes an INSERT of a block named MDK_<symbol_id> (defined once) with rotation, a negative X scale for mirroring, and TAG1/CAT/DESC1/RATING1/TERMnn attributes. <sub>`symbols_become_block_inserts_with_attributes`</sub>
- Wire numbers go to the WIRENO layer, net labels to LABELS, notes to MISC, and a harness becomes a closed dashed polyline on HARNESS with its name. <sub>`texts_and_harness_go_to_their_layers`</sub>
- Non-ASCII text is written as \U+XXXX escapes and decoded back; MTEXT paragraph breaks and formatting codes are handled on read. <sub>`unicode_is_escaped_and_decoded`</sub>
- Exporting a sheet to DXF and importing it back keeps the paper, symbols (id, position, rotation, mirror, reference, value, attributes), wires, junctions, wire numbers, labels, notes and harness names. <sub>`export_then_import_round_trips_the_sheet`</sub>
- Importing an AutoCAD Electrical style DXF reads lines on wire layers (WIRES, _MULTI_WIRE_*) as wires, assigns WIRENO texts to the nearest wire, keeps other lines and unknown blocks as skipped items, and decodes text. <sub>`imports_acade_style_wires_and_reports_unknown_blocks`</sub>
- When the import options name the wire layers explicitly, only those layers become wires. <sub>`explicit_wire_layers_limit_what_becomes_a_wire`</sub>
- A DXF without any wire layer reads every line as a wire and says so in a warning; a WIRENO text with no wire nearby becomes a note. <sub>`without_wire_layers_all_lines_become_wires`</sub>
- A DXF in inches is converted to millimetres, and the paper size is chosen as the smallest ISO A size that fits the drawing extents. <sub>`inch_files_are_scaled_and_paper_fits_extents`</sub>
- A block insert rotated by an angle that is not a multiple of 90 degrees is placed at 0 degrees with a warning. <sub>`non_right_angle_rotation_is_rounded_with_a_warning`</sub>
- A file without an ENTITIES section is rejected as not a DXF, and a group code that is not a number is a syntax error with its line number. <sub>`rejects_non_dxf_and_reports_syntax_errors`</sub>
- Symbol block graphics are written in local coordinates with Y flipped; arcs keep their sweep because the clockwise paper-space direction becomes counter-clockwise in DXF. <sub>`block_graphics_are_y_flipped_and_arcs_keep_direction`</sub>

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
- A project file saved before the PLC assignment table existed still opens: it gets an empty table and is brought up to the current format version. <sub>`an_old_project_file_opens_with_an_empty_plc_assignment_table`</sub>
- A format 2 file (no FreeCAD links, wires without a length source) opens with an empty link list and every wire length marked as manual. <sub>`a_format_2_file_opens_with_empty_mech_links_and_manual_lengths`</sub>
- A format 3 file whose FreeCAD links have no placement opens with those links unplaced, and a link with a placement round-trips through save and load. <sub>`a_format_3_file_opens_with_unplaced_links_and_placements_round_trip`</sub>

### KiCad import / export

- KiCad import converts paper size, title block, wires, junctions, labels (power symbols become net labels) and text. <sub>`imports_paper_title_block_and_geometry`</sub>
- Known lib_ids map to our symbols (Device:R -> resistor, Conn_01x03 -> connector_3p) keeping designator/value/rotation/mirror; unknown symbols are skipped and itemized in the report. <sub>`maps_symbols_and_reports_skipped`</sub>
- A file that is not a kicad_sch document is rejected with a clear error. <sub>`rejects_non_schematic`</sub>
- The S-expression parser reads atoms, quoted strings, numbers and nested lists with typed accessors. <sub>`parses_atoms_strings_numbers_and_nesting`</sub>
- Escaped quotes/newlines and multibyte (Japanese) text inside strings parse correctly. <sub>`parses_escaped_strings_and_multibyte`</sub>
- children(name) iterates every child list with the given head symbol. <sub>`children_iterates_all_matches`</sub>
- Unbalanced parentheses and unterminated strings are reported as syntax errors with a position. <sub>`syntax_errors_are_reported`</sub>
- Exporting a sheet to .kicad_sch and importing it back keeps paper, title block, symbols (id, reference, value, rotation, mirror), wires, junctions, labels and text. <sub>`export_then_import_round_trips_the_sheet`</sub>
- A wire drawn as a polyline is exported as one two-point KiCad wire per segment. <sub>`polyline_wires_are_split_into_segments`</sub>
- Every symbol used on the sheet is embedded once in lib_symbols with its graphics and pins, so KiCad opens the file without MadakeCAD libraries. <sub>`used_symbols_are_embedded_in_lib_symbols`</sub>
- A wire number becomes a KiCad label on the numbered net, so the net keeps its name in KiCad and on re-import. <sub>`wire_numbers_become_labels`</sub>
- A harness boundary is exported as a dashed closed polyline plus a text with its name. <sub>`harness_becomes_dashed_polyline_with_name`</sub>
- Quotes, backslashes and newlines in text are escaped so the exported file parses and reads back unchanged. <sub>`strings_are_escaped_and_read_back`</sub>
- Importing a schematic exported by MadakeCAD resolves "MadakeCAD:<id>" library ids directly, including parametric terminal blocks and connectors. <sub>`madakecad_lib_ids_resolve_directly_on_import`</sub>

### Circuit macros

- Saving a macro stores every coordinate relative to the base point, which is the pin closest to the bottom-left of the selection. <sub>`saving_a_macro_uses_the_bottom_left_pin_as_the_base_point`</sub>
- When the selection has no symbol pins at all, the base point falls back to the bottom-left corner of the bounding box. <sub>`the_base_point_falls_back_to_the_bounding_box_corner_without_pins`</sub>
- Wire numbers are dropped when a macro is saved (they are renumbered per drawing), while net labels are kept as part of the circuit. <sub>`saving_a_macro_drops_wire_numbers_but_keeps_net_labels`</sub>
- Saving refuses an empty selection, and an entity id that is not on the sheet is reported instead of silently skipped. <sub>`saving_a_macro_refuses_an_empty_or_unknown_selection`</sub>
- A macro saved without an id gets a stable one derived from its name, so the save dialog only has to ask for a name. <sub>`a_macro_without_an_id_derives_one_from_its_name`</sub>
- Inserting a macro puts its base point exactly under the cursor position. <sub>`inserting_a_macro_lands_the_base_point_on_the_cursor`</sub>
- Inserting a macro rotated turns the whole circuit around the insertion point and turns each symbol with it. <sub>`inserting_a_macro_rotated_turns_the_whole_circuit`</sub>
- Inserting a macro renumbers its reference designators from the highest one already used in the project, keeping the macro's own relations (K1/K2 stay two different relays, and a coil and its contact keep sharing one designator). <sub>`inserting_a_macro_renumbers_references_after_the_existing_ones`</sub>
- Any wire number left in a hand-written macro file is cleared on insert, so numbering always belongs to the drawing it lands in. <sub>`wire_numbers_in_a_macro_file_are_cleared_on_insert`</sub>
- A macro can hold alternative variants: the default commands are variant "A" and any other key picks its own circuit. <sub>`a_variant_can_be_chosen_when_inserting`</sub>
- An unknown variant key is refused with an error naming the key, and the drawing is left untouched. <sub>`an_unknown_variant_key_is_refused`</sub>
- Inserting a macro is one edit: a single undo takes the whole circuit back out, and a single redo brings it back. <sub>`inserting_a_macro_is_undone_in_one_step`</sub>
- Saving a circuit as a macro and inserting it back at its base point reproduces the drawing exactly: same pins, wires, junctions and labels at the same coordinates. <sub>`a_saved_macro_reproduces_the_drawing_when_inserted_back`</sub>
- A motor circuit saved as a macro can be inserted twice into another sheet: the two copies never share a reference designator and the drawing still passes verification with no errors or ERC warnings. <sub>`a_macro_inserted_twice_keeps_references_unique_and_passes_verification`</sub>
- Macros are written to and listed from the user's macros folder, and a file that is not a valid macro is reported with its path and reason while the others stay usable. <sub>`macros_round_trip_through_the_user_folder_and_broken_files_are_reported`</sub>
- A macro file written before value sets existed still loads and inserts: it simply has no placeholders and no value sets. <sub>`a_macro_file_without_placeholders_still_loads_and_inserts`</sub>
- Choosing a value set on insert writes its values into the value field of the symbols the placeholder points at. <sub>`a_chosen_value_set_fills_in_the_value_field_of_its_targets`</sub>
- A placeholder can point at a named attribute (attrs.<name>) instead of the value field, so ratings and part numbers land in their own slots. <sub>`a_placeholder_can_write_into_a_named_attribute`</sub>
- One value of a value set reaches every target of its placeholder at once, across several symbols and fields, and a value set can carry several keys. <sub>`one_value_set_updates_every_target_of_every_key_at_once`</sub>
- Inserting without choosing a value set leaves every field exactly as it was saved. <sub>`inserting_without_a_value_set_keeps_the_saved_values`</sub>
- An unknown value set id is refused with the id in the message, and nothing is placed. <sub>`an_unknown_value_set_id_is_refused_and_places_nothing`</sub>
- A value set that points at an entity the macro does not contain is refused, and none of its other values are applied either (no half-filled circuit). <sub>`a_target_that_is_not_in_the_macro_is_refused_without_applying_anything`</sub>
- A value set naming a placeholder key the macro does not declare is refused with the key in the message. <sub>`a_value_set_key_without_a_placeholder_is_refused`</sub>
- Saving refuses a placeholder that points outside the selection or writes into a field that does not exist, so a macro can never be saved with a target it cannot reach. <sub>`saving_refuses_a_placeholder_target_outside_the_selection_or_an_unknown_field`</sub>
- Inserting with a value set is still one edit: a single undo takes the whole circuit, values and all, back out. <sub>`inserting_with_a_value_set_is_still_undone_in_one_step`</sub>
- Placeholders and value sets survive the round trip through the macros folder, so a macro saved with them can be inserted later with any of its value sets. <sub>`placeholders_and_value_sets_round_trip_through_the_user_folder`</sub>
- Inserting into a sheet that is not in the project is refused, and an unknown macro id is refused by name. <sub>`inserting_into_an_unknown_sheet_or_by_an_unknown_id_is_refused`</sub>

### Document model

- Paper sizes follow ISO A-series dimensions, and portrait orientation swaps width and height. <sub>`paper_sizes_match_iso_and_orientation_swaps`</sub>
- A new project starts with one sheet named "Sheet1", the current file format version and an empty PLC assignment table. <sub>`new_project_has_one_default_sheet`</sub>
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
- An arc inside a symbol turns with the symbol, so the contactor's half circle keeps facing its moving contact at every rotation, and mirroring flips it without reversing the drawing direction. <sub>`arc_angles_follow_symbol_rotation_and_mirror`</sub>

### ngspice runner

- The ngspice 'print all' output parses into node voltages (v(n1) -> n1) and branch currents (v1#branch), ignoring noise lines. <sub>`parse_print_all_reads_nodes_and_branches`</sub>
- The ngspice executable is located by priority: MADAKE_NGSPICE env var, then PATH, then OS default install paths; a missing env path falls through. <sub>`find_in_prefers_env_then_path_then_candidates`</sub>
- Running a DC operating point on a 24 V / 6+6 ohm divider yields 12 V at the midpoint and 2 A of source current (requires ngspice; skipped otherwise). <sub>`run_op_solves_a_divider_when_ngspice_is_installed`</sub>

### Parts database

- Opening a new database creates the schema and seeds sample parts exactly once; reopening never re-seeds. <sub>`open_creates_schema_and_seeds_samples_once`</sub>
- Parts can be inserted, updated by part number, fetched, and searched by partial name match or exact category. <sub>`upsert_get_and_search`</sub>
- Wire parts are registered and looked up by exact color + gauge combination. <sub>`wire_parts_crud_and_lookup`</sub>
- An old schema-v1 database migrates to v2 on open, preserving existing rows and gaining the spice_model column. <sub>`v1_database_migrates_to_v2_preserving_data`</sub>
- An old schema-v2 database migrates to v3 on open, preserving existing rows and gaining the contact_config column. <sub>`v2_database_migrates_to_v3_preserving_data`</sub>
- The bundled sample relay carries its contact configuration, so a freshly placed relay can be checked for contact overflow. <sub>`sample_relay_part_has_a_contact_configuration`</sub>
- An old schema-v3 database migrates to v4 on open, preserving existing rows and gaining the plc_module column. <sub>`v3_database_migrates_to_v4_preserving_data`</sub>
- The bundled samples include three PLC modules that cover the Mitsubishi, Siemens and Allen-Bradley address styles. <sub>`sample_plc_modules_cover_the_three_address_styles`</sub>
- The PLC module list contains only parts that carry a module definition, so ordinary parts never show up in the module library. <sub>`the_plc_module_list_contains_only_parts_with_a_module_definition`</sub>
- The database path defaults to the OS app-data folder and can be overridden with MADAKE_PARTS_DB. <sub>`default_path_respects_env_override`</sub>

### PDF output

- PDF text is mapped to a Japanese-capable system font per OS: Hiragino Sans/Menlo on macOS, Yu Gothic UI/Consolas on Windows, Noto Sans CJK JP/DejaVu Sans Mono elsewhere. <sub>`preferred_families_follow_the_operating_system`</sub>
- PDF export produces a valid PDF document (%PDF- header) of non-trivial size, including Japanese text. <sub>`sheet_to_pdf_produces_pdf_bytes`</sub>
- The PDF page is exactly the size of the paper (A3 landscape = 420x297mm), so printing at 100% is 1:1. <sub>`pdf_page_is_the_size_of_the_paper`</sub>
- A PDF book is ordered cover page, then every circuit sheet, then the selected report pages. <sub>`pdf_book_is_cover_then_sheets_then_reports`</sub>
- A PDF book can also carry the graphical terminal diagram, which lands among the report pages in the order it was listed. <sub>`pdf_book_can_include_the_terminal_diagram`</sub>
- With no reports selected a PDF book holds just the cover and the circuit sheets. <sub>`pdf_book_without_reports_is_cover_and_sheets_only`</sub>
- The cover page can be turned off, leaving the circuit sheets first. <sub>`pdf_book_can_omit_the_cover`</sub>
- Exporting the book writes one PDF document holding every page, with report pages on A4 even when the circuit is A3. <sub>`pdf_book_merges_every_page_into_one_document`</sub>
- A book of a project with no sheet at all still produces a valid one-page PDF (the cover). <sub>`pdf_book_of_an_empty_project_is_just_the_cover`</sub>

### PLC I/O

- Mitsubishi-style addresses count up in octal, so the eighth point of an X module is X10 rather than X8. <sub>`mitsubishi_addresses_count_up_in_octal`</sub>
- Siemens-style addresses are written as byte.bit with eight bits per byte, so the ninth point is %I1.0. <sub>`siemens_addresses_are_written_as_byte_and_bit`</sub>
- Allen-Bradley-style addresses are written as word/bit with sixteen bits per word, so the seventeenth point is I:1/0. <sub>`allen_bradley_addresses_are_written_as_word_and_bit`</sub>
- Auto-numbering can start from a point offset, so a second module continues where the first one ended. <sub>`auto_addresses_can_start_from_a_point_offset`</sub>
- A module definition knows which dynamic symbol to place: input modules use plc_di_{n}p and output modules plc_do_{n}p. <sub>`a_module_definition_names_its_drawing_symbol`</sub>
- A module definition survives the round trip through the parts-database JSON column. <sub>`a_module_definition_round_trips_through_json`</sub>
- The assignment table is written to CSV as address, signal name and comment, and reading it back gives the same rows. <sub>`the_assignment_table_round_trips_through_csv`</sub>
- A CSV without a header row is read as data, so a spreadsheet exported without titles still loads. <sub>`a_csv_without_a_header_row_is_still_read`</sub>
- Importing a CSV replaces only the target module's rows and leaves other modules untouched; one undo puts the old table back. <sub>`importing_a_csv_replaces_only_the_target_module`</sub>
- A CSV row without the three columns is refused, and the drawing keeps its old assignment table. <sub>`a_malformed_csv_row_is_refused_without_changing_the_table`</sub>
- Each point of the assignment table picks up the connected device and the wire number from the drawing. <sub>`each_point_picks_up_its_target_and_wire_number_from_the_drawing`</sub>
- The modules placed on the drawing are listed with their point count and I/O kind, so the editor can offer them for selection. <sub>`placed_modules_are_listed_with_their_point_count_and_kind`</sub>
- Generating an I/O drawing adds one new sheet whose ladder has one rung per point of the module. <sub>`generating_an_io_drawing_makes_one_rung_per_point`</sub>
- Rungs sit at the requested vertical spacing, and the requested number of leading rung positions is left empty. <sub>`rungs_follow_the_requested_spacing_and_leading_skip`</sub>
- The whole generated page is a single edit, so one undo removes the sheet and everything on it. <sub>`a_generated_io_page_is_undone_in_one_step`</sub>
- Generating twice adds a second sheet instead of overwriting the first one. <sub>`generating_twice_adds_a_second_sheet`</sub>
- A generated I/O page has no ERC complaints: every point is wired and every wire end lands on something. <sub>`a_generated_io_page_passes_the_electrical_rule_check`</sub>
- Generating for a module with no assignments fills the table with auto-numbered addresses in the same single edit. <sub>`generating_fills_an_empty_assignment_table_with_auto_addresses`</sub>
- Existing signal names are kept when the page is generated: the assignment table stays the master of names and comments. <sub>`generating_keeps_the_signal_names_already_in_the_table`</sub>
- The two module-placement policies that are not implemented yet are refused with a clear error, and the drawing is left untouched. <sub>`the_unimplemented_placement_policies_are_refused`</sub>
- The horizontal-bus ladder style is not implemented yet and is refused the same way. <sub>`the_horizontal_bus_ladder_style_is_refused`</sub>
- A rung spacing that is not a positive multiple of the 2.5 mm grid is refused, so generated pages stay on grid. <sub>`an_off_grid_rung_spacing_is_refused`</sub>
- The I/O report lists every point as address, signal name, target, wire number and comment, with the module reference in front for the whole project. <sub>`the_io_report_lists_address_signal_target_wire_number_and_comment`</sub>

### relay_xref

- A coil and the contacts that carry the same reference designator form one relay device. <sub>`same_reference_groups_coil_and_contacts_into_one_device`</sub>
- Symbols that are not relay coils or contacts never become part of a relay device. <sub>`non_relay_symbols_are_not_relay_devices`</sub>
- The contacts of a device are ordered by sheet, then by zone, so the numbering follows the reading order of the drawing. <sub>`contacts_are_ordered_by_sheet_then_zone`</sub>
- Terminal pairs follow IEC 60947-1: the leading digit is the contact position, the last digits are 3/4 for make contacts and 1/2 for break contacts. <sub>`terminal_pairs_follow_iec_position_and_function_digits`</sub>
- The contact map under a coil lists every placed contact with its terminal pair and its drawing address. <sub>`contact_map_lists_used_contacts_with_their_addresses`</sub>
- When the part's contact configuration is known, the unused contacts are listed too, with a dash instead of an address. <sub>`contact_map_shows_dash_for_unused_contacts`</sub>
- Without a contact configuration only the contacts actually drawn are listed; no empty rows are invented. <sub>`contact_map_omits_unused_rows_without_contact_config`</sub>
- Each contact shows the address of the coil that drives it, in parentheses. <sub>`contact_shows_the_location_of_its_coil`</sub>
- A contact whose coil is missing shows no coil location at all. <sub>`contact_without_a_coil_shows_no_location`</sub>
- The per-sheet contact maps are keyed by the coil entity, so only coils on that sheet carry a table. <sub>`sheet_contact_maps_are_keyed_by_the_coil_on_that_sheet`</sub>
- A contact configuration like "2NO+2NC" is read as the number of make and break contacts the part actually has. <sub>`contact_config_parses_make_and_break_counts`</sub>
- An unreadable contact configuration is ignored instead of guessing a number of contacts. <sub>`unreadable_contact_config_is_ignored`</sub>
- Using more contacts than the assigned part provides is an error, naming the type that ran out. <sub>`using_more_contacts_than_the_part_has_is_an_error`</sub>
- A contact with no coil of the same reference anywhere in the project is an error. <sub>`a_contact_without_a_coil_is_an_error`</sub>
- A coil that drives no contact at all is only a warning, because the contact may still be planned. <sub>`a_coil_without_contacts_is_a_warning`</sub>
- A contact configuration that cannot be read is reported as a warning, because it silently disables the contact count check. <sub>`an_unreadable_contact_config_is_a_warning`</sub>
- A correctly wired relay (coil plus contacts within the part's configuration) produces no cross-reference diagnostics. <sub>`a_correct_relay_produces_no_diagnostics`</sub>
- The whole-project verification includes the relay cross-reference checks. <sub>`project_verification_includes_relay_checks`</sub>
- The contact map table is centred under the coil and grows downwards, one row per contact. <sub>`contact_map_table_is_centred_under_the_coil`</sub>
- The contact map hangs below the coil symbol's outline, and the coil location text sits to the right of the contact symbol. <sub>`annotations_are_anchored_to_the_symbol_outline`</sub>

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
- The PLC I/O report is available as CSV and as a framed drawing sheet, with one row per assigned I/O point. <sub>`the_plc_io_report_has_one_row_per_assigned_point`</sub>
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

### search

- One search covers all five targets at once: reference designators, part numbers, net names, wire numbers and free text. <sub>`search_covers_all_five_targets`</sub>
- The query matches any part of a word and ignores upper/lower case, so "my2" finds the part number "MY2N". <sub>`search_is_case_insensitive_and_partial`</sub>
- Ticking only some filter chips narrows the results to those targets, so searching "K1" with the reference filter drops the text note. <sub>`kinds_filter_narrows_the_targets`</sub>
- An empty query finds nothing at all (it never dumps the whole drawing), and blank fields are never matched. <sub>`empty_query_finds_nothing`</sub>
- Results come back in reading order — sheet number, then zone, then entity — so the same drawing always lists them the same way. <sub>`results_are_in_reading_order`</sub>
- Each hit knows where it lives — sheet, zone and the entity to select — and prints its address as "/sheet.zone". <sub>`hit_carries_its_location`</sub>
- A part-number hit shows which device it belongs to, and a reference hit shows that device's part number. <sub>`hits_carry_the_partner_field_as_detail`</sub>
- A hit on a symbol also says what that symbol does in its device, so the result list can read "relay contact 13-14". <sub>`symbol_hits_say_what_the_symbol_does`</sub>
- Attribute values such as the contact configuration are searched together with the part number. <sub>`attribute_values_are_searched_as_part_numbers`</sub>
- A relay device lists its coil first and then its contacts, using the same terminal numbers as the contact map. <sub>`relay_device_lists_coil_then_contacts`</sub>
- A terminal block appears as one "terminals" row covering every pole, with the pole count on the device. <sub>`terminal_block_device_shows_its_terminal_range`</sub>
- An ordinary part that has no separable functions shows up as a single "body" row, named after its symbol. <sub>`plain_part_is_a_single_body_row`</sub>
- Devices are keyed by reference designator, so the same designator on several sheets is one device, and symbols without a designator are not devices at all. <sub>`devices_group_by_reference_across_sheets`</sub>
- An empty project has no devices, and the tree is ordered by reference designator. <sub>`device_tree_is_ordered_and_can_be_empty`</sub>
- Every function row knows the sheet and zone to jump to, which is what the navigator and the reference surfer use to reveal it. <sub>`every_function_knows_where_to_jump`</sub>
- Filter names travel over the API as snake_case strings, and unknown names are rejected rather than guessed. <sub>`filter_names_parse_from_the_api`</sub>

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
- The sheet holding the coil shows a contact map under it: one row per contact with its terminal pair and its address. <sub>`svg_draws_the_contact_map_under_the_coil`</sub>
- The sheet holding the contact shows the coil's address in parentheses beside it. <sub>`svg_draws_the_coil_location_beside_the_contact`</sub>
- Exporting a single sheet on its own draws no contact map, because the counterpart sheets are unknown. <sub>`svg_of_a_lone_sheet_has_no_contact_map`</sub>

### Symbol library

- Every bundled symbol has a unique id and at least one pin. <sub>`builtin_symbols_have_unique_ids_and_pins`</sub>
- A parametric terminal block (e.g. 8 poles) has one left and one right connection point per terminal, all on the 2.5 mm grid and vertically centered. <sub>`dynamic_terminal_block_has_through_pins_on_grid`</sub>
- connector_2p generated dynamically has exactly the same pin coordinates as the old static definition, so existing drawings are unaffected. <sub>`dynamic_connector_2p_matches_legacy_static_def`</sub>
- resolve_symbol finds built-in ids, and rejects malformed or out-of-range dynamic ids (0 poles, 51 poles, missing count). <sub>`resolve_symbol_rejects_invalid_ids_and_finds_builtins`</sub>
- sheet_symbol_defs returns the built-in library plus definitions for every dynamic symbol actually used on the sheet. <sub>`sheet_symbol_defs_includes_dynamic_ids_in_use`</sub>
- A PLC input module symbol (e.g. 16 points) is a tall box with one connection point per I/O point on its left side, all on the 2.5 mm grid. <sub>`a_plc_input_module_has_one_connection_point_per_io_point`</sub>
- PLC modules come in an input and an output flavour, and both carry the PLC reference prefix. <sub>`plc_modules_come_in_input_and_output_flavours`</sub>
- PLC module symbols exist from 1 to 64 points; anything outside that range is not a symbol. <sub>`plc_module_point_counts_are_limited_to_one_through_sixty_four`</sub>
- The relay coil symbol carries the JIS coil terminal names A1 and A2 and the reference prefix K. <sub>`relay_coil_has_a1_a2_terminals`</sub>
- Both relay contact types (make and break) exist, share the coil's reference prefix, and have their two connection points on the 2.5 mm grid. <sub>`relay_contacts_come_in_make_and_break_types`</sub>
- Symbol definitions serialize to JSON and back without loss. <sub>`symbol_json_roundtrip`</sub>
- The bundled library covers the JIS C 0617 main symbols at a 50-symbol scale, with unique ids. <sub>`library_covers_the_main_jis_symbols`</sub>
- Every pin sits on the 2.5 mm grid, has a unique number per connection point, and points away from the body of the symbol. <sub>`every_pin_is_on_the_grid_and_points_outwards`</sub>
- Every symbol carries the four attribute slots (reference, part number, description, rating) placed clear of its outline. <sub>`every_symbol_exposes_the_standard_attribute_slots`</sub>
- Symbols are listed grouped by category in the display order used by the insert dialog, and every symbol is searchable by English and Japanese keywords. <sub>`symbols_are_grouped_by_category_in_display_order`</sub>
- The symbols shipped in the first version keep their id, category, reference prefix, pin numbers and pin positions, so drawings made before the library grew still render identically. <sub>`legacy_symbols_keep_their_definition`</sub>
- The three-pole contactor draws one make contact per pole with the contactor半円, terminals 1/2, 3/4, 5/6 and a dashed mechanical link. <sub>`contactor_3p_has_three_ganged_main_contacts`</sub>
- The single-pole circuit breaker (MCB) is a make contact whose fixed contact carries the breaker cross, with terminals 1 and 2. <sub>`circuit_breaker_1p_marks_the_fixed_contact_with_a_cross`</sub>
- The two- and three-pole circuit breakers repeat the pole at a 5 mm pitch and number the terminals 1/2, 3/4 (and 5/6). <sub>`circuit_breaker_multipole_numbers_terminals_by_pole`</sub>
- The disconnector marks its fixed contact with a short bar at right angles to the blade instead of the breaker cross. <sub>`disconnector_marks_the_fixed_contact_with_a_bar`</sub>
- The three-pole disconnector gangs three blades with terminals 1/2, 3/4, 5/6. <sub>`disconnector_3p_gangs_three_blades`</sub>
- The break-contact pushbutton keeps the two connection points of the make type and adds the button actuator. <sub>`pushbutton_nc_is_a_break_contact_with_a_button_actuator`</sub>
- The emergency stop is a break contact with the mushroom head actuator, so it opens the circuit when hit. <sub>`emergency_stop_is_a_break_contact_with_a_mushroom_head`</sub>
- The changeover switch has one common terminal and two fixed contacts, with the blade resting on the break side while it is not operated. <sub>`switch_spdt_has_a_common_and_two_fixed_contacts`</sub>
- The three-position switch shows the blade in the neutral centre position, touching neither fixed contact. <sub>`switch_3pos_shows_the_blade_in_neutral`</sub>
- The limit switch comes as a make and a break type, both driven by the position-switch actuator (a filled square on the rod). <sub>`limit_switch_comes_in_make_and_break_types`</sub>
- The relay changeover contact uses the IEC terminal numbers 11 (common), 12 (break) and 14 (make). <sub>`relay_contact_co_uses_iec_terminal_numbers`</sub>
- The two-winding transformer draws both windings as arcs on either side of the core, with primary terminals 1/2 and secondary 3/4. <sub>`transformer_has_two_windings_around_a_core`</sub>
- The AC source is a circle with a sine wave inside and two connection points. <sub>`ac_source_is_a_circle_with_a_sine_wave`</sub>
- The bridge rectifier is a diamond with a diode inside, the AC terminals on the left and right and the DC terminals on top (+) and bottom (-). <sub>`rectifier_bridge_has_ac_and_dc_terminals`</sub>
- Protective earth encloses the earth symbol in a circle, while functional (frame) earth uses the chassis symbol; both have a single connection point. <sub>`earth_symbols_distinguish_protective_and_frame_earth`</sub>
- The zener diode keeps the diode outline and bends both ends of the cathode bar. <sub>`zener_diode_bends_the_cathode_bar`</sub>
- The polarized capacitor draws one plate solid and marks the positive terminal. <sub>`capacitor_polarized_marks_the_positive_plate`</sub>
- The inductor is drawn as a row of half circles on the conductor. <sub>`inductor_is_a_row_of_half_circles`</sub>
- The variable resistor adds an arrow across the resistor body. <sub>`resistor_variable_adds_an_arrow_across_the_body`</sub>
- The varistor is a resistor crossed by an oblique line and labelled U, marking it as voltage dependent. <sub>`varistor_is_a_voltage_dependent_resistor`</sub>
- The three-phase motor has the three phase terminals U, V and W leaving the top of the circle. <sub>`motor_3ph_has_u_v_w_terminals`</sub>
- The single-phase and DC motors share the motor circle and are told apart by the 1~ and DC marks inside. <sub>`motor_1ph_and_dc_are_told_apart_by_the_mark_inside`</sub>
- The bell is a dome (half circle on its base line) with two connection points. <sub>`bell_is_a_dome_with_two_terminals`</sub>
- The voltmeter and ammeter are circles marked V and A, wired in the circuit like any two-terminal instrument. <sub>`voltmeter_and_ammeter_are_circles_marked_v_and_a`</sub>
- The current transformer has the primary conductor passing through the core and two secondary terminals S1/S2. <sub>`current_transformer_has_primary_through_and_secondary_terminals`</sub>
- The plug and the socket of a connector pair face each other: the plug is a wedge, the socket the cup that receives it. <sub>`connector_plug_and_socket_face_each_other`</sub>
- A multi-pole device conducts pole by pole (terminals 1-2, 3-4, 5-6), so the phases are never treated as connected to each other. <sub>`multipole_devices_conduct_pole_by_pole`</sub>

### Start templates

- The three bundled templates (24 V control basics, motor starter, emergency stop) are listed in that order, each with an English and a Japanese name and description. <sub>`the_three_bundled_templates_are_listed_in_both_languages`</sub>
- Even without the resource directory, the templates bundled into the build are still available. <sub>`templates_are_available_without_the_resource_directory`</sub>
- Applying "24 V control basics" puts its parts on the sheet: the DC source, the fuse, the 4-pole terminal block and the 24V/0V net labels. <sub>`applying_the_24v_template_places_its_parts_on_the_sheet`</sub>
- A template is applied as one edit: a single undo empties the drawing again, and a single redo brings the whole template back. <sub>`applying_a_template_is_undone_in_one_step`</sub>
- Every bundled template passes verification with zero errors and zero ERC warnings: they are fully wired starting points, not sketches with loose pins. <sub>`every_bundled_template_verifies_without_errors_or_erc_warnings`</sub>
- The same template can be applied twice: the second copy gets fresh entity ids instead of colliding with the first. <sub>`the_same_template_can_be_applied_twice`</sub>
- A template lands on the sheet it was asked for, even when the project has several sheets. <sub>`a_template_lands_on_the_requested_sheet`</sub>
- An unknown template id is refused with an error naming the id, and the drawing is left untouched. <sub>`an_unknown_template_id_is_refused`</sub>
- Templates the user drops into their own templates folder are listed after the bundled ones and can be applied the same way. <sub>`user_templates_are_listed_after_the_bundled_ones`</sub>
- A template file that is not valid JSON (or holds an unknown command) is reported with its path and reason, and the other templates stay usable. <sub>`a_broken_template_file_is_reported_with_its_path`</sub>
- A user template may replace a bundled one by reusing its id (the drawing office's own version wins). <sub>`a_user_template_replaces_the_bundled_one_with_the_same_id`</sub>

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

### Tidy metrics (crossings / overlaps / grid)

- Two wires laid across each other in an X count as one crossing. <sub>`two_wires_laid_across_each_other_count_as_one_crossing`</sub>
- Wires that never meet have no crossings. <sub>`wires_that_never_meet_have_no_crossings`</sub>
- Two wires joined at a shared endpoint are a connection, not a crossing. <sub>`wires_joined_at_a_shared_endpoint_are_not_a_crossing`</sub>
- A T branch, where one wire ends on the middle of another, is a connection point and is not a crossing. <sub>`a_t_branch_landing_on_another_wire_is_not_a_crossing`</sub>
- The corner of one polyline wire is not a crossing: consecutive segments of the same wire are skipped. <sub>`the_corner_of_a_polyline_wire_is_not_a_crossing`</sub>
- A wire routed back over itself so that two of its own separate segments cross does count as a crossing. <sub>`a_wire_crossing_its_own_route_counts_as_a_crossing`</sub>
- Two wires drawn on top of each other along part of their length count as one crossing (the overlap is one place to fix). <sub>`wires_drawn_on_top_of_each_other_count_as_one_crossing`</sub>
- Two wires that lie on the same line but only meet end to end are a connection, not an overlap. <sub>`wires_meeting_end_to_end_on_one_line_are_not_a_crossing`</sub>
- Two notes printed over each other count as one label overlap. <sub>`two_notes_printed_over_each_other_overlap`</sub>
- Two notes whose boxes only touch along an edge are not counted as overlapping. <sub>`notes_that_only_touch_along_an_edge_do_not_overlap`</sub>
- Notes placed well apart do not overlap. <sub>`notes_placed_well_apart_do_not_overlap`</sub>
- A note printed on top of a symbol's outline counts as a label overlap. <sub>`a_note_printed_over_a_symbol_outline_overlaps`</sub>
- A note that stops exactly at the top edge of a symbol is touching, not overlapping. <sub>`a_note_stopping_at_the_symbol_edge_does_not_overlap`</sub>
- Symbols placed on top of each other are counted separately from label overlaps. <sub>`symbols_placed_on_top_of_each_other_are_counted_separately`</sub>
- A symbol's own reference designator never counts as overlapping the symbol it belongs to. <sub>`a_reference_designator_never_overlaps_its_own_symbol`</sub>
- Symbol origins and wire vertices sitting on the 2.5 mm grid report no off-grid points. <sub>`entities_on_the_grid_report_no_off_grid_points`</sub>
- A point shifted by 0.1 mm off the grid is counted, and each stray vertex counts once. <sub>`a_point_shifted_a_tenth_of_a_millimetre_is_off_grid`</sub>
- An empty sheet is perfectly tidy: every metric is zero. <sub>`an_empty_sheet_scores_zero_on_every_metric`</sub>
- tidy_metrics gathers the four counts of one sheet in a single value. <sub>`tidy_metrics_gathers_the_four_counts`</sub>
- Measuring the same sheet twice gives exactly the same numbers, so the agent can compare before and after. <sub>`measuring_the_same_sheet_twice_gives_the_same_numbers`</sub>

### Tidy variants (sheet copies / adopt)

- A variant copy keeps the paper, title block and every entity of the original, with a new sheet id, new entity ids, the label in its name, and a complete old-to-new id map. <sub>`duplicate_keeps_content_with_fresh_ids_and_a_full_map`</sub>
- Starting a run with N variants yields N RestoreSheet commands appended after the existing sheets, each with its own label and id map; N outside 1..=4 is rejected. <sub>`start_builds_one_restore_sheet_per_variant`</sub>
- Adopting a variant writes moved entities back under their original ids, deletes what the variant removed, adds what it created with fresh ids, leaves untouched entities alone, and finally removes every variant sheet. <sub>`adopt_maps_changes_back_to_original_ids`</sub>
- Adopting an unchanged variant only removes the variant sheets. <sub>`adopting_an_unchanged_variant_only_removes_sheets`</sub>
- Through the engine, start → edit a variant → adopt leaves the original sheet with the variant's placement and no variant sheets, and each of the three steps is one undo. <sub>`engine_round_trip_is_one_undo_per_step`</sub>

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
- A multi-pole breaker conducts pole by pole, so a load fed from an unwired pole is still reported as unreachable from the source. <sub>`multipole_breaker_does_not_connect_its_poles_to_each_other`</sub>

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
- Cancel and undo-turn endpoints take a stable turn id and reject turns with nothing to roll back. <sub>`cancel_and_undo_turn_respond`</sub>
- Undoing a turn rolls back exactly the agent's edits through the command engine, keeping a manual edit the user made during the turn. <sub>`undo_turn_rolls_back_agent_edits_and_keeps_the_manual_edit`</sub>
- If a manual edit removed what the turn touched, the rollback is refused with a conflict error and the drawing is left untouched. <sub>`undo_turn_refuses_a_conflicting_rollback`</sub>
- GET /agent/events streams conversation events over SSE, in the same shape as the Tauri agent:event. <sub>`events_endpoint_streams_agent_events`</sub>
- Saving and loading a project carries the chat history alongside (.chat.json). <sub>`save_and_load_carry_the_chat_history`</sub>
- Loading a project cancels any running turn first, so the agent never edits the wrong document. <sub>`load_cancels_a_running_turn`</sub>
- Requests from external web origins are rejected; local origins and non-browser clients (no Origin header) pass. <sub>`external_origins_are_rejected_but_local_and_originless_pass`</sub>
- The origin guard treats only loopback hosts (localhost/127.0.0.1) as local. <sub>`local_origin_predicate_matches_only_loopback_hosts`</sub>
- The drawing context given to the agent summarizes the active sheet (name, nets, entity count). <sub>`drawing_context_summarizes_the_active_sheet`</sub>
- A clean drawing reports zero errors and zero warnings in the drawing context. <sub>`drawing_context_reports_a_clean_drawing_as_no_diagnostics`</sub>
- The drawing context summarizes verification: severity counts plus the first few findings. <sub>`drawing_context_summarizes_the_verification_result`</sub>
- AI settings endpoints persist changes and apply them to the agent manager. <sub>`settings_endpoints_persist_and_apply`</sub>

### REST Link API

- The REST parts endpoints support searching (sample data included), upserting, category filtering, deleting, and listing wire parts. <sub>`parts_endpoints_search_upsert_delete`</sub>
- GET /api/v1/verify returns the drawing's diagnostics as JSON (e.g. empty reference and unconnected pins). <sub>`verify_returns_diagnostics`</sub>
- POST /api/v1/import/kicad replaces the open project with the converted schematic and returns a patch plus an import report. <sub>`import_kicad_replaces_project_and_reports`</sub>
- POST /api/v1/simulate/op solves the DC operating point and returns net voltages and component currents (requires ngspice; skipped otherwise). <sub>`simulate_op_returns_result`</sub>
- POST /api/v1/export/pdf writes a valid PDF file to the requested path. <sub>`export_pdf_writes_pdf_file`</sub>
- POST /api/v1/export/dxf and /export/kicad write the sheet as DXF and .kicad_sch, and POST /api/v1/import/dxf reads the DXF back into a project with an import report. <sub>`export_dxf_kicad_and_import_dxf_round_trip`</sub>
- POST /api/v1/variants/start copies the sheet once per variant (one undo step) and returns each copy's id map; /variants/finish with a chosen sheet writes it back under the original ids and removes the copies, with null it only removes them; a count outside 1..=4 is rejected. <sub>`variants_start_and_finish_round_trip`</sub>
- POST /api/v1/export/pdf-book writes one PDF holding the cover, every sheet and the requested reports. <sub>`export_pdf_book_writes_cover_sheets_and_reports`</sub>
- The terminal endpoints list the terminal blocks of the drawing and return one block's chart and check result. <sub>`terminal_endpoints_list_chart_and_check`</sub>
- POST /api/v1/export/report writes one report as CSV or as framed PDF pages, and refuses CSV for the graphical terminal diagram. <sub>`export_report_writes_csv_and_pdf_per_report`</sub>

### Circuit macros (REST)

- POST /macros/save turns the selected entities into a macro file in the user's macros folder, and GET /macros lists it with its base point. <sub>`saving_a_selection_over_the_link_api_stores_a_macro_in_the_user_folder`</sub>
- POST /macros/apply drops the macro at the requested point as a single edit that one undo takes back, renumbering its reference designators so they do not clash. <sub>`applying_a_macro_over_the_link_api_is_one_undo_step`</sub>
- A macro saved with placeholders and value sets can be inserted over the Link API with one of them chosen, and the values land on the drawing in the same single edit. <sub>`applying_a_macro_with_a_value_set_over_the_link_api_fills_in_the_values`</sub>
- POST /macros/build turns the selection into a macro without writing any file, which is what the save dialog previews and what Cmd+C keeps in memory. <sub>`building_a_macro_does_not_write_a_file`</sub>
- POST /macros/apply-inline drops a macro handed over by value (the Cmd+C clipboard) as a single edit, renumbering its reference designators just like a stored macro. <sub>`applying_an_inline_macro_behaves_like_a_stored_one`</sub>
- An unknown macro id, an unknown variant key and an empty selection are all refused with 400 and leave the drawing untouched. <sub>`the_link_api_refuses_unknown_macros_variants_and_empty_selections`</sub>

### Edit origin (user / agent / mcp)

- Each entry point records who made the edit: the UI is a user edit, the agent's turn is an agent edit, and outside clients are mcp edits. <sub>`every_edit_path_records_who_made_the_change`</sub>
- The agent-turn marker nests and always clears, so edits after the turn are user edits again. <sub>`the_agent_turn_marker_nests_and_always_clears`</sub>
- The bridge the agent manager uses reverts only agent edits and reports how many were rolled back. <sub>`the_agent_bridge_reverts_only_agent_edits`</sub>

### PLC I/O (MCP tool / REST)

- PUT /plc/assignments replaces the whole I/O assignment table, and GET reads it back per module. <sub>`the_assignment_table_can_be_written_and_read_over_the_link_api`</sub>
- POST /plc/assignments/import loads a CSV of address, signal name and comment for one module, and one undo takes the whole import back. <sub>`a_csv_of_signal_names_can_be_imported_and_undone`</sub>
- A CSV whose rows do not have the three columns is refused with 400 and leaves the assignment table untouched. <sub>`a_malformed_csv_import_is_refused_with_a_bad_request`</sub>
- POST /plc/generate builds the I/O ladder page as a single edit: the new sheet carries the module symbol and one rung per point, and GET /plc/modules then lists the placed module. <sub>`generating_an_io_page_over_the_link_api_is_one_undo_step`</sub>
- Generation settings that are not implemented yet (sharing a ladder between modules) are refused with 400 and change nothing. <sub>`an_unimplemented_generation_setting_is_refused_with_a_bad_request`</sub>
- The PLC I/O report is exported through the ordinary report endpoint as "plc-io", listing one row per assigned point. <sub>`the_plc_io_report_is_exported_like_any_other_report`</sub>
- The PLC tools tell the agent what the assignment table is for and that generating a page is one undo step. <sub>`the_plc_tools_explain_the_assignment_table_and_the_generated_page`</sub>

### provider_api

- The settings screen learns which provider is selected and whether a key is saved, but never the key itself. <sub>`the_settings_screen_never_receives_the_api_key`</sub>
- Removing the saved key flips the "saved" flag back, so the settings screen shows it is gone. <sub>`removing_the_saved_key_flips_the_saved_flag_back`</sub>
- An empty key box is refused with an error instead of storing a useless entry. <sub>`an_empty_key_box_is_refused`</sub>
- The connection test answers with a readable reason instead of failing the request itself. <sub>`the_connection_test_answers_with_a_readable_reason`</sub>
- Choosing the Anthropic API through the settings endpoint is reflected in the provider status. <sub>`choosing_the_anthropic_api_is_reflected_in_the_provider_status`</sub>
- If the OS keychain does not answer, the settings screen still opens and says why. <sub>`a_keychain_that_never_answers_does_not_freeze_the_settings_screen`</sub>

### search_api

- GET /search finds every kind of target across the project and returns each hit with the sheet and zone to jump to. <sub>`searching_over_the_link_api_returns_located_hits`</sub>
- The kinds parameter narrows the search to the chosen filter chips, and an unknown filter name is rejected instead of guessed. <sub>`the_kinds_parameter_narrows_the_search`</sub>
- An empty query returns no hits at all, so the search bar never dumps the whole drawing. <sub>`an_empty_query_returns_nothing`</sub>
- GET /devices returns the reference-designator tree the device navigator draws, with each function's terminals and location. <sub>`the_device_tree_comes_back_over_the_link_api`</sub>
- A drawing with nothing on it has an empty device tree rather than an error. <sub>`an_empty_drawing_has_an_empty_device_tree`</sub>

### Start templates (REST)

- GET /templates lists the bundled start templates with their names in both languages. <sub>`the_link_api_lists_the_bundled_templates`</sub>
- POST /templates/apply drops the template on the sheet as a single edit that one undo takes back. <sub>`applying_a_template_over_the_link_api_is_one_undo_step`</sub>
- Applying without a sheet id targets the first sheet, and an unknown template id is refused with 400. <sub>`the_link_api_defaults_to_the_first_sheet_and_refuses_unknown_templates`</sub>

### Tidy metrics (MCP tool / REST)

- The tidy metrics tool tells the agent it is the target to aim at while tidying, and names the three things it counts. <sub>`the_tidy_metrics_tool_is_advertised_as_the_tidy_loop_target`</sub>
- GET /api/v1/tidy-metrics returns the sheet's crossing, label overlap, symbol overlap and off-grid counts as JSON. <sub>`tidy_metrics_endpoint_returns_the_four_counts`</sub>

### tool_bridge

- The API route sees exactly the same tools as the Claude Code CLI route, so neither is missing a feature. <sub>`the_api_route_sees_the_same_tools_as_the_cli_route`</sub>
- Every bridged tool carries a description and an object-shaped input schema. <sub>`every_bridged_tool_has_a_description_and_an_object_schema`</sub>
- An edit made through the bridge lands in the document and can be undone like any other edit. <sub>`an_edit_through_the_bridge_lands_in_the_document_and_can_be_undone`</sub>
- Reading tools work through the bridge too, so the agent can look at the drawing before editing. <sub>`reading_tools_work_through_the_bridge`</sub>
- A tool name that does not exist comes back as an error result instead of killing the turn. <sub>`an_unknown_tool_name_comes_back_as_an_error_result`</sub>
- Bad arguments come back as an error result carrying the reason, so the model can correct itself. <sub>`bad_arguments_come_back_with_a_reason`</sub>

### tools

- The parts search tool advertises selection, comparison and alternative-part use, so the agent reaches for it when asked "what can replace this?". <sub>`the_parts_search_tool_advertises_selection_and_comparison`</sub>
- The template tools explain that a template is a starting skeleton, that it lands ERC-clean, and that applying it is a single undo step. <sub>`the_template_tools_explain_what_a_template_is`</sub>
- Every published tool carries a description, so no tool is offered to the agent unexplained. <sub>`every_published_tool_has_a_description`</sub>


## AI assistant (madake-agent)


### anthropic_api

- Streamed assistant text arrives as text deltas and the turn ends with the assembled reply and its token usage. <sub>`streamed_text_becomes_deltas_and_a_completed_turn`</sub>
- The request carries the configured model, a streaming flag, the system prompt and the user's message. <sub>`the_request_carries_the_model_system_prompt_and_user_message`</sub>
- The bridged MCP tools are offered to the API on every request, so the agent can edit the drawing. <sub>`the_bridged_mcp_tools_are_offered_to_the_api`</sub>
- A tool the model asks for is executed locally and its result is sent back so the model can continue. <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
- A tool that fails is reported back to the model as an error result instead of aborting the turn. <sub>`a_failing_tool_is_reported_to_the_model_as_an_error_result`</sub>
- The tool loop stops after a bounded number of rounds so a looping model cannot run forever. <sub>`the_tool_loop_stops_after_a_bounded_number_of_rounds`</sub>
- A rejected API key produces a message that says the key is the problem, not a raw HTTP code. <sub>`a_rejected_api_key_is_explained_as_a_key_problem`</sub>
- An overloaded API produces a "busy, try again" message rather than a bare error code. <sub>`an_overloaded_api_is_explained_as_a_busy_service`</sub>
- An error event that arrives mid-stream is surfaced to the user too. <sub>`an_error_event_inside_the_stream_is_surfaced`</sub>
- Earlier turns of the conversation are replayed so the model remembers what was said before. <sub>`earlier_turns_of_the_conversation_are_replayed`</sub>
- The connection test reports success for a working key and a readable reason for a bad one. <sub>`the_connection_test_reports_success_or_a_readable_reason`</sub>

### Claude CLI backend

- The Claude CLI is launched headless with the required flags (-p, stream-json output, partial messages, strict MCP config). <sub>`args_contain_required_flags`</sub>
- The bundled documentation folder is opened to the agent as a readable directory, so it can quote the manual. <sub>`args_open_the_bundled_documentation_for_reading`</sub>
- Resume session id, model choice and an appended system prompt are passed through when provided. <sub>`args_include_resume_model_and_system_prompt_when_given`</sub>
- The agent's MCP config points at MadakeCAD's own local MCP server, so it uses the same tools as any client. <sub>`mcp_config_points_at_local_mcp_server`</sub>
- A turn streams events (text deltas, tool use, completion) parsed from the CLI's stream-json output. <sub>`send_streams_events_from_fake_cli`</sub>
- A CLI exiting non-zero is reported as an error event instead of hanging. <sub>`send_reports_nonzero_exit_as_error_event`</sub>
- When the CLI dies with the reason printed as plain text on stdout (e.g. a usage limit), that reason reaches the chat error. <sub>`a_plain_stdout_reason_reaches_the_error_message`</sub>
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
- Recording a rollback clears the turn's applied range, so it is no longer offered for undo. <sub>`record_reverted_clears_the_applied_range`</sub>
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
- The user prompt and the agent reply of one turn share a single stable turn id. <sub>`the_two_messages_of_a_turn_share_one_turn_id`</sub>
- Each turn gets its own turn id, so a turn can be addressed after later turns are appended. <sub>`each_turn_gets_its_own_turn_id`</sub>
- A legacy chat file (older format version, no turn ids) loads with turn ids assigned per turn boundary. <sub>`load_chat_migrates_legacy_files_by_assigning_turn_ids`</sub>
- Saving chat history stamps the current format version so migrated files are not re-migrated. <sub>`save_chat_stamps_the_current_format_version`</sub>
- The chat file lives next to the project file as <name>.chat.json. <sub>`chat_path_sits_next_to_project_file`</sub>

### copilot_cli

- The Copilot CLI is launched non-interactively with the flags needed to work unattended (JSON lines, auto-approved tools, no questions). <sub>`args_contain_required_flags`</sub>
- The first turn of a conversation pins a new session id, so later turns can continue the same session. <sub>`the_first_turn_pins_a_new_session_id`</sub>
- A follow-up turn resumes the same Copilot session instead of starting a new one, so the agent remembers the conversation. <sub>`a_follow_up_turn_resumes_the_same_session`</sub>
- The configured Copilot model is passed to the CLI ("auto" lets Copilot pick). <sub>`the_configured_model_is_passed_to_the_cli`</sub>
- Copilot is pointed at MadakeCAD's own MCP server, so it edits drawings through the same commands as every other client. <sub>`the_mcp_config_points_at_the_local_madakecad_server`</sub>
- Because the Copilot CLI has no system-prompt flag, MadakeCAD's drawing rules are prepended to the prompt under a clear heading. <sub>`the_system_prompt_is_prepended_to_the_user_prompt`</sub>
- A turn streams the events parsed from Copilot's JSON lines (session, text, tool use, completion). <sub>`a_turn_streams_events_from_the_fake_cli`</sub>
- The composed prompt actually reaches the CLI as the -p argument. <sub>`the_composed_prompt_reaches_the_cli`</sub>
- The MCP settings file handed to Copilot really exists while the turn runs, and it names MadakeCAD's server. <sub>`the_mcp_config_file_exists_while_the_turn_runs`</sub>
- When Copilot is not signed in to GitHub, the chat says so and explains how to sign in. <sub>`a_signed_out_cli_is_reported_with_login_guidance`</sub>
- The unauthenticated wording of the real CLI is recognised wherever it is printed. <sub>`the_real_unauthenticated_message_is_recognised`</sub>
- When Copilot dies with the reason printed as plain text (e.g. credits exhausted), that reason reaches the chat error. <sub>`a_plain_stdout_reason_reaches_the_error_message`</sub>
- If the event receiver goes away (the user cancelled), the turn stops promptly instead of blocking forever. <sub>`a_cancelled_turn_stops_promptly`</sub>
- Copilot CLI detection reads the version from the configured executable. <sub>`detect_reads_the_version_from_the_configured_executable`</sub>
- Detection fails cleanly when the Copilot executable is not installed. <sub>`detect_fails_when_copilot_is_not_installed`</sub>
- Detection looks for copilot on PATH and in the usual npm install locations. <sub>`default_candidates_include_the_known_install_paths`</sub>
- The connection test reports success when the CLI answers, and the sign-in guidance when it is signed out. <sub>`the_connection_test_distinguishes_success_from_being_signed_out`</sub>

### copilot_events

- A Copilot session id line becomes the session event used to continue the same conversation. <sub>`session_line_becomes_a_session_event`</sub>
- Assistant text is delivered to the chat whichever field name the CLI uses for it. <sub>`assistant_text_is_delivered_under_any_of_the_known_field_names`</sub>
- A tool call line becomes a tool-start event with its name and arguments. <sub>`a_tool_call_line_becomes_a_tool_start_event`</sub>
- Tool calls are recognised under the alternative field names too (tool/input, function/parameters). <sub>`tool_calls_are_recognised_under_alternative_field_names`</sub>
- A tool result line closes the matching tool call and carries whether it failed. <sub>`a_tool_result_line_closes_the_call_and_reports_failure`</sub>
- The tool name from the call is filled into the matching result, so the chat shows what finished. <sub>`the_tool_name_is_carried_from_the_call_to_its_result`</sub>
- The final line completes the turn with the answer and the token usage. <sub>`the_final_line_completes_the_turn_with_usage`</sub>
- Token usage is also read from OpenAI-style field names (prompt/completion tokens). <sub>`token_usage_is_also_read_from_openai_style_names`</sub>
- An error line from the CLI is shown in the chat as an error. <sub>`an_error_line_becomes_an_error_event`</sub>
- A result line flagged as an error becomes an error instead of a normal completion. <sub>`a_failed_result_line_becomes_an_error`</sub>
- Lines the parser does not understand are ignored instead of breaking the turn. <sub>`unknown_and_broken_lines_are_ignored`</sub>
- The same tool call reported twice is only shown once in the chat. <sub>`a_repeated_tool_call_is_shown_only_once`</sub>

### gemini

- Streamed model text arrives as text deltas and the turn ends with the assembled reply and its token usage. <sub>`streamed_text_becomes_deltas_and_a_completed_turn`</sub>
- The repeated running totals Gemini puts on every chunk are not added up twice. <sub>`repeated_running_token_totals_are_not_counted_twice`</sub>
- The request goes to the streaming endpoint of the configured model and carries the system instruction and the user's message. <sub>`the_request_goes_to_the_streaming_endpoint_with_the_system_instruction_and_message`</sub>
- The bridged MCP tools are offered as Gemini function declarations so the agent can edit the drawing. <sub>`the_bridged_mcp_tools_are_offered_as_function_declarations`</sub>
- Schema keywords Gemini does not accept are dropped from the tool definitions, and a property left without a type still gets one. <sub>`schema_keywords_gemini_rejects_are_dropped_from_tool_definitions`</sub>
- A tool the model asks for is executed locally and its result is sent back as a functionResponse so the model can continue. <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
- A model that omits the call id still gets its result back, matched by the function name. <sub>`a_call_without_an_id_is_answered_by_function_name`</sub>
- A tool that fails is reported back to the model as an error field instead of aborting the turn. <sub>`a_failing_tool_is_reported_to_the_model_as_an_error_result`</sub>
- The tool loop stops after a bounded number of rounds so a looping model cannot run forever. <sub>`the_tool_loop_stops_after_a_bounded_number_of_rounds`</sub>
- The API key travels only in the x-goog-api-key header, never in the URL, the body, or anything shown to the user. <sub>`the_api_key_travels_only_in_the_header`</sub>
- A rejected API key produces a message that says the key is the problem, not a raw HTTP code. <sub>`a_rejected_api_key_is_explained_as_a_key_problem`</sub>
- Hitting the Gemini usage limit is explained as a usage limit with a "try again later" hint. <sub>`a_rate_limited_service_is_explained_as_a_usage_limit`</sub>
- An unknown model name is explained as a model-name problem, naming the model that was tried. <sub>`an_unknown_model_name_is_explained_as_a_model_problem`</sub>
- A server that cannot be reached names the URL that was tried instead of failing silently. <sub>`an_unreachable_server_names_the_url_that_was_tried`</sub>
- Earlier turns of the conversation are replayed so the model remembers what was said before. <sub>`earlier_turns_of_the_conversation_are_replayed`</sub>
- The connection test reports success for a working setup and a readable, machine-tagged reason for a broken one. <sub>`the_connection_test_reports_success_or_a_readable_reason`</sub>
- The connection test uses the plain (non-streaming) endpoint of the configured model. <sub>`the_connection_test_uses_the_non_streaming_endpoint`</sub>
- The endpoint and the default model match Google's published Gemini API. <sub>`the_defaults_match_the_published_gemini_api`</sub>
- A model name pasted with the "models/" prefix still reaches the right endpoint exactly once. <sub>`a_model_name_with_the_models_prefix_still_works`</sub>
- With no model name saved the turn stops before any request, telling the user which box to fill in. <sub>`an_empty_model_name_stops_before_any_request`</sub>

### Standards knowledge injection

- The bundled standards note covers symbols, reference designators, wire colors, numbering and layout. <sub>`bundled_standards_cover_the_drawing_conventions`</sub>
- The system prompt carries the bundled standards and the verification-loop rule. <sub>`system_prompt_carries_the_standards_and_the_verification_loop`</sub>
- The system prompt tells the agent to start a blank drawing from a template instead of drawing everything by hand. <sub>`system_prompt_points_at_the_start_templates`</sub>
- The system prompt says the shell is unavailable and that entity ids are written by hand. <sub>`system_prompt_tells_the_agent_no_shell_is_available`</sub>
- The drawing context comes first in the system prompt, with the knowledge behind it. <sub>`system_prompt_puts_the_drawing_context_first`</sub>
- A knowledge file set in the settings is appended after the bundled note. <sub>`user_knowledge_file_is_appended_to_the_prompt`</sub>
- Without a knowledge file the prompt holds the bundled note only. <sub>`without_a_knowledge_file_only_the_bundled_note_is_used`</sub>
- The prompt lists the bundled manual (01-13) so how-to questions are answered from the documentation with a source. <sub>`system_prompt_lists_the_bundled_documentation`</sub>
- Questions about missing features are answered from the roadmap and the feature inventory. <sub>`system_prompt_points_unsupported_features_at_the_roadmap`</sub>
- Without bundled documentation the guide is dropped instead of pointing at files that are not there. <sub>`the_documentation_guide_is_dropped_when_the_docs_are_missing`</sub>
- A review request runs the deterministic verification first, then the five habit-based checkpoints. <sub>`system_prompt_carries_the_review_checklist`</sub>
- Review findings are listed as severity / target / finding / suggestion, and fixes wait for the user's approval. <sub>`review_findings_use_the_severity_table_and_wait_for_approval`</sub>
- A test-plan request produces a step / action / expected-result table and does not write notes into the drawing on its own. <sub>`system_prompt_carries_the_test_plan_table`</sub>
- Parts selection searches the parts database and answers with a comparison table. <sub>`system_prompt_carries_the_parts_comparison_table`</sub>
- An unreadable knowledge file is skipped without losing the bundled note. <sub>`an_unreadable_knowledge_file_is_skipped`</sub>
- The system prompt tells the agent to measure tidiness with get_tidy_metrics and to stop once it stops improving. <sub>`system_prompt_carries_the_tidy_loop`</sub>

### knowledge_docs

- The agent is pointed at the documentation folder the app resolved, so it can read the manual wherever it is installed. <sub>`the_documentation_folder_of_the_installed_app_is_handed_to_the_agent`</sub>

### Standards knowledge (resource file)

- The bundled standards note is read from the resource file when one is available. <sub>`the_standards_note_is_read_from_the_resource_file_when_present`</sub>

### Agent manager (turns)

- Sending without a conversation id creates a conversation and broadcasts its events. <sub>`send_creates_conversation_and_broadcasts_events`</sub>
- A turn that edits the document records the applied revisions and emits a turn-applied event with the undo depth. <sub>`turn_records_applied_revisions_and_emits_turn_applied`</sub>
- Edits made while a turn is running count as agent edits, and edits outside a turn stay user edits. <sub>`edits_during_a_turn_are_recorded_as_agent_edits`</sub>
- Rolling back a turn reverts only the agent's edits and keeps the user's manual edits, even those made during the turn. <sub>`undo_turn_reverts_only_the_agent_edits_of_the_turn`</sub>
- An older turn can still be rolled back later; edits made after it are kept. <sub>`undo_turn_can_revert_an_older_turn_and_keeps_later_edits`</sub>
- A rollback that conflicts with a manual edit is refused, the turn stays applied, and it can be retried later. <sub>`undo_turn_reports_a_conflict_and_keeps_the_turn_rollbackable`</sub>
- A turn id keeps pointing at the same turn even after later turns are appended. <sub>`a_turn_id_keeps_addressing_the_same_turn_after_more_turns`</sub>
- A turn that was already rolled back cannot be rolled back twice. <sub>`undo_turn_rejects_an_already_undone_turn`</sub>
- A running turn cannot be reverted. <sub>`undo_turn_rejects_a_running_turn`</sub>
- Document errors and unknown turn targets are reported as distinct errors. <sub>`undo_turn_reports_doc_errors_and_unknown_targets`</sub>
- Every broadcast event carries the sequence number of the turn that produced it, increasing with each send. <sub>`turn_events_carry_a_monotonic_turn_seq`</sub>
- Cancelling reports the cancelled turn's sequence number, and the next turn gets a higher one, so late events can be told apart. <sub>`cancel_reports_the_cancelled_turn_seq`</sub>
- Sending to a conversation that is already running a turn is rejected. <sub>`second_send_while_running_is_rejected`</sub>
- Cancel kills the CLI process and marks the message as cancelled. <sub>`cancel_stops_the_turn_and_marks_the_message`</sub>
- Sending to an unknown conversation id fails cleanly. <sub>`send_to_unknown_conversation_fails`</sub>
- The drawing context and selected model are forwarded to the CLI invocation. <sub>`context_and_model_are_forwarded_to_the_cli`</sub>
- Every turn injects the bundled standards knowledge and the verification-loop rule. <sub>`every_turn_injects_the_standards_knowledge`</sub>
- The knowledge file from the settings reaches the CLI too. <sub>`the_knowledge_file_setting_reaches_the_cli`</sub>
- Turning off auto-read-drawing suppresses the drawing context but keeps the standards knowledge. <sub>`auto_read_drawing_off_suppresses_the_drawing_context`</sub>
- The claude-path setting overrides which executable the backend runs. <sub>`claude_path_setting_becomes_the_backend_executable`</sub>
- Replacing the conversation history (project load) cancels any running turn first. <sub>`set_conversations_replaces_history_and_cancels_running_turn`</sub>
- Turns in two different conversations run at the same time instead of queuing behind each other. <sub>`turns_in_two_conversations_run_at_the_same_time`</sub>
- Edits made by two conversations at once all land in the document, in revision order and without loss. <sub>`parallel_turns_keep_every_edit_in_revision_order`</sub>
- Each conversation gets its own turn sequence numbers, so cancelling one never discards the other's events. <sub>`each_conversation_keeps_its_own_turn_seq`</sub>
- Undoing a turn that swallowed a parallel turn's edits marks that turn as reverted too, instead of leaving it looking applied. <sub>`undo_turn_marks_the_parallel_turn_whose_edits_it_swept`</sub>

### manager_api_provider

- With the Anthropic API selected, a chat turn runs even though no Claude Code CLI is installed. <sub>`a_turn_runs_on_the_api_without_any_claude_cli`</sub>
- Choosing the API without saving a key refuses the send with a message pointing at the settings screen. <sub>`sending_without_a_saved_key_points_at_the_settings_screen`</sub>
- The provider badge reports "ready" once a key is saved and "not ready" once it is removed. <sub>`the_provider_is_ready_only_while_a_key_is_saved`</sub>
- The connection test refuses before a key is saved, naming the missing key as the reason. <sub>`the_connection_test_refuses_before_a_key_is_saved`</sub>
- Nothing about the key is ever broadcast to the UI event stream. <sub>`the_api_key_never_appears_in_the_event_stream`</sub>

### manager_copilot_provider

- With GitHub Copilot selected, a chat turn runs through the Copilot CLI without any Anthropic key. <sub>`a_turn_runs_through_the_copilot_cli`</sub>
- The session id chosen for the first turn is remembered, so the next turn continues the same Copilot session. <sub>`the_session_is_remembered_for_the_next_turn`</sub>
- The provider badge reports "ready" only while the Copilot CLI can actually be found. <sub>`the_provider_is_ready_only_while_copilot_is_found`</sub>
- If the configured Copilot executable does not exist, the chat shows why instead of failing silently. <sub>`a_missing_copilot_executable_is_explained_in_the_chat`</sub>
- The connection test goes to Copilot (not to the Anthropic API) while Copilot is the chosen provider. <sub>`the_connection_test_follows_the_chosen_provider`</sub>

### manager_gemini_provider

- With Google Gemini selected, a chat turn runs even though no Claude CLI is installed. <sub>`a_turn_runs_on_google_gemini`</sub>
- Choosing Gemini without saving a key refuses the send with a message pointing at the settings screen. <sub>`sending_without_a_key_points_at_the_settings_screen`</sub>
- The Gemini key lives in its own keychain entry, so an Anthropic or OpenAI key does not make Gemini ready. <sub>`the_gemini_key_is_stored_separately_from_the_other_providers`</sub>
- Clearing the model box falls back to the recommended default model, so the provider stays usable. <sub>`a_blank_model_box_falls_back_to_the_default_model`</sub>
- While Gemini is chosen, the connection test goes to Gemini instead of to the Anthropic API. <sub>`the_connection_test_follows_the_chosen_provider`</sub>
- Nothing about the Gemini key is ever broadcast to the UI event stream or written into the chat history. <sub>`the_api_key_never_appears_in_the_event_stream`</sub>

### Provider: OpenAI-compatible / Ollama

- With an OpenAI-compatible endpoint selected, a chat turn runs even though no Claude CLI is installed. <sub>`a_turn_runs_on_an_openai_compatible_endpoint`</sub>
- A local Ollama URL needs no API key at all, so a turn runs with nothing saved in the keychain. <sub>`a_local_ollama_url_needs_no_api_key`</sub>
- Choosing a remote OpenAI-compatible endpoint without saving a key refuses the send with a message pointing at the settings screen. <sub>`sending_to_a_remote_endpoint_without_a_key_points_at_the_settings_screen`</sub>
- Leaving the model box empty refuses the send and says which box to fill in. <sub>`sending_without_a_model_name_says_which_box_to_fill_in`</sub>
- The provider badge reports "ready" once a key is saved (or the URL is local) and a model name is filled in. <sub>`the_provider_is_ready_with_a_key_or_a_local_url_and_a_model`</sub>
- While an OpenAI-compatible endpoint is chosen, the connection test goes there instead of to the Anthropic API. <sub>`the_connection_test_follows_the_chosen_provider`</sub>
- Nothing about the OpenAI key is ever broadcast to the UI event stream or written into the chat history. <sub>`the_api_key_never_appears_in_the_event_stream`</sub>

### OpenAI-compatible backend

- Streamed assistant text arrives as text deltas and the turn ends with the assembled reply and its token usage. <sub>`streamed_text_becomes_deltas_and_a_completed_turn`</sub>
- The request goes to /chat/completions under the configured base URL and carries the model, the streaming flag, the system prompt and the user's message. <sub>`the_request_goes_to_chat_completions_with_the_model_system_prompt_and_message`</sub>
- The bridged MCP tools are offered in the OpenAI function format so the agent can edit the drawing. <sub>`the_bridged_mcp_tools_are_offered_as_openai_functions`</sub>
- A tool the model asks for is executed locally and its result is sent back as a tool message so the model can continue. <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
- A tool that fails is reported back to the model as a clearly marked error instead of aborting the turn. <sub>`a_failing_tool_is_reported_to_the_model_as_an_error_result`</sub>
- The tool loop stops after a bounded number of rounds so a looping model cannot run forever. <sub>`the_tool_loop_stops_after_a_bounded_number_of_rounds`</sub>
- A saved key is sent as a bearer token, and never appears in any message shown to the user. <sub>`a_saved_key_is_sent_as_a_bearer_token`</sub>
- A local server such as Ollama is called without any Authorization header when no key is saved. <sub>`a_local_server_is_called_without_an_authorization_header`</sub>
- A rejected API key produces a message that says the key is the problem, not a raw HTTP code. <sub>`a_rejected_api_key_is_explained_as_a_key_problem`</sub>
- Hitting the provider's rate limit is explained as a usage limit with a "try again later" hint. <sub>`a_rate_limited_service_is_explained_as_a_usage_limit`</sub>
- An unknown model name is explained as a model-name problem, naming the model that was tried. <sub>`an_unknown_model_name_is_explained_as_a_model_problem`</sub>
- A server that cannot be reached names the URL that was tried and points at starting Ollama for a local URL. <sub>`an_unreachable_server_names_the_url_and_suggests_starting_ollama`</sub>
- Earlier turns of the conversation are replayed so the model remembers what was said before. <sub>`earlier_turns_of_the_conversation_are_replayed`</sub>
- The connection test reports success for a working setup and a readable, machine-tagged reason for a broken one. <sub>`the_connection_test_reports_success_or_a_readable_reason`</sub>
- The Ollama preset points at the local Ollama server's OpenAI-compatible endpoint, while the default is OpenAI itself. <sub>`the_ollama_preset_points_at_the_local_ollama_server`</sub>
- A base URL pasted with a trailing slash still reaches /chat/completions exactly once. <sub>`a_base_url_with_a_trailing_slash_still_works`</sub>

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

### secrets

- A saved API key can be read back, and deleting it makes it gone. <sub>`a_saved_api_key_can_be_read_back_and_deleted`</sub>
- Deleting a key that was never stored is not an error, so the UI can always offer "remove". <sub>`deleting_a_key_that_was_never_stored_is_not_an_error`</sub>
- Blank input is refused instead of storing an empty key that would fail later with a confusing error. <sub>`a_blank_api_key_is_refused`</sub>
- Surrounding whitespace is trimmed, so a key pasted with a stray newline still works. <sub>`a_pasted_key_is_trimmed_before_it_is_stored`</sub>
- The stored key never appears in the settings file, which stays free of any secret. <sub>`the_settings_file_never_contains_the_api_key`</sub>
- A settings file that somehow carries an api key field loses it on the next save. <sub>`an_api_key_smuggled_into_the_settings_file_is_dropped_on_save`</sub>
- The keychain entry is addressed by the app name and a fixed account, so the same key is found next launch. <sub>`the_keychain_entry_is_addressed_by_the_app_name`</sub>
- Against the real OS keychain, a key round-trips and is removed again (opt-in; skipped by default). <sub>`the_real_os_keychain_round_trips_a_key`</sub>
- The keychain is read once per app run, so the OS does not ask for permission again and again. <sub>`the_keychain_is_read_only_once_per_app_run`</sub>
- When the OS keychain cannot be read, the reason is reported instead of a silent "no key". <sub>`a_keychain_that_cannot_be_read_reports_the_reason`</sub>
- The OpenAI-compatible key lives under its own keychain entry, so saving one provider's key never disturbs the other's. <sub>`the_openai_key_is_stored_beside_the_anthropic_one_without_disturbing_it`</sub>
- A pasted OpenAI key is trimmed, and a blank one is refused instead of being stored. <sub>`a_pasted_openai_key_is_trimmed_and_a_blank_one_is_refused`</sub>
- The OpenAI key never appears in the settings file, which keeps only the URL and the model name. <sub>`the_settings_file_never_contains_the_openai_key`</sub>

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
- By default no extra knowledge file is configured (only the bundled standards note is used). <sub>`default_knowledge_path_is_unset`</sub>
- A settings file saved before the knowledge-file field existed loads with it unset. <sub>`old_settings_file_without_knowledge_path_loads_unset`</sub>
- A blank knowledge-file path is normalized away, and padding is trimmed. <sub>`normalized_drops_a_blank_knowledge_path`</sub>
- Out of the box the agent runs through the Claude Code CLI, with Claude Sonnet 5 ready for the API route. <sub>`the_default_provider_is_the_claude_code_cli`</sub>
- Choosing the Anthropic API survives a save/load round trip together with the model name. <sub>`the_anthropic_api_choice_round_trips_through_save_and_load`</sub>
- The provider is stored under readable names, so the settings file stays hand-editable. <sub>`the_provider_is_stored_under_a_readable_name`</sub>
- A settings file written before providers existed keeps working and stays on the CLI. <sub>`an_old_settings_file_without_a_provider_stays_on_the_cli`</sub>
- A provider name this build does not know falls back to the CLI instead of breaking the whole file. <sub>`an_unknown_provider_name_falls_back_to_the_cli`</sub>
- A blank model box falls back to the default model, and padding is trimmed. <sub>`a_blank_api_model_falls_back_to_the_default`</sub>
- Out of the box GitHub Copilot is set to let Copilot pick the model, with the executable found on PATH. <sub>`the_copilot_defaults_let_copilot_pick_the_model`</sub>
- Choosing GitHub Copilot survives a save/load round trip together with its path and model. <sub>`the_copilot_choice_round_trips_through_save_and_load`</sub>
- A settings file written before GitHub Copilot existed keeps working, with the Copilot fields at their defaults. <sub>`an_old_settings_file_without_copilot_fields_keeps_working`</sub>
- A blank Copilot model box returns to "auto", and a blank path returns to auto-detection. <sub>`blank_copilot_boxes_return_to_the_defaults`</sub>
- No GitHub credential is ever written to the settings file: Copilot uses its own sign-in. <sub>`no_github_credential_is_written_to_the_settings_file`</sub>
- Out of the box the OpenAI-compatible route points at OpenAI itself and leaves the model name empty on purpose. <sub>`the_openai_defaults_point_at_openai_and_ask_for_a_model`</sub>
- Choosing the OpenAI-compatible route survives a save/load round trip together with the URL and the model. <sub>`the_openai_choice_round_trips_through_save_and_load`</sub>
- A settings file written before the OpenAI-compatible route existed keeps working, with its fields at their defaults. <sub>`an_old_settings_file_without_openai_fields_keeps_working`</sub>
- A blank URL box returns to OpenAI, a trailing slash is trimmed, and the model name keeps whatever was typed (minus spaces). <sub>`blank_openai_boxes_return_to_the_defaults`</sub>
- No OpenAI key is ever written to the settings file: only the URL and the model name live there. <sub>`no_openai_key_is_written_to_the_settings_file`</sub>
- Out of the box the Gemini route is preloaded with the recommended fast model. <sub>`the_gemini_default_model_is_the_recommended_fast_one`</sub>
- Choosing Gemini survives a save/load round trip together with the model name. <sub>`the_gemini_choice_round_trips_through_save_and_load`</sub>
- A settings file written before the Gemini route existed keeps working, with its fields at their defaults. <sub>`an_old_settings_file_without_gemini_fields_keeps_working`</sub>
- A blank Gemini model box returns to the default model, and stray spaces are trimmed away. <sub>`a_blank_gemini_model_box_returns_to_the_default`</sub>
- No Gemini key is ever written to the settings file: only the model name lives there. <sub>`no_gemini_key_is_written_to_the_settings_file`</sub>

### tool_bridge

- Each MCP tool becomes one API tool keeping its name, description and input schema. <sub>`every_mcp_tool_becomes_one_api_tool`</sub>
- The JSON Schema `$schema` marker is dropped because the API only wants the shape itself. <sub>`the_schema_marker_is_dropped_from_the_input_schema`</sub>
- A tool without a usable schema still gets an empty object schema, so the API accepts it. <sub>`a_tool_without_a_schema_gets_an_empty_object_schema`</sub>
- A schema that forgot `"type": "object"` is repaired instead of being sent as-is. <sub>`a_schema_missing_its_object_type_is_repaired`</sub>
- Every bridged tool name fits the API's allowed name pattern, so no tool is rejected. <sub>`bridged_tool_names_fit_the_api_name_rules`</sub>
- A tool with an empty description keeps an explanatory placeholder rather than nothing. <sub>`a_tool_without_a_description_still_carries_some_text`</sub>


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
- madake export dxf / kicad write one sheet through the DXF and KiCad endpoints (sheet selectable). <sub>`export_dxf_and_kicad_forward_to_their_endpoints`</sub>
- madake open with a .dxf file imports it through /import/dxf, passing --wire-layer, and prints the import summary; --wire-layer is rejected for other files. <sub>`open_dxf_imports_with_wire_layers`</sub>
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
- a region is painted in the color of the conversation that made the edit <sub>`並列エージェント: 会話ごとの色`</sub>
- a region without a conversation color falls back to the default agent color <sub>`並列エージェント: 会話ごとの色`</sub>
- regions of two conversations running at once keep their own colors side by side <sub>`並列エージェント: 会話ごとの色`</sub>
- conversation colors follow the start order and wrap around after the fourth <sub>`並列エージェント: 会話ごとの色`</sub>

### deviceTree

- an expanded device is followed by one row per function <sub>`デバイスツリーの整形`</sub>
- a collapsed device shows only its heading and leaves other devices alone <sub>`デバイスツリーの整形`</sub>
- the order of devices and of their functions is kept as the core returned it <sub>`デバイスツリーの整形`</sub>
- an empty project produces no rows <sub>`デバイスツリーの整形`</sub>
- a relay is named by its kind and part number <sub>`見出しの文言`</sub>
- a terminal block is named with its pole count <sub>`見出しの文言`</sub>
- other parts are named after their symbol, in the UI language <sub>`見出しの文言`</sub>
- a function row reads like 'contact 13-14' with a '/2.B3' badge <sub>`見出しの文言`</sub>
- clicking a function row switches to its sheet and selects its entity <sub>`ツリーからの操作`</sub>
- deleting a device removes every entity of that reference designator, without duplicates <sub>`ツリーからの操作`</sub>

### dynamicSymbol

- connector_2p has the same pin coordinates as the legacy static definition <sub>`dynamicSymbol`</sub>
- terminal_block_3p has 3 terminals with left/right points, centered on the 2.5 mm grid <sub>`dynamicSymbol`</sub>
- a PLC input module has one connection point per I/O point on its left side <sub>`dynamicSymbol`</sub>
- PLC modules come in an input and an output flavour <sub>`dynamicSymbol`</sub>
- malformed dynamic ids return null <sub>`dynamicSymbol`</sub>
- PLC module symbols exist from 1 to 64 points only <sub>`dynamicSymbol`</sub>
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

### macroPreview

- lists the default variant A first, followed by the macro's own variants <sub>`macroVariantKeys`</sub>
- gives a macro without variants the single key A <sub>`macroVariantKeys`</sub>
- labels a variant with its key and name, in Japanese when the UI is Japanese <sub>`macroVariantLabel`</sub>
- shows just the key for a variant that has no name, such as the default A <sub>`macroVariantLabel`</sub>
- labels a value set in the UI language, falling back to the English name <sub>`macroValueSetLabel`</sub>
- falls back to the id for a value set with no name at all <sub>`macroValueSetLabel`</sub>
- reads the default commands for A (or no key) and the variant's own commands otherwise <sub>`macroCommands / macroEntities`</sub>
- falls back to the default commands when the variant key is unknown <sub>`macroCommands / macroEntities`</sub>
- extracts only the entities the macro adds <sub>`macroCommands / macroEntities`</sub>
- just shifts a point by the insertion position when the rotation is zero <sub>`placePoint`</sub>
- maps a point to (-y, x) for a quarter turn, in the paper coordinate system <sub>`placePoint`</sub>
- moves each symbol to the cursor and adds the placement rotation to its own <sub>`placeMacroEntities`</sub>
- moves wire points through the same transform, so a rotated macro keeps its shape <sub>`placeMacroEntities`</sub>
- works on copies, leaving the macro definition untouched so it can be placed again <sub>`placeMacroEntities`</sub>
- builds a throwaway sheet holding only the chosen variant's entities <sub>`macroSheet`</sub>
- shows the Japanese name when the UI is Japanese and the English name otherwise <sub>`macroName`</sub>
- falls back to the English name when a macro has no Japanese name <sub>`macroName`</sub>

### relayXref

- groups the coil and contacts that share a reference into one device <sub>`relayXref`</sub>
- ignores symbols that are neither relay coils nor relay contacts <sub>`relayXref`</sub>
- orders the contacts of a device by sheet and then by zone <sub>`relayXref`</sub>
- builds terminal pairs from the contact position and its function digits <sub>`relayXref`</sub>
- lists every placed contact with its terminal pair and address <sub>`relayXref`</sub>
- adds a dash row for every unused contact of the assigned part <sub>`relayXref`</sub>
- lists only the drawn contacts when no contact configuration is known <sub>`relayXref`</sub>
- keys the per-sheet contact maps by the coil on that sheet <sub>`relayXref`</sub>
- shows the address of the driving coil beside each contact <sub>`relayXref`</sub>
- shows no coil location for a contact whose coil is missing <sub>`relayXref`</sub>
- parses make and break counts and ignores unreadable configurations <sub>`relayXref`</sub>
- centres the contact map table under the coil <sub>`relayXref`</sub>
- anchors both annotations to the symbol outline <sub>`relayXref`</sub>

### renderer

- places the revision table directly above the title block with the same width and row height <sub>`revisionLayout`</sub>
- stacks revisions oldest at the bottom and newest on top, with the column header row at the very bottom <sub>`revisionLayout`</sub>
- lays out nothing for a sheet without revisions <sub>`revisionLayout`</sub>
- keeps only the newest six revisions in the drawing and drops older rows <sub>`revisionLayout`</sub>
- splits the title block width into the mark, date, description and approver columns <sub>`revisionLayout`</sub>
- returns at most the newest six revisions, still in oldest-first order <sub>`visibleRevisions / effectiveRev`</sub>
- shows the newest revision mark in the title block Rev cell, falling back to the stored value or a dash <sub>`visibleRevisions / effectiveRev`</sub>
- turns arcs with the symbol so the contactor half circle keeps facing its moving contact <sub>`rotateArcAngles`</sub>
- mirrors arcs left to right while keeping the clockwise drawing direction <sub>`rotateArcAngles`</sub>

### search

- the All chip does not narrow the targets <sub>`検索バーのフィルタ`</sub>
- the Net chip searches both net names and wire numbers <sub>`検索バーのフィルタ`</sub>
- the reference, part-number and text chips each narrow to one target <sub>`検索バーのフィルタ`</sub>
- the chips are ordered All, reference, part number, net, text <sub>`検索バーのフィルタ`</sub>
- Enter steps to the next result and wraps around at the end <sub>`Enter巡回`</sub>
- Shift+Enter steps back and wraps around at the start <sub>`Enter巡回`</sub>
- with nothing selected yet, Enter picks the first hit and Shift+Enter the last <sub>`Enter巡回`</sub>
- with no results there is nothing to select <sub>`Enter巡回`</sub>
- clicking a row jumps to that hit's sheet and selects its entity <sub>`結果行`</sub>
- the location column joins the sheet name and the zone <sub>`結果行`</sub>
- a symbol hit is described by the function it plays in its device <sub>`結果行`</sub>
- hits without a device function are described by the search target itself <sub>`結果行`</sub>
- in the tree and the surfer, functions drop the device kind from their label <sub>`結果行`</sub>
- Cmd+F opens the floating search bar <sub>`検索バー`</sub>
- Escape closes the bar and the results panel but keeps the query for next time <sub>`検索バー`</sub>
- running a search fills in the count and opens the results panel <sub>`検索バー`</sub>
- an empty query searches nothing and closes the panel <sub>`検索バー`</sub>
- the All chip searches without narrowing the targets <sub>`フィルタチップ`</sub>
- switching chips searches again with that target <sub>`フィルタチップ`</sub>
- surrounding whitespace is trimmed from the query <sub>`フィルタチップ`</sub>
- Enter walks to the next hit and wraps around at the end <sub>`Enter巡回と行クリック`</sub>
- Shift+Enter walks back to the previous hit <sub>`Enter巡回と行クリック`</sub>
- with no hits there is nothing to walk to <sub>`Enter巡回と行クリック`</sub>
- clicking a result row makes that row the current one <sub>`Enter巡回と行クリック`</sub>
- searching again resets the walk position <sub>`Enter巡回と行クリック`</sub>
- the panel's close button closes only the results panel <sub>`Enter巡回と行クリック`</sub>

### surfer

- Alt-clicking a symbol lists every function of that device with its location <sub>`参照サーフィン (Surfer)`</sub>
- Alt-clicking a net label lists every place that name appears, including its own sheet <sub>`参照サーフィン (Surfer)`</sub>
- Alt-clicking a numbered wire lists every wire carrying that number <sub>`参照サーフィン (Surfer)`</sub>
- locations come back in sheet, zone and id order, the same way every time <sub>`参照サーフィン (Surfer)`</sub>
- an element with no designator, name or number has nothing to surf <sub>`参照サーフィン (Surfer)`</sub>
- a reference designator with no device behind it has nothing to surf <sub>`参照サーフィン (Surfer)`</sub>
- choosing a row switches to that sheet and reveals the entity <sub>`参照サーフィン (Surfer)`</sub>
- the up/down cycle follows the same wrap-around rule as the search bar <sub>`参照サーフィン (Surfer)`</sub>
- Alt-clicking a symbol with a designator opens the list of where that device appears <sub>`Surferポップアップ`</sub>
- the walk starts at the element that was clicked <sub>`Surferポップアップ`</sub>
- an element with nothing to surf does not open the popup <sub>`Surferポップアップ`</sub>
- the arrow keys walk through the locations and wrap around <sub>`Surferポップアップ`</sub>
- clicking a row picks that location and out-of-range rows are ignored <sub>`Surferポップアップ`</sub>
- closing with Escape also resets the walk position <sub>`Surferポップアップ`</sub>

### symbolLibrary

- matches the English name, the Japanese name or the symbol id <sub>`symbolMatchesQuery`</sub>
- finds symbols by the shop-floor words kept in the search keywords <sub>`symbolMatchesQuery`</sub>
- treats an empty query as matching everything <sub>`symbolMatchesQuery`</sub>
- groups symbols by category keeping the library order <sub>`groupSymbolsByCategory`</sub>
- keeps only matching symbols and drops the categories left empty <sub>`groupSymbolsByCategory`</sub>

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
- sends nothing when colour, gauge, part number and length are unchanged <sub>`wireUpdateCommand`</sub>
- a hand-edited length becomes an update whose length source is manual <sub>`wireUpdateCommand`</sub>
- overwriting a FreeCAD-measured length by hand is flagged so the panel can warn <sub>`wireUpdateCommand`</sub>
- changing only colour or gauge keeps the FreeCAD length source <sub>`wireUpdateCommand`</sub>

### drawingContext

- with no selection the context covers the whole sheet <sub>`drawingContextTag`</sub>
- with no sheet the string stays well-formed <sub>`drawingContextTag`</sub>
- a selection lists reference designators (falling back to entity kinds) <sub>`drawingContextTag`</sub>
- selections beyond 10 items collapse into +N more <sub>`drawingContextTag`</sub>
- selection ids absent from the active sheet are ignored <sub>`drawingContextTag`</sub>
- an empty draft receives the context as-is <sub>`appendContextTag`</sub>
- an existing draft is separated by a newline (without doubling) <sub>`appendContextTag`</sub>

### File menu (new / open / save / save as)

- save overwrites the current file without showing a dialog <sub>`file menu: 保存`</sub>
- save asks for a path when the project has none yet <sub>`file menu: 保存`</sub>
- cancelling the save dialog writes nothing <sub>`file menu: 保存`</sub>
- save-as defaults to the currently open file's path <sub>`file menu: 保存`</sub>
- a failed save is logged and the save target stays unchanged <sub>`file menu: 保存`</sub>
- new creates an untitled project with no save target <sub>`file menu: 新規と開く`</sub>
- new asks before discarding unsaved changes, and cancel keeps the drawing <sub>`file menu: 新規と開く`</sub>
- a failed new-project is logged and the drawing stays <sub>`file menu: 新規と開く`</sub>
- new and open skip the confirmation when nothing is unsaved <sub>`file menu: 新規と開く`</sub>
- open loads the chosen .mdkproj and makes it the save target <sub>`file menu: 新規と開く`</sub>
- cancelling the open dialog loads nothing <sub>`file menu: 新規と開く`</sub>
- open asks before discarding unsaved changes; cancel shows no file dialog <sub>`file menu: 新規と開く`</sub>
- choosing a .kicad_sch imports it as a KiCad schematic and logs the counts <sub>`file menu: 新規と開く`</sub>
- choosing a .dxf imports it as a DXF drawing and logs the counts and warnings <sub>`file menu: 新規と開く`</sub>
- the import buttons open the dialog with that format's filter only and confirm unsaved changes first <sub>`file menu: 新規と開く`</sub>
- a recent file opens without a dialog <sub>`file menu: 新規と開く`</sub>
- a recent file that fails to open is dropped from the list with the reason logged <sub>`file menu: 新規と開く`</sub>
- exporting DXF asks for a path defaulting to <sheet>.dxf and writes the active sheet <sub>`file menu: シートの書き出し`</sub>
- exporting KiCad defaults to <sheet>.kicad_sch, and SVG/PDF keep their extension filters <sub>`file menu: シートの書き出し`</sub>
- cancelling the save dialog exports nothing <sub>`file menu: シートの書き出し`</sub>
- without an open sheet the export logs a hint and shows no dialog <sub>`file menu: シートの書き出し`</sub>
- a failed export is logged with its reason <sub>`file menu: シートの書き出し`</sub>

### Tidy metrics (crossings / overlaps / grid)

- offers three tidy modes: layout, wiring and labels <sub>`tidyPrompt`</sub>
- the layout prompt asks for the 2.5mm grid, no overlapping symbols and aligned rows <sub>`tidyPrompt`</sub>
- the wiring prompt asks to cut crossings, keep right angles and drop needless bends <sub>`tidyPrompt`</sub>
- the label prompt only moves labels and forbids deleting or rewriting them <sub>`tidyPrompt`</sub>
- no tidy mode is allowed to change the connections of the circuit <sub>`tidyPrompt`</sub>
- every tidy prompt asks to measure with get_tidy_metrics before and after editing <sub>`tidyPrompt`</sub>
- every tidy prompt stops when the numbers stop improving, and after three rounds at most <sub>`tidyPrompt`</sub>
- every tidy prompt requires before/after numbers in the final answer <sub>`tidyPrompt`</sub>
- with no selection the tidy covers the whole sheet <sub>`tidyPrompt`</sub>
- with a selection the tidy lists the selected entity ids and forbids touching anything else <sub>`tidyPrompt`</sub>
- ids that are not on the active sheet fall back to tidying the whole sheet <sub>`tidyPrompt`</sub>
- stays well-formed when there is no sheet at all <sub>`tidyPrompt`</sub>
- runs one tidy as exactly one chat turn <sub>`runTidy`</sub>
- opens the agent tab so the user can watch the tidy run <sub>`runTidy`</sub>
- sends the selected entity ids when a selection is active <sub>`runTidy`</sub>
- does not start a tidy while the open conversation is still answering <sub>`runTidy`</sub>

### i18n

- defaults to English and falls back to English <sub>`i18n`</sub>
- ships English, Japanese, Chinese, Spanish, French and German catalogs <sub>`i18n`</sub>
- keeps every catalog key-identical to the English one <sub>`i18n`</sub>
- rejects empty strings in any catalog <sub>`i18n`</sub>
- keeps the same named placeholders as English in every translation <sub>`i18n`</sub>
- keeps the same number of plural forms as English in every translation <sub>`i18n`</sub>
- shows each language name as its own endonym <sub>`i18n`</sub>
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
- turn_applied carries the stable turn id used to address the undo <sub>`chat store: applyAgentEvent`</sub>
- an undo depth on turn_applied records the exact edit count <sub>`chat store: applyAgentEvent`</sub>
- folding events advances the conversation's updated_at (newest-first history) <sub>`chat store: applyAgentEvent`</sub>
- a delta after completion starts a new turn <sub>`chat store: applyAgentEvent`</sub>
- multiple conversations fold independently; streaming reflects only the open conversation <sub>`chat store: applyAgentEvent`</sub>
- an unknown conversation id refetches the list instead of creating a ghost <sub>`chat store: applyAgentEvent`</sub>
- while our own turn is running, unknown-conversation events do not trigger a refetch <sub>`chat store: applyAgentEvent`</sub>
- turn_completed/error for a conversation with no running turn is dropped <sub>`chat store: applyAgentEvent`</sub>
- a failed first send shows the reason in the chat right away <sub>`chat store: アクション`</sub>
- send creates a conversation and adopts the server-assigned id <sub>`chat store: アクション`</sub>
- events arriving before send resolves still land in the same conversation <sub>`chat store: アクション`</sub>
- empty prompts and sends during streaming are ignored <sub>`chat store: アクション`</sub>
- a failed send marks the message with the error and stops streaming <sub>`chat store: アクション`</sub>
- after a failed first send, retrying sends as new without leaking the local id <sub>`chat store: アクション`</sub>
- cancel stops only the target conversation, leaving others streaming <sub>`chat store: アクション`</sub>
- cancel on a not-yet-assigned (local-) conversation skips the API and cleans up locally <sub>`chat store: アクション`</sub>
- a cancel issued before id assignment is sent to the server once the id arrives <sub>`chat store: アクション`</sub>
- events that arrive late from a cancelled turn are discarded <sub>`chat store: アクション`</sub>
- events of a new turn started after a cancel (higher sequence number) still apply <sub>`chat store: アクション`</sub>
- events without a sequence number (older server) are kept even after a cancel <sub>`chat store: アクション`</sub>
- streaming stops even if the cancel API fails <sub>`chat store: アクション`</sub>
- cancel with no conversation does nothing and never crashes <sub>`chat store: アクション`</sub>
- undoTurn calls the API with the stable turn id and withdraws the applied badge <sub>`chat store: アクション`</sub>
- undoTurn never calls the API for a turn id the conversation does not have <sub>`chat store: アクション`</sub>
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
- gives each conversation a stable overlay color in the order the conversations started <sub>`chat store: 並列エージェント`</sub>
- allows sending in the open conversation while another conversation is still answering <sub>`chat store: 並列エージェント`</sub>
- does not send again while the open conversation is still answering <sub>`chat store: 並列エージェント`</sub>
- keeps conversations that are still answering marked as running across a history reload <sub>`chat store: 並列エージェント`</sub>
- drops a conversation from the running list once its turn ends <sub>`chat store: 並列エージェント`</sub>

### devices

- opening the tab loads the tree and knows how many devices there are <sub>`デバイスツリーの読み込み`</sub>
- the tree is not reloaded while the drawing has not changed <sub>`デバイスツリーの読み込み`</sub>
- the tree is reloaded once the drawing has changed <sub>`デバイスツリーの読み込み`</sub>
- collapsing a device hides its functions and expanding brings them back <sub>`折りたたみと選択`</sub>
- a picked row stays selected <sub>`折りたたみと選択`</sub>
- deleting a device goes through the command engine, one command per sheet <sub>`デバイスの削除`</sub>
- a terminal block whose one symbol carries several functions is deleted only once <sub>`デバイスの削除`</sub>
- deleting the selected device clears the selection and reloads the tree <sub>`デバイスの削除`</sub>

### document

- entity_upserted / entity_removed patches update the mirrored sheet <sub>`document store`</sub>
- project_replaced swaps the whole mirrored project <sub>`document store`</sub>
- sheet add/remove patches keep sheet order <sub>`document store`</sub>
- sheet-meta patches never touch the entities <sub>`document store`</sub>
- patches with an older revision are discarded (duplicate delivery is safe) <sub>`document store`</sub>
- a mech_links_replaced patch replaces the FreeCAD link list <sub>`document store`</sub>

### Circuit macros

- loads the macros, the unreadable files and the user folder path <sub>`macro library listing`</sub>
- builds a category tree led by an all-macros entry, with uncategorized macros in their own group <sub>`macro library listing`</sub>
- badges every tile with its variant count, counting the default A <sub>`macro library listing`</sub>
- shows only the tiles of the selected category <sub>`macro library listing`</sub>
- filters tiles by a case-insensitive substring of the name or the id <sub>`macro library listing`</sub>
- selects a tile and resets the variant back to the default A <sub>`macro library listing`</sub>
- keeps the dialog usable and records the reason when the list cannot be read <sub>`macro library listing`</sub>
- opens the save dialog for a selection and builds a preview of it <sub>`macro save dialog`</sub>
- refuses to open the save dialog when nothing is selected <sub>`macro save dialog`</sub>
- keeps saving disabled until a name has been entered <sub>`macro save dialog`</sub>
- saves the selection with the entered name and category, then reloads the library <sub>`macro save dialog`</sub>
- leaves the dialog open with the reason when saving fails <sub>`macro save dialog`</sub>
- shows the base point derived from the selection, which the user does not edit <sub>`macro save dialog`</sub>
- lists the value field and every attribute of the selected symbols as a placeholder candidate <sub>`placeholders and value sets in the save dialog`</sub>
- groups every field that was given the same key name into one placeholder with several targets <sub>`placeholders and value sets in the save dialog`</sub>
- builds one value set per row from its name and the value of each key <sub>`placeholders and value sets in the save dialog`</sub>
- drops a value set row that has no name yet <sub>`placeholders and value sets in the save dialog`</sub>
- sends the placeholders and the value sets along with the save <sub>`placeholders and value sets in the save dialog`</sub>
- saves exactly as before when no placeholder and no value set was entered <sub>`placeholders and value sets in the save dialog`</sub>
- starts every save dialog with no placeholder keys and no value set rows <sub>`placeholders and value sets in the save dialog`</sub>
- offers no value set for a macro that defines none <sub>`choosing a value set when inserting`</sub>
- lists the value sets of the selected macro and remembers the chosen one <sub>`choosing a value set when inserting`</sub>
- clears the chosen value set when another macro is selected <sub>`choosing a value set when inserting`</sub>
- copies the selection into an in-memory macro without writing any file <sub>`unnamed clipboard macro (Cmd+C / Cmd+V)`</sub>
- does nothing on copy when nothing is selected, keeping the previous clipboard <sub>`unnamed clipboard macro (Cmd+C / Cmd+V)`</sub>
- hands the copied macro back on every paste, so it can be pasted repeatedly <sub>`unnamed clipboard macro (Cmd+C / Cmd+V)`</sub>
- pastes nothing when nothing has been copied yet <sub>`unnamed clipboard macro (Cmd+C / Cmd+V)`</sub>

### Parts database

- search stores the results from the parts API <sub>`parts store`</sub>
- a failed search clears the results and resets loading <sub>`parts store`</sub>

### plcIo

- counts Mitsubishi addresses in octal so X7 is followed by X10 <sub>`PLC address numbering`</sub>
- writes Siemens addresses as a byte and a bit <sub>`PLC address numbering`</sub>
- writes Allen-Bradley addresses as a word and a bit <sub>`PLC address numbering`</sub>
- starts the automatic numbering from the point the start address names <sub>`PLC address numbering`</sub>
- refuses a start address that does not match the address style of the module <sub>`PLC address numbering`</sub>
- loads the PLC modules placed in the drawing and shows the first one <sub>`PLC I/O assignment editor`</sub>
- shows one grid row per I/O point with the address it would get from the numbering <sub>`PLC I/O assignment editor`</sub>
- shows the address, the signal name and the comment of the rows already saved <sub>`PLC I/O assignment editor`</sub>
- takes the target and the wire number of each point from the drawing <sub>`PLC I/O assignment editor`</sub>
- fills every address from the start address without touching signal names <sub>`PLC I/O assignment editor`</sub>
- renumbers from the start address the user typed <sub>`PLC I/O assignment editor`</sub>
- saves the whole table with a single set_plc_assignments command <sub>`PLC I/O assignment editor`</sub>
- keeps the rows of the other modules untouched when saving <sub>`PLC I/O assignment editor`</sub>
- keeps the id of the rows that already exist in the table <sub>`PLC I/O assignment editor`</sub>
- sends no command when nothing was edited <sub>`PLC I/O assignment editor`</sub>
- drops the empty rows at the end while keeping a blank row in the middle <sub>`PLC I/O assignment editor`</sub>
- imports a CSV file into the table of the selected module <sub>`PLC I/O assignment editor`</sub>
- reports the error of a malformed CSV and leaves the table alone <sub>`PLC I/O assignment editor`</sub>
- refreshes the wiring columns when the drawing changes from outside the editor <sub>`PLC I/O assignment editor`</sub>
- keeps the unsaved signal names while refreshing after an outside change <sub>`PLC I/O assignment editor`</sub>
- reloads the table itself after an outside change when nothing is being edited <sub>`PLC I/O assignment editor`</sub>
- does not fetch anything while the editor is closed <sub>`PLC I/O assignment editor`</sub>
- shows an empty table when no PLC module is placed <sub>`PLC I/O assignment editor`</sub>
- throws away the draft when the editor is closed <sub>`PLC I/O assignment editor`</sub>
- defaults to a vertical bus ladder with a 10mm rung spacing <sub>`PLC I/O sheet generation settings`</sub>
- generates the I/O sheet from the settings and the module spec of the parts library <sub>`PLC I/O sheet generation settings`</sub>
- refuses to generate with a ladder style that is not implemented yet <sub>`PLC I/O sheet generation settings`</sub>
- refuses to generate with a module placement that is not implemented yet <sub>`PLC I/O sheet generation settings`</sub>
- saves the pending edits before generating the sheet <sub>`PLC I/O sheet generation settings`</sub>
- falls back to the default address style when the module is not in the parts library <sub>`PLC I/O sheet generation settings`</sub>

### Project file (open / save / recent files)

- right after startup the loaded drawing counts as saved, with nothing unsaved <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- editing the drawing marks it as having unsaved changes <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- opening a file makes its path the save target and clears unsaved changes <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- opening a file empties the undo/redo history <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- save writes to the current file when one is known <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- save does nothing and returns null when no file is known <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- save-as writes to the new path, which becomes the save target <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- an edit that arrives while saving still counts as unsaved afterwards <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- a new project starts with no file and no unsaved changes <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- importing a KiCad schematic leaves no file and counts as unsaved <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- importing a DXF drawing leaves no file and counts as unsaved <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- recent files are read from storage at startup <sub>`projectFile store: 最近使ったファイル`</sub>
- opened and saved files go to the front of the list and are persisted <sub>`projectFile store: 最近使ったファイル`</sub>
- reopening a file moves it to the front instead of duplicating it <sub>`projectFile store: 最近使ったファイル`</sub>
- the list is capped, dropping the oldest entries <sub>`projectFile store: 最近使ったファイル`</sub>
- removing a file drops it from the list and from storage <sub>`projectFile store: 最近使ったファイル`</sub>
- fileNameOf returns the last path segment for both separators <sub>`projectFile helpers`</sub>
- isDxfPath recognises the .dxf extension regardless of case <sub>`projectFile helpers`</sub>
- isKicadPath recognises the .kicad_sch extension regardless of case <sub>`projectFile helpers`</sub>

### provider

- defaults to the Claude Code CLI with Claude Sonnet 5 ready for the API route <sub>`AI provider settings`</sub>
- choosing a provider saves it <sub>`AI provider settings`</sub>
- loading the provider status tells whether a key is saved <sub>`AI provider settings`</sub>
- the api key itself is never kept in the frontend state <sub>`AI provider settings`</sub>
- a blank key box is refused without calling the backend <sub>`AI provider settings`</sub>
- removing the key clears the saved badge <sub>`AI provider settings`</sub>
- a successful connection test shows the model it reached <sub>`AI provider settings`</sub>
- a failed connection test shows the reason <sub>`AI provider settings`</sub>
- the previous test result is cleared while a new test runs <sub>`AI provider settings`</sub>
- the connection badge follows the CLI for the CLI route and the saved key for the API route <sub>`AI provider settings`</sub>
- a saved key is shown as dots, never as its value <sub>`AI provider settings`</sub>
- offers GitHub Copilot CLI as a provider <sub>`GitHub Copilot CLI provider`</sub>
- defaults Copilot to the auto model and PATH lookup <sub>`GitHub Copilot CLI provider`</sub>
- choosing GitHub Copilot saves it <sub>`GitHub Copilot CLI provider`</sub>
- saves the Copilot model and executable path <sub>`GitHub Copilot CLI provider`</sub>
- the provider status reports whether the Copilot CLI was found <sub>`GitHub Copilot CLI provider`</sub>
- the connection badge follows the Copilot CLI detection for the Copilot route <sub>`GitHub Copilot CLI provider`</sub>
- a signed-out Copilot fails the connection test with sign-in guidance <sub>`GitHub Copilot CLI provider`</sub>
- never keeps a GitHub credential in the frontend state <sub>`GitHub Copilot CLI provider`</sub>
- offers an OpenAI-compatible endpoint as a provider <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- defaults to OpenAI itself with an empty model box <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- the Ollama preset points at the local Ollama server <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- saves the endpoint URL and the model name <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- saves its key separately from the Anthropic one and never keeps the value <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- removing its key asks the backend for that provider's entry <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- the connection badge needs a model plus either a saved key or a local URL <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- the provider status reports the endpoint, the model and whether a key is saved <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- a rejected key fails the connection test with its own error kind <sub>`OpenAI-compatible provider (incl. Ollama)`</sub>
- offers Google Gemini as a provider with a default model <sub>`Google Gemini provider`</sub>
- saves the Gemini model name <sub>`Google Gemini provider`</sub>
- saves its key separately from the other providers and never keeps the value <sub>`Google Gemini provider`</sub>
- removing its key asks the backend for that provider's entry <sub>`Google Gemini provider`</sub>
- the connection badge needs a saved key plus a model name <sub>`Google Gemini provider`</sub>
- the provider status reports the Gemini model and whether a key is saved <sub>`Google Gemini provider`</sub>
- a rejected key fails the connection test with its own error kind <sub>`Google Gemini provider`</sub>

### Reports (BOM / wire list)

- offers the six report kinds of the reports tab <sub>`report generation dialog store`</sub>
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
- saves the knowledge-file path and adopts the backend's normalized value <sub>`settings store`</sub>
- a failed save keeps the error and leaves the shown settings untouched <sub>`settings store`</sub>

### simulation

- run fetches the DC result and opens the panel <sub>`simulation store`</sub>
- toggled switches are passed as open_switches on the next run <sub>`simulation store`</sub>
- failures (e.g. ngspice missing) keep the error message and still open the panel <sub>`simulation store`</sub>

### Start templates

- opens with the first template selected <sub>`template picker store`</sub>
- switches the selected template when another tile is picked <sub>`template picker store`</sub>
- applies the selected template to the current sheet and closes <sub>`template picker store`</sub>
- mirrors the applied template into the drawing and enables undo <sub>`template picker store`</sub>
- applies nothing when the dialog is cancelled <sub>`template picker store`</sub>
- keeps the dialog open and shows why when applying fails <sub>`template picker store`</sub>
- reports template files it could not read <sub>`template picker store`</sub>
- opens with nothing selected when there are no templates <sub>`template picker store`</sub>
- opens the user template folder in the file manager <sub>`template picker store`</sub>
- falls back to naming the folder when it cannot be opened <sub>`template picker store`</sub>
- shows names and descriptions in the UI language <sub>`template picker store`</sub>
- builds a throwaway sheet from the template's commands <sub>`template preview`</sub>
- centres the template's drawing in the preview canvas <sub>`template preview`</sub>
- survives a template that draws nothing <sub>`template preview`</sub>

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

### Tidy variants (sheet copies / adopt)

- start copies the active sheet once per variant, sends the same tidy prompt in a separate conversation per copy, and opens the panel <sub>`variants store: 整え案の開始`</sub>
- a selection is mapped to the copy's entity ids in each prompt <sub>`variants store: 整え案の開始`</sub>
- metrics of the original and every variant are fetched after start <sub>`variants store: 整え案の開始`</sub>
- start refuses without a sheet, with a count outside 2..4, and while a comparison is in progress <sub>`variants store: 整え案の開始`</sub>
- a failed copy is logged and nothing starts <sub>`variants store: 整え案の開始`</sub>
- variants count as running while their conversation answers, and adopt waits for all of them <sub>`variants store: 比較・採用・破棄`</sub>
- adopt finishes with the chosen sheet, returns to the original sheet and ends the comparison <sub>`variants store: 比較・採用・破棄`</sub>
- discard cancels the conversations still running and removes every copy <sub>`variants store: 比較・採用・破棄`</sub>
- a failed finish is logged and the comparison stays open <sub>`variants store: 比較・採用・破棄`</sub>
- closing the panel keeps the comparison and it can be reopened; nothing opens without a run <sub>`variants store: 比較・採用・破棄`</sub>
- refreshing metrics leaves a variant without metrics when its sheet cannot be measured <sub>`variants store: 比較・採用・破棄`</sub>

### verification

- run fetches diagnostics, opens the panel, and counts by severity <sub>`verification store`</sub>
- close hides the panel but keeps the diagnostics <sub>`verification store`</sub>
- a failed run resets the running flag and leaves diagnostics empty <sub>`verification store`</sub>

### macroPlacement

- switches to the macro tool with variant A and no rotation <sub>`starting macro placement`</sub>
- leaves macro placement on Escape, back to the select tool <sub>`starting macro placement`</sub>
- rotates the ghost a quarter turn per R, back to the start after four <sub>`R and Tab during macro placement`</sub>
- cycles the variants A, B, C and back to A on Tab <sub>`R and Tab during macro placement`</sub>
- keeps the single variant A on Tab when the macro has no other variants <sub>`R and Tab during macro placement`</sub>
- does not swallow Tab when no macro is being placed <sub>`R and Tab during macro placement`</sub>
- applies a library macro by id at the clicked point with the current variant and rotation <sub>`confirming a macro placement`</sub>
- applies a pasted macro by value instead of by id, since it has no file <sub>`confirming a macro placement`</sub>
- stays in macro placement after a click, so the same macro can be placed again <sub>`confirming a macro placement`</sub>
- passes the value set chosen in the dialog to the insert <sub>`placing a macro with a value set`</sub>
- inserts a macro with no value set chosen just as it was saved <sub>`placing a macro with a value set`</sub>
- copies the selection on Cmd+C and starts placing it on Cmd+V <sub>`Cmd+C / Cmd+V`</sub>
- does not swallow Cmd+C with an empty selection nor Cmd+V with an empty clipboard <sub>`Cmd+C / Cmd+V`</sub>


## FreeCAD add-on (MadakeCAD Link)


### Link API client (Python)

- The client talks to MadakeCAD's Link API on localhost, port 9310 unless told otherwise. <sub>`test_base_url_points_at_the_local_link_api`</sub>
- The connection check and the project snapshot come back as decoded JSON. <sub>`test_health_and_project_are_decoded_from_json`</sub>
- Netlist and parts requests pass the sheet id, search text and category as query parameters. <sub>`test_netlist_and_parts_pass_their_filters_as_query_parameters`</sub>
- Commands are posted as a JSON array and one patch comes back per command, so every write goes through MadakeCAD's Command engine. <sub>`test_commands_are_posted_as_a_json_array_and_return_one_patch_each`</sub>
- A refused connection, an HTTP error and invalid JSON all raise LinkError with a message that says what went wrong. <sub>`test_errors_are_reported_as_link_errors_with_a_readable_message`</sub>
- The event stream yields each patch as ("patch", {revision, ops}), joining multi-line data and skipping keep-alive comments. <sub>`test_events_yield_patches_from_the_sse_stream`</sub>
- Lines without an event name are "message" events, data is dispatched at the blank line, and a trailing event without a blank line is still delivered. <sub>`test_parse_sse_defaults_the_event_name_and_dispatches_on_blank_lines`</sub>

### Live follow & settings

- A patch is applied only when its revision is newer than the last one seen; duplicates and older patches are ignored. <sub>`test_only_newer_revisions_are_accepted`</sub>
- The panel reloads when the shown sheet's entities change or when sheets are added, removed, renamed or the project is replaced, but not for edits on another sheet. <sub>`test_refresh_is_needed_for_the_shown_sheet_and_structural_changes`</sub>
- Before a sheet has been chosen, any entity change reloads the panel so the first view is current. <sub>`test_with_no_sheet_shown_yet_any_entity_change_reloads`</sub>
- The port setting keeps integers from 1 to 65535 and falls back to 9310 for anything else (text, 0, too large). <sub>`test_port_setting_accepts_valid_ports_and_falls_back_to_the_default`</sub>

### Part linking & wire-length write-back

- The parts list has one row per symbol, sorted by sheet and designator, with the 3D model of its part number and the FreeCAD object it is linked to. <sub>`test_part_rows_list_symbols_with_their_3d_model_and_linked_object`</sub>
- The wire list has one row per wire with its net or wire number, current length, where the length came from, and the linked route object. <sub>`test_wire_rows_show_net_length_source_and_linked_route`</sub>
- STEP/IGES/BREP files are inserted as shapes, .FCStd files are merged as documents, and anything else cannot be inserted. <sub>`test_model_kind_recognises_shape_files_and_freecad_documents`</sub>
- Linking sends a set_mech_link command with the entity id, the FreeCAD document path, the object name and the sync time; unlinking sends remove_mech_link. <sub>`test_link_commands_carry_the_entity_id_document_path_object_name_and_time`</sub>
- Route objects are matched to wires by their madake_id; lengths are converted from millimetres to metres (1 mm resolution) and objects that are not wires are ignored. <sub>`test_measured_route_lengths_are_converted_to_metres_and_matched_to_wires`</sub>
- The write-back is one set_wire_lengths command per sheet whose entries carry the metre length and the source "freecad", so MadakeCAD can warn before a manual overwrite. <sub>`test_write_back_is_one_command_per_sheet_with_the_freecad_source`</sub>
- Measuring also registers the route object as the wire's link, but only when the wire is not already linked to that same object. <sub>`test_routes_are_linked_only_when_not_already_linked_to_that_object`</sub>

### Project overview & netlist view

- The project summary shows the project name, the revision and, per sheet, how many symbols, wires and other entities it holds. <sub>`test_summary_carries_name_revision_and_per_sheet_entity_counts`</sub>
- An empty or partial snapshot summarizes to an unnamed project with no sheets instead of failing. <sub>`test_an_empty_snapshot_summarizes_to_zero`</sub>
- Each netlist row shows the net name, wire number, label, the pins as "K1:A1, TB1:3" and how many wires form the net. <sub>`test_rows_show_net_name_wire_number_label_pins_and_wire_count`</sub>
- A pin whose symbol has no designator is shown as "?:<pin>" so the row still reads. <sub>`test_pins_without_a_designator_show_a_placeholder`</sub>

### Route sync, net highlight & placements

- A wire whose net joins two linked parts gets one route stub from one part's placement to the other's, tagged with the wire id and named after the net. <sub>`test_a_wire_between_two_linked_parts_gets_one_stub_between_their_placements`</sub>
- Nets whose parts are not linked, or whose linked objects are missing from the FreeCAD document, produce no stub. <sub>`test_parts_without_a_link_or_not_in_the_document_produce_no_stub`</sub>
- Three linked parts on one net are chained in name order into two segments, assigned to the net's wires in order; extra wires get no stub and extra segments are dropped when wires run out. <sub>`test_a_net_with_three_parts_is_chained_and_segments_follow_the_wire_ids`</sub>
- Wires that already have a route object in the document are skipped, so creating stubs again never duplicates them. <sub>`test_rerunning_skips_wires_that_already_have_a_route_object`</sub>
- Highlighting a net selects the FreeCAD objects of its linked parts and of its linked route wires, each object once, ignoring unlinked pins. <sub>`test_highlighting_a_net_selects_its_linked_parts_and_routes_once_each`</sub>
- Syncing a placement sends set_mech_link with the object's base point in millimetres (1 µm resolution) and its rotation in degrees, keeping the existing link data. <sub>`test_placement_command_carries_the_base_in_mm_and_the_rotation_in_degrees`</sub>
- Sync placements writes one command per linked object present in the document and skips objects whose stored placement is already the same. <sub>`test_sync_placements_writes_only_linked_objects_whose_placement_changed`</sub>

