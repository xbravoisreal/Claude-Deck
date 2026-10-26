use crate::tasks;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter};

const SERVICE: &str = "claudedeck";
const USER: &str = "remote-token";
const PORT: u16 = 7878;
const PAGE: &str = include_str!("remote.html");

/// Moi lan bat/tat tang so nay; luong server cu thay so doi thi tu dong dung (giai phong cong).
static GEN: AtomicU64 = AtomicU64::new(0);

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, USER).map_err(|e| e.to_string())
}

/// Token truy cap trang xem tu xa: tu sinh lan dau, luu trong Credential Manager.
fn token() -> Result<String, String> {
    let e = entry()?;
    if let Some(t) = e.get_password().ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
        return Ok(t);
    }
    let r = || RandomState::new().build_hasher().finish();
    let t = format!("{:016x}{:016x}", r(), r());
    e.set_password(&t).map_err(|e| e.to_string())?;
    Ok(t)
}

/// IP Tailscale (IPv4) cua may nay. Server chi bind dia chi nay: khong mo ra LAN/Internet.
fn tailscale_ip() -> Result<String, String> {
    for exe in ["tailscale", r"C:\Program Files\Tailscale\tailscale.exe"] {
        let mut c = std::process::Command::new(exe);
        c.args(["ip", "-4"]);
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000);
        if let Ok(o) = c.output() {
            let ip = String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").trim().to_string();
            if o.status.success() && !ip.is_empty() {
                return Ok(ip);
            }
        }
    }
    Err("khong thay Tailscale (cai va dang nhap Tailscale tren may nay)".into())
}

/// (status, content-type, body, co doi task khong). Moi route deu can token (header X-Token hoac ?t=).
fn route(req: &str, token: &str, root: &str) -> (u16, &'static str, String, bool) {
    let mut first = req.lines().next().unwrap_or("").split_whitespace();
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let given = req
        .lines()
        .find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("x-token")).map(|(_, v)| v.trim()))
        .or_else(|| query.split('&').find_map(|kv| kv.strip_prefix("t=")))
        .unwrap_or("");
    if given != token {
        return (401, "text/plain", "token sai".into(), false);
    }
    let body = req.split_once("\r\n\r\n").map_or("", |(_, b)| b);
    let done = |r: Result<tasks::Task, String>| match r {
        Ok(_) => (200, "text/plain", "ok".into(), true),
        Err(e) => (409, "text/plain", e, false),
    };
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    match (method, parts.as_slice()) {
        ("GET", [""]) => (200, "text/html; charset=utf-8", PAGE.into(), false),
        ("GET", ["api", "tasks"]) => match tasks::task_list(root.into()) {
            Ok(mut ts) => {
                // Task xong chi giu 10 cai moi nhat cho nhe.
                ts.sort_by_key(|t| std::cmp::Reverse(t.finished.max(t.created)));
                let mut n = 0;
                ts.retain(|t| {
                    n += (t.status == "done") as usize;
                    t.status != "done" || n <= 10
                });
                (200, "application/json", serde_json::to_string(&ts).unwrap_or_default(), false)
            }
            Err(e) => (500, "text/plain", e, false),
        },
        ("POST", ["api", "approve", id]) => done(tasks::update(root, id, tasks::approve)),
        ("POST", ["api", "cancel", id]) => done(tasks::update(root, id, tasks::cancel)),
        ("POST", ["api", "rework", id]) => {
            let note = serde_json::from_str::<serde_json::Value>(body).ok().and_then(|v| v["note"].as_str().map(|s| s.trim().to_string())).unwrap_or_default();
            if note.is_empty() {
                return (400, "text/plain", "thieu nhan xet".into(), false);
            }
            done(tasks::update(root, id, |t| tasks::rework(t, &note)))
        }
        _ => (404, "text/plain", "khong co".into(), false),
    }
}

fn serve(app: AppHandle, listener: TcpListener, root: String, gen: u64) {
    let _ = listener.set_nonblocking(true);
    while GEN.load(Ordering::SeqCst) == gen {
        let Ok((mut stream, _)) = listener.accept() else {
            std::thread::sleep(std::time::Duration::from_millis(150));
            continue;
        };
        let _ = stream.set_nonblocking(false);
        let Ok(tok) = token() else { continue };
        let req = crate::pty::read_request(&mut stream);
        let (code, ct, body, changed) = route(&req, &tok, &root);
        let _ = write!(stream, "HTTP/1.1 {code} X\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}", body.len());
        if changed {
            let _ = app.emit("office-tasks-changed", ());
        }
    }
}

/// Bat/tat trang xem tu xa. Tra ve URL (da kem token) khi bat, rong khi tat.
#[tauri::command]
pub fn remote_config(app: AppHandle, enabled: bool, root: String) -> Result<String, String> {
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    if !enabled {
        return Ok(String::new());
    }
    let ip = tailscale_ip()?;
    // Luong cu can toi da ~150ms de nhan ra va nha cong; thu lai vai lan khi doi root/bat lai.
    let mut last = String::new();
    for _ in 0..10 {
        match TcpListener::bind((ip.as_str(), PORT)) {
            Ok(listener) => {
                std::thread::spawn(move || serve(app, listener, root, gen));
                return Ok(format!("http://{ip}:{PORT}/?t={}", token()?));
            }
            Err(e) => last = e.to_string(),
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(format!("khong mo duoc {ip}:{PORT}: {last}"))
}

/// Doi token (thiet bi cu mat quyen truy cap). Goi remote_config lai de lay URL moi.
#[tauri::command]
pub fn remote_reset_token() -> Result<(), String> {
    entry()?.delete_credential().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_token_and_serves_page() {
        let r = "/nonexistent-root";
        assert_eq!(route("GET /api/tasks HTTP/1.1\r\n\r\n", "abc", r).0, 401);
        assert_eq!(route("GET /?t=zzz HTTP/1.1\r\n\r\n", "abc", r).0, 401);
        assert_eq!(route("GET /?t=abc HTTP/1.1\r\n\r\n", "abc", r).0, 200);
        assert_eq!(route("GET /api/tasks HTTP/1.1\r\nX-Token: abc\r\n\r\n", "abc", r).2, "[]");
        assert_eq!(route("POST /api/rework/x HTTP/1.1\r\nX-Token: abc\r\n\r\n{}", "abc", r).0, 400);
        assert_eq!(route("POST /api/approve/x HTTP/1.1\r\nX-Token: abc\r\n\r\n", "abc", r).0, 409);
    }
}
