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
    /// agent id -> tran token doc cache rieng (Agent thu viec); thieu -> defaults.maxTokens.
    #[serde(default)]
    pub budget: HashMap<String, u64>,
    /// vd {"maxTokens": 4000000}
    #[serde(default)]
    pub defaults: serde_json::Value,
    /// Chon model theo loai task (luat dau tien khop thang); khong luat nao khop -> model trong file Agent.
    #[serde(default)]
    pub router: Vec<Route>,
}

/// vd {"agent": "dev", "match": ["typo", "readme"], "model": "haiku"}; agent rong = moi Agent cua phong.
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Route {
    #[serde(default)]
    pub agent: String,
    #[serde(rename = "match")]
    pub keywords: Vec<String>,
    pub model: String,
}

/// Ten model di vao frontmatter plugin: alias (haiku, sonnet, opus) hoac id day du (claude-...).
pub fn valid_model(m: &str) -> bool {
    !m.is_empty() && m.len() <= 60 && m.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
}

/// Model cho task moi theo router cua phong ("" = model mac dinh cua Agent). So khop chu thuong, chua tu khoa.
pub fn route(c: &DeptConfig, agent: &str, text: &str) -> String {
    let text = text.to_lowercase();
    c.router
        .iter()
        .find(|r| (r.agent.is_empty() || r.agent == agent) && valid_model(&r.model) && r.keywords.iter().map(|k| k.trim().to_lowercase()).any(|k| !k.is_empty() && text.contains(&k)))
        .map_or(String::new(), |r| r.model.clone())
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

/// active | probation | retired (thieu = active).
pub fn agent_status<'a>(c: &'a DeptConfig, id: &str) -> &'a str {
    c.status.get(id).map_or("active", String::as_str)
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
    // Agent da nghi khong vao plugin: Lead khong goi duoc no lam subagent.
    for a in d.agent_defs.iter().filter(|a| agent_status(&d.config, &a.id) != "retired") {
        let q = |s: &str| serde_json::Value::from(s).to_string();
        let mut head = format!("name: {}\ndescription: {}\n", a.id, q(&a.description));
        if !a.tools.is_empty() {
            head += &format!("tools: {}\n", safe_tools(&a.tools).join(", "));
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
    // Template phong (nut + "Mo phong moi"): them thu muc vao templates/departments la co template moi, khong sua code.
    ("templates/departments/software-dev/department.json", include_str!("../office-seed/departments/software-dev/department.json")),
    ("templates/departments/software-dev/room.json", include_str!("../office-seed/departments/software-dev/room.json")),
    ("templates/departments/software-dev/agents/tech-lead.md", include_str!("../office-seed/departments/software-dev/agents/tech-lead.md")),
    ("templates/departments/software-dev/agents/dev.md", include_str!("../office-seed/departments/software-dev/agents/dev.md")),
    ("templates/departments/software-dev/agents/reviewer.md", include_str!("../office-seed/departments/software-dev/agents/reviewer.md")),
    ("templates/departments/software-dev/agents/qa.md", include_str!("../office-seed/departments/software-dev/agents/qa.md")),
    ("templates/departments/media-content/department.json", include_str!("../office-seed/templates/departments/media-content/department.json")),
    ("templates/departments/media-content/room.json", include_str!("../office-seed/templates/departments/media-content/room.json")),
    ("templates/departments/media-content/agents/strategist.md", include_str!("../office-seed/templates/departments/media-content/agents/strategist.md")),
    ("templates/departments/media-content/agents/scriptwriter.md", include_str!("../office-seed/templates/departments/media-content/agents/scriptwriter.md")),
    ("templates/departments/media-content/agents/video-editor.md", include_str!("../office-seed/templates/departments/media-content/agents/video-editor.md")),
    ("templates/departments/media-content/agents/social-poster.md", include_str!("../office-seed/templates/departments/media-content/agents/social-poster.md")),
    ("templates/departments/media-content/agents/analyst.md", include_str!("../office-seed/templates/departments/media-content/agents/analyst.md")),
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

fn templates_dir(root: &Path) -> std::path::PathBuf {
    root.join("templates").join("departments")
}

/// Template phong: `<root>/templates/departments/<id>/` gom department.json (lead, agents, status), agents/*.md, room.json.
#[derive(Serialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub mission: String,
    pub agents: Vec<String>,
}

#[tauri::command]
pub fn office_templates(root: String) -> Vec<Template> {
    let Ok(rd) = std::fs::read_dir(templates_dir(Path::new(&root))) else { return vec![] };
    let mut out: Vec<Template> = rd
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().into_string().ok().filter(|id| crate::hr::valid_id(id))?;
            let c: DeptConfig = serde_json::from_str(&std::fs::read_to_string(e.path().join("department.json")).ok()?).ok()?;
            Some(Template { id, name: c.name, icon: c.icon, mission: c.mission, agents: c.agents })
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Doc het template vao bo nho truoc khi tao phong: template hong thi khong de lai phong do dang.
fn read_template(root: &Path, tid: &str) -> Result<(DeptConfig, Vec<(String, String)>, Option<String>), String> {
    let dir = templates_dir(root).join(tid);
    if !crate::hr::valid_id(tid) || !dir.is_dir() {
        return Err(format!("khong co template {tid}"));
    }
    let c: DeptConfig = serde_json::from_str(&std::fs::read_to_string(dir.join("department.json")).map_err(|e| format!("template {tid}: {e}"))?).map_err(|e| format!("template {tid}: {e}"))?;
    if !c.agents.contains(&c.lead) {
        return Err(format!("template {tid}: lead '{}' khong nam trong agents", c.lead));
    }
    let mut agents = vec![];
    for a in &c.agents {
        if !crate::hr::valid_id(a) {
            return Err(format!("template {tid}: agent id '{a}' khong hop le"));
        }
        let md = std::fs::read_to_string(dir.join("agents").join(format!("{a}.md"))).map_err(|e| format!("template {tid}: {a}.md: {e}"))?;
        parse_agent(a, &md)?;
        agents.push((a.clone(), md));
    }
    Ok((c, agents, std::fs::read_to_string(dir.join("room.json")).ok()))
}

/// Tao phong moi (nut + tren man Office): thu muc theo ten (chu thuong, khong dau cach), department.json, room.json.
/// Co `template` thi chep doi hinh (agent, lead, trang thai) va room.json cua template; khong thi phong trong,
/// Agent them sau (sua department.json hoac qua HR).
#[tauri::command]
pub fn office_create_department(root: String, name: String, icon: String, mission: String, template: Option<String>) -> Result<String, String> {
    let tpl = template.filter(|t| !t.is_empty()).map(|t| read_template(Path::new(&root), &t)).transpose()?;
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
    let mut config = DeptConfig { id: id.clone(), name: name.trim().into(), icon, mission: mission.trim().into(), ..Default::default() };
    let mut room = serde_json::to_string_pretty(&serde_json::json!({
        "cols": 6, "rows": 4, "floor": "floor.wood",
        "sign": { "x": 0, "y": 0 }, "workspace": { "x": 3, "y": 0, "w": 3 },
        "decor": [{ "slot": "decor.plant", "x": 5, "y": 3 }],
        "seats": {}
    }))
    .map_err(err)?;
    if let Some((t, agents, t_room)) = tpl {
        std::fs::create_dir_all(dir.join("agents")).map_err(err)?;
        for (a, md) in agents {
            std::fs::write(dir.join("agents").join(format!("{a}.md")), md).map_err(err)?;
        }
        (config.lead, config.agents, config.status) = (t.lead, t.agents, t.status);
        room = t_room.unwrap_or(room);
    }
    write_department(&dir, &config).map_err(err)?;
    std::fs::write(dir.join("room.json"), room).map_err(err)?;
    Ok(id)
}

/// Sua department.json qua Value: giu nguyen cac truong khac (ke ca truong nguoi dung tu them).
pub fn edit_department(root: &Path, dept: &str, f: impl FnOnce(&mut serde_json::Value) -> Result<(), String>) -> Result<(), String> {
    let p = root.join("departments").join(dept).join("department.json");
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).map_err(err)?).map_err(err)?;
    f(&mut v)?;
    std::fs::write(&p, serde_json::to_string_pretty(&v).map_err(err)?).map_err(err)
}

