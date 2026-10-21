# Office Giai đoạn 2: Task và duyệt — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Giao task cho Agent → Agent làm trong worktree riêng → "Chờ síp" → sếp duyệt hoặc yêu cầu làm lại từ danh sách ☰; kèm hàng đợi, cảnh báo ngữ cảnh và trần token.

**Architecture:** Rust `tasks.rs` giữ `tasks.json` trong office root (khóa toàn cục khi đọc-sửa-ghi), tạo thư mục bàn giao + worktree, xếp lịch, chuyển trạng thái và đọc transcript. Frontend mở mỗi task thành một tab Agent (dùng lại `office::launch_spec`), nghe tín hiệu hook Stop/Notification đã có để gọi `task_idle`, và hiện drawer ☰.

**Tech Stack:** Tauri 2, Rust (serde_json, chrono, dirs — không thêm crate), React 18 + TS, CSS thuần.

**Spec:** `docs/superpowers/specs/2026-10-21-office-design.md` (mục 5 cảnh báo ngữ cảnh, 6 bàn giao/cách ly, 7 luồng task, 8 danh sách chờ duyệt).

## Global Constraints

- Phiên Office không bao giờ có `--dangerously-skip-permissions`; luôn qua `office::launch_spec`.
- Trạng thái: `backlog → running → waiting → review → done`, làm lại quay về `backlog` (rồi tự chạy lại). Chữ bóng thoại: **"Chờ síp"** (review), **"Chờ anh"** (waiting).
- Tối đa 3 phiên sống (`running` + `waiting`), chỉnh trong Settings. Mỗi Agent tối đa một task sống.
- Thư mục bàn giao: `<workspace>/.office/tasks/<id>/`, hoặc `<notesRoot>/ClaudeDeck/<dept>/<id>/` khi có notesRoot. File: `brief.md`, `result.md` (+ `spec.md`, `review.md` do Agent tự ghi).
- Frontmatter file bàn giao: `task`, `agent`, `department`, `status`, `created`, `tokens`.
- Worktree: `<workspace>/.office/worktrees/<id>`, nhánh `office/<id>`. Không auto-merge, không push.
- Trần token: `department.json` → `defaults.maxTokens`, mặc định `4000000` token đọc cache; vượt → gửi Ctrl+C, task `waiting`, lý do "vượt trần".
- Cảnh báo ngữ cảnh: mặc định 150K (chỉnh trong Settings) → viền cam + tooltip "phiên dài, nên tách task".
- Code comment / chuỗi UI: tiếng Việt không dấu, trừ "Chờ síp" / "Chờ anh" (theo spec).

## Review Focus

1. Workspace không phải git repo → task vẫn chạy được, cwd = workspace (không lỗi). Test: `create_task_without_git_uses_workspace`.
2. Bấm Duyệt hai lần / duyệt task không ở `review` → báo "da xu ly roi", không đổi trạng thái. Test: `approve_twice_errors`.
3. Làm lại khi `result.md` cũ còn → phải không bị coi là xong ngay lượt sau. Test: `rework_moves_old_result`.
4. Hai lần gọi `task_start_next` liền nhau không mở trùng một task. Test: `start_next_marks_running` (task đã `running` không được xếp lại).
5. Transcript ghi một message nhiều dòng (stream) → không cộng trùng token. Test: `stats_dedupes_message_ids`.

---

### Task 1: Mô hình task, tạo task, xếp lịch, chuyển trạng thái

**Files:**
- Create: `src-tauri/src/tasks.rs` (+ tests trong cùng file như `office.rs`)
- Modify: `src-tauri/src/lib.rs` (`mod tasks;` + đăng ký command)
- Modify: `src-tauri/src/history.rs` (`project_dir` → `pub`)

