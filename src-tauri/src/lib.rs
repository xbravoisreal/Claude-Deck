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

/// Mo file/thu muc bang app mac dinh cua he thong.
#[tauri::command]
fn open_in_editor(path: String) -> Result<(), String> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}
