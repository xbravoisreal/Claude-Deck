use crate::history::project_dir;
use crate::office::{load_office, prepare_launch, Department};
use crate::pty::ps_quote;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::RandomState;
use std::collections::HashSet;
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Task {
    pub id: String,
    pub dept: String,
    pub agent: String,
    pub title: String,
    /// backlog | running | waiting | review | done
    pub status: String,
    #[serde(default)]
    pub reason: String,
    pub session: String,
    /// Thu muc ban giao (brief.md, result.md).
    pub dir: String,
    /// Noi phien chay: worktree cua task, hoac workspace neu khong phai git repo.
    pub cwd: String,
    pub created: i64,
    #[serde(default)]
    pub started: bool,
    /// Muc cache_read luc sep cho chay tiep sau khi vuot tran; tran = maxTokens + cap_base.
    #[serde(default)]
    pub cap_base: u64,
    /// Luc duyet hoac huy (millis), 0 = chua xong. Dung cho tab Lich su.
    #[serde(default)]
    pub finished: i64,
    /// message_id tin Telegram con nut bam cua task (0 = khong co).
    #[serde(default)]
    pub tg_msg: i64,
    #[serde(default, skip_deserializing)]
    pub summary: String,
}

fn err<E: ToString>(e: E) -> String {
    e.to_string()
}

/// Thieu file -> []; file hong -> loi (khong de ghi de mat du lieu).
pub fn load_tasks(root: &Path) -> Result<Vec<Task>, String> {
    match std::fs::read_to_string(root.join("tasks.json")) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| format!("tasks.json: {e}")),
        Err(_) => Ok(vec![]),
    }
}

static LOCK: Mutex<()> = Mutex::new(());

/// Doc - sua - ghi tasks.json duoi mot khoa chung (command Tauri chay song song).
pub fn with_tasks<R>(root: &Path, f: impl FnOnce(&mut Vec<Task>) -> Result<R, String>) -> Result<R, String> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ts = load_tasks(root)?;
    let r = f(&mut ts)?;
    std::fs::create_dir_all(root).map_err(err)?;
    std::fs::write(root.join("tasks.json"), serde_json::to_string_pretty(&ts).map_err(err)?).map_err(err)?;
    Ok(r)
}

/// Chay git trong `dir`, khong bat cua so console. Loi -> stderr.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut c = Command::new("git");
    c.arg("-C").arg(dir).args(args);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000);
    let o = c.output().map_err(err)?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
    }
}

/// Them `.office/` vao .git/info/exclude (khong dung toi .gitignore duoc track).
fn exclude_office(ws: &Path) {
    let Ok(p) = git(ws, &["rev-parse", "--git-path", "info/exclude"]) else { return };
    let p = ws.join(p);
    let cur = std::fs::read_to_string(&p).unwrap_or_default();
    if !cur.lines().any(|l| l.trim() == ".office/") {
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(&p, format!("{cur}{}.office/\n", if cur.is_empty() || cur.ends_with('\n') { "" } else { "\n" }));
    }
}