**Interfaces:**
- Consumes: `office::{load_office, Department}`.
- Produces:
  ```rust
  #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
  pub struct Task { pub id: String, pub dept: String, pub agent: String, pub title: String,
      pub status: String, #[serde(default)] pub reason: String, pub session: String,
      pub dir: String, pub cwd: String, pub created: i64, #[serde(default)] pub started: bool,
      #[serde(default, skip_deserializing)] pub summary: String }
  pub fn load_tasks(root: &Path) -> Vec<Task>              // <root>/tasks.json, thieu/hong -> []
  pub fn save_tasks(root: &Path, t: &[Task]) -> std::io::Result<()>
  pub fn with_tasks<R>(root: &Path, f: impl FnOnce(&mut Vec<Task>) -> Result<R, String>) -> Result<R, String> // static Mutex, load -> f -> save
  pub fn create_task(d: &Department, agent: &str, title: &str, body: &str, notes: Option<&str>) -> Result<Task, String>
  pub fn schedule(tasks: &[Task], max_live: usize) -> Vec<String>   // id backlog duoc chay, theo created
  pub fn mark_idle(t: &mut Task)                // result.md co -> review, khong -> waiting
  pub fn approve(t: &mut Task) -> Result<(), String>
  pub fn rework(t: &mut Task, note: &str) -> Result<(), String>
  fn new_session_id() -> String                 // uuid v4 tu 2 lan RandomState::new().build_hasher().finish()
  ```
  Commands: `task_list(root) -> Vec<Task>` (dien `summary` = 2 dong khong rong dau tien cua result.md sau frontmatter), `task_create(root, dept, agent, title, body, notes_root: Option<String>) -> Task`, `task_idle(root, id) -> Task`, `task_busy(root, id)` (waiting -> running), `task_approve(root, id)`, `task_rework(root, id, note)`, `task_stop(root, id, reason)` (-> waiting + reason).

- [ ] **Step 1: Viết test fail** (dùng `tmp()` / `software_dev()` giống `office.rs`; test git dùng `git init` trong thư mục tạm)

```rust
#[test] fn create_task_writes_brief_and_worktree() {
    // workspace = git repo co 1 commit
    let t = create_task(&d, "dev", "Sua nut", "Mo ta", None).unwrap();
    assert_eq!(t.status, "backlog");
    assert!(Path::new(&t.dir).ends_with(format!(".office/tasks/{}", t.id)));
    let brief = fs::read_to_string(Path::new(&t.dir).join("brief.md")).unwrap();
    for k in ["task: ", "agent: dev", "department: software-dev", "status: backlog", "created: ", "tokens: 0", "# Sua nut", "Mo ta"] { assert!(brief.contains(k), "{k}"); }
    assert!(Path::new(&t.cwd).ends_with(format!(".office/worktrees/{}", t.id)));
    assert!(git(&ws, "branch --list office/*").contains(&t.id));
    assert!(fs::read_to_string(ws.join(".git/info/exclude")).unwrap().contains(".office/"));
}
#[test] fn create_task_without_git_uses_workspace() { /* cwd == workspace */ }
#[test] fn create_task_uses_notes_root() { /* dir == <notes>/ClaudeDeck/software-dev/<id> */ }
#[test] fn create_task_requires_workspace() { /* workspace "" -> Err */ }
#[test] fn schedule_respects_limits() {
    // a: dev running; b: dev backlog; c: qa backlog; d: reviewer backlog; max 2
    assert_eq!(schedule(&ts, 2), vec!["c"]);           // b: dev ban, d: het cho
}
#[test] fn mark_idle_needs_result() { /* khong result.md -> waiting; co -> review */ }
#[test] fn approve_twice_errors() { /* review -> done; lan 2 Err chua "da xu ly roi", van done */ }
#[test] fn rework_moves_old_result() {
    // review + result.md -> rework("them test"): status backlog, brief.md chua "## Lam lai" va "them test",
    // result.md khong con, result.prev.md co; mark_idle sau do -> waiting
}
#[test] fn session_ids_are_uuid_and_unique() { /* 36 ky tu, 4 dau '-', hai lan khac nhau */ }
```

