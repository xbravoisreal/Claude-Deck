use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpListener,
    sync::{Mutex, OnceLock},
};
use tauri::{AppHandle, Emitter, State};

pub struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

#[derive(Default)]
pub struct Sessions(Mutex<HashMap<String, Session>>);

fn err<E: ToString>(e: E) -> String {
    e.to_string()
}

static SIGNAL_PORT: OnceLock<u16> = OnceLock::new();
const RESP: &[u8] = b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n";

/// Server localhost nho: hook Stop/Notification cua claude goi GET /<id>
/// -> emit `claude-signal-<id>` de frontend cho tab nhap nhay.
fn signal_port(app: &AppHandle) -> u16 {
    *SIGNAL_PORT.get_or_init(|| {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind signal server");
        let port = listener.local_addr().unwrap().port();
        let app = app.clone();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                // Hook Notification gui kem JSON (message: xin quyen hay chi ranh) qua than request.
                if let Some((id, kind, message)) = parse_signal(&read_request(&mut stream)) {
                    let _ = app.emit(&format!("claude-signal-{id}"), serde_json::json!({ "kind": kind, "message": message }));
                }
                let _ = stream.write_all(RESP);
            }
        });
        port
    })
}

/// Ghi file settings chua hook Stop + Notification tro ve signal server (gop vao `extra`), tra ve duong dan.
fn hook_settings(app: &AppHandle, id: &str, extra: Option<serde_json::Value>) -> Result<String, String> {
    let port = signal_port(app);
    // curl doc JSON cua hook tu stdin (--data-binary @-) va gui len server.
    let hook = |kind: &str| {
        let cmd = format!("curl.exe -s -m 2 --data-binary @- http://127.0.0.1:{port}/{id}/{kind}");
        serde_json::json!([{ "hooks": [{ "type": "command", "command": cmd }] }])
    };
    let mut settings = extra.filter(|v| v.is_object()).unwrap_or_else(|| serde_json::json!({}));
    settings["hooks"] = serde_json::json!({ "Stop": hook("stop"), "Notification": hook("notify") });
    let path = std::env::temp_dir().join(format!("claudedeck-hooks-{id}.json"));
    std::fs::write(&path, settings.to_string()).map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Chuoi nhay don cua PowerShell: `'` trong chuoi viet thanh `''`.
pub fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// kind = "chat": chay `claude` trong PowerShell. kind = "shell": PowerShell thuong.
/// args: tham so them cho claude, vd "--resume <id>".
#[tauri::command]
pub fn pty_spawn(
    app: AppHandle,
    state: State<Sessions>,
    id: String,
    cwd: String,
    kind: String,
    cols: u16,
    rows: u16,
    args: Option<String>,
    settings: Option<serde_json::Value>,
) -> Result<(), String> {
    let pair = native_pty_system()
        .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(err)?;

    let mut cmd = CommandBuilder::new("powershell.exe");
    cmd.arg("-NoLogo");
    if kind == "chat" {
        let settings = hook_settings(&app, &id, settings)?;
        let claude = format!("claude --settings {} {}", ps_quote(&settings), args.unwrap_or_default());
        cmd.args(["-NoExit", "-Command", &claude]);
    }
    cmd.cwd(&cwd);
    cmd.env("TERM", "xterm-256color");

    let child = pair.slave.spawn_command(cmd).map_err(err)?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(err)?;
    let writer = pair.master.take_writer().map_err(err)?;
    let event = format!("pty-data-{id}");

    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut pending: Vec<u8> = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    // Chi gui phan UTF-8 hop le, giu lai byte cat doi o cuoi chunk.
                    let valid = match std::str::from_utf8(&pending) {
                        Ok(_) => pending.len(),
                        Err(e) if e.error_len().is_none() => e.valid_up_to(),
                        Err(_) => pending.len(),
                    };
                    if valid > 0 {
                        let s = String::from_utf8_lossy(&pending[..valid]).to_string();
                        let _ = app.emit(&event, s);
                        pending.drain(..valid);
                    }
                }
            }
        }
    });

    state
        .0
        .lock()
        .unwrap()
        .insert(id, Session { master: pair.master, writer, child });
    Ok(())
}

#[tauri::command]
pub fn pty_write(state: State<Sessions>, id: String, data: String) -> Result<(), String> {
    if let Some(s) = state.0.lock().unwrap().get_mut(&id) {
        s.writer.write_all(data.as_bytes()).map_err(err)?;
        s.writer.flush().map_err(err)?;
    }
    Ok(())
}

#[tauri::command]
pub fn pty_resize(state: State<Sessions>, id: String, cols: u16, rows: u16) -> Result<(), String> {
    if let Some(s) = state.0.lock().unwrap().get(&id) {
        s.master
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(err)?;
    }
    Ok(())
}

/// "<METHOD> /<id>[/<kind>] HTTP/1.1 ... \r\n\r\n<json hook>" -> (id, kind, message). Khong co kind = stop.
pub fn parse_signal(req: &str) -> Option<(String, String, String)> {
    let path = req.lines().next()?.split_whitespace().nth(1)?.trim_start_matches('/');
    let (id, kind) = path.split_once('/').unwrap_or((path, "stop"));
    let body = req.split_once("\r\n\r\n").map_or("", |(_, b)| b);
    let msg = serde_json::from_str::<serde_json::Value>(body).ok().and_then(|v| v["message"].as_str().map(String::from)).unwrap_or_default();
    Some((id.into(), kind.into(), msg))
}

/// Doc tron mot request (header + than theo Content-Length), toi da 64KB, cho toi da 2s.
fn read_request(stream: &mut std::net::TcpStream) -> String {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    while data.len() < 65536 {
        let n = match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        data.extend_from_slice(&buf[..n]);
        let s = String::from_utf8_lossy(&data);
        if let Some((head, body)) = s.split_once("\r\n\r\n") {
            let len = head
                .lines()
                .find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.trim().parse::<usize>().ok()))
                .unwrap_or(0);
            if body.len() >= len {
                break;
            }
        }
    }
    String::from_utf8_lossy(&data).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_reads_body() {
        let body = r#"{"hook_event_name":"Notification","message":"Claude needs your permission to use Bash"}"#;
        let req = format!("POST /abc-chat-0/notify HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        assert_eq!(parse_signal(&req), Some(("abc-chat-0".into(), "notify".into(), "Claude needs your permission to use Bash".into())));
        assert_eq!(parse_signal("GET /abc/stop HTTP/1.1\r\n\r\n"), Some(("abc".into(), "stop".into(), String::new())));
        assert_eq!(parse_signal("GET /abc HTTP/1.1\r\n\r\n"), Some(("abc".into(), "stop".into(), String::new())));
        assert_eq!(parse_signal("rac"), None);
    }
}

#[tauri::command]
pub fn pty_kill(state: State<Sessions>, id: String) {
    if let Some(mut s) = state.0.lock().unwrap().remove(&id) {
        let _ = s.child.kill();
    }
}
