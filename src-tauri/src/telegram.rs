use crate::office::load_office;
use crate::tasks::{self, Task};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Hanh dong sep bam/gui tren Telegram.
#[derive(Debug, PartialEq)]
pub enum Action {
    Approve(String),
    Rework(String),
    ReworkNote(String, String),
    Allow(String),
    Deny(String),
    /// Tra loi (reply) vao tin co message_id nay.
    Reply(i64, String),
    /// Tin thuong: viec moi, cho chon agent.
    NewTask(String),
    /// (phong, agent, noi dung) sau khi bam nut chon agent.
    Assign(String, String, String),
    /// /duan: chon phong de doi du an.
    Projects,
    /// Da chon phong, cho chon/go ten du an.
    PickDept(String),
    /// (phong, ten du an trong thu muc goc) - chua co thi tao moi.
    SetProject(String, String),
    Ignore,
}

/// Trang thai hoi thoai: dang cho nhan xet lam lai cho task nao; viec moi dang cho chon agent.
#[derive(Default)]
pub struct Chat {
    pub pending_rework: Option<String>,
    pub pending_task: Option<String>,
    /// Phong dang cho ten du an.
    pub pending_project: Option<String>,
}

/// Thoat ky tu cho parse_mode HTML cua Telegram.
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Cat chuoi toi da n ky tu (them … neu cat). Tin Telegram chi gui tom tat, khong gui code.
fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.into()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

/// Doc mot update. Chi nhan tu dung chat cua sep; nguoi khac -> Ignore va khong dong vao trang thai.
/// Tra ve kem callback_query id (de answerCallbackQuery).
pub fn parse_update(u: &Value, chat_id: i64, st: &mut Chat) -> (Action, Option<String>) {
    // Chat rieng (id > 0): nguoi gui cung phai la sep. Nhom (id < 0): moi thanh vien nhom deu duyet duoc.
    let sender_ok = |x: &Value| chat_id < 0 || x["from"]["id"].as_i64().is_none_or(|f| f == chat_id);
    if let Some(q) = u.get("callback_query") {
        if q["message"]["chat"]["id"].as_i64() != Some(chat_id) || !sender_ok(q) {
            return (Action::Ignore, None);
        }
        let qid = q["id"].as_str().map(String::from);
        let Some((kind, id)) = q["data"].as_str().and_then(|d| d.split_once(':')) else { return (Action::Ignore, qid) };
        let id = id.to_string();
        let a = match kind {
            "ok" => Action::Approve(id),
            "re" => {
                st.pending_project = None;
                st.pending_rework = Some(id.clone());
                Action::Rework(id)
            }
            "al" => Action::Allow(id),
            "de" => Action::Deny(id),
            "pd" => {
                (st.pending_task, st.pending_rework, st.pending_project) = (None, None, Some(id.clone()));
                Action::PickDept(id)
            }
            "pj" => match id.split_once('/') {
                Some((dept, name)) => {
                    st.pending_project = None;
                    Action::SetProject(dept.into(), name.into())
                }
                None => Action::Ignore,
            },
            "as" => match (id.split_once('/'), st.pending_task.take()) {
                (Some((dept, agent)), Some(text)) => Action::Assign(dept.into(), agent.into(), text),
                _ => Action::Ignore,
            },
            _ => Action::Ignore,
        };
        return (a, qid);
    }
    let m = &u["message"];
    if m["chat"]["id"].as_i64() != Some(chat_id) || !sender_ok(m) {
        return (Action::Ignore, None);
    }
    let text = m["text"].as_str().map(str::trim).unwrap_or("");
    if text == "/duan" || text.starts_with("/duan@") {
        return (Action::Projects, None);
    }
    if text.is_empty() || text.starts_with('/') {
        return (Action::Ignore, None);
    }
    let a = if let Some(to) = m["reply_to_message"]["message_id"].as_i64() {
        Action::Reply(to, text.into())
    } else if let Some(dept) = st.pending_project.take() {
        Action::SetProject(dept, text.into())
    } else if let Some(id) = st.pending_rework.take() {
        Action::ReworkNote(id, text.into())
    } else {
        st.pending_task = Some(text.into());
        Action::NewTask(text.into())
    };
    (a, None)
}

