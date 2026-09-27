# Security Policy

## Reporting a vulnerability

Please report security issues through GitHub's **private vulnerability reporting** on this repository (Security → Report a vulnerability) rather than a public issue. Include the version (or commit), the platform, and steps to reproduce. You will get an acknowledgement within a week.

## What is in scope

- The desktop app (Tauri shell, Vue UI) and the `madake` CLI
- The built-in servers the app starts while it runs: MCP at `127.0.0.1:9310/mcp` and the REST Link API at `127.0.0.1:9310/api/v1`
- The FreeCAD add-on (`freecad-addon/`)
- Handling of AI provider credentials

## Security design notes

- The built-in servers bind to `127.0.0.1` only and reject browser requests from non-local origins. They are meant for local tools (AI agents, the CLI, FreeCAD) on the same machine.
- API keys for AI providers are stored only in the OS keychain (never in the settings file, logs or events). CLI-based providers hold no credentials in MadakeCAD at all.
- Project files (`.mdkproj`) are plain JSON; the app does not execute anything from them.

Supported versions: the latest release and the `master` branch.
