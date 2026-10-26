# Office Phase 5: HR Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tuyen duoc mot Agent moi qua phong HR (yeu cau → Recruiter soan ho so → thu viec → sep Nhan → onboard) va giao viec duoc cho no; them Cho nghi / Chinh thuc.

**Architecture:** HR la mot phong binh thuong: yeu cau tuyen = task cua `hr/recruiter`, chay trong workspace `<root>/departments/hr/candidates` (khong phai git → lam thang). Recruiter ghi moi ung vien vao `candidates/<cid>/` (agent.md, candidate.json, sample-task.md). Module Rust moi `hr.rs` doc ung vien, chay thu viec bang `claude -p` trong `<root>/hr/probation/<cid>/`, va onboard/tu choi. Trang thai Agent (`active|probation|retired`) va ngan sach (`budget`) nam trong `department.json`.

**Tech Stack:** Tauri 2 (Rust), React 18 + TS, plain CSS. Khong them dependency.

**Spec:** `docs/superpowers/specs/2026-10-21-office-design.md` (muc 5, 6, 8 "Phong HR", 10 "Thu viec HR") + `claudedeck-office-plan-v2.md` muc 4 (quy trinh 6 buoc, rang buoc an toan).

## Global Constraints

- Phien Office khong bao gio dung `bypassPermissions` / `--dangerously-skip-permissions`; moi phien (ke ca thu viec) mang `DENY_ALL` cua `office.rs`.
- Recruiter chi ghi vao `candidates/` cho toi khi sep duyet; ClaudeDeck (khong phai Recruiter) chay thu viec va onboard.
- Agent moi luon bat dau `probation`, ngan sach thap hon: `budget[id] = 1_000_000` token doc cache (mac dinh phong 4M).
- Cho nghi = `retired`, khong xoa file. Tu choi/Nhan ung vien = chuyen thu muc vao `candidates/_rejected/` / `candidates/_hired/`, khong xoa.
- Thu viec: `claude -p ... --max-budget-usd 0.5`, thu muc nhap `<root>/hr/probation/<cid>/`; ket qua, so luot, token ghi vao ho so ung vien.
- Chu tren UI tieng Viet co dau cho nhan trang thai moi ("Thử việc", "Chính thức", "Cho nghỉ", "+ Tuyển"); code comment khong dau nhu code hien co.

## Review Focus

1. `cid`/agent id do Recruiter dat ("..", "a/b", "CON", trung agent dang co) → onboard phai tu choi, khong ghi ra ngoai `departments/<dept>/agents/`. (Test o Task 2.)
2. Thu viec treo hoac `claude` khong co trong PATH → sau 10 phut giet ca cay tien trinh, ho so ghi loi, UI het quay. (Test cho phan thuan o Task 3; chay that la test `#[ignore]`.)
3. Cho nghi Agent con task `backlog/running/waiting/review` → tu choi kem ly do, khong de task mo coi. (Test o Task 1.)
4. Agent `retired` van duoc Tech Lead goi lam subagent hoac van nhan task → plugin tam bo agent retired, `create_task` bao loi. (Test o Task 1.)
5. Ho so ung vien thieu/hong (`agent.md` khong co frontmatter, `candidate.json` sai phong) → the van hien kem loi, nut Nhan tat. (Test o Task 2.)

---

### Task 1: Trang thai Agent va ngan sach

**Files:**
- Modify: `src-tauri/src/office.rs` (DeptConfig, write_plugin, command moi), `src-tauri/src/tasks.rs` (create_task, task_stats), `src-tauri/src/lib.rs` (dang ky command)

**Interfaces:**
- Produces: `DeptConfig.budget: HashMap<String, u64>` (serde default); `pub fn agent_status(c: &DeptConfig, id: &str) -> &str` (mac dinh "active"); `#[tauri::command] office_set_agent_status(root: String, dept: String, agent: String, status: String) -> Result<(), String>`; `pub fn edit_department(root: &Path, dept: &str, f: impl FnOnce(&mut serde_json::Value) -> Result<(), String>) -> Result<(), String>` (sua department.json qua Value, giu field la — office_set_workspace chuyen sang dung no).

