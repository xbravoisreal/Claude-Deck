# ClaudeDeck

[English](README.md) | **Tiếng Việt**

Ứng dụng desktop Tauri 2 + React chạy Claude Code theo tab — mỗi tab là một thư mục dự án.

## Tính năng

- **Hai khung mỗi tab:** `claude` (chat) ở trên, PowerShell (shell) ở dưới. Kéo thanh chia để đổi tỉ lệ; tỉ lệ dùng chung cho mọi tab.
- **Header:** chọn model (`/model`: default / opus / sonnet / haiku), tạo chat mới, xem lịch sử session (`claude --resume`).
- **Nút tự động duyệt (auto):** khởi động lại `claude` với `--dangerously-skip-permissions` (giữ hội thoại bằng `--continue`).
- **Dán ảnh (Ctrl+V)** vào chat: lưu ra file tạm và dán đường dẫn cho `claude`.
- **Ctrl+click đường dẫn** trong terminal: file mở bằng Notepad++ (nhảy tới dòng nếu có `:line`), thư mục mở bằng File Explorer.
- **Đổi account:** lưu và chuyển giữa nhiều tài khoản Claude (lưu tại `~/.claude/claudedeck-accounts/`). `claude` đang chạy giữ token cũ cho tới khi mở chat mới.
- **Footer usage:** mức dùng 5h / 1w lấy từ usage API (OAuth token trong `~/.claude/.credentials.json`), nếu lỗi thì ước tính token từ `~/.claude/projects/**/*.jsonl`.
- **Bố cục:** xem từng tab hoặc chia đôi màn hình; tab nhấp nháy khi chat chạy nền đã xong. Thư mục đang mở và tuỳ chọn được khôi phục khi mở lại.

## Yêu cầu

- Windows, [Node.js](https://nodejs.org/), [Rust](https://rustup.rs/) và [điều kiện cần của Tauri 2](https://tauri.app/start/prerequisites/)
- Đã cài [Claude Code](https://claude.com/claude-code) (`claude`) và có trong `PATH`

## Chạy

```
npm install
npm run tauri dev      # hoặc bấm đúp open.bat
npm run tauri build    # bộ cài nằm trong src-tauri/target/release/bundle
```
