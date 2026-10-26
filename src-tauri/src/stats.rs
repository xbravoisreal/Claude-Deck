use crate::history::project_dir;
use crate::tasks::{load_tasks, Task};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

/// Token cong don (moi message.id tinh mot lan, stream ghi nhieu dong cung id).
#[derive(Serialize, Default, Clone, Debug, PartialEq)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_create: u64,
    /// So luot goi API.
    pub turns: u64,
}

impl Usage {
    fn add(&mut self, o: &Usage) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_create += o.cache_create;
        self.turns += o.turns;
    }
}

/// Theo model ("claude-sonnet-..."): mot transcript co the doi model giua chung (/model).
pub fn transcript_usage(jsonl: &Path) -> BTreeMap<String, Usage> {
    let mut out: BTreeMap<String, Usage> = BTreeMap::new();
    let Ok(s) = std::fs::read_to_string(jsonl) else { return out };
    let mut seen = HashSet::new();
    for line in s.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let m = &v["message"];
        let u = &m["usage"];
        let id = m["id"].as_str().unwrap_or("");
        if v["type"] != "assistant" || !u.is_object() || (!id.is_empty() && !seen.insert(id.to_string())) {
            continue;
        }
        let g = |k: &str| u[k].as_u64().unwrap_or(0);
        let model = m["model"].as_str().filter(|s| !s.is_empty() && *s != "<synthetic>").unwrap_or("khac");
        out.entry(model.into()).or_default().add(&Usage {
            input: g("input_tokens"),
            output: g("output_tokens"),
            cache_read: g("cache_read_input_tokens"),
            cache_create: g("cache_creation_input_tokens"),
            turns: 1,
        });
    }
    out
}

/// Usage cua task chia theo Agent: phien chinh tinh cho Agent cua task, subagent (`<session>/subagents/*.jsonl`)
/// tinh cho Agent ghi trong meta.json (`agentType: office:<id>`).
pub fn task_usage(t: &Task) -> Vec<(String, BTreeMap<String, Usage>)> {
    let Some(p) = project_dir(&t.cwd) else { return vec![] };
    let mut out = vec![(t.agent.clone(), transcript_usage(&p.join(format!("{}.jsonl", t.session))))];
    let subs = p.join(&t.session).join("subagents");
    for e in std::fs::read_dir(&subs).into_iter().flatten().flatten() {
        let f = e.path();
        if f.extension().is_none_or(|x| x != "jsonl") {
            continue;
        }
        let meta: serde_json::Value = std::fs::read_to_string(f.with_extension("meta.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let agent = meta["agentType"].as_str().and_then(|a| a.strip_prefix("office:")).unwrap_or(&t.agent).to_string();
        out.push((agent, transcript_usage(&f)));
    }
    out
}

/// Mot dong thong ke: (phong, Agent). Task dem cho Agent duoc giao; token gom ca phan lam subagent cho Agent khac.
#[derive(Serialize, Default, Debug)]
pub struct AgentStats {
    pub dept: String,
    pub agent: String,
    pub tasks: u64,
    pub approved: u64,
    pub cancelled: u64,
    pub open: u64,
    /// Tong so lan sep bam Lam lai.
    pub reworks: u64,
    pub total: Usage,
    pub by_model: BTreeMap<String, Usage>,
}

/// So lan Lam lai = so muc "## Lam lai (" trong brief.md (tasks::rework ghi them moi lan).
fn reworks(t: &Task) -> u64 {
    std::fs::read_to_string(Path::new(&t.dir).join("brief.md")).map_or(0, |s| s.matches("\n## Lam lai (").count() as u64)
}

pub fn compute(tasks: &[Task], usage: impl Fn(&Task) -> Vec<(String, BTreeMap<String, Usage>)>) -> Vec<AgentStats> {
    let mut rows: BTreeMap<(String, String), AgentStats> = BTreeMap::new();
    fn row<'a>(rows: &'a mut BTreeMap<(String, String), AgentStats>, dept: &str, agent: &str) -> &'a mut AgentStats {
        rows.entry((dept.into(), agent.into())).or_insert_with(|| AgentStats { dept: dept.into(), agent: agent.into(), ..Default::default() })
    }
    for t in tasks {
        let r = row(&mut rows, &t.dept, &t.agent);
        r.tasks += 1;
        match (t.status.as_str(), t.reason == "da huy") {
            ("done", true) => r.cancelled += 1,
            ("done", false) => r.approved += 1,
            _ => r.open += 1,
        }
        r.reworks += reworks(t);
        for (agent, by_model) in usage(t) {
            let r = row(&mut rows, &t.dept, &agent);
            for (m, u) in by_model {
                r.total.add(&u);
                r.by_model.entry(m).or_default().add(&u);
            }
        }
    }
    rows.into_values().collect()
}

