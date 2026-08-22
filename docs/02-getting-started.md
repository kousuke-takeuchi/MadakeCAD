# Getting Started

**日本語: [02-getting-started.ja.md](02-getting-started.ja.md)** | ← [Overview](01-overview.md) | Next: [Schematic Editor →](03-schematic-editor.md)

Currently developed and tested on macOS (the core is OS-independent; Windows/Linux builds are on the roadmap, M6).

## 1. Prerequisites

| Software | Required | Get it | Check |
|---|---|---|---|
| Rust (rustup) | ✅ | https://rustup.rs | `cargo --version` |
| Node.js 20+ | ✅ | https://nodejs.org | `node --version` |
| ngspice | optional | `brew install ngspice` | `ngspice --version` |
| Claude Code CLI | optional | https://claude.com/claude-code | `claude --version` |
| GitHub Copilot CLI | optional | `npm i -g @github/copilot` | `copilot --version` |
| Ollama | optional | https://ollama.com | `ollama list` |

- **ngspice** powers circuit-solved verification and DC simulation. Without it, verification falls back to a graph approximation (an Info diagnostic tells you); simulation asks you to install it. Linux: `apt install ngspice`; Windows: official installer. A custom binary can be set via `MADAKE_NGSPICE`
- **The AI chat needs one provider, not a specific one.** The Claude Code CLI is the default and reuses your Claude Pro/Max sign-in (no API key), but an Anthropic API key, the GitHub Copilot CLI, any OpenAI-compatible endpoint, a local Ollama, or a Gemini key work just as well — pick one in Settings → Agent. Nothing here is required to draw by hand. See [AI Assistant → Providers](09-ai-assistant.md#providers)
- rustup installs `cargo` into `~/.cargo/bin`; add it to PATH if needed

## 2. Build & run

```bash
git clone <this repository>
cd MadakeCAD
npm install
npm run tauri dev        # vite (port 1420) + cargo build + native window
```

Run the tests:

```bash
cd src-tauri && cargo test
cd .. && npx vitest run
```

While the app runs, a built-in server listens on `127.0.0.1:9310` (MCP at `/mcp`, REST at `/api/v1`).

## 3. Optional: the `madake` CLI

```bash
cd src-tauri && cargo install --path crates/madake-cli
madake status            # verify connection to the running app
```

## 4. Optional: AI integration

- **From Claude Code**: with the app running, just open `claude` in this repository — `.mcp.json` connects it automatically and it can edit your drawing
- **In-app chat**: works out of the box if you are signed in to `claude`. To use another route, open **Settings → Agent → Provider**, fill in the path / URL / model, save the key if the route needs one (it goes to the OS keychain), and press **Test connection**. The keyless option is **Ollama (local)**: run `ollama serve`, pull a model whose capabilities include `tools`, then press the *Ollama (local)* preset button

## 5. Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `cargo: command not found` | rustup missing or PATH: `export PATH="$HOME/.cargo/bin:$PATH"` |
| Port 9310 in use | another instance running; use `MADAKE_MCP_PORT=19310 npm run tauri dev` (CLI: `--port 19310`) |
| Port 1420 in use | stale vite process; kill it and restart |
| Verification says "approximate mode" (Info) | ngspice not found — install it (search order: `MADAKE_NGSPICE` → PATH → OS default paths) |
| Simulation error "ngspice not found" | same as above (simulation does not fall back by design) |
| Browser shows "startup error: Failed to fetch" | page opened before the backend was up; reload after the app starts |
| CLI says the app is not running | start `npm run tauri dev` first; use `--port` when using a non-default port |
| Reset the parts database | quit the app, delete `~/Library/Application Support/MadakeCAD/parts.sqlite` (samples reseed on creation) |

## 6. UI development notes

With `npm run tauri dev` running, opening http://localhost:1420 in a browser connects the frontend to the real backend over the REST API — handy for screenshots and click-through testing. Visual changes are designed first in `MadakeCAD.pen` (Pencil); see the internal [design system](internal/design-system.md).