pub fn create_task(d: &Department, agent: &str, title: &str, body: &str, notes: Option<&str>) -> Result<Task, String> {
    let ws = Path::new(&d.config.workspace);
    if d.config.workspace.is_empty() {
        return Err("phong chua co thu muc du an".into());
    }
    if !d.agent_defs.iter().any(|a| a.id == agent) {
        return Err(format!("khong co agent {agent}"));
    }
    let base = match notes.filter(|n| !n.is_empty()) {
        Some(n) => Path::new(n).join("ClaudeDeck").join(&d.config.id),
        None => ws.join(".office").join("tasks"),
    };
    let trees = ws.join(".office").join("worktrees");
    let now = chrono::Local::now();
    let stamp = now.format("%y%m%d-%H%M%S").to_string();
    let (mut id, mut n) = (stamp.clone(), 1);
    while base.join(&id).exists() || trees.join(&id).exists() {
        n += 1;
        id = format!("{stamp}-{n}");
    }
    // Chi lam thang trong workspace khi no khong phai git repo; repo ma tao worktree loi thi bao loi (khong mat cach ly).
    let tree = trees.join(&id);
    let cwd = if git(ws, &["rev-parse", "--is-inside-work-tree"]).is_ok() {
        git(ws, &["worktree", "add", &tree.to_string_lossy(), "-b", &format!("office/{id}")]).map_err(|e| format!("tao worktree loi: {e}"))?;
        exclude_office(ws);
        tree
    } else {
        ws.to_path_buf()
    };
    let dir = base.join(&id);
    std::fs::create_dir_all(&dir).map_err(err)?;
    let brief = format!(
        "---\ntask: {id}\nagent: {agent}\ndepartment: {}\nstatus: backlog\ncreated: {}\ntokens: 0\n---\n# {title}\n\n{body}\n",
        d.config.id,
        now.to_rfc3339()
    );
    std::fs::write(dir.join("brief.md"), brief).map_err(err)?;
    Ok(Task {
        id,
        dept: d.config.id.clone(),
        agent: agent.into(),
        title: title.into(),
        status: "backlog".into(),
        reason: String::new(),
        session: new_session_id(),
        dir: dir.to_string_lossy().into_owned(),
        cwd: cwd.to_string_lossy().into_owned(),
        created: now.timestamp_millis(),
        started: false,
        cap_base: 0,
        finished: 0,
        tg_msg: 0,
        summary: String::new(),
    })
}

fn live(t: &Task) -> bool {
    t.status == "running" || t.status == "waiting"
}

/// Task backlog duoc chay ngay: theo thu tu tao, moi agent mot task song, tong so phien song <= max_live.
pub fn schedule(tasks: &[Task], max_live: usize) -> Vec<String> {
    let mut busy: Vec<(&str, &str)> = tasks.iter().filter(|t| live(t)).map(|t| (t.dept.as_str(), t.agent.as_str())).collect();
    let mut queue: Vec<&Task> = tasks.iter().filter(|t| t.status == "backlog").collect();
    queue.sort_by_key(|t| t.created);
    let mut out = vec![];
    for t in queue {
        let key = (t.dept.as_str(), t.agent.as_str());
        if busy.len() >= max_live {
            break;
        }
        if !busy.contains(&key) {
            busy.push(key);
            out.push(t.id.clone());
        }
    }
    out
}

/// Phien ranh. stop = hook Stop (het luot): co result.md -> Cho sip, khong -> dang hoi sep.
/// Hook Notification (xin quyen / ranh lau) giua luot -> chi waiting, khong bao gio Cho sip (tranh dong PTY khi Agent con lam).
pub fn mark_idle(t: &mut Task, stop: bool) {
    if live(t) {
        t.status = if stop && Path::new(&t.dir).join("result.md").exists() { "review" } else { "waiting" }.into();
        t.reason.clear();
    }
}

/// Sep huy task (bo, khong lam nua): giai phong cho trong hang doi.
pub fn cancel(t: &mut Task) -> Result<(), String> {
    if !(live(t) || t.status == "backlog") {
        return Err("da xu ly roi".into());
    }
    t.status = "done".into();
    t.reason = "da huy".into();
    t.finished = chrono::Local::now().timestamp_millis();
    Ok(())
}

/// Sep tra loi trong terminal -> lam tiep. Neu dang dung vi vuot tran: tran moi tinh tu muc da dung.
pub fn resume_busy(t: &mut Task, cache_read: u64) {
    if t.status == "waiting" {
        if t.reason == "vuot tran" {
            t.cap_base = cache_read;
        }
        t.status = "running".into();
        t.reason.clear();
    }
}

pub fn approve(t: &mut Task) -> Result<(), String> {
    if t.status != "review" {
        return Err("da xu ly roi".into());
    }
    t.status = "done".into();
    t.finished = chrono::Local::now().timestamp_millis();
    Ok(())
}

