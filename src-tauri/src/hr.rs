use crate::office::{edit_department, load_office, parse_agent, safe_tools, write_plugin, AgentDef, DeptConfig, Department, DENY_ALL};
use crate::tasks::{create_task, with_tasks, Task};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Agent moi luon bat dau thu viec voi tran token thap hon (mac dinh phong 4M).
pub const PROBATION_BUDGET: u64 = 1_000_000;

/// Ho so ung vien: `candidates/<id>/` gom agent.md, candidate.json {dept, reason}, sample-task.md, probation.json.
#[derive(Serialize)]
pub struct Candidate {
    pub id: String,
    pub dept: String,
    pub reason: String,
    pub agent: Option<AgentDef>,
    pub sample: String,
    pub probation: Option<Probation>,
    /// Ho so hong / thieu: the van hien, nut Nhan tat.
    pub error: String,
}

/// Ket qua thu viec (`claude -p`) ghi vao probation.json cua ho so.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Probation {
    pub ok: bool,
    pub result: String,
    pub turns: u64,
    pub tokens: u64,
    pub cost: f64,
    pub files: Vec<String>,
    pub when: i64,
    /// Tool bi chan trong luc thu viec (permission_denials).
    #[serde(default)]
    pub denied: Vec<String>,
}

/// Ung vien dang thu viec: khong cho chay trung, khong Nhan/Tu choi/Sua giua chung (thu muc dang dung).
static RUNNING: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

pub struct Running(String);

impl Running {
    pub fn start(cid: &str) -> Result<Self, String> {
        let mut r = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        if r.iter().any(|x| x == cid) {
            return Err(format!("{cid} dang thu viec"));
        }
        r.push(cid.into());
        Ok(Running(cid.into()))
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.lock().unwrap_or_else(|e| e.into_inner()).retain(|x| *x != self.0);
    }
}

fn idle(cid: &str) -> Result<(), String> {
    if RUNNING.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|x| x == cid) {
        return Err(format!("{cid} dang thu viec, doi xong da"));
    }
    Ok(())
}

fn err<E: ToString>(e: E) -> String {
    e.to_string()
}

pub fn candidates_dir(root: &Path) -> PathBuf {
    root.join("departments").join("hr").join("candidates")
}

