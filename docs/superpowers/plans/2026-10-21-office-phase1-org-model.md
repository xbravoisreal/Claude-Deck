# Office Giai đoạn 1: mô hình tổ chức, khởi chạy Agent, Settings

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Từ ClaudeDeck mở được phiên Claude Code cho từng Agent của phòng Software Development (Tech Lead, Dev, Reviewer, QA) và HR (Recruiter), đúng prompt, công cụ, subagent và quyền; có màn Settings với `officeRoot` và `notesRoot`.

**Architecture:** Module Rust mới `office.rs` đọc thư mục `office/` (department.json + Agent `.md`), tạo dữ liệu mặc định khi chưa có, và dựng "launch spec" (chuỗi tham số CLI + JSON settings bổ sung). `pty_spawn` nhận thêm `settings` để gộp với hook settings sẵn có. Frontend thêm màn Office dạng danh sách đơn giản (giao diện pixel-art là Giai đoạn 3) và modal Settings; mở phiên Agent = mở một tab Terminal với tham số của Agent.

**Tech Stack:** Tauri 2, Rust (serde_json, dirs có sẵn), React 18 + TypeScript, `@tauri-apps/plugin-store`, `@tauri-apps/plugin-dialog`. Không thêm dependency.

**Spec:** `docs/superpowers/specs/2026-10-21-office-design.md` (mục 3, 5, 6, 10 bước 1).

## Global Constraints

- Phiên Office chạy `--permission-mode auto`; KHÔNG BAO GIỜ `--dangerously-skip-permissions`/`bypassPermissions`. Checkbox "auto" ẩn trên tab Agent.
- Không ghi gì vào `.claude/` của dự án đích. Agent truyền qua `--agents <file>`, phiên chính qua `--agent <id>`.
- Mỗi phiên Office có `--strict-mcp-config` và `--tools` giới hạn. Mục tiêu ngữ cảnh mở đầu ≤ 25K token (đo ở Task 5).
- Danh sách cấm chung (settings `permissions.deny`): `Bash(git push:*)`, `Bash(rm -rf:*)`, `Bash(Remove-Item:*)`, `Bash(del /s:*)`, `Bash(curl:*)`, `Bash(Invoke-WebRequest:*)`, `WebFetch`, `WebSearch`. Reviewer thêm `Edit`, `Write`, `NotebookEdit`.
- Thư mục office mặc định: `%APPDATA%\claudedeck\office` (`dirs::config_dir()/claudedeck/office`).
- Không dùng `--max-budget-usd` (chỉ chạy với `--print`).
- PowerShell 5.1 làm hỏng dấu `"` khi truyền cho exe: không đưa JSON vào chuỗi tham số; JSON đi qua file. Đường dẫn trong tham số bọc nháy đơn `'...'`.
- Comment code bằng tiếng Việt không dấu, giống code hiện có. Chữ trên UI giai đoạn này cũng không dấu như các nút hiện tại.
- Không thêm dependency npm hay crate.

## Review Focus

1. Đường dẫn có dấu cách (`notesRoot = H:\My Notes\Vault`) → tham số vẫn đúng. Test: `launch_spec_quotes_paths_with_spaces` (Task 3).
2. File Agent `.md` hỏng (không có frontmatter, thiếu `description`) → phòng vẫn load, Agent đó bị bỏ kèm lỗi, app không crash. Test: `load_skips_broken_agent` (Task 1).
3. `department.json` liệt kê Agent không có file → các Agent khác vẫn load. Test: `load_skips_missing_agent_file` (Task 1).
4. Chạy lần hai sau khi người dùng đã sửa file mẫu → seed không ghi đè. Test: `seed_does_not_overwrite` (Task 2).
5. Prompt Agent chứa nháy đơn, nháy kép, xuống dòng → vẫn tới Claude nguyên vẹn (đi qua file JSON). Test: `agents_json_keeps_prompt_verbatim` (Task 1).

---

### Task 1: Đọc mô hình office (`office.rs`)

