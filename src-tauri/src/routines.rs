use crate::office::{agent_status, load_office};
use crate::tasks::{task_create, Task};
use chrono::{DateTime, Datelike, Local, NaiveTime};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Routine = task giao tu dong theo gio cho mot Agent. Den gio thi tao task binh thuong (hang doi, worktree,
/// tran token, Cho sip duyet, Telegram dung chung). Luu o `departments/<dept>/routines.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Routine {
    #[serde(default)]
    pub id: String,
    pub dept: String,
    pub agent: String,
    pub title: String,
    #[serde(default)]
    pub prompt: String,
    /// "HH:MM" hang ngay.
    pub at: String,
    /// 1 = thu Hai ... 7 = Chu nhat; rong = moi ngay.
    #[serde(default)]
    pub days: Vec<u32>,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Lan giao gan nhat (millis) va task da giao.
    #[serde(default)]
    pub last: i64,
    #[serde(default)]
    pub task: String,
}

fn yes() -> bool {
    true
}

/// Doc + ghi routines.json khong chen nhau (tick 30s va nut tren UI).
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn file(root: &Path, dept: &str) -> PathBuf {
    root.join("departments").join(dept).join("routines.json")
}

fn load(root: &Path, dept: &str) -> Result<Vec<Routine>, String> {
    match std::fs::read_to_string(file(root, dept)) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| format!("{dept}/routines.json: {e}")),
        Err(_) => Ok(vec![]),
    }
}

