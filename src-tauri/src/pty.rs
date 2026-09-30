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
                let mut buf = [0u8; 512];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                // "GET /<id> HTTP/1.1"
                if let Some(id) = req.split_whitespace().nth(1).map(|p| p.trim_start_matches('/')) {
                    let _ = app.emit(&format!("claude-signal-{id}"), ());
                }
                let _ = stream.write_all(RESP);
            }
        });
        port
    })
}

/// Ghi file settings chua hook Stop + Notification tro ve signal server, tra ve duong dan.
fn hook_settings(app: &AppHandle, id: &str) -> Result<String, String> {
    let cmd = format!("curl.exe -s -m 2 http://127.0.0.1:{}/{id}", signal_port(app));
    let hook = serde_json::json!([{ "hooks": [{ "type": "command", "command": cmd }] }]);
    let settings = serde_json::json!({ "hooks": { "Stop": hook, "Notification": hook } });
    let path = std::env::temp_dir().join(format!("claudedeck-hooks-{id}.json"));
    std::fs::write(&path, settings.to_string()).map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
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
) -> Result<(), String> {
    let pair = native_pty_system()
        .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(err)?;

    let mut cmd = CommandBuilder::new("powershell.exe");
    cmd.arg("-NoLogo");
    if kind == "chat" {
        let settings = hook_settings(&app, &id)?;
        let claude = format!("claude --settings '{settings}' {}", args.unwrap_or_default());
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

#[tauri::command]
pub fn pty_kill(state: State<Sessions>, id: String) {
    if let Some(mut s) = state.0.lock().unwrap().remove(&id) {
        let _ = s.child.kill();
    }
}