- [ ] **Step 2: Chạy** `cd src-tauri; cargo test tasks` — Expected: FAIL (chưa có module / hàm).
- [ ] **Step 3: Cài đặt** theo Interfaces. Id task = `chrono::Local::now().format("%y%m%d-%H%M%S")` (+ `-2`, `-3`… nếu trùng thư mục). Worktree: `git -C <ws> worktree add <ws>/.office/worktrees/<id> -b office/<id>`; lệnh lỗi → `cwd = workspace`. Ghi `.office/` vào `.git/info/exclude` (không sửa `.gitignore` được track). `created` = millis.
- [ ] **Step 4: Chạy** `cargo test` — Expected: mọi test PASS (14 cũ + mới).
- [ ] **Step 5: Commit** `feat(office): tasks model, handoff dirs, worktree, scheduling`

### Task 2: Khởi chạy task và đọc transcript

**Files:** Modify `src-tauri/src/tasks.rs`, `src-tauri/src/lib.rs`.

**Interfaces:**
- Consumes: Task 1; `office::{write_plugin, launch_spec, LaunchSpec}`; `history::project_dir`.
- Produces:
  ```rust
  pub fn task_args(t: &Task) -> String
  // chua started: --session-id <sid> '<prompt>'; da started: --resume <sid> '<prompt lam lai>'; + --add-dir '<dir>'
  // prompt: "Task <id>: doc <dir>\brief.md roi lam. Xong thi ghi <dir>\result.md: 2 dong dau la tom tat, sau do chi tiet. Can hoi sep thi hoi roi dung, khong ghi result.md."
  // prompt lam lai: "Lam lai task <id>: doc nhan xet moi o cuoi <dir>\brief.md." (moi chuoi qua ps_quote)
  #[derive(Serialize)] pub struct Started { pub task: Task, pub label: String, pub args: String, pub settings: serde_json::Value }
  #[derive(Serialize, Default, Debug, PartialEq)] pub struct Stats { pub context: u64, pub cache_read: u64, pub max_tokens: u64 }
  pub fn session_stats(jsonl: &Path) -> Stats   // context = input+cache_read+cache_creation cua assistant cuoi; cache_read cong theo message.id khong trung
  ```
  Commands: `task_start_next(root, max_live, notes_root) -> Vec<Started>` (trong `with_tasks`: `schedule` → mỗi id: `status=running`, `started=true`, args = `launch_spec(...).args + " " + task_args` tính **trước** khi đặt `started`), `task_resume(root, id, notes_root) -> Started` (mở lại task đã đóng, không đổi trạng thái, luôn `--resume`), `task_stats(root, id) -> Stats` (`max_tokens` từ `defaults.maxTokens`, mặc định 4_000_000).

- [ ] **Step 1: Test fail**
```rust
#[test] fn task_args_first_and_resume() {
    // started=false -> chua "--session-id <sid> 'Task <id>: doc " va "--add-dir '<dir>'", khong co "--resume"
    // started=true  -> chua "--resume <sid> 'Lam lai task "
    // dir co dau ' -> duoc nhan doi
}
#[test] fn start_next_marks_running() {
    // 1 task backlog -> task_start_next tra 1 Started, args chua "--agent 'office:dev'" va "--session-id"; goi lan 2 -> []
}
#[test] fn stats_dedupes_message_ids() {
    // jsonl: 2 dong assistant cung message.id (cache_read 100), 1 dong id khac (cache_read 300, input 5, cache_creation 10), 1 dong user
    assert_eq!(session_stats(&p), Stats { context: 315, cache_read: 400, max_tokens: 0 });
}
#[test] fn stats_missing_file_is_zero() {}
```
- [ ] **Step 2:** `cargo test tasks` — Expected: FAIL.
- [ ] **Step 3: Cài đặt.** Transcript: `project_dir(&t.cwd)/<session>.jsonl`.
- [ ] **Step 4:** `cargo test` — Expected: PASS hết.
- [ ] **Step 5: Commit** `feat(office): start queued tasks and read session stats`

### Task 3: Giao việc và vòng đời tab task (frontend)

