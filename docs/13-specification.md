# Detailed Specification

**日本語: [13-specification.ja.md](13-specification.ja.md)** | ← [Roadmap](12-roadmap.md)

> **Generated from the test suite — do not edit by hand.**
> Every clause below is enforced by an automated test; the test id is shown in gray.
> Regenerate with `python3 scripts/gen_spec.py` after changing tests.

This document is the living, always-verified specification of MadakeCAD:
if a behavior is listed here, a test proves it on every run of the suite.


**2927 specification clauses** across 6 areas.


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
- A block pin may list contacts; they reach the pin definition in order, trimmed, and a pin without contacts has none. <sub>`contacts_are_trimmed`</sub>
- Contacts that are empty, repeated on one pin, alone (one conductor is a plain pin) or more than 32 are refused with the pin number in the message. <sub>`bad_contacts_are_refused`</sub>

### cable

- Placing a cable assigns the wires its definition line crosses as cores 1, 2, 3 in order from the line's start, each with the wire's colour; one undo removes the cable. <sub>`placing_a_cable_assigns_the_crossed_wires_as_cores_in_order`</sub>
- A cable placed with no core count takes as many cores as wires it crosses; wires the line does not reach are left out. <sub>`a_cable_without_a_core_count_takes_the_number_of_crossed_wires`</sub>
- The label next to a cable reads designator, part number, cores times gauge and length. <sub>`the_cable_label_reads_designator_part_cores_gauge_and_length`</sub>
- Assigning more wires than the cable has cores is an ERC error naming the cable. <sub>`more_assigned_wires_than_cores_is_an_erc_error`</sub>
- The cable list shows each cable with its part, cores, gauge, length, the devices at both ends of its wires, used and spare cores, shield and sheet. <sub>`the_cable_list_shows_ends_used_and_spare_cores`</sub>
- In the BOM, cables are counted per part number: by total length when every cable has a length, otherwise by the number of cables; a cable without a part number is left out. <sub>`the_bom_counts_cables_per_part_number`</sub>
- The printed sheet draws the cable's definition line, the shield ellipse when it has a shield, and the label. <sub>`the_printed_sheet_draws_the_definition_line_shield_and_label`</sub>
- A cable whose definition line does not have two points or that has no cores is refused; a core whose wire is not on the sheet (deleted earlier) is accepted and left to ERC, so undoing a cable deletion never fails. <sub>`a_malformed_cable_is_refused`</sub>
- A definition-line cable whose core wire was deleted still renders in the netlist, ERC, cable list, wire list, BOM and printed sheet without panicking; it can be renamed, and deleting it then undoing twice brings back both the cable and the wire. <sub>`a_definition_cable_whose_wire_is_gone_still_works`</sub>
- A run cable is refused when a core lacks an end, reuses a wire or a pin, points a wire end at a part or a pin end at a wire, still uses the definition-line field, or the run has fewer than two points; a definition-line cable with core ends is refused too. An end whose wire, part or pin is not on the sheet is accepted (ERC reports it). <sub>`a_run_cable_is_checked_when_placed`</sub>
- Two cores of a run cable with the same name (after trimming spaces) are refused when placed, naming the duplicate. <sub>`a_run_cable_with_duplicate_core_names_is_refused`</sub>
- Deleting one breakout wire, then the cable as a separate edit, then undoing twice brings back the cable and the wire; redo deletes them again. <sub>`deleting_a_breakout_wire_then_the_cable_can_be_undone`</sub>
- Deleting the part a run cable lands on, then the cable as a separate edit, then undoing twice brings back both; redo deletes them again. <sub>`deleting_the_landing_part_then_the_cable_can_be_undone`</sub>
- A run cable whose breakout wire was deleted can still be updated (renamed); ERC keeps reporting the missing core. <sub>`a_run_cable_whose_wire_is_gone_can_be_updated`</sub>
- Deleting a run cable together with its breakout wires and the part it lands on, then undoing, brings all of them back, and redo deletes them again (the cable is restored after what it points at). <sub>`deleting_a_run_cable_with_its_wires_can_be_undone`</sub>
- The LTE harness drawn with a single-line run passes ERC: no short, no unconnected plug pin, no dangling breakout wire and no cable finding. <sub>`the_lte_harness_passes_erc`</sub>
- Two cores of one cable that end up in the same net are an ERC error naming the cable and both cores, for a run cable and for a definition-line cable alike. <sub>`two_cores_in_one_net_is_a_short`</sub>
- A run-cable core whose wire or part has been deleted is an ERC error naming the cores. <sub>`a_core_whose_target_is_gone_is_unassigned`</sub>
- A single-line run whose end is far from the part its cores land on is warned about; one that touches the part is not. <sub>`a_run_end_away_from_its_part_is_warned`</sub>
- The breakout ends of a run cable's wires are not dangling; without the cable they would be. <sub>`breakout_ends_are_not_dangling`</sub>
- A run cable with more cores than its core count is the same core-overflow error as a definition-line cable. <sub>`a_run_cable_with_too_many_cores_overflows`</sub>
- A run-cable core with both ends unassigned is reported as unassigned, with no extra floating-net or short finding. <sub>`a_core_with_both_ends_unassigned_is_only_unassigned`</sub>
- The cable list names the devices at each end of a single-line cable (the board its breakout wires go to, the plug its run lands on) and counts the cores that have both ends as used. <sub>`the_cable_list_shows_the_ends_of_a_run_cable`</sub>
- The printed sheet draws a single-line cable as a thick line with a slash, a bar across the plug pins it lands on, fan lines to the breakout wires with the core names, and the cable label. <sub>`the_printed_sheet_draws_a_run_cable`</sub>
- Several cores may land on one port pin as long as each names a different contact; two cores on the same contact are refused. <sub>`cores_on_one_port_pin_need_different_contacts`</sub>
- A core end without a contact is written without a "contact" key, so files without ports do not change. <sub>`a_pin_end_without_contact_has_no_contact_key`</sub>

### cable_run