/// Ghi nhan xet vao brief.md, cat result.md cu (de lan sau khong bi coi la xong ngay), xep hang chay lai.
pub fn rework(t: &mut Task, note: &str) -> Result<(), String> {
    if t.status != "review" {
        return Err("da xu ly roi".into());
    }
    let dir = Path::new(&t.dir);
    let mut f = std::fs::OpenOptions::new().append(true).create(true).open(dir.join("brief.md")).map_err(err)?;
    write!(f, "\n## Lam lai ({})\n\n{note}\n", chrono::Local::now().format("%Y-%m-%d %H:%M")).map_err(err)?;
    std::fs::rename(dir.join("result.md"), dir.join("result.prev.md")).map_err(err)?;
    t.status = "backlog".into();
    t.reason.clear();
    Ok(())
}

/// uuid v4 tu RandomState (moi lan tao co khoa ngau nhien moi), khoi them crate.
fn new_session_id() -> String {
    let r = || RandomState::new().build_hasher().finish();
    let a = (r() & 0xffff_ffff_ffff_0fff) | 0x4000;
    let b = (r() & 0x3fff_ffff_ffff_ffff) | 0x8000_0000_0000_0000;
    format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", a >> 32, (a >> 16) & 0xffff, a & 0xffff, b >> 48, b & 0xffff_ffff_ffff)
}

/// 2 dong khong rong dau tien cua result.md (bo frontmatter).
pub(crate) fn summary(dir: &str) -> String {
    let Ok(s) = std::fs::read_to_string(Path::new(dir).join("result.md")) else { return String::new() };
    let body = s.strip_prefix("---").and_then(|r| r.split_once("\n---")).map_or(s.as_str(), |(_, b)| b);
    body.lines().map(str::trim).filter(|l| !l.is_empty()).take(2).collect::<Vec<_>>().join("\n")
}

/// Sua mot task theo id, tra ve ban sau khi sua.
pub(crate) fn update(root: &str, id: &str, f: impl FnOnce(&mut Task) -> Result<(), String>) -> Result<Task, String> {
    update_with(root, id, |t| f(t).map(|_| t.clone()))
}

fn update_with<R>(root: &str, id: &str, f: impl FnOnce(&mut Task) -> Result<R, String>) -> Result<R, String> {
    with_tasks(Path::new(root), |ts| f(ts.iter_mut().find(|t| t.id == id).ok_or(format!("khong co task {id}"))?))
}

#[tauri::command]
pub fn task_list(root: String) -> Result<Vec<Task>, String> {
    let mut ts = load_tasks(Path::new(&root))?;
    for t in ts.iter_mut().filter(|t| t.status == "review" || t.status == "done") {
        t.summary = summary(&t.dir);
    }
    Ok(ts)
}

#[tauri::command]
pub fn task_create(root: String, dept: String, agent: String, title: String, body: String, notes_root: Option<String>) -> Result<Task, String> {
    let d = load_office(Path::new(&root)).into_iter().find(|d| d.config.id == dept).ok_or(format!("khong co phong {dept}"))?;
    let t = create_task(&d, &agent, &title, &body, notes_root.as_deref())?;
    with_tasks(Path::new(&root), |ts| Ok(ts.push(t.clone())))?;
    Ok(t)
}

#[tauri::command]
pub fn task_idle(root: String, id: String, stop: bool) -> Result<Task, String> {
    update(&root, &id, |t| Ok(mark_idle(t, stop)))
}

#[tauri::command]
pub fn task_busy(root: String, id: String) -> Result<Task, String> {
    let read = task_stats(root.clone(), id.clone()).map_or(0, |s| s.cache_read);
    update(&root, &id, |t| Ok(resume_busy(t, read)))
}

#[tauri::command]
pub fn task_cancel(root: String, id: String) -> Result<Task, String> {
    update(&root, &id, cancel)
}

#[tauri::command]
pub fn task_approve(root: String, id: String) -> Result<Task, String> {
    update(&root, &id, approve)
}

#[tauri::command]
pub fn task_rework(root: String, id: String, note: String) -> Result<Task, String> {
    update(&root, &id, |t| rework(t, &note))
}

