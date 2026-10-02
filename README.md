# ClaudeDeck

**English** | [Tiếng Việt](README-vi.md)

A Tauri 2 + React desktop app that runs Claude Code in tabs — one tab per project folder.

## Features

- **Two panes per tab:** `claude` (chat) on top, PowerShell (shell) below. Drag the splitter to resize; the ratio is shared across tabs.
- **Header:** pick a model (`/model`: default / opus / sonnet / haiku), start a new chat, browse session history (`claude --resume`).
- **Auto-approve toggle:** restarts `claude` with `--dangerously-skip-permissions` (keeps the conversation via `--continue`).
- **Paste images (Ctrl+V)** into the chat: saved to a temp file and its path is passed to `claude`.
- **Ctrl+click paths** in the terminal: files open in Notepad++ (jumps to `:line` if given), folders open in File Explorer.
- **Account switcher:** save and switch between Claude accounts (stored in `~/.claude/claudedeck-accounts/`). A running `claude` keeps its old token until you open a new chat.
- **Footer usage meters:** 5h / 1w usage from the usage API (OAuth token in `~/.claude/.credentials.json`), falling back to a token estimate from `~/.claude/projects/**/*.jsonl`.
- **Layout:** single-tab view or split screen; tabs flash when a chat goes idle in the background. Open folders and preferences are restored on launch.

## Requirements

- Windows, [Node.js](https://nodejs.org/), [Rust](https://rustup.rs/) and the [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/)
- [Claude Code](https://claude.com/claude-code) (`claude`) available on `PATH`

## Run

```
npm install
npm run tauri dev      # or double-click open.bat
npm run tauri build    # installer in src-tauri/target/release/bundle
```