/// Thu muc du an moi/cu nam canh du an hien tai cua phong (cung thu muc cha). Chi nhan ten, khong nhan duong dan.
pub fn project_path(workspace: &str, name: &str) -> Result<PathBuf, String> {
    let parent = Path::new(workspace).parent().filter(|p| !p.as_os_str().is_empty()).ok_or("phòng chưa có dự án: chọn thư mục trên app trước")?;
    let name = name.trim();
    let lower = name.to_lowercase();
    let reserved = ["con", "prn", "aux", "nul"].contains(&lower.as_str())
        || (lower.len() == 4 && (lower.starts_with("com") || lower.starts_with("lpt")) && lower.ends_with(|c: char| c.is_ascii_digit()));
    if name.is_empty() || name.starts_with('.') || name.ends_with('.') || reserved || name.chars().any(|c| c.is_control() || r#"\/:*?"<>|"#.contains(c)) {
        return Err(format!("tên dự án không hợp lệ: {name}"));
    }
    Ok(parent.join(name))
}

/// Thu muc con (du an) trong thu muc goc, moi sua truoc. Bo thu muc an.
fn projects(parent: &Path) -> Vec<String> {
    let mut v: Vec<_> = std::fs::read_dir(parent)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| Some((e.metadata().and_then(|m| m.modified()).ok()?, e.file_name().into_string().ok()?)))
        .filter(|(_, n)| !n.starts_with('.') && !n.starts_with('$'))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().map(|(_, n)| n).collect()
}

fn buttons(rows: &[(&str, String)]) -> Value {
    json!({ "inline_keyboard": [rows.iter().map(|(t, d)| json!({ "text": t, "callback_data": d })).collect::<Vec<_>>()] })
}

pub fn review_message(agent: &str, dept: &str, t: &Task) -> (String, Value) {
    let mut text = format!("🟠 <b>Chờ síp duyệt</b> · <b>{}</b> · {}\nTask: {}", escape(agent), escape(dept), escape(&clip(&t.title, 120)));
    if !t.summary.is_empty() {
        text += &format!("\nTóm tắt: {}", escape(&clip(&t.summary, 300)));
    }
    (text, buttons(&[("✅ Duyệt", format!("ok:{}", t.id)), ("↩ Làm lại", format!("re:{}", t.id))]))
}

/// Cho anh: chi xin quyen moi co nut Cho phep / Tu choi.
pub fn waiting_message(agent: &str, dept: &str, t: &Task, permission: bool) -> (String, Value) {
    let mut text = format!("🟡 <b>Chờ síp trả lời</b> · <b>{}</b> · {}\nTask: {}", escape(agent), escape(dept), escape(&clip(&t.title, 120)));
    if !t.reason.is_empty() {
        text += &format!("\nLý do: {}", escape(&clip(&t.reason, 200)));
    }
    if !t.ask.is_empty() {
        text += &format!("\n💬 {}", escape(&clip(&t.ask, 800)));
    }
    if !permission {
        text += "
↩ Trả lời (reply) tin này để nhắn agent.";
    }
    let kb = if permission { buttons(&[("✅ Cho phép", format!("al:{}", t.id)), ("⛔ Từ chối", format!("de:{}", t.id))]) } else { Value::Null };
    (text, kb)
}

// ---------------- Chay that: Credential Manager, HTTP, vong long-polling ----------------

const SERVICE: &str = "claudedeck-telegram";
const USER: &str = "bot-token";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, USER).map_err(|e| e.to_string())
}

/// Token chi doc trong Rust, khong bao gio tra len frontend.
fn token() -> Option<String> {
    entry().ok()?.get_password().ok().filter(|t| !t.is_empty())
}

#[derive(Clone)]
struct Cfg {
    token: String,
    chat: i64,
    root: String,
    notes: String,
}

static CFG: Mutex<Option<Cfg>> = Mutex::new(None);
/// Moi lan cau hinh lai tang GEN; vong polling cu thay GEN doi thi tu dung.
static GEN: AtomicU64 = AtomicU64::new(0);
static STATUS: Mutex<String> = Mutex::new(String::new());

fn set_status(s: impl Into<String>) {
    *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = s.into();
}

fn cfg() -> Result<Cfg, String> {
    CFG.lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or_else(|| "chua cau hinh Telegram".into())
}

fn client() -> &'static reqwest::blocking::Client {
    static C: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    C.get_or_init(|| reqwest::blocking::Client::builder().timeout(Duration::from_secs(45)).build().expect("http client"))
}

/// Goi Bot API. Loi khong kem URL (URL co token).
fn api(token: &str, method: &str, body: Value) -> Result<Value, String> {
    let e = |e: reqwest::Error| e.without_url().to_string();
    let r: Value = client().post(format!("https://api.telegram.org/bot{token}/{method}")).json(&body).send().map_err(e)?.json().map_err(e)?;
    if r["ok"].as_bool() == Some(true) {
        Ok(r["result"].clone())
    } else {
        Err(r["description"].as_str().unwrap_or("loi Telegram").into())
    }
}