/// Dung tam (vuot tran, app dong...): task sang waiting kem ly do.
#[tauri::command]
pub fn task_stop(root: String, id: String, reason: String) -> Result<Task, String> {
    update(&root, &id, |t| {
        if live(t) {
            t.status = "waiting".into();
            t.reason = reason;
        }
        Ok(())
    })
}

/// Tham so rieng cua task. Prompt dung dau: `--add-dir`/`--tools` nhan nhieu gia tri nen se nuot prompt dung sau.
/// reopen = mo lai phien da co (khong gui prompt).
pub fn task_args(t: &Task, reopen: bool) -> String {
    let dir = Path::new(&t.dir);
    let add = format!("--add-dir {}", ps_quote(&t.dir));
    if reopen {
        return format!("--resume {} {add}", t.session);
    }
    let brief = dir.join("brief.md").to_string_lossy().into_owned();
    if t.started {
        let p = format!("Lam lai task {}: doc nhan xet moi o cuoi {brief}.", t.id);
        return format!("{} --resume {} {add}", ps_quote(&p), t.session);
    }
    let p = format!(
        "Task {}: doc {brief} roi lam. Xong thi ghi {}: 2 dong dau la tom tat, sau do chi tiet. Can hoi sep thi hoi roi dung, khong ghi result.md.",
        t.id,
        dir.join("result.md").to_string_lossy()
    );
    format!("{} --session-id {} {add}", ps_quote(&p), t.session)
}

fn start(depts: &[Department], t: &Task, reopen: bool, notes: Option<&str>) -> Result<Started, String> {
    let d = depts.iter().find(|d| d.config.id == t.dept).ok_or(format!("khong co phong {}", t.dept))?;
    let name = d.agent_defs.iter().find(|a| a.id == t.agent).map_or(t.agent.as_str(), |a| a.name.as_str());
    let l = prepare_launch(d, &t.agent, notes)?;
    Ok(Started {
        task: t.clone(),
        label: format!("{} {name} · {}", d.config.icon, t.title),
        args: format!("{} {}", task_args(t, reopen), l.args),
        settings: l.settings,
    })
}

#[derive(Serialize)]
pub struct Started {
    pub task: Task,
    pub label: String,
    pub args: String,
    pub settings: serde_json::Value,
}

#[derive(Serialize, Default, Debug, PartialEq)]
pub struct Stats {
    pub context: u64,
    pub cache_read: u64,
    pub max_tokens: u64,
    /// Ten tool Agent dang dung ("" neu dang viet/nghi).
    pub tool: String,
}

/// context = do dai ngu canh cua luot cuoi; cache_read cong don, moi message.id tinh mot lan (stream ghi nhieu dong).
/// ponytail: doc lai ca file moi lan, doc tang dan theo offset neu transcript qua lon.
pub fn session_stats(jsonl: &Path) -> Stats {
    let mut st = Stats::default();
    let Ok(s) = std::fs::read_to_string(jsonl) else { return st };
    let mut seen = HashSet::new();
    for line in s.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let u = &v["message"]["usage"];
        if v["type"] != "assistant" || !u.is_object() {
            continue;
        }
        // Tool dang chay = tool_use gan nhat, cho toi khi Agent viet text moi.
        for c in v["message"]["content"].as_array().into_iter().flatten() {
            match c["type"].as_str() {
                Some("tool_use") => st.tool = c["name"].as_str().unwrap_or_default().into(),
                Some("text") => st.tool.clear(),
                _ => {}
            }
        }
        let g = |k: &str| u[k].as_u64().unwrap_or(0);
        st.context = g("input_tokens") + g("cache_read_input_tokens") + g("cache_creation_input_tokens");
        let id = v["message"]["id"].as_str().unwrap_or("");
        if id.is_empty() || seen.insert(id.to_string()) {
            st.cache_read += g("cache_read_input_tokens");
        }
    }
    st
}

