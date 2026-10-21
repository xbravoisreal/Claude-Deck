use crate::office::{load_office, Department};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::RandomState;
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

/// Chay git trong `dir`, khong bat cua so console. None neu loi.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let mut c = Command::new("git");
    c.arg("-C").arg(dir).args(args);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000);
    let o = c.output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Them `.office/` vao .git/info/exclude (khong dung toi .gitignore duoc track).
fn exclude_office(ws: &Path) {
    let Some(p) = git(ws, &["rev-parse", "--git-path", "info/exclude"]) else { return };
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
    let dir = base.join(&id);
    std::fs::create_dir_all(&dir).map_err(err)?;
    let brief = format!(
        "---\ntask: {id}\nagent: {agent}\ndepartment: {}\nstatus: backlog\ncreated: {}\ntokens: 0\n---\n# {title}\n\n{body}\n",
        d.config.id,
        now.to_rfc3339()
    );
    std::fs::write(dir.join("brief.md"), brief).map_err(err)?;
    let tree = trees.join(&id);
    let branch = format!("office/{id}");
    let cwd = match git(ws, &["worktree", "add", &tree.to_string_lossy(), "-b", &branch]) {
        Some(_) => {
            exclude_office(ws);
            tree
        }
        None => ws.to_path_buf(), // khong phai git repo: lam thang trong workspace
    };
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

/// Phien ranh (hook Stop/Notification): co result.md -> Cho sip, khong -> dang hoi sep.
pub fn mark_idle(t: &mut Task) {
    if live(t) {
        t.status = if Path::new(&t.dir).join("result.md").exists() { "review" } else { "waiting" }.into();
        t.reason.clear();
    }
}

pub fn approve(t: &mut Task) -> Result<(), String> {
    if t.status != "review" {
        return Err("da xu ly roi".into());
    }
    t.status = "done".into();
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
fn summary(dir: &str) -> String {
    let Ok(s) = std::fs::read_to_string(Path::new(dir).join("result.md")) else { return String::new() };
    let body = s.strip_prefix("---").and_then(|r| r.split_once("\n---")).map_or(s.as_str(), |(_, b)| b);
    body.lines().map(str::trim).filter(|l| !l.is_empty()).take(2).collect::<Vec<_>>().join("\n")
}

/// Sua mot task theo id, tra ve ban sau khi sua.
fn update(root: &str, id: &str, f: impl FnOnce(&mut Task) -> Result<(), String>) -> Result<Task, String> {
    with_tasks(Path::new(root), |ts| {
        let t = ts.iter_mut().find(|t| t.id == id).ok_or(format!("khong co task {id}"))?;
        f(t)?;
        Ok(t.clone())
    })
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
pub fn task_idle(root: String, id: String) -> Result<Task, String> {
    update(&root, &id, |t| Ok(mark_idle(t)))
}

/// Sep tra loi trong terminal -> Agent lam tiep.
#[tauri::command]
pub fn task_busy(root: String, id: String) -> Result<Task, String> {
    update(&root, &id, |t| {
        if t.status == "waiting" {
            t.status = "running".into();
            t.reason.clear();
        }
        Ok(())
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::office::seed_office;
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
        mark_idle(&mut t);
        assert_eq!(t.status, "waiting");
        fs::write(root.join("result.md"), "xong").unwrap();
        mark_idle(&mut t);
        assert_eq!(t.status, "review");
        t.status = "done".into();
        mark_idle(&mut t); // tin hieu tre khong keo task da duyet ve
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
        mark_idle(&mut t);
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