#[tauri::command]
pub fn office_set_workspace(root: String, dept: String, workspace: String) -> Result<(), String> {
    edit_department(Path::new(&root), &dept, |v| Ok(v["workspace"] = workspace.into()))
}

/// Chinh thuc (active, bo tran rieng) / thu viec / cho nghi. Cho nghi khi con task chua xong -> tu choi.
#[tauri::command]
pub fn office_set_agent_status(root: String, dept: String, agent: String, status: String) -> Result<(), String> {
    if !["active", "probation", "retired"].contains(&status.as_str()) {
        return Err(format!("trang thai khong hop le: {status}"));
    }
    let rp = Path::new(&root);
    if status == "retired" && load_office(rp).iter().any(|d| d.config.id == dept && d.config.lead == agent) {
        return Err(format!("{agent} la lead cua phong, khong cho nghi duoc"));
    }
    // Kiem tra task va ghi trong cung khoa tasks.json: task moi khong chen vao giua.
    crate::tasks::with_tasks(rp, |ts| {
        let open = ts.iter().filter(|t| t.dept == dept && t.agent == agent && t.status != "done").count();
        if status == "retired" && open > 0 {
            return Err(format!("{agent} con task chua xong ({open}): duyet hoac huy truoc"));
        }
        edit_department(rp, &dept, |v| set_status(v, &agent, &status))
    })
}