/// Chay cac task backlog den luot; task loi khoi chay -> waiting kem ly do.
#[tauri::command]
pub fn task_start_next(root: String, max_live: usize, notes_root: Option<String>) -> Result<Vec<Started>, String> {
    let depts = load_office(Path::new(&root));
    with_tasks(Path::new(&root), |ts| {
        let mut out = vec![];
        for id in schedule(ts, max_live) {
            let t = ts.iter_mut().find(|t| t.id == id).unwrap();
            match start(&depts, t, false, notes_root.as_deref()) {
                Ok(mut s) => {
                    t.status = "running".into();
                    t.started = true;
                    s.task = t.clone();
                    out.push(s);
                }
                Err(e) => {
                    t.status = "waiting".into();
                    t.reason = e;
                }
            }
        }
        Ok(out)
    })
}

/// Mo lai phien cua task da dong (Cho sip, app vua mo lai...): khong doi trang thai.
/// Task chua tung chay duoc (khoi chay loi) -> chay moi tu dau.
#[tauri::command]
pub fn task_resume(root: String, id: String, notes_root: Option<String>) -> Result<Started, String> {
    let depts = load_office(Path::new(&root));
    update_with(&root, &id, |t| {
        let mut s = start(&depts, t, t.started, notes_root.as_deref())?;
        if !t.started {
            t.started = true;
            t.status = "running".into();
            t.reason.clear();
            s.task = t.clone();
        }
        Ok(s)
    })
}

