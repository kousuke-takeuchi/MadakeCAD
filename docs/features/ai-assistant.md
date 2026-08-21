# AI Assistant

[日本語](ai-assistant.ja.md)

MadakeCAD is AI-first by architecture: the assistant edits drawings through exactly the same Command API as the human user, so every AI edit is undoable, live-rendered, and auditable.

## Available

- **In-app chat**: docked left panel and a floating card on the canvas; streaming responses; conversation history saved with the project
- **Claude Code backend**: runs the local Claude Code CLI headlessly and reuses your existing Claude Pro/Max sign-in — no API key required. The agent connects back to MadakeCAD's own MCP server, so it uses the same tools as any other client
- **Tool transparency**: every tool call appears as a chip in the conversation (✓ place_symbol …) with an "applied at rev N" record
- **Per-turn undo**: each assistant turn tracks how many undo steps it produced; "revert this turn" is exactly that many undos
- **Edit overlay**: regions being edited by the agent pulse in cyan on the canvas so you always see what the AI is touching
- **Drawing context**: the current sheet summary (nets, verification state) is supplied to the agent

## Planned (M3 — "AI-first drafting")

- **Robustness**: stable turn IDs, edit-origin tracking (user vs. agent) so turn-revert never swallows your manual edits
- **Standards knowledge**: JIS symbol usage, reference-designator rules, and wiring conventions injected into the agent, plus a **verification loop** — the agent runs ERC/electrical checks after editing and fixes its own errors
- **Templates**: standard circuit starting points (24 V control circuit, motor starter, …) usable from chat or the new-project flow
- **Tidy-up automation**: iterative "arrange placement / tidy wiring / align labels" passes with convergence criteria, one undo step per pass
- **Parallel agents**: multiple conversations working simultaneously or producing alternative proposals
- **Multi-provider**: Anthropic API, OpenAI-compatible endpoints (incl. Ollama local models), Gemini — with credentials in the OS keychain, never in files