fn set_status(v: &mut serde_json::Value, agent: &str, status: &str) -> Result<(), String> {
    {
        if !(v["status"].is_object() || v["status"].is_null()) {
            return Err("department.json: status phai la object".into());
        }
        v["status"][agent] = status.into();
        if status == "active" {
            if let Some(b) = v.get_mut("budget").and_then(|b| b.as_object_mut()) {
                b.remove(agent);
            }
        }
        Ok(())
    }
}

/// Cam cung cho moi phien Office (spec muc 5).
pub(crate) const DENY_ALL: &[&str] = &[
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

/// Ten tool di vao dong lenh (PowerShell / cmd): chi giu ky tu an toan, bo tool la (vd do Recruiter viet "Read; Remove-Item ...").
pub fn safe_tools(tools: &[String]) -> Vec<&str> {
    tools
        .iter()
        .map(String::as_str)
        .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || "_-:*.() /".contains(c)))
        .collect()
}

#[derive(Serialize, Debug)]
pub struct LaunchSpec {
    /// Tham so them cho `claude` (pty_spawn dat sau `--settings`).
    pub args: String,
    /// Gop vao file settings cua phien (cung file voi hook).
    pub settings: serde_json::Value,
}

/// Agent di qua plugin dir (file), khong qua chuoi tham so: PowerShell 5.1 lam hong dau `"` khi truyen cho exe.
pub fn launch_spec(d: &Department, agent_id: &str, plugin_dir: &Path, notes_root: Option<&str>) -> Result<LaunchSpec, String> {
    let agent = d.agent_defs.iter().find(|a| a.id == agent_id).ok_or(format!("khong co agent {agent_id}"))?;
    if agent_status(&d.config, agent_id) == "retired" {
        return Err(format!("{agent_id} da nghi viec"));
    }
    // Tool la bi loc mat thi Claude se cap moi tool: tu choi chay thay vi mo rong quyen.
    if safe_tools(&agent.tools).len() != agent.tools.len() {
        return Err(format!("{agent_id}: tools khong hop le ({})", agent.tools.join(", ")));
    }
    // Lead goi subagent nen phien can du tool cua ca phong; agent khac chi can tool cua minh.
    let mut tools: Vec<&str> = vec![];
    let pool: Vec<&AgentDef> = if agent_id == d.config.lead { d.agent_defs.iter().collect() } else { vec![agent] };
    for a in pool.iter().filter(|a| a.id == agent_id || agent_status(&d.config, &a.id) != "retired") {
        for t in safe_tools(&a.tools) {
            if !tools.contains(&t) {
                tools.push(t);
            }
        }
    }
    let mut args = format!(
        "--plugin-dir {} --agent {} --permission-mode auto --strict-mcp-config",
        ps_quote(&plugin_dir.to_string_lossy()),
        ps_quote(&format!("{PLUGIN}:{agent_id}"))
    );
    if !tools.is_empty() {
        args += &format!(" --tools {}", tools.iter().map(|t| ps_quote(t)).collect::<Vec<_>>().join(" "));
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
/// `model` khac model cua Agent (router / sep chon) -> plugin rieng chi doi model cua Agent chinh, subagent giu nguyen.
pub fn prepare_launch(d: &Department, agent: &str, notes_root: Option<&str>, model: &str) -> Result<LaunchSpec, String> {
    let mut d = d.clone();
    let mut dir = std::env::temp_dir().join(format!("claudedeck-plugin-{}", d.config.id));
    if let Some(a) = d.agent_defs.iter_mut().find(|a| a.id == agent && valid_model(model) && a.model.as_deref() != Some(model)) {
        a.model = Some(model.into());
        dir = std::env::temp_dir().join(format!("claudedeck-plugin-{}-{agent}-{model}", d.config.id));
    }
    let d = &d;
    write_plugin(d, &dir).map_err(err)?;
    if let Some(n) = notes_root.filter(|n| !n.is_empty()) {
        std::fs::create_dir_all(Path::new(n).join("ClaudeDeck")).map_err(err)?;
    }
    launch_spec(d, agent, &dir, notes_root)
}

#[tauri::command]
pub fn office_launch(root: String, dept: String, agent: String, notes_root: Option<String>) -> Result<LaunchSpec, String> {
    let d = load_office(Path::new(&root)).into_iter().find(|d| d.config.id == dept).ok_or(format!("khong co phong {dept}"))?;
    prepare_launch(&d, &agent, notes_root.as_deref(), "")
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
    fn write_plugin_skips_retired() {
        let root = tmp("retired");
        dept(&root, &["ok", "qa"], &[("ok", OK), ("qa", OK)]);
        let p = root.join("departments/t/department.json");
        let mut v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        v["status"] = serde_json::json!({ "qa": "retired" });
        fs::write(&p, v.to_string()).unwrap();
        let d = &load_office(&root)[0];
        assert_eq!(agent_status(&d.config, "qa"), "retired");
        assert_eq!(agent_status(&d.config, "ok"), "active");
        let dir = root.join("plugin");
        write_plugin(d, &dir).unwrap();
        assert!(dir.join("agents/ok.md").exists() && !dir.join("agents/qa.md").exists());
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
        let id = office_create_department(r.clone(), "Content Media!".into(), "🎬".into(), "Lam video".into(), None).unwrap();
        assert_eq!(id, "content-media");
        let d = load_office(&root).into_iter().find(|d| d.config.id == id).unwrap();
        assert_eq!((d.config.name.as_str(), d.config.icon.as_str(), d.config.mission.as_str()), ("Content Media!", "🎬", "Lam video"));
        assert!(d.errors.is_empty());
        assert_eq!(d.room["workspace"]["w"], 3);
        assert!(office_create_department(r.clone(), "content media".into(), "".into(), "".into(), None).is_err()); // trung
        assert!(office_create_department(r, "!!!".into(), "".into(), "".into(), None).is_err()); // ten rong sau khi lam sach
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn router_picks_first_match_and_plugin_carries_model() {
        let mut c = DeptConfig::default();
        let rule = |agent: &str, k: &[&str], m: &str| Route { agent: agent.into(), keywords: k.iter().map(|s| s.to_string()).collect(), model: m.into() };
        c.router = vec![rule("qa", &["typo"], "opus"), rule("", &["Typo", "README"], "haiku"), rule("", &["x"], "Bad Model;")];
        assert_eq!(route(&c, "dev", "Sua TYPO trong readme"), "haiku");
        assert_eq!(route(&c, "qa", "sua typo"), "opus");
        assert_eq!(route(&c, "dev", "lam tinh nang moi"), "");
        assert_eq!(route(&c, "dev", "x"), ""); // model la bi bo qua
        let root = tmp("router");
        seed_office(&root).unwrap();
        let d = load_office(&root).into_iter().find(|d| d.config.id == "software-dev").unwrap();
        let s = prepare_launch(&d, "dev", None, "haiku").unwrap();
        let dir = std::env::temp_dir().join("claudedeck-plugin-software-dev-dev-haiku");
        assert!(s.args.contains("claudedeck-plugin-software-dev-dev-haiku"), "{}", s.args);
        assert!(fs::read_to_string(dir.join("agents/dev.md")).unwrap().contains("model: haiku"));
        assert!(fs::read_to_string(dir.join("agents/qa.md")).unwrap().contains("model: sonnet")); // subagent giu model
        assert!(!prepare_launch(&d, "dev", None, "").unwrap().args.contains("-dev-")); // khong chon -> plugin chung
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn create_department_from_seeded_templates() {
        let root = tmp("tpl");
        let r = root.to_string_lossy().to_string();
        seed_office(&root).unwrap();
        let ts = office_templates(r.clone());
        assert_eq!(ts.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), vec!["media-content", "software-dev"]);
        assert_eq!(ts[0].agents.len(), 5);
        let id = office_create_department(r.clone(), "Video Ngan".into(), "🎬".into(), "".into(), Some("media-content".into())).unwrap();
        let d = load_office(&root).into_iter().find(|d| d.config.id == id).unwrap();
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        assert_eq!((d.config.lead.as_str(), d.agent_defs.len()), ("strategist", 5));
        assert_eq!(agent_status(&d.config, "analyst"), "active");
        assert_eq!(d.room["seats"]["analyst"]["x"], 8);
        assert!(d.agent_defs.iter().all(|a| safe_tools(&a.tools).len() == a.tools.len() && !a.tools.is_empty()));
        // Template sai / thieu agent -> loi, khong de lai phong do dang.
        assert!(office_create_department(r.clone(), "X".into(), "".into(), "".into(), Some("../hr".into())).is_err());
        fs::remove_file(root.join("templates/departments/software-dev/agents/qa.md")).unwrap();
        assert!(office_create_department(r, "Y".into(), "".into(), "".into(), Some("software-dev".into())).unwrap_err().contains("qa.md"));
        assert!(!root.join("departments/y").exists());
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
    fn launch_spec_drops_unsafe_tools() {
        let (root, mut d) = software_dev("ls-unsafe");
        // Lead gom tool cua ca phong: tool la cua agent khac bi bo, chi giu tool an toan (trong nhay).
        for a in d.agent_defs.iter_mut() {
            a.tools = if a.id == "qa" { ["Read", "Bash(npm test:*)", "x; Remove-Item -Recurse C:\\", "a`b", "q'r"].map(String::from).to_vec() } else { vec!["Read".into()] };
        }
        let s = launch_spec(&d, "tech-lead", Path::new("a"), None).unwrap();
        assert!(s.args.ends_with("--tools 'Read' 'Bash(npm test:*)'"), "{}", s.args);
        assert_eq!(safe_tools(&["Read & calc".to_string(), "Edit".into(), "50%".into()]), vec!["Edit"]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn launch_spec_refuses_filtered_tools_and_retired() {
        let (root, mut d) = software_dev("ls-refuse");
        d.agent_defs.iter_mut().find(|a| a.id == "qa").unwrap().tools = vec!["Read; Remove-Item x".into()];
        assert!(launch_spec(&d, "qa", Path::new("a"), None).unwrap_err().contains("tools"));
        d.config.status.insert("dev".into(), "retired".into());
        assert!(launch_spec(&d, "dev", Path::new("a"), None).unwrap_err().contains("nghi"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn lead_cannot_retire() {
        let root = tmp("lead-retire");
        seed_office(&root).unwrap();
        let r = root.to_string_lossy().to_string();
        assert!(office_set_agent_status(r.clone(), "software-dev".into(), "tech-lead".into(), "retired".into()).unwrap_err().contains("lead"));
        assert!(office_set_agent_status(r, "hr".into(), "recruiter".into(), "retired".into()).is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn launch_spec_unknown_agent_errors() {
        let (root, d) = software_dev("ls-ghost");
        assert!(launch_spec(&d, "ghost", Path::new("a.json"), None).is_err());
        fs::remove_dir_all(&root).unwrap();
    }
}