/// ponytail: doc lai moi transcript moi lan mo bang thong ke; cache theo (file, mtime) neu cham.
#[tauri::command]
pub async fn office_stats(root: String) -> Result<Vec<AgentStats>, String> {
    tauri::async_runtime::spawn_blocking(move || Ok(compute(&load_tasks(Path::new(&root))?, task_usage))).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Doc office that cua may (chi doc): cargo test --lib stats_real_office -- --ignored --nocapture
    #[test]
    #[ignore]
    fn stats_real_office() {
        let root = dirs::config_dir().unwrap().join("claudedeck").join("office");
        let rows = compute(&load_tasks(&root).unwrap(), task_usage);
        println!("{}", crate::hr::review_table(&root, &rows));
    }

    #[test]
    fn transcript_usage_by_model_dedups_message_id() {
        let dir = std::env::temp_dir().join(format!("cd-stats-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let line = |id: &str, model: &str, out: u64| {
            serde_json::json!({"type":"assistant","message":{"id":id,"model":model,"usage":{"input_tokens":1,"output_tokens":out,"cache_read_input_tokens":100,"cache_creation_input_tokens":10}}}).to_string()
        };
        let f = dir.join("s.jsonl");
        let lines = [line("a", "claude-sonnet-5-5", 5), line("a", "claude-sonnet-5-5", 5), line("b", "claude-haiku-5-5", 7), "{\"type\":\"user\"}".into(), "hong".into()];
        fs::write(&f, lines.join("\n")).unwrap();
        let u = transcript_usage(&f);
        assert_eq!(u["claude-sonnet-5-5"], Usage { input: 1, output: 5, cache_read: 100, cache_create: 10, turns: 1 });
        assert_eq!(u["claude-haiku-5-5"].output, 7);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn compute_counts_outcomes_and_credits_subagents() {
        let dir = std::env::temp_dir().join(format!("cd-stats-c-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("brief.md"), "# x\n\n## Lam lai (2026-10-26 10:00)\n\nsua\n\n## Lam lai (2026-10-26 11:00)\n\nnua\n").unwrap();
        let t = |id: &str, status: &str, reason: &str| Task {
            id: id.into(), dept: "sd".into(), agent: "lead".into(), title: "".into(), status: status.into(), reason: reason.into(),
            session: "".into(), dir: dir.to_string_lossy().into(), cwd: "".into(), created: 0, started: true, cap_base: 0, finished: 0,
            tg_msg: 0, ask: "".into(), summary: "".into(), model: "".into(),
        };
        let ts = [t("1", "done", ""), t("2", "done", "da huy"), t("3", "review", "")];
        let u = |n: u64| BTreeMap::from([("m".to_string(), Usage { output: n, turns: 1, ..Default::default() })]);
        let rows = compute(&ts, |_| vec![("lead".into(), u(10)), ("dev".into(), u(3))]);
        assert_eq!(rows.len(), 2);
        let dev = rows.iter().find(|r| r.agent == "dev").unwrap();
        assert_eq!((dev.tasks, dev.total.output, dev.by_model["m"].turns), (0, 9, 3)); // chi lam subagent
        let lead = rows.iter().find(|r| r.agent == "lead").unwrap();
        assert_eq!((lead.tasks, lead.approved, lead.cancelled, lead.open, lead.reworks, lead.total.output), (3, 1, 1, 1, 6, 30));
        let _ = fs::remove_dir_all(&dir);
    }
}
