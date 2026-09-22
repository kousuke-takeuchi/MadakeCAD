# Detailed Specification

**日本語: [13-specification.ja.md](13-specification.ja.md)** | ← [Roadmap](12-roadmap.md)

> **Generated from the test suite — do not edit by hand.**
> Every clause below is enforced by an automated test; the test id is shown in gray.
> Regenerate with `python3 scripts/gen_spec.py` after changing tests.

This document is the living, always-verified specification of MadakeCAD:
if a behavior is listed here, a test proves it on every run of the suite.


**1952 specification clauses** across 5 areas.


## Core domain (madake-core)


### block

- A block symbol puts its named pins on both sides of a box, 5 mm apart and centred on the 2.5 mm grid, each pointing outwards. <sub>`block_pins_sit_on_both_sides_five_millimetres_apart`</sub>
- Pin numbers are drawn outside the outline next to the lead, and pin names inside it. <sub>`pin_numbers_are_drawn_outside_and_pin_names_inside`</sub>
- The box grows automatically with the number of pins and the length of the pin names, always in 5 mm steps. <sub>`auto_size_grows_with_pin_count_and_name_length`</sub>
- A block with no pin table at all is still a valid, minimum-sized empty box. <sub>`a_block_without_a_pin_table_is_an_empty_box`</sub>
- Top and bottom pins run left to right, and their captions stay inside the box because text cannot be drawn sideways. <sub>`top_and_bottom_pins_run_left_to_right`</sub>
- An explicit size is rounded up to the 5 mm step so the outline keeps sitting on the grid. <sub>`an_explicit_size_is_rounded_up_to_the_grid`</sub>
- Two pins with the same number, or a pin with an empty number, are rejected instead of silently drawn. <sub>`duplicate_or_empty_pin_numbers_are_rejected`</sub>
- A size too small for the pins is rejected, and so is a pin table that is not valid JSON or uses an unknown side. <sub>`a_size_too_small_or_a_malformed_pin_table_is_rejected`</sub>
- A block cannot exceed the pin limit, so a runaway pin table fails loudly. <sub>`too_many_pins_are_rejected`</sub>
- The attribute slots (tag, part number, function, rating) stack at the centre of the box. <sub>`attribute_slots_stack_at_the_centre_of_the_box`</sub>
- Rotating or mirroring a block moves its pins with the body but never renumbers them. <sub>`rotating_or_mirroring_keeps_the_pin_numbers`</sub>

### Command engine (undo/redo)

- A symbol whose id is not in the library is refused instead of being stored as an invisible entity, and the sheet is left untouched. <sub>`an_unknown_symbol_id_is_refused_and_changes_nothing`</sub>
- A parametric symbol asked for outside its pole range (0 poles, or more than the limit) is refused with a message that says the range. <sub>`a_pole_count_outside_the_range_is_refused`</sub>
- Every ordinary placement still goes through: bundled symbols, parametric symbols inside their range, and a block defined by its attributes. <sub>`valid_symbol_ids_are_still_accepted`</sub>
- Changing an existing symbol to an unknown id is refused too, so a symbol on the sheet can always be drawn. <sub>`changing_a_symbol_to_an_unknown_id_is_refused`</sub>
- A block whose pin table is broken (duplicate numbers) is refused when added, and the sheet is left untouched. <sub>`a_block_with_a_broken_pin_table_is_refused_and_changes_nothing`</sub>
- Editing a good block into a broken one is refused too, so a block on the sheet always has a drawable pin table. <sub>`editing_a_block_into_a_broken_one_is_refused`</sub>
- A note whose colour is a name the drawing does not know (a typo, or a raw colour code) is refused instead of being stored and silently drawn in black. <sub>`a_note_with_an_unknown_colour_is_refused`</sub>
- A note that uses one of the drawing's colour tokens (or no colour at all) is accepted. <sub>`a_note_with_a_known_colour_token_is_accepted`</sub>
- Changing an existing note to an unknown colour is refused too, and the note keeps the colour it had. <sub>`recolouring_a_note_to_an_unknown_colour_is_refused`</sub>
- Adding an entity can be undone (the entity disappears) and redone (it comes back). <sub>`add_undo_redo_entity`</sub>
- Moving entities shifts their coordinates; undo restores the original position exactly. <sub>`move_and_undo_restores_position`</sub>
- Deleting several entities at once is a single undo step that restores all of them. <sub>`delete_multiple_and_undo`</sub>
- Sheets can be added and removed; undoing a removal restores the sheet with its original id and position. <sub>`sheet_add_remove_undo`</sub>
- Adding a sheet without naming a kind gives an ordinary schematic sheet, and a panel-layout sheet is added by asking for the panel kind (undo removes it either way). <sub>`add_sheet_defaults_to_schematic_and_can_add_a_panel_sheet`</sub>
- A sheet-add command written before panel sheets existed (no kind field) still works and adds a schematic sheet. <sub>`a_sheet_add_command_without_a_kind_still_adds_a_schematic_sheet`</sub>
- The four panel entities (panel outline, DIN rail, duct, footprint) are added, moved and deleted with the ordinary entity commands, and every step can be undone. <sub>`panel_entities_are_added_moved_and_deleted_with_undo`</sub>
- Editing a footprint (its reference, part number or rotation) replaces it wholesale, and undo brings back the previous version. <sub>`a_footprint_can_be_edited_and_undone`</sub>
- Commands serialize to JSON and back without loss, so any client can send them over the wire. <sub>`command_json_roundtrip`</sub>
- UpdateEntity replaces an entity wholesale; undo brings back the previous version. <sub>`update_entity_undo_restores_previous_version`</sub>
- SetTitleBlock updates the sheet's title block; undo restores the previous fields. <sub>`set_title_block_is_undoable`</sub>
- SetRevisions replaces the revision-table rows; undo restores the previous list. <sub>`set_revisions_is_undoable`</sub>
- The annotation text height of a sheet is set through a command, and undo puts the previous height back. <sub>`set_sheet_annotation_text_is_undoable`</sub>
- Clearing the annotation text height with null puts the sheet back to the default 2.0mm. <sub>`clearing_the_annotation_text_height_restores_the_default`</sub>
- An annotation text height outside 1.0-3.5mm is rejected with a reason, and the sheet is left untouched. <sub>`an_out_of_range_annotation_text_height_is_rejected`</sub>
- The annotation text height command is plain JSON, so AI agents and the CLI can send it. <sub>`set_sheet_annotation_text_is_plain_json`</sub>
- SetPlcAssignments replaces the whole PLC I/O assignment table of the project; undo restores the previous table. <sub>`set_plc_assignments_is_undoable`</sub>
- SetFrameTemplate switches the frame template of the whole project in one edit, and undo puts the previous one back (undoing the very first switch returns to the default frame). <sub>`set_frame_template_is_undoable`</sub>
- SetFrameTemplate accepts an id nobody knows: the drawing keeps printing with the default frame, so a project made elsewhere still opens. <sub>`set_frame_template_accepts_an_unknown_id`</sub>
- A harness boundary is added, renamed and deleted with the ordinary entity commands, and every step can be undone. <sub>`harness_add_rename_delete_are_undoable`</sub>
- Moving a harness boundary shifts all of its corners, and undo puts them back exactly. <sub>`harness_move_and_undo_restores_every_corner`</sub>
- A harness boundary survives a JSON round trip with the kind tag "harness", so any client can send it. <sub>`harness_command_json_roundtrip_uses_the_harness_kind`</sub>
- All four kinds of shape are added, edited and deleted with the ordinary entity commands, and every step can be undone. <sub>`shape_add_edit_delete_are_undoable_for_every_kind`</sub>
- Moving a shape shifts every one of its points, and undo puts them back exactly. <sub>`shape_move_and_undo_restores_every_point`</sub>
- A shape with a raw colour code instead of a colour token is refused, and the drawing is left untouched. <sub>`a_shape_with_a_raw_colour_code_is_refused`</sub>
- A shape whose stroke is too thin to print or thick enough to swamp the grid is refused. <sub>`a_shape_with_an_unprintable_stroke_width_is_refused`</sub>
- A rectangle sent with the wrong number of points is refused, so a half-drawn shape never reaches the drawing. <sub>`a_shape_with_the_wrong_number_of_points_is_refused`</sub>
- A shape survives a JSON round trip with the kind tag "shape", so any client can send it. <sub>`shape_command_json_roundtrip_uses_the_shape_kind`</sub>
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
- Numbering the panel gives every footprint on that sheet an item number in reading order. <sub>`renumbering_a_panel_numbers_every_footprint`</sub>
- Undoing the numbering puts the previous item numbers back, including parts that had no number at all. <sub>`undoing_the_numbering_restores_the_previous_numbers`</sub>
- Numbering the same panel a second time changes nothing on the drawing: the numbers are already the ones the reading order asks for. <sub>`numbering_an_already_numbered_panel_changes_nothing`</sub>
- The numbering command is plain JSON, so agents and the CLI can send it. <sub>`the_panel_numbering_command_is_plain_json`</sub>
- Numbering a sheet that does not exist is refused instead of silently doing nothing. <sub>`numbering_an_unknown_sheet_is_refused`</sub>

### Frame templates (plot frames)

- The two bundled frame templates (JIS A3 landscape and JIS A4 portrait) are listed in that order with English and Japanese names, and both are valid. <sub>`the_two_bundled_frame_templates_are_listed_in_both_languages`</sub>
- Even without the resource directory, the frame templates compiled into the build are still available. <sub>`frame_templates_are_available_without_the_resource_directory`</sub>
- The jis-a3 template describes the current JIS frame: 10 mm margin, 4x6 zones with numeric columns and alphabetic rows, and a 120x32 mm title block anchored at the bottom-right corner. <sub>`the_jis_a3_template_describes_the_current_jis_frame`</sub>
- The jis-a4-portrait template is a 210x297 frame whose title block keeps the same cells as jis-a3 but is narrowed to 110 mm, with revision columns that still add up to the title-block width. <sub>`the_a4_portrait_template_narrows_the_same_title_block`</sub>
- Every documented bind resolves to its value: project name and sheet number/total, sheet name, latest revision, all title-block fields and the {date}/{scale}/{designer}/{approver} aliases. <sub>`every_documented_bind_resolves_to_its_value`</sub>
- {rev.latest} follows the newest revision row rather than the stored Rev field. <sub>`the_latest_revision_bind_follows_the_revision_table`</sub>
- An unknown bind becomes an empty string instead of an error, so a drawing with a typo in its template still prints. <sub>`an_unknown_bind_becomes_an_empty_string`</sub>
- Text with no binds is printed as is, and an unclosed brace is left alone instead of swallowing the rest of the line. <sub>`plain_text_and_unclosed_braces_are_left_alone`</sub>
- On its own, a sheet is bound as sheet 1 of 1 with an empty project name; inside a project it gets its real position. <sub>`a_lone_sheet_is_bound_as_sheet_one_of_one`</sub>
- The project number bind is known but always empty for now, because the project model has no number field yet. <sub>`the_project_number_bind_is_known_but_empty_for_now`</sub>
- A cell with fixed text prints it verbatim, braces included, while a cell with a bind resolves it. <sub>`a_fixed_text_cell_is_printed_verbatim`</sub>
- A template whose cell has both text and bind, or neither, is rejected with a message naming the cell. <sub>`a_cell_must_have_either_text_or_bind`</sub>
- The revision columns must be the four JIS columns and add up to the title-block width, otherwise the revision table would not line up with the title block. <sub>`the_revision_columns_must_add_up_to_the_title_block_width`</sub>
- Zero zone columns or rows, a negative margin and a zero-height title block are all rejected. <sub>`degenerate_frame_definitions_are_rejected`</sub>
- Two cells may carry the same bind and a cell may sit outside the title-block rectangle: both are normal (the company name is printed to the left of the block). <sub>`duplicate_binds_and_cells_outside_the_block_are_allowed`</sub>
- Frame templates the user drops into ~/MadakeCAD/templates/frames are listed after the bundled ones, and one with a bundled id replaces it. <sub>`user_frame_templates_are_listed_after_the_bundled_ones`</sub>
- A frame template file that is broken JSON, or valid JSON with an invalid definition, is reported with its path and reason while the other templates stay usable. <sub>`a_broken_frame_template_file_is_reported_with_its_path`</sub>
- Resolving no id at all, or an id nobody knows, both fall back to the bundled jis-a3 frame so the drawing still prints. <sub>`an_unknown_frame_template_id_falls_back_to_jis_a3`</sub>
- A frame template is a paper-independent definition: it is drawn on whatever paper the sheet uses, so switching frames never resizes the sheet. <sub>`a_frame_template_does_not_change_the_paper_size`</sub>
- Zone labels come from the template: numeric columns count 1,2,3 and alphabetic rows run A,B,C wrapping back to A after Z. <sub>`zone_labels_follow_the_configured_style`</sub>

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
- A project file saved before frame templates existed opens with no frame template set, which means it is drawn with the default JIS A3 frame exactly as before. <sub>`an_old_project_file_opens_with_the_default_frame_template`</sub>
- A project file saved before panel layouts existed opens with every sheet as a schematic sheet and is brought up to the current format version. <sub>`an_old_project_file_opens_with_schematic_sheets`</sub>
- A project file saved before shapes existed opens exactly as it was, with no shapes on its sheets, and is brought up to the current format version. <sub>`an_old_project_file_opens_without_any_shapes`</sub>
- A shape drawn on a sheet survives a save and reload with its kind, points and look unchanged. <sub>`a_shape_survives_a_save_and_reload`</sub>
- A panel sheet and the panel entities on it survive a save and reload unchanged. <sub>`a_panel_sheet_survives_a_save_and_reload`</sub>
- A project file saved before text styling existed opens with every note unstyled (left-aligned, default colour, not bold) and is brought up to the current format version. <sub>`an_old_project_file_opens_with_unstyled_notes`</sub>
- The current file format version is 8, the version that added the graphics-only shape entity. <sub>`the_current_format_version_is_eight`</sub>
- A drawing saved before the annotation text height existed opens at the default 2.0mm, so its notes look exactly as before. <sub>`an_old_project_file_opens_with_the_default_annotation_text_height`</sub>
- The annotation text height set on a sheet is saved to the file and comes back on reload. <sub>`the_annotation_text_height_survives_a_save_and_reload`</sub>
- A panel drawn before item numbers existed opens with no balloons at all: the footprints keep their place and can be numbered whenever the drawing is next tidied up. <sub>`an_old_panel_file_opens_without_item_numbers`</sub>
- The frame template chosen for a project is saved to the file and comes back on reload. <sub>`the_frame_template_survives_a_save_and_reload`</sub>

### KiCad import