- [ ] **Step 1: Tests (office.rs + tasks.rs)**
  - `write_plugin_skips_retired`: phong co `status: {qa: "retired"}` → thu muc plugin khong co `agents/qa.md`, van co `dev.md`.
  - `create_task_rejects_retired_agent`: `create_task` cho agent retired → `Err` chua "da nghi".
  - `set_status_retire_refuses_with_open_tasks`: co task `backlog` cua agent → `office_set_agent_status(.., "retired")` → `Err` chua "con task"; sau khi task `done` → Ok, department.json co `"retired"`.
  - `set_status_active_clears_budget`: `budget: {x: 1000000}`, status probation → set "active" → `budget` khong con `x`.
  - `set_status_rejects_unknown`: status "boss" → Err.
  - `task_stats_uses_agent_budget`: `budget: {dev: 1000000}` → `max_tokens == 1_000_000 + cap_base`; agent khac → `defaults.maxTokens`.
- [ ] **Step 2: Run `cargo test --lib` → FAIL (compile errors / asserts)**
- [ ] **Step 3: Implement.** `office_set_agent_status` kiem tra task mo bang `tasks::load_tasks(root)` (status khac `done`, cung dept+agent). Status hop le: `active|probation|retired`. `active` xoa `budget[agent]`.
- [ ] **Step 4: `cargo test --lib` → PASS**
- [ ] **Step 5: Commit** `feat(office): agent status (probation/retired) and per-agent token budget`

### Task 2: Ung vien, yeu cau tuyen, Nhan / Tu choi / Sua

