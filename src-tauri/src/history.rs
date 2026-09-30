use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct Entry {
    id: String,
    title: String,
    modified: i64,
}

/// Thu muc transcript cua claude: ~/.claude/projects/<cwd, ky tu khong phai chu/so thanh '-'>.
fn project_dir(cwd: &str) -> Option<PathBuf> {
    let name: String = cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    Some(dirs::home_dir()?.join(".claude").join("projects").join(name))
}

/// Tieu de = tin nhan dau tien cua user trong transcript.
fn title(path: &Path) -> Option<String> {
    let f = std::fs::File::open(path).ok()?;
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        if v["type"] != "user" {
            continue;
        }
        let c = &v["message"]["content"];
        let text = c.as_str().map(str::to_string).or_else(|| {
            c.as_array()?.iter().find_map(|x| x["text"].as_str().map(str::to_string))
        });
        if let Some(t) = text.filter(|t| !t.starts_with('<')) {
            return Some(t.chars().take(120).collect());
        }
    }
    None
}

/// Danh sach session claude cua thu muc, moi nhat truoc.
#[tauri::command]
pub fn list_sessions(cwd: String) -> Vec<Entry> {
    let Some(dir) = project_dir(&cwd) else { return vec![] };
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<Entry> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|p| {
            let modified = p.metadata().ok()?.modified().ok()?;
            Some(Entry {
                id: p.file_stem()?.to_string_lossy().into_owned(),
                title: title(&p)?,
                modified: chrono::DateTime::<chrono::Utc>::from(modified).timestamp_millis(),
            })
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}