**Files:** Modify `src/Office.tsx`, `src/App.tsx`, `src/Pane.tsx`, `src/Settings.tsx`, `src/styles.css`.

**Interfaces:**
- Consumes: commands Task 1–2.
- Produces: type `Task` (TS, cùng trường Rust) export từ `src/Office.tsx`; `OfficePrefs = { root, notes, maxLive: number, warnK: number }` (mặc định 3 và 150, store keys `maxLive`, `warnK`).

Hành vi:
- Office: mỗi dòng Agent có nút **Giao viec** → form nhỏ (tiêu đề + mô tả) → `task_create` → `startNext()`. Dòng Agent hiện task hiện tại: "Chờ síp" / "Chờ anh: <reason>" / "Dang: <title>" / "Hang doi N".
- App: `tasks` state, `startNext()` = `task_start_next` rồi mở tab Agent cho mỗi `Started` (`Ws.task = id`), gọi sau mọi thay đổi task và lúc khởi động (sau khi đọc prefs).
- Tab task: ẩn ＋ và ⟲ (pane id cố định `${ws.id}-chat-0`). `onIdle` → `task_idle` → status `review` thì đóng tab (giải phóng PTY) rồi `startNext()`.
- Pane: prop `onEnter` gọi khi `term.onData` chứa `\r`; tab task ở `waiting` → `task_busy`.
- Khởi động app: task `running`/`waiting` không có tab → `task_stop(id, "app dong")` (giữ chỗ, mở lại qua ☰ → Mở phiên).

- [ ] **Step 1:** `npm run build` — Expected: PASS (tsc + vite) sau khi sửa.
- [ ] **Step 2: Kiểm tay** (sếp chạy `npm run tauri dev`): giao task nhỏ cho Dev trên một repo git → tab mở, Agent làm, ghi `result.md` → tab đóng, dòng Dev hiện "Chờ síp"; `git worktree list` thấy `office/<id>`.
- [ ] **Step 3: Commit** `feat(office): assign tasks and run them as agent tabs`

### Task 4: Danh sách chờ duyệt ☰, cảnh báo ngữ cảnh, trần token

**Files:** Create `src/Drawer.tsx`; modify `src/App.tsx`, `src/Office.tsx`, `src/styles.css`.

Hành vi:
- Nút ☰ đầu thanh tab (trái nút Office) + huy hiệu đỏ đếm task `review` + `waiting`. Drawer trượt từ trái, đè màn Office. Mục: `<Agent> · <phòng>`, tiêu đề, thời gian, `summary` (review) hoặc `reason` (waiting), nút **Duyệt** (`task_approve`), **Làm lại** (ô nhận xét bắt buộc → `task_rework` → `startNext()`), **Mở phiên** (tab có sẵn → kích hoạt; không có → `task_resume` rồi mở tab).
- Mỗi 20 giây, với mỗi tab task: `task_stats`. `context >= warnK*1000` → dòng Agent viền cam (`--warn` token màu) + `title="phien dai, nen tach task"`. `cache_read > max_tokens` → `pty_write` `\x03` vào `${ws.id}-chat-0`, `task_stop(id, "vuot tran")`.
- Settings thêm ô "Phien song toi da" và "Canh bao ngu canh (K token)".

- [ ] **Step 1:** `npm run build` — Expected: PASS.
- [ ] **Step 2: Kiểm tay:** task ở Chờ síp hiện trong ☰ với tóm tắt; Làm lại có nhận xét → phiên resume, Agent đọc nhận xét; Duyệt → biến khỏi ☰, bấm lại không có tác dụng.
- [ ] **Step 3: Commit** `feat(office): approval drawer, context warning and token cap`

## Ngoài phạm vi (giai đoạn sau)

Telegram (gửi ở 250K, waiting sau 45s, nút Cho phép/Từ chối) → Giai đoạn 4. Bàn pixel-art, bóng thoại, đếm trạng thái trên thanh trên → Giai đoạn 3. Token của subagent (transcript riêng) chưa cộng vào trần.
