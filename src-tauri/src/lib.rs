mod accounts;
mod clip;
mod history;
mod pty;
mod usage;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(pty::Sessions::default())
        .invoke_handler(tauri::generate_handler![
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            usage::get_usage,
            clip::save_image,
            history::list_sessions,
            open_in_editor,
            accounts::list_accounts,
            accounts::switch_account,
            accounts::remove_account
        ])
        .run(tauri::generate_context!())
        .expect("error while running app");
}

/// Mo file trong Notepad++ tai dong `line`. Khong phai file (thu muc, path sai) -> Err, JS fallback sang Explorer.
#[tauri::command]
fn open_in_editor(path: String, line: Option<u32>) -> Result<(), String> {
    if !std::path::Path::new(&path).is_file() {
        return Err("not a file".into());
    }
    let mut args = vec![path];
    if let Some(n) = line {
        args.push(format!("-n{n}"));
    }
    // ponytail: hardcode cho cai dat mac dinh, them setting khi can editor khac.
    [r"C:\Program Files\Notepad++\notepad++.exe", r"C:\Program Files (x86)\Notepad++\notepad++.exe", "notepad++"]
        .iter()
        .find_map(|exe| std::process::Command::new(exe).args(&args).spawn().ok())
        .map(|_| ())
        .ok_or_else(|| "notepad++ not found".into())
}