- Planning the LTE harness draws one wire per core from the board pins to a breakout 10 mm out, runs one line from there to the plug, and maps the cores to the pins in pin order; the result is four nets. <sub>`plans_the_lte_harness`</sub>
- Two connectors joined end to end by one line (3.5 mm AUX) get a run between their pin columns and no wires. <sub>`plans_a_single_line_between_two_connectors`</sub>
- Pins may be given by name as well as number, and a side whose pin count differs from the core count is refused, naming the side and both counts. <sub>`pins_by_name_and_a_count_mismatch`</sub>
- Planning a cable with two cores of the same name (after trimming spaces) is refused, naming the duplicate. <sub>`duplicate_core_names_are_refused`</sub>
- A single line landing on a through terminal block maps each core to one terminal number, using the left connection points unless "#2" picks the right ones. <sub>`a_single_line_on_a_through_terminal_block`</sub>
- A breakout placed behind the pins, or whose wire ends would land on another wire, is refused with a hint to move breakout_at. <sub>`a_bad_breakout_is_refused`</sub>
- With labels on one side every core gets a 10 mm breakout wire running straight away from the run and ending in a net label of that name; the core ends are those wires, and a second sheet carrying the same labels joins each core to its pins in the project netlist. <sub>`breakout_wires_end_at_net_labels`</sub>
- Labels with a single line, a wrong label count, labels together with pins, and labels without breakout_at are each refused with a Japanese message. <sub>`bad_label_sides_are_refused`</sub>
- Breakout wires get no gauge and no colour of their own (the colour stays on the cable core), so they carry no wire note, while the wire list still shows each core's colour. <sub>`breakout_wires_carry_no_note`</sub>
- When the label wires run vertically the labels sit side by side, so the core pitch grows to the widest label plus a gap and no two labels overlap; the run direction follows the first via point. <sub>`side_by_side_labels_do_not_overlap_and_via_sets_the_direction`</sub>
- Two labels with the same name (after trimming) on one side are refused naming the duplicate. <sub>`duplicate_label_names_are_refused`</sub>

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
- A note whose leader points at an entity id that does not exist on the sheet is refused instead of being stored with a leader line that never draws. <sub>`a_note_with_an_unknown_leader_target_is_refused`</sub>
- A note can set its leader to another entity already on the sheet, and clearing the leader (setting it back to None) is undoable like any other update. <sub>`a_note_can_target_an_existing_entity_and_the_leader_can_be_cleared`</sub>
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
- SetPlcSheetOptions saves the PLC I/O drawing settings with the project, and undo puts back what was saved before. <sub>`set_plc_sheet_options_saves_the_settings_and_undo_restores_them`</sub>
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
- An undo that cannot be applied changes nothing and keeps the edit in the undo history (it is neither half applied nor lost). <sub>`a_failed_undo_keeps_the_edit_in_the_history`</sub>
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
- Renumbering an 8-pole terminal block from a given start writes every pin's new label in pin order. <sub>`renumbering_writes_every_terminal_number_from_the_start_value`</sub>
- On a multi-level block, renumbering keeps each pole's level letters and only changes the pole number, so every level of one pole shares the same new number. <sub>`renumbering_a_multilevel_block_keeps_the_level_letters`</sub>
- A prefix and a step greater than one both apply: the step advances the pole number and the prefix is written before every label. <sub>`renumbering_applies_the_step_and_prefix`</sub>
- Renumbering rewrites the block's jumper spec to the new labels in the same command, so a jumper between two poles still names the terminals it actually connects. <sub>`renumbering_rewrites_the_jumpers_to_the_new_labels`</sub>
- Renumbering also rewrites a pin-group range's endpoints to the new labels, since the range names terminals by number. <sub>`renumbering_rewrites_pin_group_ranges_too`</sub>
- After a renumbering, the pin-group bracket is still drawn on the drawing and the design check stays clean: the stored range is matched against the block's displayed terminal numbers. <sub>`a_pin_group_bracket_still_draws_after_renumbering_the_terminals`</sub>
- On a multi-level block, a pin-group range that names one level's terminals still draws its bracket after a renumbering, because the level letters are kept. <sub>`a_multilevel_pin_group_bracket_survives_a_renumbering`</sub>
- A jumper item the editor cannot read (or that names a terminal the block does not have) is carried through a renumbering unchanged, so the design check keeps pointing at it and undo brings the attribute back byte for byte here — this holds whenever the old and new label sets do not overlap (see `undoing_a_renumbering_of_an_unreadable_item_is_not_byte_identical_when_old_and_new_labels_overlap` for the documented exception). <sub>`a_broken_jumper_item_survives_a_renumbering_and_comes_back_unchanged_on_undo`</sub>
- The undo of a renumbering does NOT always restore an unreadable jumper item byte for byte: when the old and new label sets overlap, a token the forward rewrite could not resolve as an old label (because it never existed under the old numbering) can become resolvable as a new label on the reverse pass, so the reverse rewrite touches an item the forward rewrite left alone. This is a documented corner case (docs/05-wire-management), not a bug this task fixes. <sub>`undoing_a_renumbering_of_an_unreadable_item_is_not_byte_identical_when_old_and_new_labels_overlap`</sub>
- Undoing a renumbering restores a pin-group spec byte for byte, spacing included: the rewrite only replaces the terminal-number tokens it recognises. <sub>`undoing_a_renumbering_restores_the_pin_group_spec_byte_for_byte`</sub>
- A chain of jumpers over three terminals follows a renumbering with a step of two: both endpoints of every link get the new label and the links stay a chain. <sub>`a_jumper_chain_follows_a_renumbering_with_a_step`</sub>
- Renumbering a symbol that is not a terminal block is refused: the terminal-number override would rewrite that symbol's pin-number texts on the drawing. <sub>`renumbering_a_symbol_that_is_not_a_terminal_block_is_refused`</sub>
- Terminal numbers are stored with the surrounding spaces removed, so what the command validated is exactly what the chart reads back. <sub>`terminal_numbers_are_stored_without_their_surrounding_spaces`</sub>
- Two labels that differ only in their surrounding spaces count as a duplicate and are refused: they would collapse to the same terminal number in storage. <sub>`terminal_numbers_that_are_duplicates_once_trimmed_are_refused`</sub>
- A terminal number containing a comma is refused: the comma is the separator of the stored list, so the value would read back as a different number of terminals. <sub>`a_terminal_number_containing_a_comma_is_refused`</sub>
- Undoing a renumbering restores the previous terminal numbers exactly: a prior direct override comes back, and renumbering from the symbol's defaults undoes to no override at all. <sub>`undoing_a_renumbering_restores_the_previous_numbers`</sub>
- Undoing a renumbering restores an explicit override even when that override's value happened to equal the symbol's own default labels: the key was present, so it comes back, instead of being treated as "no override" and dropped (Task 3 ルーリング3a: had_overrideの判定はキーの有無と妥当性で決め、展開結果が既定と同じかどうかでは決めない). <sub>`undoing_a_renumbering_restores_an_override_that_matches_the_default_value`</sub>
- Setting a number list whose length does not match the pin count is refused and leaves the drawing untouched. <sub>`a_terminal_number_count_mismatch_is_refused_without_changing_the_drawing`</sub>
- A duplicate label in the number list is refused without changing the drawing. <sub>`duplicate_terminal_numbers_are_refused`</sub>
- An empty label in the number list is refused without changing the drawing. <sub>`an_empty_terminal_number_is_refused`</sub>
- Setting the numbers to `None` after a direct override returns the block to the symbol's default numbers (the `terminal_numbers` attribute itself disappears). <sub>`setting_numbers_to_none_returns_to_the_defaults`</sub>
- Renumbering an entity id that does not exist on the sheet is refused instead of silently doing nothing. <sub>`renumbering_an_unknown_entity_is_refused`</sub>
- The renumbering command is plain JSON with sensible defaults for the start and step, so agents and the CLI can send it with just an entity id. <sub>`the_terminal_renumbering_command_is_plain_json`</sub>
- Turning a single-level 4-pole terminal block into 2 levels rewrites its symbol_id and leaves the terminal_numbers attribute unset (no explicit override existed). <sub>`setting_levels_turns_a_single_level_block_into_two_levels`</sub>
- Undoing a level increase (or setting levels back to 1 directly) restores the plain single-level symbol_id, never a "_1l" alias. <sub>`setting_levels_back_to_one_restores_the_plain_single_level_id`</sub>
- Increasing levels expands an explicit terminal_numbers override into one lettered label per level, pole-major, exactly as the plan's example describes ("11,12,…" -> "11a,11b,12a,12b,…"). <sub>`increasing_levels_expands_an_explicit_terminal_numbers_override`</sub>
- A jumper written without a level letter on a single-level block moves to level a when the block gains a second level, so the existing connection is not silently dropped. <sub>`increasing_levels_moves_a_levelless_jumper_to_level_a`</sub>
- Decreasing levels is refused, and the drawing is left completely untouched, when a wire is still connected to a pin of the level that would vanish. <sub>`decreasing_levels_is_refused_when_a_wire_is_connected_to_a_vanishing_level`</sub>
- Decreasing levels is refused when a jumper still names a terminal of the level that would vanish. <sub>`decreasing_levels_is_refused_when_a_jumper_names_a_vanishing_level`</sub>
- Decreasing levels is refused when a pin-group range spans terminals of the level that would vanish. <sub>`decreasing_levels_is_refused_when_a_pin_group_spans_a_vanishing_level`</sub>
- Decreasing levels is refused when an accessory spec names a terminal of the level that would vanish (spec §1.1 accessories format, checked ahead of its own command landing in a later task). <sub>`decreasing_levels_is_refused_when_an_accessory_names_a_vanishing_level`</sub>
- When nothing references the vanishing level, decreasing levels succeeds and both the symbol_id and the terminal-number override drop that level's labels. <sub>`decreasing_levels_succeeds_and_drops_the_vanishing_levels_labels`</sub>
- Changing the level count keeps level a's terminals at exactly the same coordinates on the sheet, whatever the block's rotation or mirroring, so nothing that was drawn onto those terminals has to move. <sub>`a_level_change_keeps_level_a_terminals_at_the_same_sheet_coordinates`</sub>
- A wire drawn to terminal 1 of a single-level block is still electrically on the same terminal (now called 1a) after a second level is added, and it is not re-mapped to the new level's terminal. <sub>`a_wire_on_terminal_one_stays_on_level_a_after_adding_a_level`</sub>
- Decreasing levels is refused when the only thing sitting on a vanishing level's pin is a net label (no wire at all), the same coordinate-coincidence rule the netlist uses. <sub>`decreasing_levels_is_refused_when_a_net_label_sits_on_a_vanishing_pin`</sub>
- Going from 3 levels to 2 is refused while a jumper still names a level-c terminal, and allowed once it is gone - then only level c's labels are dropped and the remaining jumper is kept. <sub>`decreasing_three_levels_to_two_is_refused_only_while_level_c_is_referenced`</sub>
- Increasing levels rewrites the terminal label of an accessory spec the same way it rewrites jumpers, so an accessory pinned to terminal 1 follows to 1a; the kind and part number are left byte for byte. <sub>`increasing_levels_rewrites_accessory_terminal_labels`</sub>
- A terminal_numbers value that is present but invalid (its item count does not match the pin count) survives a level change untouched and comes back byte for byte after undo; the command never deletes a key it cannot restore. <sub>`a_present_but_invalid_terminal_numbers_value_survives_a_level_change`</sub>
- A level change whose relabelled bases would collide (one pole's level-a label ends in level a's own letter and strips to the same base as another pole's level-a label that never had a letter to strip) is refused before anything is written, instead of silently storing a `terminal_numbers` value with duplicate labels. <sub>`a_level_change_that_would_produce_duplicate_labels_is_refused_without_writing_anything`</sub>
- Undoing a level change restores the previous symbol_id and the instance position together, in one undo step, so the terminals land back on their original coordinates. <sub>`undoing_a_level_change_restores_the_symbol_id_and_the_position_together`</sub>
- Asking for a level count outside 1-3 is refused without touching the drawing. <sub>`setting_levels_outside_one_to_three_is_refused`</sub>
- Changing the level count of a symbol that is not a terminal block is refused, the same gate RenumberTerminals/SetTerminalNumbers already use. <sub>`setting_levels_on_a_symbol_that_is_not_a_terminal_block_is_refused`</sub>
- A symbol_id spelled with the "_1l" one-level alias is refused when placed on the drawing: a single-level terminal block has exactly one valid spelling of its symbol_id. <sub>`a_one_level_alias_symbol_id_is_refused_when_placed`</sub>
- Growing a terminal block's pole count (as inserting a spare does) never moves an existing pin's sheet coordinates — the instance is shifted the opposite way to compensate — so no wire that was attached to it comes loose. <sub>`inserting_a_spare_terminal_keeps_every_existing_pins_coordinates`</sub>
- Inserting a spare after a chosen terminal writes the renumbered label sequence and rewrites the jumper spec that used the labels which moved, so the jumper still names the same physical pair of terminals. <sub>`inserting_a_spare_after_a_terminal_renumbers_labels_and_follows_the_jumper`</sub>
- A count of zero is refused without touching the drawing. <sub>`inserting_zero_spares_is_refused_without_touching_the_drawing`</sub>
- Naming a terminal the block does not have as the insertion point is refused. <sub>`inserting_after_an_unknown_terminal_is_refused`</sub>
- Inserting past the dynamic symbol family's pole cap is refused. <sub>`inserting_past_the_pole_cap_is_refused`</sub>
- Inserting a spare into a symbol that is not a terminal block is refused, the same gate the other terminal commands already use. <sub>`inserting_a_spare_on_a_symbol_that_is_not_a_terminal_block_is_refused`</sub>
- Undoing an inserted spare restores the symbol_id, the terminal numbers and the position together in one step. <sub>`undoing_an_inserted_spare_restores_symbol_id_numbers_and_position_together`</sub>
- A multi-level block's inserted spare pole adds one terminal per level ("9a","9b" for a 2-level block). <sub>`inserting_a_spare_on_a_multilevel_block_adds_one_terminal_per_level`</sub>
- The insertion command's JSON with `after` omitted (the minimal form MCP/the Link API/the CLI can send) is accepted as an append at the end. <sub>`the_insert_spare_terminals_command_accepts_after_omitted`</sub>
- Inserting a second spare into a block that already has one works, both at the end and after any terminal, because the command counts the insertion point in the order the chart shows rather than the physical pin order. <sub>`inserting_a_second_spare_into_the_same_block_succeeds`</sub>
- Inserting a spare into a block renumbered with a prefix keeps the prefix on every label and does not panic (the prefixed labels are not readable as plain pole numbers, which used to overflow the numbering arithmetic). <sub>`inserting_a_spare_into_a_prefixed_block_keeps_the_prefix`</sub>
- The "leave the spare with a vacant number" mode writes the spare into the vacant number and leaves every following terminal's number (and so every jumper naming them) untouched. <sub>`inserting_a_spare_without_renumbering_leaves_the_following_numbers_alone`</sub>
- In the "leave the numbers alone" mode an insertion whose spare number is already taken is refused without touching the drawing. <sub>`inserting_a_spare_without_renumbering_is_refused_when_the_number_is_taken`</sub>
- Inserting a spare keeps every existing terminal at exactly the same coordinates on the sheet whatever the block's rotation or mirroring, so nothing drawn onto those terminals has to move. <sub>`inserting_a_spare_keeps_pin_coordinates_at_every_rotation_and_mirroring`</sub>
- Changing a sheet's paper size and orientation (G15) keeps every existing entity at its original coordinates; nothing is moved or deleted. <sub>`changing_sheet_paper_keeps_entity_coordinates`</sub>
- Undoing a paper size change restores both the previous size and orientation. <sub>`undoing_a_paper_change_restores_size_and_orientation`</sub>
- Changing the paper leaves the sheet's own zone division alone: a sheet divided into its own number of zones keeps that division. <sub>`changing_paper_keeps_the_sheets_own_zone_division`</sub>
- Undoing a paper change puts the sheet back exactly as it was, so a cross-reference zone address computed on it points at the same zone as before. <sub>`undoing_a_paper_change_restores_the_zone_addresses`</sub>
- A paper size change on a sheet that does not exist is refused. <sub>`changing_paper_on_an_unknown_sheet_is_refused`</sub>
- `set_project_meta` sets the project name and number in one command, leaving every sheet's title block untouched when those fields are omitted. <sub>`set_project_meta_sets_name_and_number`</sub>
- `set_project_meta` writes the title-block fields (company, drawing number, title) to every sheet in the project with one undoable command. <sub>`set_project_meta_writes_title_block_to_every_sheet`</sub>
- Undoing `set_project_meta` restores each sheet's own previous title block, even when the sheets had different values beforehand (not a single shared value). <sub>`undoing_set_project_meta_restores_each_sheets_own_title_block`</sub>
- The project number command is plain JSON, so agents, the CLI and the Link API can all send it. <sub>`set_project_meta_command_is_plain_json`</sub>
- Undoing the very first project number puts the project back to having no number at all, not to an empty one. <sub>`undoing_the_first_project_number_restores_the_unset_state`</sub>
- Sending the project number as null clears it back to unset, which is how a number set by mistake is removed. <sub>`a_null_project_number_clears_it`</sub>
- Undoing a cleared project number brings the previous number back exactly as it was. <sub>`undoing_a_cleared_project_number_brings_it_back`</sub>
- Leaving the project number out of the command leaves an existing number untouched, even while other fields change. <sub>`omitting_the_project_number_leaves_it_untouched`</sub>
- Registering a FreeCAD link stores it under the entity id, registering again replaces it, removing it takes it away, and each step undoes back to the previous list. <sub>`mech_links_are_upserted_removed_and_undone`</sub>
- Writing measured wire lengths back sets each wire's length and marks it as measured by FreeCAD in one undo step; a write-back that changes nothing costs no undo step at all. <sub>`wire_lengths_write_back_with_their_source`</sub>
- A write-back naming a wire that is not on the sheet changes nothing: the other wires of the same batch keep their lengths and no undo step is created. <sub>`wire_lengths_write_back_is_all_or_nothing`</sub>

### DXF interop (AutoCAD Electrical / EPLAN)

- The exported DXF is an AutoCAD 2000 (AC1015) file in millimetres with HEADER, TABLES, BLOCKS, ENTITIES and OBJECTS sections and unique entity handles. <sub>`export_writes_a_well_formed_ac1015_file`</sub>
- Wires are written as one LINE per segment on the WIRES layer with the Y axis flipped to DXF's upward convention. <sub>`wires_become_lines_on_the_wires_layer_with_y_flipped`</sub>
- Each symbol becomes an INSERT of a block named MDK_<symbol_id> (defined once) with a negative X scale for mirroring and TAG1/CAT/DESC1/RATING1/TERMnn attributes. <sub>`symbols_become_block_inserts_with_attributes`</sub>
- Wire numbers go to the WIRENO layer, net labels to LABELS, notes to MISC, and a harness becomes a closed dashed polyline on HARNESS with its name. <sub>`texts_and_harness_go_to_their_layers`</sub>
- Non-ASCII text is written as \U+XXXX escapes and decoded back; MTEXT paragraph breaks and formatting codes are handled on read. <sub>`unicode_is_escaped_and_decoded`</sub>
- Exporting a sheet to DXF and importing it back keeps the paper, symbols (id, position, rotation, mirror, reference, value, attributes), wires, junctions, wire numbers, labels, notes and harness names. <sub>`export_then_import_round_trips_the_sheet`</sub>
- Importing an AutoCAD Electrical style DXF reads lines on wire layers (WIRES, _MULTI_WIRE_*) as wires, assigns WIRENO texts to the nearest wire, keeps other lines and unknown blocks as skipped items, and decodes text. <sub>`imports_acade_style_wires_and_reports_unknown_blocks`</sub>
- When the import options name the wire layers explicitly, only those layers become wires. <sub>`explicit_wire_layers_limit_what_becomes_a_wire`</sub>
- When the named wire layers hold no line at all, no wire is imported and the warning lists the layers the file really has (the explicit narrowing never widens into "every line is a wire"). <sub>`explicit_wire_layers_that_match_nothing_import_no_wire`</sub>
- A small circle becomes a junction only on a wire layer: under the "every line is a wire" fallback the pin circles of a foreign drawing stay skipped instead of silently merging nets. <sub>`small_circles_become_junctions_only_on_wire_layers`</sub>
- A DXF without any wire layer reads every line as a wire and says so in a warning; a WIRENO text with no wire nearby becomes a note. <sub>`without_wire_layers_all_lines_become_wires`</sub>
- The paper size and origin come from the header drawing limits ($LIMMIN/$LIMMAX, else $EXTMIN/$EXTMAX) when the file has them, so an A3 drawing whose content only covers a corner still imports as A3 at the same coordinates. <sub>`header_limits_decide_the_paper_and_origin`</sub>
- A justified TEXT (group 72 or 73 non-zero) is placed at its alignment point in groups 11/21, not at the ignored group 10/20. <sub>`justified_text_is_placed_at_its_alignment_point`</sub>
- A DXF in inches is converted to millimetres, and the paper size is chosen as the smallest ISO A size that fits the drawing extents. <sub>`inch_files_are_scaled_and_paper_fits_extents`</sub>
- A block insert rotated by an angle that is not a multiple of 90 degrees is placed at 0 degrees with a warning. <sub>`non_right_angle_rotation_is_rounded_with_a_warning`</sub>
- A file without an ENTITIES section is rejected as not a DXF, and a group code that is not a number is a syntax error with its line number. <sub>`rejects_non_dxf_and_reports_syntax_errors`</sub>
- Symbol block graphics are written in local coordinates with Y flipped; arcs keep their sweep because the clockwise paper-space direction becomes counter-clockwise in DXF. <sub>`block_graphics_are_y_flipped_and_arcs_keep_direction`</sub>
- The frame goes to the FRAME layer: the border, the zone ticks with their letters and numbers, and the title block cells with their binds resolved (drawing number, title, revision). <sub>`the_frame_and_title_block_go_to_the_frame_layer`</sub>
- Shapes, the wire part legend and the panel layout entities are exported too, each on its own layer (SHAPES / ANNOTATION / PANEL), and the export report counts what was written per kind. <sub>`shapes_legends_and_panel_layout_are_exported_on_their_own_layers`</sub>
- A symbol whose definition cannot be resolved is reported in the export report instead of disappearing without a word. <sub>`an_unresolvable_symbol_is_reported_as_skipped`</sub>
- A rotated or mirrored symbol's pins land on exactly the paper positions the model gives them, so the exported pin still meets the wire drawn to it, and reading the file back gives the same rotation and mirror. <sub>`rotated_symbol_pins_meet_their_wires_in_the_exported_dxf`</sub>

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
- Outside a project (a lone sheet) the project number bind is known but empty, since there is no project to read it from. <sub>`the_project_number_bind_is_empty_outside_a_project`</sub>
- Inside a project, the project number bind reads `Project::no` (G16), the same field `set_project_meta` writes. <sub>`the_project_number_bind_reads_the_project_number`</sub>
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
- The frame editor saves a user frame as {id}.json in the user folder, and the list then shows it as a user frame with the edits. <sub>`the_frame_editor_saves_a_user_frame_that_lists_back`</sub>
- Saving under the id of a bundled frame is refused and nothing is written, so the user must save under another name. <sub>`saving_over_a_bundled_frame_id_is_refused`</sub>
- A broken frame (revision columns not adding up to the title block width) or an id that cannot be a file name is refused and nothing is written. <sub>`an_invalid_frame_or_id_is_not_saved`</sub>

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
- A project file from the version that introduced shapes still opens with its shapes intact, so a drawing made back then looks the same today. <sub>`a_project_file_from_the_version_that_introduced_shapes_keeps_them`</sub>
- A project file saved before the note leader line existed opens with every note's leader target unset, so no note draws a leader line, and is brought up to the current format version. <sub>`an_old_project_file_opens_with_no_leader_targets`</sub>
- A shape drawn on a sheet survives a save and reload with its kind, points and look unchanged. <sub>`a_shape_survives_a_save_and_reload`</sub>
- A panel sheet and the panel entities on it survive a save and reload unchanged. <sub>`a_panel_sheet_survives_a_save_and_reload`</sub>
- A project file saved before text styling existed opens with every note unstyled (left-aligned, default colour, not bold) and is brought up to the current format version. <sub>`an_old_project_file_opens_with_unstyled_notes`</sub>
- The current file format version is 17, the version that added single-line cable runs (saved as "run_cable"). <sub>`the_current_format_version_is_seventeen`</sub>
- A cable with a single-line run is saved under the entity kind "run_cable" and comes back with its run and core ends; such a file has no entity of kind "cable". <sub>`a_run_cable_is_saved_as_run_cable_and_round_trips`</sub>
- A "run_cable" entry whose run is empty (for example after hand-editing the file) opens as a plain definition-line cable and is written back with the kind "cable". <sub>`a_run_cable_without_a_run_opens_as_a_plain_cable`</sub>
- A file saved by a newer MadakeCAD (a higher format version) is refused with a message naming both versions, instead of being opened with parts missing. <sub>`a_file_from_a_newer_app_is_refused`</sub>
- A cable with its definition line, core assignments, shield and length survives a save and reload unchanged. <sub>`a_cable_round_trips_through_a_saved_file`</sub>
- A version-13 file, written before PLC drawing settings could be saved, opens with no saved settings. <sub>`a_version_13_file_opens_without_saved_plc_settings`</sub>
- A project file saved before tidy variants existed opens with every sheet an ordinary sheet (no variant mark), and is brought up to the current format version. <sub>`an_old_project_file_opens_with_no_variant_marks`</sub>
- A project file saved before the wire part-number legend block existed opens with no legend blocks at all, and is brought up to the current format version. <sub>`an_old_project_file_opens_without_any_wire_legends`</sub>
- A wire part-number legend block placed on a sheet survives a save and reload with its place unchanged. <sub>`a_wire_legend_survives_a_save_and_reload`</sub>
- A project file saved before the project number existed opens with no project number set, so the `{project.no}` title-block bind stays blank exactly as before, and is brought up to the current format version. <sub>`an_old_project_file_opens_with_no_project_number`</sub>
- A drawing saved before the annotation text height existed opens at the default 2.0mm, so its notes look exactly as before. <sub>`an_old_project_file_opens_with_the_default_annotation_text_height`</sub>
- The annotation text height set on a sheet is saved to the file and comes back on reload. <sub>`the_annotation_text_height_survives_a_save_and_reload`</sub>
- A panel drawn before item numbers existed opens with no balloons at all: the footprints keep their place and can be numbered whenever the drawing is next tidied up. <sub>`an_old_panel_file_opens_without_item_numbers`</sub>
- The frame template chosen for a project is saved to the file and comes back on reload. <sub>`the_frame_template_survives_a_save_and_reload`</sub>
- A format 2 file (no FreeCAD links, wires without a length source) opens with an empty link list and every wire length marked as manual. <sub>`a_format_2_file_opens_with_empty_mech_links_and_manual_lengths`</sub>
- A format 3 file whose FreeCAD links have no placement opens with those links unplaced, and a link with a placement round-trips through save and load. <sub>`a_format_3_file_opens_with_unplaced_links_and_placements_round_trip`</sub>
- A file written by the cloud build (its own format 4, with FreeCAD links and a wire length measured in FreeCAD) opens with those fields intact and is rewritten as the current merged format version. <sub>`a_cloud_format_4_file_keeps_its_mech_links_and_length_source`</sub>

### KiCad import / export

- KiCad import converts paper size, title block, wires, junctions, labels (power symbols become net labels) and text. <sub>`imports_paper_title_block_and_geometry`</sub>
- Known lib_ids map to our symbols (Device:R -> resistor, Conn_01x03 -> connector_3p) keeping designator/value/rotation/mirror; unknown symbols are skipped and itemized in the report. <sub>`maps_symbols_and_reports_skipped`</sub>
- A file that is not a kicad_sch document is rejected with a clear error. <sub>`rejects_non_schematic`</sub>
- The S-expression parser reads atoms, quoted strings, numbers and nested lists with typed accessors. <sub>`parses_atoms_strings_numbers_and_nesting`</sub>
- Escaped quotes/newlines and multibyte (Japanese) text inside strings parse correctly. <sub>`parses_escaped_strings_and_multibyte`</sub>
- children(name) iterates every child list with the given head symbol. <sub>`children_iterates_all_matches`</sub>
- Unbalanced parentheses and unterminated strings are reported as syntax errors with a position. <sub>`syntax_errors_are_reported`</sub>
- The KiCad export report counts the electrical kinds it wrote and names the kinds it does not write at all (shapes, the wire part legend and the panel layout entities), so nothing is dropped without a word. <sub>`the_kicad_export_report_names_the_kinds_it_cannot_write`</sub>
- Exporting a sheet to .kicad_sch and importing it back keeps paper, title block, symbols (id, reference, value, rotation, mirror), wires, junctions, labels and text. <sub>`export_then_import_round_trips_the_sheet`</sub>
- A wire drawn as a polyline is exported as one two-point KiCad wire per segment. <sub>`polyline_wires_are_split_into_segments`</sub>
- Every symbol used on the sheet is embedded once in lib_symbols with its graphics and pins, so KiCad opens the file without MadakeCAD libraries. <sub>`used_symbols_are_embedded_in_lib_symbols`</sub>
- A wire number becomes a KiCad label on the numbered net, so the net keeps its name in KiCad and on re-import. <sub>`wire_numbers_become_labels`</sub>
- A harness boundary is exported as a dashed closed polyline plus a text with its name. <sub>`harness_becomes_dashed_polyline_with_name`</sub>
- Quotes, backslashes and newlines in text are escaped so the exported file parses and reads back unchanged. <sub>`strings_are_escaped_and_read_back`</sub>
- Importing a schematic exported by MadakeCAD resolves "MadakeCAD:<id>" library ids directly, including parametric terminal blocks and connectors. <sub>`madakecad_lib_ids_resolve_directly_on_import`</sub>

### leader

- A leader line drawn from a note above its target starts at the midpoint of the note's bottom edge and ends at the nearest point on the target's top edge. <sub>`a_leader_above_its_target_runs_from_the_bottom_edge_to_the_top`</sub>
- A leader line drawn from a note beside its target runs horizontally from the note's near vertical edge to the target's bounding box. <sub>`a_leader_beside_its_target_runs_horizontally`</sub>
- leader_for follows a note that is moved: recomputing after the note's position changes yields a different geometry without touching the model. <sub>`leader_for_recomputes_after_the_note_moves`</sub>
- leader_for returns None when leader_to is unset, so an ordinary note draws no leader line. <sub>`leader_for_is_none_without_a_leader_target`</sub>
- A leader aimed at a symbol ends on exactly the outline the drawing shows: the placement point is always inside it, and a text mark inside the symbol counts as its own reference point, not as a box of letters. <sub>`a_leader_targets_exactly_the_outline_the_drawing_shows`</sub>
- A leader aimed at a symbol carrying pin-group brackets stops outside the caption beside them, not on the symbol body, because the brackets and captions are drawn with the symbol and a line ending on top of them would be harder to read. <sub>`a_leader_stops_outside_a_symbols_pin_group_caption`</sub>
- entity_bounds resolves a symbol instance's bounding box from its definition, so a leader can target a symbol. <sub>`entity_bounds_resolves_a_symbol_instance`</sub>

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
- Saving a selection under an existing macro with a new variant key adds that variant and keeps the default and the other variants as they were. <sub>`saving_with_a_new_variant_key_adds_a_variant_to_the_existing_macro`</sub>
- Saving under a variant key that is already registered replaces only that variant; key A replaces the default circuit. <sub>`saving_over_a_registered_variant_replaces_only_that_variant`</sub>
- A variant key must be a single letter from A to Z; an empty key, two letters or a digit is refused. <sub>`a_variant_key_must_be_one_letter`</sub>
- A placeholder follows a new variant to the symbol with the same designator, so inserting that variant with a value set fills it in too. <sub>`placeholders_follow_a_new_variant_by_designator`</sub>
- Without a variant key, or when no existing macro is given, saving makes a new macro exactly as before. <sub>`saving_without_a_variant_or_existing_macro_makes_a_new_macro`</sub>
- A harness copied with the cable selected before its wires and part is saved with the cable last, pastes in one edit, and the pasted cable's cores point at the pasted wires and part. <sub>`a_cable_selected_first_pastes_after_its_wires`</sub>

### mate

- Two connectors that declare each other as their mate, with matching pole counts, produce no ERC finding at all. <sub>`a_reciprocated_matching_pair_has_no_erc_findings`</sub>
- A connector that declares a mate with a different pole count gets a pole-mismatch warning. <sub>`a_pole_count_mismatch_is_a_warning`</sub>
- A connector that declares a mate the other side does not declare back gets an informational one-sided-declaration finding. <sub>`a_one_sided_declaration_is_info`</sub>
- Declaring an unknown reference designator as a mate is a warning, and so is declaring oneself. <sub>`an_unknown_or_self_reference_is_a_warning`</sub>
- A connector with no `mates_with` attribute produces no mate-related finding at all. <sub>`a_connector_with_no_declaration_is_silent`</sub>
- A terminal block, even though it shares the "connector" category, is not treated as a mating connector (no mate diagnostics, even with the attribute set). <sub>`a_terminal_block_is_not_a_mating_connector`</sub>
- Two mated connectors on the same sheet within 50mm produce exactly one drawn mate link. <sub>`a_close_pair_on_one_sheet_produces_one_mate_link`</sub>
- Two mated connectors more than 50mm apart on the same sheet produce no drawn mate link (the reports still carry the pair). <sub>`a_far_pair_produces_no_mate_link`</sub>
- The mate declared on a connector instance is what the wire list / from-to report's mate column reads. <sub>`declared_mate_reads_the_attribute`</sub>

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
- A run cable lists the wires and parts its cores land on as in-sheet references, and remapping (sheet duplication) points every core end at the copies; ends with no copy are dropped. <sub>`a_run_cable_remaps_its_core_ends`</sub>
- Moving a run cable moves every point of its single-line run. <sub>`translating_a_run_cable_moves_its_run`</sub>

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
- The LTE harness (four board wires breaking out of a single line that lands on the USB plug) gives four separate nets, each joining one board pin to the matching plug pin through its cable core. <sub>`a_run_cable_keeps_each_core_its_own_net`</sub>
- A single line lying across every pin of a connector joins none of them: a cable's run never connects by position. <sub>`a_run_over_a_pin_column_joins_nothing`</sub>
- A single-line cable between two connectors (3.5 mm AUX: L, R, GND) gives one net per core even though no wire is drawn. <sub>`a_connector_to_connector_run_gives_one_net_per_core`</sub>
- Nets made only of cable cores are named after the nets that have wires, so adding such a cable does not rename existing nets. <sub>`core_only_nets_are_named_after_wire_nets`</sub>
- Moving the part a single line lands on keeps every core connected to the same pin: connectivity follows the assignment, not the drawing. <sub>`moving_the_landing_part_keeps_core_connectivity`</sub>
- Two port pins joined by one wire give one net per contact, named N001.<contact> in contact order, each holding both pins with that contact and the shared wire. <sub>`two_ports_joined_by_one_wire_split_into_one_net_per_contact`</sub>
- The unsplit view keeps one net for the USB wire, and a drawing without ports gives exactly the same nets (and JSON, with no contact keys) from both functions. <sub>`without_ports_nothing_changes`</sub>
- A mate half passes every contact through inside its own net, but plug and socket are still not joined: port → socket | plug → label … port gives two groups of contact nets. <sub>`a_mate_half_passes_contacts_through_but_does_not_conduct`</sub>
- In a port net a plain pin named like a contact joins that contact; a plain pin with another name goes to a residual net named like the base and is listed in the port report. <sub>`a_plain_pin_joins_the_contact_with_its_name`</sub>
- The LTE harness drawn as four board wires into one run that ends on the single port pin 39.U3 gives four nets, each one board pin and one contact of the port. <sub>`the_lte_harness_lands_on_one_port_pin`</sub>
- A label named <port label>.<contact> on a plain wire joins that one contact of the port net. <sub>`a_dotted_label_taps_one_contact`</sub>
- A single-line run between two port pins with no wire at all still gives one net per core, each with both ports. <sub>`a_run_between_two_ports_gives_one_net_per_core`</sub>
- Two ports with different contact sets join only the shared contacts; the others stay alone on their port, with no residual net, and the report lists them. <sub>`ports_with_different_contacts_join_only_the_shared_ones`</sub>
- When a core joins two port clusters, the contact net takes the base name a cluster already has from an earlier net, and no new number is used for it. <sub>`a_contact_net_across_two_port_clusters_reuses_the_earlier_base`</sub>
- A net pin prints as REF:PIN, with /CONTACT added when it carries a port contact. <sub>`a_net_pin_prints_with_its_contact`</sub>

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
- A nameplate tied to a footprint sits centred under it with a small gap, or above it when asked; a standalone one stays where it was put. <sub>`a_tied_nameplate_sits_centred_under_its_footprint`</sub>
- When it follows the description, a tied nameplate's first line is the schematic description of the same designator; otherwise it keeps its own text. <sub>`a_nameplate_can_follow_the_schematic_description`</sub>
- A dimension between two footprints measures the distance between their centres along its axis, and follows when a footprint moves. <sub>`a_dimension_measures_between_footprint_centres_and_follows_them`</sub>
- Moving a dimension moves its line (the measured ends stay on their parts). <sub>`moving_a_dimension_moves_its_line`</sub>
- Nameplates with a part number are counted per part number in the BOM. <sub>`nameplates_are_counted_in_the_bom`</sub>
- The printed panel sheet draws the nameplate text and the dimension value. <sub>`the_printed_panel_sheet_draws_nameplates_and_dimensions`</sub>
- A nameplate tied to a footprint that is not on the sheet, or a dimension end pointing at a missing part, is refused. <sub>`nameplates_and_dimensions_must_point_at_footprints_on_the_sheet`</sub>
- Aligning moves the other selected footprints so their edge or centre lines up with the first one, in one undoable command. <sub>`aligning_lines_footprints_up_with_the_first_one`</sub>
- Vertical alignment leaves a footprint mounted on a DIN rail where it is, so nothing is pulled off its rail. <sub>`vertical_alignment_keeps_rail_mounted_footprints_on_their_rail`</sub>
- Distributing spaces the footprints evenly between the outer two, or at a given centre pitch from the first. <sub>`distributing_spaces_footprints_evenly_or_at_a_pitch`</sub>
- Aligning fewer than two footprints, distributing fewer than three without a pitch, or a non-positive pitch is refused. <sub>`too_few_footprints_or_a_bad_pitch_is_refused`</sub>

### panel_wiring

- The panel wiring lists, for each part on a panel sheet, every wire on its pins in the schematic: pin, wire number, the other end, colour and gauge. <sub>`the_panel_wiring_lists_each_pin_wire_with_its_destination`</sub>
- A run-cable breakout wire on a pin is listed with its core's colour and the cable's gauge, not blank. <sub>`a_breakout_wire_is_listed_with_the_core_colour_and_cable_gauge`</sub>
- The panel wiring sheet draws each part with its terminal rows, and is an empty heading page when no panel has parts. <sub>`the_panel_wiring_sheet_draws_parts_or_an_empty_page`</sub>

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
- An old schema-v6 database migrates to v7 on open, keeping its rows, gaining the terminal kind column, filling the bundled terminal block's kind and adding the terminal accessory samples. <sub>`v6_database_migrates_to_v7_with_terminal_kinds_and_accessory_samples`</sub>
- The BOM lookup table built from the parts database holds the maker, name, price, currency and purchase link of every listed part number it knows, and leaves unknown part numbers out. <sub>`the_bom_catalog_holds_only_registered_part_numbers`</sub>
- A freshly created parts database already holds the terminal block sample with its kind and the three terminal accessory samples. <sub>`a_new_database_has_terminal_kinds_and_accessory_samples`</sub>

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
- A panel-layout sheet is part of the book: exporting the project puts it in the PDF right after the schematic sheets. <sub>`the_pdf_book_includes_the_panel_sheet`</sub>
- The panel placement table can be bound into the PDF book like any other report. <sub>`the_pdf_book_can_include_the_panel_placement_table`</sub>

### pin_groups

- A single "range:caption" item, and a single-pin item with no dash, both parse to one group each. <sub>`a_range_and_a_single_pin_each_parse_to_one_group`</sub>
- A range written back to front (end before start in chart order) is reported as invalid and produces no group. <sub>`a_backwards_range_is_reported_as_invalid`</sub>
- A range naming a pin the instance does not have is reported with the unknown-pin code and produces no group. <sub>`a_range_with_an_unknown_pin_is_reported`</sub>
- Unparseable text (no colon, or an empty range) is reported as invalid instead of panicking, and the rest of the spec still parses. <sub>`unparseable_text_is_reported_without_panicking`</sub>
- Two groups whose chart-order ranges overlap are both reported and neither produces a group, while a third, non-overlapping group is kept. <sub>`overlapping_groups_are_both_rejected`</sub>
- A multi-level terminal block's chart order runs level by level (1a,2a,…,1b,2b,…), so a range like "1a-4a" names a contiguous run within one level. <sub>`a_multilevel_range_within_one_level_parses`</sub>
- Wrapping a caption splits it into chunks of at most 4 characters, in order, approximating vertical writing with stacked horizontal lines. <sub>`wrap_caption_splits_into_chunks_of_four`</sub>
- A bracket over pins on the left side of a symbol is a vertical bar to the left of the pins, spanning from the topmost to the bottommost pin, with its caption further left still. <sub>`a_bracket_on_left_pins_is_a_vertical_bar_further_left`</sub>
- A pin whose own number contains a hyphen cannot be named in a pin group, and says so instead of being reported as a pin that does not exist. <sub>`a_pin_number_containing_a_hyphen_is_refused_with_its_own_reason`</sub>
- On a multi-level terminal block, a group over the second level's terminals brackets the outside of that level's own column, never the gap between the columns. <sub>`a_multilevel_group_brackets_the_outside_of_its_own_column`</sub>
- When a terminal block's numbers are overridden, a pin-group range written with the displayed labels still finds its pins and draws its bracket, and reports no unknown pin. <sub>`a_range_written_with_the_displayed_labels_still_draws_its_bracket`</sub>
- On a multi-level block whose numbers are overridden, a range over one level's terminals brackets that level's own column just as it does without the override. <sub>`a_multilevel_range_still_brackets_its_own_column_after_an_override`</sub>
- A range that spans two levels of a multi-level terminal block is reported instead of quietly drawing nothing, because the two levels face opposite sides and no single bracket can hold them. <sub>`a_range_across_two_levels_is_reported_instead_of_silently_skipped`</sub>
- A bracket spans pins from two different sides of a symbol (mixed pin direction) is refused: the shape would not be a straight bracket. <sub>`a_bracket_across_mixed_pin_directions_is_refused`</sub>

### pin_wiring

- A plug's pin is reported at its paper position, and its direction follows rotation and mirror. <sub>`pin_direction_follows_rotation_and_mirror`</sub>
- A block's named pins are reported with their numbers, names, outside positions and the rounded outline. <sub>`block_pins_and_outline_are_reported`</sub>
- Looking up a reference returns every part that carries it, and an entity id finds exactly that part. <sub>`parts_are_found_by_reference_or_entity_id`</sub>
- A pin specifier finds the pin by number first and by name second, splitting at the last dot. <sub>`a_pin_is_resolved_by_number_then_by_name`</sub>
- A wrong specifier is refused with what the agent needs to fix it: the format, similar references, the part's pins, or the entity ids to choose from. <sub>`a_wrong_specifier_is_refused_with_candidates`</sub>
- Two left/right pins are joined with a Z whose vertical leg sits midway on the 2.5 mm grid. <sub>`horizontal_pins_get_a_z_route`</sub>
- Two up/down pins are joined with a Z whose horizontal leg sits midway. <sub>`vertical_pins_get_a_z_route`</sub>
- A left/right pin and an up/down pin are joined with an L, each end leaving in its own direction. <sub>`mixed_pins_get_an_l_route`</sub>
- Pins that are already aligned get one straight segment, whatever their directions. <sub>`aligned_pins_get_a_straight_wire`</sub>
- Pins off the grid keep their exact end points while the bend snaps to the grid. <sub>`off_grid_pins_keep_their_ends`</sub>
- With via points the wire passes each one in order, adding a horizontal-first corner where two points are not aligned. <sub>`via_points_are_passed_in_order`</sub>
- A mate pair puts the plug and the socket 5 mm apart facing each other, in both orientations and with the male part on either side; both carry the designator and the socket carries the value. <sub>`a_mate_pair_faces_each_other_in_every_layout`</sub>
- A route that would run over a third pin is refused, because the netlist would join that pin to the wire; the message names the pin. <sub>`a_route_that_passes_over_another_pin_is_refused`</sub>
- Wiring a pin to itself is refused, whether it is named by number or by name. <sub>`wiring_a_pin_to_itself_is_refused`</sub>
- When one part has the same pin number twice (a through terminal), the bare number is refused with both choices, and "#n" picks one. <sub>`a_through_terminal_side_is_chosen_with_an_occurrence`</sub>
- A mate pair sits on the 2.5 mm grid and its two halves only touch, so the tidy metrics count no overlap and no off-grid point for it. <sub>`a_mate_pair_is_tidy`</sub>
- The plug half of a mate pair hides its designator, so the shared designator is printed once, on the socket. <sub>`the_plug_hides_the_shared_designator`</sub>
- get_pins shows a port pin's contacts and leaves them out for plain pins. <sub>`part_pins_show_contacts`</sub>

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
- The horizontal-bus style lays the bus along the top and drops one vertical rung per point to the module lying below it, points running left to right; the page passes the ERC. <sub>`the_horizontal_bus_style_drops_vertical_rungs_left_to_right`</sub>
- With "share if it fits", a module that fits in the rungs left on the last generated page goes onto that page instead of a new one. <sub>`share_if_fits_puts_a_small_module_on_the_last_page`</sub>
- With "share if it fits", a module too big for the rungs left goes onto a new page whole. <sub>`share_if_fits_starts_a_new_page_when_the_module_does_not_fit`</sub>
- With "share and split", the rungs left on the last page are filled in whole split units and the rest of the module continues on a new page, each piece drawn as a split module with continuation references; one undo removes everything. <sub>`share_or_split_fills_the_last_page_and_continues_on_a_new_one`</sub>
- With "share and split", a module larger than a whole page is spread over as many pages as it needs, each piece no longer than a page. <sub>`share_or_split_spreads_a_big_module_over_several_pages`</sub>
- Splitting is refused for a split unit of zero, and a start skip that leaves no rung on a page is refused too; the drawing is not touched. <sub>`a_zero_split_unit_or_an_oversized_start_skip_is_refused`</sub>
- Only a page generated with the same ladder style and rung spacing can be shared; otherwise the next module starts a new page. <sub>`a_page_of_another_style_or_spacing_is_not_shared`</sub>
- A rung spacing that is not a positive multiple of the 2.5 mm grid is refused, so generated pages stay on grid. <sub>`an_off_grid_rung_spacing_is_refused`</sub>
- The I/O report lists every point as address, signal name, target, wire number and comment, with the module reference in front for the whole project. <sub>`the_io_report_lists_address_signal_target_wire_number_and_comment`</sub>

### raster

- A sheet becomes a PNG whose size is the paper size times the requested pixels per millimetre. <sub>`a_sheet_becomes_a_png_of_the_paper_size`</sub>
- A region in millimetres crops the image, so the agent can zoom into one part of the sheet. <sub>`a_region_crops_the_image`</sub>
- An image larger than the limit is scaled down to fit, and the scale actually used is reported. <sub>`a_large_image_is_scaled_down_to_the_limit`</sub>
- A region outside the paper or with no area is refused. <sub>`an_empty_region_is_refused`</sub>

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
- When the contact carries a part number of its own, the coil location does not escape onto that text: it goes on past it, to the other side of the outline. <sub>`coil_location_does_not_escape_onto_the_instances_own_value_label`</sub>
- The escape measures the wire note at the height that sheet actually draws it, so a sheet with taller annotation text still moves the coil location out of the way. <sub>`coil_location_measures_the_wire_note_at_the_sheets_own_text_height`</sub>
- A coil is only shown as a physical device block when its "device_block" attribute is exactly "true"; any other value, including empty, keeps the distributed placement. <sub>`device_block_attribute_selects_the_display_mode`</sub>
- The device block's contact image lists each contact with its terminal pair and drawing address, in the same order as the contact map. <sub>`device_block_lists_the_same_contacts_as_the_contact_map`</sub>
- A blank part number produces a device block with no part number line, rather than an empty one. <sub>`device_block_has_no_part_number_line_when_blank`</sub>
- Only the coils whose instance has "device_block" set are collected into the sheet's device blocks; a coil left in the default distributed placement is not. <sub>`sheet_device_blocks_only_includes_flagged_coils`</sub>
- A coil shown as a device block no longer gets the separate hanging contact map, because its contacts are already listed inside the block. <sub>`device_block_coils_are_excluded_from_the_separate_contact_map`</sub>
- The device block's outline is at least as wide as the coil's pin span, so its A1/A2 pins land exactly on the outline's left and right edge and no lead line is needed. <sub>`device_block_layout_is_no_narrower_than_the_pin_span`</sub>
- The device block's minimum width is exactly the relay coil symbol's own pin span, so the narrowest block reaches both of the coil's pins. <sub>`device_block_min_width_is_the_coil_symbols_pin_span`</sub>
- A part number adds a second header row, and pins still reach the outline through a lead when the content widens the box past the pin span. <sub>`device_block_layout_grows_for_the_part_number_and_keeps_the_pins_reachable`</sub>
- The row dividers never sit at the height the coil's leads attach, so the two leads are never joined into one line across the block. <sub>`device_block_rows_keep_clear_of_the_lead_height`</sub>
- However wide the outline grows, an unturned coil's leads stay horizontal and end on the left and right edge at the pin's own height, where its wires come in. <sub>`device_block_leads_run_along_the_pin_axis_however_wide_the_block`</sub>
- A coil turned 90 or 270 degrees sends its leads straight up and down to the top and bottom edge, and they never run through a line of the contact image, however many contacts the block lists. <sub>`a_turned_device_blocks_leads_run_vertically_without_crossing_the_rows`</sub>
- The device block stays upright whatever the rotation: the outline keeps its size and position, and only the pins turn, so the leads reach the turned pins along their own axis. <sub>`device_block_bounds_stay_upright_when_the_instance_turns`</sub>
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
- The framed terminal chart sheet draws its rows in the chart's own row order, so changing `row_order` really moves the rows down the drawing sheet instead of only relabelling them. <sub>`the_terminal_chart_sheet_draws_the_rows_in_the_chart_row_order`</sub>
- Asking for a terminal chart sheet of something that is not a terminal block yields no page at all. <sub>`terminal_chart_sheet_is_none_for_other_entities`</sub>
- The From-To wire list sheet uses the same columns and rows as the CSV report. <sub>`wire_list_sheet_matches_report_rows`</sub>
- The BOM sheet lists every part group with its reference designators and quantity. <sub>`bom_sheet_lists_parts_with_quantity`</sub>
- Exporting the BOM with a parts catalog fills the catalog columns in both the CSV and the PDF, and the CSV row count includes the total row. <sub>`exporting_the_bom_with_a_catalog_fills_the_catalog_columns`</sub>
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
- The BOM has no wire roll-up rows at all when the project has no wires (a project with only components is unaffected). <sub>`bom_has_no_wire_rollup_rows_without_any_wires`</sub>
- After the component rows, the BOM adds one row per wire part number with the project-wide total length (in metres, 2 decimals) in the quantity column. <sub>`bom_rolls_up_wires_by_part_number_after_the_component_rows`</sub>
- Wires whose part number cannot be resolved roll up under a single "(unassigned)" row, instead of being invented or dropped. <sub>`bom_rolls_up_unresolved_wires_under_one_unassigned_row`</sub>
- Each BOM row looks its part number up in the parts catalog and shows the maker, name, unit price, currency, amount (quantity times unit price) and where to buy it. <sub>`bom_fills_maker_name_price_and_amount_from_the_catalog`</sub>
- A part number the catalog does not know keeps its BOM row; only the catalog columns stay empty. <sub>`a_part_missing_from_the_catalog_keeps_its_row_with_empty_catalog_columns`</sub>
- A wire's unit price is per metre: the amount of a wire roll-up row is its total length in metres times the unit price. <sub>`a_wire_roll_up_row_is_priced_per_metre`</sub>
- When at least one row has a price, the BOM ends with one total row per currency, and says how many rows were left out for having no price. <sub>`the_bom_ends_with_a_total_per_currency_noting_rows_without_a_price`</sub>
- The part numbers looked up in the parts catalog are every non-empty part number in the BOM, once each, without the "(unassigned)" wire row. <sub>`the_part_numbers_to_look_up_skip_blanks_and_the_unassigned_wire_row`</sub>
- The BOM CSV has the same eleven columns as the BOM drawing sheet, with the catalog columns filled in. <sub>`the_bom_csv_carries_the_catalog_columns`</sub>
- Terminal block accessories appear in the BOM as one row per part number with how many are used, listing the terminal blocks that use them, after the wire roll-up rows. <sub>`bom_counts_terminal_accessories_by_part_number`</sub>
- An accessory without a part number, or one the terminal check cannot read, adds no row to the BOM. <sub>`bom_skips_accessories_without_a_part_number_or_that_cannot_be_read`</sub>
- When the only wires whose part number cannot be resolved have no length, the BOM adds no "(unassigned) 0.00 m" row. <sub>`bom_adds_no_unassigned_row_when_those_wires_have_no_length`</sub>
- BOM fields containing commas are quoted so the CSV stays valid. <sub>`bom_escapes_fields_with_commas`</sub>
- The wire list is a From-To list: every row starts with the sheet name and the two ends of the wire, followed by the mate and remark columns (G14/G18), and the older column order (sheet, wire number, harness, ...) is no longer produced. <sub>`wire_list_is_a_from_to_list`</sub>
- An end of a wire that lands on a symbol pin is written as "reference:pin number" (for example K1:A1). <sub>`a_wire_end_on_a_pin_is_written_as_reference_and_pin_number`</sub>
- A wire end on a block pin is written with the pin's name (for example CTRL1:OUT1), and the block itself is counted in the parts list. <sub>`a_wire_end_on_a_block_pin_uses_the_pin_name_and_the_block_is_in_the_parts_list`</sub>
- An end of a wire that carries a net label is written as the label name. <sub>`a_wire_end_with_a_net_label_is_written_as_the_label_name`</sub>
- A wire end on a net label resolves through the net to the single device pin on the other side (G18), when exactly one other pin belongs to that net. <sub>`a_label_end_resolves_to_the_one_to_one_device`</sub>
- The net still resolves correctly when it is stitched together purely by two same-named net label instances with no direct wire between them (a chain through two labels), and both wires in the chain independently resolve to the device across the chain. <sub>`the_net_chains_through_two_same_named_labels`</sub>
- When a net label's net has more than one other pin, the label end keeps the net name instead of picking one device, and the remark column marks the row as multi-ended (G18). <sub>`a_label_end_with_multiple_other_pins_keeps_the_label_and_marks_the_remark`</sub>
- A net that is entirely label-to-label with no device pin anywhere leaves the label name in place on both ends and remarks `none` (nothing to reach), which is a different reason from the multi-ended one. <sub>`a_label_to_label_net_with_no_device_keeps_the_label_name`</sub>
- A wire ending on a connector that declares a mate carries the mate's reference designator in the wire list's mate column (G14). <sub>`wire_list_carries_the_declared_mate`</sub>
- A wire whose two ends are both connectors with their own declared mates lists both mates in the same order however the wire was drawn (G14). <sub>`the_mate_column_does_not_depend_on_the_direction_the_wire_was_drawn`</sub>
- A wire between two devices that neither is a connector with a declared mate leaves the mate column empty. <sub>`wire_list_mate_column_is_empty_without_a_declaration`</sub>
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
- A single-line cable gives one From-To row per core, end to end: the board pin reached through the breakout wire, the plug pin the core lands on, the core colour, the cable gauge, length and part, the cable as harness and the "cable/core" column; its breakout wires get no rows of their own. <sub>`a_run_cable_gives_one_wire_list_row_per_core`</sub>
- When a breakout wire ends on a net label and nothing else on the sheet carries that label (the continuation is on another sheet), the core's row shows the label name and the "none" remark. <sub>`a_breakout_wire_ending_on_a_label_shows_the_label`</sub>
- Wires that are cores of a cable (breakout wires of a single-line cable, and wires crossed by a definition line) are not added again to the BOM wire roll-up, since the cable row already counts them. <sub>`cable_core_wires_are_not_counted_as_loose_wire`</sub>
- A static plug or socket carrying a pole number shows it in the wire list (X44A:2) instead of its only pin 1; one without a pole is unchanged. <sub>`wire_list_shows_the_pole_of_a_mate_half`</sub>
- A run-cable core that lands on a port pin shows its contact in the From-To row (39:U3/VBUS). <sub>`a_core_on_a_port_pin_shows_its_contact`</sub>

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
- A relay lists the contacts of its contact configuration that are not drawn yet as unplaced functions, numbered like the unused rows of the contact map, with the symbol to place for each. <sub>`a_relay_lists_its_undrawn_contacts_as_unplaced`</sub>
- A relay without a readable contact configuration, and any device that is not a relay, has no unplaced functions. <sub>`devices_without_a_contact_configuration_have_nothing_unplaced`</sub>
- A terminal block appears as one "terminals" row covering every pole, with the pole count on the device. <sub>`terminal_block_device_shows_its_terminal_range`</sub>
- A multi-level terminal block counts its poles, not its terminals: a 4-pole 2-level block is 4 poles over 2 levels, and its terminal row spans the first to the last terminal number (1a-4b). <sub>`multilevel_terminal_block_device_counts_poles_not_terminals`</sub>
- An ordinary part that has no separable functions shows up as a single "body" row, named after its symbol. <sub>`plain_part_is_a_single_body_row`</sub>
- Devices are keyed by reference designator, so the same designator on several sheets is one device, and symbols without a designator are not devices at all. <sub>`devices_group_by_reference_across_sheets`</sub>
- An empty project has no devices, and the tree is ordered by reference designator. <sub>`device_tree_is_ordered_and_can_be_empty`</sub>
- Every function row knows the sheet and zone to jump to, which is what the navigator and the reference surfer use to reveal it. <sub>`every_function_knows_where_to_jump`</sub>
- Filter names travel over the API as snake_case strings, and unknown names are rejected rather than guessed. <sub>`filter_names_parse_from_the_api`</sub>
- A hit on a single-line cable (which has no definition line) is placed at the start of its run, not at the sheet origin. <sub>`a_run_cable_hit_is_at_its_run`</sub>

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
- A note with no leader target draws exactly as many lines as the frame and title block alone (no leader line is added), exactly like drawings made before the feature existed. <sub>`a_note_without_a_leader_target_draws_no_leader_line`</sub>
- A note placed directly above its target draws a straight line from the midpoint of its bottom edge down to the top edge of the target's bounding box, with a filled dot at the target end, the same dot the wire junction uses. <sub>`a_note_with_a_leader_target_draws_a_leader_line_to_it`</sub>
- A symbol's function label (its third text field) is printed under the symbol outline, centred, alongside the designator and the type number above it. <sub>`the_function_label_is_printed_under_the_symbol`</sub>
- A symbol with no function label is drawn exactly as before, with only its designator and type number. <sub>`a_symbol_without_a_function_label_is_drawn_as_before`</sub>
- The wire number of a horizontal wire is printed in a monospaced font 2.5 mm above the middle of the wire. <sub>`svg_draws_wire_number_above_a_horizontal_wire`</sub>
- The wire number of a vertical wire is printed 2.5 mm to the left of the middle of the wire. <sub>`svg_draws_wire_number_left_of_a_vertical_wire`</sub>
- One net is labelled once, at the middle of its longest segment, however many wires it is drawn with. <sub>`svg_draws_the_wire_number_once_on_the_longest_segment`</sub>
- A net without a wire number gets no number text at all. <sub>`svg_omits_wire_number_for_unnumbered_nets`</sub>
- A harness boundary is drawn as a dashed rectangle (IEC 61082-1 group enclosure) around the wires it holds. <sub>`svg_draws_a_harness_as_a_dashed_rectangle`</sub>
- The harness name is printed just outside the top-left corner of the boundary. <sub>`svg_labels_the_harness_at_its_top_left_corner`</sub>
- Two mated connectors on the same sheet within 50mm are joined by a short dashed line between their outlines (G14). <sub>`svg_draws_a_dashed_line_between_a_mated_pair_on_one_sheet`</sub>
- The mate link of two 3-pole connectors 30mm apart runs between the facing edges of their outlines, at the height of their placement points (G14). <sub>`the_mate_link_runs_between_the_facing_outline_edges`</sub>
- A mated pair exactly 50mm apart is still drawn: the distance limit includes 50mm itself. <sub>`the_mate_link_distance_limit_includes_exactly_fifty_millimetres`</sub>
- A point inside a symbol's outline is pushed out onto the nearest edge of that outline, so the mate link never starts inside the symbol. <sub>`a_point_inside_the_outline_is_pushed_out_to_the_nearest_edge`</sub>
- A mated pair more than 50mm apart on the same sheet draws no line at all (the reports carry the pair instead). <sub>`svg_draws_no_line_for_a_mated_pair_more_than_50mm_apart`</sub>
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
- A symbol whose "hide_reference" attribute is "true" keeps its designator in the data but does not print it, so one half of a mated plug/socket pair does not draw the same designator twice. <sub>`svg_does_not_print_a_hidden_designator`</sub>
- A coil whose "device_block" attribute is "true" is drawn as one outline with its designator, part number and contact image, instead of the small coil symbol and the separate hanging contact map. <sub>`svg_draws_a_device_block_for_a_flagged_coil`</sub>
- However a device-block coil is turned or mirrored, each lead line still starts exactly on one of the coil's A1/A2 pin points, so the wires attached there stay connected. <sub>`a_turned_device_block_keeps_its_leads_on_the_coil_pins`</sub>
- However a device-block coil is turned or mirrored, its table is drawn upright: every line of text stays inside the outline and no two lines are drawn on top of each other. <sub>`a_turned_device_block_keeps_its_lettering_upright_and_inside_the_outline`</sub>
- A device-block coil still prints its function label under the outline, the same as a coil in the ordinary distributed placement. <sub>`a_device_block_keeps_the_function_label_under_its_outline`</sub>
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
- The wire part-number legend block is drawn where it was placed, listing the colour, gauge, part number and summed length of the sheet's wires. <sub>`svg_draws_the_wire_legend_block`</sub>
- A sheet with no wires at all still draws the wire legend block's header row. <sub>`svg_draws_the_wire_legend_header_even_with_no_wires`</sub>
- The wire legend block resolves a blank part number from the project's wire-part table, the same way the wire list does. <sub>`svg_wire_legend_resolves_part_number_from_the_project_table`</sub>
- Exporting one sheet on its own also fills the legend's part numbers, as long as the caller hands over the project's wire-part table. <sub>`svg_wire_legend_resolves_part_number_on_the_sheet_only_export`</sub>
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
- A jumper bar is part of its terminal block's symbol, so it is drawn or hidden with the symbol and never on its own: the export, which has no per-view-class visibility at all, always writes it out, however the block is turned. <sub>`svg_export_always_draws_the_jumper_bar_with_its_symbol`</sub>
- A jumper bar turns with its terminal block, so on a block rotated a quarter turn the bar lies between the rotated terminals instead of staying upright. <sub>`svg_draws_the_jumper_bar_turned_with_a_rotated_terminal_block`</sub>
- The lettering inside a symbol (a motor's "M", for instance) is drawn at the place the symbol definition gives it and stays there whatever the symbol's rotation or mirroring, because the letters themselves are always written horizontally. <sub>`the_lettering_inside_a_symbol_stays_put_when_the_symbol_is_turned`</sub>
- A jumper bar between two renumbered terminals is attributed to the terminals whose printed labels it actually connects, matched by the label text rather than by mere geometric proximity — closing the phase-6 follow-up where a nearest-mark test could not detect swapped terminal numbers. <sub>`a_jumper_bar_is_attributed_to_its_terminals_by_label_text_after_renumbering`</sub>
- A terminal number of one or two characters sits wholly inside its own round mark, centred on it, however the block is turned or mirrored, so a rotated terminal block still reads terminal by terminal. <sub>`a_turned_terminal_block_keeps_a_short_number_inside_its_own_mark`</sub>
- In the exported drawing (SVG/PDF) an unturned terminal block writes each terminal number with its baseline exactly half the character height (1.0 mm at the 2.0 mm terminal-number height) below the centre of its own mark, single-level and multi-level blocks alike; the on-screen canvas uses a smaller offset of its own. <sub>`a_terminal_number_sits_half_a_character_below_its_mark_centre`</sub>
- A three-character terminal number (a multi-level block's "10a") is as wide as its own mark, so it is centred on the mark and hangs at most three tenths of a millimetre over it, while still never running into a jumper bar or into a neighbouring terminal's mark. <sub>`a_three_character_terminal_number_hangs_at_most_a_little_over_its_own_mark`</sub>
- `attrs["terminal_numbers"]` overrides which label is drawn on each mark: the symbol definition's raw pin number text never appears, and every overridden label appears exactly once. <sub>`svg_draws_the_overridden_terminal_number_instead_of_the_raw_pin_number`</sub>
- A three-character overridden label (a renumbered multi-level pole like "11a", or a prefixed pole like "P12") stays inside the legibility bounds already proven for a symbol-default three-character number: centred on its own mark, hanging over it by at most the same tolerance, and clear of the jumper bar and neighbouring marks. <sub>`an_overridden_three_character_label_hangs_at_most_a_little_over_its_own_mark`</sub>
- A jumper bar is drawn at least twice as thick as an ordinary symbol line, so that the short bar in the narrow gap between two terminal marks still reads as a shorting bar when the sheet is printed. <sub>`the_jumper_bar_is_at_least_twice_as_thick_as_a_symbol_line`</sub>
- A jumper written between two terminals that are not adjacent is invalid (the same rule the terminal chart enforces), so no bar is drawn for it at all. <sub>`svg_draws_no_bar_for_a_jumper_between_non_adjacent_terminals`</sub>
- A terminal block with several jumpers gets one bar per jumper. <sub>`svg_draws_one_bar_per_jumper_when_there_are_several`</sub>
- A terminal block with no jumpers draws no bar, and a symbol that is not a terminal block never gets one even if it happened to carry a "jumpers" attribute. <sub>`svg_draws_no_bar_without_jumpers_or_on_a_non_terminal_block`</sub>
- A pin_groups range draws one square-bracket polyline and its caption text on the sheet. <sub>`svg_draws_a_bracket_and_caption_for_a_pin_group`</sub>
- After the terminal numbers are renumbered, the pin-group bracket keeps being drawn: the range is matched against the block's displayed numbers, which is what the renumbering rewrote it to. <sub>`svg_still_draws_the_bracket_after_the_terminals_are_renumbered`</sub>
- A symbol with no pin_groups attribute, or an empty one, draws no bracket polyline. <sub>`svg_draws_no_bracket_without_pin_groups`</sub>
- Overlapping pin_groups items are reported by verify (tested in verify.rs) and draw no bracket at all, while an unrelated valid item on the same instance still draws. <sub>`svg_draws_nothing_for_overlapping_pin_groups_but_keeps_the_rest`</sub>
- Pin group brackets and their captions never land on a pin number, a pin name or the symbol's designator, type or function label, for a block, a terminal block and a connector, at every rotation and mirrored. <sub>`svg_pin_group_boxes_never_land_on_any_label_at_any_rotation`</sub>
- No two pieces of a pin group - the bracket and each caption line - ever overlap each other, so a caption split over several lines stays readable at every rotation and mirrored. <sub>`svg_pin_group_pieces_never_overlap_each_other_at_any_rotation`</sub>
- A caption split over several lines is always stacked downwards on the paper - the lines share one centre line and step down one after another - because lettering stays horizontal however the symbol is turned. <sub>`svg_pin_group_caption_lines_stack_downwards_on_paper`</sub>
- A pin group's caption block sits wholly beyond its bracket: no caption line touches the bracket, and every line is further from the symbol than the bracket bar, at every rotation and mirrored. <sub>`svg_pin_group_caption_block_sits_wholly_beyond_its_bracket`</sub>
- A coil shown as a device block reserves no room for pin-group brackets even if it carries the attribute, because none are drawn there: its outline, and so its designator and contact map, sit exactly where they would without the attribute. <sub>`a_device_block_coil_reserves_no_room_for_brackets_it_never_draws`</sub>
- The dot at the target end of a note's leader line is drawn exactly the size of a wire junction dot, so the two read as the same mark. <sub>`svg_leader_dot_is_the_size_of_a_wire_junction_dot`</sub>

### Symbol library

- Every bundled symbol has a unique id and at least one pin. <sub>`builtin_symbols_have_unique_ids_and_pins`</sub>
- A parametric terminal block (e.g. 8 poles) has one left and one right connection point per terminal, all on the 2.5 mm grid and vertically centered. <sub>`dynamic_terminal_block_has_through_pins_on_grid`</sub>
- A multi-level terminal block (8 poles, 2 levels) numbers its pins with a level letter (1a, 1b, 2a, ... 8b), one pin number per terminal and two connection points each, like a single-level block. <sub>`dynamic_multilevel_terminal_block_numbers_pins_with_level_letters`</sub>
- A multi-level terminal block is drawn as side-by-side terminal columns, one column per level, so a 3-level block has three columns of terminals evenly spaced left to right. <sub>`a_multilevel_terminal_block_draws_one_terminal_column_per_level`</sub>
- Every terminal of the same pole sits on one row across all levels, so 1a and 1b share a row and the rows keep the 5 mm pitch of a single-level block. <sub>`every_level_of_the_same_pole_sits_on_one_row`</sub>
- Every connection point of a multi-level terminal block sits on the 2.5 mm grid, and no two terminals share a connection point, so a wire can always reach each terminal on its own. <sub>`multilevel_terminal_block_pins_are_on_the_grid_and_never_coincide`</sub>
- A terminal block asking for 0 or 4 levels (outside the 1-3 range) is rejected as an unknown symbol, the same way an out-of-range pole count is. <sub>`multilevel_terminal_block_rejects_level_count_outside_one_to_three`</sub>
- "terminal_block_8p_1l" is rejected as an unknown symbol, not accepted as an alias of "terminal_block_8p": a single-level terminal block has exactly one spelling of its symbol_id (Task 3). <sub>`multilevel_terminal_block_rejects_the_one_level_alias_spelling`</sub>
- The single-level terminal block id keeps its plain numeric pin numbers (1, 2, ...) exactly as before, unaffected by the new multi-level id shape, so existing projects stay valid. <sub>`single_level_terminal_block_id_is_unaffected_by_multilevel_support`</sub>
- connector_2p generated dynamically has exactly the same pin coordinates as the old static definition, so existing drawings are unaffected. <sub>`dynamic_connector_2p_matches_legacy_static_def`</sub>
- resolve_symbol finds built-in ids, and rejects malformed or out-of-range dynamic ids (0 poles, 51 poles, missing count). <sub>`resolve_symbol_rejects_invalid_ids_and_finds_builtins`</sub>
- sheet_symbol_defs returns the built-in library plus definitions for every dynamic symbol actually used on the sheet. <sub>`sheet_symbol_defs_includes_dynamic_ids_in_use`</sub>
- A PLC input module symbol (e.g. 16 points) is a tall box with one connection point per I/O point on its left side, all on the 2.5 mm grid. <sub>`a_plc_input_module_has_one_connection_point_per_io_point`</sub>
- A split piece of a PLC module, plc_di_{n}p_s{start}, numbers its points from the start point, so its pins carry the module's own point numbers. <sub>`a_split_plc_module_piece_numbers_its_points_from_the_start`</sub>
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
- The loudspeaker is a coil box joined to a widening cone, with two connection points on its flat (coil) side. <sub>`loudspeaker_is_a_coil_box_joined_to_a_cone`</sub>
- The buzzer is a half circle whose flat edge faces its two connection points, distinguishing it from the bell's dome (flat edge on the base, not the side). <sub>`buzzer_is_a_half_circle_facing_its_terminals`</sub>
- The siren is a right triangle, with its two connection points below the triangle's base corners. <sub>`siren_is_a_right_triangle`</sub>
- The fan is a square housing around a 3-blade pinwheel, with two connection points on either side. <sub>`fan_is_a_pinwheel_in_a_square_housing`</sub>
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
- A run-cable breakout wire has no colour or gauge of its own, so the row shows the core's colour and the cable's gauge and part number. <sub>`a_breakout_wire_row_shows_the_core_and_cable_specification`</sub>
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
- When `attrs["terminal_numbers"]` overrides the default numbers, the chart's rows, jumper column and CSV all show the overridden labels instead of the symbol's default pin numbers. <sub>`an_overridden_terminal_number_shows_up_in_the_chart_and_csv`</sub>
- A number list is judged on its trimmed values: spaces around a label are ignored, two labels that differ only in their spacing count as a duplicate, and a label carrying the list's own separator (a comma) is refused. <sub>`a_number_list_is_validated_on_its_trimmed_values`</sub>
- The label map converts a raw pin number to the block's displayed label and back again, and it is the identity map when the block carries no override (so every caller can pass it through without a branch). <sub>`the_label_map_converts_a_raw_pin_number_both_ways`</sub>
- A symbol that is not a terminal block never gets a label map, even if a hand-edited file left a `terminal_numbers` attribute on it: the override only applies to terminal blocks. <sub>`a_symbol_that_is_not_a_terminal_block_has_no_label_map`</sub>
- Rewriting a jumper spec replaces only the terminal numbers it recognises and leaves everything else (spacing, separators, items it cannot read) exactly as it was, so the same rewrite run backwards restores the original text. <sub>`rewriting_a_jumper_spec_only_touches_the_numbers_it_recognises`</sub>
- An override that swaps the terminal count is invalid (too few/many labels), so the chart falls back to the symbol's default numbers instead of losing rows or panicking. <sub>`an_invalid_override_falls_back_to_the_default_numbers`</sub>
- On a multi-level block, an override that changes only the pole (keeping the step at more than 1, e.g. "11a" and "13a") still recognises its jumpers as adjacent, because adjacency is decided from the symbol's raw pin layout, not from the arithmetic difference between the displayed labels. <sub>`adjacency_after_an_override_is_based_on_the_raw_pin_layout_not_the_label_arithmetic`</sub>
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
- Turning a single-level 8-pole terminal block into 2 levels changes its symbol_id to the 2-level dynamic id. <sub>`multilevel_id_turns_a_single_level_block_into_two_levels`</sub>
- Undoing multi-level (setting levels back to 1) returns the plain single-level id, never a "_1l" alias. <sub>`multilevel_id_normalises_one_level_to_the_plain_single_level_id`</sub>
- Asking for 0 levels or more levels than the symbol library supports is rejected, not silently clamped. <sub>`multilevel_id_rejects_zero_and_too_many_levels`</sub>
- A non terminal-block symbol_id is rejected outright: level count only makes sense for terminal blocks. <sub>`multilevel_id_rejects_a_symbol_that_is_not_a_terminal_block`</sub>
- Going from 2 levels to 3 keeps the pole count read from the current symbol_id. <sub>`multilevel_id_keeps_the_pole_count_when_changing_between_multilevel_counts`</sub>
- Increasing levels expands each pole's plain label into one label per level, pole-major with level letters appended ("11,12" -> "11a,11b,12a,12b"). <sub>`relabel_for_levels_expands_plain_labels_into_lettered_levels_pole_major`</sub>
- Decreasing levels back to 1 drops the vanishing level's labels and strips the level letter from what remains. <sub>`relabel_for_levels_drops_vanishing_levels_and_strips_the_letter_at_one_level`</sub>
- Going from 2 levels to 3 keeps the existing level-a/level-b labels and adds a fresh level-c label per pole. <sub>`relabel_for_levels_adds_a_new_level_on_top_of_existing_ones`</sub>
- Only a trailing letter that matches level a's own letter is stripped from the base; an override whose level-a label happens to end in some other letter (part of the user's own text, not a level letter this mechanism appended) keeps that letter, so two poles overridden to labels ending in different letters never collapse onto the same base (Task 3 re-review carry-over: previously any trailing alphabetic character was stripped, which could both destroy part of a hand-set label and produce duplicate bases). <sub>`relabel_for_levels_only_strips_a_trailing_letter_that_matches_level_as_own_letter`</sub>
- Inserting one spare after a chosen terminal appends the pole at the end of the symbol (so existing pins never move) while the returned number sequence shows every later pole's number pushed up by one and the vacated number handed to the new spare. <sub>`inserting_a_spare_pushes_the_numbers_after_the_chosen_terminal`</sub>
- Inserting a spare with no chosen terminal (append) simply continues the numbering: the new pole is physically last and numerically last too, so nothing already on the block is renumbered. <sub>`inserting_a_spare_at_the_end_does_not_renumber_anything`</sub>
- The row the pure function's numbers put in place for the new spare comes out of the chart as an ordinary spare row: both sides unconnected, `spare` set, nothing about it distinguishes it from a symbol-default terminal. <sub>`the_inserted_spares_row_is_unconnected_in_the_chart`</sub>
- A count of zero is refused instead of producing a no-op symbol change. <sub>`inserting_zero_spares_is_refused`</sub>
- Naming a terminal the block does not have as the insertion point is refused, not silently treated as "append at the end". <sub>`inserting_after_an_unknown_terminal_is_refused`</sub>
- Inserting spares beyond the dynamic symbol family's pole cap is refused. <sub>`inserting_past_the_pole_cap_is_refused`</sub>
- On a multi-level block, one inserted spare pole grows into one terminal per level (two new labels "3a"/"3b" for a 2-level block), keeping every level's own letter. <sub>`inserting_a_spare_on_a_multilevel_block_adds_one_terminal_per_level`</sub>
- A second and a third spare insertion into the same block both work, because the insertion point is counted in the order the chart shows (the numbers), not in the physical pin order the first insertion deliberately left out of step. <sub>`inserting_a_second_spare_counts_the_position_in_the_charts_order`</sub>
- Every insertion point offered after a first mid-block insertion is accepted, including the popover's default 末尾 — the position named is looked up among the displayed numbers, so none of them collides with the number the first spare took. <sub>`every_insertion_point_still_works_after_a_mid_block_insertion`</sub>
- A second insertion on a multi-level block also works and again adds one terminal per level, numbered after the pole that is last in the chart's order. <sub>`inserting_a_second_spare_on_a_multilevel_block_adds_one_terminal_per_level`</sub>
- Spare labels are derived by bumping only the numeric part of the neighbouring label, so a block renumbered with a prefix keeps its prefix (and never overflows on the "unreadable number" sentinel). <sub>`inserting_a_spare_keeps_a_prefixed_labels_prefix`</sub>
- A label with no digits at all cannot be numbered from, so the insertion is refused with a reason instead of panicking or silently destroying the label. <sub>`inserting_a_spare_next_to_a_label_without_digits_is_refused`</sub>
- Leaving the following numbers alone (the "leave the spare with a vacant number" mode) changes no existing label and hands the spare the vacant number after the chosen terminal. <sub>`leaving_the_following_numbers_alone_gives_the_spare_the_vacant_number`</sub>
- In the "leave the numbers alone" mode an insertion that would give the spare a number another terminal already uses is refused with a reason, instead of writing duplicate labels. <sub>`leaving_the_following_numbers_alone_is_refused_when_the_number_is_taken`</sub>
- The chart shows the spare exactly where its number says it belongs, not where its physical pole happens to sit: default (number) order re-sorts by the parsed pole number, so an insertion in the middle of the block reads as inserted in the middle of the chart too. <sub>`the_chart_shows_the_spare_after_its_chosen_terminal_not_at_the_physical_end`</sub>
- Rewriting an accessory spec touches only the leading terminal-number token of each item; the kind, the part number, the "end" sentinel and any item naming an unknown terminal stay byte for byte. <sub>`rewrite_accessory_labels_only_replaces_the_terminal_number_token`</sub>
- A terminal_numbers override that happens to equal the symbol's default labels is still reported as an override, because the key is present and valid (not judged only by whether the expanded value differs from the default). <sub>`terminal_numbers_override_reports_a_key_that_matches_the_default_value`</sub>
- An invalid terminal_numbers value (wrong element count) is not treated as an override: the display already falls back to the default, and there is nothing valid to restore. <sub>`terminal_numbers_override_is_none_for_an_invalid_value`</sub>
- With no terminal_numbers key at all, there is no override. <sub>`terminal_numbers_override_is_none_when_the_key_is_absent`</sub>
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
- An unset, empty or unrecognised `row_order` attribute falls back to number order. <sub>`an_unset_or_unknown_row_order_falls_back_to_number_order`</sub>
- Number order (the default) sorts poles as integers, so it stays correct past nine poles. <sub>`number_order_sorts_poles_as_integers`</sub>
- Number order sorts by the parsed pole number of the displayed label, not by the physical (raw pin) order the rows started in — so an override whose labels are not physically monotonic (as spare insertion produces, Task 4) still comes out numerically ascending. <sub>`number_order_sorts_by_the_parsed_label_even_when_the_physical_order_disagrees`</sub>
- Alphanumeric order compares terminal numbers as text, so past nine poles it disagrees with number order ("10" sorts before "2"). <sub>`alnum_order_compares_terminal_numbers_as_text`</sub>
- Page order sorts poles by where their wiring first appears on the sheet in reading order (top-to-bottom, then left-to-right) — even against numeric order — and puts unwired (spare) poles last, in pole-number order. <sub>`page_order_follows_where_the_wiring_first_appears_on_the_sheet`</sub>
- Cable order gathers the poles of each external harness together in the order that harness name first appears (not alphabetically), putting poles with no harness last in pole-number order. <sub>`cable_order_groups_poles_by_their_external_harness_in_first_seen_order`</sub>
- Jumper order brings every pole that has any jumper to the front (in pole-number order), followed by the poles with no jumper (also in pole-number order). <sub>`jumper_order_brings_every_jumpered_pole_to_the_front`</sub>
- On a 3-level terminal block, whichever row order is chosen, the three levels of one pole always stay next to each other in the resulting order. <sub>`every_row_order_keeps_the_three_levels_of_one_pole_adjacent`</sub>
- The terminal chart CSV always lists rows in the same order as the chart (the editor), for every row order. <sub>`the_csv_row_order_always_matches_the_chart_row_order`</sub>
- An accessory spec lists each accessory as "terminal:kind:part number"; "end" places it after the last terminal, and the part number may be left out. <sub>`accessories_are_read_in_order_with_end_and_optional_part_number`</sub>
- An accessory item with an unknown kind, a terminal the block does not have, or a broken format is skipped on its own and reported, while the other items still count. <sub>`a_bad_accessory_item_is_skipped_alone_and_reported`</sub>
- In the terminal chart an accessory gets its own row right before the terminal it names, and "end" accessories come after the last terminal. <sub>`accessory_rows_sit_before_their_terminal_and_end_rows_come_last`</sub>
- The terminal check reports a broken accessory item as a warning and names the item. <sub>`the_terminal_check_warns_about_a_broken_accessory_item`</sub>
- Swapping a terminal's inside and outside exchanges only that terminal's two sides in the chart; the other terminals keep theirs. <sub>`swapping_a_terminal_exchanges_only_its_inside_and_outside`</sub>
- An empty swap setting means no terminal is swapped. <sub>`an_empty_swap_setting_swaps_nothing`</sub>
- A swap setting naming a terminal the block does not have ignores just that item and the terminal check warns about it. <sub>`an_unknown_swapped_terminal_is_ignored_and_warned_about`</sub>
- On a three-level block, swapping and sorting at the same time keep the sorted row order and swap only the named terminals. <sub>`swapping_and_sorting_a_three_level_block_work_together`</sub>

### Terminal connection diagrams

- The terminal strip is drawn as one numbered box per terminal, stacked from top to bottom in terminal order. <sub>`the_strip_stacks_one_numbered_box_per_terminal`</sub>
- The terminal diagram follows the same row order as the chart (the editor): when `row_order` groups the jumpered poles first, the strip's boxes stack in that order too, not plain terminal-number order. <sub>`the_strip_follows_the_chart_row_order_not_plain_terminal_order`</sub>
- The outside of the panel is drawn to the left of the strip and the inside to the right, each under its own caption. <sub>`the_outside_is_on_the_left_and_the_inside_on_the_right`</sub>
- A terminal with nothing wired to it stays in the strip as a lightly filled box marked as a spare. <sub>`a_spare_terminal_stays_in_the_strip_lightly_filled`</sub>
- Wires of the same harness are gathered into one bracket at the outer end of their lead lines, labelled with the harness name. <sub>`wires_of_one_harness_are_gathered_into_a_bracket`</sub>
- A wire that belongs to no harness gets no bracket at all. <sub>`a_wire_without_a_harness_gets_no_bracket`</sub>
- A saddle jumper between neighbouring terminals is drawn as a vertical link on the inside edge of the terminal boxes. <sub>`a_jumper_is_drawn_on_the_inside_edge_of_the_boxes`</sub>
- Each lead line carries the wire it stands for: colour, gauge in sq and part number, written small under the line. <sub>`each_lead_line_carries_the_wire_specification`</sub>
- A terminal whose inside and outside are swapped is drawn with its two wires on the opposite sides; the other terminals stay where they were. <sub>`a_swapped_terminal_draws_its_wires_on_the_opposite_sides`</sub>
- A multi-level terminal block's diagram labels each box with its full level-letter terminal number (1a, 1b, 2a, 2b, ...), so the two levels are never drawn as if they were the same terminal. <sub>`a_multilevel_terminal_block_labels_boxes_with_the_level_letter`</sub>
- A multi-level terminal block's diagram shows the levels as columns: the terminal strip is split into one column per level, level a on the left, and each terminal's box sits in its own level's column. <sub>`a_multilevel_terminal_diagram_draws_each_level_as_a_column`</sub>
- Each level column of a multi-level terminal diagram is headed by its level letter, so a reader can tell which column is which level without reading the box numbers. <sub>`a_multilevel_terminal_diagram_heads_each_level_column_with_its_letter`</sub>
- A jumper linking two poles of the same level ("1a-2a") skips over the other level's row in between ("1b"), so it is routed through a lane beyond every row's lead line — never through the zone where that in-between row's own lead line runs. <sub>`a_same_level_jumper_that_skips_a_row_never_crosses_the_skipped_rows_lead_line`</sub>
- A jumper between the two levels of one pole ("1a-1b") always links physically adjacent rows (levels cluster together), so it still uses the same close lane a single-level block would, right next to the boxes. <sub>`a_cross_level_jumper_between_adjacent_rows_uses_the_same_near_lane_as_single_level`</sub>
- Two same-level jumpers on different levels ("1a-2a" and "1b-2b") each skip over the other level's row, so each gets its own lane: the lanes are distinct, and neither one falls in the zone where any row's lead line runs. <sub>`each_level_gets_its_own_distinct_jumper_lane`</sub>
- Every per-level jumper lane sits outside the 20 mm harness zone (where the brackets and harness names are drawn) and still inside the printable page, on a 2-level and on a 3-level block alike, so a lane never runs over a harness name and never falls off the paper. <sub>`every_jumper_lane_clears_the_harness_zone_and_stays_inside_the_page`</sub>
- The inside connection labels are shortened by exactly the width of the lane band, so the text can never reach the lanes; on a single-level block the band is zero and the label width is unchanged. <sub>`the_inside_label_width_reserves_the_lane_band`</sub>
- Even with same-level jumpers that skip rows, the staggered strip (one box per row, columns by level) keeps its outside-left/inside-right layout: box columns and lead-line directions are unaffected by which lane a jumper's line happens to use. <sub>`the_staggered_layout_keeps_outside_left_and_inside_right_with_skipping_jumpers`</sub>
- A single-level terminal diagram gets no level heading, because it has only one column of terminals. <sub>`a_single_level_terminal_diagram_has_no_level_heading`</sub>
- Terminals that do not fit on one page continue on the next, and each page title states the range of terminals it holds. <sub>`terminals_that_do_not_fit_continue_on_the_next_page`</sub>
- The page title's terminal range names the smallest and largest terminal on that page, so it still reads as a range when a row order such as jumper order puts the rows out of terminal-number sequence. <sub>`the_page_title_range_names_the_smallest_and_largest_terminal_on_the_page`</sub>
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
- A pin group's caption (G8) is one of the text boxes measured on a sheet, so a caption that overlaps another label is caught by the overlap metric. <sub>`a_pin_group_caption_is_measured_as_a_text_box`</sub>
- The measured text of a tagged net label sits inside its flag, so the overlap count follows what is actually printed. <sub>`a_tagged_net_label_is_measured_inside_its_flag`</sub>
- A smaller annotation text height makes the measured net label and wire number smaller too. <sub>`measured_annotation_text_follows_the_sheet_setting`</sub>
- Symbol origins and wire vertices sitting on the 2.5 mm grid report no off-grid points. <sub>`entities_on_the_grid_report_no_off_grid_points`</sub>
- A point shifted by 0.1 mm off the grid is counted, and each stray vertex counts once. <sub>`a_point_shifted_a_tenth_of_a_millimetre_is_off_grid`</sub>
- A hidden designator takes no space in the tidy metrics, because it is not printed. <sub>`a_hidden_designator_has_no_text_box`</sub>
- An empty sheet is perfectly tidy: every metric is zero. <sub>`an_empty_sheet_scores_zero_on_every_metric`</sub>
- tidy_metrics gathers the four counts of one sheet in a single value. <sub>`tidy_metrics_gathers_the_four_counts`</sub>
- Measuring the same sheet twice gives exactly the same numbers, so the agent can compare before and after. <sub>`measuring_the_same_sheet_twice_gives_the_same_numbers`</sub>
- tidy_issues lists every counted problem with its kind, the entities involved, the text and where it is, one entry per count, so the agent can find and fix each one. <sub>`tidy_issues_name_each_counted_problem`</sub>
- A run cable's text (label, core count, fan core names) is measured as text boxes owned by the cable, so a label lying over a block's outline is a label overlap that shows up in the issue details, while an unobstructed one is not. <sub>`run_cable_text_is_measured_and_can_overlap`</sub>

### Tidy variants (sheet copies / adopt)

- A variant copy keeps the paper, title block, revision rows, sheet kind and note height of the original and every entity, with a new sheet id, new entity ids, the label in its name, a mark saying which sheet it copies, and a complete old-to-new id map. <sub>`duplicate_keeps_content_with_fresh_ids_and_a_full_map`</sub>
- A leader note in the copy points at the copy of its target, so the copy is a valid sheet on its own: the target exists, nothing points outside the sheet, and the project's own verification finds no missing leader target. <sub>`duplicate_repoints_leader_lines_inside_the_copy`</sub>
- While variant copies exist, the cross-reference addresses (`/2.B3`) of neither the original nor the copies change, because the copies do not count as another sheet carrying the same reference. <sub>`variant_copies_do_not_grow_cross_reference_addresses`</sub>
- Starting a run with N variants yields N RestoreSheet commands appended after the existing sheets, each with its own label and id map; N outside 1..=4 is rejected. <sub>`start_builds_one_restore_sheet_per_variant`</sub>
- Adopting a variant writes moved entities back under their original ids, deletes what the variant removed, adds what it created with fresh ids, leaves untouched entities alone, and finally removes every variant sheet. <sub>`adopt_maps_changes_back_to_original_ids`</sub>
- Adopting an unchanged variant only removes the variant sheets: the leader note counts as unchanged although its target id differs between the copy and the original. <sub>`adopting_an_unchanged_variant_only_removes_sheets`</sub>
- Adopting a variant that repointed a leader to another entity, and one that added a new note with a leader, writes both back with original-side targets and succeeds through the engine; the target of an entity the variant deleted is dropped instead of left dangling. <sub>`adopt_repoints_leaders_to_original_side_ids`</sub>
- When the variant deletes the entity a leader pointed at, adopting drops that leader instead of writing back a target that will not exist. <sub>`adopt_drops_a_leader_whose_target_the_variant_deleted`</sub>
- finish rejects a run whose original or variant sheets are gone, whose variant sheets are not copies of that original, whose id map points back at the original's own ids, whose adopt target is not one of the copies, or whose original sheet was hand-edited since the comparison started; discarding after a hand edit is still allowed. <sub>`finish_validates_the_run_against_the_current_drawing`</sub>
- Through the engine, start → edit a variant → adopt leaves the original sheet with the variant's placement and no variant sheets, and each of the three steps is one undo. <sub>`engine_round_trip_is_one_undo_per_step`</sub>

### Verification (ERC & electrical)

- A fully wired circuit produces no ERC findings. <sub>`fully_wired_pair_has_no_erc_findings`</sub>
- An entity that sits well inside the sheet's paper produces no `erc.outside_paper` warning. <sub>`an_entity_inside_the_paper_has_no_outside_paper_warning`</sub>
- An entity whose bounds sit exactly on the paper edge is not flagged (only crossing the edge counts). <sub>`an_entity_exactly_on_the_paper_edge_has_no_warning`</sub>
- An entity that falls outside the sheet's current paper bounds is reported as `erc.outside_paper`, without being moved or removed from the drawing. <sub>`an_entity_outside_the_paper_is_reported`</sub>
- Unconnected pins are reported as one warning per symbol, listing the affected pin numbers. <sub>`unconnected_pins_are_reported_per_symbol`</sub>
- Unconnected pins of a block are listed by their pin names, so a controller's dangling input is named, not numbered. <sub>`unconnected_block_pins_are_listed_by_pin_name`</sub>
- A terminal-block terminal counts as connected if either its left or right side is wired. <sub>`terminal_block_terminal_counts_connected_if_either_side_wired`</sub>
- Missing and duplicate reference designators are flagged, but relay coil + contacts legitimately share one designator. <sub>`empty_and_duplicate_references_are_flagged_but_relays_allowed`</sub>
- A plug and a socket that share one designator are one mated connector, so they raise no duplicate error, while a plug sharing a designator with an unrelated part still does. <sub>`a_plug_and_socket_may_share_a_designator`</sub>
- A pin_groups attribute that overlaps another group, or names a pin the terminal block does not have, is reported as an ERC error naming the terminal block and the bad item, instead of being silently ignored. <sub>`a_bad_pin_groups_item_is_reported_as_an_erc_error`</sub>
- A coil drawn as a device block cannot carry pin group brackets, and says so as an ERC error instead of dropping them without a word. <sub>`pin_groups_on_a_device_block_coil_are_reported_instead_of_silently_dropped`</sub>
- A note whose leader target was deleted afterwards is reported, so the drawing never keeps a leader pointing at nothing without saying so. <sub>`a_note_whose_leader_target_was_deleted_is_reported`</sub>
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

### wire_legend

- A run-cable breakout wire is counted under its core's colour and the cable's gauge, so it adds no blank row. <sub>`a_breakout_wire_adds_no_blank_row`</sub>
- The netlist of a sheet is the same before and after a legend block is placed on it: the block carries no electrical meaning. <sub>`a_legend_block_does_not_appear_in_the_netlist`</sub>
- Placing a legend block raises no verification diagnostic of its own, and leaves the sheet's other diagnostics untouched. <sub>`a_legend_block_raises_no_verification_diagnostic`</sub>
- The BOM is the same before and after a legend block is placed: it is not a component and it adds no wire length. <sub>`a_legend_block_does_not_change_the_bom`</sub>
- The wire list is the same before and after a legend block is placed: the block is not a conductor. <sub>`a_legend_block_does_not_change_the_wire_list`</sub>
- A sheet with no wires at all still gets a header-only legend block (no data rows). <sub>`an_empty_sheet_has_only_the_header_row`</sub>
- Wires that share the same colour, gauge and resolved part number collapse into one row whose length is the sum of theirs. <sub>`wires_of_the_same_colour_gauge_and_part_number_are_summed_into_one_row`</sub>
- The same colour written in English and in Japanese is one stock item, so it makes a single row whose length is the sum of both. <sub>`a_colour_written_in_english_and_in_japanese_makes_one_row`</sub>
- The colour printed in the legend is the same name the wire's own annotation prints on the sheet. <sub>`the_legend_prints_the_same_colour_name_as_the_wire_annotation`</sub>
- Rows are sorted by colour, then by gauge within the same colour. <sub>`rows_are_sorted_by_colour_then_gauge`</sub>
- Lengths that cannot be added up exactly in binary floating point still print as a short decimal, never as a long string of digits. <sub>`a_summed_length_is_rounded_instead_of_showing_floating_point_noise`</sub>
- A wire whose colour and gauge are not in the wire-part table shows a blank part number, instead of an invented one. <sub>`an_unresolved_part_number_shows_as_blank`</sub>

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
- Renumbering wires on a sheet with a connector-to-connector single-line cable numbers the drawn wires 1, 2 with no gap left by the cable's wire-less core nets. <sub>`core_only_nets_take_no_wire_number`</sub>

### wire_note

- A wire that has a gauge, a colour and a cut length is annotated "0.75sq 白 0.4m" on the drawing. <sub>`note_shows_gauge_colour_and_length`</sub>
- A missing attribute is simply left out of the note, so a wire without a length reads "0.75sq 白". <sub>`note_omits_missing_attributes`</sub>
- A plain black wire with no gauge and no length gets no note, because black is the default colour and "黒" on every line only clutters the sheet; a black wire with a gauge or length still names its colour. <sub>`a_plain_black_wire_gets_no_note`</sub>
- A wire with no gauge, no colour and no length gets no note at all instead of an empty one. <sub>`wire_without_attributes_has_no_note`</sub>
- Colour names are matched ignoring capitals and surrounding space: a known colour prints its display name, and one the drawing does not know prints as typed but lower-cased. <sub>`colour_names_are_matched_ignoring_capitals_and_space`</sub>
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
- A relay coil and the contact it drives are skipped by the device cross-reference, because the coil⇔contact cross-reference already writes where each of them is. <sub>`a_relay_coil_and_its_contact_are_skipped_by_the_device_cross_reference`</sub>
- A coil with no contacts anywhere gets nothing from the coil⇔contact cross-reference, so a designator carried only by such coils still gets the ordinary device cross-reference. <sub>`a_contactless_coil_on_two_sheets_still_gets_a_device_cross_reference`</sub>
- The per-sheet device cross-reference table maps each symbol entity to the text drawn beside it. <sub>`sheet_device_cross_reference_table_maps_symbols_to_text`</sub>
- A designator on three sheets lists every other sheet's address, in sheet order, on one line. <sub>`a_designator_on_three_sheets_lists_every_other_sheet`</sub>
- The device cross-reference text sits to the right of the symbol outline, on the same row as the designator label. <sub>`device_cross_reference_text_sits_right_of_the_outline`</sub>
- A designator label wider than its symbol pushes the device cross-reference further right, so the two never overlap. <sub>`device_cross_reference_clears_a_designator_label_wider_than_the_symbol`</sub>


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

### drawing_tools

- place_symbol returns the new part's pins with paper positions, so the agent can wire it without computing coordinates. <sub>`place_symbol_returns_the_pins_of_the_new_part`</sub>
- get_pins finds parts by reference or entity id and returns their pins and outline. <sub>`get_pins_finds_parts_by_reference_or_entity_id`</sub>
- The pin tools tell the agent to wire by pin name instead of computing pin coordinates. <sub>`the_pin_tools_tell_the_agent_not_to_compute_coordinates`</sub>
- connect_pins wires two pins by name with right-angle segments, and verification then finds no unconnected pin. <sub>`connect_pins_wires_two_pins_by_name`</sub>
- connect_pins passes the given via points in order. <sub>`connect_pins_passes_via_points`</sub>
- connect_pins refuses an unknown part, an unknown pin, an ambiguous reference and a pin wired to itself, and creates nothing. <sub>`connect_pins_refuses_bad_specifiers_and_creates_nothing`</sub>
- place_mate puts a plug/socket pair in one step, both carrying the designator without any reference warning, returns both outer connection points, and one undo removes the pair. <sub>`place_mate_puts_a_pair_that_one_undo_removes`</sub>
- connect_pins refuses a route that would run over a third pin and creates nothing, so no pin is joined to a net by accident. <sub>`connect_pins_refuses_a_route_over_another_pin`</sub>
- connect_pins takes a list of connections and draws them all in one call that one undo removes, so the agent can wire a whole sheet without one call per wire. <sub>`connect_pins_draws_a_list_of_connections_in_one_edit`</sub>
- If any connection in the list is wrong, connect_pins names its index and creates none of them. <sub>`connect_pins_list_is_all_or_nothing`</sub>
- place_mate takes a list of pairs and places them all in one call that one undo removes. <sub>`place_mate_places_a_list_of_pairs_in_one_edit`</sub>
- In a place_mate list, mates repeating the same reference without poles are numbered 1, 2, 3 in list order, an explicit pole wins, a lone reference gets none, and both halves carry the pole attribute. <sub>`place_mate_list_numbers_the_poles`</sub>
- place_mate numbering skips poles given explicitly for the same reference and treats a blank pole as none: [X, X pole "1", X] gets poles 2, 1, 3. <sub>`place_mate_numbering_skips_explicit_poles`</sub>
- connect_cable draws the LTE harness in one call: four board wires to a breakout, one line to the USB plug, cores mapped by pin order; the netlist has four nets, ERC finds no unconnected pin, and one undo removes it all. <sub>`connect_cable_draws_a_breakout_harness_in_one_edit`</sub>
- connect_cable with labels on the board side draws a labelled wire per core in one edit, answers the label names as the core ends, joins each core to its label net, refuses labels with single or a wrong count, and its description mentions labels. <sub>`connect_cable_labels_end_the_breakout_wires_at_net_labels`</sub>
- If any cable in the list is wrong, connect_cable names its index and draws none of them; a correct list is drawn in one edit. <sub>`connect_cable_list_is_all_or_nothing`</sub>
- connect_cable's description tells the agent that cores follow pin order and to use it instead of a fake breakout block. <sub>`connect_cable_is_described_for_the_agent`</sub>

### Frame templates (REST)

- GET /frame-templates lists the bundled frame templates with their names in both languages and the folder where the user can drop their own. <sub>`the_link_api_lists_the_bundled_frame_templates`</sub>
- The listed frame template carries the whole frame definition (paper, margin, zones, title-block cells and revision columns), so a client can preview it without reading any file. <sub>`a_listed_frame_template_carries_its_whole_definition`</sub>
- POST /commands with set_frame_template switches the whole project's frame, and one undo puts the previous frame back. <sub>`setting_the_frame_template_over_the_link_api_is_one_undo_step`</sub>
- The exported SVG follows the project's frame template: after switching to A4 portrait the title block is drawn 110 mm wide instead of 120 mm. <sub>`the_exported_svg_follows_the_projects_frame_template`</sub>
- A new project starts with the frame template picked as the global default in the app settings. <sub>`a_new_project_starts_with_the_default_frame_template_from_the_settings`</sub>
- Without a global default in the settings a new project leaves the frame unset, so it is drawn with the bundled JIS A3 frame. <sub>`a_new_project_without_a_default_setting_keeps_the_frame_unset`</sub>
- A blank default frame template in the settings is treated as "no default" instead of an unknown id. <sub>`a_blank_default_frame_template_setting_is_ignored`</sub>
- POST /frame-templates/save refuses with 400 a frame that reuses a bundled frame's id or whose id cannot be a file name, and writes nothing (the editor then asks for another name). <sub>`the_link_api_refuses_to_save_over_a_bundled_frame`</sub>

### instance_guard

- A request carrying another app run's instance id is refused before it reaches the drawing, so an agent CLI left over from a crashed or restarted app cannot edit the new session. <sub>`a_request_from_an_earlier_app_run_is_refused`</sub>
- The agent's own requests (this run's id) and external clients without the header are served as before. <sub>`this_run_and_external_clients_are_served`</sub>
- Every app run gets its own instance id. <sub>`each_app_run_gets_its_own_instance_id`</sub>

### REST Link API

- The REST parts endpoints support searching (sample data included), upserting, category filtering, deleting, and listing wire parts. <sub>`parts_endpoints_search_upsert_delete`</sub>
- GET /api/v1/verify returns the drawing's diagnostics as JSON (e.g. empty reference and unconnected pins). <sub>`verify_returns_diagnostics`</sub>
- POST /api/v1/import/kicad replaces the open project with the converted schematic and returns a patch plus an import report. <sub>`import_kicad_replaces_project_and_reports`</sub>
- POST /api/v1/simulate/op solves the DC operating point and returns net voltages and component currents (requires ngspice; skipped otherwise). <sub>`simulate_op_returns_result`</sub>
- POST /api/v1/export/pdf writes a valid PDF file to the requested path. <sub>`export_pdf_writes_pdf_file`</sub>
- POST /api/v1/export/dxf and /export/kicad write the sheet as DXF and .kicad_sch, and POST /api/v1/import/dxf reads the DXF back into a project with an import report. <sub>`export_dxf_kicad_and_import_dxf_round_trip`</sub>
- POST /api/v1/variants/start copies the sheet once per variant (one undo step) and returns each copy's id map; /variants/finish with a chosen sheet writes it back under the original ids and removes the copies, with null it only removes them; a count outside 1..=4 is rejected. <sub>`variants_start_and_finish_round_trip`</sub>
- POST /api/v1/variants/finish refuses a run whose copies no longer exist, a chosen sheet that is not one of the copies, and adopting after the original sheet was edited by hand during the comparison; discarding still works, and a count of 0 is rejected at start. <sub>`variants_finish_refuses_a_run_that_no_longer_matches_the_drawing`</sub>
- POST /api/v1/export/pdf-book writes one PDF holding the cover, every sheet and the requested reports. <sub>`export_pdf_book_writes_cover_sheets_and_reports`</sub>
- The terminal endpoints list the terminal blocks of the drawing and return one block's chart and check result. <sub>`terminal_endpoints_list_chart_and_check`</sub>
- POST /api/v1/export/report writes one report as CSV or as framed PDF pages, and refuses CSV for the graphical terminal diagram. <sub>`export_report_writes_csv_and_pdf_per_report`</sub>
- POST /api/v1/export/report fills the BOM's maker, name, price, amount and purchase columns from the parts database, and leaves them empty for a part number the database does not know. <sub>`export_report_bom_looks_part_numbers_up_in_the_parts_database`</sub>

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
- Generation settings that cannot produce a drawing (a split unit of zero points) are refused with 400 and change nothing. <sub>`an_impossible_generation_setting_is_refused_with_a_bad_request`</sub>
- The PLC I/O report is exported through the ordinary report endpoint as "plc-io", listing one row per assigned point. <sub>`the_plc_io_report_is_exported_like_any_other_report`</sub>
- The PLC tools tell the agent what the assignment table is for and that generating a page is one undo step. <sub>`the_plc_tools_explain_the_assignment_table_and_the_generated_page`</sub>

### project_tools

- A project saved with save_project opens again with open_project, replacing the drawing that was open. <sub>`a_saved_project_opens_again_and_replaces_the_open_drawing`</sub>
- Opening a missing file or a file that is not a MadakeCAD project fails with the reason and leaves the open drawing as it was. <sub>`opening_a_missing_or_foreign_file_fails_and_keeps_the_drawing`</sub>
- The in-app agent's own tool window (no agent manager attached) cannot open a project, because that would replace the drawing under its own running turn. <sub>`the_in_app_tool_window_cannot_open_a_project`</sub>
- While an agent turn is running, open_project refuses instead of cancelling that turn, so an agent connected over MCP cannot cut off its own reply. <sub>`opening_is_refused_while_an_agent_turn_is_running`</sub>
- Saving from the in-app tool window writes the drawing only; the chat history is saved only where the agent manager is attached. <sub>`saving_without_the_agent_manager_writes_the_drawing_only`</sub>
- The MCP server offers open_project and save_project, and their descriptions say which file types they take and that opening replaces the drawing. <sub>`the_server_offers_open_and_save_with_clear_descriptions`</sub>

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

### sheet_image

- The sheet image is a PNG of the whole sheet at 4 px/mm by default, and a region in millimetres zooms into part of it. <sub>`the_sheet_image_is_a_png_of_the_sheet_or_a_region`</sub>
- Backends that cannot show tool images (API and Copilot routes through the tool bridge) get a short note instead of a huge base64 string. <sub>`the_bridge_replaces_the_image_with_a_short_note`</sub>
- The tool tells the agent to look at the drawing after editing it. <sub>`the_image_tool_tells_the_agent_to_look_after_editing`</sub>

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
- The tidy metrics also list each counted problem (kind, entities, text, place), so the agent can find what to fix; the list is capped and says how many were left out. <sub>`tidy_metrics_list_each_problem_with_its_place`</sub>

### tool_bridge

- The API route sees exactly the same tools as the Claude Code CLI route, so neither is missing a feature. <sub>`the_api_route_sees_the_same_tools_as_the_cli_route`</sub>
- Every bridged tool carries a description and an object-shaped input schema. <sub>`every_bridged_tool_has_a_description_and_an_object_schema`</sub>
- An edit made through the bridge lands in the document and can be undone like any other edit. <sub>`an_edit_through_the_bridge_lands_in_the_document_and_can_be_undone`</sub>
- A multi-pin device with named pins is placed in a single call by passing its pin table in attrs. <sub>`a_block_with_named_pins_is_placed_in_one_call`</sub>
- A block whose pin table is broken comes back as an error instead of a blank box on the sheet. <sub>`a_block_with_a_broken_pin_table_is_refused`</sub>
- Reading tools work through the bridge too, so the agent can look at the drawing before editing. <sub>`reading_tools_work_through_the_bridge`</sub>
- A tool name that does not exist comes back as an error result instead of killing the turn. <sub>`an_unknown_tool_name_comes_back_as_an_error_result`</sub>
- Bad arguments come back as an error result carrying the reason, so the model can correct itself. <sub>`bad_arguments_come_back_with_a_reason`</sub>
- The input schema published for execute_commands lists the run_cable entity kind, so an agent can see how a cable with a run is written. <sub>`execute_commands_schema_lists_the_run_cable_kind`</sub>

### tools

- The parts search tool advertises selection, comparison and alternative-part use, so the agent reaches for it when asked "what can replace this?". <sub>`the_parts_search_tool_advertises_selection_and_comparison`</sub>
- The template tools explain that a template is a starting skeleton, that it lands ERC-clean, and that applying it is a single undo step. <sub>`the_template_tools_explain_what_a_template_is`</sub>
- Every published tool carries a description, so no tool is offered to the agent unexplained. <sub>`every_published_tool_has_a_description`</sub>
- The edit tool tells an agent how to write a jumper on a multi-level terminal block (with the level letter), so it does not send a plain numeric pair that would be refused. <sub>`the_edit_tool_explains_the_multilevel_jumper_form`</sub>

### Wire part numbers / command errors (REST)

- A command the engine refuses comes back as HTTP 400 saying what went wrong and which command in the list failed, instead of a silent 200. <sub>`a_refused_command_returns_400_with_the_reason_and_the_command_index`</sub>
- A parametric symbol asked for beyond its pole limit is refused the same way, with a message that names the usable range. <sub>`a_pole_count_beyond_the_limit_is_refused_over_the_api`</sub>
- A valid command list still returns 200 with one patch per command. <sub>`a_valid_command_list_still_succeeds`</sub>
- Asking for a batch (`?batch=true`) runs the whole command list as a single edit, so one patch comes back and one undo takes all of it away again. <sub>`a_batch_command_list_is_one_undo_step`</sub>
- A batch whose last command is refused applies nothing at all: the drawing is untouched and the reply says zero commands were applied. <sub>`a_refused_batch_applies_nothing`</sub>
- Posting a wire-part CSV fills the project's colour+gauge table, and the wire list then prints the part number of a wire that has none of its own. <sub>`importing_a_wire_part_csv_fills_the_part_number_column_of_the_wire_list`</sub>
- A CSV that cannot be read is refused with 400 and the table is left exactly as it was. <sub>`a_malformed_wire_part_csv_is_refused_and_changes_nothing`</sub>


## AI assistant (madake-agent)


### anthropic_api

- Streamed assistant text arrives as text deltas and the turn ends with the assembled reply and its token usage. <sub>`streamed_text_becomes_deltas_and_a_completed_turn`</sub>
- The request carries the configured model, a streaming flag, the system prompt and the user's message. <sub>`the_request_carries_the_model_system_prompt_and_user_message`</sub>
- The bridged MCP tools are offered to the API on every request, so the agent can edit the drawing. <sub>`the_bridged_mcp_tools_are_offered_to_the_api`</sub>
- Thinking the API streams arrives as thinking events in order (empty pieces and signatures are skipped), separate from the reply text. <sub>`streamed_thinking_becomes_thinking_events`</sub>
- A tool the model asks for is executed locally and its result is sent back so the model can continue. The finish event carries the tool's result. <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
- A tool that fails is reported back to the model as an error result instead of aborting the turn. <sub>`a_failing_tool_is_reported_to_the_model_as_an_error_result`</sub>
- The tool loop stops after a bounded number of rounds so a looping model cannot run forever. <sub>`the_tool_loop_stops_after_a_bounded_number_of_rounds`</sub>
- A rejected API key produces a message that says the key is the problem, not a raw HTTP code. <sub>`a_rejected_api_key_is_explained_as_a_key_problem`</sub>
- An overloaded API produces a "busy, try again" message rather than a bare error code. <sub>`an_overloaded_api_is_explained_as_a_busy_service`</sub>
- An error event that arrives mid-stream is surfaced to the user too. <sub>`an_error_event_inside_the_stream_is_surfaced`</sub>
- Earlier turns of the conversation are replayed so the model remembers what was said before. <sub>`earlier_turns_of_the_conversation_are_replayed`</sub>
- The connection test reports success for a working key and a readable reason for a bad one. <sub>`the_connection_test_reports_success_or_a_readable_reason`</sub>
- An image and a PDF attached to the message reach the API as an image block and a document block before the text. <sub>`attachments_reach_the_api_as_image_and_document_blocks`</sub>

### Claude CLI backend

- The Claude CLI is launched headless with the required flags (-p, stream-json output, partial messages, strict MCP config). <sub>`args_contain_required_flags`</sub>
- The bundled documentation folder is opened to the agent as a readable directory, so it can quote the manual. <sub>`args_open_the_bundled_documentation_for_reading`</sub>
- Resume session id, model choice and an appended system prompt are passed through when provided. <sub>`args_include_resume_model_and_system_prompt_when_given`</sub>
- The agent's MCP config points at MadakeCAD's own local MCP server, so it uses the same tools as any client. <sub>`mcp_config_points_at_local_mcp_server`</sub>
- The agent's MCP config carries this app run's instance id as a header, so a CLI left over from an earlier run is refused by the server; without an id no header is sent. <sub>`mcp_config_carries_the_instance_header`</sub>
- A turn streams events (thinking and text deltas, tool use, completion) parsed from the CLI's stream-json output. <sub>`send_streams_events_from_fake_cli`</sub>
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
- With attachments the CLI is started with stream-json input and receives one user message whose image and PDF blocks come before the text. <sub>`attachments_reach_the_cli_as_stream_json_content_blocks`</sub>
- Without attachments the arguments stay as before (no stream-json input), so plain turns are unchanged. <sub>`plain_turns_keep_the_text_input`</sub>

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
- A reply is recorded as blocks in the order it happened: consecutive pieces of the same kind join one block, and a change of kind starts a new block, while the plain text and tool list are still filled as before. <sub>`replies_are_recorded_as_blocks_in_the_order_they_happened`</sub>
- A tool result longer than 8 KB keeps only its head, cut before a character that would cross the limit, and the full size is kept so the screen can say how much was left out. <sub>`a_long_tool_result_is_cut_on_a_character_boundary`</sub>
- A tool finish without an id (some Copilot lines) still puts its result on the open call of the same tool, as before. <sub>`a_finish_without_an_id_puts_the_result_on_the_open_call_of_that_tool`</sub>
- A chat file saved before blocks and tool results existed still loads, its messages have no blocks, and saving it again does not write empty blocks or results. <sub>`an_old_chat_file_without_blocks_still_loads`</sub>
- A chat file containing a block kind this build does not know still loads: the unknown block is kept as an unknown placeholder and the rest of the history is intact. <sub>`a_chat_file_with_an_unknown_block_kind_still_loads`</sub>

### copilot_cli

- The Copilot CLI is launched non-interactively with the flags needed to work unattended (JSON lines, auto-approved tools, no questions). <sub>`args_contain_required_flags`</sub>
- The first turn of a conversation pins a new session id, so later turns can continue the same session. <sub>`the_first_turn_pins_a_new_session_id`</sub>
- A follow-up turn resumes the same Copilot session instead of starting a new one, so the agent remembers the conversation. <sub>`a_follow_up_turn_resumes_the_same_session`</sub>
- The configured Copilot model is passed to the CLI ("auto" lets Copilot pick). <sub>`the_configured_model_is_passed_to_the_cli`</sub>
- Copilot is pointed at MadakeCAD's own MCP server, so it edits drawings through the same commands as every other client. <sub>`the_mcp_config_points_at_the_local_madakecad_server`</sub>
- Copilot's MCP config carries this app run's instance id as a header, so a Copilot left over from an earlier run is refused. <sub>`the_mcp_config_carries_the_instance_header`</sub>
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
- Copilot gets attachments as files saved in a folder it is allowed to read (--add-dir), listed by path after the text; the folder is removed after the turn. <sub>`copilot_reads_attachments_from_a_temporary_allowed_folder`</sub>
- For the Copilot CLI only images and PDFs are saved to the turn's folder; a spreadsheet is not saved because it goes into the message as text, and with nothing to save no folder is made. <sub>`only_images_and_pdfs_are_saved_for_the_copilot_cli`</sub>

### copilot_events

- The assistant's answer arrives as message deltas and is shown in the chat as it is typed. <sub>`assistant_message_deltas_are_shown_as_they_arrive`</sub>
- The model's private reasoning is not shown in the chat, even though it streams like the answer. <sub>`reasoning_deltas_are_not_shown_in_the_chat`</sub>
- A completed assistant message carries the tool calls the model asked for, with their arguments. <sub>`a_completed_message_carries_the_requested_tool_calls`</sub>
- Starting a tool reports its name and arguments, so the chat can show what the agent is doing. <sub>`a_tool_execution_start_reports_the_tool_and_its_arguments`</sub>
- Finishing a tool closes the matching call, reports whether it succeeded and carries the result text the CLI printed. <sub>`a_tool_execution_complete_closes_the_call_with_its_outcome`</sub>
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
- Gemini is not asked for its thoughts, so a reply never produces thinking events. <sub>`gemini_replies_produce_no_thinking_events`</sub>
- A tool the model asks for is executed locally and its result is sent back as a functionResponse so the model can continue. The finish event carries the tool's result. <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
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
- An image and a PDF attached to the message reach Gemini as inline data parts before the text part. <sub>`attachments_reach_gemini_as_inline_data_parts`</sub>

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
- The system prompt tells the agent to wire diagrams by pin name with the pin tools, batched in one call, instead of computing pin coordinates. <sub>`system_prompt_tells_the_agent_to_wire_by_pin_name`</sub>
- The system prompt tells the agent to look at the sheet image after drawing, instead of judging the layout by numbers alone. <sub>`system_prompt_tells_the_agent_to_look_at_the_sheet`</sub>

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
- A message with attachments keeps only their name, kind and size in the conversation (not their content), and the turn receives the files. <sub>`a_message_keeps_the_attachment_names_but_not_their_content`</sub>
- A message whose attachments break the limits is refused before anything is added to the conversation. <sub>`invalid_attachments_are_refused_before_the_turn_starts`</sub>

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
- Reasoning that a model streams as reasoning_content or reasoning (Ollama, DeepSeek and others) arrives as thinking events, separate from the reply text; empty pieces are skipped. <sub>`streamed_reasoning_becomes_thinking_events`</sub>
- A delta that carries the same reasoning in both reasoning_content and reasoning (some proxies mirror them) yields one thinking event, not two. <sub>`mirrored_reasoning_fields_are_not_doubled`</sub>
- A tool the model asks for is executed locally and its result is sent back as a tool message so the model can continue. The finish event carries the tool's result. <sub>`a_requested_tool_is_executed_and_its_result_is_sent_back`</sub>
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
- An attached image reaches an OpenAI-compatible server as a data URL part before the text. <sub>`an_attached_image_reaches_the_server_as_a_data_url`</sub>
- An attached PDF is refused with a reason before anything is sent, because OpenAI-compatible models cannot read PDFs. <sub>`an_attached_pdf_is_refused_before_sending`</sub>

### stream-json parser

- The CLI's system/init line yields a session-started event carrying the session id. <sub>`init_line_yields_session_started`</sub>
- Text deltas become text events and thinking deltas become thinking events, each joined in order; signature deltas are ignored. <sub>`text_and_thinking_deltas_become_their_own_events`</sub>
- A tool result carries what the tool returned, whether the CLI sends it as a string or as an array of text blocks. <sub>`a_tool_result_carries_what_the_tool_returned`</sub>
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
- Out of the box the app checks for a newer version at startup; a settings file written before the option existed also checks. <sub>`update_checks_are_on_by_default_and_for_old_settings_files`</sub>

### tool_bridge

- Each MCP tool becomes one API tool keeping its name, description and input schema. <sub>`every_mcp_tool_becomes_one_api_tool`</sub>
- The JSON Schema `$schema` marker is dropped because the API only wants the shape itself. <sub>`the_schema_marker_is_dropped_from_the_input_schema`</sub>
- A tool without a usable schema still gets an empty object schema, so the API accepts it. <sub>`a_tool_without_a_schema_gets_an_empty_object_schema`</sub>
- A schema that forgot `"type": "object"` is repaired instead of being sent as-is. <sub>`a_schema_missing_its_object_type_is_repaired`</sub>
- Every bridged tool name fits the API's allowed name pattern, so no tool is rejected. <sub>`bridged_tool_names_fit_the_api_name_rules`</sub>
- A tool with an empty description keeps an explanatory placeholder rather than nothing. <sub>`a_tool_without_a_description_still_carries_some_text`</sub>

### attachments

- An Excel workbook becomes text: every sheet in order under its own heading, one row per line with tab-separated columns, empty rows left out, whole numbers without ".0", and date cells written as dates. <sub>`an_excel_workbook_becomes_tab_separated_text_per_sheet`</sub>
- A workbook too long to send whole is cut at a row boundary and says how many rows were left out. <sub>`a_long_workbook_is_cut_and_says_how_many_rows_were_left_out`</sub>
- A file that is not a readable workbook is refused with its file name, and nothing is sent. <sub>`a_broken_workbook_is_refused_with_its_name`</sub>
- A CSV file is sent as its text with a leading byte order mark removed; a CSV that is not UTF-8 (for example Shift_JIS) is refused with a hint to save it as UTF-8. <sub>`a_csv_is_sent_as_text_and_must_be_utf8`</sub>
- Every provider gets a spreadsheet as plain text placed before the message: a text block for Anthropic and the Claude Code CLI, a text part for Gemini and OpenAI-compatible models (so models that cannot read PDFs can still read Excel), and inline in the message for the Copilot CLI. <sub>`every_provider_gets_a_spreadsheet_as_text_before_the_message`</sub>
- Images (PNG/JPEG/GIF/WebP), PDFs, Excel workbooks (.xlsx) and CSV files can be attached; anything else, more than five files or a file over 20MB is refused as a whole. <sub>`only_images_pdfs_and_spreadsheets_within_the_limits_are_accepted`</sub>
- The kind of a picked file comes from its extension, case-insensitively; unknown extensions are not attachable. <sub>`the_kind_of_a_picked_file_comes_from_its_extension`</sub>
- For the Anthropic API an image becomes an image block and a PDF a document block, both before the text. <sub>`anthropic_gets_image_and_document_blocks_before_the_text`</sub>
- Gemini receives attachments as inline data parts followed by the text part. <sub>`gemini_gets_inline_data_parts`</sub>
- An OpenAI-compatible model gets images as data URLs, and a PDF is refused before anything is sent. <sub>`openai_gets_data_url_images_and_refuses_pdfs`</sub>
- The Claude Code CLI gets one stream-json user message line carrying the same content blocks. <sub>`claude_cli_gets_one_stream_json_line`</sub>
- Saving for a CLI keeps the file names, makes path separators harmless and numbers duplicates, and the paths are listed after the text. <sub>`saving_for_a_cli_keeps_safe_unique_names`</sub>
- A picked file is read with its kind from the extension, and pasted base64 content is decoded; broken base64 is refused. <sub>`files_and_pasted_content_become_attachments`</sub>
- Previewing a picked file reports its name, kind and size, and the reason when it cannot be attached (wrong kind or missing file). <sub>`previewing_a_file_reports_its_kind_size_and_problems`</sub>
- An attachment given by the screen is read from its file path or decoded from base64, and one unreadable item refuses the whole message. <sub>`inputs_from_the_screen_resolve_from_a_path_or_base64`</sub>


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
- `madake export cable-list` writes the cable list report (CSV by default). <sub>`export_cable_list_writes_the_cable_list_report`</sub>
- `madake export panel-wiring` writes the panel wiring diagram report. <sub>`export_panel_wiring_writes_the_panel_wiring_report`</sub>
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
- madake export dxf / kicad write one sheet through the DXF and KiCad endpoints (sheet selectable) and print the export breakdown: how many of each kind was written and what could not be written. <sub>`export_dxf_and_kicad_forward_to_their_endpoints`</sub>
- madake open with a .dxf file imports it through /import/dxf, passing --wire-layer, and prints the import summary; the extension is matched case-insensitively for KiCad too; --wire-layer is rejected for other files. <sub>`open_dxf_imports_with_wire_layers`</sub>
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
- keeps only the ops of the sheet on screen <sub>`patchOverlayTargets`</sub>
- uses the running conversation's color when only one is running, and the default color otherwise <sub>`patchOverlayTargets`</sub>
- keeps the per-conversation color for a variant sheet even with several conversations running <sub>`patchOverlayTargets`</sub>

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

### cable

- labels a cable with designator, part, cores times gauge and length, skipping empty items <sub>`多芯ケーブル`</sub>
- draws the shield as an ellipse around the definition line, a little longer than it <sub>`多芯ケーブル`</sub>
- numbers a new cable after the W numbers of the cables and harnesses on the sheet <sub>`多芯ケーブル`</sub>
- turns a drag into an add_cable command with no core count, and a mere click into nothing <sub>`多芯ケーブル`</sub>
- is picked by clicking near its definition line <sub>`多芯ケーブル`</sub>
- recognises both cable kinds and lists the points of the run <sub>`多芯ケーブル`</sub>
- names a pin end by designator and pin, and a breakout wire end as null <sub>`多芯ケーブル`</sub>
- marks a run with a slash through the middle of its longest segment, like the printed sheet <sub>`多芯ケーブル`</sub>
- draws a bar across the landed pins and a fan to each breakout wire <sub>`多芯ケーブル`</sub>
- is picked by clicking near its run <sub>`多芯ケーブル`</sub>

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
- unplaced contacts follow the placed functions, one row each <sub>`未配置機能 (予約接点)`</sub>
- a collapsed device hides its unplaced rows too <sub>`未配置機能 (予約接点)`</sub>
- the filter keeps devices with unplaced functions, or only fully placed ones <sub>`未配置機能 (予約接点)`</sub>
- an unplaced row is named by its contact kind and terminals <sub>`未配置機能 (予約接点)`</sub>

### dynamicSymbol

- connector_2p has the same pin coordinates as the legacy static definition <sub>`dynamicSymbol`</sub>
- terminal_block_3p has 3 terminals with left/right points, centered on the 2.5 mm grid <sub>`dynamicSymbol`</sub>
- a PLC input module has one connection point per I/O point on its left side <sub>`dynamicSymbol`</sub>
- PLC modules come in an input and an output flavour <sub>`dynamicSymbol`</sub>
- malformed dynamic ids return null <sub>`dynamicSymbol`</sub>
- PLC module symbols exist from 1 to 64 points only <sub>`dynamicSymbol`</sub>
- a split PLC module piece numbers its points from the start point and keeps the shape of a module of that size <sub>`dynamicSymbol`</sub>
- a multi-level terminal block (4 poles, 2 levels) numbers its pins with a level letter <sub>`dynamicSymbol`</sub>
- draws one terminal column per level, with the same pole of every level on one row <sub>`dynamicSymbol`</sub>
- keeps every connection point on the 2.5 mm grid and never lets two terminals share one <sub>`dynamicSymbol`</sub>
- a terminal block with a level count outside 1-3 returns null <sub>`dynamicSymbol`</sub>
- rejects the one-level alias spelling of a single-level terminal block <sub>`dynamicSymbol`</sub>
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
- grows to take in a pin group's bracket and caption, so labels are placed clear of them <sub>`symbolBounds`</sub>
- still takes in a pin group's box after the terminals are renumbered <sub>`symbolBounds`</sub>
- reserves no room on a device-block coil for brackets it never draws <sub>`symbolBounds`</sub>
- grows upward instead when the symbol is turned 90 degrees, keeping labels off the caption <sub>`symbolBounds`</sub>
- never lets a bracket and its caption lines overlap each other, at any rotation <sub>`captionLineAnchors`</sub>
- stacks caption lines downwards on the paper, sharing one centre line, at any rotation <sub>`captionLineAnchors`</sub>

### frameEditor

- shows paper coordinates and stores them relative to the title block corner <sub>`frame editor geometry`</sub>
- adds one title-block grid line without touching the original template <sub>`frame editor geometry`</sub>
- adds nothing for a zero-length line or a flat rectangle <sub>`frame editor geometry`</sub>
- turns a rectangle into four grid lines whichever way it is dragged <sub>`frame editor geometry`</sub>
- keeps only the dominant axis when Shift constrains a line <sub>`frame editor geometry`</sub>
- adds a fixed-text cell for the text tool and a bound cell for the placeholder tool <sub>`frame editor geometry`</sub>
- adds a sized bound cell with its four border lines for the title-cell tool <sub>`frame editor geometry`</sub>
- makes the rectangle the logo frame and replaces it when drawn again <sub>`frame editor geometry`</sub>
- picks the grid line when on it, the cell when inside it and nothing on empty paper <sub>`frame editor selection and editing`</sub>
- moves an item by the drag and deletes it <sub>`frame editor selection and editing`</sub>
- keeps either the fixed text or the bind on a cell, never both <sub>`frame editor selection and editing`</sub>
- filters placeholders by part of the name and lists all for an empty query <sub>`frame editor placeholders and pre-save checks`</sub>
- reads the {names} out of a bind string in order <sub>`frame editor placeholders and pre-save checks`</sub>
- finds nothing to report on the bundled frame <sub>`frame editor placeholders and pre-save checks`</sub>
- counts unknown placeholders and items outside the frame without blocking the save <sub>`frame editor placeholders and pre-save checks`</sub>
- reports revision widths that do not add up and a zero text height as blocking errors <sub>`frame editor placeholders and pre-save checks`</sub>
- suggests an unused -copy id and accepts only file-name-safe ids <sub>`frame editor placeholders and pre-save checks`</sub>
- opens a bundled frame as a new draft with a -copy id <sub>`frame editor store`</sub>
- undoes and redoes editor edits step by step without sending any drawing command <sub>`frame editor store`</sub>
- skips no-op edits, drops the redo steps on a new edit and caps the history <sub>`frame editor store`</sub>
- saves a user frame under its own id, reloads the list and clears the unsaved mark <sub>`frame editor store`</sub>
- opens save-as instead of writing a draft that has no file yet <sub>`frame editor store`</sub>
- refuses an unsafe or taken id on save-as and otherwise writes under the new id and the entered name <sub>`frame editor store`</sub>
- does not save while the pre-save check reports an error <sub>`frame editor store`</sub>
- asks before discarding unsaved changes and closes when told to discard <sub>`frame editor store`</sub>

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

### leader

- runs from a note's bottom edge to its target's top edge when the note sits above it <sub>`leaderLine`</sub>
- runs horizontally from a note's near vertical edge when the note sits beside its target <sub>`leaderLine`</sub>
- returns null when the note has no leader target <sub>`leaderFor`</sub>
- recomputes after the note moves, following whichever position it is drawn at <sub>`leaderFor`</sub>
- returns null when the target id does not exist on the sheet <sub>`leaderFor`</sub>
- resolves a symbol instance's bounding box from its definition <sub>`entityBounds`</sub>
- ends on exactly the outline the drawing shows, counting a text mark as its own point <sub>`entityBounds`</sub>
- stops outside a symbol's pin-group caption, not on the symbol body <sub>`entityBounds`</sub>
- resolves shapes, harnesses, wires and junctions, and refuses panel-only entities <sub>`entityBounds`</sub>

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
- moves a wire legend block's placement the same way as any other entity <sub>`placeMacroEntities`</sub>
- works on copies, leaving the macro definition untouched so it can be placed again <sub>`placeMacroEntities`</sub>
- builds a throwaway sheet holding only the chosen variant's entities <sub>`macroSheet`</sub>
- shows the Japanese name when the UI is Japanese and the English name otherwise <sub>`macroName`</sub>
- falls back to the English name when a macro has no Japanese name <sub>`macroName`</sub>

### mate

- only symbol ids starting with connector_ are treated as mating connectors <sub>`mate: connector family gate`</sub>
- reads the mate declared on a connector instance <sub>`mate: declaredMate`</sub>
- returns empty string when there is no declaration <sub>`mate: declaredMate`</sub>
- a close pair on one sheet produces exactly one mate link <sub>`mate: mateLinks`</sub>
- a pair more than 50mm apart produces no mate link <sub>`mate: mateLinks`</sub>
- includes exactly 50mm in the distance limit <sub>`mate: mateLinks`</sub>
- a connector with no declaration produces no mate link <sub>`mate: mateLinks`</sub>
- clamps toward an external point onto the rectangle's boundary <sub>`mate: rectBoundaryPointToward`</sub>
- pushes a point inside the rectangle out onto its nearest edge <sub>`mate: rectBoundaryPointToward`</sub>
- runs between the facing outline edges of the two connectors <sub>`mate: mateLinkSegment (SVG/canvas parity)`</sub>

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

### panelMarks

- sits centred under its footprint, above it when asked, and a standalone one stays put <sub>`盤の銘板`</sub>
- takes the schematic description as its first line when it follows it <sub>`盤の銘板`</sub>
- measures the horizontal distance between two footprint centres <sub>`盤の寸法線`</sub>
- places a dimension along the larger distance, 10mm outside, and nothing for the same point <sub>`盤の寸法線`</sub>
- finds the footprint under a click and picks dimensions and nameplates <sub>`盤の寸法線`</sub>

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

### pinGroups

- parses a range and a dash-less single pin to one group each <sub>`parsePinGroups`</sub>
- reports a backwards range as invalid <sub>`parsePinGroups`</sub>
- reports a range naming an unknown pin <sub>`parsePinGroups`</sub>
- reports unparseable text without throwing, keeping the rest <sub>`parsePinGroups`</sub>
- rejects both of two overlapping groups but keeps a third that does not overlap <sub>`parsePinGroups`</sub>
- accepts a multi-level range that stays within one level <sub>`parsePinGroups`</sub>
- refuses a pin whose own number contains a hyphen, with its own reason <sub>`parsePinGroups`</sub>
- splits into chunks of at most 4 characters <sub>`wrapCaption`</sub>
- draws a vertical bar further left than left-side pins, with the caption further out still <sub>`pinGroupBracket`</sub>
- puts a block's bracket beyond the pin stubs and its caption beyond the bracket <sub>`pinGroupBracket`</sub>
- puts a connector's bracket to the right of its pin stubs <sub>`pinGroupBracket`</sub>
- brackets the outside of a multi-level terminal block's own column, not the gap between columns <sub>`pinGroupBracket`</sub>
- reports a range that spans two levels instead of quietly drawing nothing <sub>`pinGroupBracket`</sub>
- draws a bracket for a range written with the overridden labels <sub>`pinGroupBracket`</sub>
- keeps a multi-level bracket outside its own column after an override <sub>`pinGroupBracket`</sub>
- refuses a bracket across mixed pin directions <sub>`pinGroupBracket`</sub>

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
- does not escape the coil location onto the contact's own value label <sub>`relayXref`</sub>
- measures the wire note at the text height that sheet draws it with <sub>`relayXref`</sub>
- selects the display mode from the device_block attribute <sub>`relayXref`</sub>
- only collects coils flagged as a device block <sub>`relayXref`</sub>
- excludes device-block coils from the separate contact map <sub>`relayXref`</sub>
- keeps the device block layout no narrower than the pin span <sub>`relayXref`</sub>
- grows the device block for the part number and keeps the pins reachable <sub>`relayXref`</sub>
- keeps the row dividers clear of the height the leads attach at <sub>`relayXref`</sub>
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
- shrinks symbol lettering with the zoom instead of keeping a minimum size, and drops it when unreadable <sub>`drawSymbol text placement`</sub>
- keeps the lettering inside a symbol where the symbol definition puts it, whatever the rotation <sub>`drawSymbol text placement`</sub>
- keeps a one- or two-character terminal number inside its own mark, whatever the rotation <sub>`drawSymbol text placement`</sub>
- puts an unturned terminal block's number just over a third of a character height below its mark centre on the canvas, a smaller offset than the exported drawing's half <sub>`drawSymbol text placement`</sub>
- draws a jumper as a bar only, with no round mark at either end <sub>`drawSymbol text placement`</sub>
- draws the overridden terminal number instead of the symbol's raw pin number <sub>`drawSymbol text placement`</sub>
- attributes a jumper bar between renumbered terminals to the marks whose printed labels it connects <sub>`drawSymbol text placement`</sub>
- keeps the lead lines on the coil's own pins, whatever the rotation <sub>`drawDeviceBlock`</sub>
- draws the same upright outline whatever the rotation <sub>`drawDeviceBlock`</sub>
- keeps the leads on the pin axis however wide the block grows <sub>`drawDeviceBlock`</sub>
- runs a turned coil's leads vertically without crossing the contact rows <sub>`drawDeviceBlock`</sub>
- keeps every line of text inside its own cell and clear of the others, whatever the rotation <sub>`drawDeviceBlock`</sub>
- strokes the outline in the selection colour while the coil is selected <sub>`drawDeviceBlock`</sub>
- prints the function label under the block outline <sub>`drawDeviceBlock`</sub>
- hides the contact image rows with the reference-designator view class <sub>`drawDeviceBlock`</sub>
- hides the mate link together with the symbols view class <sub>`renderSheet mate links`</sub>
- keeps the mate link when only the reference-designator class is hidden <sub>`renderSheet mate links`</sub>
- hides the jumper bar together with the symbols view class, matching the other symbol-attached drawings <sub>`jumper bar view-class gating (Task 3, フェーズ6積み残し)`</sub>
- keeps the jumper bar when only the reference-designator class is hidden <sub>`jumper bar view-class gating (Task 3, フェーズ6積み残し)`</sub>
- draws a note's leader dot exactly the size of a wire junction dot <sub>`renderSheet dots`</sub>
- draws the header row and the rows derived from the sheet's wires <sub>`wire legend block drawing`</sub>
- outlines the legend block in the selection colour while it is selected <sub>`wire legend block drawing`</sub>
- does not draw the designator when hide_reference is true <sub>`hidden designator`</sub>

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
- accepts a list with the right length and no duplicates or empty items <sub>`validTerminalNumbers`</sub>
- rejects a list whose length does not match the pin count <sub>`validTerminalNumbers`</sub>
- rejects a list with a duplicate label <sub>`validTerminalNumbers`</sub>
- rejects a list with an empty item <sub>`validTerminalNumbers`</sub>
- judges the list on its trimmed values <sub>`validTerminalNumbers`</sub>
- rejects a label containing the stored list's own separator <sub>`validTerminalNumbers`</sub>
- converts a raw pin number to the displayed label and back <sub>`terminalLabels / rewriteJumperLabels`</sub>
- never builds a label map for a symbol that is not a terminal block <sub>`terminalLabels / rewriteJumperLabels`</sub>
- rewrites only the terminal numbers it recognises and round-trips <sub>`terminalLabels / rewriteJumperLabels`</sub>
- falls back to the symbol's default numbers when there is no override <sub>`terminalNumbersOfInstance / terminalLabels`</sub>
- uses a valid override and builds a raw-to-display rename map <sub>`terminalNumbersOfInstance / terminalLabels`</sub>
- falls back to the defaults when the override's item count is wrong <sub>`terminalNumbersOfInstance / terminalLabels`</sub>
- numbers every pin from the start value on a single-level block <sub>`terminalRenumberPlan`</sub>
- keeps the level letters on a multi-level block and numbers only the pole <sub>`terminalRenumberPlan`</sub>
- applies the step and the prefix <sub>`terminalRenumberPlan`</sub>
- counts poles and levels from the terminal numbers <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- normalises one level back to the plain single-level symbol id <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- returns null for a symbol that is not a terminal block and for an out-of-range level count <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- expands labels into lettered levels and drops the vanishing ones when decreasing <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- keeps a label that does not end in a level letter intact as the base <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- only strips a trailing letter that matches level a's own letter, so two poles overridden with different trailing letters keep distinct bases <sub>`level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)`</sub>
- pushes the numbers after the chosen terminal and hands the vacated one to the new spare <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- does not renumber anything when appending at the end <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- keeps a prefixed label's prefix and shifts only its numeric part <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- counts the insertion point in the chart's number order so a second insertion still works <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- leaves the following numbers alone and gives the spare the vacant number when not renumbering <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- returns null when not renumbering would collide with an existing number <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- returns null when the insertion point's label has no digits <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- returns null for a non-positive count <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- returns null when the insertion point names an unknown terminal <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- adds one terminal per level on a multi-level block <sub>`insertSpareTerminals (Task 4: 予備端子の挿入, Rust側 insert_spare_terminals の双子)`</sub>
- keeps adjacency based on the raw pin layout, not on the arithmetic difference between displayed labels <sub>`parseJumperSpec with an overridden display order`</sub>

### textSize

- scales drawing text with the zoom like the shapes, and drops it once it is too small to read <sub>`drawing text size on screen`</sub>
- skips unreadably small text without setting a 0px font, which WebKit ignores and would draw at the previous size <sub>`drawing text size on screen`</sub>

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

### wireLegend

- counts a breakout wire under its core colour and cable gauge instead of a blank row <sub>`wire legend with run-cable breakout wires`</sub>
- has only the header row for a sheet with no wires <sub>`wire legend block geometry`</sub>
- sums the length of wires sharing the same colour, gauge and part number into one row <sub>`wire legend block geometry`</sub>
- sorts rows by colour then by gauge <sub>`wire legend block geometry`</sub>
- rounds a summed length instead of showing floating point noise <sub>`wire legend block geometry`</sub>
- merges a colour written in English and in Japanese into one row <sub>`wire legend block geometry`</sub>
- prints the same colour name as the wire annotation on the same sheet <sub>`wire legend block geometry`</sub>
- leaves the part number blank when the colour and gauge are not in the wire-part table <sub>`wire legend block geometry`</sub>
- builds an add_entity command that only sets the placement <sub>`wire legend block geometry`</sub>
- lists the wire legend entities placed on a sheet <sub>`wire legend block geometry`</sub>

### wireNotes

- annotates a wire with its gauge, colour and cut length <sub>`wireNoteText`</sub>
- leaves out attributes the wire does not have <sub>`wireNoteText`</sub>
- gives a wire with no attributes no note at all <sub>`wireNoteText`</sub>
- gives a plain black wire no note <sub>`wireNoteText`</sub>
- matches colour names ignoring capitals and surrounding space <sub>`wireNoteText`</sub>
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
- skips a relay coil and the contact it drives <sub>`G13 device cross-reference`</sub>
- still cross-references a contactless coil that appears on two sheets <sub>`G13 device cross-reference`</sub>
- a tidy variant copy adds no cross-reference address to the original or to itself <sub>`G13 device cross-reference`</sub>
- maps each symbol entity id to the text drawn beside it <sub>`G13 device cross-reference`</sub>
- lists every other sheet for a designator on three sheets <sub>`G13 device cross-reference`</sub>
- places the text right of the symbol outline, level with the designator label <sub>`G13 device cross-reference`</sub>
- clears a designator label wider than the symbol itself <sub>`G13 device cross-reference`</sub>

### blocks

- groups consecutive tool calls into one run that text or thinking ends, flagging failures and running steps <sub>`agent reply display`</sub>
- shows a message without blocks the old way: the text, then its tools as one run <sub>`agent reply display`</sub>
- opens only the last row while streaming, keeps finished and loaded replies closed, and honours the user's choice <sub>`agent reply display`</sub>
- rounds thinking time to seconds and hides it below one second or when unknown <sub>`agent reply display`</sub>
- reports how many KB of a truncated result were left out, and nothing when it is complete <sub>`agent reply display`</sub>
- pretty-prints tool input as two-space JSON and shows strings as they are <sub>`agent reply display`</sub>

### markdown

- renders headings, lists, tables, emphasis, inline code, quotes and rules <sub>`reply markdown`</sub>
- gives code blocks a language label and a copy button, and draws an unclosed block up to what has arrived <sub>`reply markdown`</sub>
- shows raw HTML in a reply as text instead of running it <sub>`reply markdown`</sub>
- keeps only http(s) links and turns javascript:, file: and data: links into plain text <sub>`reply markdown`</sub>
- does not load images; an image becomes a link labelled with its alt text <sub>`reply markdown`</sub>
- treats only http(s) URLs as safe to open externally <sub>`reply markdown`</sub>

### File keyboard shortcuts

- Cmd+N is new, Cmd+O is open, Cmd+S is save and Shift+Cmd+S is save-as <sub>`editor shortcuts: ファイル操作`</sub>
- Ctrl works the same as Cmd on Windows and Linux <sub>`editor shortcuts: ファイル操作`</sub>
- plain N, O and S are not file shortcuts <sub>`editor shortcuts: ファイル操作`</sub>
- file shortcuts do nothing while a modal is open <sub>`editor shortcuts: ファイル操作`</sub>
- adding Option/Alt makes it a different shortcut, not a file action <sub>`editor shortcuts: ファイル操作`</sub>
- Shift+Cmd+N and Shift+Cmd+O are not file actions <sub>`editor shortcuts: ファイル操作`</sub>
- the decision is made on the physical key, so non-Latin layouts still work <sub>`editor shortcuts: ファイル操作`</sub>
- Cmd+Z is undo and Shift+Cmd+Z is redo, with Ctrl working the same <sub>`editor shortcuts: 元に戻す・やり直し`</sub>
- plain Z, Option combinations and other keys are not undo or redo <sub>`editor shortcuts: 元に戻す・やり直し`</sub>

### entityKindLabels

- has a label for every entity kind that can be placed on a sheet <sub>`entity kind labels`</sub>
- resolves every label through keys that exist in both languages <sub>`entity kind labels`</sub>
- names the wire legend block in both languages <sub>`entity kind labels`</sub>

### FreeCAD link display (properties panel)

- prints a placement as (x, y, z) mm / angle°, the same text as the FreeCAD panel <sub>`mechLink`</sub>
- rounds computed fractions to six significant digits instead of showing 45.500000000000004 <sub>`mechLink`</sub>
- shows only the object name until a placement has been synced <sub>`mechLink`</sub>

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
- keeps the mate attribute of a non-connector symbol, which the panel has no field for <sub>`symbolAttrs`</sub>
- removes the mate attribute of a connector when the field is emptied <sub>`symbolAttrs`</sub>
- leaves attributes the panel does not own untouched <sub>`symbolAttrs`</sub>
- sends nothing when colour, gauge, part number and length are unchanged <sub>`wireUpdateCommand`</sub>
- a hand-edited length becomes an update whose length source is manual <sub>`wireUpdateCommand`</sub>
- overwriting a FreeCAD-measured length by hand is flagged so the panel can warn <sub>`wireUpdateCommand`</sub>
- changing only colour or gauge keeps the FreeCAD length source <sub>`wireUpdateCommand`</sub>
- writes part, cores, gauge, shield and length in one update and keeps the core assignments <sub>`cableUpdateCommand`</sub>
- refuses a core count below one or a gauge or length that is not a number <sub>`cableUpdateCommand`</sub>
- sends nothing when nothing changed <sub>`cableUpdateCommand`</sub>
- keeps the kind, run and core ends of a single-line cable <sub>`cableUpdateCommand`</sub>
- writes text, height, position, following and part in one update without an empty second line <sub>`nameplateUpdateCommand`</sub>
- sends nothing for a non-positive text height or when nothing changed <sub>`nameplateUpdateCommand`</sub>

### ribbonLayout

- keeps one column when the buttons fit in it <sub>`ribbon small-button columns`</sub>
- moves the buttons that no longer fit into a further column <sub>`ribbon small-button columns`</sub>
- makes no column at all for a group with no small buttons <sub>`ribbon small-button columns`</sub>
- adds as many columns as the buttons need <sub>`ribbon small-button columns`</sub>

### symbolPickerDynamic

- keeps the level count inside the supported range of 1 to 3 levels <sub>`clampLevels`</sub>
- rounds a fractional level count and falls back to one level for a non-number <sub>`clampLevels`</sub>
- keeps the pole count inside the supported range of 1 to 50 poles <sub>`clampPins`</sub>
- offers the levels stepper for a terminal block only, not for a connector <sub>`supportsLevels`</sub>
- places the plain single-level terminal block id when the level count is left at one <sub>`dynamicSymbolId`</sub>
- places the multi-level terminal block id when the level count is raised <sub>`dynamicSymbolId`</sub>
- never adds a level suffix to a connector, whatever the level count says <sub>`dynamicSymbolId`</sub>
- has the levels stepper label and hint in both the English and the Japanese catalogue <sub>`段数ステッパーの表示文字列`</sub>

### updateStatus

- says up to date in green with the time of the last check <sub>`update status line`</sub>
- names the new version with its release date, or the version alone without a date <sub>`update status line`</sub>
- shows the download in MB, or only what arrived when the total is unknown <sub>`update status line`</sub>
- shows a failure in red with its reason <sub>`update status line`</sub>
- says quietly that a development build or browser mode does not check <sub>`update status line`</sub>
- excerpts the first three non-empty note lines and keeps only the date part of the release date <sub>`update status line`</sub>

### chatComposer

- tokens are added up across the turns of a conversation <sub>`sumUsage`</sub>
- Copilot's premium requests are counted separately from tokens, as a fraction <sub>`sumUsage`</sub>
- a conversation with no usage yet counts as zero <sub>`sumUsage`</sub>
- sends on Cmd+Enter <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- sends on Ctrl+Enter <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- does not send on a plain Enter, which stays a line break <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- does not send on Shift+Enter <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- does not send while an IME composition is in progress, even with Cmd or Ctrl <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- does not send when the Enter is reported as keyCode 229 by the IME <sub>`isSendShortcut (チャット入力の送信キー)`</sub>
- does not send on keys other than Enter <sub>`isSendShortcut (チャット入力の送信キー)`</sub>

### Unsaved guard when closing or reloading

- answering no to the discard confirmation keeps the window open <sub>`close guard: アプリ (Tauri)`</sub>
- answering yes to the discard confirmation lets the window close <sub>`close guard: アプリ (Tauri)`</sub>
- with nothing unsaved the window closes without asking <sub>`close guard: アプリ (Tauri)`</sub>
- detaching the guard stops it from intercepting close requests <sub>`close guard: アプリ (Tauri)`</sub>
- in a plain browser the leave confirmation appears only when something is unsaved <sub>`close guard: ブラウザ検証`</sub>
- detaching the guard removes the browser leave confirmation <sub>`close guard: ブラウザ検証`</sub>

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
- a failed overwrite logs the path it tried and the drawing stays unsaved <sub>`file menu: 保存`</sub>
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
- a recent file that no longer exists is dropped from the list with the reason logged <sub>`file menu: 新規と開く`</sub>
- a recent file that failed for a temporary reason stays in the list <sub>`file menu: 新規と開く`</sub>
- only a missing-file failure is recognised as the file being gone <sub>`fileActions helpers`</sub>
- exporting DXF asks for a path defaulting to <sheet>.dxf and writes the active sheet <sub>`file menu: シートの書き出し`</sub>
- exporting KiCad defaults to <sheet>.kicad_sch, and SVG/PDF keep their extension filters <sub>`file menu: シートの書き出し`</sub>
- cancelling the save dialog exports nothing <sub>`file menu: シートの書き出し`</sub>
- without an open sheet the export logs a hint and shows no dialog <sub>`file menu: シートの書き出し`</sub>
- a failed export is logged with its reason <sub>`file menu: シートの書き出し`</sub>
- a DXF or KiCad export logs what it wrote per kind and what it could not write <sub>`file menu: シートの書き出し`</sub>
- a DXF import with no wire layer logs the candidate layers and remembers them <sub>`file menu: DXFの配線レイヤ`</sub>
- re-importing with a chosen wire layer reads the same file with only that layer as wires <sub>`file menu: DXFの配線レイヤ`</sub>
- re-importing does nothing when no DXF import is remembered <sub>`file menu: DXFの配線レイヤ`</sub>

### Native dialogs (confirm / notice / file picker)

- a confirmation goes through the plugin's ask as a warning and yes means true <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- choosing no in a confirmation returns false <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- the confirmation title and kind can be overridden by the caller <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- a notice goes through the plugin's message dialog <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- a save target is chosen with the plugin's save dialog, given a default name and filters <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- a file to open is chosen with a single-selection dialog <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- cancelling the open dialog yields null, and so does a multi-selection result <sub>`native dialogs: Tauriのdialogプラグイン`</sub>
- in a plain browser a confirmation uses window.confirm <sub>`native dialogs: Tauri外 (ブラウザ検証) のフォールバック`</sub>
- in a plain browser a notice uses window.alert <sub>`native dialogs: Tauri外 (ブラウザ検証) のフォールバック`</sub>
- without a native file dialog the save target is typed in, defaulting to the file name <sub>`native dialogs: Tauri外 (ブラウザ検証) のフォールバック`</sub>
- the path to open is typed in too, and cancelling yields null <sub>`native dialogs: Tauri外 (ブラウザ検証) のフォールバック`</sub>
- a host without dialogs answers no and null instead of throwing <sub>`native dialogs: Tauri外 (ブラウザ検証) のフォールバック`</sub>

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
- compiles every message in every catalog <sub>`i18n`</sub>
- keeps the same named placeholders as English in every translation <sub>`i18n`</sub>
- keeps the same number of plural forms as English in every translation <sub>`i18n`</sub>
- shows each language name as its own endonym <sub>`i18n`</sub>
- resolves language tags leniently and falls back to English for unknown values <sub>`i18n`</sub>

### chat

- text deltas append to the in-progress message of a known conversation <sub>`chat store: applyAgentEvent`</sub>
- turn_completed stops streaming and finalizes the token usage <sub>`chat store: applyAgentEvent`</sub>
- when no deltas arrived, the body is filled from turn_completed's result <sub>`chat store: applyAgentEvent`</sub>
- tool calls become chips, resolved to success/failure by id <sub>`chat store: applyAgentEvent`</sub>
- builds the reply as blocks in the order they happened, joining runs of one kind and timing the thinking <sub>`chat store: applyAgentEvent`</sub>
- messages loaded from the conversation list keep their blocks and tool results, dropping block kinds it does not know <sub>`chat store: applyAgentEvent`</sub>
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
- passes attachments to the backend and shows their names on the message <sub>`chat store: アクション`</sub>
- reads attachment information back from saved conversations <sub>`chat store: アクション`</sub>
- send creates a conversation and adopts the server-assigned id <sub>`chat store: アクション`</sub>
- events arriving before send resolves still land in the same conversation <sub>`chat store: アクション`</sub>
- empty prompts and sends during streaming are ignored <sub>`chat store: アクション`</sub>
- an interrupting send cancels the running turn and then sends the new prompt to the same conversation <sub>`chat store: アクション`</sub>
- an interrupting send behaves like a normal send when nothing is running <sub>`chat store: アクション`</sub>
- an interrupting send is refused while the first send is still waiting for its conversation id <sub>`chat store: アクション`</sub>
- an empty interrupting send is ignored and leaves the running turn alone <sub>`chat store: アクション`</sub>
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

### chatAttachments

- accepts images, PDFs, Excel workbooks and CSV files only, judging the kind by extension too <sub>`pending chat attachments`</sub>
- trusts a known extension over the type the browser reports, since Windows labels CSV files as Excel <sub>`pending chat attachments`</sub>
- lets spreadsheets go to OpenAI-compatible models, which cannot read PDFs, because they are sent as text <sub>`pending chat attachments`</sub>
- marks files over 20 MB as too large and shows sizes in MB or KB <sub>`pending chat attachments`</sub>
- keeps pasted or browser files as base64 and turns them into outgoing attachments <sub>`pending chat attachments`</sub>
- names pasted images in order since the clipboard has no file name <sub>`pending chat attachments`</sub>
- keeps picked or dropped files as paths with the preview from Rust <sub>`pending chat attachments`</sub>
- blocks the sixth attachment and unblocks it once one is removed <sub>`pending chat attachments`</sub>
- blocks a PDF for an OpenAI-compatible model but not for other providers <sub>`pending chat attachments`</sub>
- leaves attachments with a problem out of what is sent <sub>`pending chat attachments`</sub>
- takes only images from a paste and reacts only to drags that carry files <sub>`pending chat attachments`</sub>

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

- recomputes the sheet annotations only when the document changes <sub>`document store`</sub>
- entity_upserted / entity_removed patches update the mirrored sheet <sub>`document store`</sub>
- project_replaced swaps the whole mirrored project <sub>`document store`</sub>
- sheet add/remove patches keep sheet order <sub>`document store`</sub>
- sheet-meta patches never touch the entities <sub>`document store`</sub>
- patches with an older revision are discarded (duplicate delivery is safe) <sub>`document store`</sub>
- a mech_links_replaced patch replaces the FreeCAD link list <sub>`document store`</sub>
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
- picks the first letter that is not registered as the next free variant, or none when all 26 are used <sub>`adding a variant to an existing macro from the save dialog`</sub>
- lists registered keys plus the next free key as chips, marking a selected registered key as an overwrite <sub>`adding a variant to an existing macro from the save dialog`</sub>
- suggests existing macros whose name contains what was typed <sub>`adding a variant to an existing macro from the save dialog`</sub>
- choosing an existing macro takes its name and category and selects its next free variant <sub>`adding a variant to an existing macro from the save dialog`</sub>
- saves into the existing macro by passing its id and the chosen variant key <sub>`adding a variant to an existing macro from the save dialog`</sub>
- asks before overwriting a registered variant and saves only after confirmation <sub>`adding a variant to an existing macro from the save dialog`</sub>
- saving to another variant from the overwrite prompt uses the next free key <sub>`adding a variant to an existing macro from the save dialog`</sub>
- clearing the existing macro goes back to saving a new macro with only variant A <sub>`adding a variant to an existing macro from the save dialog`</sub>

### nameplates

- starts the first line from the schematic description when opened on a part <sub>`銘板の挿入`</sub>
- inserts a nameplate tied to the part with one add_entity, or a standalone one <sub>`銘板の挿入`</sub>

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
- saves the settings to the project and generates the I/O sheet from them and the module spec <sub>`PLC I/O sheet generation settings`</sub>
- generates with the horizontal bus style and the share-or-split placement <sub>`PLC I/O sheet generation settings`</sub>
- starts from the settings saved with the project and does not save them again when unchanged <sub>`PLC I/O sheet generation settings`</sub>
- does not save the settings when saving with the project is unchecked <sub>`PLC I/O sheet generation settings`</sub>
- saves the pending edits before generating the sheet <sub>`PLC I/O sheet generation settings`</sub>
- falls back to the default address style when the module is not in the parts library <sub>`PLC I/O sheet generation settings`</sub>

### Project file (open / save / recent files)

- right after startup the loaded drawing counts as saved, with nothing unsaved <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- editing the drawing marks it as having unsaved changes <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
- undoing back to the saved content still counts as unsaved <sub>`projectFile store: 開いているファイルと未保存の編集`</sub>
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
- duplicates in stored data are removed when the list is read at startup <sub>`projectFile store: 最近使ったファイル`</sub>
- stored data over the cap is trimmed when the list is read at startup <sub>`projectFile store: 最近使ったファイル`</sub>
- a written list can be read back from localStorage <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- an empty store starts with an empty list <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- broken JSON in storage yields an empty list instead of failing startup <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- a stored value that is not an array yields an empty list <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- elements that are not strings are dropped from the list <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- a store that refuses writes does not throw; the list only lives for the session <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- a runtime without localStorage is treated as an empty list <sub>`projectFile store: 最近使ったファイルの保管 (localStorage)`</sub>
- fileNameOf returns the last path segment for both separators <sub>`projectFile helpers`</sub>
- isDxfPath recognises the .dxf extension regardless of case <sub>`projectFile helpers`</sub>
- isKicadPath recognises the .kicad_sch extension regardless of case <sub>`projectFile helpers`</sub>

### projectInfo

- opens with the project's name/number and the active sheet's title block <sub>`project info dialog store`</sub>
- sends no command when nothing changed <sub>`project info dialog store`</sub>
- sends only the changed fields, leaving the rest undefined <sub>`project info dialog store`</sub>
- sends every field when every field is changed <sub>`project info dialog store`</sub>
- clears the project number with null when the field is emptied <sub>`project info dialog store`</sub>
- discards the draft and sends nothing on cancel <sub>`project info dialog store`</sub>

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

- offers the nine report kinds of the reports tab <sub>`report generation dialog store`</sub>
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

### sheetPaper

- opens with the sheet's current size and orientation <sub>`sheet paper dialog store`</sub>
- sends no command when nothing changed <sub>`sheet paper dialog store`</sub>
- sends one set_sheet_paper command with the new size and orientation <sub>`sheet paper dialog store`</sub>
- discards the draft and sends nothing on cancel <sub>`sheet paper dialog store`</sub>

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
- lists the same five row orders as the Rust RowOrder enum <sub>`row order (parity with madake-core terminal_chart::RowOrder)`</sub>
- falls back to number order for an unset or unrecognised attribute <sub>`row order (parity with madake-core terminal_chart::RowOrder)`</sub>
- resolves every known word to its row order <sub>`row order (parity with madake-core terminal_chart::RowOrder)`</sub>
- loads the terminal blocks of the sheet and shows the first one <sub>`terminal strip editor store`</sub>
- shows one grid row per terminal with the wiring taken from the drawing <sub>`terminal strip editor store`</sub>
- carries the level letter of each row for multi-level terminal blocks <sub>`terminal strip editor store`</sub>
- falls back to the chart rows to decide whether the level column is needed <sub>`terminal strip editor store`</sub>
- reports no levels for a single-level terminal block <sub>`terminal strip editor store`</sub>
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
- changes the row order with one update_entity command <sub>`terminal strip editor store`</sub>
- drops the row_order attribute entirely when number order is chosen again <sub>`terminal strip editor store`</sub>
- reflects the row order the chart already came back sorted in <sub>`terminal strip editor store`</sub>
- jumps to the drawing by passing the sheet and entity id of the current block to reveal <sub>`terminal strip editor store`</sub>
- closes the editor before revealing the drawing so the jump is not hidden behind the modal <sub>`terminal strip editor store`</sub>
- restores the same block and row selection when the editor is reopened after a jump <sub>`terminal strip editor store`</sub>
- keeps no resume memo when the editor is closed by hand <sub>`terminal strip editor store`</sub>
- reloads the list and chart only once per sort change, not twice <sub>`terminal strip editor store`</sub>
- does nothing when asked to jump to the drawing without a terminal block <sub>`terminal strip editor store`</sub>
- reloads the grid when the drawing changes from outside the editor, e.g. an undo <sub>`terminal strip editor store`</sub>
- does not fetch anything while the editor is closed <sub>`terminal strip editor store`</sub>
- summarizes the terminal block check as counts per severity <sub>`terminal strip editor store`</sub>
- reports a clean terminal block when the check finds nothing <sub>`terminal strip editor store`</sub>
- clears the selection and the previous check result when another block is picked <sub>`terminal strip editor store`</sub>
- shows an empty grid on a sheet without any terminal block <sub>`terminal strip editor store`</sub>
- throws away the chart, the selection and the check result when closed <sub>`terminal strip editor store`</sub>
- previews the renumbering as an old-to-new mapping and keeps level letters <sub>`renumber popover`</sub>
- refuses a step of zero and names the label that would repeat <sub>`renumber popover`</sub>
- sends no command at all for a plan it cannot apply <sub>`renumber popover`</sub>
- rounds a negative start and a fractional step into whole numbers before sending <sub>`renumber popover`</sub>
- reports that there is nothing to renumber on a sheet without a terminal block <sub>`renumber popover`</sub>
- previews the jumper specification as it will read after the renumbering <sub>`renumber popover`</sub>
- shows no jumper line when the block carries no jumpers <sub>`renumber popover`</sub>
- reports the block's current level count <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- falls back to counting distinct level letters in the chart rows <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- sends one set_terminal_levels command with the requested level count <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- sends nothing when the requested level count matches the current one <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- refuses an out-of-range level count without sending a command <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- keeps the backend's refusal reason in error and leaves the drawing untouched <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- previews the new symbol id, the new terminal numbers and the jumpers that follow <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- shows no preview for the level count the block already has <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- lists each vanishing terminal that still has a connection, with its target <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- lists a jumper that still names a vanishing level as a blocker <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- never blocks an increase in the level count <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- refuses a level decrease that has a blocker without sending a command <sub>`multi-level stepper (Task 3: 多段端子の生成・解除)`</sub>
- sends one insert_spare_terminals command with the requested position and count <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- sends null for after when inserting at the end <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- keeps following labels and gives the spare the next free number when 'leave vacant' is chosen at the end <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- reports keepDuplicate and sends nothing when 'leave vacant' would collide with an existing number <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- refuses a non-positive count without sending a command <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- keeps the backend's refusal reason in error and leaves the drawing untouched <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- previews the new symbol id and the new spare's labels <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- flags a non-positive count as an issue <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- flags a count that would exceed the pole cap <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- lists one insertion-point option per pole, using level a's label on a multi-level block <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- clamps the count stepper's input to a positive integer <sub>`spare insertion popover (Task 4: 予備端子の挿入)`</sub>
- formats accessory rows as terminal:kind:part, trimming the part number and dropping commas and colons <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- formats no rows as an empty string <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- turns chart accessories into popover rows, mapping the end position to 'end' <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- assigns a part with one update_entity that writes the part number and the terminal kind <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- removes a stale terminal kind when the assigned part has none <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- drops the accessories attribute when every accessory row is removed <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- writes the accessories attribute in the documented format with one update_entity <sub>`part assignment and accessories (M4 phase 5 Task 5)`</sub>
- toggles the selected terminals in the swap setting, adding new ones and removing swapped ones <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>
- keeps unknown terminal numbers and drops blank items when toggling the swap setting <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>
- cannot swap while no terminal is selected <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>
- swaps the selected terminals with one update_entity that writes the swapped attribute and keeps the selection <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>
- drops the swapped attribute when every swapped terminal is swapped back <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>
- carries whether each row is swapped, treating a missing flag as not swapped <sub>`inside/outside swap (M4 phase 5 Task 6)`</sub>

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

### updates

- shows the notice with the version, date and notes when the startup check finds an update <sub>`updates store`</sub>
- does not check at startup when turned off, in a development build or in browser mode <sub>`updates store`</sub>
- stays silent when the startup check cannot reach the server <sub>`updates store`</sub>
- reports up to date or the reason of a failure when checked by hand <sub>`updates store`</sub>
- counts the download progress and relaunches once the update is installed <sub>`updates store`</sub>
- asks first when the drawing has unsaved changes and does nothing if refused <sub>`updates store`</sub>
- shows the reason and does not relaunch when the download or install fails <sub>`updates store`</sub>
- hides the notice for the dismissed version for the rest of the session <sub>`updates store`</sub>
- only opens the release page for a Linux .deb or .rpm install <sub>`updates store`</sub>

### Tidy variants (sheet copies / adopt)

- start copies the active sheet once per variant, sends the same tidy prompt in a separate conversation per copy, and opens the panel <sub>`variants store: 整え案の開始`</sub>
- a selection is mapped to the copy's entity ids in each prompt <sub>`variants store: 整え案の開始`</sub>
- metrics of the original and every variant are fetched after start, each variant measuring its own sheet <sub>`variants store: 整え案の開始`</sub>
- each variant's prompt names its own sheet id in the measuring and verification steps <sub>`variants store: 整え案の開始`</sub>
- start refuses without a sheet, with a count outside 2..4, and while a comparison is in progress <sub>`variants store: 整え案の開始`</sub>
- a failed copy is logged and nothing starts <sub>`variants store: 整え案の開始`</sub>
- variants count as running while their conversation answers, and adopt waits for all of them <sub>`variants store: 比較・採用・破棄`</sub>
- adopt finishes with the chosen sheet, returns to the original sheet and ends the comparison <sub>`variants store: 比較・採用・破棄`</sub>
- discard cancels the conversations still running and removes every copy <sub>`variants store: 比較・採用・破棄`</sub>
- a failed finish is logged and the comparison stays open <sub>`variants store: 比較・採用・破棄`</sub>
- a run whose copy sheets are gone (an undone start) is cleared with a message instead of lingering <sub>`variants store: 比較・採用・破棄`</sub>
- the conversation of a variant sheet can be looked up from the sheet id <sub>`variants store: 比較・採用・破棄`</sub>
- closing the panel keeps the comparison and it can be reopened; nothing opens without a run <sub>`variants store: 比較・採用・破棄`</sub>
- refreshing metrics leaves a variant without metrics when its sheet cannot be measured <sub>`variants store: 比較・採用・破棄`</sub>

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
- selects a device-block coil anywhere inside the block it draws, not just the small coil <sub>`picking a symbol on the canvas`</sub>
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

### wireLegendPlacement

- places one legend block with the clicked point as its top-left corner <sub>`wire legend block on the canvas`</sub>
- selects the legend block by clicking inside its table, and not outside it <sub>`wire legend block on the canvas`</sub>
- moves a selected legend block by dragging it <sub>`wire legend block on the canvas`</sub>


## FreeCAD add-on (MadakeCAD Link)


### Link API client (Python)

- The client talks to MadakeCAD's Link API on localhost, port 9310 unless told otherwise. <sub>`test_base_url_points_at_the_local_link_api`</sub>
- The connection check and the project snapshot come back as decoded JSON. <sub>`test_health_and_project_are_decoded_from_json`</sub>
- Netlist and parts requests pass the sheet id, search text and category as query parameters. <sub>`test_netlist_and_parts_pass_their_filters_as_query_parameters`</sub>
- Commands are posted as a JSON array and one patch comes back per command, so every write goes through MadakeCAD's Command engine. <sub>`test_commands_are_posted_as_a_json_array_and_return_one_patch_each`</sub>
- Posting with batch asks for the whole array to be one edit (?batch=true), so a panel action costs the user a single undo in MadakeCAD. <sub>`test_a_batch_post_asks_madakecad_for_a_single_undo_step`</sub>
- A stream that dies while being read (app quit mid-message, socket reset, nothing arriving) is reported as a sentence naming the port and telling the user to connect again, never as a raw socket error. <sub>`test_errors_while_reading_the_event_stream_become_readable_link_errors`</sub>
- A refused connection, an HTTP error and invalid JSON all raise LinkError with a message that says what went wrong. <sub>`test_errors_are_reported_as_link_errors_with_a_readable_message`</sub>
- The event stream yields each patch as ("patch", {revision, ops}), joining multi-line data and skipping keep-alive comments. <sub>`test_events_yield_patches_from_the_sse_stream`</sub>
- Lines without an event name are "message" events, data is dispatched at the blank line, and a trailing event without a blank line is still delivered. <sub>`test_parse_sse_defaults_the_event_name_and_dispatches_on_blank_lines`</sub>

### Live follow & settings

- A patch is applied only when its revision is newer than the last one seen; duplicates and older patches are ignored. <sub>`test_only_newer_revisions_are_accepted`</sub>
- The panel reloads when the shown sheet's entities change or when sheets are added, removed, renamed or the project is replaced, but not for edits on another sheet. <sub>`test_refresh_is_needed_for_the_shown_sheet_and_structural_changes`</sub>
- Before a sheet has been chosen, any entity change reloads the panel so the first view is current. <sub>`test_with_no_sheet_shown_yet_any_entity_change_reloads`</sub>
- Adding, replacing or undoing a FreeCAD link in MadakeCAD reloads the panel even while a sheet is shown, so the linked-object and placement columns never show links that are gone. <sub>`test_a_change_to_the_freecad_links_reloads_whatever_sheet_is_shown`</sub>
- An op is scoped to the sheet named by its sheet_id or by the sheet it carries; an op naming no sheet is project-wide and always reloads, so a new project-wide op is never missed. <sub>`test_an_op_that_names_no_sheet_counts_as_project_wide`</sub>
- The port setting keeps integers from 1 to 65535 and falls back to 9310 for anything else (text, 0, too large). <sub>`test_port_setting_accepts_valid_ports_and_falls_back_to_the_default`</sub>

### Part linking & wire-length write-back

- The parts list has one row per symbol, sorted by sheet and designator, with the 3D model of its part number and the FreeCAD object it is linked to. <sub>`test_part_rows_list_symbols_with_their_3d_model_and_linked_object`</sub>
- The wire list has one row per wire with its net or wire number, current length, where the length came from, and the linked route object. <sub>`test_wire_rows_show_net_length_source_and_linked_route`</sub>
- STEP/IGES/BREP files are inserted as shapes, .FCStd files are merged as documents, and anything else cannot be inserted. <sub>`test_model_kind_recognises_shape_files_and_freecad_documents`</sub>
- Linking sends a set_mech_link command with the entity id, the FreeCAD document path, the object name and the sync time; unlinking sends remove_mech_link. <sub>`test_link_commands_carry_the_entity_id_document_path_object_name_and_time`</sub>
- Linking an entity again keeps the placement FreeCAD has already synced for it, so inserting or re-linking a model never throws the panel layout's position away. <sub>`test_relinking_keeps_the_placement_freecad_already_synced`</sub>
- Route objects are matched to wires by their madake_id; lengths are converted from millimetres to metres (1 mm resolution) and objects that are not wires are ignored. <sub>`test_measured_route_lengths_are_converted_to_metres_and_matched_to_wires`</sub>
- The write-back is one set_wire_lengths command per sheet whose entries carry the metre length and the source "freecad", so MadakeCAD can warn before a manual overwrite. <sub>`test_write_back_is_one_command_per_sheet_with_the_freecad_source`</sub>
- Measuring also registers the route object as the wire's link, but only when the wire is not already linked to that same object. <sub>`test_routes_are_linked_only_when_not_already_linked_to_that_object`</sub>
- Pointing a wire at a different route object keeps the placement already synced for that wire, so measuring never erases geometry FreeCAD masters. <sub>`test_relinking_a_route_to_another_object_keeps_its_synced_placement`</sub>

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
- A stub whose two parts sit at the same position cannot be a line, so it is separated out to be reported to the user while the other stubs are still created. <sub>`test_stubs_between_parts_at_the_same_position_are_reported_instead_of_drawn`</sub>
- Ends closer together than a thousandth of a millimetre count as the same point (FreeCAD cannot make a line from them), anything further apart is drawn. <sub>`test_ends_a_thousandth_of_a_millimetre_apart_still_count_as_the_same_point`</sub>
- Highlighting a net selects the FreeCAD objects of its linked parts and of its linked route wires, each object once, ignoring unlinked pins. <sub>`test_highlighting_a_net_selects_its_linked_parts_and_routes_once_each`</sub>
- Syncing a placement sends set_mech_link with the object's base point in millimetres (1 µm resolution) and its rotation in degrees, keeping the existing link data. <sub>`test_placement_command_carries_the_base_in_mm_and_the_rotation_in_degrees`</sub>
- Sync placements writes one command per linked object present in the document and skips objects whose stored placement is already the same. <sub>`test_sync_placements_writes_only_linked_objects_whose_placement_changed`</sub>
- A rotation about Z is stored in degrees, signed by the axis direction; a part tilted about another axis has no panel rotation at all, so it is reported instead of being stored as that angle. <sub>`test_only_a_rotation_about_the_z_axis_is_stored_as_a_panel_rotation`</sub>