fn save(root: &Path, dept: &str, rs: &[Routine]) -> Result<(), String> {
    std::fs::write(file(root, dept), serde_json::to_string_pretty(rs).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn edit<R>(root: &Path, dept: &str, f: impl FnOnce(&mut Vec<Routine>) -> Result<R, String>) -> Result<R, String> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut rs = load(root, dept)?;
    let r = f(&mut rs)?;
    save(root, dept, &rs)?;
    Ok(r)
}

/// Den gio hom nay ma chua giao ke tu moc do. Mo app tre hon gio hen van giao bu mot lan trong ngay.
pub fn due(r: &Routine, now: DateTime<Local>) -> bool {
    let Ok(at) = NaiveTime::parse_from_str(&r.at, "%H:%M") else { return false };
    let Some(slot) = now.date_naive().and_time(at).and_local_timezone(Local).earliest() else { return false };
    r.enabled && (r.days.is_empty() || r.days.contains(&now.weekday().number_from_monday())) && now >= slot && r.last < slot.timestamp_millis()
}

fn give(root: &str, r: &Routine, notes: Option<String>) -> Result<Task, String> {
    let body = format!("{}\n\n(Routine `{}` · {} {})", r.prompt, r.id, r.at, if r.days.is_empty() { "moi ngay".into() } else { format!("thu {:?}", r.days) });
    task_create(root.into(), r.dept.clone(), r.agent.clone(), format!("⏰ {}", r.title), body, notes)
}

#[tauri::command]
pub fn routine_list(root: String) -> Result<Vec<Routine>, String> {
    let rp = Path::new(&root);
    let mut out = vec![];
    for d in load_office(rp) {
        out.extend(load(rp, &d.config.id)?);
    }
    Ok(out)
}

/// Them (id rong) hoac sua routine.
#[tauri::command]
pub fn routine_save(root: String, mut routine: Routine) -> Result<Routine, String> {
    let rp = Path::new(&root);
    let d = load_office(rp).into_iter().find(|d| d.config.id == routine.dept).ok_or(format!("khong co phong {}", routine.dept))?;
    if !d.agent_defs.iter().any(|a| a.id == routine.agent) || agent_status(&d.config, &routine.agent) == "retired" {
        return Err(format!("khong co agent {} (hoac da nghi)", routine.agent));
    }
    if routine.title.trim().is_empty() {
        return Err("routine can tieu de".into());
    }
    if NaiveTime::parse_from_str(&routine.at, "%H:%M").is_err() {
        return Err(format!("gio '{}' phai dang HH:MM", routine.at));
    }
    if routine.days.iter().any(|d| !(1..=7).contains(d)) {
        return Err("ngay trong tuan: 1 (thu Hai) .. 7 (Chu nhat)".into());
    }
    edit(rp, &routine.dept.clone(), |rs| {
        match rs.iter_mut().find(|r| !routine.id.is_empty() && r.id == routine.id) {
            // Sua: giu lich su giao (last/task) cua ban dang luu.
            Some(r) => {
                (routine.last, routine.task) = (r.last, r.task.clone());
                *r = routine.clone();
            }
            None => {
                let stamp = Local::now().format("r%y%m%d-%H%M%S").to_string();
                let (mut id, mut n) = (stamp.clone(), 1);
                while rs.iter().any(|r| r.id == id) {
                    n += 1;
                    id = format!("{stamp}-{n}");
                }
                routine.id = id;
                // Tao luc 9h cho routine 8h: khong giao bu ngay hom nay.
                routine.last = Local::now().timestamp_millis();
                rs.push(routine.clone());
            }
        }
        Ok(routine)
    })
}

#[tauri::command]
pub fn routine_delete(root: String, dept: String, id: String) -> Result<(), String> {
    edit(Path::new(&root), &dept, |rs| Ok(rs.retain(|r| r.id != id)))
}

/// "Chay ngay": giao task luon, khong doi gio.
#[tauri::command]
pub fn routine_run(root: String, dept: String, id: String, notes_root: Option<String>) -> Result<Task, String> {
    edit(Path::new(&root), &dept, |rs| {
        let r = rs.iter_mut().find(|r| r.id == id).ok_or(format!("khong co routine {id}"))?;
        let t = give(&root, r, notes_root)?;
        (r.last, r.task) = (Local::now().timestamp_millis(), t.id.clone());
        Ok(t)
    })
}

/// Goi moi 30s tu app (chi chay khi app mo, ke ca an duoi khay). Tra ve task vua giao.
#[tauri::command]
pub fn routine_tick(root: String, notes_root: Option<String>) -> Vec<Task> {
    let rp = Path::new(&root);
    let now = Local::now();
    let open: Vec<String> = crate::tasks::load_tasks(rp).unwrap_or_default().into_iter().filter(|t| t.status != "done").map(|t| t.id).collect();
    let mut out = vec![];
    for d in load_office(rp) {
        let _ = edit(rp, &d.config.id, |rs| {
            // Lan truoc chua xong thi chua giao them (khong chat dong task); xong roi se giao bu trong ngay.
            for r in rs.iter_mut().filter(|r| due(r, now) && !open.contains(&r.task)) {
                r.last = now.timestamp_millis();
                match give(&root, r, notes_root.clone()) {
                    Ok(t) => {
                        r.task = t.id.clone();
                        out.push(t);
                    }
                    // Agent nghi / phong chua co du an: bao mot lan, khong thu lai den hom sau.
                    Err(e) => crate::telegram::tg_say(format!("⚠️ Routine <b>{}</b> không giao được: {}", crate::telegram::escape(&r.title), crate::telegram::escape(&e))),
                }
            }
            Ok(())
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::fs;

    fn at(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap()
    }

    fn r(at: &str, days: Vec<u32>) -> Routine {
        Routine { dept: "software-dev".into(), agent: "dev".into(), title: "bao cao".into(), at: at.into(), days, enabled: true, ..Default::default() }
    }

    #[test]
    fn due_rules() {
        let mon = at(2026, 10, 26, 9, 0); // thu Hai
        assert!(due(&r("08:30", vec![]), mon));
        assert!(!due(&r("09:30", vec![]), mon)); // chua toi gio
        assert!(!due(&r("08:30", vec![2, 3]), mon)); // khong phai ngay chay
        assert!(due(&r("08:30", vec![1]), mon));
        let mut done = r("08:30", vec![]);
        done.last = at(2026, 10, 26, 8, 31).timestamp_millis();
        assert!(!due(&done, mon)); // da giao hom nay
        done.last = at(2026, 10, 25, 8, 31).timestamp_millis();
        assert!(due(&done, mon)); // hom qua -> hom nay giao tiep
        assert!(!due(&Routine { enabled: false, ..r("08:30", vec![]) }, mon));
        assert!(!due(&r("8h", vec![]), mon));
    }

    #[test]
    fn save_validates_and_tick_gives_once() {
        let root = std::env::temp_dir().join(format!("cd-routine-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        crate::office::seed_office(&root).unwrap();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap(); // khong phai git -> task lam thang trong ws
        crate::office::office_set_workspace(root.to_string_lossy().into(), "software-dev".into(), ws.to_string_lossy().into()).unwrap();
        let rt = root.to_string_lossy().to_string();
        assert!(routine_save(rt.clone(), Routine { agent: "khong-co".into(), ..r("08:00", vec![]) }).is_err());
        assert!(routine_save(rt.clone(), r("25:00", vec![])).is_err());
        assert!(routine_save(rt.clone(), r("08:00", vec![8])).is_err());
        let saved = routine_save(rt.clone(), r("00:00", vec![])).unwrap();
        assert!(!saved.id.is_empty());
        assert!(routine_tick(rt.clone(), None).is_empty()); // vua tao: khong giao bu hom nay
        // Gia nhu lan cuoi la hom qua -> tick giao 1 task, tick tiep khong giao them.
        edit(&root, "software-dev", |rs| Ok(rs[0].last = 0)).unwrap();
        let ts = routine_tick(rt.clone(), None);
        assert_eq!(ts.len(), 1);
        assert!(ts[0].title.starts_with("⏰"));
        assert!(routine_tick(rt.clone(), None).is_empty());
        // Task cu chua xong -> khong chat them du da toi lich.
        edit(&root, "software-dev", |rs| Ok(rs[0].last = 0)).unwrap();
        assert!(routine_tick(rt.clone(), None).is_empty());
        let t = routine_run(rt.clone(), "software-dev".into(), saved.id.clone(), None).unwrap(); // chay ngay: van giao
        assert_eq!(routine_list(rt.clone()).unwrap()[0].task, t.id);
        routine_delete(rt.clone(), "software-dev".into(), saved.id).unwrap();
        assert!(routine_list(rt).unwrap().is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
