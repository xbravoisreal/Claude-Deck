use crate::office::{edit_department, load_office, parse_agent, AgentDef};
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
    let c = read_candidate(rp, &candidates_dir(rp).join(&cid), &cid);
    if !c.error.is_empty() {
        return Err(if c.error.contains("da co agent") { format!("da co agent {cid}") } else { c.error });
    }
    let md = std::fs::read_to_string(candidates_dir(rp).join(&cid).join("agent.md")).map_err(err)?;
    let agents = rp.join("departments").join(&c.dept).join("agents");
    std::fs::create_dir_all(&agents).map_err(err)?;
    std::fs::write(agents.join(format!("{cid}.md")), md).map_err(err)?;
    edit_department(rp, &c.dept, |v| {
        if !v["agents"].is_array() {
            v["agents"] = serde_json::json!([]);
        }
        v["agents"].as_array_mut().unwrap().push(cid.clone().into());
        v["status"][&cid] = "probation".into();
        v["budget"][&cid] = PROBATION_BUDGET.into();
        Ok(())
    })?;
    archive(rp, &cid, "_hired")
}

#[tauri::command]
pub fn hr_reject(root: String, cid: String) -> Result<(), String> {
    let rp = Path::new(&root);
    if !valid_id(&cid) || !candidates_dir(rp).join(&cid).is_dir() {
        return Err(format!("khong co ung vien {cid}"));
    }
    archive(rp, &cid, "_rejected")
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