**Files:**
- Create: `src-tauri/src/office.rs`
- Modify: `src-tauri/src/lib.rs` (thêm `mod office;`)
- Test: trong `office.rs`, `#[cfg(test)] mod tests`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Serialize, Deserialize, Clone)]
  pub struct AgentDef { pub id: String, pub name: String, pub description: String,
                        pub tools: Vec<String>, pub model: Option<String>, pub prompt: String }
  #[derive(Serialize, Deserialize, Clone)]
  pub struct DeptConfig { pub id: String, pub name: String, pub icon: String, pub mission: String,
                          #[serde(default)] pub workspace: String, pub lead: String, pub agents: Vec<String>,
                          #[serde(default)] pub status: HashMap<String, String>,   // agent id -> active|probation|retired
                          #[serde(default)] pub defaults: serde_json::Value }      // vd {"maxTokens": 4000000}
  #[derive(Serialize, Clone)]
  pub struct Department { #[serde(flatten)] pub config: DeptConfig,
                          pub agent_defs: Vec<AgentDef>, pub errors: Vec<String> }
  pub fn parse_agent(id: &str, md: &str) -> Result<AgentDef, String>
  pub fn load_office(root: &Path) -> Vec<Department>   // moi thu muc root/departments/*/department.json, sap xep theo id
  pub fn agents_json(d: &Department) -> serde_json::Value
  // {"<id>": {"description":..,"prompt":..,"tools":[..],"model":..}} (bo "model" neu None)
  ```

Định dạng Agent `.md` (giống subagent Claude Code):
```
---
name: Tech Lead
description: Nhan task, viet spec ngan, giao viec cho dev/reviewer/qa, tong hop ket qua.
tools: Read, Grep, Glob, Write, Edit, Agent
model: sonnet
---
<system prompt>
```
`tools` tách bằng dấu phẩy; giá trị có thể bọc `"`/`'` (bỏ nháy). Thiếu `name` → dùng `id`. Thiếu `description` hoặc thiếu frontmatter → `Err`. File Agent ở `departments/<dept>/agents/<id>.md`.

- [ ] **Step 1: Viết test hỏng**

```rust
#[test] fn parse_agent_reads_frontmatter() {
    let a = parse_agent("dev", "---\nname: Dev\ndescription: \"Lam code\"\ntools: Read, Edit, Bash\nmodel: sonnet\n---\nBan la dev.\n").unwrap();
    assert_eq!(a.name, "Dev"); assert_eq!(a.description, "Lam code");
    assert_eq!(a.tools, vec!["Read", "Edit", "Bash"]); assert_eq!(a.model.as_deref(), Some("sonnet"));
    assert_eq!(a.prompt.trim(), "Ban la dev.");
}
#[test] fn parse_agent_rejects_missing_description() {
    assert!(parse_agent("x", "---\nname: X\n---\nbody").is_err());
    assert!(parse_agent("x", "no frontmatter").is_err());
}
#[test] fn load_skips_broken_agent() { /* temp dir: dept liet ke ["ok","bad"]; bad.md khong co frontmatter
    -> agent_defs ids == ["ok"], errors.len() == 1 */ }
#[test] fn load_skips_missing_agent_file() { /* dept liet ke ["ok","ghost"], chi co ok.md
    -> agent_defs ids == ["ok"], errors[0] chua "ghost" */ }
#[test] fn agents_json_keeps_prompt_verbatim() {
    // prompt = "Dung 'x' va \"y\"\ndong 2" -> agents_json(..)["ok"]["prompt"] == prompt nguyen van
}
```
Thư mục tạm cho test: `std::env::temp_dir().join(format!("cd-office-test-{}-{}", name, std::process::id()))`, xóa ở cuối test.

- [ ] **Step 2: Chạy test, xác nhận hỏng**

Run: `cd src-tauri && cargo test office`
Expected: lỗi biên dịch (chưa có `parse_agent`, `load_office`, `agents_json`).

- [ ] **Step 3: Cài đặt** các struct và ba hàm ở trên trong `src-tauri/src/office.rs`. Frontmatter tự parse theo dòng `key: value` (không thêm crate YAML). Thêm `mod office;` vào `lib.rs`.

- [ ] **Step 4: Chạy test, xác nhận qua**

Run: `cd src-tauri && cargo test office`
Expected: 5 test PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/office.rs src-tauri/src/lib.rs
git commit -m "feat(office): load departments and agent definitions"
```

---

### Task 2: Dữ liệu mặc định và lệnh Tauri

**Files:**
- Create: `src-tauri/office-seed/departments/software-dev/department.json`
- Create: `src-tauri/office-seed/departments/software-dev/agents/{tech-lead,dev,reviewer,qa}.md`
- Create: `src-tauri/office-seed/departments/hr/department.json`, `src-tauri/office-seed/departments/hr/agents/recruiter.md`
- Modify: `src-tauri/src/office.rs`, `src-tauri/src/lib.rs` (đăng ký lệnh)

**Interfaces:**
- Consumes: `load_office`, `DeptConfig` (Task 1).
- Produces:
  ```rust
  pub fn seed_office(root: &Path) -> std::io::Result<()>   // chi ghi file chua ton tai
  #[tauri::command] pub fn office_root() -> String          // dirs::config_dir()/claudedeck/office
  #[tauri::command] pub fn office_load(root: String) -> Result<Vec<Department>, String>   // seed_office roi load_office
  #[tauri::command] pub fn office_set_workspace(root: String, dept: String, workspace: String) -> Result<(), String>
  ```

Nội dung seed (nhúng bằng `include_str!`):

| id | tools | model |
|---|---|---|
| tech-lead | Read, Grep, Glob, Write, Edit, Agent | sonnet |
| dev | Read, Grep, Glob, Edit, Write, Bash | sonnet |
| reviewer | Read, Grep, Glob, Bash | sonnet |
| qa | Read, Grep, Glob, Edit, Write, Bash | sonnet |
| recruiter | Read, Grep, Glob, Write | sonnet |

`software-dev/department.json`: `{"id":"software-dev","name":"Software Development","icon":"💻","mission":"Bien yeu cau thanh code da review va test.","workspace":"","lead":"tech-lead","agents":["tech-lead","dev","reviewer","qa"],"status":{"tech-lead":"active","dev":"active","reviewer":"active","qa":"active"},"defaults":{"maxTokens":4000000}}`. `hr/department.json` tương tự: `id "hr"`, `name "HR"`, icon `🧑‍💼`, `lead "recruiter"`, `agents ["recruiter"]`.

Prompt mỗi Agent: 5–10 dòng, một việc duy nhất theo bảng mục 5 của spec. Prompt nào cũng có câu: "Neu thu muc ClaudeDeck trong notes co project-notes.md cua phong, doc no truoc khi doc code." Tech Lead thêm: "Giao viec code cho subagent dev, review cho reviewer, test cho qa; khong tu sua code nguon." Reviewer thêm: "Khong sua file. Chi bao cao." (Định dạng `result.md` và luồng task là Giai đoạn 2, chưa đưa vào.)

- [ ] **Step 1: Viết test hỏng**

```rust
#[test] fn seed_creates_two_departments() {
    // root tam rong -> seed_office -> load_office: ids == ["hr","software-dev"],
    // software-dev.agent_defs.len() == 4, errors rong
}
#[test] fn seed_does_not_overwrite() {
    // seed, ghi de dev.md bang noi dung "---\nname: Dev2\ndescription: d\n---\nx", seed lan nua
    // -> parse lai dev.md co name == "Dev2"
}
#[test] fn set_workspace_persists() {
    // seed, office_set_workspace(root,"software-dev","H:\\proj") -> load lai workspace == "H:\\proj",
    // cac truong khac (agents, status) giu nguyen
}
```

- [ ] **Step 2: Chạy test, xác nhận hỏng**

Run: `cd src-tauri && cargo test office`
Expected: lỗi biên dịch (chưa có `seed_office`, `office_set_workspace`).

- [ ] **Step 3: Cài đặt** file seed, `seed_office`, ba lệnh. `office_set_workspace` đọc `department.json` dạng `serde_json::Value`, chỉ đổi trường `workspace`, ghi lại (pretty) để giữ các trường lạ người dùng thêm. Đăng ký ba lệnh trong `generate_handler!`.

- [ ] **Step 4: Chạy test, xác nhận qua**

Run: `cd src-tauri && cargo test office`
Expected: 8 test PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/office-seed src-tauri/src/office.rs src-tauri/src/lib.rs
git commit -m "feat(office): seed Software Dev and HR departments"
```

