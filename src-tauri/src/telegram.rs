use crate::tasks::Task;
use serde_json::{json, Value};

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
