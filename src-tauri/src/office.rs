use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::pty::ps_quote;
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

#[derive(Serialize, Deserialize, Clone, Default)]
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
    /// room.json (bo tri phong, xem themes/README.md); thieu/hong -> {}.
    pub room: serde_json::Value,
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
            if !dir.is_dir() {
                return None;
            }
            let id = dir.file_name()?.to_string_lossy().into_owned();
            // Thu muc phong moi tao tay chua co department.json -> tao file toi thieu (ten = ten thu muc).
            if !dir.join("department.json").exists() {
                let _ = write_department(&dir, &DeptConfig { id: id.clone(), name: id.clone(), icon: "🏢".into(), ..Default::default() });
            }
            let parsed = std::fs::read_to_string(dir.join("department.json"))
                .map_err(err)
                .and_then(|s| serde_json::from_str::<DeptConfig>(&s).map_err(err));
            // department.json hong -> van hien phong kem loi, khong de phong bien mat.
            let config = match parsed {
                Ok(c) => c,
                Err(e) => {
                    let config = DeptConfig { id: id.clone(), name: id, ..Default::default() };
                    return Some(Department { config, agent_defs: vec![], errors: vec![format!("department.json: {e}")], room: serde_json::json!({}) });
                }
            };
            let mut errors = vec![];
            let room = match std::fs::read_to_string(dir.join("room.json")) {
                Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                    errors.push(format!("room.json: {e}"));
                    serde_json::json!({})
                }),
                Err(_) => serde_json::json!({}),
            };
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
            Some(Department { config, agent_defs, errors, room })
        })
        .collect();
    out.sort_by(|a, b| a.config.id.cmp(&b.config.id));
    out
}

/// Ten plugin tam; agent trong phien la `office:<id>`.
const PLUGIN: &str = "office";

/// Ghi plugin tam chua agent cua phong (`--agents <file>` chi chay voi --print, nen dung `--plugin-dir`).
/// Plugin agent duoc goi bang `name` -> ghi name = id. Gia tri ghi dang chuoi JSON (hop le YAML).
pub fn write_plugin(d: &Department, dir: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir.join(".claude-plugin"))?;
    std::fs::create_dir_all(dir.join("agents"))?;
    std::fs::write(dir.join(".claude-plugin").join("plugin.json"), serde_json::json!({ "name": PLUGIN, "version": "0.0.0" }).to_string())?;
    for a in &d.agent_defs {
        let q = |s: &str| serde_json::Value::from(s).to_string();
        let mut head = format!("name: {}\ndescription: {}\n", a.id, q(&a.description));
        if !a.tools.is_empty() {
            head += &format!("tools: {}\n", a.tools.join(", "));
        }
        if let Some(m) = &a.model {
            head += &format!("model: {m}\n");
        }
        std::fs::write(dir.join("agents").join(format!("{}.md", a.id)), format!("---\n{head}---\n{}", a.prompt))?;
    }
    Ok(())
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
    ("departments/software-dev/room.json", include_str!("../office-seed/departments/software-dev/room.json")),
    ("departments/hr/room.json", include_str!("../office-seed/departments/hr/room.json")),
    ("themes/README.md", include_str!("../office-seed/themes/README.md")),
];