- KiCad import converts paper size, title block, wires, junctions, labels (power symbols become net labels) and text. <sub>`imports_paper_title_block_and_geometry`</sub>
- Known lib_ids map to our symbols (Device:R -> resistor, Conn_01x03 -> connector_3p) keeping designator/value/rotation/mirror; unknown symbols are skipped and itemized in the report. <sub>`maps_symbols_and_reports_skipped`</sub>
- A file that is not a kicad_sch document is rejected with a clear error. <sub>`rejects_non_schematic`</sub>
- The S-expression parser reads atoms, quoted strings, numbers and nested lists with typed accessors. <sub>`parses_atoms_strings_numbers_and_nesting`</sub>
- Escaped quotes/newlines and multibyte (Japanese) text inside strings parse correctly. <sub>`parses_escaped_strings_and_multibyte`</sub>
- children(name) iterates every child list with the given head symbol. <sub>`children_iterates_all_matches`</sub>
- Unbalanced parentheses and unterminated strings are reported as syntax errors with a position. <sub>`syntax_errors_are_reported`</sub>

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
- A new project starts with one sheet named "Sheet1", the current file format version, an empty PLC assignment table and no explicit frame template (so it uses the default JIS A3 frame). <sub>`new_project_has_one_default_sheet`</sub>
- Entity::translate moves every coordinate of the entity: all wire points, or the anchor of symbols/labels/text. <sub>`translate_moves_all_coordinates`</sub>
- A new sheet is a schematic sheet, and a panel-layout sheet is a separate sheet kind recorded in the file. <sub>`a_new_sheet_is_schematic_and_a_panel_sheet_is_a_separate_kind`</sub>
- A sheet saved before panel layouts existed carries no sheet kind, and it opens as an ordinary schematic sheet. <sub>`a_sheet_without_a_kind_opens_as_a_schematic_sheet`</sub>
- The sheet kind travels as JSON in snake_case ("schematic" / "panel"). <sub>`the_sheet_kind_is_serialized_in_snake_case`</sub>
- Moving a panel sheet moves its panel entities too: outline, duct and footprint move by their top-left corner, and a DIN rail moves along both axes. <sub>`translate_moves_panel_entities`</sub>
- Panel entities travel as JSON under their own kind tags, so any client can send them. <sub>`panel_entities_have_their_own_json_kind_tags`</sub>
- A note written before text styling existed opens with the look it always had: left-aligned, plain black, not bold. <sub>`an_old_note_opens_with_the_previous_look`</sub>
- The text anchor travels as JSON in snake_case ("start" / "middle" / "end"). <sub>`the_text_anchor_is_serialized_in_snake_case`</sub>
- A styled note keeps its anchor, colour token and bold flag through a JSON round trip. <sub>`a_styled_note_survives_a_json_round_trip`</sub>
- Moving a shape shifts every one of its points, so a rectangle keeps its size and a polyline keeps its bends. <sub>`translate_moves_every_point_of_a_shape`</sub>
- A shape travels as JSON under the kind tag "shape", with its own kind of shape in snake_case, so any client can send one. <sub>`a_shape_is_serialized_under_the_shape_kind_tag`</sub>
- The four kinds of shape all travel in snake_case. <sub>`the_shape_kinds_are_serialized_in_snake_case`</sub>
- A shape sent without the optional look fields opens plain: default colour, hairline stroke, no fill, solid line. <sub>`a_shape_without_look_fields_opens_plain`</sub>
- Entity::id() returns the inner entity's UUID regardless of the entity kind. <sub>`entity_id_is_uniform_across_kinds`</sub>

### net_tag

- A net label with tag "right" is drawn as a five-cornered flag whose point sticks out to the right of the label position. <sub>`tag_right_points_to_the_right`</sub>
- A net label with tag "left" is drawn as the mirrored flag, with the point sticking out to the left. <sub>`tag_left_points_to_the_left`</sub>
- The body of the flag never covers the label's own position: it grows away from the point, so the wire the label sits on stays readable. <sub>`the_flag_body_grows_away_from_the_point`</sub>
- The label position is the root of the point, so a left flag and a right flag are exact mirror images of each other. <sub>`the_label_position_is_the_root_of_the_point`</sub>
- The tag text always sits inside the flag, and the flag grows with the length of the net name. <sub>`tag_box_follows_the_text_width`</sub>
- Both tag directions produce a flag of the very same size, so left and right labels line up. <sub>`both_directions_have_the_same_size`</sub>
- The cross-sheet reference is pushed out on the body side of the flag, so it never lands on the point or on the wire. <sub>`the_cross_reference_escapes_on_the_body_side`</sub>
- A smaller annotation text height shrinks the whole flag - text, padding and point - keeping its shape. <sub>`a_smaller_annotation_height_shrinks_the_whole_flag`</sub>
- A net label without the tag attribute keeps the plain text look it has always had. <sub>`label_without_tag_attribute_stays_plain_text`</sub>
- An unknown tag value (a typo, or "up") is ignored and the label falls back to plain text instead of failing. <sub>`unknown_tag_value_is_ignored`</sub>

### Netlist extraction

- A wire whose endpoints touch two symbol pins joins those pins into one net. <sub>`wires_connect_symbol_pins_into_one_net`</sub>
- A wire that meets a block pin joins that pin to the net under the pin's own name, not a number. <sub>`block_pins_join_nets_under_their_own_names`</sub>
- Wires that merely cross do NOT connect; a net label attached to a wire names its net. <sub>`crossing_without_junction_stays_separate_and_label_names_net`</sub>
- A junction dot connects crossing wires, and same-named labels merge distant nets into one. <sub>`junction_connects_crossing_wires_and_same_labels_merge`</sub>
- A net label placed exactly on a symbol pin connects that pin to the net of the same name elsewhere, without a wire stub. <sub>`a_label_on_a_pin_joins_that_pin_to_the_net_of_the_same_name`</sub>
- The attachment test used by the netlist accepts a label on a wire vertex, in the middle of a wire segment, or on a symbol pin, and rejects one that floats even slightly off. <sub>`the_label_attachment_test_accepts_vertices_segments_and_pins`</sub>
- Unnamed nets receive deterministic sequential names (N001, N002, ...). <sub>`unnamed_nets_get_deterministic_sequential_names`</sub>
- Two connection points of the same symbol sharing a pin number (a feed-through terminal) are internally shorted, and appear once in the net's pin list. <sub>`same_pin_number_points_short_internally`</sub>
- A multi-level terminal block never shorts its levels together: level a and level b of the same pole end up on two separate nets, even though each level's own left and right connection points are still shorted internally (through-terminal). <sub>`each_level_of_a_multilevel_terminal_block_is_an_independent_net`</sub>
- A wire number written on the wires becomes the net's displayed name when the net has no net label. <sub>`wire_number_names_a_net_without_a_label`</sub>
- A net label always wins over the wire number, which in turn wins over the automatic N001 name. <sub>`net_name_prefers_label_then_wire_number_then_auto_name`</sub>
- Pin positions honor the symbol's rotation (clockwise in the Y-down paper coordinate system) and placement. <sub>`pin_positions_apply_rotation_and_translation`</sub>
- Mirroring flips pins across the vertical axis before rotation is applied. <sub>`pin_positions_apply_mirror_before_rotation`</sub>
- An arc inside a symbol turns with the symbol, so the contactor's half circle keeps facing its moving contact at every rotation, and mirroring flips it without reversing the drawing direction. <sub>`arc_angles_follow_symbol_rotation_and_mirror`</sub>

### ngspice runner

