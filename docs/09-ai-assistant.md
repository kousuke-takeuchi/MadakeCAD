# AI Assistant

[日本語](09-ai-assistant.ja.md)

MadakeCAD is AI-first by architecture: the assistant edits drawings through exactly the same Command API as the human user, so every AI edit is undoable, live-rendered, and auditable.

## Available

- **In-app chat**: docked left panel and a floating card on the canvas; streaming responses; conversation history saved with the project
- **Six provider routes**: Claude Code CLI, Anthropic API, GitHub Copilot CLI, any OpenAI-compatible endpoint, a local Ollama, or Google Gemini — see [Providers](#providers) below. Whichever you pick, the agent gets the same tools from the same in-process MCP server, so every edit goes through the Command engine, the undo history and live patching, and the chat looks identical
- **Tool transparency**: every tool call appears as a chip in the conversation (✓ place_symbol …) with an "applied at rev N" record
- **Safe turn revert**: every turn carries a stable turn ID, and every history entry records its origin (user / agent / mcp). "Revert this turn" re-applies the inverse of *the agent's* edits only, so manual edits made during or after the turn survive. If a manual edit makes the revert impossible (the agent's target is gone), the turn is left untouched and the conflict is reported instead of half-reverting. The revert is itself an ordinary history entry, so plain undo takes it back. **Granularity with parallel conversations**: an edit's origin only records "agent or not" — the MCP entry point does not name a conversation — so reverting a turn whose edits interleaved with another conversation's turn takes *both* turns back. The swept turn is marked reverted too, rather than leaving an "applied" turn whose revert would do nothing
- **Verification loop**: after editing, the agent must run `run_verification`, fix any remaining errors and re-verify (up to three rounds), then report the counts — replies end with `検証: エラー0・警告1` style totals. If errors survive three rounds it stops and explains rather than leaving a broken drawing
- **Standards knowledge**: a bundled, editable Markdown note (`resources/knowledge/standards.md`) is injected on every turn — JIS C 0617 symbol usage, reference-designator prefixes, wire colour / sq conventions and ampacity limits, wire-number and harness rules, grid and layout rules, the standard drafting sequence, and how to read each diagnostic code. It is injected even when "read the drawing automatically" is off. **Settings → Agent → Advanced → knowledge file** appends your own Markdown after it, so site-specific rules win
- **Documentation answers with sources**: the agent is given the `docs/` index and told to read the file before answering questions about operation, standards, or what is supported, and to cite it (`出典: 05-wire-management.ja.md`). Unsupported features are answered from the roadmap ("planned for M4") instead of being invented
- **Design review and test plans**: "review this drawing" combines `run_verification` with a five-point human checklist (designator scheme, colour/sq convention, label naming, layout, title-block completeness) into one severity-ranked findings table; the agent proposes fixes and waits for approval. "Write me a test procedure" returns a step → expected-result table
- **Part selection**: `search_parts` backs comparison tables (up to 10 candidates with rating, price, purchase link and a "difference from the current part" column) and alternative suggestions; replacements are proposed, not applied
- **Start templates**: three bundled circuits (24 V control basics, motor starter, emergency stop) apply as a **single history entry** — one undo removes the whole template. Reachable from the Project ribbon, the part-insert group, and the agent's own `list_templates` / `apply_template` tools; `~/MadakeCAD/templates/*.json` adds your own
- **Tidy-up passes**: the wand button in the chat footer opens *Tidy up automatically* — **Layout / Wiring / Labels**. Picking a mode sends one ordinary turn whose prompt tells the agent to measure with `get_tidy_metrics` (wire crossings, label overlaps, symbol overlaps, off-grid points), edit, measure again, and stop when the numbers stop improving or after three rounds, then report the before/after counts. One pass = one turn, so **a single undo takes the whole tidy back**. With a selection the pass is limited to those entities ("do not move anything else"); with nothing selected it covers the sheet. Asking for a tidy in plain words runs the same loop — the rule is in the bundled standards knowledge. The pass never changes connections, adds or deletes parts, or edits label text
- **Parallel conversations**: conversations run at the same time. Switch to another conversation and send while the first one is still answering (a single conversation still takes one turn at a time, and the send guard only applies to the conversation you have open). Each conversation is given a colour from a four-colour palette in start order, used for its **edit overlay** on the canvas, the dot and spinner in the conversation list, and the "N running" badge on the tab row
- **Edit overlay**: regions being edited by the agent pulse on the canvas — in the conversation's colour — so you always see what the AI is touching
- **Drawing context**: the current sheet summary — sheets, nets, and a verification summary (error/warning/info counts plus the top few diagnostics) — is supplied to the agent at the start of each turn
- **Cancel safety**: agent events carry a turn sequence number, so late events from a cancelled turn are discarded instead of landing in the next conversation

