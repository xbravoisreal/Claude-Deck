use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
pub struct AgentDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub model: Option<String>,
    pub prompt: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DeptConfig {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub mission: String,
    #[serde(default)]
    pub workspace: String,
    pub lead: String,
    pub agents: Vec<String>,
    /// agent id -> active | probation | retired
    #[serde(default)]
    pub status: HashMap<String, String>,
    /// vd {"maxTokens": 4000000}
    #[serde(default)]
    pub defaults: serde_json::Value,
}

#[derive(Serialize, Clone)]
pub struct Department {
    #[serde(flatten)]
    pub config: DeptConfig,
    pub agent_defs: Vec<AgentDef>,
    pub errors: Vec<String>,
}

/// File agent dang subagent cua Claude Code: frontmatter `key: value` + than la system prompt.
pub fn parse_agent(id: &str, md: &str) -> Result<AgentDef, String> {
    let rest = md.trim_start().strip_prefix("---").ok_or(format!("{id}: thieu frontmatter"))?;
    let (head, body) = rest.split_once("\n---").ok_or(format!("{id}: frontmatter chua dong"))?;
    let mut kv = HashMap::new();
    for line in head.lines() {
        if let Some((k, v)) = line.split_once(':') {
            kv.insert(k.trim(), v.trim().trim_matches(|c| c == '"' || c == '\'').to_string());
        }
    }
    let description = kv.get("description").filter(|d| !d.is_empty()).ok_or(format!("{id}: thieu description"))?.clone();
    Ok(AgentDef {
        id: id.into(),
        name: kv.get("name").cloned().unwrap_or_else(|| id.into()),
        description,
        tools: kv.get("tools").map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default(),
        model: kv.get("model").cloned().filter(|m| !m.is_empty()),
        prompt: body.trim_start_matches(|c| c == '\r' || c == '\n').to_string(),
    })
}

/// Doc moi root/departments/*/department.json kem agent; agent loi bi bo qua va ghi vao `errors`.
pub fn load_office(root: &Path) -> Vec<Department> {
    let Ok(rd) = std::fs::read_dir(root.join("departments")) else { return vec![] };
    let mut out: Vec<Department> = rd
        .filter_map(Result::ok)
        .filter_map(|e| {
            let dir = e.path();
            let config: DeptConfig = serde_json::from_str(&std::fs::read_to_string(dir.join("department.json")).ok()?).ok()?;
            let mut errors = vec![];
            let agent_defs = config
                .agents
                .iter()
                .filter_map(|id| {
                    let r = std::fs::read_to_string(dir.join("agents").join(format!("{id}.md")))
                        .map_err(|e| format!("{id}: {e}"))
                        .and_then(|md| parse_agent(id, &md));
                    r.map_err(|e| errors.push(e)).ok()
                })
                .collect();
            Some(Department { config, agent_defs, errors })
        })
        .collect();
    out.sort_by(|a, b| a.config.id.cmp(&b.config.id));
    out
}

/// JSON cho `claude --agents`: {"<id>": {description, prompt, tools, model?}}.
pub fn agents_json(d: &Department) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for a in &d.agent_defs {
        let mut v = serde_json::json!({ "description": a.description, "prompt": a.prompt, "tools": a.tools });
        if let Some(model) = &a.model {
            v["model"] = model.clone().into();
        }
        m.insert(a.id.clone(), v);
    }
    m.into()
}

/// Du lieu mac dinh: (duong dan tuong doi trong root, noi dung).
const SEED: &[(&str, &str)] = &[
    ("departments/software-dev/department.json", include_str!("../office-seed/departments/software-dev/department.json")),
    ("departments/software-dev/agents/tech-lead.md", include_str!("../office-seed/departments/software-dev/agents/tech-lead.md")),
    ("departments/software-dev/agents/dev.md", include_str!("../office-seed/departments/software-dev/agents/dev.md")),
    ("departments/software-dev/agents/reviewer.md", include_str!("../office-seed/departments/software-dev/agents/reviewer.md")),
    ("departments/software-dev/agents/qa.md", include_str!("../office-seed/departments/software-dev/agents/qa.md")),
    ("departments/hr/department.json", include_str!("../office-seed/departments/hr/department.json")),
    ("departments/hr/agents/recruiter.md", include_str!("../office-seed/departments/hr/agents/recruiter.md")),
];

/// Ghi du lieu mac dinh, chi nhung file chua ton tai (khong de len ban nguoi dung da sua).
pub fn seed_office(root: &Path) -> std::io::Result<()> {
    for (rel, content) in SEED {
        let p = root.join(rel);
        if !p.exists() {
            std::fs::create_dir_all(p.parent().unwrap())?;
            std::fs::write(p, content)?;
        }
    }
    Ok(())
}