**Files:**
- Create: `src-tauri/src/hr.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `edit_department`, `agent_status` (Task 1); `office::parse_agent`, `load_office`; `tasks::create_task` + `with_tasks` (nhu `task_create`).
- Produces:
  ```rust
  #[derive(Serialize)] pub struct Candidate { id, dept, reason: String, agent: Option<AgentDef>, sample: String, probation: Option<Probation>, error: String }
  #[derive(Serialize, Deserialize)] pub struct Probation { ok: bool, result: String, turns: u64, tokens: u64, cost: f64, files: Vec<String>, when: i64 }
  pub fn candidates_dir(root: &Path) -> PathBuf            // <root>/departments/hr/candidates
  pub fn valid_id(s: &str) -> bool                         // ^[a-z0-9][a-z0-9-]{0,39}$
  #[tauri::command] hr_candidates(root) -> Vec<Candidate>
  #[tauri::command] hr_request(root, dept, need, notes_root: Option<String>) -> Result<Task, String>
  #[tauri::command] hr_revise(root, cid, note, notes_root: Option<String>) -> Result<Task, String>
  #[tauri::command] hr_hire(root, cid) -> Result<(), String>
  #[tauri::command] hr_reject(root, cid) -> Result<(), String>
  ```
- Ho so `candidates/<cid>/`: `agent.md` (frontmatter name, description, tools, model + prompt), `candidate.json` `{"dept": "<id phong>", "reason": "..."}`, `sample-task.md`, (sau thu viec) `probation.json`. Bo qua thu muc bat dau bang `.` hoac `_`.
- `hr_request`: neu workspace phong HR rong → dat thanh `candidates_dir` (tao thu muc). Tao task cho `hr/recruiter`, tieu de `Tuyen: <dong dau need, toi da 60 ky tu>`, than brief gom: need; phong dich (id, ten, mission, tung agent: id, name, description, tools); dinh dang ho so o tren voi `cid` = agent id hop le, chua co trong phong dich; quy tac: quyen toi thieu, khong WebFetch/WebSearch/mcp, model mac dinh sonnet, sample-task.md la mot viec nho lam duoc trong thu muc trong (khong can repo); hoi sep neu mo ho; xong thi ghi result.md nhu moi task.
- `hr_revise`: task cho recruiter `Sua ung vien <cid>` voi nhan xet, dan sua tai cho `candidates/<cid>/`.
- `hr_hire`: `valid_id(cid)`; agent.md parse duoc; phong dich ton tai; cid chua co trong `agents`; ghi `departments/<dept>/agents/<cid>.md` (copy nguyen agent.md); `edit_department`: push agents, `status[cid]="probation"`, `budget[cid]=1000000`; chuyen thu muc ung vien sang `_hired/<cid>-<yymmdd-HHMMSS>`.
- `hr_reject`: chuyen sang `_rejected/<cid>-<stamp>`.

- [ ] **Step 1: Tests (hr.rs)**
  - `valid_id_rules`: ok `changelog-writer`, `qa2`; loi `""`, `..`, `a/b`, `A`, `-x`, `con`? (con hop le regex nhung la ten Windows cam → them vao loi), id 41 ky tu.
  - `candidates_lists_and_reports_errors`: 3 thu muc: hop le; thieu frontmatter (error chua "frontmatter"); `_hired/x` (khong liet ke) → 2 the, the hong co `error` khac rong va `agent == None`.
  - `candidate_for_unknown_dept_has_error`: candidate.json dept "khong-co" → error chua "phong".
  - `hire_onboards_as_probation`: sau `hr_hire` → `load_office` thay agent moi trong phong dich voi `status == probation`, `budget == 1000000`; thu muc ung vien nam o `_hired/`; `hr_candidates` khong con no.
  - `hire_rejects_existing_or_bad_id`: cid `dev` (da co) → Err "da co"; thu muc `..`-like khong the tao nen test bang cid `Bad` → Err.
  - `reject_moves_folder`: `_rejected/` chua ho so, khong mat file.
  - `request_sets_hr_workspace_and_brief`: `hr_request` → task `dept == "hr"`, `agent == "recruiter"`, workspace HR = candidates_dir, `brief.md` chua ten phong dich, mo ta agent dang co, va "candidate.json".
- [ ] **Step 2: Run → FAIL**
- [ ] **Step 3: Implement `hr.rs`** theo Interfaces.
- [ ] **Step 4: `cargo test --lib` → PASS**
- [ ] **Step 5: Commit** `feat(hr): candidates, recruitment request, hire/reject/revise`

### Task 3: Thu viec

**Files:**
- Modify: `src-tauri/src/hr.rs`, `src-tauri/src/office.rs` (dung lai `write_plugin`, `DENY_ALL`)

**Interfaces:**
- Produces:
  ```rust
  pub fn probation_cmdline(plugin_dir: &Path, settings: &Path, agent: &AgentDef) -> String   // dong lenh sau "cmd /C"
  pub fn parse_probation(stdout: &str, files: Vec<String>, when: i64) -> Probation
  #[tauri::command] async fn hr_probation(root: String, cid: String) -> Result<Probation, String>
  ```
- Lenh (chay bang `cmd /C` + `raw_arg`, vi `claude` la `claude.cmd`): `claude -p "Lam viec mo ta trong file sample-task.md o thu muc hien tai." --output-format json --max-budget-usd 0.5 --plugin-dir "<dir>" --agent office:<id> --permission-mode auto --strict-mcp-config --settings "<file>" [--tools <tools cach nhau boi dau cach>]`. Prompt ngay sau `-p` (truoc option nhieu gia tri). Duong dan bao bang `"`.
- Ruling vs spec: dung `--plugin-dir` + `--agent office:<id>` thay vi `--agents <json>` — cung co che voi phien thuong, tranh truyen JSON qua command line Windows.
- Chay: xoa va tao lai `<root>/hr/probation/<cid>/` (thu muc nhap cua app), copy `sample-task.md`; plugin tam chua dung agent ung vien o `%TEMP%\claudedeck-probation-<cid>`; settings `{"permissions":{"deny": DENY_ALL}}` ghi o `%TEMP%`. Cho toi da 10 phut (`try_wait` moi 500ms); qua han → `taskkill /T /F /PID`. `files` = ten file trong thu muc nhap sau khi chay (tru sample-task.md). Ghi `probation.json` vao ho so ung vien.
- `parse_probation`: JSON `{is_error, result, num_turns, total_cost_usd, usage{input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens}}`; tokens = tong 4 so; `ok = !is_error`; `result` cat 2000 ky tu; stdout khong phai JSON → `ok=false`, `result` = 500 ky tu dau cua stdout (hoac "khong co ket qua").

