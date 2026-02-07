# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-02-07

First release of Claude Memory Viewer.

### Added

#### Project Browser
- Auto-scan `~/.claude/projects/` directory to discover all Claude Code projects
- Display project path, session count, and last active time
- Sort projects by most recently active

#### Session List
- Read Claude Code's `sessions-index.json` for instant loading
- Show session summary, first prompt preview, message count, Git branch, created/modified timestamps
- One-click Resume button to open `claude --resume {sessionId}` in system terminal

#### Message Detail
- Full conversation rendering with paginated loading (infinite scroll)
- **User messages** — plain text and tool result display
- **Assistant messages** — Markdown rendering with GFM support (tables, task lists, strikethrough)
- **Code blocks** — Syntax highlighting via Prism (oneDark theme), supporting 100+ languages
- **Thinking blocks** — Collapsible display of Claude's reasoning process
- **Tool calls** — Collapsible display of tool name, input parameters, and results
- Large content truncation (2000 chars) with expand option

#### Global Search
- Cross-project, cross-session full-text search
- Parallel scanning powered by Rayon (Rust)
- Case-insensitive matching with keyword highlighting
- Click results to navigate directly to the matching message

#### Token Statistics
- Read `stats-cache.json` for usage data
- Summary cards: total messages, sessions, tool calls, tokens
- Daily activity bar chart (messages + tool calls)
- Token usage trend area chart (input / output over time)
- Model usage distribution with progress bars

#### Resume Session (Cross-platform)
- **Windows** — Opens new CMD window via `cmd /c start`
- **macOS** — Uses AppleScript to open Terminal.app
- **Linux** — Auto-detects gnome-terminal / konsole / xfce4-terminal / xterm

#### Infrastructure
- Tauri v2 desktop app (Rust backend + React frontend)
- React 19 + TypeScript + Vite 6 + Tailwind CSS
- Zustand state management
- GitHub Actions CI (cargo check + clippy + tsc)
- GitHub Actions Release workflow for multi-platform builds (Windows / macOS Intel / macOS ARM / Linux)
- MIT License

### Technical Details

- **JSONL Parser**: Stream-based parsing with `BufReader` + line-level pre-filtering, skips `progress` and `file-history-snapshot` records for performance
- **Session Index**: Leverages Claude Code's built-in `sessions-index.json` for millisecond-level session list loading
- **Search**: Rayon parallel brute-force search across all JSONL files
- **Path Handling**: Cross-platform Claude home detection (`%USERPROFILE%\.claude` on Windows, `~/.claude` on Unix)

[0.1.0]: https://github.com/zuoliangyu/claude-memory-viewer/releases/tag/v0.1.0