/// Id agent/ung vien: chu thuong, so, '-'; khong bat dau bang '-'; toi da 40; khong trung ten thiet bi Windows.
pub fn valid_id(s: &str) -> bool {
    let reserved = ["con", "prn", "aux", "nul"].contains(&s) || (s.len() == 4 && (s.starts_with("com") || s.starts_with("lpt")) && s.ends_with(|c: char| c.is_ascii_digit()));
    !s.is_empty() && s.len() <= 40 && !s.starts_with('-') && !reserved && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn read_candidate(root: &Path, dir: &Path, id: &str) -> Candidate {
    let read = |f: &str| std::fs::read_to_string(dir.join(f));
    let meta: serde_json::Value = read("candidate.json").ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let dept = meta["dept"].as_str().unwrap_or("").to_string();
    let mut errors = vec![];
    let agent = match read("agent.md") {
        Ok(md) => parse_agent(id, &md).map_err(|e| errors.push(e)).ok(),
        Err(_) => {
            errors.push("thieu agent.md".into());
            None
        }
    };
    if let Some(a) = &agent {
        // Khong khai tools = Claude cap moi tool; tool la bi loc khoi dong lenh -> cung thanh moi tool. Ca hai deu chan.
        if a.tools.is_empty() {
            errors.push("agent.md phai khai bao tools (de trong = moi tool)".into());
        } else if safe_tools(&a.tools).len() != a.tools.len() {
            errors.push(format!("tools khong hop le: {}", a.tools.join(", ")));
        }
    }
    if !valid_id(id) {
        errors.push(format!("ten thu muc '{id}' khong hop le lam id agent"));
    }
    match load_office(root).into_iter().find(|d| d.config.id == dept) {
        None => errors.push(format!("candidate.json: khong co phong '{dept}'")),
        Some(d) if d.config.agents.iter().any(|a| a == id) => errors.push(format!("phong {dept} da co agent {id}")),
        _ => {}
    }
    Candidate {
        id: id.into(),
        dept,
        reason: meta["reason"].as_str().unwrap_or("").into(),
        agent,
        sample: read("sample-task.md").unwrap_or_default(),
        probation: read("probation.json").ok().and_then(|s| serde_json::from_str(&s).ok()),
        error: errors.join("; "),
    }
}

#[tauri::command]
pub fn hr_candidates(root: String) -> Vec<Candidate> {
    let root = Path::new(&root);
    let Ok(rd) = std::fs::read_dir(candidates_dir(root)) else { return vec![] };
    let mut out: Vec<Candidate> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        // .office = thu muc task cua Recruiter; _hired/_rejected = luu tru
        .filter(|n| !n.starts_with('.') && !n.starts_with('_'))
        .map(|id| read_candidate(root, &candidates_dir(root).join(&id), &id))
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Giao viec cho Recruiter (workspace HR = thu muc candidates, tao neu chua co).
fn recruiter_task(root: &Path, title: &str, body: &str, notes: Option<&str>) -> Result<Task, String> {
    let cand = candidates_dir(root);
    let hr = load_office(root).into_iter().find(|d| d.config.id == "hr").ok_or("khong co phong hr")?;
    let mut hr = hr;
    if hr.config.workspace.is_empty() {
        std::fs::create_dir_all(&cand).map_err(err)?;
        let ws = cand.to_string_lossy().into_owned();
        edit_department(root, "hr", |v| Ok(v["workspace"] = ws.clone().into()))?;
        hr.config.workspace = ws;
    }
    let t = create_task(&hr, &hr.config.lead.clone(), title, body, notes)?;
    with_tasks(root, |ts| Ok(ts.push(t.clone())))?;
    Ok(t)
}

fn clip(s: &str, n: usize) -> String {
    let s = s.lines().next().unwrap_or("").trim();
    if s.chars().count() <= n { s.into() } else { s.chars().take(n).collect::<String>() + "…" }
}

/// Yeu cau tuyen tu ban "+ Tuyen" cua phong `dept`.
#[tauri::command]
pub fn hr_request(root: String, dept: String, need: String, notes_root: Option<String>) -> Result<Task, String> {
    let rp = Path::new(&root);
    let d = load_office(rp).into_iter().find(|d| d.config.id == dept).ok_or(format!("khong co phong {dept}"))?;
    let team: String = d
        .agent_defs
        .iter()
        .map(|a| format!("- {} ({}): {} | tools: {}\n", a.id, a.name, a.description, a.tools.join(", ")))
        .collect();
    let cand = candidates_dir(rp);
    let body = format!(
        "## Yeu cau tuyen\n{need}\n\n## Phong can nguoi\n- id: {id}\n- ten: {name}\n- muc tieu: {mission}\n\nAgent dang co:\n{team}\n\
## Cach nop ho so\nTao thu muc `{cand}\\<id>\\` (id: chu thuong, so, dau '-', toi da 40 ky tu, chua co trong phong) gom:\n\
- `agent.md`: frontmatter `name`, `description` (khi nao dung Agent nay), `tools` (cach nhau dau phay), `model` (mac dinh sonnet), sau do la system prompt.\n\
- `candidate.json`: {{\"dept\": \"{id}\", \"reason\": \"vi sao can, lam gi\"}}\n\
- `sample-task.md`: mot viec nho de thu viec, lam duoc trong mot thu muc trong (khong can repo).\n\n\
Quy tac: quyen toi thieu, mot Agent mot viec; khong cap WebFetch, WebSearch, mcp__*; khong git push, khong xoa hang loat. \
Chi ghi trong `{cand}`. Mo ho thi hoi sep truoc. Xong thi ghi result.md nhu moi task (2 dong dau: tom tat ung vien).\n",
        id = d.config.id,
        name = d.config.name,
        mission = d.config.mission,
        cand = cand.to_string_lossy()
    );
    recruiter_task(rp, &format!("Tuyen: {}", clip(&need, 60)), &body, notes_root.as_deref())
}

/// Sep bam "Sua" tren the ung vien: Recruiter sua tai cho theo nhan xet.
#[tauri::command]
pub fn hr_revise(root: String, cid: String, note: String, notes_root: Option<String>) -> Result<Task, String> {
    let rp = Path::new(&root);
    if !valid_id(&cid) || !candidates_dir(rp).join(&cid).is_dir() {
        return Err(format!("khong co ung vien {cid}"));
    }
    idle(&cid)?;
    let body = format!(
        "Sua ho so ung vien `{}` theo nhan xet cua sep (sua tai cho, giu dinh dang agent.md / candidate.json / sample-task.md; xoa probation.json neu doi agent.md hoac sample-task.md):\n\n{note}\n",
        candidates_dir(rp).join(&cid).to_string_lossy()
    );
    recruiter_task(rp, &format!("Sua ung vien {cid}"), &body, notes_root.as_deref())
}

/// Chuyen ho so vao `_hired`/`_rejected` (khong xoa).
fn archive(root: &Path, cid: &str, to: &str) -> Result<(), String> {
    let base = candidates_dir(root);
    std::fs::create_dir_all(base.join(to)).map_err(err)?;
    let stamp = chrono::Local::now().format("%y%m%d-%H%M%S");
    std::fs::rename(base.join(cid), base.join(to).join(format!("{cid}-{stamp}"))).map_err(err)
}

/// Onboard: chep agent.md vao phong dich, agent vao danh sach o trang thai thu viec, tran token thap.
#[tauri::command]
pub fn hr_hire(root: String, cid: String) -> Result<(), String> {
    let rp = Path::new(&root);
    if !valid_id(&cid) || !candidates_dir(rp).join(&cid).is_dir() {
        return Err(format!("khong co ung vien {cid}"));
    }
    idle(&cid)?;
    let c = read_candidate(rp, &candidates_dir(rp).join(&cid), &cid);
    if !c.error.is_empty() {
        return Err(if c.error.contains("da co agent") { format!("da co agent {cid}") } else { c.error });
    }
    if load_office(rp).iter().any(|d| d.config.id == c.dept && d.errors.iter().any(|e| e.starts_with("department.json"))) {
        return Err(format!("department.json cua phong {} dang hong", c.dept));
    }
    let md = std::fs::read_to_string(candidates_dir(rp).join(&cid).join("agent.md")).map_err(err)?;
    let agents = rp.join("departments").join(&c.dept).join("agents");
    let file = agents.join(format!("{cid}.md"));
    if file.exists() {
        return Err(format!("da co file agents/{cid}.md trong phong {} (khong ghi de)", c.dept));
    }
    std::fs::create_dir_all(&agents).map_err(err)?;
    std::fs::write(&file, md).map_err(err)?;
    let edited = edit_department(rp, &c.dept, |v| {
        for k in ["status", "budget"] {
            match &v[k] {
                serde_json::Value::Null => v[k] = serde_json::json!({}),
                serde_json::Value::Object(_) => {}
                _ => return Err(format!("department.json: {k} phai la object")),
            }
        }
        match v["agents"].as_array_mut() {
            Some(a) => a.push(cid.clone().into()),
            None => v["agents"] = serde_json::json!([cid]),
        }
        v["status"][&cid] = "probation".into();
        v["budget"][&cid] = PROBATION_BUDGET.into();
        Ok(())
    });
    if let Err(e) = edited {
        let _ = std::fs::remove_file(&file); // file vua tao o tren, chua ai dung
        return Err(e);
    }
    archive(rp, &cid, "_hired")
}

#[tauri::command]
pub fn hr_reject(root: String, cid: String) -> Result<(), String> {
    let rp = Path::new(&root);
    if !valid_id(&cid) || !candidates_dir(rp).join(&cid).is_dir() {
        return Err(format!("khong co ung vien {cid}"));
    }
    idle(&cid)?;
    archive(rp, &cid, "_rejected")
}

const PROBATION_PROMPT: &str = "Lam viec mo ta trong file sample-task.md o thu muc hien tai.";
/// ponytail: tran thoi gian co dinh 10 phut; dua vao Settings neu task mau dai hon.
const PROBATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// Dong lenh chay sau `cmd /C` (claude la claude.cmd). Prompt dung ngay sau -p, truoc option nhieu gia tri;
/// duong dan va tool deu nam trong "..." (cmd khong hieu & | > ben trong nhay).
pub fn probation_cmdline(plugin_dir: &Path, settings: &Path, agent: &AgentDef) -> String {
    let mut s = format!(
        "claude -p \"{PROBATION_PROMPT}\" --output-format json --max-budget-usd 0.5 --plugin-dir \"{}\" --agent office:{} --permission-mode auto --strict-mcp-config --settings \"{}\"",
        plugin_dir.to_string_lossy(),
        agent.id,
        settings.to_string_lossy()
    );
    let tools = safe_tools(&agent.tools);
    if !tools.is_empty() {
        s += &format!(" --tools {}", tools.iter().map(|t| format!("\"{t}\"")).collect::<Vec<_>>().join(" "));
    }
    s
}

fn chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Doc JSON cua `claude -p --output-format json`; khong phai JSON (loi dang nhap...) -> ok=false kem 500 ky tu dau.
pub fn parse_probation(out: &str, files: Vec<String>, when: i64) -> Probation {
    let base = Probation { files, when, ..Default::default() };
    match serde_json::from_str::<serde_json::Value>(out.trim()) {
        Ok(v) if v.is_object() => {
            let u = &v["usage"];
            let n = |k: &str| u[k].as_u64().unwrap_or(0);
            Probation {
                ok: v["is_error"] == false,
                result: chars(v["result"].as_str().or(v["subtype"].as_str()).unwrap_or("khong co ket qua"), 2000),
                turns: v["num_turns"].as_u64().unwrap_or(0),
                tokens: n("input_tokens") + n("output_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens"),
                cost: v["total_cost_usd"].as_f64().unwrap_or(0.0),
                denied: v["permission_denials"].as_array().into_iter().flatten().filter_map(|d| d["tool_name"].as_str().map(String::from)).collect(),
                ..base
            }
        }
        _ => Probation { result: if out.trim().is_empty() { "khong co ket qua".into() } else { chars(out.trim(), 500) }, ..base },
    }
}

/// ClaudeDeck (khong phai Recruiter) chay ung vien tren task mau trong thu muc nhap <root>/hr/probation/<cid>/.
fn run_probation(root: &Path, cid: &str) -> Result<Probation, String> {
    use std::io::Read;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    if !valid_id(cid) {
        return Err(format!("khong co ung vien {cid}"));
    }
    let cdir = candidates_dir(root).join(cid);
    let c = read_candidate(root, &cdir, cid);
    let agent = match (c.agent, c.error.is_empty()) {
        (Some(a), true) => a,
        _ => return Err(c.error),
    };
    let _guard = Running::start(cid)?;
    let sample = cdir.join("sample-task.md");
    if !sample.is_file() {
        return Err("thieu sample-task.md".into());
    }
    // Thu muc nhap cua app: lam moi moi lan thu.
    let work = root.join("hr").join("probation").join(cid);
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(err)?;
    std::fs::copy(&sample, work.join("sample-task.md")).map_err(err)?;
    let tmp = std::env::temp_dir();
    let plugin = tmp.join(format!("claudedeck-probation-{cid}"));
    let dept = Department {
        config: DeptConfig { id: "probation".into(), lead: cid.into(), agents: vec![cid.into()], ..Default::default() },
        agent_defs: vec![agent.clone()],
        errors: vec![],
        room: serde_json::json!({}),
    };
    write_plugin(&dept, &plugin).map_err(err)?;
    let settings = tmp.join(format!("claudedeck-probation-{cid}.json"));
    std::fs::write(&settings, serde_json::json!({ "permissions": { "deny": DENY_ALL } }).to_string()).map_err(err)?;

    let mut child = Command::new("cmd")
        .raw_arg(format!("/C {}", probation_cmdline(&plugin, &settings, &agent)))
        .current_dir(&work)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .spawn()
        .map_err(|e| format!("khong chay duoc claude: {e}"))?;
    // Doc o luong rieng de ong khong day lam claude treo. Tra qua kenh: tien trinh con mo coi (vd agent chay
    // `start ...`) giu ong mo sau khi cmd thoat -> chi cho them 5s roi bo, khong treo thu viec.
    let pipe = |r: Option<Box<dyn Read + Send>>| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut b = Vec::new();
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut b);
            }
            let _ = tx.send(String::from_utf8_lossy(&b).into_owned());
        });
        rx
    };
    let out = pipe(child.stdout.take().map(|r| Box::new(r) as Box<dyn Read + Send>));
    let errs = pipe(child.stderr.take().map(|r| Box::new(r) as Box<dyn Read + Send>));
    let start = std::time::Instant::now();
    let mut timed_out = false;
    while child.try_wait().map_err(err)?.is_none() {
        if start.elapsed() > PROBATION_TIMEOUT {
            // Giet ca cay (cmd -> node claude).
            let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()]).creation_flags(0x0800_0000).output();
            let _ = child.wait();
            timed_out = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let wait = std::time::Duration::from_secs(5);
    let (out, errs) = (out.recv_timeout(wait).unwrap_or_default(), errs.recv_timeout(wait).unwrap_or_default());
    let files: Vec<String> = std::fs::read_dir(&work)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n != "sample-task.md")
        .collect();
    let mut p = parse_probation(if out.trim().is_empty() { &errs } else { &out }, files, chrono::Local::now().timestamp_millis());
    if timed_out {
        p.ok = false;
        p.result = format!("Qua {} phut, da dung. {}", PROBATION_TIMEOUT.as_secs() / 60, p.result);
    }
    std::fs::write(cdir.join("probation.json"), serde_json::to_string_pretty(&p).map_err(err)?).map_err(err)?;
    Ok(p)
}