/// Ghi du lieu mac dinh, chi nhung file chua ton tai (khong de len ban nguoi dung da sua).
/// Theme mac dinh cung duoc ghi ra `themes/default` lam mau cho chu app copy.
pub fn seed_office(root: &Path) -> std::io::Result<()> {
    let theme = crate::theme::DEFAULT_FILES.iter().map(|(n, c)| (format!("themes/default/{n}"), *c));
    for (rel, content) in SEED.iter().map(|(r, c)| (r.to_string(), *c)).chain(theme) {
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

fn write_department(dir: &Path, c: &DeptConfig) -> std::io::Result<()> {
    let mut v = serde_json::to_value(c)?;
    v["defaults"] = serde_json::json!({ "maxTokens": 4_000_000 });
    std::fs::write(dir.join("department.json"), serde_json::to_string_pretty(&v)?)
}

/// Tao phong moi (nut + tren man Office): thu muc theo ten (chu thuong, khong dau cach), department.json, room.json.
/// Tra ve id phong. Agent them sau (sua department.json hoac qua HR).
#[tauri::command]
pub fn office_create_department(root: String, name: String, icon: String, mission: String) -> Result<String, String> {
    let mut id = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c);
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    let id = id.trim_matches('-').to_string();
    if id.is_empty() {
        return Err("ten phong can co chu cai hoac so (khong dau)".into());
    }
    let dir = Path::new(&root).join("departments").join(&id);
    if dir.exists() {
        return Err(format!("da co phong {id}"));
    }
    std::fs::create_dir_all(&dir).map_err(err)?;
    let icon = if icon.trim().is_empty() { "🏢".into() } else { icon.trim().into() };
    write_department(&dir, &DeptConfig { id: id.clone(), name: name.trim().into(), icon, mission: mission.trim().into(), ..Default::default() }).map_err(err)?;
    let room = serde_json::json!({
        "cols": 6, "rows": 4, "floor": "floor.wood",
        "sign": { "x": 0, "y": 0 }, "workspace": { "x": 3, "y": 0, "w": 3 },
        "decor": [{ "slot": "decor.plant", "x": 5, "y": 3 }],
        "seats": {}
    });
    std::fs::write(dir.join("room.json"), serde_json::to_string_pretty(&room).map_err(err)?).map_err(err)?;
    Ok(id)
}

/// Chi doi truong `workspace`, giu nguyen cac truong khac (ke ca truong nguoi dung tu them).
#[tauri::command]
pub fn office_set_workspace(root: String, dept: String, workspace: String) -> Result<(), String> {
    let p = Path::new(&root).join("departments").join(&dept).join("department.json");
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).map_err(err)?).map_err(err)?;
    v["workspace"] = workspace.into();
    std::fs::write(&p, serde_json::to_string_pretty(&v).map_err(err)?).map_err(err)
}

/// Cam cung cho moi phien Office (spec muc 5).
const DENY_ALL: &[&str] = &[
    "Bash(git push:*)",
    "Bash(rm -rf:*)",
    "Bash(Remove-Item:*)",
    "Bash(del /s:*)",
    "Bash(curl:*)",
    "Bash(Invoke-WebRequest:*)",
    "WebFetch",
    "WebSearch",
];
const DENY_REVIEWER: &[&str] = &["Edit", "Write", "NotebookEdit"];

#[derive(Serialize)]
pub struct LaunchSpec {
    /// Tham so them cho `claude` (pty_spawn dat sau `--settings`).
    pub args: String,
    /// Gop vao file settings cua phien (cung file voi hook).
    pub settings: serde_json::Value,
}

/// Agent di qua plugin dir (file), khong qua chuoi tham so: PowerShell 5.1 lam hong dau `"` khi truyen cho exe.
pub fn launch_spec(d: &Department, agent_id: &str, plugin_dir: &Path, notes_root: Option<&str>) -> Result<LaunchSpec, String> {
    let agent = d.agent_defs.iter().find(|a| a.id == agent_id).ok_or(format!("khong co agent {agent_id}"))?;
    // Lead goi subagent nen phien can du tool cua ca phong; agent khac chi can tool cua minh.
    let mut tools: Vec<&str> = vec![];
    let pool: Vec<&AgentDef> = if agent_id == d.config.lead { d.agent_defs.iter().collect() } else { vec![agent] };
    for t in pool.iter().flat_map(|a| &a.tools) {
        if !tools.contains(&t.as_str()) {
            tools.push(t);
        }
    }
    let mut args = format!(
        "--plugin-dir {} --agent {} --permission-mode auto --strict-mcp-config",
        ps_quote(&plugin_dir.to_string_lossy()),
        ps_quote(&format!("{PLUGIN}:{agent_id}"))
    );
    if !tools.is_empty() {
        args += &format!(" --tools {}", tools.join(" "));
    }
    if let Some(n) = notes_root.filter(|n| !n.is_empty()) {
        args += &format!(" --add-dir {}", ps_quote(&Path::new(n).join("ClaudeDeck").to_string_lossy()));
    }
    let mut deny: Vec<&str> = DENY_ALL.to_vec();
    if agent_id == "reviewer" {
        deny.extend(DENY_REVIEWER);
    }
    Ok(LaunchSpec { args, settings: serde_json::json!({ "permissions": { "deny": deny } }) })
}

