mod accounts;
mod clip;
mod history;
mod hr;
mod office;
mod pty;
mod tasks;
mod telegram;
mod theme;
mod usage;

pub fn run() {
    tauri::Builder::default()
        // Phai dang ky dau tien. App dang an duoi khay ma mo lan nua -> hien cua so cu, khong chay ban thu hai
        // (hai ban se tranh nhau getUpdates cua bot va mo hai signal server).
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(pty::Sessions::default())
        .setup(|app| {
            tray(app)?;
            Ok(())
        })
        // Nut X = thu xuong khay: phien Agent va bot Telegram van chay; thoat han tu menu khay.
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                api.prevent_close();
                let _ = w.hide();
            }
        })
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
            office::office_set_agent_status,
            hr::hr_candidates,
            hr::hr_request,
            hr::hr_revise,
            hr::hr_hire,
            hr::hr_reject,
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
            telegram::tg_set_token,
            telegram::tg_has_token,
            telegram::tg_status,
            telegram::tg_config,
            telegram::tg_test,
            telegram::tg_detect_chat,
            telegram::tg_notify,
            telegram::tg_resolved,
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

fn show_main(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Icon khay: bam trai mo cua so; menu "Mo ClaudeDeck" / "Thoat".
fn tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let show = MenuItem::with_id(app, "show", "Mo ClaudeDeck", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Thoat", true, None::<&str>)?;
    let mut b = TrayIconBuilder::new()
        .tooltip("ClaudeDeck")
        .menu(&Menu::with_items(app, &[&show, &quit])?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "show" => show_main(app),
            "quit" => {
                telegram::confirm_offset();
                app.exit(0)
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.build(app)?;
    Ok(())
}

/// Mo file/thu muc bang app mac dinh cua he thong.
#[tauri::command]
fn open_in_editor(path: String) -> Result<(), String> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}