---

### Task 3: Launch spec và `pty_spawn` nhận settings

**Files:**
- Modify: `src-tauri/src/office.rs`, `src-tauri/src/pty.rs:48-81`, `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `Department`, `agents_json`, `load_office` (Task 1).
- Produces:
  ```rust
  #[derive(Serialize)] pub struct LaunchSpec { pub args: String, pub settings: serde_json::Value }
  pub fn launch_spec(d: &Department, agent_id: &str, agents_file: &Path, notes_root: Option<&str>) -> Result<LaunchSpec, String>
  #[tauri::command] pub fn office_launch(root: String, dept: String, agent: String, notes_root: Option<String>) -> Result<LaunchSpec, String>
  // pty.rs: pty_spawn them tham so `settings: Option<serde_json::Value>`
  ```

`args` có dạng (đúng thứ tự, nháy đơn quanh đường dẫn):
`--agent <id> --agents '<agents_file>' --permission-mode auto --strict-mcp-config --tools <union tools cua moi agent trong phong, them "Agent" neu lead co> [--add-dir '<notes_root>\ClaudeDeck']`
`--tools` là hợp các tool của mọi Agent trong phòng (subagent chỉ dùng được tool có trong phiên); quyền của phiên chính do frontmatter của `--agent` quyết định.
`settings` = `{"permissions":{"deny":[...danh sach cam chung..., + Edit/Write/NotebookEdit neu agent_id == "reviewer"]}}`.
`office_launch`: load phòng, ghi `agents_json` vào `std::env::temp_dir()/claudedeck-agents-<dept>.json`, tạo `<notes_root>\ClaudeDeck` nếu có notes_root, trả `launch_spec`. Agent không tồn tại → `Err`.
`pty_spawn`: `hook_settings` nhận `extra: Option<Value>`, bắt đầu từ `extra` (hoặc `{}`) rồi gán khóa `"hooks"` như hiện tại.

- [ ] **Step 1: Viết test hỏng**

```rust
#[test] fn launch_spec_for_reviewer() {
    // phong seed, agent "reviewer", agents_file "C:\\t\\a.json", notes None
    // args bat dau bang "--agent reviewer --agents 'C:\\t\\a.json' --permission-mode auto --strict-mcp-config --tools "
    // args khong chua "dangerously" va khong chua "--add-dir"
    // settings.permissions.deny chua "Bash(git push:*)", "WebFetch", "Edit", "Write"
}
#[test] fn launch_spec_dev_can_edit() { /* deny cua "dev" khong chua "Edit" nhung chua "Bash(git push:*)" */ }
#[test] fn launch_spec_quotes_paths_with_spaces() {
    // notes Some("H:\\My Notes\\Vault") -> args ket thuc bang "--add-dir 'H:\\My Notes\\Vault\\ClaudeDeck'"
}
#[test] fn launch_spec_unknown_agent_errors() { /* agent "ghost" -> Err */ }
```

- [ ] **Step 2: Chạy test, xác nhận hỏng**

Run: `cd src-tauri && cargo test office`
Expected: lỗi biên dịch (chưa có `launch_spec`).

- [ ] **Step 3: Cài đặt** `LaunchSpec`, `launch_spec`, `office_launch`, tham số `settings` của `pty_spawn`/`hook_settings`. Đăng ký `office_launch`.

- [ ] **Step 4: Chạy test và build**

Run: `cd src-tauri && cargo test office && cargo build`
Expected: 12 test PASS, build không lỗi.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/office.rs src-tauri/src/pty.rs src-tauri/src/lib.rs
git commit -m "feat(office): build agent launch args and permission settings"
```