While two or more conversations are running, canvas highlights that come from a document patch rather than from a tool call are drawn in the default colour, because a patch does not say which conversation produced it.

## Providers

**Settings → Agent → Provider** picks one of six routes. You only need *one* of them — the Claude Code CLI is not a requirement for the app, only the default.

| Route | Authentication | What you need | Verified so far |
|---|---|---|---|
| **Claude Code CLI** (default) | the CLI's own sign-in (Claude Pro/Max OAuth); MadakeCAD holds no credential | `claude` installed and signed in; optional explicit path | ✅ full drafting turns |
| **Anthropic API** | API key → OS keychain (`anthropic_api_key`) | key from the Anthropic Console; model (default `claude-sonnet-5`) | 🔶 UI, 401 handling, no-key guidance. **A real key is yours to try** |
| **GitHub Copilot CLI** | the CLI's own GitHub sign-in — **no token is stored by MadakeCAD** | `copilot` (`npm i -g @github/copilot`) signed in via `copilot` → `/login` (or `GITHUB_TOKEN` / `GH_TOKEN` / `COPILOT_GITHUB_TOKEN`); optional path and model (`auto`) | 🔶 launch flags, MCP config shape, signed-out detection. **`/login` and one turn are yours to try** |
| **OpenAI-compatible API** (OpenAI / xAI / OpenRouter …) | API key → OS keychain (`openai_compat_api_key`) | base URL (default `https://api.openai.com/v1`) **and** a model name (no default — every endpoint names its models differently) | 🔶 mock round-trips; the same code path is proven against Ollama |
| **Ollama (local)** | none — the key header is omitted entirely for local URLs | Ollama running; the **Ollama (local)** preset button sets `http://localhost:11434/v1`; a model whose `ollama show` capabilities include `tools` | ✅ full drafting turn on real hardware |
| **Google Gemini** | API key → OS keychain (`gemini_api_key`) | key from Google AI Studio; model (default `gemini-2.5-flash`) | 🔶 real endpoint rejecting an invalid key reported correctly. **A real key is yours to try** |

Ollama is the OpenAI-compatible route with a local preset, not a separate backend.

**Differences the routes cannot hide**: all of them run the same tool loop (up to 16 round trips per turn) against MadakeCAD's own MCP server, but they are told about it differently. The API routes receive the drawing context and drafting rules as a system message; **Copilot CLI has no system-prompt flag**, so the same text is prepended to the prompt under a `# システム指示` heading, MadakeCAD's MCP server is handed over as a temporary session-only config, GitHub's built-in MCP server is switched off, and the CLI is told to ignore any `AGENTS.md`. **Gemini** accepts only the OpenAPI subset of JSON Schema, so tool schemas are trimmed to the keys it allows before being sent. On the **OpenAI-compatible** route a failed tool is returned to the model prefixed with `ERROR: `, because its tool messages carry no success flag.

**Setting one up**: Settings → Agent → Provider, pick the route, fill in its box (path / URL / model), press **Save**, then **Test connection** — the badge reports the CLI version, a reachable endpoint, or the exact failure. Errors are reported by kind rather than as a raw dump: bad key, rate limit, unknown model, overloaded, server, malformed request, network.

**Where keys live**: API keys are typed masked and written only to the **OS keychain** (macOS Keychain / Windows Credential Manager / Linux Secret Service) under service `MadakeCAD` and the account name in the table. They are never written to `~/.madakecad/settings.json`, to the project file, to logs, or to agent events; the settings file has no field for them at all. Each provider box has Save / Remove and a "key saved" badge, and on macOS the key is read once per process so the keychain prompt does not repeat. The CLI routes store nothing — they borrow the sign-in the CLI already has.

**Endpoint overrides for testing or a corporate gateway**: `MADAKE_ANTHROPIC_BASE_URL` and `MADAKE_GEMINI_BASE_URL`; the OpenAI-compatible route already takes its URL from the settings box.

> **What "verified" means above.** ✅ = a real drafting turn was run through the route (edit → verify → report, then undone). 🔶 = everything observable without a paid credential was checked — provider switching, masked key save and removal, the real endpoint's rejection of a bad or absent key, the guidance shown in chat, and the absence of key material in the settings file — while the paid round-trip is left to you. For Copilot the machine was signed out, so its JSON-lines event shapes are covered by a tolerant parser plus fixtures rather than by the real stream.

## Planned (M3 phase 4 and later)

- **Tidy variants**: producing two to four alternative tidy results side by side to compare, instead of a single pass
- **Alternative-proposal UX for parallel agents**: duplicated sheets or patch previews for comparing two agents' answers to the same instruction