fn send(c: &Cfg, text: &str, markup: &Value) -> Result<i64, String> {
    let mut body = json!({ "chat_id": c.chat, "text": text, "parse_mode": "HTML" });
    if !markup.is_null() {
        body["reply_markup"] = markup.clone();
    }
    Ok(api(&c.token, "sendMessage", body)?["message_id"].as_i64().unwrap_or(0))
}

/// Sua noi dung tin (khong gui reply_markup = bo nut).
fn edit(c: &Cfg, msg: i64, text: &str) {
    if msg != 0 {
        let _ = api(&c.token, "editMessageText", json!({ "chat_id": c.chat, "message_id": msg, "text": text, "parse_mode": "HTML" }));
    }
}

fn answer(c: &Cfg, qid: &Option<String>, text: &str) {
    if let Some(q) = qid {
        let _ = api(&c.token, "answerCallbackQuery", json!({ "callback_query_id": q, "text": text }));
    }
}

/// (ten Agent, ten phong) de ghi trong tin.
fn names(root: &str, t: &Task) -> (String, String) {
    let d = load_office(Path::new(root)).into_iter().find(|d| d.config.id == t.dept);
    let agent = d.as_ref().and_then(|d| d.agent_defs.iter().find(|a| a.id == t.agent)).map_or(t.agent.clone(), |a| a.name.clone());
    (agent, d.map_or(t.dept.clone(), |d| d.config.name))
}

fn find(root: &str, id: &str) -> Option<Task> {
    tasks::load_tasks(Path::new(root)).ok()?.into_iter().find(|t| t.id == id)
}