---

### Task 4: Modal Settings (`officeRoot`, `notesRoot`)

**Files:**
- Create: `src/Settings.tsx`
- Modify: `src/App.tsx` (nút ⚙ ở thanh tab, state `office: { root: string; notes: string }` load/lưu trong `state.json` với khóa `officeRoot`, `notesRoot`), `src/styles.css`

**Interfaces:**
- Consumes: lệnh `office_root` (Task 2).
- Produces:
  ```ts
  export type OfficePrefs = { root: string; notes: string };   // notes "" = khong dung Obsidian
  export default function Settings(p: { value: OfficePrefs; onSave: (v: OfficePrefs) => void; onClose: () => void }): JSX.Element
  ```

Hai ô: "Thu muc office" (mặc định = `office_root()` khi `state.json` chưa có `officeRoot`) và "Vault Obsidian (notesRoot)", mỗi ô có nút "Chon..." dùng `open({ directory: true })`. Nút "Luu" và "Huy". Modal giống khối `.history` hiện có về style.

- [ ] **Step 1: Cài đặt** `Settings.tsx`, nút ⚙ và state trong `App.tsx`, CSS.
- [ ] **Step 2: Kiểm tra kiểu**

Run: `npm run build`
Expected: `tsc` và `vite build` không lỗi.

- [ ] **Step 3: Kiểm tra tay**

Run: `npm run tauri dev`. Bấm ⚙ → ô "Thu muc office" hiện `C:\Users\<user>\AppData\Roaming\claudedeck\office`. Chọn vault, Lưu, đóng app, mở lại, bấm ⚙ → giá trị vẫn còn.

- [ ] **Step 4: Commit**