/// Ghi plugin tam cua phong + thu muc notes, roi dung tham so khoi chay.
pub fn prepare_launch(d: &Department, agent: &str, notes_root: Option<&str>) -> Result<LaunchSpec, String> {
    let dir = std::env::temp_dir().join(format!("claudedeck-plugin-{}", d.config.id));
    write_plugin(d, &dir).map_err(err)?;
    if let Some(n) = notes_root.filter(|n| !n.is_empty()) {
        std::fs::create_dir_all(Path::new(n).join("ClaudeDeck")).map_err(err)?;
    }
    launch_spec(d, agent, &dir, notes_root)
}

#[tauri::command]
pub fn office_launch(root: String, dept: String, agent: String, notes_root: Option<String>) -> Result<LaunchSpec, String> {
    let d = load_office(Path::new(&root)).into_iter().find(|d| d.config.id == dept).ok_or(format!("khong co phong {dept}"))?;
    prepare_launch(&d, &agent, notes_root.as_deref())
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
    fn plugin_keeps_prompt_verbatim() {
        let root = tmp("verbatim");
        let prompt = "Dung 'x' va \"y\"\ndong 2";
        dept(&root, &["ok"], &[("ok", &format!("---\nname: Ok Agent\ndescription: a: \"b\"\n---\n{prompt}"))]);
        let d = &load_office(&root)[0];
        let dir = root.join("plugin");
        write_plugin(d, &dir).unwrap();
        let md = fs::read_to_string(dir.join("agents/ok.md")).unwrap();
        let a = parse_agent("ok", &md).unwrap();
        assert_eq!(a.prompt, prompt);
        assert_eq!(a.name, "ok"); // plugin agent duoc goi bang name -> phai la id
        assert!(!md.contains("tools:")); // khong co tools -> bo truong (ke thua)
        let plugin: serde_json::Value = serde_json::from_str(&fs::read_to_string(dir.join(".claude-plugin/plugin.json")).unwrap()).unwrap();
        assert_eq!(plugin["name"], "office");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn load_reports_broken_department() {
        let root = tmp("broken-dept");
        fs::create_dir_all(root.join("departments/oops")).unwrap();
        fs::write(root.join("departments/oops/department.json"), "{ not json").unwrap();
        let o = load_office(&root);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].config.id, "oops");
        assert!(o[0].errors[0].contains("department.json"));
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
    fn seed_writes_rooms_and_theme() {
        let root = tmp("seed-room");
        seed_office(&root).unwrap();
        let o = load_office(&root);
        assert_eq!(o[1].room["seats"]["dev"]["y"], 3);
        assert_eq!(o[0].room["floor"], "floor.carpet");
        assert!(root.join("themes/default/theme.json").exists());
        assert!(root.join("themes/default/desk.svg").exists());
        assert!(root.join("themes/README.md").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn load_creates_missing_department_json() {
        let root = tmp("nodept");
        fs::create_dir_all(root.join("departments/content-media")).unwrap();
        let o = load_office(&root);
        assert_eq!(o.len(), 1);
        assert!(o[0].errors.is_empty(), "{:?}", o[0].errors);
        assert_eq!(o[0].config.id, "content-media");
        assert_eq!(o[0].config.name, "content-media");
        assert!(root.join("departments/content-media/department.json").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn create_department_writes_files() {
        let root = tmp("newdept");
        let r = root.to_string_lossy().to_string();
        let id = office_create_department(r.clone(), "Content Media!".into(), "🎬".into(), "Lam video".into()).unwrap();
        assert_eq!(id, "content-media");
        let d = load_office(&root).into_iter().find(|d| d.config.id == id).unwrap();
        assert_eq!((d.config.name.as_str(), d.config.icon.as_str(), d.config.mission.as_str()), ("Content Media!", "🎬", "Lam video"));
        assert!(d.errors.is_empty());
        assert_eq!(d.room["workspace"]["w"], 3);
        assert!(office_create_department(r.clone(), "content media".into(), "".into(), "".into()).is_err()); // trung
        assert!(office_create_department(r, "!!!".into(), "".into(), "".into()).is_err()); // ten rong sau khi lam sach
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn room_missing_is_default() {
        let root = tmp("room");
        dept(&root, &["ok"], &[("ok", OK)]);
        let d = &load_office(&root)[0];
        assert_eq!(d.room, serde_json::json!({}));
        assert!(d.errors.is_empty());
        fs::write(root.join("departments/t/room.json"), "{ hong").unwrap();
        let d = &load_office(&root)[0];
        assert_eq!(d.room, serde_json::json!({}));
        assert!(d.errors[0].contains("room.json"));
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

    fn software_dev(name: &str) -> (PathBuf, Department) {
        let root = tmp(name);
        seed_office(&root).unwrap();
        let d = load_office(&root).into_iter().find(|d| d.config.id == "software-dev").unwrap();
        (root, d)
    }

    fn deny(s: &LaunchSpec) -> Vec<String> {
        serde_json::from_value(s.settings["permissions"]["deny"].clone()).unwrap()
    }

    #[test]
    fn launch_spec_for_reviewer() {
        let (root, d) = software_dev("ls-rev");
        let s = launch_spec(&d, "reviewer", Path::new("C:\\t\\p"), None).unwrap();
        assert!(s.args.starts_with("--plugin-dir 'C:\\t\\p' --agent 'office:reviewer' --permission-mode auto --strict-mcp-config --tools "), "{}", s.args);
        assert!(!s.args.contains("dangerously"));
        assert!(!s.args.contains("--add-dir"));
        let d = deny(&s);
        for x in ["Bash(git push:*)", "WebFetch", "Edit", "Write"] {
            assert!(d.contains(&x.to_string()), "{x}");
        }
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn launch_spec_dev_can_edit() {
        let (root, d) = software_dev("ls-dev");
        let d = deny(&launch_spec(&d, "dev", Path::new("a.json"), None).unwrap());
        assert!(!d.contains(&"Edit".to_string()));
        assert!(d.contains(&"Bash(git push:*)".to_string()));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn launch_spec_quotes_paths_with_spaces() {
        let (root, d) = software_dev("ls-space");
        let s = launch_spec(&d, "dev", Path::new("a.json"), Some("H:\\My Notes\\Vault")).unwrap();
        assert!(s.args.ends_with("--add-dir 'H:\\My Notes\\Vault\\ClaudeDeck'"), "{}", s.args);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn launch_spec_escapes_apostrophe() {
        let (root, d) = software_dev("ls-apos");
        let s = launch_spec(&d, "dev", Path::new("a"), Some("H:\\Bob's Vault")).unwrap();
        assert!(s.args.ends_with("--add-dir 'H:\\Bob''s Vault\\ClaudeDeck'"), "{}", s.args);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn launch_spec_unknown_agent_errors() {
        let (root, d) = software_dev("ls-ghost");
        assert!(launch_spec(&d, "ghost", Path::new("a.json"), None).is_err());
        fs::remove_dir_all(&root).unwrap();
    }
}