/// Xu ly mot update da qua kiem tra chat id.
fn handle(app: &AppHandle, c: &Cfg, u: &Value, chat: &mut Chat) {
    let (mut action, qid) = parse_update(u, c.chat, chat);
    let waiting_by_msg = |to: i64| tasks::load_tasks(Path::new(&c.root)).ok()?.into_iter().find(|t| to != 0 && t.tg_msg == to && t.status == "waiting");
    // Reply khong vao tin cho tra loi nao ma dang cho nhan xet lam lai (vd reply vao cau hoi "Nhan xet...?") -> nhan xet.
    if let Action::Reply(to, text) = &action {
        if waiting_by_msg(*to).is_none() {
            if let Some(dept) = chat.pending_project.take() {
                action = Action::SetProject(dept, text.clone());
            } else if let Some(id) = chat.pending_rework.take() {
                action = Action::ReworkNote(id, text.clone());
            }
        }
    }
    let cb_msg = u["callback_query"]["message"]["message_id"].as_i64().unwrap_or(0);
    let changed = || {
        let _ = app.emit("office-tasks-changed", ());
    };
    match action {
        Action::Approve(id) => match tasks::update(&c.root, &id, tasks::approve) {
            Ok(t) => {
                answer(c, &qid, "Đã duyệt");
                edit(c, cb_msg, &format!("✅ Đã duyệt · <b>{}</b>", escape(&t.title)));
                let _ = tasks::update(&c.root, &id, |t| Ok(t.tg_msg = 0));
                changed();
            }
            Err(_) => answer(c, &qid, "Đã xử lý rồi"),
        },
        Action::Rework(id) => match find(&c.root, &id).filter(|t| t.status == "review") {
            Some(t) => {
                answer(c, &qid, "");
                let _ = send(c, &format!("↩ Nhận xét để làm lại <b>{}</b>? (trả lời tin này)", escape(&t.title)), &json!({ "force_reply": true }));
            }
            None => {
                chat.pending_rework = None;
                answer(c, &qid, "Đã xử lý rồi");
            }
        },
        Action::ReworkNote(id, note) => match tasks::update(&c.root, &id, |t| tasks::rework(t, &note)) {
            Ok(t) => {
                edit(c, t.tg_msg, &format!("↩ Làm lại · <b>{}</b>", escape(&t.title)));
                let _ = tasks::update(&c.root, &id, |t| Ok(t.tg_msg = 0));
                let _ = send(c, &format!("↩ Đã gửi làm lại: <b>{}</b>", escape(&t.title)), &Value::Null);
                changed();
            }
            Err(e) => {
                let _ = send(c, &format!("Không làm lại được: {}", escape(&e)), &Value::Null);
            }
        },
        // Nut chi con hieu luc khi dung tin dang treo cua lan xin quyen hien tai (tin cu / lan hoi khac -> bo qua).
        Action::Allow(id) | Action::Deny(id)
            if find(&c.root, &id).is_some_and(|t| t.status == "waiting" && t.reason.starts_with("xin quyen") && t.tg_msg == cb_msg) =>
        {
            let allow = u["callback_query"]["data"].as_str().is_some_and(|d| d.starts_with("al:"));
            let _ = tasks::update(&c.root, &id, |t| Ok(t.tg_msg = 0));
            let _ = app.emit("office-permission", json!({ "task": id, "allow": allow }));
            answer(c, &qid, if allow { "Đã cho phép" } else { "Đã từ chối" });
            edit(c, cb_msg, &format!("{} · task {}", if allow { "✅ Đã cho phép" } else { "⛔ Đã từ chối" }, escape(&id)));
        }
        Action::Allow(_) | Action::Deny(_) => answer(c, &qid, "Đã xử lý rồi"),
        Action::Reply(to, text) => match waiting_by_msg(to) {
            Some(t) if t.reason.starts_with("xin quyen") => {
                let _ = send(c, "Đang xin quyền: bấm nút ✅ Cho phép / ⛔ Từ chối.", &Value::Null);
            }
            Some(t) => {
                let _ = tasks::update(&c.root, &t.id, |t| Ok(t.tg_msg = 0));
                edit(c, to, &format!("💬 Đã trả lời · <b>{}</b>
{}", escape(&t.title), escape(&clip(&text, 300))));
                let _ = app.emit("office-reply", json!({ "task": t.id, "text": text }));
            }
            None => {
                let _ = send(c, "Tin này không còn chờ trả lời.", &Value::Null);
            }
        },
        Action::NewTask(text) => {
            // Moi agent cua phong da co thu muc du an thanh mot nut "as:<phong>/<agent>" (callback_data toi da 64 byte).
            let rows: Vec<Value> = load_office(Path::new(&c.root))
                .iter()
                .filter(|d| !d.config.workspace.is_empty())
                .flat_map(|d| d.agent_defs.iter().map(move |a| (format!("{} · {}", a.name, d.config.name), format!("as:{}/{}", d.config.id, a.id))))
                .filter(|(_, data)| data.len() <= 64)
                .map(|(label, data)| json!([{ "text": label, "callback_data": data }]))
                .collect();
            if rows.is_empty() {
                chat.pending_task = None;
                let _ = send(c, "Chưa có phòng nào có thư mục dự án để giao việc.", &Value::Null);
            } else {
                let _ = send(c, &format!("📌 Giao cho ai?
{}", escape(&clip(&text, 300))), &json!({ "inline_keyboard": rows }));
            }
        }
        Action::Projects => {
            let rows: Vec<Value> = load_office(Path::new(&c.root))
                .iter()
                .map(|d| {
                    let cur = Path::new(&d.config.workspace).file_name().map_or("chưa có".into(), |n| n.to_string_lossy().into_owned());
                    json!([{ "text": format!("{} {} · {cur}", d.config.icon, d.config.name), "callback_data": format!("pd:{}", d.config.id) }])
                })
                .collect();
            let _ = send(c, "📁 Đổi dự án cho phòng nào?", &json!({ "inline_keyboard": rows }));
        }
        Action::PickDept(dept) => {
            answer(c, &qid, "");
            let d = load_office(Path::new(&c.root)).into_iter().find(|d| d.config.id == dept && !d.config.workspace.is_empty());
            match d.as_ref().and_then(|d| Path::new(&d.config.workspace).parent().map(|p| (d, p))) {
                Some((d, parent)) => {
                    let cur = Path::new(&d.config.workspace).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    // ponytail: toi da 30 du an moi sua gan nhat; du an cu hon thi go ten.
                    let names: Vec<String> = projects(parent).into_iter().filter(|n| format!("pj:{dept}/{n}").len() <= 64).take(30).collect();
                    let rows: Vec<Value> = names
                        .chunks(2)
                        .map(|ch| Value::Array(ch.iter().map(|n| json!({ "text": if *n == cur { format!("✓ {n}") } else { n.clone() }, "callback_data": format!("pj:{dept}/{n}") })).collect()))
                        .collect();
                    let text = format!("📁 <b>{}</b> · gốc {}\nChọn dự án, hoặc gõ tên (chưa có thì tạo thư mục mới).", escape(&d.config.name), escape(&parent.to_string_lossy()));
                    let _ = send(c, &text, &json!({ "inline_keyboard": rows }));
                }
                None => {
                    chat.pending_project = None;
                    let _ = send(c, "Phòng chưa có dự án: chọn thư mục trên app trước.", &Value::Null);
                }
            }
        }
        Action::SetProject(dept, name) => {
            answer(c, &qid, "");
            let ws = load_office(Path::new(&c.root)).into_iter().find(|d| d.config.id == dept).map(|d| d.config.workspace).unwrap_or_default();
            let res = project_path(&ws, &name).and_then(|p| {
                let new = !p.is_dir();
                if new {
                    std::fs::create_dir(&p).map_err(|e| format!("không tạo được thư mục: {e}"))?;
                }
                crate::office::office_set_workspace(c.root.clone(), dept.clone(), p.to_string_lossy().into_owned())?;
                Ok((p, new))
            });
            let text = match res {
                Ok((p, new)) => {
                    let _ = app.emit("office-depts-changed", ());
                    format!("📁 Phòng giờ làm dự án <b>{}</b>{}\n{}", escape(name.trim()), if new { " (mới tạo)" } else { "" }, escape(&p.to_string_lossy()))
                }
                Err(e) => format!("Không đổi được dự án: {}", escape(&e)),
            };
            if cb_msg != 0 {
                edit(c, cb_msg, &text);
            } else {
                let _ = send(c, &text, &Value::Null);
            }
        }
        Action::Assign(dept, agent, text) => {
            let title = clip(text.lines().next().unwrap_or(""), 80);
            match tasks::task_create(c.root.clone(), dept, agent, title, text, Some(c.notes.clone())) {
                Ok(t) => {
                    answer(c, &qid, "Đã giao");
                    let (agent, dept) = names(&c.root, &t);
                    edit(c, cb_msg, &format!("📌 Đã giao cho <b>{}</b> · {}
Task: {}", escape(&agent), escape(&dept), escape(&t.title)));
                    changed();
                }
                Err(e) => {
                    answer(c, &qid, "");
                    edit(c, cb_msg, &format!("Không giao được: {}", escape(&e)));
                }
            }
        }
        Action::Ignore => answer(c, &qid, ""),
    }
}

/// Offset getUpdates dung chung moi vong (cau hinh lai khong doc lai update da xu ly).
static OFFSET: AtomicI64 = AtomicI64::new(0);
/// Cac chat da nhan cho bot (ghi tu vong polling, truoc khi loc chat id) de "Tim chat id" khong phai goi getUpdates song song.
static SEEN: Mutex<Vec<(i64, String)>> = Mutex::new(Vec::new());

fn remember_chats(list: &Value) {
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    for u in list.as_array().into_iter().flatten() {
        let ch = &u["message"]["chat"];
        if let Some(id) = ch["id"].as_i64().filter(|id| !seen.iter().any(|(x, _)| x == id)) {
            let name = ch["title"].as_str().or(ch["username"].as_str()).or(ch["first_name"].as_str()).unwrap_or("?");
            seen.push((id, name.into()));
        }
    }
}

/// Bao Telegram da xu ly toi OFFSET (Telegram chi ghi nhan offset o lan getUpdates ke tiep).
pub fn confirm_offset() {
    let (Ok(c), off) = (cfg(), OFFSET.load(Ordering::SeqCst)) else { return };
    if off > 0 {
        let _ = api(&c.token, "getUpdates", json!({ "offset": off, "timeout": 0, "limit": 1 }));
    }
}

/// Vong long-polling getUpdates. Loi mang/token -> cho 5s, 10s... toi da 60s. Chi vong dang song moi ghi trang thai.
fn poll(app: AppHandle, c: Cfg, gen: u64) {
    let (mut chat, mut wait) = (Chat::default(), 5u64);
    let _ = api(&c.token, "setMyCommands", json!({ "commands": [{ "command": "duan", "description": "Đổi dự án của phòng" }] }));
    let live = || GEN.load(Ordering::SeqCst) == gen;
    while live() {
        let off = OFFSET.load(Ordering::SeqCst);
        match api(&c.token, "getUpdates", json!({ "offset": off, "timeout": 30, "allowed_updates": ["message", "callback_query"] })) {
            Ok(list) if live() => {
                set_status("dang ket noi");
                wait = 5;
                remember_chats(&list);
                for u in list.as_array().into_iter().flatten() {
                    // Ghi offset truoc khi xu ly: loi giua chung cung khong xu ly lai lan nua.
                    OFFSET.fetch_max(u["update_id"].as_i64().unwrap_or(0) + 1, Ordering::SeqCst);
                    if live() {
                        handle(&app, &c, u, &mut chat);
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                if live() {
                    set_status(format!("loi: {e}"));
                }
                std::thread::sleep(Duration::from_secs(wait));
                wait = (wait * 2).min(60);
            }
        }
    }
}

/// Luu token vao Windows Credential Manager (rong = xoa).
#[tauri::command]
pub fn tg_set_token(token: String) -> Result<(), String> {
    let e = entry()?;
    // Bot khac co update_id khac: bat dau doc lai tu dau.
    OFFSET.store(0, Ordering::SeqCst);
    SEEN.lock().unwrap_or_else(|e| e.into_inner()).clear();
    if token.trim().is_empty() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(x) => Err(x.to_string()),
        }
    } else {
        e.set_password(token.trim()).map_err(|x| x.to_string())
    }
}

#[tauri::command]
pub fn tg_has_token() -> bool {
    token().is_some()
}

#[tauri::command]
pub fn tg_status() -> String {
    STATUS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Cau hinh lai va (khoi) chay vong polling. Thieu token hoac chat id -> tat bot.
#[tauri::command]
pub fn tg_config(app: AppHandle, chat_id: String, root: String, notes: String) {
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let c = match (token(), chat_id.trim().parse::<i64>()) {
        (Some(token), Ok(chat)) => Cfg { token, chat, root, notes },
        (None, _) => return (*CFG.lock().unwrap_or_else(|e| e.into_inner()) = None, set_status("tat (chua co token)")).0,
        (_, Err(_)) => return (*CFG.lock().unwrap_or_else(|e| e.into_inner()) = None, set_status("tat (chua co chat id)")).0,
    };
    *CFG.lock().unwrap_or_else(|e| e.into_inner()) = Some(c.clone());
    set_status("dang ket noi...");
    std::thread::spawn(move || poll(app, c, gen));
}

#[tauri::command]
pub async fn tg_test(chat_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let c = Cfg { token: token().ok_or("chua luu token")?, chat: chat_id.trim().parse().map_err(|_| "chat id phai la so")?, root: String::new(), notes: String::new() };
        send(&c, "✅ ClaudeDeck đã kết nối", &Value::Null).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Cac chat da nhan cho bot gan day: (id, ten) — de sep chon chat id.
#[tauri::command]
pub async fn tg_detect_chat() -> Result<Vec<(i64, String)>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        // Bot dang chay thi vong polling da ghi lai chat (goi getUpdates song song se bi 409 va cuop update).
        if cfg().is_err() {
            let list = api(&token().ok_or("chua luu token")?, "getUpdates", json!({ "timeout": 0 }))?;
            remember_chats(&list);
        }
        Ok(SEEN.lock().unwrap_or_else(|e| e.into_inner()).clone())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Gui thong bao cua task. kind: review | waiting | permission | context | cap. Chay nen, loi ghi vao status.
#[tauri::command]
pub fn tg_notify(id: String, kind: String) {
    std::thread::spawn(move || {
        let Ok(c) = cfg() else { return };
        let Some(mut t) = find(&c.root, &id) else { return };
        t.summary = tasks::summary(&t.dir);
        let (agent, dept) = names(&c.root, &t);
        let head = |icon: &str, what: &str| format!("{icon} <b>{what}</b> · <b>{}</b> · {}\nTask: {}", escape(&agent), escape(&dept), escape(&clip(&t.title, 120)));
        let (text, kb) = match kind.as_str() {
            "review" => review_message(&agent, &dept, &t),
            "waiting" => waiting_message(&agent, &dept, &t, false),
            "permission" => waiting_message(&agent, &dept, &t, true),
            "context" => (head("⚠️", "Phiên dài") + "\nNgữ cảnh vượt 250K token, nên tách task.", Value::Null),
            "cap" => (head("⛔", "Vượt trần token") + "\nĐã dừng phiên, mở ☰ trên ClaudeDeck để xem.", Value::Null),
            _ => return,
        };
        match send(&c, &text, &kb) {
            // Tin co nut hoac cho tra loi: nho message_id de bo nut / nhan reply.
            Ok(msg) if matches!(kind.as_str(), "review" | "waiting" | "permission") => {
                let _ = tasks::update(&c.root, &id, |t| Ok(t.tg_msg = msg));
            }
            Ok(_) => {}
            Err(e) => set_status(format!("loi gui tin: {e}")),
        }
    });
}

/// Task da xu ly tren app -> sua tin con nut thanh `text`, bo nut.
#[tauri::command]
pub fn tg_resolved(id: String, text: String) {
    std::thread::spawn(move || {
        let Ok(c) = cfg() else { return };
        let Some(t) = find(&c.root, &id).filter(|t| t.tg_msg != 0) else { return };
        edit(&c, t.tg_msg, &format!("{} · <b>{}</b>", escape(&text), escape(&t.title)));
        let _ = tasks::update(&c.root, &id, |t| Ok(t.tg_msg = 0));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: i64 = 42;

    fn task(summary: &str) -> Task {
        serde_json::from_value(json!({
            "id": "t1", "dept": "software-dev", "agent": "dev", "title": "Sua <nut> & menu", "status": "review",
            "reason": "xin quyen: Bash", "session": "s", "dir": "d", "cwd": "c", "created": 1
        }))
        .map(|mut t: Task| {
            t.summary = summary.into();
            t
        })
        .unwrap()
    }

    fn cb(chat: i64, data: &str) -> Value {
        json!({"update_id": 1, "callback_query": {"id": "q1", "data": data, "message": {"message_id": 7, "chat": {"id": chat}}}})
    }

    fn msg(chat: i64, text: Option<&str>) -> Value {
        let mut m = json!({"message_id": 8, "chat": {"id": chat}});
        if let Some(t) = text {
            m["text"] = t.into();
        }
        json!({"update_id": 2, "message": m})
    }

    /// Cham Credential Manager that (muc thu rieng, tu xoa). Chay tay: cargo test keyring_roundtrip -- --ignored
    #[test]
    #[ignore]
    fn keyring_roundtrip() {
        let e = keyring::Entry::new("claudedeck-test", "bot-token").unwrap();
        e.set_password("123:abc").unwrap();
        assert_eq!(e.get_password().unwrap(), "123:abc");
        e.delete_credential().unwrap();
        assert!(matches!(e.get_password(), Err(keyring::Error::NoEntry)));
    }

    #[test]
    fn ignores_other_chats() {
        let mut st = Chat::default();
        assert_eq!(parse_update(&cb(999, "ok:t1"), ME, &mut st).0, Action::Ignore);
        st.pending_rework = Some("t1".into());
        assert_eq!(parse_update(&msg(999, Some("lam lai di")), ME, &mut st).0, Action::Ignore);
        assert_eq!(st.pending_rework.as_deref(), Some("t1")); // nguoi la khong pha duoc hoi thoai
    }

    #[test]
    fn private_chat_requires_owner_as_sender() {
        // Chat rieng: chat.id == from.id cua sep. Tin "chuyen tiep" hay update gia co from khac -> bo qua.
        let mut st = Chat::default();
        let mut u = cb(ME, "ok:t1");
        u["callback_query"]["from"] = json!({"id": 777});
        assert_eq!(parse_update(&u, ME, &mut st).0, Action::Ignore);
        u["callback_query"]["from"] = json!({"id": ME});
        assert_eq!(parse_update(&u, ME, &mut st).0, Action::Approve("t1".into()));
        st.pending_rework = Some("t1".into());
        let mut m = msg(ME, Some("x"));
        m["message"]["from"] = json!({"id": 777});
        assert_eq!(parse_update(&m, ME, &mut st).0, Action::Ignore);
    }

    #[test]
    fn callback_maps_to_action() {
        let mut st = Chat::default();
        assert_eq!(parse_update(&cb(ME, "ok:t1"), ME, &mut st), (Action::Approve("t1".into()), Some("q1".into())));
        assert_eq!(parse_update(&cb(ME, "al:t1"), ME, &mut st).0, Action::Allow("t1".into()));
        assert_eq!(parse_update(&cb(ME, "de:t1"), ME, &mut st).0, Action::Deny("t1".into()));
        assert_eq!(parse_update(&cb(ME, "xx:t1"), ME, &mut st).0, Action::Ignore);
        assert_eq!(parse_update(&cb(ME, "re:t1"), ME, &mut st).0, Action::Rework("t1".into()));
        assert_eq!(st.pending_rework.as_deref(), Some("t1"));
    }

    #[test]
    fn rework_waits_for_text() {
        let mut st = Chat { pending_rework: Some("t1".into()), ..Default::default() };
        assert_eq!(parse_update(&msg(ME, None), ME, &mut st).0, Action::Ignore); // anh/sticker
        assert_eq!(parse_update(&msg(ME, Some("   ")), ME, &mut st).0, Action::Ignore);
        assert_eq!(parse_update(&msg(ME, Some("them test")), ME, &mut st).0, Action::ReworkNote("t1".into(), "them test".into()));
        assert!(st.pending_rework.is_none());
        // het cho nhan xet: tin thuong = giao task moi
        assert_eq!(parse_update(&msg(ME, Some("chao")), ME, &mut st).0, Action::NewTask("chao".into()));
    }

    #[test]
    fn reply_answers_the_message_it_quotes() {
        let mut st = Chat { pending_rework: Some("t1".into()), ..Default::default() };
        let mut u = msg(ME, Some("commit readme"));
        u["message"]["reply_to_message"] = json!({ "message_id": 5 });
        assert_eq!(parse_update(&u, ME, &mut st).0, Action::Reply(5, "commit readme".into()));
        assert_eq!(st.pending_rework.as_deref(), Some("t1")); // handle quyet dinh: tin 5 khong phai task cho -> nhan xet lam lai
    }

    #[test]
    fn project_flow_pick_room_then_name() {
        let mut st = Chat { pending_task: Some("x".into()), ..Default::default() };
        assert_eq!(parse_update(&msg(ME, Some("/duan")), ME, &mut st).0, Action::Projects);
        assert_eq!(parse_update(&msg(ME, Some("/duan@Claudeck17_bot")), ME, &mut st).0, Action::Projects);
        assert_eq!(parse_update(&cb(ME, "pd:software-dev"), ME, &mut st).0, Action::PickDept("software-dev".into()));
        assert!(st.pending_task.is_none()); // moi luc chi cho mot cau hoi
        assert_eq!(parse_update(&msg(ME, Some("app moi")), ME, &mut st).0, Action::SetProject("software-dev".into(), "app moi".into()));
        assert_eq!(parse_update(&msg(ME, Some("sua readme")), ME, &mut st).0, Action::NewTask("sua readme".into()));
        assert_eq!(parse_update(&cb(ME, "pj:software-dev/caby"), ME, &mut st).0, Action::SetProject("software-dev".into(), "caby".into()));
    }

    #[test]
    fn project_path_stays_in_parent_of_workspace() {
        let ws = std::env::temp_dir().join("cd-pj").join("caby");
        let ws = ws.to_string_lossy();
        assert_eq!(project_path(&ws, " app moi ").unwrap(), std::env::temp_dir().join("cd-pj").join("app moi"));
        for bad in ["", "..", "../x", "a\\b", "a/b", "c:", "x?", ".git", "con"] {
            assert!(project_path(&ws, bad).is_err(), "{bad}");
        }
        assert!(project_path("", "x").is_err()); // phong chua co du an -> khong biet thu muc goc
    }

    #[test]
    fn plain_text_then_agent_button_assigns_task() {
        let mut st = Chat::default();
        assert_eq!(parse_update(&msg(ME, Some("/start")), ME, &mut st).0, Action::Ignore);
        assert_eq!(parse_update(&msg(ME, Some("sua readme")), ME, &mut st).0, Action::NewTask("sua readme".into()));
        assert_eq!(parse_update(&cb(ME, "as:software-dev/dev"), ME, &mut st).0, Action::Assign("software-dev".into(), "dev".into(), "sua readme".into()));
        assert_eq!(parse_update(&cb(ME, "as:software-dev/dev"), ME, &mut st).0, Action::Ignore); // da giao, bam lai khong tao trung
        assert_eq!(parse_update(&cb(999, "as:software-dev/dev"), ME, &mut st).0, Action::Ignore);
    }

    #[test]
    fn review_message_escapes_html() {
        let (text, kb) = review_message("Dev", "Software Dev", &task("a < b && c > d"));
        assert!(text.contains("Chờ síp duyệt") && text.contains("<b>Dev</b>") && text.contains("Software Dev"), "{text}");
        assert!(text.contains("Sua &lt;nut&gt; &amp; menu") && text.contains("a &lt; b &amp;&amp; c &gt; d"), "{text}");
        assert_eq!(kb["inline_keyboard"][0][0]["callback_data"], "ok:t1");
        assert_eq!(kb["inline_keyboard"][0][1]["callback_data"], "re:t1");
    }

    #[test]
    fn summary_is_truncated() {
        let (text, _) = review_message("Dev", "D", &task(&"x".repeat(1000)));
        assert!(text.chars().count() < 450, "{}", text.chars().count());
        assert!(text.contains('…'));
    }

    #[test]
    fn waiting_message_buttons_only_for_permission() {
        let (text, kb) = waiting_message("Dev", "D", &task(""), true);
        assert!(text.contains("Chờ síp trả lời") && text.contains("xin quyen: Bash"), "{text}");
        assert_eq!(kb["inline_keyboard"][0][0]["callback_data"], "al:t1");
        assert_eq!(kb["inline_keyboard"][0][1]["callback_data"], "de:t1");
        let mut t = task("");
        t.ask = "Sep muon commit <gi>?".into();
        let (text, kb) = waiting_message("Dev", "D", &t, false);
        assert!(kb.is_null());
        assert!(text.contains("Sep muon commit &lt;gi&gt;?"), "{text}");
    }
}
