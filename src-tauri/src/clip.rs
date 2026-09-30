use tauri::ipc::{InvokeBody, Request};

/// Luu anh paste tu clipboard ra file tam, tra ve duong dan de dan vao claude.
#[tauri::command]
pub fn save_image(request: Request) -> Result<String, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw bytes".into());
    };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    // ponytail: file tam khong bao gio xoa, de OS/nguoi dung don temp.
    let path = std::env::temp_dir().join(format!("claudedeck-{ts}.png"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}
