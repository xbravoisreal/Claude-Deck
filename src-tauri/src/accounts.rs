use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;

/// Moi account = 1 file ~/.claude/claudedeck-accounts/<email>.json chua
/// { credentials: <.credentials.json>, oauthAccount: <~/.claude.json#oauthAccount> }.
/// Doi account = ghi lai 2 cho do. Claude dang chay giu token cu toi khi mo chat moi.
#[derive(Serialize)]
pub struct Accounts {
    current: Option<String>,
    saved: Vec<Saved>,
}

#[derive(Serialize)]
pub struct Saved {
    email: String,
    plan: String,
}

fn e<E: ToString>(e: E) -> String {
    e.to_string()
}

fn home() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "no home".into())
}

fn creds_path() -> Result<PathBuf, String> {
    Ok(home()?.join(".claude").join(".credentials.json"))
}

fn config_path() -> Result<PathBuf, String> {
    Ok(home()?.join(".claude.json"))
}

fn store() -> Result<PathBuf, String> {
    let d = home()?.join(".claude").join("claudedeck-accounts");
    std::fs::create_dir_all(&d).map_err(e)?;
    Ok(d)
}

fn read(p: &PathBuf) -> Result<Value, String> {
    serde_json::from_str(&std::fs::read_to_string(p).map_err(e)?).map_err(e)
}

fn write(p: &PathBuf, v: &Value) -> Result<(), String> {
    std::fs::write(p, serde_json::to_string_pretty(v).map_err(e)?).map_err(e)
}

/// Snapshot account dang dang nhap (token co the vua duoc claude refresh). None neu chua login.
fn save_current() -> Result<Option<String>, String> {
    let Ok(account) = read(&config_path()?).map(|c| c["oauthAccount"].clone()) else { return Ok(None) };
    let Some(email) = account["emailAddress"].as_str().map(String::from) else { return Ok(None) };
    let Ok(credentials) = read(&creds_path()?) else { return Ok(None) };
    write(&store()?.join(format!("{email}.json")), &json!({ "credentials": credentials, "oauthAccount": account }))?;
    Ok(Some(email))
}

#[tauri::command]
pub fn list_accounts() -> Result<Accounts, String> {
    let current = save_current()?;
    let mut saved: Vec<Saved> = std::fs::read_dir(store()?)
        .map_err(e)?
        .filter_map(Result::ok)
        .filter_map(|f| {
            let v = read(&f.path()).ok()?;
            Some(Saved {
                email: v["oauthAccount"]["emailAddress"].as_str()?.into(),
                plan: v["credentials"]["claudeAiOauth"]["subscriptionType"].as_str().unwrap_or("").into(),
            })
        })
        .collect();
    saved.sort_by(|a, b| a.email.cmp(&b.email));
    Ok(Accounts { current, saved })
}

#[tauri::command]
pub fn switch_account(email: String) -> Result<(), String> {
    save_current()?;
    let snap = read(&store()?.join(format!("{email}.json")))?;
    write(&creds_path()?, &snap["credentials"])?;
    // ponytail: read-modify-write ~/.claude.json, co the dua voi claude dang ghi cung luc.
    let mut cfg = read(&config_path()?)?;
    cfg["oauthAccount"] = snap["oauthAccount"].clone();
    write(&config_path()?, &cfg)
}

#[tauri::command]
pub fn remove_account(email: String) -> Result<(), String> {
    std::fs::remove_file(store()?.join(format!("{email}.json"))).map_err(e)
}
