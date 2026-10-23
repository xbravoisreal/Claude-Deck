# Office Giai đoạn 4: Telegram + khay hệ thống — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Việc chờ duyệt được gửi qua bot Telegram; sếp duyệt / yêu cầu làm lại / cho phép lệnh ngay trên điện thoại, kể cả khi cửa sổ ClaudeDeck đã thu xuống khay.

**Architecture:** Rust `telegram.rs`: phần thuần (định dạng tin, đọc update → hành động, có test) + vòng long-polling `getUpdates` chạy nền (reqwest đã có). Hành động duyệt/làm lại ghi thẳng `tasks.json` qua `tasks.rs` rồi phát event để frontend làm mới và chạy hàng đợi. Frontend báo cho Rust khi task đổi trạng thái (gửi tin) và khi duyệt trên app (sửa tin cũ). Khay: plugin tray có sẵn trong Tauri (feature `tray-icon`).

**Tech Stack:** Rust (reqwest, serde_json, tauri tray-icon), React 18 + TS.

**Spec:** `docs/superpowers/specs/2026-10-21-office-design.md` mục 9 (Telegram), mục 5 (xin quyền qua Telegram, cảnh báo 250K), mục 7 (vượt trần → Telegram), mục 2 (khay).

## Global Constraints

- Chỉ nhận update có `chat.id` đúng chat id đã cấu hình; mọi nguồn khác bỏ qua im lặng.
- Long-polling `getUpdates`, không mở cổng mạng. Tin gửi chỉ có tiêu đề + tóm tắt rút gọn (≤ 300 ký tự), không gửi code.
- Review: `🟠 Chờ síp · <Agent> · <Phòng>\nTask: …\nTóm tắt: …` + nút `✅ Duyệt` / `↩ Làm lại`. Làm lại → bot hỏi nhận xét, tin trả lời kế tiếp là nhận xét.
- Waiting: gửi tin không nút, chỉ khi vẫn `waiting` sau 45 giây. Xin quyền: tin có nút `Cho phép` / `Từ chối` → app gõ `1` / `Esc` vào PTY của phiên.
- Duyệt trên app → sửa tin Telegram thành "đã duyệt" (bỏ nút). Bấm nút của task đã xử lý → bot trả lời "đã xử lý rồi".
- Ngữ cảnh ≥ 250K và vượt trần token → gửi tin (mỗi task mỗi loại một lần).
- Đóng cửa sổ = thu xuống khay; menu khay: "Mo ClaudeDeck", "Thoat".
- Token bot lưu trong **Windows Credential Manager** (crate `keyring`, service `claudedeck-telegram`, user `bot-token`) — chủ app chọn. Token chỉ đi một chiều frontend → Rust; không trả ngược lên UI, không ghi log, không nằm trong `state.json`. Chat id để trong store.

## Review Focus

1. Update từ chat lạ có `callback_data` hợp lệ → bị bỏ qua, không đổi task. Test: `ignores_other_chats`.
2. Bấm Duyệt hai lần / bấm trên task đã duyệt ở app → "đã xử lý rồi", trạng thái không đổi. Test: `approve_twice_reports_handled`.
3. Bấm Làm lại rồi gửi ảnh/sticker (không có text) → vẫn chờ nhận xét, không làm lại với nhận xét rỗng. Test: `rework_waits_for_text`.
4. Tóm tắt có `<`, `&` → tin HTML không vỡ. Test: `review_message_escapes_html`.
5. Mất mạng / token sai → vòng polling không làm sập app, thử lại có giãn cách, lỗi hiện trong Settings. Kiểm tay.

---

### Task 1: Telegram thuần — định dạng và đọc update

**Files:** Create `src-tauri/src/telegram.rs` (phần thuần + test).