```bash
git add src/Settings.tsx src/App.tsx src/styles.css
git commit -m "feat(office): settings for office root and Obsidian notes root"
```

---

### Task 5: Màn Office (danh sách) và mở phiên Agent

**Files:**
- Create: `src/Office.tsx`
- Modify: `src/App.tsx`, `src/Pane.tsx` (prop `settings`), `src/styles.css`, `docs/superpowers/specs/2026-10-21-office-design.md` (ghi số đo)

**Interfaces:**
- Consumes: `office_load`, `office_set_workspace`, `office_launch` (Task 2, 3); `OfficePrefs` (Task 4); `pty_spawn` nhận `settings` (Task 3).
- Produces:
  ```ts
  type Ws = { id: string; cwd: string; agent?: { label: string; args: string; settings: unknown } };
  export default function Office(p: { prefs: OfficePrefs; onOpen: (cwd: string, agent: NonNullable<Ws["agent"]>) => void }): JSX.Element
  // Pane: prop moi `settings?: unknown`, truyen vao invoke("pty_spawn", {..., settings})
  ```

Hành vi:
- Thanh tab có hai nút `Office` / `Terminal` ở đầu; mặc định mở app vào `Office`. Ở `Office`, `.stage` ẩn bằng CSS (`display:none`), Workspace vẫn mounted để PTY không chết.
- Office: mỗi phòng một thẻ (icon, tên, mission, workspace hoặc nút "Chon thu muc du an"), trong thẻ là danh sách Agent (tên, model, tools) với nút "Mo phien". Phòng có `errors` thì hiện chúng màu đỏ dưới tiêu đề.
- "Mo phien": nếu phòng chưa có workspace → `open({directory:true})`; hủy thì dừng; chọn thì `office_set_workspace` rồi tiếp. Sau đó `office_launch(root, dept, agent, notes || null)` → `onOpen(workspace, {label: "<icon> <ten agent>", args, settings})` → App thêm tab và chuyển sang `Terminal`.
- Workspace của tab Agent: `flags` = `agent.args` + `chat.args` (không thêm `--model`, không thêm `--dangerously-skip-permissions`); ẩn select model và checkbox auto; tiêu đề tab = `agent.label`. Tab Agent không lưu vào `folders` của `state.json`.

- [ ] **Step 1: Cài đặt** `Office.tsx`, thay đổi `App.tsx`, `Pane.tsx`, CSS.
- [ ] **Step 2: Kiểm tra kiểu**

Run: `npm run build`
Expected: không lỗi.

- [ ] **Step 3: Kiểm tra tay: Tech Lead nhận đúng cấu hình**

`npm run tauri dev` → Office → Software Development → chọn thư mục một dự án nhỏ → "Mo phien" ở Tech Lead. Trong phiên, gõ: `Liet ke ten cac subagent ban goi duoc va cac tool ban co.`
Expected: trả lời có `dev`, `reviewer`, `qa`; tool của Tech Lead không có `Bash`; không có tool MCP nào (`mcp__...`). Thư mục `.claude/` của dự án không có file mới (`git status` sạch).

- [ ] **Step 4: Kiểm tra tay: Reviewer bị cấm sửa**

Mở phiên Reviewer, gõ: `Tao file test.txt voi noi dung abc.`
Expected: bị từ chối (deny), không có file `test.txt`. Thêm: `Chay git push.` → bị từ chối.

- [ ] **Step 5: Đo ngữ cảnh mở đầu**

Với phiên Tech Lead ở Step 3, chạy:
```bash
python -c "import json,glob,os;f=max(glob.glob(os.path.expanduser('~/.claude/projects/*/*.jsonl')),key=os.path.getmtime);u=next(json.loads(l)['message']['usage'] for l in open(f,encoding='utf8') if '\"usage\"' in l and json.loads(l).get('type')=='assistant');print(u['input_tokens']+u['cache_creation_input_tokens']+u['cache_read_input_tokens'])"
```
Expected: in ra một số. Ghi số đó vào spec mục 5, đoạn "Khởi chạy tối giản": `Đo Giai đoạn 1 (2026-xx-xx): <N> token.` Nếu N > 25000, ghi thêm phần lớn nhất còn lại (hook SessionStart, danh sách skill của plugin...) để Giai đoạn 2 xử lý; không chặn việc commit.

- [ ] **Step 6: Commit**

```bash
git add src/Office.tsx src/App.tsx src/Pane.tsx src/styles.css docs/superpowers/specs/2026-10-21-office-design.md
git commit -m "feat(office): office view and launching agent sessions"
```
