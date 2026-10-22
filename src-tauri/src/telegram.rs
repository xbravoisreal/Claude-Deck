use crate::office::load_office;
use crate::tasks::{self, Task};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
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
    Ignore,
}

/// Trang thai hoi thoai: dang cho nhan xet lam lai cho task nao.
#[derive(Default)]
pub struct Chat {
    pub pending_rework: Option<String>,
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
    if let Some(q) = u.get("callback_query") {
        if q["message"]["chat"]["id"].as_i64() != Some(chat_id) {
            return (Action::Ignore, None);
        }
        let qid = q["id"].as_str().map(String::from);
        let Some((kind, id)) = q["data"].as_str().and_then(|d| d.split_once(':')) else { return (Action::Ignore, qid) };
        let id = id.to_string();
        let a = match kind {
            "ok" => Action::Approve(id),
            "re" => {
                st.pending_rework = Some(id.clone());
                Action::Rework(id)
            }
            "al" => Action::Allow(id),
            "de" => Action::Deny(id),
            _ => Action::Ignore,
        };
        return (a, qid);
    }
    let m = &u["message"];
    if m["chat"]["id"].as_i64() != Some(chat_id) {
        return (Action::Ignore, None);
    }
    match (m["text"].as_str().map(str::trim).filter(|t| !t.is_empty()), &st.pending_rework) {
        (Some(text), Some(_)) => (Action::ReworkNote(st.pending_rework.take().unwrap(), text.into()), None),
        _ => (Action::Ignore, None),
    }
}

fn buttons(rows: &[(&str, String)]) -> Value {
    json!({ "inline_keyboard": [rows.iter().map(|(t, d)| json!({ "text": t, "callback_data": d })).collect::<Vec<_>>()] })
}

pub fn review_message(agent: &str, dept: &str, t: &Task) -> (String, Value) {
    let mut text = format!("🟠 <b>Chờ síp</b> · <b>{}</b> · {}\nTask: {}", escape(agent), escape(dept), escape(&clip(&t.title, 120)));
    if !t.summary.is_empty() {
        text += &format!("\nTóm tắt: {}", escape(&clip(&t.summary, 300)));
    }
    (text, buttons(&[("✅ Duyệt", format!("ok:{}", t.id)), ("↩ Làm lại", format!("re:{}", t.id))]))
}

/// Cho anh: chi xin quyen moi co nut Cho phep / Tu choi.
pub fn waiting_message(agent: &str, dept: &str, t: &Task, permission: bool) -> (String, Value) {
    let mut text = format!("🟡 <b>Chờ anh</b> · <b>{}</b> · {}\nTask: {}", escape(agent), escape(dept), escape(&clip(&t.title, 120)));
    if !t.reason.is_empty() {
        text += &format!("\nLý do: {}", escape(&clip(&t.reason, 200)));
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
    let (action, qid) = parse_update(u, c.chat, chat);
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
            Err(_) => {
                let _ = send(c, "Task đã xử lý rồi.", &Value::Null);
            }
        },
        Action::Allow(id) | Action::Deny(id) if find(&c.root, &id).is_some_and(|t| t.status == "waiting") => {
            let allow = u["callback_query"]["data"].as_str().is_some_and(|d| d.starts_with("al:"));
            let _ = app.emit("office-permission", json!({ "task": id, "allow": allow }));
            answer(c, &qid, if allow { "Đã cho phép" } else { "Đã từ chối" });
            edit(c, cb_msg, &format!("{} · task {}", if allow { "✅ Đã cho phép" } else { "⛔ Đã từ chối" }, escape(&id)));
        }
        Action::Allow(_) | Action::Deny(_) => answer(c, &qid, "Đã xử lý rồi"),
        Action::Ignore => answer(c, &qid, ""),
    }
}