**Produces:**
```rust
pub enum Action { Approve(String), Rework(String), ReworkNote(String, String), Allow(String), Deny(String), Ignore }
pub struct Chat { pub pending_rework: Option<String> }                       // trang thai hoi nhan xet
pub fn parse_update(u: &Value, chat_id: i64, st: &mut Chat) -> (Action, Option<String> /*callback_query_id*/)
pub fn review_message(agent: &str, dept: &str, t: &Task) -> (String /*html*/, Value /*reply_markup*/)
pub fn waiting_message(agent: &str, dept: &str, t: &Task, permission: bool) -> (String, Value)
pub fn escape(s: &str) -> String
// callback_data: "ok:<id>", "re:<id>", "al:<id>", "de:<id>"
```
- [ ] Test fail: `ignores_other_chats`, `callback_maps_to_action`, `rework_waits_for_text`, `review_message_escapes_html`, `summary_is_truncated`.
- [ ] Cài đặt, `cargo test` PASS, commit `feat(telegram): message format and update parsing`.

### Task 2: Telegram chạy thật + tín hiệu xin quyền

**Files:** Modify `telegram.rs`, `tasks.rs` (`Task.tg_msg: i64`, `#[serde(default)]`), `pty.rs` (hook Notification gửi kèm nội dung qua `curl --data-binary @-`; server đọc thân HTTP, event payload `{kind, message}`), `lib.rs`.

**Produces (commands):**
- `tg_set_token(token)` — ghi vào Credential Manager (rỗng = xoá). `tg_has_token() -> bool`.
- `tg_config(chat_id, root)` — đọc token từ Credential Manager, (khởi) chạy vòng polling; không có token hoặc chat id = tắt.
- `tg_test(chat_id) -> Result<(), String>` — gửi "ClaudeDeck đã kết nối".
- `tg_detect_chat() -> Vec<(i64, String)>` — các chat đã nhắn bot gần đây (để sếp chọn chat id).
- `tg_notify(id, kind)` — kind: `review` | `waiting` | `permission` | `context` | `cap`; gửi tin, lưu `message_id` vào `Task.tg_msg` (review/permission).
- `tg_resolved(id, text)` — sửa tin `tg_msg` thành text, bỏ nút.
- Event: `office-tasks-changed` (sau duyệt/làm lại từ Telegram), `office-permission` `{task, allow}`.
- Hàm thuần mới có test: `parse_signal(req: &str) -> (id, kind, message)` (đọc dòng đầu + thân HTTP). Test `signal_reads_body`.
- Vòng polling: lỗi → chờ 5s, 10s… tối đa 60s; lỗi gần nhất đọc được qua `tg_status() -> String`.

- [ ] Test fail → cài đặt → `cargo test` PASS → commit `feat(telegram): long polling, notifications, permission signal`.

### Task 3: Frontend + khay

**Files:** Modify `src/Settings.tsx` (ô Bot token, Chat id, nút "Tim chat id", "Gui thu", trạng thái), `src/App.tsx`, `src-tauri/Cargo.toml` (`tauri` feature `tray-icon`), `src-tauri/src/lib.rs` (tray + đóng cửa sổ = ẩn).

Hành vi:
- Khi `task_idle` → `review`: `tg_notify(id, "review")`. → `waiting`: hẹn 45 giây, vẫn `waiting` thì `tg_notify(id, "waiting")`. Notification có message chứa "permission" → task waiting lý do `xin quyen: <message>` + `tg_notify(id, "permission")`.
- Duyệt / Làm lại / Huỷ trên app → `tg_resolved(id, "Da duyet" | "Lam lai" | "Da huy")`.
- Event `office-tasks-changed` → `refresh()` + `startNext()`; `office-permission` → `pty_write` `1` hoặc `\x1b` vào pane chat của task.
- Poll 20s: context ≥ 250K lần đầu → `tg_notify(id, "context")`; vượt trần → `tg_notify(id, "cap")`.
- Khay: icon app, menu "Mo ClaudeDeck" / "Thoat"; nút X của cửa sổ ẩn xuống khay; bấm icon khay mở lại.

- [ ] `npm run build` + `cargo test` PASS.
- [ ] Kiểm tay (sếp): tạo bot (@BotFather), nhập token, nhắn bot 1 câu, "Tim chat id", "Gui thu"; giao task nhỏ → Telegram nhận "Chờ síp" có nút → Duyệt trên điện thoại → app chuyển `done`; thử Làm lại + nhận xét; đóng cửa sổ → vẫn duyệt được, mở lại từ khay.
- [ ] Commit `feat(telegram): settings, app wiring and tray`.

## Ngoài phạm vi

Gửi file/diff qua Telegram; nhiều người duyệt; duyệt khi app đã thoát hẳn.
