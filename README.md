# ClaudeDeck

Tauri 2 + React app chay Claude Code theo tab, moi tab la mot thu muc:

- Tren: `claude` (chat), duoi: PowerShell (shell).
- Header: chon model (`/model`), chat moi, lich su session (`claude --resume`).
- Paste anh (Ctrl+V) vao chat: luu ra file tam va dan duong dan cho claude.
- Footer: usage 5h / 1w tu usage API (OAuth token trong `~/.claude/.credentials.json`),
  fallback uoc tinh token tu `~/.claude/projects/**/*.jsonl`.

## Chay

```
npm install
npm run tauri dev      # hoac open.bat
npm run tauri build    # installer trong src-tauri/target/release/bundle
```