fn err<E: ToString>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub fn office_root() -> String {
    dirs::config_dir().unwrap_or_default().join("claudedeck").join("office").to_string_lossy().into_owned()
}

#[tauri::command]
pub fn office_load(root: String) -> Result<Vec<Department>, String> {
    let root = Path::new(&root);
    seed_office(root).map_err(err)?;
    Ok(load_office(root))
}

/// Chi doi truong `workspace`, giu nguyen cac truong khac (ke ca truong nguoi dung tu them).
#[tauri::command]
pub fn office_set_workspace(root: String, dept: String, workspace: String) -> Result<(), String> {
    let p = Path::new(&root).join("departments").join(&dept).join("department.json");
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).map_err(err)?).map_err(err)?;
    v["workspace"] = workspace.into();
    std::fs::write(&p, serde_json::to_string_pretty(&v).map_err(err)?).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cd-office-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// Tao phong "t" voi danh sach agent va cac file agent cho truoc.
    fn dept(root: &Path, agents: &[&str], files: &[(&str, &str)]) {
        let d = root.join("departments").join("t");
        fs::create_dir_all(d.join("agents")).unwrap();
        let cfg = serde_json::json!({"id":"t","name":"T","icon":"x","mission":"m","lead":agents[0],"agents":agents});
        fs::write(d.join("department.json"), cfg.to_string()).unwrap();
        for (id, md) in files {
            fs::write(d.join("agents").join(format!("{id}.md")), md).unwrap();
        }
    }

    const OK: &str = "---\nname: Ok\ndescription: d\ntools: Read\n---\nbody";

    #[test]
    fn parse_agent_reads_frontmatter() {
        let a = parse_agent("dev", "---\nname: Dev\ndescription: \"Lam code\"\ntools: Read, Edit, Bash\nmodel: sonnet\n---\nBan la dev.\n").unwrap();
        assert_eq!(a.name, "Dev");
        assert_eq!(a.description, "Lam code");
        assert_eq!(a.tools, vec!["Read", "Edit", "Bash"]);
        assert_eq!(a.model.as_deref(), Some("sonnet"));
        assert_eq!(a.prompt.trim(), "Ban la dev.");
    }

    #[test]
    fn parse_agent_rejects_missing_description() {
        assert!(parse_agent("x", "---\nname: X\n---\nbody").is_err());
        assert!(parse_agent("x", "no frontmatter").is_err());
    }

    #[test]
    fn load_skips_broken_agent() {
        let root = tmp("broken");
        dept(&root, &["ok", "bad"], &[("ok", OK), ("bad", "no frontmatter")]);
        let d = &load_office(&root)[0];
        assert_eq!(d.agent_defs.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["ok"]);
        assert_eq!(d.errors.len(), 1);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn load_skips_missing_agent_file() {
        let root = tmp("missing");
        dept(&root, &["ok", "ghost"], &[("ok", OK)]);
        let d = &load_office(&root)[0];
        assert_eq!(d.agent_defs.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["ok"]);
        assert!(d.errors[0].contains("ghost"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn agents_json_keeps_prompt_verbatim() {
        let root = tmp("verbatim");
        let prompt = "Dung 'x' va \"y\"\ndong 2";
        dept(&root, &["ok"], &[("ok", &format!("---\ndescription: d\n---\n{prompt}"))]);
        let d = &load_office(&root)[0];
        assert_eq!(agents_json(d)["ok"]["prompt"], prompt);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn seed_creates_two_departments() {
        let root = tmp("seed");
        seed_office(&root).unwrap();
        let o = load_office(&root);
        assert_eq!(o.iter().map(|d| d.config.id.as_str()).collect::<Vec<_>>(), vec!["hr", "software-dev"]);
        assert_eq!(o[1].agent_defs.len(), 4);
        assert!(o.iter().all(|d| d.errors.is_empty()));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn seed_does_not_overwrite() {
        let root = tmp("seed-keep");
        seed_office(&root).unwrap();
        let dev = root.join("departments/software-dev/agents/dev.md");
        fs::write(&dev, "---\nname: Dev2\ndescription: d\n---\nx").unwrap();
        seed_office(&root).unwrap();
        assert_eq!(parse_agent("dev", &fs::read_to_string(&dev).unwrap()).unwrap().name, "Dev2");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn set_workspace_persists() {
        let root = tmp("ws");
        seed_office(&root).unwrap();
        let r = root.to_string_lossy().to_string();
        office_set_workspace(r, "software-dev".into(), "H:\\proj".into()).unwrap();
        let d = load_office(&root).into_iter().find(|d| d.config.id == "software-dev").unwrap();
        assert_eq!(d.config.workspace, "H:\\proj");
        assert_eq!(d.config.agents.len(), 4);
        assert_eq!(d.config.status.len(), 4);
        fs::remove_dir_all(&root).unwrap();
    }
}