#[tauri::command]
pub fn task_stats(root: String, id: String) -> Result<Stats, String> {
    let t = load_tasks(Path::new(&root))?.into_iter().find(|t| t.id == id).ok_or(format!("khong co task {id}"))?;
    let mut st = match project_dir(&t.cwd) {
        Some(p) => session_stats(&p.join(format!("{}.jsonl", t.session))),
        None => Stats::default(),
    };
    let d = load_office(Path::new(&root)).into_iter().find(|d| d.config.id == t.dept);
    st.max_tokens = d.and_then(|d| d.config.defaults["maxTokens"].as_u64()).unwrap_or(4_000_000) + t.cap_base;
    Ok(st)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::office::{office_set_workspace, seed_office};
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cd-tasks-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn git(dir: &Path, args: &str) -> String {
        let o = Command::new("git").arg("-C").arg(dir).args(args.split(' ')).output().unwrap();
        String::from_utf8_lossy(&o.stdout).into_owned()
    }

    /// Office root + phong software-dev voi workspace `ws` (git repo neu `repo`).
    fn setup(name: &str, repo: bool) -> (PathBuf, PathBuf, Department) {
        let root = tmp(name);
        seed_office(&root.join("office")).unwrap();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        if repo {
            git(&ws, "init -q");
            git(&ws, "-c user.name=t -c user.email=t@t commit -q --allow-empty -m init");
        }
        let mut d = load_office(&root.join("office")).into_iter().find(|d| d.config.id == "software-dev").unwrap();
        d.config.workspace = ws.to_string_lossy().into_owned();
        (root, ws, d)
    }

    fn task(id: &str, agent: &str, status: &str, created: i64) -> Task {
        Task {
            id: id.into(),
            dept: "d".into(),
            agent: agent.into(),
            title: id.into(),
            status: status.into(),
            reason: String::new(),
            session: String::new(),
            dir: String::new(),
            cwd: String::new(),
            created,
            started: false,
            cap_base: 0,
            finished: 0,
            tg_msg: 0,
            summary: String::new(),
        }
    }

    #[test]
    fn create_task_writes_brief_and_worktree() {
        let (root, ws, d) = setup("create", true);
        let t = create_task(&d, "dev", "Sua nut", "Mo ta", None).unwrap();
        assert_eq!(t.status, "backlog");
        assert!(Path::new(&t.dir).ends_with(format!(".office/tasks/{}", t.id)), "{}", t.dir);
        let brief = fs::read_to_string(Path::new(&t.dir).join("brief.md")).unwrap();
        for k in ["task: ", "agent: dev", "department: software-dev", "status: backlog", "created: ", "tokens: 0", "# Sua nut", "Mo ta"] {
            assert!(brief.contains(k), "{k}");
        }
        assert!(Path::new(&t.cwd).ends_with(format!(".office/worktrees/{}", t.id)), "{}", t.cwd);
        assert!(Path::new(&t.cwd).join(".git").exists());
        assert!(git(&ws, "branch --list office/*").contains(&t.id));
        assert!(fs::read_to_string(ws.join(".git/info/exclude")).unwrap().contains(".office/"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn create_task_ids_unique() {
        let (root, _ws, d) = setup("unique", true);
        let a = create_task(&d, "dev", "a", "", None).unwrap();
        let b = create_task(&d, "qa", "b", "", None).unwrap();
        assert_ne!(a.id, b.id);
        assert_ne!(a.session, b.session);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn create_task_without_git_uses_workspace() {
        let (root, ws, d) = setup("nogit", false);
        let t = create_task(&d, "dev", "x", "", None).unwrap();
        assert_eq!(Path::new(&t.cwd), ws);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn create_task_uses_notes_root() {
        let (root, _ws, d) = setup("notes", false);
        let notes = root.join("vault");
        let t = create_task(&d, "dev", "x", "", Some(&notes.to_string_lossy())).unwrap();
        assert_eq!(Path::new(&t.dir), notes.join("ClaudeDeck").join("software-dev").join(&t.id));
        assert!(Path::new(&t.dir).join("brief.md").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn create_task_requires_workspace_and_agent() {
        let (root, _ws, mut d) = setup("nows", false);
        assert!(create_task(&d, "ghost", "x", "", None).is_err());
        d.config.workspace = String::new();
        assert!(create_task(&d, "dev", "x", "", None).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn schedule_respects_limits() {
        let ts = vec![task("a", "dev", "running", 1), task("b", "dev", "backlog", 2), task("c", "qa", "backlog", 3), task("d", "reviewer", "backlog", 4)];
        assert_eq!(schedule(&ts, 2), vec!["c"]);
        assert_eq!(schedule(&ts, 5), vec!["c", "d"]);
        let ts = vec![task("y", "qa", "backlog", 9), task("x", "dev", "backlog", 1), task("z", "dev", "backlog", 5)];
        assert_eq!(schedule(&ts, 3), vec!["x", "y"]); // theo created, moi agent mot task
    }

    #[test]
    fn mark_idle_needs_result() {
        let root = tmp("idle");
        let mut t = task("a", "dev", "running", 1);
        t.dir = root.to_string_lossy().into_owned();
        mark_idle(&mut t, true);
        assert_eq!(t.status, "waiting");
        fs::write(root.join("result.md"), "xong").unwrap();
        mark_idle(&mut t, true);
        assert_eq!(t.status, "review");
        t.status = "done".into();
        mark_idle(&mut t, true); // tin hieu tre khong keo task da duyet ve
        assert_eq!(t.status, "done");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn approve_twice_errors() {
        let mut t = task("a", "dev", "review", 1);
        approve(&mut t).unwrap();
        assert_eq!(t.status, "done");
        assert!(approve(&mut t).unwrap_err().contains("da xu ly roi"));
        assert_eq!(t.status, "done");
        let mut w = task("b", "dev", "waiting", 1);
        assert!(approve(&mut w).is_err());
        assert_eq!(w.status, "waiting");
    }

    #[test]
    fn rework_moves_old_result() {
        let root = tmp("rework");
        fs::write(root.join("brief.md"), "# x\n").unwrap();
        fs::write(root.join("result.md"), "xong").unwrap();
        let mut t = task("a", "dev", "review", 1);
        t.dir = root.to_string_lossy().into_owned();
        rework(&mut t, "them test").unwrap();
        assert_eq!(t.status, "backlog");
        let brief = fs::read_to_string(root.join("brief.md")).unwrap();
        assert!(brief.contains("## Lam lai") && brief.contains("them test"));
        assert!(!root.join("result.md").exists());
        assert!(root.join("result.prev.md").exists());
        t.status = "running".into();
        mark_idle(&mut t, true);
        assert_eq!(t.status, "waiting");
        assert!(rework(&mut t, "x").is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn with_tasks_persists_and_refuses_broken_file() {
        let root = tmp("persist");
        with_tasks(&root, |ts| Ok(ts.push(task("a", "dev", "backlog", 1)))).unwrap();
        assert_eq!(load_tasks(&root).unwrap()[0].id, "a");
        fs::write(root.join("tasks.json"), "{ hong").unwrap();
        assert!(with_tasks(&root, |_| Ok(())).is_err());
        assert_eq!(fs::read_to_string(root.join("tasks.json")).unwrap(), "{ hong"); // khong ghi de
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn create_task_errors_when_worktree_fails() {
        let (root, ws, d) = setup("wtfail", true);
        fs::create_dir_all(ws.join(".office")).unwrap();
        fs::write(ws.join(".office/worktrees"), "").unwrap(); // worktree add chac chan hong
        let e = create_task(&d, "dev", "x", "", None).unwrap_err(); // khong duoc lam thang tren checkout cua sep
        assert!(e.contains("worktree"), "{e}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn notify_never_moves_to_review() {
        let root = tmp("notify");
        fs::write(root.join("result.md"), "xong").unwrap();
        let mut t = task("a", "dev", "running", 1);
        t.dir = root.to_string_lossy().into_owned();
        mark_idle(&mut t, false); // xin quyen giua chung, result.md co tu truoc
        assert_eq!(t.status, "waiting");
        mark_idle(&mut t, true);
        assert_eq!(t.status, "review");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn approve_and_cancel_record_finish_time() {
        let mut t = task("a", "dev", "review", 1);
        assert_eq!(t.finished, 0);
        approve(&mut t).unwrap();
        assert!(t.finished > 1_700_000_000_000, "{}", t.finished);
        let mut c = task("b", "dev", "waiting", 1);
        cancel(&mut c).unwrap();
        assert!(c.finished > 0);
    }

    #[test]
    fn cancel_frees_slot() {
        let mut t = task("a", "dev", "waiting", 1);
        cancel(&mut t).unwrap();
        assert_eq!((t.status.as_str(), t.reason.as_str()), ("done", "da huy"));
        assert!(schedule(&[t.clone(), task("b", "dev", "backlog", 2)], 1) == vec!["b"]);
        assert!(cancel(&mut t).is_err());
    }

    #[test]
    fn resume_unstarted_task_starts_it() {
        let (root, _ws, d) = setup("resume0", true);
        let r = root.join("office").to_string_lossy().to_string();
        office_set_workspace(r.clone(), "software-dev".into(), d.config.workspace.clone()).unwrap();
        let t = task_create(r.clone(), "software-dev".into(), "dev".into(), "x".into(), "".into(), None).unwrap();
        task_stop_any(&r, &t.id); // gia lap khoi chay loi: waiting, started=false
        let s = task_resume(r.clone(), t.id.clone(), None).unwrap();
        assert!(s.args.contains("--session-id") && !s.args.contains("--resume"), "{}", s.args);
        let t = load_tasks(Path::new(&r)).unwrap().into_iter().next().unwrap();
        assert!(t.started && t.status == "running");
        let _ = fs::remove_dir_all(&root);
    }

    fn task_stop_any(root: &str, id: &str) {
        update(root, id, |t| Ok((t.status = "waiting".into(), t.reason = "loi".into()).0)).unwrap();
    }

    #[test]
    fn busy_after_cap_raises_budget() {
        let mut t = task("a", "dev", "waiting", 1);
        t.reason = "vuot tran".into();
        resume_busy(&mut t, 5_000_000);
        assert_eq!((t.status.as_str(), t.cap_base), ("running", 5_000_000));
        let mut w = task("b", "dev", "waiting", 1); // cho vi ly do khac: khong doi ngan sach
        resume_busy(&mut w, 9);
        assert_eq!((w.status.as_str(), w.cap_base), ("running", 0));
    }

    #[test]
    fn task_args_first_and_resume() {
        let mut t = task("a1", "dev", "backlog", 1);
        t.session = "sid".into();
        t.dir = "C:\\Bob's\\a1".into();
        let a = task_args(&t, false);
        assert!(a.starts_with("'Task a1: doc C:\\Bob''s\\a1\\brief.md"), "{a}"); // prompt dung truoc option variadic
        assert!(a.contains("--session-id sid") && a.contains("--add-dir 'C:\\Bob''s\\a1'") && !a.contains("--resume"), "{a}");
        t.started = true;
        let a = task_args(&t, false);
        assert!(a.starts_with("'Lam lai task a1") && a.contains("--resume sid"), "{a}");
        let a = task_args(&t, true);
        assert!(a.starts_with("--resume sid --add-dir"), "{a}"); // mo lai: khong kem prompt
    }

    #[test]
    fn start_next_marks_running() {
        let (root, _ws, d) = setup("start", true);
        let office = root.join("office");
        let r = office.to_string_lossy().to_string();
        office_set_workspace(r.clone(), "software-dev".into(), d.config.workspace.clone()).unwrap();
        let t = task_create(r.clone(), "software-dev".into(), "dev".into(), "x".into(), "y".into(), None).unwrap();
        let s = task_start_next(r.clone(), 3, None).unwrap();
        assert_eq!(s.len(), 1);
        assert!(s[0].args.contains("--agent 'office:dev'") && s[0].args.contains(&format!("--session-id {}", t.session)), "{}", s[0].args);
        assert!(!s[0].args.contains("dangerously"));
        assert_eq!(s[0].task.status, "running");
        assert!(s[0].task.started);
        assert!(task_start_next(r, 3, None).unwrap().is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn stats_reports_last_tool() {
        let root = tmp("tool");
        let p = root.join("s.jsonl");
        let a = |c: serde_json::Value| serde_json::json!({"type":"assistant","message":{"id":"m","content":[c],"usage":{}}}).to_string();
        let edit = a(serde_json::json!({"type":"tool_use","name":"Edit"}));
        let text = a(serde_json::json!({"type":"text","text":"xong"}));
        let result = serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result"}]}}).to_string();
        fs::write(&p, [text.clone(), edit, result].join("\n")).unwrap();
        assert_eq!(session_stats(&p).tool, "Edit"); // tool_result chua co text moi -> van dang o tool
        fs::write(&p, [fs::read_to_string(&p).unwrap(), text].join("\n")).unwrap();
        assert_eq!(session_stats(&p).tool, "");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn stats_dedupes_message_ids() {
        let root = tmp("stats");
        let p = root.join("s.jsonl");
        let line = |id: &str, inp: u64, read: u64, cre: u64| {
            serde_json::json!({"type":"assistant","message":{"id":id,"usage":{"input_tokens":inp,"cache_read_input_tokens":read,"cache_creation_input_tokens":cre}}}).to_string()
        };
        let user = serde_json::json!({"type":"user","message":{"content":"hi"}}).to_string();
        fs::write(&p, [line("m1", 1, 100, 0), line("m1", 1, 100, 0), user, line("m2", 5, 300, 10), "hong".into()].join("\n")).unwrap();
        assert_eq!(session_stats(&p), Stats { context: 315, cache_read: 400, max_tokens: 0, tool: String::new() });
        assert_eq!(session_stats(&root.join("none.jsonl")), Stats::default());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn summary_skips_frontmatter() {
        let root = tmp("summary");
        fs::write(root.join("result.md"), "---\ntask: a\n---\n\nDong 1\n\nDong 2\nDong 3").unwrap();
        assert_eq!(summary(&root.to_string_lossy()), "Dong 1\nDong 2");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn session_ids_are_uuid_and_unique() {
        let (a, b) = (new_session_id(), new_session_id());
        assert_eq!(a.len(), 36);
        assert_eq!(a.matches('-').count(), 4);
        assert_eq!(&a[14..15], "4");
        assert_ne!(a, b);
    }
}