- The ngspice 'print all' output parses into node voltages (v(n1) -> n1) and branch currents (v1#branch), ignoring noise lines. <sub>`parse_print_all_reads_nodes_and_branches`</sub>
- The ngspice executable is located by priority: MADAKE_NGSPICE env var, then PATH, then OS default install paths; a missing env path falls through. <sub>`find_in_prefers_env_then_path_then_candidates`</sub>
- Running a DC operating point on a 24 V / 6+6 ohm divider yields 12 V at the midpoint and 2 A of source current (requires ngspice; skipped otherwise). <sub>`run_op_solves_a_divider_when_ngspice_is_installed`</sub>

### panel

- A footprint occupies the rectangle of its own size, and turning it 90 degrees swaps width and height. <sub>`a_rotated_footprint_swaps_width_and_height`</sub>
- The panel outline draws the enclosure and, inset inside it, the mounting plate that parts are screwed onto. <sub>`the_panel_outline_carries_a_mounting_plate_inside_it`</sub>
- A panel outline smaller than the inset on both sides has no mounting plate (there is nothing left to draw). <sub>`a_tiny_panel_outline_has_no_mounting_plate`</sub>
- A DIN rail is drawn as a horizontal band centred on its mounting line, so parts snapped to the line sit on the rail. <sub>`a_din_rail_is_a_band_centred_on_its_mounting_line`</sub>
- A rail drawn from right to left covers the same span as one drawn from left to right. <sub>`a_rail_drawn_backwards_covers_the_same_span`</sub>
- A duct is filled with 45-degree hatch lines that stay inside it, spaced on a fixed pitch so neighbouring ducts line up. <sub>`a_duct_is_filled_with_45_degree_hatch_lines`</sub>
- A duct with no width or height has nothing to hatch. <sub>`a_flat_duct_has_no_hatch_lines`</sub>
- Every part on the schematic that has no footprint on a panel sheet yet is listed as unplaced, with its part number and outline size. <sub>`parts_without_a_footprint_are_listed_as_unplaced`</sub>
- A part whose type number has no size in the parts database is still placeable: it comes with the default rectangle, which can be adjusted afterwards. <sub>`an_unknown_type_number_falls_back_to_the_default_footprint_size`</sub>
- With no panel sheet in the project, every part of the schematic is unplaced. <sub>`without_a_panel_sheet_every_part_is_unplaced`</sub>
- One device is listed once even when it appears on the schematic several times (a relay coil and its contacts share one reference). <sub>`a_device_drawn_several_times_is_listed_once`</sub>
- Symbols with no reference designator yet (nothing to place on the panel) are left out of the unplaced list. <sub>`symbols_without_a_reference_are_not_listed`</sub>
- A footprint whose reference does not exist on the schematic is reported as an orphan, so a renamed or deleted part is not forgotten on the panel. <sub>`a_footprint_with_no_matching_part_is_reported`</sub>
- A footprint that is not linked to a reference designator (a panel-only part) is not reported as an orphan. <sub>`a_footprint_without_a_reference_is_not_reported`</sub>
- Item numbers are handed out in reading order: the upper row first, and within a row from left to right. <sub>`item_numbers_follow_reading_order`</sub>
- Parts mounted on the same DIN rail form one row even when their outlines have different heights. <sub>`parts_on_the_same_rail_form_one_row`</sub>
- Renumbering gives 1..N in reading order, so the numbers on the drawing read from the top left down. <sub>`renumbering_hands_out_one_to_n`</sub>
- A footprint with no item number yet has no balloon (nothing is drawn until the parts are numbered). <sub>`an_unnumbered_footprint_has_no_balloon`</sub>
- The balloon is a small circle just outside the top-left corner of the footprint, joined to that corner by a short leader line. <sub>`the_balloon_sits_outside_the_top_left_corner`</sub>
- Every reference designator placed on a panel carries its item number, so the parts list can show the same numbers. <sub>`item_numbers_can_be_looked_up_by_reference`</sub>
- The legend lists the numbered parts of its own sheet in item-number order, as item number, reference and part number. <sub>`the_legend_lists_the_numbered_parts_in_order`</sub>
- Parts that have not been numbered yet still appear at the end of the legend with an empty number column, so nothing on the panel is silently left out. <sub>`unnumbered_parts_come_last_in_the_legend`</sub>
- The legend table grows downwards one row at a time, and its width is fixed by the column widths. <sub>`the_legend_table_grows_with_its_rows`</sub>
- The panel placement table lists every footprint with its item number, reference, part number, position and rail. <sub>`the_placement_table_lists_position_and_rail`</sub>
- A part hung between the rails leaves the rail column empty in the placement table. <sub>`a_part_off_the_rail_has_an_empty_rail_column`</sub>
- Rails are numbered from the top of the panel down, so "rail 1" means the same thing in the placement table and in the properties panel. <sub>`rails_are_numbered_from_the_top_down`</sub>
- Two footprints that overlap on the mounting plate are reported, because the parts cannot both be screwed there. <sub>`overlapping_footprints_are_reported`</sub>
- Footprints that only touch along an edge are not reported: parts standing side by side on a rail are normal. <sub>`footprints_that_only_touch_are_not_reported`</sub>
- A part that sits on no DIN rail is reported as a note, so a part left hanging between the rails is noticed. <sub>`a_part_hanging_off_the_rails_is_noted`</sub>
- On a panel drawn without any DIN rail nothing is reported as hanging: everything on that panel is mounted directly on the plate. <sub>`a_panel_without_rails_reports_nothing_as_hanging`</sub>
- Every DIN rail reports how much of its length is taken up by parts. <sub>`each_rail_reports_how_full_it_is`</sub>
- A rail packed beyond 80 percent is raised to a warning: there is no room left to fit or to service the parts. <sub>`a_rail_packed_over_eighty_percent_becomes_a_warning`</sub>
- The checks only look at panel sheets, so a schematic never produces panel diagnostics. <sub>`the_panel_checks_ignore_schematic_sheets`</sub>

### Parts database

- Opening a new database creates the schema and seeds sample parts exactly once; reopening never re-seeds. <sub>`open_creates_schema_and_seeds_samples_once`</sub>
- Parts can be inserted, updated by part number, fetched, and searched by partial name match or exact category. <sub>`upsert_get_and_search`</sub>
- Wire parts are registered and looked up by exact color + gauge combination. <sub>`wire_parts_crud_and_lookup`</sub>
- An old schema-v1 database migrates to v2 on open, preserving existing rows and gaining the spice_model column. <sub>`v1_database_migrates_to_v2_preserving_data`</sub>
- An old schema-v2 database migrates to v3 on open, preserving existing rows and gaining the contact_config column. <sub>`v2_database_migrates_to_v3_preserving_data`</sub>
- The bundled sample relay carries its contact configuration, so a freshly placed relay can be checked for contact overflow. <sub>`sample_relay_part_has_a_contact_configuration`</sub>
- An old schema-v3 database migrates to v4 on open, preserving existing rows and gaining the plc_module column. <sub>`v3_database_migrates_to_v4_preserving_data`</sub>
- Opening an older parts database adds the bundled PLC module samples, so PLC I/O drawings can be generated without registering a module by hand. <sub>`an_older_database_gains_the_bundled_plc_module_samples`</sub>
- The bundled samples include PLC modules covering the Mitsubishi, Siemens and Allen-Bradley address styles. <sub>`sample_plc_modules_cover_the_three_address_styles`</sub>
- The PLC module list contains only parts that carry a module definition, so ordinary parts never show up in the module library. <sub>`the_plc_module_list_contains_only_parts_with_a_module_definition`</sub>
- An old schema-v5 database migrates to v6 on open, preserving existing rows and gaining the footprint size columns used by panel layouts. <sub>`v5_database_migrates_to_v6_preserving_data`</sub>
- The bundled sample parts carry footprint sizes, so a part placed on a panel sheet gets its own outline instead of the fallback rectangle. <sub>`sample_parts_have_footprint_sizes`</sub>
- Opening an older parts database fills in the footprint sizes of the bundled samples that have none yet, without touching sizes someone entered by hand. <sub>`an_older_database_gains_the_bundled_footprint_sizes`</sub>
- A part with only one of the two footprint dimensions filled in has no usable footprint size, because both width and height are needed to draw the outline. <sub>`a_half_filled_footprint_size_is_not_used`</sub>
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
- A panel-layout sheet is part of the book: exporting the project puts it in the PDF right after the schematic sheets. <sub>`the_pdf_book_includes_the_panel_sheet`</sub>
- The panel placement table can be bound into the PDF book like any other report. <sub>`the_pdf_book_can_include_the_panel_placement_table`</sub>

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
- Wiring a field device onto a generated rung makes that device the target of the matching point in the assignment table. <sub>`a_device_wired_onto_a_generated_rung_becomes_that_points_target`</sub>
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
- A changeover contact counts as a contact of its relay, exactly like a make or a break contact. <sub>`a_changeover_contact_belongs_to_its_relay`</sub>
- A relay drawn with changeover contacts is no longer reported as a coil that drives nothing. <sub>`a_relay_with_changeover_contacts_is_not_reported_as_contactless`</sub>
- The terminals of a changeover contact are written as three numbers - common, break, make - like "11-12-14". <sub>`changeover_terminals_are_written_common_break_make`</sub>
- The contact map under the coil lists changeover contacts with their three terminals and their address. <sub>`contact_map_lists_changeover_contacts`</sub>
- A contact configuration like "2CO" is read as two changeover poles. <sub>`contact_config_parses_changeover_poles`</sub>
- The unused poles of a "2CO" relay with one contact drawn are listed as the remaining changeover contact. <sub>`contact_map_shows_the_remaining_changeover_poles`</sub>
- A changeover pole may be drawn as a plain make or break contact, so a "2CO" relay accepts two make contacts. <sub>`a_changeover_pole_can_be_used_as_a_make_or_break_contact`</sub>
- Using more poles than the part has is still an error, and the message names the changeover contact. <sub>`using_more_changeover_contacts_than_the_part_has_is_an_error`</sub>
- A make contact cannot borrow a changeover pole that a changeover contact already uses. <sub>`a_make_contact_cannot_borrow_a_pole_already_in_use`</sub>
- The contact map table is centred under the coil and grows downwards, one row per contact. <sub>`contact_map_table_is_centred_under_the_coil`</sub>
- The contact map hangs below the coil symbol's outline, and the coil location text sits to the right of the contact symbol. <sub>`annotations_are_anchored_to_the_symbol_outline`</sub>
- A changeover contact's coil location moves off the wire note above it, reproducing the collision described in G22. <sub>`coil_location_moves_off_a_colliding_wire_note`</sub>
- When even the space above is occupied, the coil location falls back to below the outline. <sub>`coil_location_falls_back_below_when_above_is_also_occupied`</sub>
- A coil is only shown as a physical device block when its "device_block" attribute is exactly "true"; any other value, including empty, keeps the distributed placement. <sub>`device_block_attribute_selects_the_display_mode`</sub>
- The device block's contact image lists each contact with its terminal pair and drawing address, in the same order as the contact map. <sub>`device_block_lists_the_same_contacts_as_the_contact_map`</sub>
- A blank part number produces a device block with no part number line, rather than an empty one. <sub>`device_block_has_no_part_number_line_when_blank`</sub>
- Only the coils whose instance has "device_block" set are collected into the sheet's device blocks; a coil left in the default distributed placement is not. <sub>`sheet_device_blocks_only_includes_flagged_coils`</sub>
- A coil shown as a device block no longer gets the separate hanging contact map, because its contacts are already listed inside the block. <sub>`device_block_coils_are_excluded_from_the_separate_contact_map`</sub>
- The device block's outline is at least as wide as the coil's pin span, so its A1/A2 pins land exactly on the outline's left and right edge and no lead line is needed. <sub>`device_block_layout_is_no_narrower_than_the_pin_span`</sub>
- A part number adds a second header row, and pins still reach the outline through a lead when the content widens the box past the pin span. <sub>`device_block_layout_grows_for_the_part_number_and_keeps_the_pins_reachable`</sub>
- The device block's contact rows sit directly under the header rows, and the outline's total height covers every row with no gap. <sub>`device_block_layout_stacks_contact_rows_under_the_header`</sub>

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
- The panel placement table can be written as CSV, one row per footprint with its item number, position and rail. <sub>`the_panel_placement_table_is_available_as_csv`</sub>
- The panel placement table also comes as a framed A4 drawing sheet, so it can be bound with the drawings. <sub>`the_panel_placement_table_is_available_as_a_drawing_sheet`</sub>
- A project with no panel sheet still produces the placement table, with the column headings and no rows. <sub>`the_panel_placement_table_of_a_project_without_panels_is_empty`</sub>

### Reports (BOM / wire list)

- The BOM groups symbols by part number and counts quantities per group. <sub>`bom_groups_by_value_and_counts`</sub>
- The parts list carries the panel item number as its first column, so the balloons on the panel drawing and the parts list line up. <sub>`bom_starts_with_the_panel_item_number`</sub>
- Parts that are not on a panel yet leave the item number column empty; the rest of the parts list is unaffected. <sub>`bom_leaves_the_item_number_empty_for_unplaced_parts`</sub>
- BOM fields containing commas are quoted so the CSV stays valid. <sub>`bom_escapes_fields_with_commas`</sub>
- The wire list is a From-To list: every row starts with the sheet name and the two ends of the wire, and the older column order (sheet, wire number, harness, ...) is no longer produced. <sub>`wire_list_is_a_from_to_list`</sub>
- An end of a wire that lands on a symbol pin is written as "reference:pin number" (for example K1:A1). <sub>`a_wire_end_on_a_pin_is_written_as_reference_and_pin_number`</sub>
- A wire end on a block pin is written with the pin's name (for example CTRL1:OUT1), and the block itself is counted in the parts list. <sub>`a_wire_end_on_a_block_pin_uses_the_pin_name_and_the_block_is_in_the_parts_list`</sub>
- An end of a wire that carries a net label is written as the label name. <sub>`a_wire_end_with_a_net_label_is_written_as_the_label_name`</sub>
- An end of a wire that touches nothing is left empty in the list. <sub>`an_unconnected_wire_end_is_empty`</sub>
- Of the two ends, the one whose text sorts first alphabetically becomes From, so the same wire always yields the same From and To no matter which end was drawn first. <sub>`from_is_the_alphabetically_smaller_of_the_two_ends`</sub>
- When only one end is connected, that end is always From and the empty one is always To. <sub>`a_connected_end_becomes_from_and_the_unconnected_end_becomes_to`</sub>
- Both sides of a feed-through terminal share one terminal number, so wires on the inside and the outside of the same terminal are both written as "TB1:1". <sub>`both_sides_of_a_feed_through_terminal_use_the_same_terminal_number`</sub>
- The wire list keeps the wire-number column introduced with wire numbering: it holds the number written on the wire, or is empty when the wire is not numbered yet. <sub>`wire_list_has_a_wire_number_column`</sub>
- The wire list keeps the harness column: it carries the name of the harness boundary that fully encloses the wire, and stays empty for wires outside every harness. <sub>`wire_list_has_a_harness_column`</sub>
- The wire list contains each wire's color, gauge, length and part number. <sub>`wire_list_contains_attributes`</sub>
- The part-number column of the wire list is filled from the project's wire-part table (colour + gauge), so wires never have to be given a part number one by one. <sub>`wire_list_part_number_comes_from_the_wire_part_table`</sub>
- A combination that is not in the wire-part table leaves the part-number column empty. <sub>`wire_list_part_number_is_empty_when_the_table_has_no_match`</sub>
- A part number written on the wire itself is kept in the list even when the table says otherwise. <sub>`wire_list_keeps_a_part_number_written_on_the_wire`</sub>
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
- A multi-level terminal block counts its poles, not its terminals: a 4-pole 2-level block is 4 poles over 2 levels, and its terminal row spans the first to the last terminal number (1a-4b). <sub>`multilevel_terminal_block_device_counts_poles_not_terminals`</sub>
- An ordinary part that has no separable functions shows up as a single "body" row, named after its symbol. <sub>`plain_part_is_a_single_body_row`</sub>
- Devices are keyed by reference designator, so the same designator on several sheets is one device, and symbols without a designator are not devices at all. <sub>`devices_group_by_reference_across_sheets`</sub>
- An empty project has no devices, and the tree is ordered by reference designator. <sub>`device_tree_is_ordered_and_can_be_empty`</sub>
- Every function row knows the sheet and zone to jump to, which is what the navigator and the reference surfer use to reveal it. <sub>`every_function_knows_where_to_jump`</sub>
- Filter names travel over the API as snake_case strings, and unknown names are rejected rather than guessed. <sub>`filter_names_parse_from_the_api`</sub>

### shape

- A new shape starts in the default colour, with a hairline stroke, no fill and a solid line, so a rectangle drawn without touching any property looks exactly like the frame lines. <sub>`a_new_shape_starts_plain`</sub>
- A rectangle is two diagonal corners, and it normalises to the same rectangle whichever way it was dragged. <sub>`a_rectangle_is_two_diagonal_corners`</sub>
- A circle is a centre point plus a point on its edge, so its radius is the distance between the two. <sub>`a_circle_is_a_centre_and_a_point_on_its_edge`</sub>
- A polyline keeps every point it was drawn with, in order. <sub>`a_polyline_keeps_every_point`</sub>
- A rectangle, line or circle with anything other than two points is rejected, and a polyline needs at least two. <sub>`the_number_of_points_must_match_the_shape_kind`</sub>
- A shape colour is a named token, never a raw colour code, so a shape can never print in a colour the paper cannot show. <sub>`a_shape_colour_must_be_a_token`</sub>
- The stroke width has to stay between the thinnest line A3 can print and the thickest line that does not swamp a 2.5 mm grid. <sub>`the_stroke_width_has_a_printable_range`</sub>
- A coordinate that is not a real number is rejected, so a broken shape can never make the drawing unpaintable. <sub>`a_non_finite_coordinate_is_rejected`</sub>
- Shapes print in the colour their token stands for, and a shape with no fill is not painted at all. <sub>`shapes_print_in_their_token_colour`</sub>
- Shapes carry no electrical meaning: adding them changes neither the netlist nor the ERC result, even when a shape lies exactly on top of a wire. <sub>`shapes_are_invisible_to_the_netlist_and_the_checks`</sub>
- Shapes never appear in the parts list or the wire list, and a wire inside a shape keeps an empty harness column — which is why an enclosure is drawn with a shape and not with a harness boundary (G11). <sub>`shapes_never_reach_the_parts_list_or_the_wire_list`</sub>
- Shapes are left out of the tidy metrics, so drawing an annotation off the grid never counts against the drawing. <sub>`shapes_are_left_out_of_the_tidy_metrics`</sub>
- Listing the shapes of a sheet returns only shapes, never wires, notes or harness boundaries. <sub>`only_shapes_are_listed`</sub>

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
- A block symbol is drawn with its outline, its pin numbers and its pin names, taken from the instance attributes. <sub>`svg_draws_block_outline_with_pin_numbers_and_names`</sub>
- Drawing the jis-a3 frame template produces exactly the JIS frame this program has always printed, down to the last character of the SVG (frame, zone ticks, revision table and every title-block cell). <sub>`the_jis_a3_template_reproduces_the_previous_frame_exactly`</sub>
- An empty A4 sheet with no revisions also prints exactly the frame it always did: no revision table, and the title-block cells left blank rather than dropped. <sub>`an_empty_sheet_reproduces_the_previous_frame_exactly`</sub>
- The A4 portrait template prints a 190x277 frame with its 110 mm title block in the bottom-right corner, and the same 4x6 zone ticks. <sub>`the_a4_portrait_template_draws_a_narrower_frame_and_title_block`</sub>
- The frame template is what the drawing follows: switching the project to A4 portrait changes every sheet's frame, and it goes back on undo. <sub>`switching_the_project_frame_template_changes_the_output`</sub>
- A project pointing at a frame template nobody knows still prints, using the default JIS A3 frame. <sub>`an_unknown_frame_template_still_prints_with_the_default_frame`</sub>
- Title-block cells print the resolved binds: the sheet number out of the total, the project name and the latest revision all land in the block. <sub>`title_block_cells_print_resolved_binds`</sub>
- A frame template with a logo frame draws an empty rectangle where the logo goes, so the space is reserved even before an image is set. <sub>`a_logo_frame_is_drawn_as_an_empty_rectangle`</sub>
- Special characters in titles (<, >, &, quotes) are XML-escaped in the SVG. <sub>`svg_escapes_xml_special_chars`</sub>
- Two revisions draw a table right above the title block, oldest at the bottom and newest on top, with a column header row. <sub>`svg_draws_revision_table_above_title_block`</sub>
- A sheet with no revisions draws no revision table at all, not even an empty frame. <sub>`svg_omits_revision_table_when_no_revisions`</sub>
- The Rev field of the title block shows the mark of the newest revision, and falls back to the stored value when there are no revisions. <sub>`svg_title_block_rev_follows_latest_revision`</sub>
- With seven revisions only the newest six rows are drawn; the oldest row is dropped from the drawing while the data keeps it. <sub>`svg_revision_table_shows_only_newest_six_rows`</sub>
- A note with no styling is exported exactly as notes have always been exported: left-aligned, black, normal weight. <sub>`an_unstyled_note_is_exported_exactly_as_before`</sub>
- Choosing the default colour token explicitly changes nothing in the exported drawing. <sub>`the_default_colour_token_changes_nothing_in_the_export`</sub>
- A note's anchor, colour token and bold flag all reach the exported drawing. <sub>`a_styled_note_is_exported_with_its_anchor_colour_and_weight`</sub>
- A symbol's function label (its third text field) is printed under the symbol outline, centred, alongside the designator and the type number above it. <sub>`the_function_label_is_printed_under_the_symbol`</sub>
- A symbol with no function label is drawn exactly as before, with only its designator and type number. <sub>`a_symbol_without_a_function_label_is_drawn_as_before`</sub>
- The wire number of a horizontal wire is printed in a monospaced font 2.5 mm above the middle of the wire. <sub>`svg_draws_wire_number_above_a_horizontal_wire`</sub>
- The wire number of a vertical wire is printed 2.5 mm to the left of the middle of the wire. <sub>`svg_draws_wire_number_left_of_a_vertical_wire`</sub>
- One net is labelled once, at the middle of its longest segment, however many wires it is drawn with. <sub>`svg_draws_the_wire_number_once_on_the_longest_segment`</sub>
- A net without a wire number gets no number text at all. <sub>`svg_omits_wire_number_for_unnumbered_nets`</sub>
- A harness boundary is drawn as a dashed rectangle (IEC 61082-1 group enclosure) around the wires it holds. <sub>`svg_draws_a_harness_as_a_dashed_rectangle`</sub>
- The harness name is printed just outside the top-left corner of the boundary. <sub>`svg_labels_the_harness_at_its_top_left_corner`</sub>
- A harness with no name draws only its dashed boundary, without a label. <sub>`svg_omits_the_label_of_an_unnamed_harness`</sub>
- A rectangle shape is drawn as a plain rectangle at the two corners it was given. <sub>`svg_draws_a_rectangle_shape`</sub>
- A line shape is drawn as a straight line between its two points, and a polyline shape follows every point in order. <sub>`svg_draws_line_and_polyline_shapes`</sub>
- A circle shape is drawn at its centre with the radius reaching the point on its edge. <sub>`svg_draws_a_circle_shape`</sub>
- A shape prints in the colour its token stands for, at the stroke width it was given, dashed when it is set to dashed and shaded when it has a fill. <sub>`svg_honours_the_colour_width_dash_and_fill_of_a_shape`</sub>
- Shapes are drawn behind the wires, so a filled highlight box never hides the circuit on top of it. <sub>`svg_draws_shapes_behind_the_wires`</sub>
- Rotated symbols are drawn with their shapes actually rotated (90 deg makes a resistor body vertical). <sub>`svg_renders_rotated_symbol_primitives`</sub>
- A sheet exported as part of its project shows, next to each net label, the "/sheet.zone" address of the same-named label on the other sheet. <sub>`svg_draws_cross_reference_address_next_to_net_label`</sub>
- A net label with no counterpart on another sheet gets no cross-reference text. <sub>`svg_omits_cross_reference_when_there_is_no_counterpart`</sub>
- Exporting a single sheet on its own (no project context) never draws cross-references. <sub>`svg_of_a_lone_sheet_has_no_cross_reference`</sub>
- The sheet holding the coil shows a contact map under it: one row per contact with its terminal pair and its address. <sub>`svg_draws_the_contact_map_under_the_coil`</sub>
- The sheet holding the contact shows the coil's address in parentheses beside it. <sub>`svg_draws_the_coil_location_beside_the_contact`</sub>
- Exporting a single sheet on its own draws no contact map, because the counterpart sheets are unknown. <sub>`svg_of_a_lone_sheet_has_no_contact_map`</sub>
- A coil whose "device_block" attribute is "true" is drawn as one outline with its designator, part number and contact image, instead of the small coil symbol and the separate hanging contact map. <sub>`svg_draws_a_device_block_for_a_flagged_coil`</sub>
- Toggling "device_block" off keeps the previous distributed placement: the small coil symbol with its own separate contact map underneath. <sub>`svg_keeps_the_distributed_placement_without_the_attribute`</sub>
- A non-relay designator drawn on two sheets gets an automatic "(/2.B3)"-style cross-reference beside it, on both sheets (G13). <sub>`svg_draws_a_device_cross_reference_for_a_designator_on_two_sheets`</sub>
- A panel-layout sheet is exported with the same drawing frame as a schematic, plus the enclosure and its mounting plate. <sub>`svg_of_a_panel_sheet_draws_the_enclosure_and_its_mounting_plate`</sub>
- A duct is drawn as a rectangle filled with 45-degree hatch lines. <sub>`svg_of_a_panel_sheet_hatches_the_duct`</sub>
- A DIN rail is drawn as a horizontal band centred on its mounting line. <sub>`svg_of_a_panel_sheet_draws_the_din_rail`</sub>
- A footprint is drawn as its outline rectangle with the reference designator and the type number inside. <sub>`svg_of_a_panel_sheet_draws_the_footprint_with_its_reference`</sub>
- A footprint turned 90 degrees is drawn lying down (its width and height swap). <sub>`svg_draws_a_rotated_footprint_lying_down`</sub>
- A numbered footprint carries a balloon: a white circle holding the item number, joined to the part by a short leader line. <sub>`svg_draws_a_balloon_on_a_numbered_footprint`</sub>
- A footprint that has not been numbered yet has no balloon on the drawing. <sub>`svg_draws_no_balloon_before_numbering`</sub>
- The legend table is drawn where it was placed, listing the item number, the reference and the type number of every part on the sheet. <sub>`svg_draws_the_legend_table`</sub>
- Every wire prints its gauge, colour and cut length under the wire, which is what the shop needs to make the harness. <sub>`svg_annotates_each_wire_with_gauge_colour_and_length`</sub>
- The wire note is part of the exported drawing itself, so it comes out in SVG and PDF without turning anything on. <sub>`wire_notes_are_exported_by_default`</sub>
- Hiding a class on screen never changes the exported file: the export always draws the whole drawing. <sub>`export_ignores_screen_only_view_classes`</sub>
- A wire with no gauge, colour or length is drawn without any note next to it. <sub>`svg_omits_the_note_of_a_bare_wire`</sub>
- A net label tagged "right" is drawn as a flag outline pointing right, with the net name inside the body that runs to the left of the label position. <sub>`svg_draws_a_directional_flag_around_a_tagged_net_label`</sub>
- A sheet with a smaller annotation text height exports smaller wire numbers, wire notes and net tags, and nothing else changes size. <sub>`svg_follows_the_sheet_annotation_text_height`</sub>
- A net label with no tag keeps the plain text look, with no outline drawn around it. <sub>`svg_draws_no_outline_for_an_untagged_net_label`</sub>
- An unknown tag value is ignored and the label falls back to plain text rather than breaking the export. <sub>`svg_ignores_an_unknown_tag_value`</sub>
- A jumper bar is always drawn clear of every terminal number, whatever jumpers the block carries and however it is turned or mirrored, so the numbers stay readable. <sub>`a_jumper_bar_never_covers_a_terminal_number`</sub>
- A jumper between two adjacent terminals is drawn on the schematic as one short thick bar filling the gap between their two round marks, and nothing else is drawn around it. <sub>`svg_draws_a_jumper_bar_between_adjacent_terminals`</sub>
- On a multi-level terminal block, a jumper written with level letters ("1a-1b") is drawn on that pole's row as a bar in the gap between the two level columns' marks. <sub>`svg_draws_a_jumper_bar_across_the_levels_of_a_multilevel_block`</sub>
- A jumper bar turns with its terminal block, so on a block rotated a quarter turn the bar lies between the rotated terminals instead of staying upright. <sub>`svg_draws_the_jumper_bar_turned_with_a_rotated_terminal_block`</sub>
- The lettering inside a symbol (a motor's "M", for instance) is drawn at the place the symbol definition gives it and stays there whatever the symbol's rotation or mirroring, because the letters themselves are always written horizontally. <sub>`the_lettering_inside_a_symbol_stays_put_when_the_symbol_is_turned`</sub>
- A terminal number of one or two characters sits wholly inside its own round mark, centred on it, however the block is turned or mirrored, so a rotated terminal block still reads terminal by terminal. <sub>`a_turned_terminal_block_keeps_a_short_number_inside_its_own_mark`</sub>
- In the exported drawing (SVG/PDF) an unturned terminal block writes each terminal number with its baseline exactly half the character height (1.0 mm at the 2.0 mm terminal-number height) below the centre of its own mark, single-level and multi-level blocks alike; the on-screen canvas uses a smaller offset of its own. <sub>`a_terminal_number_sits_half_a_character_below_its_mark_centre`</sub>
- A three-character terminal number (a multi-level block's "10a") is as wide as its own mark, so it is centred on the mark and hangs at most three tenths of a millimetre over it, while still never running into a jumper bar or into a neighbouring terminal's mark. <sub>`a_three_character_terminal_number_hangs_at_most_a_little_over_its_own_mark`</sub>
- A jumper bar is drawn at least twice as thick as an ordinary symbol line, so that the short bar in the narrow gap between two terminal marks still reads as a shorting bar when the sheet is printed. <sub>`the_jumper_bar_is_at_least_twice_as_thick_as_a_symbol_line`</sub>
- A jumper written between two terminals that are not adjacent is invalid (the same rule the terminal chart enforces), so no bar is drawn for it at all. <sub>`svg_draws_no_bar_for_a_jumper_between_non_adjacent_terminals`</sub>
- A terminal block with several jumpers gets one bar per jumper. <sub>`svg_draws_one_bar_per_jumper_when_there_are_several`</sub>
- A terminal block with no jumpers draws no bar, and a symbol that is not a terminal block never gets one even if it happened to carry a "jumpers" attribute. <sub>`svg_draws_no_bar_without_jumpers_or_on_a_non_terminal_block`</sub>

### Symbol library

- Every bundled symbol has a unique id and at least one pin. <sub>`builtin_symbols_have_unique_ids_and_pins`</sub>
- A parametric terminal block (e.g. 8 poles) has one left and one right connection point per terminal, all on the 2.5 mm grid and vertically centered. <sub>`dynamic_terminal_block_has_through_pins_on_grid`</sub>
- A multi-level terminal block (8 poles, 2 levels) numbers its pins with a level letter (1a, 1b, 2a, ... 8b), one pin number per terminal and two connection points each, like a single-level block. <sub>`dynamic_multilevel_terminal_block_numbers_pins_with_level_letters`</sub>
- A multi-level terminal block is drawn as side-by-side terminal columns, one column per level, so a 3-level block has three columns of terminals evenly spaced left to right. <sub>`a_multilevel_terminal_block_draws_one_terminal_column_per_level`</sub>
- Every terminal of the same pole sits on one row across all levels, so 1a and 1b share a row and the rows keep the 5 mm pitch of a single-level block. <sub>`every_level_of_the_same_pole_sits_on_one_row`</sub>
- Every connection point of a multi-level terminal block sits on the 2.5 mm grid, and no two terminals share a connection point, so a wire can always reach each terminal on its own. <sub>`multilevel_terminal_block_pins_are_on_the_grid_and_never_coincide`</sub>
- A terminal block asking for 0 or 4 levels (outside the 1-3 range) is rejected as an unknown symbol, the same way an out-of-range pole count is. <sub>`multilevel_terminal_block_rejects_level_count_outside_one_to_three`</sub>
- The single-level terminal block id keeps its plain numeric pin numbers (1, 2, ...) exactly as before, unaffected by the new multi-level id shape, so existing projects stay valid. <sub>`single_level_terminal_block_id_is_unaffected_by_multilevel_support`</sub>
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

### User symbol library

- A folder with no symbol files yields an empty user library that still reports where the user should drop their JSON files. <sub>`an_empty_symbols_folder_lists_nothing_but_its_path`</sub>
- A saved user symbol is written as one JSON file named after its id and comes back from the listing unchanged (shapes, pins and attribute slots). <sub>`a_saved_user_symbol_round_trips_through_the_folder`</sub>
- Whatever category the file declares, a user symbol is always listed under the single "user" category so it appears as its own group in the insert dialog. <sub>`a_user_symbol_is_always_filed_under_the_user_category`</sub>
- The insert list is the bundled library first and the user symbols after it, so the user's own group never breaks the bundled category order. <sub>`the_insert_list_puts_user_symbols_after_the_bundled_ones`</sub>
- A file that is not valid JSON does not stop the listing: the rest of the library still loads and the broken file is reported with its path and the reason. <sub>`a_broken_json_file_is_reported_with_its_path_and_reason`</sub>
- A file whose content is valid JSON but not a usable symbol (here: a pin off the 2.5 mm grid) is reported as an issue instead of being listed. <sub>`a_symbol_file_that_fails_validation_is_reported_instead_of_listed`</sub>
- A user symbol may not take the id of a bundled symbol: otherwise dropping a file would silently change every drawing that already uses that symbol. <sub>`a_user_symbol_may_not_take_the_id_of_a_bundled_symbol`</sub>
- A user symbol may not take the id of a parametric symbol either (for example terminal_block_8p), because that name is generated on demand. <sub>`a_user_symbol_may_not_take_a_parametric_symbol_id`</sub>
- Two files declaring the same id keep the first one (file name order) and report the second, so which symbol wins never depends on the order the folder is read in. <sub>`two_files_with_the_same_id_keep_the_first_and_report_the_second`</sub>
- Saving reports every problem at once and writes no file at all, so a half-fixed symbol never lands in the library. <sub>`saving_an_invalid_symbol_reports_every_problem_and_writes_nothing`</sub>
- Connection points must sit on the 2.5 mm grid, so wires drawn on the grid always meet them exactly. <sub>`pins_must_sit_on_the_grid`</sub>
- The same pin number at two different points is allowed: that is how a through terminal (internally shorted left and right side) is drawn. <sub>`the_same_pin_number_at_two_points_is_a_through_terminal`</sub>
- Two connection points with the same number at the same place are rejected: the second one can never be reached. <sub>`the_same_pin_number_at_the_same_point_is_rejected`</sub>
- Two connection points with different numbers at the same place are rejected: a wire ending there would not know which terminal it touches. <sub>`two_different_pin_numbers_at_the_same_point_are_rejected`</sub>
- A connection point needs a number (it is what the netlist and the terminal chart print), but its name is optional. <sub>`a_pin_needs_a_number_but_not_a_name`</sub>
- A symbol must carry the reference-designator slot (TAG): without it the placed part could never show its K1 / PB2 label. <sub>`a_symbol_without_a_tag_slot_is_rejected`</sub>
- An attribute slot key the renderer does not know (a typo, say) is rejected instead of silently drawing nothing. <sub>`an_unknown_attribute_slot_key_is_rejected`</sub>
- A symbol needs at least one shape and at least one connection point, otherwise there is nothing to see and nothing to wire. <sub>`a_symbol_needs_at_least_one_shape_and_one_pin`</sub>
- The id is used as the file name, so only letters, digits, underscore and hyphen are allowed. <sub>`an_id_with_unusable_characters_is_rejected`</sub>
- A symbol named only in Japanese is accepted and the Japanese name is copied into the English field, so it still shows up in an English UI. <sub>`a_japanese_only_name_fills_both_name_fields`</sub>
- Duplicating a bundled symbol gives a draft with a new free id, a "copy" name and the user category, and writes nothing to disk yet. <sub>`duplicating_a_bundled_symbol_gives_a_free_id_and_a_copy_name`</sub>
- Duplicating the same symbol twice never reuses an id: the second copy is numbered. <sub>`duplicating_twice_never_reuses_an_id`</sub>
- A user symbol can be duplicated too (that is how a family of variants is built), and a parametric symbol can be the starting point as well. <sub>`a_user_symbol_and_a_parametric_symbol_can_both_be_duplicated`</sub>
- Duplicating a symbol that does not exist fails instead of creating an empty one. <sub>`duplicating_an_unknown_symbol_fails`</sub>
- Deleting a user symbol removes its file from the folder; deleting an unknown id fails and touches nothing. <sub>`deleting_a_user_symbol_removes_its_file`</sub>
- The usage count tells how many placements of a symbol the project has, across every sheet, so the UI can warn before a symbol is deleted. <sub>`the_usage_count_counts_placements_across_every_sheet`</sub>

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
- The terminal chart CSV has the columns terminal, level, inside, wire number, wire, outside, jumper, and one line per terminal. <sub>`the_terminal_chart_csv_has_the_designed_columns`</sub>
- A multi-level terminal block shows its level letter in the chart's Lv column, and its terminal column carries the full pin number ("1a"), so the two levels never look like the same terminal. <sub>`a_multilevel_terminal_block_shows_its_level_letter_in_the_lv_column`</sub>
- On a multi-level terminal block a jumper is written with the level letter, so "1a-1b" links the two levels of the same pole and is accepted. <sub>`a_jumper_can_link_the_two_levels_of_one_pole`</sub>
- On a multi-level terminal block a jumper between neighbouring poles of the same level ("1a-2a") is accepted just like on a single-level block. <sub>`a_jumper_can_link_neighbouring_poles_of_one_level`</sub>
- A jumper written without the level letter ("1-2") on a multi-level terminal block is refused, because it does not say which level it is on. <sub>`a_jumper_without_a_level_letter_is_refused_on_a_multilevel_block`</sub>
- A jumper pointing at a level the multi-level terminal block does not have ("1a-1c" on a 2-level block) is refused as a terminal that does not exist. <sub>`a_jumper_to_a_level_the_block_does_not_have_is_refused`</sub>
- A diagonal jumper ("1a-2b" — a different pole and a different level at once) is refused, because those two terminals are not next to each other. <sub>`a_diagonal_jumper_across_a_pole_and_a_level_is_refused`</sub>
- The jumper column of a multi-level chart shows a jumper only on the two rows it actually links, never on the other levels of the same poles. <sub>`a_multilevel_jumper_is_shown_only_on_the_rows_it_links`</sub>
- The terminal block check reports a jumper written without its level letter on a multi-level block as an error, so the drawing and the report can never disagree about it. <sub>`the_check_reports_a_jumper_without_a_level_letter_as_an_error`</sub>
- Asking for the chart of something that is not a terminal block gives nothing. <sub>`only_terminal_blocks_have_a_chart`</sub>
- The terminal block check reports every unwired terminal as information, so spares are visible without being treated as mistakes. <sub>`the_check_reports_unwired_terminals_as_information`</sub>
- The terminal block check reports a jumper between non-adjacent terminals as an error. <sub>`the_check_reports_an_invalid_jumper_as_an_error`</sub>
- The terminal block check reports a jumper to a terminal that does not exist as an error. <sub>`the_check_reports_a_jumper_to_a_missing_terminal_as_an_error`</sub>
- A terminal block with every terminal wired and correct jumpers passes the check with nothing to report. <sub>`a_fully_wired_terminal_block_passes_the_check`</sub>
- The editor lists every terminal block of the project with its sheet, its pole count and its jumpers, sheet by sheet and in reference-designator order. <sub>`the_editor_lists_every_terminal_block_with_its_sheet_poles_and_jumpers`</sub>
- The summary of a multi-level terminal block tells its pole count and its level count apart from its terminal count: a 4-pole 2-level block has 4 poles, 2 levels and 8 terminals. <sub>`a_multilevel_terminal_block_summary_separates_poles_levels_and_terminals`</sub>
- A single-level terminal block reports one level, so its pole count and its terminal count are the same number. <sub>`a_single_level_terminal_block_summary_has_one_level`</sub>
- Naming a sheet narrows the list to the terminal blocks drawn on that sheet. <sub>`naming_a_sheet_narrows_the_terminal_block_list_to_that_sheet`</sub>
- Symbols that are not terminal blocks never show up in the list. <sub>`other_symbols_never_show_up_in_the_terminal_block_list`</sub>
- The chart and the check of a terminal block can be looked up by entity id alone, without knowing which sheet it sits on. <sub>`a_terminal_block_can_be_looked_up_by_id_across_sheets`</sub>
- The jumper bar between the two levels of one pole lies on that pole's row, in the gap between the two level columns' marks. <sub>`a_cross_level_jumper_bar_lies_on_the_row_between_the_two_marks`</sub>
- Each end of a jumper bar sits exactly on the edge of its own terminal's round mark, and every other terminal of the block is farther away, so a jumper always reads as joining the right pair. <sub>`each_jumper_bar_end_sits_on_the_edge_of_its_own_terminal_mark`</sub>
- A jumper bar is exactly as long as the gap between the two terminal marks it joins, that is the terminals' own spacing less one mark radius at each end. <sub>`a_jumper_bar_is_exactly_as_long_as_the_gap_between_the_two_marks`</sub>
- A jumper between two terminals of one column is drawn entirely in the gap between their two marks, so it never runs over a wire connection point of the terminal block. <sub>`a_column_jumper_stays_in_the_gap_between_the_two_marks`</sub>
- Several jumpers on the same terminal block each produce their own bar segment. <sub>`multiple_jumpers_each_produce_their_own_bar_segment`</sub>
- A jumper referencing a terminal number the block does not have produces no segment instead of panicking. <sub>`a_jumper_to_a_missing_terminal_produces_no_bar_segment`</sub>
- Jumpers are set with the ordinary update_entity command, so the chart follows the change and undo takes it back. <sub>`setting_jumpers_through_update_entity_is_undoable`</sub>

### Terminal connection diagrams

- The terminal strip is drawn as one numbered box per terminal, stacked from top to bottom in terminal order. <sub>`the_strip_stacks_one_numbered_box_per_terminal`</sub>
- The outside of the panel is drawn to the left of the strip and the inside to the right, each under its own caption. <sub>`the_outside_is_on_the_left_and_the_inside_on_the_right`</sub>
- A terminal with nothing wired to it stays in the strip as a lightly filled box marked as a spare. <sub>`a_spare_terminal_stays_in_the_strip_lightly_filled`</sub>
- Wires of the same harness are gathered into one bracket at the outer end of their lead lines, labelled with the harness name. <sub>`wires_of_one_harness_are_gathered_into_a_bracket`</sub>
- A wire that belongs to no harness gets no bracket at all. <sub>`a_wire_without_a_harness_gets_no_bracket`</sub>
- A saddle jumper between neighbouring terminals is drawn as a vertical link on the inside edge of the terminal boxes. <sub>`a_jumper_is_drawn_on_the_inside_edge_of_the_boxes`</sub>
- Each lead line carries the wire it stands for: colour, gauge in sq and part number, written small under the line. <sub>`each_lead_line_carries_the_wire_specification`</sub>
- A multi-level terminal block's diagram labels each box with its full level-letter terminal number (1a, 1b, 2a, 2b, ...), so the two levels are never drawn as if they were the same terminal. <sub>`a_multilevel_terminal_block_labels_boxes_with_the_level_letter`</sub>
- A multi-level terminal block's diagram shows the levels as columns: the terminal strip is split into one column per level, level a on the left, and each terminal's box sits in its own level's column. <sub>`a_multilevel_terminal_diagram_draws_each_level_as_a_column`</sub>
- Each level column of a multi-level terminal diagram is headed by its level letter, so a reader can tell which column is which level without reading the box numbers. <sub>`a_multilevel_terminal_diagram_heads_each_level_column_with_its_letter`</sub>
- A single-level terminal diagram gets no level heading, because it has only one column of terminals. <sub>`a_single_level_terminal_diagram_has_no_level_heading`</sub>
- Terminals that do not fit on one page continue on the next, and each page title states the range of terminals it holds. <sub>`terminals_that_do_not_fit_continue_on_the_next_page`</sub>
- Every terminal block gets exactly one page; anything that is not a terminal block gets none. <sub>`one_page_per_terminal_block_and_none_for_anything_else`</sub>
- A terminal diagram page is an A4 landscape sheet with the JIS frame and a title block naming the terminal block. <sub>`the_page_has_the_frame_and_a_title_block`</sub>
- The terminal diagram is offered as a report named terminal-diagram, spelled the same way in the CLI, Link API and MCP. <sub>`the_terminal_diagram_is_a_report_named_terminal_diagram`</sub>
- A project without any terminal block still yields one page, so the report is never empty. <sub>`a_project_without_terminal_blocks_still_yields_one_page`</sub>

### text_style

- Every note colour is a named token with a print colour, and the default token prints black. <sub>`every_colour_token_has_a_print_colour`</sub>
- A raw colour code is not a colour token, so it can never sneak into a drawing. <sub>`a_raw_colour_code_is_not_a_colour_token`</sub>
- A note with no colour is drawn in the default colour, and a note with a token is drawn in that token's print colour. <sub>`a_note_without_a_colour_uses_the_default_colour`</sub>
- The three anchors map to the three SVG text-anchor values, and the default anchor is the one drawings have always used. <sub>`anchors_map_to_svg_text_anchor`</sub>
- A symbol with no function label attribute has nothing extra to draw. <sub>`a_symbol_without_a_function_label_draws_nothing_extra`</sub>
- A blank function label counts as no label, so a stray space does not print an empty line. <sub>`a_blank_function_label_counts_as_no_label`</sub>
- With a bundled symbol (which defines no function slot) the function label lands just below the symbol outline, centred on the placement point. <sub>`the_function_label_sits_below_a_symbol_that_defines_no_slot`</sub>
- When the symbol definition does place a FUNCTION slot, the label follows the slot (rotation and all) instead of the automatic position. <sub>`the_function_label_follows_a_slot_defined_by_the_symbol`</sub>

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
- The measured text of a tagged net label sits inside its flag, so the overlap count follows what is actually printed. <sub>`a_tagged_net_label_is_measured_inside_its_flag`</sub>
- A smaller annotation text height makes the measured net label and wire number smaller too. <sub>`measured_annotation_text_follows_the_sheet_setting`</sub>
- Symbol origins and wire vertices sitting on the 2.5 mm grid report no off-grid points. <sub>`entities_on_the_grid_report_no_off_grid_points`</sub>
- A point shifted by 0.1 mm off the grid is counted, and each stray vertex counts once. <sub>`a_point_shifted_a_tenth_of_a_millimetre_is_off_grid`</sub>
- An empty sheet is perfectly tidy: every metric is zero. <sub>`an_empty_sheet_scores_zero_on_every_metric`</sub>
- tidy_metrics gathers the four counts of one sheet in a single value. <sub>`tidy_metrics_gathers_the_four_counts`</sub>
- Measuring the same sheet twice gives exactly the same numbers, so the agent can compare before and after. <sub>`measuring_the_same_sheet_twice_gives_the_same_numbers`</sub>

### Verification (ERC & electrical)

- A fully wired circuit produces no ERC findings. <sub>`fully_wired_pair_has_no_erc_findings`</sub>
- Unconnected pins are reported as one warning per symbol, listing the affected pin numbers. <sub>`unconnected_pins_are_reported_per_symbol`</sub>
- Unconnected pins of a block are listed by their pin names, so a controller's dangling input is named, not numbered. <sub>`unconnected_block_pins_are_listed_by_pin_name`</sub>
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
- A net label that sits on a wire vertex, in the middle of a wire, or on a symbol pin is properly attached and is not reported. <sub>`a_net_label_on_a_wire_or_a_pin_is_not_reported`</sub>
- A net label placed even half a millimetre off a wire is reported as a warning naming the label, because it silently fails to join the net. <sub>`a_net_label_that_floats_off_the_wire_is_reported`</sub>
- A net label within the 0.01 mm connection tolerance still counts as attached, so ordinary rounding never triggers the warning. <sub>`a_net_label_within_the_connection_tolerance_is_still_attached`</sub>
- A single-sheet label conflict is reported exactly once when the whole project is verified. <sub>`project_verification_reports_a_single_sheet_label_conflict_once`</sub>
- Nets joined across sheets by a shared label name are checked as one net, so a conflict spanning two sheets is reported once with all offending labels. <sub>`project_verification_merges_label_conflicts_across_sheets`</sub>
- A net continued onto another sheet with the same label name is not a conflict. <sub>`project_verification_accepts_a_net_continued_onto_another_sheet`</sub>
- A multi-pole breaker conducts pole by pole, so a load fed from an unwired pole is still reported as unreachable from the source. <sub>`multipole_breaker_does_not_connect_its_poles_to_each_other`</sub>
- A panel-layout sheet is not checked against the schematic rules: it carries no wiring, so nothing there counts as an unconnected pin or a dangling wire. <sub>`a_panel_sheet_is_not_checked_against_the_schematic_rules`</sub>
- A part placed on the panel whose reference designator is nowhere on the schematic is reported as a warning, and the whole-project check reports it together with the schematic findings. <sub>`an_orphan_footprint_is_reported_by_the_project_check`</sub>

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

### wire_note

- A wire that has a gauge, a colour and a cut length is annotated "0.75sq 白 0.4m" on the drawing. <sub>`note_shows_gauge_colour_and_length`</sub>
- A missing attribute is simply left out of the note, so a wire without a length reads "0.75sq 白". <sub>`note_omits_missing_attributes`</sub>
- A wire with no gauge, no colour and no length gets no note at all instead of an empty one. <sub>`wire_without_attributes_has_no_note`</sub>
- A colour name the drawing does not know is printed as it was typed, so nothing is lost. <sub>`unknown_colour_names_are_printed_unchanged`</sub>
- The note of a horizontal wire sits below the wire, on the opposite side from its wire number. <sub>`horizontal_wire_note_sits_below_the_wire`</sub>
- The note of a vertical wire sits to the right of the wire, on the opposite side from its wire number. <sub>`vertical_wire_note_sits_right_of_the_wire`</sub>
- Every wire gets its own note, unlike the wire number which is drawn once per net. <sub>`each_wire_gets_its_own_note`</sub>
- A sheet that has no annotation text height set keeps the 2.0mm notes it has always had. <sub>`a_sheet_without_a_setting_keeps_the_default_note_height`</sub>
- Setting the sheet's annotation text height to 1.4mm draws the wire notes at exactly that height. <sub>`the_sheet_setting_is_the_wire_note_height`</sub>
- Smaller annotation text also moves the note closer to its wire, so a row of wires 5mm apart stays readable. <sub>`smaller_annotation_text_moves_the_note_closer_to_the_wire`</sub>
- On a bent wire the note is placed on the longest straight run, where there is room to read it. <sub>`note_is_placed_on_the_longest_straight_run`</sub>

### Wire part numbers (colour + gauge table)

- A wire's part number is looked up in the project's wire-part table by its colour and gauge. <sub>`a_wire_takes_its_part_number_from_the_colour_and_gauge_table`</sub>
- A colour and gauge combination that is not in the table leaves the part number empty; no number is invented. <sub>`an_unregistered_combination_stays_empty`</sub>
- A part number written on the wire itself wins over the table, so a hand-picked part is never overwritten. <sub>`a_part_number_written_on_the_wire_wins_over_the_table`</sub>
- The lookup ignores surrounding spaces and upper/lower case in the colour name. <sub>`the_lookup_ignores_spaces_and_letter_case_in_the_colour`</sub>
- The English colour name on a wire and the Japanese colour name in the table mean the same wire, so a table written in Japanese serves a drawing drawn with English colour tokens. <sub>`the_english_and_japanese_names_of_a_colour_mean_the_same_wire`</sub>
- A wire with no colour or no gauge is not looked up at all. <sub>`a_wire_without_a_colour_or_a_gauge_is_not_looked_up`</sub>
- When the table has two rows for the same colour and gauge, the row written first wins. <sub>`the_first_row_wins_when_the_table_has_duplicates`</sub>
- The wire-part table round trips through CSV: colour, gauge, part number and note come back unchanged. <sub>`the_wire_part_table_round_trips_through_csv`</sub>
- A CSV without a header row is read as data, and the note column may be left out. <sub>`a_csv_without_a_header_row_and_without_the_note_column_is_read`</sub>
- A malformed CSV (wrong number of columns, a non-numeric gauge, or an empty part number) is refused and no row is taken. <sub>`a_malformed_csv_is_refused`</sub>
- Importing a CSV replaces the whole table through the command engine, and one undo brings the previous table back. <sub>`importing_a_csv_replaces_the_table_and_undo_restores_it`</sub>
- A CSV that cannot be read leaves the table exactly as it was. <sub>`a_csv_that_cannot_be_read_leaves_the_table_unchanged`</sub>

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
- A smaller annotation text height moves the cross-reference closer to the label, keeping the gap in proportion. <sub>`cross_reference_follows_the_annotation_text_height`</sub>
- The cross-reference table has one row per project-wide net, listing the sheets it spans and the drawing addresses of its labels. <sub>`cross_reference_table_lists_one_row_per_net_with_its_sites`</sub>
- The columns of the cross-reference table are net / wire number / connected pins / sheets / sites. <sub>`cross_reference_table_columns_are_net_wire_pins_sheets_sites`</sub>
- A designator drawn on two different sheets gets a mutual cross-reference: each instance shows the other sheet's address. <sub>`a_designator_on_two_sheets_cross_references_each_other`</sub>
- A designator drawn on a single sheet only has no cross-reference at all. <sub>`a_designator_on_a_single_sheet_has_no_cross_reference`</sub>
- Relay coils and contacts are skipped by the device cross-reference, because relay_xref already annotates their coil location automatically. <sub>`relay_coil_and_contacts_are_skipped_by_the_device_cross_reference`</sub>
- The per-sheet device cross-reference table maps each symbol entity to the text drawn beside it. <sub>`sheet_device_cross_reference_table_maps_symbols_to_text`</sub>
- A designator on three sheets lists every other sheet's address, in sheet order, on one line. <sub>`a_designator_on_three_sheets_lists_every_other_sheet`</sub>
- The device cross-reference text sits to the right of the symbol outline, on the same row as the designator label. <sub>`device_cross_reference_text_sits_right_of_the_outline`</sub>


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

### Frame templates (REST)

- GET /frame-templates lists the bundled frame templates with their names in both languages and the folder where the user can drop their own. <sub>`the_link_api_lists_the_bundled_frame_templates`</sub>
- The listed frame template carries the whole frame definition (paper, margin, zones, title-block cells and revision columns), so a client can preview it without reading any file. <sub>`a_listed_frame_template_carries_its_whole_definition`</sub>
- POST /commands with set_frame_template switches the whole project's frame, and one undo puts the previous frame back. <sub>`setting_the_frame_template_over_the_link_api_is_one_undo_step`</sub>
- The exported SVG follows the project's frame template: after switching to A4 portrait the title block is drawn 110 mm wide instead of 120 mm. <sub>`the_exported_svg_follows_the_projects_frame_template`</sub>
- A new project starts with the frame template picked as the global default in the app settings. <sub>`a_new_project_starts_with_the_default_frame_template_from_the_settings`</sub>
- Without a global default in the settings a new project leaves the frame unset, so it is drawn with the bundled JIS A3 frame. <sub>`a_new_project_without_a_default_setting_keeps_the_frame_unset`</sub>
- A blank default frame template in the settings is treated as "no default" instead of an unknown id. <sub>`a_blank_default_frame_template_setting_is_ignored`</sub>

### REST Link API

- The REST parts endpoints support searching (sample data included), upserting, category filtering, deleting, and listing wire parts. <sub>`parts_endpoints_search_upsert_delete`</sub>
- GET /api/v1/verify returns the drawing's diagnostics as JSON (e.g. empty reference and unconnected pins). <sub>`verify_returns_diagnostics`</sub>
- POST /api/v1/import/kicad replaces the open project with the converted schematic and returns a patch plus an import report. <sub>`import_kicad_replaces_project_and_reports`</sub>
- POST /api/v1/simulate/op solves the DC operating point and returns net voltages and component currents (requires ngspice; skipped otherwise). <sub>`simulate_op_returns_result`</sub>
- POST /api/v1/export/pdf writes a valid PDF file to the requested path. <sub>`export_pdf_writes_pdf_file`</sub>
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

### User symbol library (REST)

- GET /symbols lists the bundled library and the user's own symbols together, with the user symbols last and always in the "user" category. <sub>`the_link_api_lists_bundled_and_user_symbols_together`</sub>
- GET /symbols/user lists only the user's symbols and says where the folder is, so a client can offer "open the symbols folder". <sub>`the_link_api_lists_the_user_symbols_with_their_folder`</sub>
- A user symbol saved over the Link API is immediately placeable on a sheet and its pins take part in the checks, exactly like a bundled symbol. <sub>`a_saved_user_symbol_can_be_placed_like_a_bundled_one`</sub>
- Editing a user symbol changes the parts already placed in the drawing: the drawing keeps no snapshot of the definition, it only stores the symbol id. <sub>`editing_a_user_symbol_changes_the_parts_already_placed`</sub>
- Saving an invalid symbol answers with every reason at once and writes no file, so the library never holds a half-fixed definition. <sub>`saving_an_invalid_user_symbol_returns_every_reason_and_writes_nothing`</sub>
- A symbol whose id is already taken by a bundled symbol is refused, so dropping a file can never silently change the drawings that use that symbol. <sub>`a_user_symbol_may_not_take_a_bundled_symbol_id`</sub>
- The symbol library lives outside the undo history: undoing after a save takes back the last drawing edit, not the saved symbol. <sub>`the_symbol_library_is_outside_the_undo_history`</sub>
- Deleting a user symbol that parts in the drawing still use is refused (and says how many), unless the caller insists with force. <sub>`deleting_a_user_symbol_in_use_is_refused_unless_forced`</sub>
- An unused user symbol is deleted right away. <sub>`deleting_an_unused_user_symbol_succeeds`</sub>
- Duplicating a symbol returns a draft with a free id and a "copy" name; nothing is written until the draft is saved. <sub>`duplicating_a_symbol_returns_a_draft_that_is_not_saved_yet`</sub>
- Duplicating a symbol that does not exist answers with an error instead of an empty symbol. <sub>`duplicating_an_unknown_symbol_is_an_error`</sub>
- A broken file in the symbols folder never stops the listing: the good symbols still load and the broken file is reported with its path and reason. <sub>`a_broken_file_in_the_symbols_folder_is_reported_by_the_api`</sub>

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
- A multi-pin device with named pins is placed in a single call by passing its pin table in attrs. <sub>`a_block_with_named_pins_is_placed_in_one_call`</sub>
- A block whose pin table is broken comes back as an error instead of a blank box on the sheet. <sub>`a_block_with_a_broken_pin_table_is_refused`</sub>
- Reading tools work through the bridge too, so the agent can look at the drawing before editing. <sub>`reading_tools_work_through_the_bridge`</sub>
- A tool name that does not exist comes back as an error result instead of killing the turn. <sub>`an_unknown_tool_name_comes_back_as_an_error_result`</sub>
- Bad arguments come back as an error result carrying the reason, so the model can correct itself. <sub>`bad_arguments_come_back_with_a_reason`</sub>

### tools

- The parts search tool advertises selection, comparison and alternative-part use, so the agent reaches for it when asked "what can replace this?". <sub>`the_parts_search_tool_advertises_selection_and_comparison`</sub>
- The template tools explain that a template is a starting skeleton, that it lands ERC-clean, and that applying it is a single undo step. <sub>`the_template_tools_explain_what_a_template_is`</sub>
- Every published tool carries a description, so no tool is offered to the agent unexplained. <sub>`every_published_tool_has_a_description`</sub>
- The edit tool tells an agent how to write a jumper on a multi-level terminal block (with the level letter), so it does not send a plain numeric pair that would be refused. <sub>`the_edit_tool_explains_the_multilevel_jumper_form`</sub>

### Wire part numbers / command errors (REST)

- A command the engine refuses comes back as HTTP 400 saying what went wrong and which command in the list failed, instead of a silent 200. <sub>`a_refused_command_returns_400_with_the_reason_and_the_command_index`</sub>
- A parametric symbol asked for beyond its pole limit is refused the same way, with a message that names the usable range. <sub>`a_pole_count_beyond_the_limit_is_refused_over_the_api`</sub>
- A valid command list still returns 200 with one patch per command. <sub>`a_valid_command_list_still_succeeds`</sub>
- Posting a wire-part CSV fills the project's colour+gauge table, and the wire list then prints the part number of a wire that has none of its own. <sub>`importing_a_wire_part_csv_fills_the_part_number_column_of_the_wire_list`</sub>
- A CSV that cannot be read is refused with 400 and the table is left exactly as it was. <sub>`a_malformed_wire_part_csv_is_refused_and_changes_nothing`</sub>


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
- The session id MadakeCAD pinned is the one the CLI reports back, so it is announced only once. <sub>`the_pinned_session_id_is_announced_once`</sub>
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

- The assistant's answer arrives as message deltas and is shown in the chat as it is typed. <sub>`assistant_message_deltas_are_shown_as_they_arrive`</sub>
- The model's private reasoning is not shown in the chat, even though it streams like the answer. <sub>`reasoning_deltas_are_not_shown_in_the_chat`</sub>
- A completed assistant message carries the tool calls the model asked for, with their arguments. <sub>`a_completed_message_carries_the_requested_tool_calls`</sub>
- Starting a tool reports its name and arguments, so the chat can show what the agent is doing. <sub>`a_tool_execution_start_reports_the_tool_and_its_arguments`</sub>
- Finishing a tool closes the matching call and reports whether it succeeded. <sub>`a_tool_execution_complete_closes_the_call_with_its_outcome`</sub>
- The final line ends the turn and reports how many Copilot premium requests it used. <sub>`the_final_line_completes_the_turn_with_the_premium_requests_used`</sub>
- A non-zero exit code in the final line is reported to the chat as an error, not as a normal answer. <sub>`a_failed_final_line_becomes_an_error`</sub>
- The session id the CLI actually used is reported, so the next turn resumes the same conversation. <sub>`the_session_line_reports_the_id_to_resume`</sub>
- Setup and bookkeeping lines (MCP status, skills, tokens spent so far, turn boundaries) do not appear in the chat. <sub>`setup_and_bookkeeping_lines_are_ignored`</sub>
- Lines the parser does not understand are ignored instead of breaking the turn. <sub>`unknown_and_broken_lines_are_ignored`</sub>
- The tool name from the call is filled into the matching result, so the chat shows what finished. <sub>`the_tool_name_is_carried_from_the_call_to_its_result`</sub>
- The same tool call is announced by both the message and the execution line, but the chat shows it once. <sub>`a_tool_call_announced_twice_is_shown_only_once`</sub>
- The turn's answer is the assistant's last message, because the final line does not repeat it. <sub>`the_turn_result_comes_from_the_last_assistant_message`</sub>
- Text that only arrived in the completed message (never as a delta) is still shown in the chat. <sub>`text_missing_from_the_deltas_is_still_delivered`</sub>
- The whole answer is delivered once when the CLI sends no deltas at all. <sub>`the_whole_answer_is_delivered_when_no_deltas_arrive`</sub>
- The session id MadakeCAD pinned is not announced twice when the CLI confirms it in the final line. <sub>`a_confirmed_session_id_is_not_announced_twice`</sub>
- If the CLI answers under a different session id, that id is announced so the next turn resumes the right conversation. <sub>`a_different_session_id_from_the_cli_wins`</sub>
- Assistant text is delivered whichever field name the CLI uses for it. <sub>`assistant_text_is_delivered_under_any_of_the_known_field_names`</sub>
- Tool calls are recognised under the alternative field names too (tool/input, function/parameters). <sub>`tool_calls_are_recognised_under_alternative_field_names`</sub>
- A tool result is recognised as failed under any of the known ways of saying so. <sub>`a_failed_tool_result_is_recognised_under_any_wording`</sub>
- Token usage is also read from OpenAI-style field names (prompt/completion tokens). <sub>`token_usage_is_also_read_from_openai_style_names`</sub>
- An error line from the CLI is shown in the chat as an error. <sub>`an_error_line_becomes_an_error_event`</sub>
- A result line flagged as an error becomes an error instead of a normal completion. <sub>`a_failed_result_line_becomes_an_error`</sub>

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
- Out of the box no default frame template is chosen, so a new drawing starts with the bundled JIS A3 frame. <sub>`default_frame_template_is_unset`</sub>
- A settings file written before the default frame template existed still loads, with no default frame chosen. <sub>`old_settings_file_without_default_frame_template_loads_as_unset`</sub>
- The default frame template id is trimmed, and an empty entry means "no default" rather than an empty id. <sub>`default_frame_template_is_trimmed_and_blank_means_unset`</sub>
- The chosen default frame template is still there after saving and reloading the settings. <sub>`default_frame_template_round_trips_through_save_and_load`</sub>

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
- madake terminals counts a multi-level terminal block by its poles and names its level count, so a 4-pole 2-level block is not mistaken for an 8-pole one. <sub>`terminals_shows_a_multilevel_block_as_poles_and_levels`</sub>
- An empty terminal list prints a friendly message instead of an empty table. <sub>`terminals_handles_empty`</sub>
- Open messages include the loaded path and resulting revision. <sub>`opened_reports_path_and_revision`</sub>


## Frontend (editor UI)


### agentOverlay

- place_symbol's x/y produces a region around the approximate symbol size <sub>`toolBox`</sub>
- the drawing lights up the same region whichever provider called the tool <sub>`toolBox`</sub>
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

### annotationText

- uses the default 2.0mm on a sheet with no setting <sub>`annotationText`</sub>
- scales the whole annotation layer to 0.7 at a 1.4mm text height <sub>`annotationText`</sub>
- falls back to the default for a broken value instead of losing the annotation <sub>`annotationText`</sub>
- accepts only heights between 1.0 and 3.5mm <sub>`annotationText`</sub>

### blockEditor

- starts with one pin on each side, numbered 1 and 2, and an automatic outline <sub>`block pin table draft`</sub>
- numbers a new pin after the highest number already in the table <sub>`block pin table draft`</sub>
- falls back to the pin count plus one when no pin number is a plain number <sub>`block pin table draft`</sub>
- removes just the deleted row from the table <sub>`block pin table draft`</sub>
- swaps a pin with its neighbour and keeps the ends in place <sub>`block pin table draft`</sub>
- writes only the sides that have pins, and omits empty pin names <sub>`block attributes built from the draft`</sub>
- drops rows whose number and name are both blank <sub>`block attributes built from the draft`</sub>
- writes the outline size only when the size is set by hand <sub>`block attributes built from the draft`</sub>
- reads a placed block's attributes back into the same table <sub>`block attributes built from the draft`</sub>
- opens a block with unreadable attributes as an empty table <sub>`block attributes built from the draft`</sub>
- derives the automatic outline size from the pins, rounded to 5 mm steps <sub>`block attributes built from the draft`</sub>
- previews the very symbol that will be drawn, with one connection point per pin <sub>`block attributes built from the draft`</sub>
- reports nothing for a table that is filled in correctly <sub>`checks on the pin table`</sub>
- reports a pin number that is used twice <sub>`checks on the pin table`</sub>
- reports a row that has a name but no pin number <sub>`checks on the pin table`</sub>
- reports a hand-set outline that is too small for its pins <sub>`checks on the pin table`</sub>
- reports a block with more than 200 pins <sub>`checks on the pin table`</sub>
- lists the pins a wire actually reaches <sub>`pins that are already wired`</sub>
- follows the rotation of the placed block when looking for wires <sub>`pins that are already wired`</sub>
- finds no wired pins on a block that nothing is connected to <sub>`pins that are already wired`</sub>
- flags a wired pin that the edit deletes or renumbers <sub>`pins that are already wired`</sub>
- stays quiet when the wired pins survive the edit <sub>`pins that are already wired`</sub>
- opens a pin table with one pin per side when the generic block is picked <sub>`starting a new block from the insert dialog`</sub>
- hands the edited table to the placement as block attributes <sub>`starting a new block from the insert dialog`</sub>
- seeds the hand-set size with the automatic one when switching to manual <sub>`starting a new block from the insert dialog`</sub>
- refuses to place while two pins share a number <sub>`starting a new block from the insert dialog`</sub>
- loads the pin table of the selected block <sub>`editing a block that is already on the drawing`</sub>
- applies the table as a single update_entity, keeping the tag, part number and position <sub>`editing a block that is already on the drawing`</sub>
- keeps attributes that are not part of the pin table <sub>`editing a block that is already on the drawing`</sub>
- drops the hand-set size attribute when the outline goes back to automatic <sub>`editing a block that is already on the drawing`</sub>
- sends no command when the table was not changed <sub>`editing a block that is already on the drawing`</sub>
- throws the draft away when the editor is closed <sub>`editing a block that is already on the drawing`</sub>
- warns that a wired pin loses its connection when its number changes <sub>`warning about wired pins`</sub>
- warns the same way when a wired pin is deleted <sub>`warning about wired pins`</sub>
- stays quiet when the deleted pin had no wire on it <sub>`warning about wired pins`</sub>
- still allows applying the edit, warning and all <sub>`warning about wired pins`</sub>

### deviceTree

- an expanded device is followed by one row per function <sub>`デバイスツリーの整形`</sub>
- a collapsed device shows only its heading and leaves other devices alone <sub>`デバイスツリーの整形`</sub>
- the order of devices and of their functions is kept as the core returned it <sub>`デバイスツリーの整形`</sub>
- an empty project produces no rows <sub>`デバイスツリーの整形`</sub>
- a multi-level terminal block is named by its poles and its levels, not by its terminal count <sub>`見出しの文言`</sub>
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
- a multi-level terminal block (4 poles, 2 levels) numbers its pins with a level letter <sub>`dynamicSymbol`</sub>
- draws one terminal column per level, with the same pole of every level on one row <sub>`dynamicSymbol`</sub>
- keeps every connection point on the 2.5 mm grid and never lets two terminals share one <sub>`dynamicSymbol`</sub>
- a terminal block with a level count outside 1-3 returns null <sub>`dynamicSymbol`</sub>
- the plain single-level terminal block id keeps its numeric-only pin numbers <sub>`dynamicSymbol`</sub>
- static definitions win; unknown ids fall back to dynamic generation <sub>`resolveSymbolDef`</sub>
- puts named pins on both sides of the box, 5 mm apart on the 2.5 mm grid <sub>`blockSymbol`</sub>
- draws pin numbers outside the outline and pin names inside it <sub>`blockSymbol`</sub>
- sizes the box automatically from the pin count and the name length, in 5 mm steps <sub>`blockSymbol`</sub>
- rounds an explicit size up to the 5 mm step <sub>`blockSymbol`</sub>
- rejects duplicate or empty pin numbers, a size too small, and a malformed pin table <sub>`blockSymbol`</sub>
- makes an empty minimum-sized box when there is no pin table <sub>`blockSymbol`</sub>
- builds a block's definition from the instance attributes, not from its id <sub>`resolveInstanceDef`</sub>
- returns nothing for a block whose attributes are broken <sub>`resolveInstanceDef`</sub>

### Frame template layout (canvas)

- mirrors the JIS frame the drawings have always used: 10 mm margin, 4x6 zones and a 120x32 mm bottom-right title block <sub>`図枠テンプレート (jis-a3の既定値)`</sub>
- is a byte-for-byte copy of the bundled jis-a3.json, so the canvas and the print output never drift apart <sub>`図枠テンプレート (jis-a3の既定値)`</sub>
- numbers zone columns 1,2,3 and letters zone rows A,B,C, wrapping back to A after Z <sub>`図枠テンプレート (jis-a3の既定値)`</sub>
- resolves every documented bind to its value <sub>`表題欄のバインド解決`</sub>
- treats {date}, {scale}, {designer} and {approver} as aliases of the title-block fields <sub>`表題欄のバインド解決`</sub>
- turns an unknown bind into an empty string instead of failing <sub>`表題欄のバインド解決`</sub>
- leaves plain text and an unclosed brace exactly as they are <sub>`表題欄のバインド解決`</sub>
- follows the newest revision row for {rev.latest} rather than the stored Rev field <sub>`表題欄のバインド解決`</sub>
- binds a lone sheet as sheet 1 of 1 with an empty project name <sub>`表題欄のバインド解決`</sub>
- prints a fixed-text cell verbatim and resolves only bind cells <sub>`表題欄のバインド解決`</sub>
- lays the default frame on an A3 sheet: a 400x277 frame inset by 10 mm and the title block at (290, 255) <sub>`frameLayout`</sub>
- takes the zone counts from the sheet and only the label style from the template <sub>`frameLayout`</sub>
- moves the title-block rules and cells into paper coordinates with their text resolved <sub>`frameLayout`</sub>
- puts the company name outside the left edge of the title block and drops it when empty <sub>`frameLayout`</sub>
- narrows the title block and the revision columns to 110 mm with the A4 portrait frame <sub>`frameLayout`</sub>
- reports the logo frame rectangle so the space is reserved even without an image <sub>`frameLayout`</sub>
- places the revision table directly above the title block with the same width and row height <sub>`revisionLayout (図枠テンプレート版)`</sub>
- keeps only as many revision rows as the template allows, dropping the oldest <sub>`revisionLayout (図枠テンプレート版)`</sub>
- shows the bind placeholders themselves for the title block cells that would come out blank <sub>`bindOverlays (図枠プレビューの青いプレースホルダ)`</sub>
- never covers a cell that already resolves to a real value <sub>`bindOverlays (図枠プレビューの青いプレースホルダ)`</sub>
- leaves the fixed heading cells alone because they hold no binds <sub>`bindOverlays (図枠プレビューの青いプレースホルダ)`</sub>
- still shows the placeholder for a cell that the drawing omits when empty (the company name) <sub>`bindOverlays (図枠プレビューの青いプレースホルダ)`</sub>
- shows the Japanese name in the Japanese UI and the English one everywhere else <sub>`frameTemplateName (UI言語ごとの図枠名)`</sub>
- falls back to the English name for a user frame that has no Japanese name <sub>`frameTemplateName (UI言語ごとの図枠名)`</sub>
- summarises the paper, the zone grid, the title block size and the revision table <sub>`frameTemplateSummary (一覧・情報行の要約)`</sub>
- lists the binds the title block uses once each, in the order they appear <sub>`bindNames (フィールドバインド表)`</sub>
- comes back empty for a frame whose title block holds only fixed text <sub>`bindNames (フィールドバインド表)`</sub>

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

### netTag

- draws a tag pointing right as a five-cornered flag with its point on the right <sub>`netTag`</sub>
- draws a tag pointing left as the mirrored flag, with its point on the left <sub>`netTag`</sub>
- grows the flag body away from its point so the wire under the label stays visible <sub>`netTag`</sub>
- puts the label position at the root of the point, mirroring left and right flags <sub>`netTag`</sub>
- keeps the text inside the flag and widens the flag for a longer net name <sub>`netTag`</sub>
- gives both directions the same flag size <sub>`netTag`</sub>
- escapes the cross-reference on the body side of the flag <sub>`netTag`</sub>
- shrinks the whole flag when the annotation text height is smaller <sub>`netTag`</sub>
- leaves a label without the tag attribute as plain text <sub>`netTag`</sub>
- ignores an unknown tag value instead of breaking <sub>`netTag`</sub>

### panel

- gives a footprint the rectangle of its own size <sub>`panel layout geometry`</sub>
- swaps width and height for a footprint turned 90 degrees <sub>`panel layout geometry`</sub>
- falls back to the default rectangle for a part with no size <sub>`panel layout geometry`</sub>
- draws the mounting plate inset inside the panel outline <sub>`panel layout geometry`</sub>
- has no mounting plate when the outline is smaller than the inset <sub>`panel layout geometry`</sub>
- draws a DIN rail as a band centred on its mounting line <sub>`panel layout geometry`</sub>
- covers the same span for a rail drawn backwards <sub>`panel layout geometry`</sub>
- fills a duct with 45 degree hatch lines that stay inside it <sub>`panel layout geometry`</sub>
- has no hatch lines for a flat duct <sub>`panel layout geometry`</sub>
- snaps a part dropped near a rail onto its mounting line <sub>`panel layout geometry`</sub>
- orders the parts for numbering top row first, then left to right <sub>`panel item numbers and legend`</sub>
- keeps parts on the same rail in one row even with different heights <sub>`panel item numbers and legend`</sub>
- draws no balloon before the part is numbered <sub>`panel item numbers and legend`</sub>
- puts the balloon just outside the top-left corner with a short leader <sub>`panel item numbers and legend`</sub>
- lists numbered parts in order and unnumbered ones last <sub>`panel item numbers and legend`</sub>
- grows the legend table one row at a time <sub>`panel item numbers and legend`</sub>
- numbers the rails from the top of the panel down <sub>`panel item numbers and legend`</sub>
- loads the unplaced parts from the engine <sub>`unplaced parts store`</sub>
- does not reload while the drawing has not changed <sub>`unplaced parts store`</sub>
- drops a part from the list once it has been placed <sub>`unplaced parts store`</sub>
- keeps the panel open when the list cannot be read <sub>`unplaced parts store`</sub>
- highlights the row of the part being placed <sub>`unplaced parts store`</sub>

### panelTools

- turns a rectangle drag into one add_entity command for the enclosure <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- accepts a drag in any direction for the enclosure <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- numbers each new enclosure (CP-1, CP-2 …) <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- sends no command when the enclosure drag is a mere click <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- draws a DIN rail horizontally from the point where the drag started <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- turns a rectangle drag into one duct <sub>`panel drawing tools (enclosure / DIN rail / duct)`</sub>
- shows the reference, part number and outline size of an unplaced part <sub>`unplaced parts panel`</sub>
- marks a part that falls back to the default rectangle <sub>`unplaced parts panel`</sub>
- flags a part with no part number as unregistered <sub>`unplaced parts panel`</sub>
- centres the footprint on the cursor <sub>`footprint placement and rail snapping`</sub>
- centres a footprint turned 90 degrees on its swapped outline <sub>`footprint placement and rail snapping`</sub>
- snaps the footprint centre onto a nearby DIN rail <sub>`footprint placement and rail snapping`</sub>
- leaves the footprint where the cursor is when no rail is near <sub>`footprint placement and rail snapping`</sub>
- adds one footprint carrying the reference and part number <sub>`footprint placement and rail snapping`</sub>
- corrects a drag so the moved footprint lands on the rail <sub>`footprint placement and rail snapping`</sub>
- tells which rail a footprint sits on <sub>`footprint placement and rail snapping`</sub>
- turns a property edit into one update_entity command <sub>`panel entity properties`</sub>
- sends no command when nothing changed <sub>`panel entity properties`</sub>
- ignores a width or height that is not a positive number <sub>`panel entity properties`</sub>
- keeps the rotation at 0 or 90 degrees <sub>`panel entity properties`</sub>
- edits the enclosure name and size, and the rail line and length <sub>`panel entity properties`</sub>
- refuses to edit a schematic entity through the panel properties <sub>`panel entity properties`</sub>
- disables the schematic tools on a panel sheet <sub>`tools allowed per sheet kind`</sub>
- disables the panel tools on a schematic sheet <sub>`tools allowed per sheet kind`</sub>
- disables both tool sets when no sheet is open <sub>`tools allowed per sheet kind`</sub>
- turns balloon numbering into one renumber_panel_items command <sub>`balloon numbering and the legend table`</sub>
- sends no command when there is nothing to number <sub>`balloon numbering and the legend table`</sub>
- inserts the legend table with one add_entity command carrying only its position <sub>`balloon numbering and the legend table`</sub>

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
- counts a changeover contact as a contact of its relay <sub>`relayXref`</sub>
- writes the terminals of a changeover contact as common, break and make <sub>`relayXref`</sub>
- lists the remaining changeover poles of a 2CO relay as unused rows <sub>`relayXref`</sub>
- lets a changeover pole be drawn as a plain make or break contact <sub>`relayXref`</sub>
- centres the contact map table under the coil <sub>`relayXref`</sub>
- anchors both annotations to the symbol outline <sub>`relayXref`</sub>
- moves the coil location off a colliding wire note <sub>`relayXref`</sub>
- falls back below the outline when the space above is also occupied <sub>`relayXref`</sub>
- selects the display mode from the device_block attribute <sub>`relayXref`</sub>
- only collects coils flagged as a device block <sub>`relayXref`</sub>
- excludes device-block coils from the separate contact map <sub>`relayXref`</sub>
- keeps the device block layout no narrower than the pin span <sub>`relayXref`</sub>
- grows the device block for the part number and keeps the pins reachable <sub>`relayXref`</sub>
- stacks the device block's contact rows under the header <sub>`relayXref`</sub>

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
- keeps the lettering inside a symbol where the symbol definition puts it, whatever the rotation <sub>`drawSymbol text placement`</sub>
- keeps a one- or two-character terminal number inside its own mark, whatever the rotation <sub>`drawSymbol text placement`</sub>
- puts an unturned terminal block's number just over a third of a character height below its mark centre on the canvas, a smaller offset than the exported drawing's half <sub>`drawSymbol text placement`</sub>
- draws a jumper as a bar only, with no round mark at either end <sub>`drawSymbol text placement`</sub>

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

### shapes

- starts a new shape plain: default colour, hairline stroke, no fill, solid line <sub>`shape geometry`</sub>
- normalises a rectangle to the same corners whichever way it was dragged <sub>`shape geometry`</sub>
- reads a circle as a centre plus a point on its edge <sub>`shape geometry`</sub>
- keeps every point of a polyline and bounds them all <sub>`shape geometry`</sub>
- requires exactly two points for a rectangle, line and circle, and any number for a polyline <sub>`shape geometry`</sub>
- accepts only printable stroke widths <sub>`shape geometry`</sub>
- draws a shape in its token's screen colour, and in the selection colour while selected <sub>`shape colours on screen`</sub>
- shades a filled shape lightly so the circuit underneath stays readable <sub>`shape colours on screen`</sub>
- uses the same dash rhythm as a harness boundary <sub>`shape colours on screen`</sub>
- grabs a rectangle by its outline, not by the area inside it <sub>`shape hit testing`</sub>
- grabs a circle on its edge <sub>`shape hit testing`</sub>
- grabs a line or a polyline anywhere along it <sub>`shape hit testing`</sub>
- turns a finished drawing into one add_entity command <sub>`shape commands`</sub>
- sends nothing for a collapsed shape from a stray click <sub>`shape commands`</sub>
- merges the duplicate point a double-click leaves at the end of a polyline <sub>`shape commands`</sub>
- updates the look with one update_entity command, and sends nothing when nothing changed <sub>`shape commands`</sub>
- refuses to send a stroke width the engine would reject <sub>`shape commands`</sub>
- makes no command for anything that is not a shape <sub>`shape commands`</sub>
- lists only the shapes of a sheet <sub>`shape commands`</sub>
- maps each drawing tool to one kind of shape <sub>`shape commands`</sub>
- keeps a shape out of the harness column of the wire list <sub>`shapes and the reports`</sub>

### sheets

- adds a panel sheet when the panel kind is chosen <sub>`sheet kind (schematic / panel)`</sub>
- adds a schematic sheet when the schematic kind is chosen <sub>`sheet kind (schematic / panel)`</sub>
- numbers the new sheet within its own kind <sub>`sheet kind (schematic / panel)`</sub>
- skips a number that another sheet already uses <sub>`sheet kind (schematic / panel)`</sub>
- keeps the paper size and orientation of the sheet in front <sub>`sheet kind (schematic / panel)`</sub>
- treats a sheet with no kind as a schematic sheet <sub>`sheet kind (schematic / panel)`</sub>
- labels the sheet kind shown next to the drawing tabs <sub>`sheet kind (schematic / panel)`</sub>

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

### symbolEditor

- snaps a click position to the nearest 2.5 mm grid intersection <sub>`symbol editor canvas geometry`</sub>
- snaps negative coordinates (left of and above the origin) to the grid too <sub>`symbol editor canvas geometry`</sub>
- picks the connection point on top of a shape rather than the shape itself <sub>`symbol editor canvas geometry`</sub>
- picks the most recently drawn shape where two shapes overlap <sub>`symbol editor canvas geometry`</sub>
- selects nothing when the click is away from every shape and point <sub>`symbol editor canvas geometry`</sub>
- finds an attribute slot by clicking its anchor point <sub>`symbol editor canvas geometry`</sub>
- moves every point of a shape by the same amount when it is dragged <sub>`symbol editor canvas geometry`</sub>
- numbers a new connection point one past the largest number in use <sub>`symbol editor canvas geometry`</sub>
- places a new connection point one grid step outside the shape on the chosen side <sub>`symbol editor canvas geometry`</sub>
- never stacks a second connection point on the same point as an existing one <sub>`symbol editor canvas geometry`</sub>
- reports no problem for a well-formed symbol <sub>`symbol editor validation`</sub>
- reports a connection point that is off the 2.5 mm grid <sub>`symbol editor validation`</sub>
- reports two connection points with the same number at the same position <sub>`symbol editor validation`</sub>
- reports two different connection point numbers stacked on one position <sub>`symbol editor validation`</sub>
- accepts the same connection point number at two separate positions <sub>`symbol editor validation`</sub>
- reports a missing reference designator (TAG) attribute slot <sub>`symbol editor validation`</sub>
- rejects an id that is already used by a bundled symbol <sub>`symbol editor validation`</sub>
- rejects an empty id, an id with unusable characters, and an empty symbol <sub>`symbol editor validation`</sub>
- returns every problem at once instead of stopping at the first <sub>`symbol editor validation`</sub>
- always shows the four pre-save rows (grid, numbers, required and optional attributes) <sub>`symbol editor pre-save checklist`</sub>
- marks the optional-attribute row as a caution while some optional slots are unplaced <sub>`symbol editor pre-save checklist`</sub>
- marks all four rows as passed once every optional attribute is placed <sub>`symbol editor pre-save checklist`</sub>
- turns the affected row into an error and lists the remaining reasons after it <sub>`symbol editor pre-save checklist`</sub>
- keeps the save button disabled while any row is an error <sub>`symbol editor pre-save checklist`</sub>
- starts a blank draft that already carries the required TAG attribute slot <sub>`symbol editor store`</sub>
- opens a duplicate of an existing symbol with its shapes and points copied <sub>`symbol editor store`</sub>
- selects a shape right after it is drawn <sub>`symbol editor store`</sub>
- moves the selected shape by the dragged amount <sub>`symbol editor store`</sub>
- deletes the selected shape or connection point <sub>`symbol editor store`</sub>
- gives a connection point added from a side button the next number, on the grid <sub>`symbol editor store`</sub>
- snaps a connection point dropped on the canvas onto a grid intersection <sub>`symbol editor store`</sub>
- edits the number, name and direction of a connection point from the table <sub>`symbol editor store`</sub>
- places and removes an attribute slot, and cautions about the removed ones <sub>`symbol editor store`</sub>
- edits the symbol information (id, names, prefix and search keys) from the right panel <sub>`symbol editor store`</sub>
- keeps the save path in step with the symbol id <sub>`symbol editor store`</sub>
- refuses to save under an id that a bundled symbol already uses <sub>`symbol editor store`</sub>
- shows off-grid points, duplicate numbers and a missing TAG slot in the checklist <sub>`symbol editor store`</sub>
- writes the draft to the user library and reports where it was saved <sub>`symbol editor store`</sub>
- clears the unsaved-changes mark and reloads the insert list after a successful save <sub>`symbol editor store`</sub>
- keeps the editor open and shows every reason when the save is refused <sub>`symbol editor store`</sub>
- asks for confirmation instead of closing while there are unsaved changes <sub>`symbol editor store`</sub>
- offers every symbol (bundled and user) in the duplicate-from picker <sub>`symbol editor store`</sub>

### symbolLibrary

- matches the English name, the Japanese name or the symbol id <sub>`symbolMatchesQuery`</sub>
- finds symbols by the shop-floor words kept in the search keywords <sub>`symbolMatchesQuery`</sub>
- treats an empty query as matching everything <sub>`symbolMatchesQuery`</sub>
- groups symbols by category keeping the library order <sub>`groupSymbolsByCategory`</sub>
- keeps only matching symbols and drops the categories left empty <sub>`groupSymbolsByCategory`</sub>
- shows the Japanese name in Japanese and the English name in English <sub>`symbolName`</sub>
- falls back to whichever name the symbol actually has <sub>`symbolName`</sub>
- shows the symbol id when both names are empty <sub>`symbolName`</sub>

### terminalJumpers

- keeps jumpers between adjacent terminals and drops non-adjacent ones <sub>`parseJumperSpec`</sub>
- keeps every jumper when there are several, in terminal order <sub>`parseJumperSpec`</sub>
- treats empty or unreadable text as no jumpers <sub>`parseJumperSpec`</sub>
- accepts level-aware jumpers on a multi-level block <sub>`parseJumperSpec`</sub>
- drops a jumper written without its level letter on a multi-level block <sub>`parseJumperSpec`</sub>
- drops a jumper to a level the block does not have and a diagonal one <sub>`parseJumperSpec`</sub>
- keeps the same millimetre values as the Rust side <sub>`jumper bar dimensions`</sub>
- is drawn in the gap between the two marks, on the column's centre line <sub>`jumperBars`</sub>
- is exactly as long as the gap between the two marks it joins <sub>`jumperBars`</sub>
- lies on the pole's row, between the two level columns' marks <sub>`jumperBars`</sub>
- ends exactly on the edge of the mark of the terminal it belongs to <sub>`jumperBars`</sub>
- never covers a terminal number <sub>`jumperBars`</sub>
- produces one segment per jumper when there are several <sub>`jumperBars`</sub>
- produces no segment for a jumper to a terminal the block does not have <sub>`jumperBars`</sub>

### textStyle

- offers a small fixed set of colour tokens including the default <sub>`note styling`</sub>
- does not accept a raw colour code as a colour token <sub>`note styling`</sub>
- draws a note without a colour in the default screen colour <sub>`note styling`</sub>
- shows the selection highlight instead of the note colour while it is selected <sub>`note styling`</sub>
- maps the three anchors to the three canvas text alignments <sub>`note styling`</sub>
- puts the hit box to the right of the anchor point for a left-aligned note <sub>`note hit box`</sub>
- extends the hit box to the left for centred and right-aligned notes <sub>`note hit box`</sub>
- widens the hit box with the length of the text <sub>`note hit box`</sub>
- draws nothing extra for a symbol with no function label <sub>`function label`</sub>
- treats a blank function label as no label and trims the surrounding spaces <sub>`function label`</sub>
- places the label just below the outline when the definition has no slot <sub>`function label`</sub>
- follows the FUNCTION slot when the symbol definition defines one <sub>`function label`</sub>

### viewClasses

- entity kinds map to view classes (junctions count as wires) <sub>`entityViewClass`</sub>
- VIEW_CLASSES enumerates every class exactly once <sub>`entityViewClass`</sub>
- shows every view class, including wire numbers and harnesses, until it is switched off <sub>`view class visibility`</sub>

### viewport

- world<->screen conversion round-trips and zoom keeps the anchor fixed <sub>`Viewport`</sub>
- panning moves the view in screen pixels <sub>`Viewport`</sub>
- snapping rounds to the 2.5 mm grid by default <sub>`Viewport`</sub>
- zoom is clamped to sane bounds <sub>`Viewport`</sub>

### wireNotes

- annotates a wire with its gauge, colour and cut length <sub>`wireNoteText`</sub>
- leaves out attributes the wire does not have <sub>`wireNoteText`</sub>
- gives a wire with no attributes no note at all <sub>`wireNoteText`</sub>
- prints an unknown colour name exactly as it was typed <sub>`wireNoteText`</sub>
- places the note of a horizontal wire below the wire, opposite its wire number <sub>`wireNoteLabels`</sub>
- places the note of a vertical wire to the right of the wire, opposite its wire number <sub>`wireNoteLabels`</sub>
- annotates every wire, unlike the wire number which is drawn once per net <sub>`wireNoteLabels`</sub>
- puts the note on the longest straight run of a bent wire <sub>`wireNoteLabels`</sub>
- skips wires that carry no attributes <sub>`wireNoteLabels`</sub>
- keeps the default 2.0mm note height on a sheet with no setting <sub>`annotation text height`</sub>
- draws the wire notes at the sheet's annotation text height and moves them closer <sub>`annotation text height`</sub>

### wireNumbers

- puts the number of a horizontal wire 2.5mm above the midpoint, centred <sub>`wireNumberLabels`</sub>
- puts the number of a vertical wire 2.5mm to the left of the midpoint, right aligned <sub>`wireNumberLabels`</sub>
- draws one net's number only once, at the midpoint of its longest segment <sub>`wireNumberLabels`</sub>
- skips wires without a number and numbers that are only whitespace <sub>`wireNumberLabels`</sub>
- returns every number in a stable order <sub>`wireNumberLabels`</sub>
- places a diagonal wire's number above when it runs wide and to the left when it runs tall <sub>`wireNumberLabels`</sub>
- ignores entities that are not wires <sub>`wireNumberLabels`</sub>
- keeps the default wire number size and offset on a sheet with no setting <sub>`annotation text height`</sub>
- shrinks the wire number in proportion to the sheet's annotation text height <sub>`annotation text height`</sub>
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

### wireParts

- looks a wire's part number up by colour and gauge <sub>`wirePartNo`</sub>
- leaves an unregistered combination empty <sub>`wirePartNo`</sub>
- prefers a part number written on the wire <sub>`wirePartNo`</sub>
- ignores spaces and letter case in the colour <sub>`wirePartNo`</sub>
- treats the English and the Japanese name of a colour as the same wire <sub>`wirePartNo`</sub>
- does not look up a wire without a colour or a gauge <sub>`wirePartNo`</sub>
- takes the first of two rows for the same colour and gauge <sub>`wirePartNo`</sub>

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
- cross-references a designator on two sheets both ways <sub>`G13 device cross-reference`</sub>
- shows nothing for a designator on a single sheet <sub>`G13 device cross-reference`</sub>
- skips relay coils and contacts <sub>`G13 device cross-reference`</sub>
- maps each symbol entity id to the text drawn beside it <sub>`G13 device cross-reference`</sub>
- lists every other sheet for a designator on three sheets <sub>`G13 device cross-reference`</sub>
- places the text right of the symbol outline, level with the designator label <sub>`G13 device cross-reference`</sub>

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
- builds a sheet command for the entered annotation text height <sub>`annotationTextCommand`</sub>
- clears the height back to the default when the field is emptied <sub>`annotationTextCommand`</sub>
- sends nothing for a height outside the allowed range or a non-number <sub>`annotationTextCommand`</sub>
- sends nothing when the height did not change <sub>`annotationTextCommand`</sub>

### symbolPickerDynamic

- keeps the level count inside the supported range of 1 to 3 levels <sub>`clampLevels`</sub>
- rounds a fractional level count and falls back to one level for a non-number <sub>`clampLevels`</sub>
- keeps the pole count inside the supported range of 1 to 50 poles <sub>`clampPins`</sub>
- offers the levels stepper for a terminal block only, not for a connector <sub>`supportsLevels`</sub>
- places the plain single-level terminal block id when the level count is left at one <sub>`dynamicSymbolId`</sub>
- places the multi-level terminal block id when the level count is raised <sub>`dynamicSymbolId`</sub>
- never adds a level suffix to a connector, whatever the level count says <sub>`dynamicSymbolId`</sub>
- has the levels stepper label and hint in both the English and the Japanese catalogue <sub>`段数ステッパーの表示文字列`</sub>

### chatComposer

- tokens are added up across the turns of a conversation <sub>`sumUsage`</sub>
- Copilot's premium requests are counted separately from tokens, as a fraction <sub>`sumUsage`</sub>
- a conversation with no usage yet counts as zero <sub>`sumUsage`</sub>

### drawingContext

- with no selection the context covers the whole sheet <sub>`drawingContextTag`</sub>
- with no sheet the string stays well-formed <sub>`drawingContextTag`</sub>
- a selection lists reference designators (falling back to entity kinds) <sub>`drawingContextTag`</sub>
- selections beyond 10 items collapse into +N more <sub>`drawingContextTag`</sub>
- selection ids absent from the active sheet are ignored <sub>`drawingContextTag`</sub>
- an empty draft receives the context as-is <sub>`appendContextTag`</sub>
- an existing draft is separated by a newline (without doubling) <sub>`appendContextTag`</sub>

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
- ships English and Japanese catalogs <sub>`i18n`</sub>
- keeps the English and Japanese catalogs key-identical <sub>`i18n`</sub>
- rejects empty strings in either catalog <sub>`i18n`</sub>
- compiles every message in both catalogs <sub>`i18n`</sub>
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
- the same tool shows the same name whichever provider called it <sub>`summarizeToolUse`</sub>
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
- frame_template_changed patches update the mirror both ways (switch and undo) <sub>`document store`</sub>
- loads the frame template catalogue on bootstrap <sub>`document store: 図枠テンプレートの解決`</sub>
- still opens the drawing when the frame template catalogue cannot be read <sub>`document store: 図枠テンプレートの解決`</sub>
- resolves the frame template the project has chosen <sub>`document store: 図枠テンプレートの解決`</sub>
- falls back to the bundled JIS A3 frame when the project names none <sub>`document store: 図枠テンプレートの解決`</sub>
- falls back to JIS A3 for a frame id this installation does not know <sub>`document store: 図枠テンプレートの解決`</sub>
- swaps the frame handed to the canvas as soon as the project's frame changes <sub>`document store: 図枠テンプレートの解決`</sub>
- binds the title block against the project, so each sheet knows its own number out of the total <sub>`document store: 図枠テンプレートの解決`</sub>
- has no frame options at all when the project holds no sheet <sub>`document store: 図枠テンプレートの解決`</sub>

### frameTemplates

- opens with the bundled and user frames listed and the project's current frame selected <sub>`frame template dialog store`</sub>
- selects the bundled JIS A3 frame for a project that has never chosen one <sub>`frame template dialog store`</sub>
- marks each row as a bundled or a user frame <sub>`frame template dialog store`</sub>
- switches the preview and the summary line to the frame that was clicked <sub>`frame template dialog store`</sub>
- applies to this project with a single set_frame_template command <sub>`frame template dialog store`</sub>
- sends nothing when the frame that is already in use is applied again <sub>`frame template dialog store`</sub>
- moves the in-use marker to the applied frame so a second click sends nothing <sub>`frame template dialog store`</sub>
- saves the frame as the default for new projects instead of editing the drawing <sub>`frame template dialog store`</sub>
- keeps the project edit and the global default apart when both are used in one visit <sub>`frame template dialog store`</sub>
- OK applies and then closes the dialog <sub>`frame template dialog store`</sub>
- Apply keeps the dialog open so another frame can be tried <sub>`frame template dialog store`</sub>
- Cancel closes without applying anything <sub>`frame template dialog store`</sub>
- stays open and shows the reason when applying fails <sub>`frame template dialog store`</sub>
- reports unreadable frame JSON files instead of dropping the list <sub>`frame template dialog store`</sub>
- opens the user frame folder and reloads the list afterwards <sub>`frame template dialog store`</sub>
- just names the folder when it cannot be opened (browser verification mode) <sub>`frame template dialog store`</sub>

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

- offers the seven report kinds of the reports tab <sub>`report generation dialog store`</sub>
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
- starts with no global default frame template <sub>`settings store`</sub>
- saves the global default frame template chosen in the frame picker <sub>`settings store`</sub>
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
- names a terminal block by its pole count, not by its terminal count <sub>`terminal strip editor store`</sub>
- counts the pole count from the chart rows while the block list is not loaded yet <sub>`terminal strip editor store`</sub>
- reports no poles while neither the block list nor the chart is loaded <sub>`terminal strip editor store`</sub>
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
- starts on the schematic ribbon tab <sub>`シート種別に合わせた画面の切り替え`</sub>
- opens the panel ribbon tab and the unplaced parts dock for a panel sheet <sub>`シート種別に合わせた画面の切り替え`</sub>
- returns to the schematic tab and the project tree on a schematic sheet <sub>`シート種別に合わせた画面の切り替え`</sub>
- leaves a ribbon tab the user chose after the panel tab is once shown <sub>`シート種別に合わせた画面の切り替え`</sub>
- does not steal the left dock from an open agent conversation <sub>`シート種別に合わせた画面の切り替え`</sub>

### verification

- run fetches diagnostics, opens the panel, and counts by severity <sub>`verification store`</sub>
- close hides the panel but keeps the diagnostics <sub>`verification store`</sub>
- a failed run resets the running flag and leaves diagnostics empty <sub>`verification store`</sub>

### blockPlacement

- shows a ghost shaped by the pin table that is about to be placed <sub>`placing a block with a pin table`</sub>
- adds the block with its pin table in a single add_entity <sub>`placing a block with a pin table`</sub>
- selects a tall block anywhere inside its outline, far from its origin <sub>`picking a symbol on the canvas`</sub>
- does not select the block when the click is outside its outline <sub>`picking a symbol on the canvas`</sub>
- selects a small symbol inside its own outline <sub>`picking a symbol on the canvas`</sub>
- leaves a small symbol alone when the click is well outside it <sub>`picking a symbol on the canvas`</sub>
- still lets a block with broken attributes be picked near where it was placed <sub>`picking a symbol on the canvas`</sub>

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

### panelPlacement

- draws the enclosure with one rectangle drag <sub>`panel drawing tools on a panel sheet`</sub>
- draws a DIN rail with one horizontal drag <sub>`panel drawing tools on a panel sheet`</sub>
- draws a duct with one rectangle drag <sub>`panel drawing tools on a panel sheet`</sub>
- sends nothing when the drawing is abandoned with Escape <sub>`panel drawing tools on a panel sheet`</sub>
- enters footprint placement for the chosen part <sub>`placing an unplaced part`</sub>
- commits the footprint on click and snaps it onto the rail <sub>`placing an unplaced part`</sub>
- toggles the footprint between 0 and 90 degrees on R <sub>`placing an unplaced part`</sub>
- returns to the select tool once the part is placed <sub>`placing an unplaced part`</sub>
- selects panel entities by clicking them <sub>`placing an unplaced part`</sub>
- snaps a footprint drag onto the rail <sub>`placing an unplaced part`</sub>
- refuses the schematic tools on a panel sheet <sub>`tools that do not belong to the sheet`</sub>
- refuses the panel tools on a schematic sheet <sub>`tools that do not belong to the sheet`</sub>
- does not start footprint placement on a schematic sheet <sub>`tools that do not belong to the sheet`</sub>
- keeps select, move and delete available on a panel sheet <sub>`tools that do not belong to the sheet`</sub>

### shapePlacement

- draws a rectangle with one drag <sub>`drawing shapes`</sub>
- draws a line and a circle with one drag each <sub>`drawing shapes`</sub>
- creates nothing from a stray click <sub>`drawing shapes`</sub>
- draws a polyline by clicking each corner and finishing with a double click <sub>`drawing shapes`</sub>
- finishes a polyline with Enter and throws away a single stray point <sub>`drawing shapes`</sub>
- refuses the shape tools on a panel layout sheet <sub>`drawing shapes`</sub>
- picks a shape by its outline, not by the area inside it <sub>`selecting shapes`</sub>
- moves every point of a shape with one move_entities command <sub>`selecting shapes`</sub>

