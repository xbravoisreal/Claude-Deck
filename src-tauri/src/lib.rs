mod accounts;
mod clip;
mod history;
mod office;
mod pty;
mod tasks;
mod theme;
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
            office::office_root,
            office::office_load,
            office::office_set_workspace,
            office::office_launch,
            office::office_create_department,
            tasks::task_list,
            tasks::task_create,
            tasks::task_idle,
            tasks::task_busy,
            tasks::task_approve,
            tasks::task_rework,
            tasks::task_stop,
            tasks::task_cancel,
            tasks::task_start_next,
            tasks::task_resume,
            tasks::task_stats,
            theme::theme_load,
            theme::theme_list,
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
