# Contributing to MadakeCAD

[日本語](CONTRIBUTING.ja.md)

Thanks for your interest. MadakeCAD is an industrial electrical-drawing CAD (Tauri 2 + Vue 3 + Rust) and is developed **docs-first, design-first, test-first**. This page is the short version; the internal docs under [`docs/internal/`](docs/internal/README.md) are the long one.

## Ground rules

1. **Every edit to a drawing goes through a `Command`** executed by `Engine::execute()` in `madake-core`. Never mutate `Project` / `Sheet` / `Entity` directly (see `CLAUDE.md`, "アーキテクチャの絶対原則"). UI, MCP, REST, CLI and the FreeCAD add-on all share that one path, so undo/redo and patch broadcasting stay consistent.
2. **Tests are the specification.** Every test carries a bilingual one-line spec (Rust: two `///` lines above `#[test]`; Vitest: `it("…")` plus a `// ja:` comment; Python: a two-line docstring). `docs/13-specification.md` is generated from them with `python3 scripts/gen_spec.py` and checked in CI. Behaviour without a test does not exist.
3. **English is canonical for documentation**, with a Japanese twin (`*.ja.md`). New docs are written in both.
4. **UI strings live in the message catalogs** (`src/locales/*.json`), never as literals in components. All catalogs must stay key-identical (a test enforces it).
5. **Design before UI.** Screens are designed in Pencil (`MadakeCAD.pen`) and described in `docs/internal/design-system.md` before they are implemented. Reuse existing tokens and components; add new ones to the design system first.

## Workflow

1. Read the spec for the area you touch (`docs/internal/specs/`) and the master design (`docs/superpowers/specs/`). If the behaviour changes, update the spec first.
2. For anything beyond a small fix, add a plan under `docs/superpowers/plans/` (date-prefixed; see existing plans for the shape) and work it task by task with red → green commits.
3. Run the checks before pushing:

   ```bash
   cd src-tauri && cargo test            # Rust (all crates)
   npx vitest run && npx vue-tsc --noEmit   # frontend
   python3 -m unittest discover -s freecad-addon/tests   # FreeCAD add-on
   python3 scripts/gen_spec.py --check   # spec docs are up to date
   ```

4. Keep commits small and descriptive. Commit messages may be Japanese or English.
5. Open a pull request using the template; link the spec/plan you followed.

## What belongs where

| Area | Path |
|---|---|
| Document model, commands, symbol library, file I/O, reports, verification | `src-tauri/crates/madake-core` |
| Built-in MCP server and REST Link API | `src-tauri/crates/madake-mcp` |
| AI assistant backends | `src-tauri/crates/madake-agent` |
| `madake` CLI (thin client) | `src-tauri/crates/madake-cli` |
| Tauri shell (IPC handlers) | `src-tauri/src` |
| Vue 3 UI, Canvas2D renderer, Pinia stores | `src/` |
| FreeCAD add-on | `freecad-addon/` |

## Reporting problems

Use the issue templates. For security issues see [SECURITY.md](SECURITY.md). Please do not attach confidential drawings; a minimal `.mdkproj` that reproduces the problem is ideal.

## License

By contributing you agree that your contribution is licensed under MIT OR Apache-2.0, like the rest of the project (see [README](README.md#license)).