#[tauri::command]
pub async fn hr_probation(root: String, cid: String) -> Result<Probation, String> {
    tauri::async_runtime::spawn_blocking(move || run_probation(Path::new(&root), &cid)).await.map_err(err)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::office::{agent_status, seed_office};
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cd-hr-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        seed_office(&p).unwrap();
        p
    }

    fn candidate(root: &Path, id: &str, dept: &str, md: &str) {
        let d = candidates_dir(root).join(id);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("agent.md"), md).unwrap();
        fs::write(d.join("candidate.json"), serde_json::json!({ "dept": dept, "reason": "viet changelog" }).to_string()).unwrap();
        fs::write(d.join("sample-task.md"), "Viet CHANGELOG.md").unwrap();
    }

    const MD: &str = "---\nname: Writer\ndescription: Viet changelog\ntools: Read, Write\nmodel: sonnet\n---\nBan viet changelog.";

    fn writer() -> AgentDef {
        parse_agent("writer", MD).unwrap()
    }

    #[test]
    fn probation_cmdline_puts_prompt_first() {
        let s = probation_cmdline(Path::new("C:\\t\\p d"), Path::new("C:\\t\\s.json"), &writer());
        assert!(s.starts_with("claude -p \""), "{s}");
        for x in ["--output-format json", "--max-budget-usd 0.5", "--plugin-dir \"C:\\t\\p d\"", "--agent office:writer", "--permission-mode auto", "--strict-mcp-config", "--settings \"C:\\t\\s.json\""] {
            assert!(s.contains(x), "{x} / {s}");
        }
        assert!(s.ends_with("--tools \"Read\" \"Write\""), "{s}");
        assert!(!s.contains("bypass") && !s.contains("dangerously"));
    }

    #[test]
    fn parse_probation_reads_usage() {
        let out = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":3,"result":"Da viet CHANGELOG.md","total_cost_usd":0.12,
            "usage":{"input_tokens":2,"output_tokens":10,"cache_read_input_tokens":100,"cache_creation_input_tokens":1000},
            "permission_denials":[{"tool_name":"Bash","tool_input":{}}]}"#;
        let p = parse_probation(out, vec!["CHANGELOG.md".into()], 7);
        assert_eq!(p, Probation { ok: true, result: "Da viet CHANGELOG.md".into(), turns: 3, tokens: 1112, cost: 0.12, files: vec!["CHANGELOG.md".into()], when: 7, denied: vec!["Bash".into()] });
        let over = parse_probation(r#"{"subtype":"error_max_budget_usd","is_error":true,"num_turns":9}"#, vec![], 0);
        assert!(!over.ok && over.result.contains("error_max_budget_usd"));
    }

    #[test]
    fn parse_probation_bad_output() {
        let p = parse_probation("Error: not logged in", vec![], 0);
        assert!(!p.ok && p.result.contains("not logged in"));
        assert!(!parse_probation("  ", vec![], 0).result.is_empty());
    }

    /// Chay that (ton it token): cargo test --lib probation_real_run -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probation_real_run() {
        let root = tmp("real");
        let md = "---\nname: Hello\ndescription: Ghi file chao\ntools: Write, Read\nmodel: haiku\n---\nBan ghi file theo yeu cau, khong hoi lai.";
        candidate(&root, "hello", "software-dev", md);
        fs::write(candidates_dir(&root).join("hello").join("sample-task.md"), "Tao file hello.txt voi noi dung: xin chao").unwrap();
        let p = run_probation(&root, "hello").unwrap();
        println!("{p:?}");
        assert!(p.files.contains(&"hello.txt".to_string()), "{p:?}");
        assert!(candidates_dir(&root).join("hello").join("probation.json").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn candidate_tools_must_be_declared_and_safe() {
        let root = tmp("tools");
        candidate(&root, "evil", "software-dev", "---\nname: E\ndescription: d\ntools: Read; Remove-Item x\n---\np");
        candidate(&root, "all", "software-dev", "---\nname: A\ndescription: d\n---\np"); // khong khai tools = moi tool
        let cs = hr_candidates(root.to_string_lossy().into());
        assert!(cs.iter().all(|c| c.error.contains("tools")), "{:?}", cs.iter().map(|c| &c.error).collect::<Vec<_>>());
        assert!(hr_hire(root.to_string_lossy().into(), "evil".into()).is_err());
        assert!(run_probation(&root, "evil").unwrap_err().contains("tools"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn busy_candidate_cannot_be_hired_or_rejected() {
        let root = tmp("busy");
        let r = root.to_string_lossy().to_string();
        candidate(&root, "writer", "software-dev", MD);
        let g = Running::start("writer").unwrap();
        assert!(Running::start("writer").is_err()); // khong chay thu viec trung
        assert!(hr_hire(r.clone(), "writer".into()).unwrap_err().contains("thu viec"));
        assert!(hr_reject(r.clone(), "writer".into()).unwrap_err().contains("thu viec"));
        drop(g);
        hr_reject(r, "writer".into()).unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn hire_into_broken_department_writes_nothing() {
        let root = tmp("hire-broken");
        candidate(&root, "writer", "software-dev", MD);
        crate::office::edit_department(&root, "software-dev", |v| Ok(v["status"] = "hong".into())).unwrap();
        assert!(hr_hire(root.to_string_lossy().into(), "writer".into()).is_err()); // khong panic
        assert!(!root.join("departments/software-dev/agents/writer.md").exists());
        assert!(candidates_dir(&root).join("writer").exists());
        // agents/<cid>.md co san ma khong nam trong danh sach: khong ghi de
        crate::office::edit_department(&root, "software-dev", |v| Ok(v["status"] = serde_json::json!({}))).unwrap();
        fs::write(root.join("departments/software-dev/agents/writer.md"), "cu").unwrap();
        assert!(hr_hire(root.to_string_lossy().into(), "writer".into()).unwrap_err().contains("da co file"));
        assert_eq!(fs::read_to_string(root.join("departments/software-dev/agents/writer.md")).unwrap(), "cu");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn valid_id_rules() {
        for ok in ["changelog-writer", "qa2", "a"] {
            assert!(valid_id(ok), "{ok}");
        }
        for bad in ["", "..", "a/b", "A", "-x", "con", "com1", "a b", &"x".repeat(41)] {
            assert!(!valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn candidates_lists_and_reports_errors() {
        let root = tmp("list");
        candidate(&root, "writer", "software-dev", MD);
        candidate(&root, "broken", "software-dev", "khong co frontmatter");
        candidate(&root, "nodept", "khong-co", MD);
        candidate(&root, "dev", "software-dev", MD); // trung agent dang co
        fs::create_dir_all(candidates_dir(&root).join("_hired").join("old")).unwrap();
        fs::create_dir_all(candidates_dir(&root).join(".office")).unwrap();
        let cs = hr_candidates(root.to_string_lossy().into());
        assert_eq!(cs.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), vec!["broken", "dev", "nodept", "writer"]);
        let by = |id: &str| cs.iter().find(|c| c.id == id).unwrap();
        assert!(by("writer").error.is_empty() && by("writer").agent.is_some());
        assert_eq!(by("writer").sample, "Viet CHANGELOG.md");
        assert!(by("broken").agent.is_none() && by("broken").error.contains("frontmatter"));
        assert!(by("nodept").error.contains("phong"));
        assert!(by("dev").error.contains("da co agent"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn hire_onboards_as_probation() {
        let root = tmp("hire");
        let r = root.to_string_lossy().to_string();
        candidate(&root, "writer", "software-dev", MD);
        hr_hire(r.clone(), "writer".into()).unwrap();
        let d = load_office(&root).into_iter().find(|d| d.config.id == "software-dev").unwrap();
        assert!(d.agent_defs.iter().any(|a| a.id == "writer" && a.name == "Writer"));
        assert_eq!(agent_status(&d.config, "writer"), "probation");
        assert_eq!(d.config.budget.get("writer"), Some(&PROBATION_BUDGET));
        assert!(hr_candidates(r.clone()).is_empty());
        let hired: Vec<_> = fs::read_dir(candidates_dir(&root).join("_hired")).unwrap().flatten().collect();
        assert_eq!(hired.len(), 1);
        assert!(hired[0].path().join("agent.md").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn hire_rejects_existing_bad_or_broken() {
        let root = tmp("hire-bad");
        let r = root.to_string_lossy().to_string();
        candidate(&root, "dev", "software-dev", MD);
        assert!(hr_hire(r.clone(), "dev".into()).unwrap_err().contains("da co"));
        candidate(&root, "broken", "software-dev", "x");
        assert!(hr_hire(r.clone(), "broken".into()).is_err());
        assert!(hr_hire(r.clone(), "..".into()).is_err());
        assert!(hr_hire(r.clone(), "Bad".into()).is_err());
        // khong ghi gi vao phong khi tu choi
        let d = load_office(&root).into_iter().find(|d| d.config.id == "software-dev").unwrap();
        assert_eq!(d.config.agents.len(), 4);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reject_moves_folder() {
        let root = tmp("reject");
        let r = root.to_string_lossy().to_string();
        candidate(&root, "writer", "software-dev", MD);
        hr_reject(r.clone(), "writer".into()).unwrap();
        assert!(!candidates_dir(&root).join("writer").exists());
        let kept: Vec<_> = fs::read_dir(candidates_dir(&root).join("_rejected")).unwrap().flatten().collect();
        assert!(kept[0].path().join("sample-task.md").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn request_sets_hr_workspace_and_brief() {
        let root = tmp("request");
        let r = root.to_string_lossy().to_string();
        let t = hr_request(r.clone(), "software-dev".into(), "Can nguoi viet changelog\nchi tiet".into(), None).unwrap();
        assert_eq!((t.dept.as_str(), t.agent.as_str(), t.title.as_str()), ("hr", "recruiter", "Tuyen: Can nguoi viet changelog"));
        let hr = load_office(&root).into_iter().find(|d| d.config.id == "hr").unwrap();
        assert_eq!(Path::new(&hr.config.workspace), candidates_dir(&root));
        assert_eq!(Path::new(&t.cwd), candidates_dir(&root));
        let brief = fs::read_to_string(Path::new(&t.dir).join("brief.md")).unwrap();
        assert!(brief.contains("Software Development") && brief.contains("tech-lead") && brief.contains("candidate.json"), "{brief}");
        assert_eq!(crate::tasks::load_tasks(&root).unwrap().len(), 1);
        // .office cua task nam trong candidates nhung khong thanh ung vien
        assert!(hr_candidates(r).is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