- [ ] **Step 1: Tests**
  - `probation_cmdline_puts_prompt_first`: bat dau `claude -p "`, chua `--agent office:writer`, `--max-budget-usd 0.5`, `--strict-mcp-config`, khong chua `bypass`/`dangerously`; `--tools Read Write` dung sau cung.
  - `parse_probation_reads_usage`: JSON mau → turns 3, tokens = tong, cost 0.12, ok.
  - `parse_probation_bad_output`: "Error: not logged in" → ok false, result chua "not logged in".
  - `#[ignore] probation_real_run`: ung vien that nho (agent ghi file `hello.txt`) → `files` chua `hello.txt`.
- [ ] **Step 2: Run → FAIL**
- [ ] **Step 3: Implement**
- [ ] **Step 4: `cargo test --lib` → PASS; chay tay `cargo test --lib probation_real_run -- --ignored` → PASS**
- [ ] **Step 5: Commit** `feat(hr): probation run with claude -p in a scratch folder`

### Task 4: Giao dien HR

**Files:**
- Create: `src/Candidates.tsx`
- Modify: `src/Room.tsx` (ban "+ Tuyển", nhan thu viec, an agent retired), `src/Office.tsx` (form tuyen, gan Candidates vao phong HR), `src/AgentPanel.tsx` (trang thai + nut), `src/App.css` hoac file CSS dang dung cho Office

**Interfaces:**
- Consumes: moi command o Task 1–3. `Dept` TS them `status: Record<string,string>; budget: Record<string,number>`.
- UI:
  - Moi phong: ban trong vien dut "+ Tuyển" xep bang `seatsFor` (them id gia `__hire` vao cuoi danh sach). Bam → form trong Office: textarea "Cần Agent làm gì?" → `hr_request(root, dept, need, notesRoot)` → `onTasks()`.
  - Agent `retired` khong ve ban. Agent `probation` co nhan "Thử việc" duoi ten.
  - Phong HR: duoi canh phong, khu "Ứng viên chờ duyệt" (`Candidates`), tai lai khi mo man Office va sau moi thao tac. The: `{agent.name} → {ten phong dich}`, description, chip tools, reason, khoi thu viec ("Chưa thử việc" hoac ok/loi, result 3 dong, luot, token, $), loi ho so mau do. Nut: **Thử việc** (dang chay: "Đang thử việc…", khoa nut), **Nhận** (tat khi co loi), **Sửa** (o nhan xet → `hr_revise`), **Từ chối**.
  - AgentPanel: nhan trang thai; nut **Chính thức** (khi probation), **Cho nghỉ** (bam hai lan: lan dau doi chu thanh "Bấm lần nữa để cho nghỉ"); loi (con task) hien trong panel.
- [ ] **Step 1: `npm run build` → PASS sau khi sua**
- [ ] **Step 2: Chay tay (`npm run tauri dev`):** tuyen mot Agent "viet changelog" cho Software Development tu "+ Tuyển" → tra loi Recruiter → ung vien hien o HR → Thử việc → Nhận → ban moi co nhan "Thử việc" → Giao viec cho no → task chay. Cho nghi mot agent khong co task → ban bien mat.
- [ ] **Step 3: Commit** `feat(hr): recruit desk, candidate cards, agent status in panel`
