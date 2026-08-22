# AI Assistant

[日本語](09-ai-assistant.ja.md)

MadakeCAD is AI-first by architecture: the assistant edits drawings through exactly the same Command API as the human user, so every AI edit is undoable, live-rendered, and auditable.

## Available

- **In-app chat**: docked left panel and a floating card on the canvas; streaming responses; conversation history saved with the project
- **Claude Code backend**: runs the local Claude Code CLI headlessly and reuses your existing Claude Pro/Max sign-in — no API key required. The agent connects back to MadakeCAD's own MCP server, so it uses the same tools as any other client
- **Tool transparency**: every tool call appears as a chip in the conversation (✓ place_symbol …) with an "applied at rev N" record
- **Safe turn revert**: every turn carries a stable turn ID, and every history entry records its origin (user / agent / mcp). "Revert this turn" re-applies the inverse of *the agent's* edits only, so manual edits made during or after the turn survive. If a manual edit makes the revert impossible (the agent's target is gone), the turn is left untouched and the conflict is reported instead of half-reverting. The revert is itself an ordinary history entry, so plain undo takes it back
- **Verification loop**: after editing, the agent must run `run_verification`, fix any remaining errors and re-verify (up to three rounds), then report the counts — replies end with `検証: エラー0・警告1` style totals. If errors survive three rounds it stops and explains rather than leaving a broken drawing
- **Standards knowledge**: a bundled, editable Markdown note (`resources/knowledge/standards.md`) is injected on every turn — JIS C 0617 symbol usage, reference-designator prefixes, wire colour / sq conventions and ampacity limits, wire-number and harness rules, grid and layout rules, the standard drafting sequence, and how to read each diagnostic code. It is injected even when "read the drawing automatically" is off. **Settings → Agent → Advanced → knowledge file** appends your own Markdown after it, so site-specific rules win
- **Documentation answers with sources**: the agent is given the `docs/` index and told to read the file before answering questions about operation, standards, or what is supported, and to cite it (`出典: 05-wire-management.ja.md`). Unsupported features are answered from the roadmap ("planned for M4") instead of being invented
- **Design review and test plans**: "review this drawing" combines `run_verification` with a five-point human checklist (designator scheme, colour/sq convention, label naming, layout, title-block completeness) into one severity-ranked findings table; the agent proposes fixes and waits for approval. "Write me a test procedure" returns a step → expected-result table
- **Part selection**: `search_parts` backs comparison tables (up to 10 candidates with rating, price, purchase link and a "difference from the current part" column) and alternative suggestions; replacements are proposed, not applied
- **Start templates**: three bundled circuits (24 V control basics, motor starter, emergency stop) apply as a **single history entry** — one undo removes the whole template. Reachable from the Project ribbon, the part-insert group, and the agent's own `list_templates` / `apply_template` tools; `~/MadakeCAD/templates/*.json` adds your own
- **Edit overlay**: regions being edited by the agent pulse in cyan on the canvas so you always see what the AI is touching
- **Drawing context**: the current sheet summary — sheets, nets, and a verification summary (error/warning/info counts plus the top few diagnostics) — is supplied to the agent at the start of each turn
- **Cancel safety**: agent events carry a turn sequence number, so late events from a cancelled turn are discarded instead of landing in the next conversation

## Planned (M3 phase 2 and later)

- **Tidy-up automation**: iterative "arrange placement / tidy wiring / align labels" passes with convergence criteria, one undo step per pass
- **Parallel agents**: multiple conversations working simultaneously or producing alternative proposals
- **Multi-provider**: Anthropic API, OpenAI-compatible endpoints (incl. Ollama local models), Gemini — with credentials in the OS keychain, never in files