/// Vong long-polling getUpdates. Loi mang/token -> cho 5s, 10s... toi da 60s.
fn poll(app: AppHandle, c: Cfg, gen: u64) {
    let (mut offset, mut chat, mut wait) = (0i64, Chat::default(), 5u64);
    while GEN.load(Ordering::SeqCst) == gen {
        match api(&c.token, "getUpdates", json!({ "offset": offset, "timeout": 30, "allowed_updates": ["message", "callback_query"] })) {
            Ok(list) => {
                set_status("dang ket noi");
                wait = 5;
                for u in list.as_array().into_iter().flatten() {
                    offset = offset.max(u["update_id"].as_i64().unwrap_or(0) + 1);
                    if GEN.load(Ordering::SeqCst) == gen {
                        handle(&app, &c, u, &mut chat);
                    }
                }
            }
            Err(e) => {
                set_status(format!("loi: {e}"));
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
pub fn tg_config(app: AppHandle, chat_id: String, root: String) {
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let c = match (token(), chat_id.trim().parse::<i64>()) {
        (Some(token), Ok(chat)) => Cfg { token, chat, root },
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
        let c = Cfg { token: token().ok_or("chua luu token")?, chat: chat_id.trim().parse().map_err(|_| "chat id phai la so")?, root: String::new() };
        send(&c, "✅ ClaudeDeck đã kết nối", &Value::Null).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Cac chat da nhan cho bot gan day: (id, ten) — de sep chon chat id.
#[tauri::command]
pub async fn tg_detect_chat() -> Result<Vec<(i64, String)>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let list = api(&token().ok_or("chua luu token")?, "getUpdates", json!({ "timeout": 0 }))?;
        let mut out: Vec<(i64, String)> = vec![];
        for u in list.as_array().into_iter().flatten() {
            let ch = &u["message"]["chat"];
            if let Some(id) = ch["id"].as_i64().filter(|id| !out.iter().any(|(x, _)| x == id)) {
                let name = ch["title"].as_str().or(ch["username"].as_str()).or(ch["first_name"].as_str()).unwrap_or("?");
                out.push((id, name.into()));
            }
        }
        Ok(out)
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
            Ok(msg) if !kb.is_null() => {
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

    #[test]
    fn ignores_other_chats() {
        let mut st = Chat::default();
        assert_eq!(parse_update(&cb(999, "ok:t1"), ME, &mut st).0, Action::Ignore);
        st.pending_rework = Some("t1".into());
        assert_eq!(parse_update(&msg(999, Some("lam lai di")), ME, &mut st).0, Action::Ignore);
        assert_eq!(st.pending_rework.as_deref(), Some("t1")); // nguoi la khong pha duoc hoi thoai
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
        let mut st = Chat { pending_rework: Some("t1".into()) };
        assert_eq!(parse_update(&msg(ME, None), ME, &mut st).0, Action::Ignore); // anh/sticker
        assert_eq!(parse_update(&msg(ME, Some("   ")), ME, &mut st).0, Action::Ignore);
        assert_eq!(parse_update(&msg(ME, Some("them test")), ME, &mut st).0, Action::ReworkNote("t1".into(), "them test".into()));
        assert!(st.pending_rework.is_none());
        assert_eq!(parse_update(&msg(ME, Some("chao")), ME, &mut st).0, Action::Ignore); // khong con cho nhan xet
    }

    #[test]
    fn review_message_escapes_html() {
        let (text, kb) = review_message("Dev", "Software Dev", &task("a < b && c > d"));
        assert!(text.contains("Chờ síp") && text.contains("<b>Dev</b>") && text.contains("Software Dev"), "{text}");
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
        assert!(text.contains("Chờ anh") && text.contains("xin quyen: Bash"), "{text}");
        assert_eq!(kb["inline_keyboard"][0][0]["callback_data"], "al:t1");
        assert_eq!(kb["inline_keyboard"][0][1]["callback_data"], "de:t1");
        let (_, kb) = waiting_message("Dev", "D", &task(""), false);
        assert!(kb.is_null());
    }
}
