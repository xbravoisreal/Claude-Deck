use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::Value;
use std::io::{BufRead, BufReader};

#[derive(Serialize)]
pub struct Usage {
    source: String,
    five_hour_pct: Option<f64>,
    five_hour_reset: Option<String>,
    seven_day_pct: Option<f64>,
    seven_day_reset: Option<String>,
    five_hour_tokens: u64,
    seven_day_tokens: u64,
}

/// Uu tien usage API (OAuth token cua claude), loi thi uoc tinh tu transcript local.
#[tauri::command]
pub async fn get_usage() -> Result<Usage, String> {
    let (five_hour_tokens, seven_day_tokens) = count_logs();
    let mut u = Usage {
        source: "logs".into(),
        five_hour_pct: None,
        five_hour_reset: None,
        seven_day_pct: None,
        seven_day_reset: None,
        five_hour_tokens,
        seven_day_tokens,
    };
    if let Ok(v) = fetch_oauth().await {
        u.source = "oauth".into();
        u.five_hour_pct = v["five_hour"]["utilization"].as_f64();
        u.five_hour_reset = v["five_hour"]["resets_at"].as_str().map(Into::into);
        u.seven_day_pct = v["seven_day"]["utilization"].as_f64();
        u.seven_day_reset = v["seven_day"]["resets_at"].as_str().map(Into::into);
    }
    Ok(u)
}

fn e<E: ToString>(e: E) -> String {
    e.to_string()
}

async fn fetch_oauth() -> Result<Value, String> {
    let path = dirs::home_dir().ok_or("no home")?.join(".claude").join(".credentials.json");
    let creds: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(e)?).map_err(e)?;
    let token = creds["claudeAiOauth"]["accessToken"].as_str().ok_or("no token")?;
    reqwest::Client::new()
        .get("https://api.anthropic.com/api/oauth/usage")
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await
        .map_err(e)?
        .error_for_status()
        .map_err(e)?
        .json()
        .await
        .map_err(e)
}

/// Tong token (input + output) trong 5h va 7 ngay qua tu ~/.claude/projects/**/*.jsonl.
// ponytail: quet lai toan bo log moi phut, cache theo mtime neu log qua lon.
fn count_logs() -> (u64, u64) {
    let Some(root) = dirs::home_dir().map(|h| h.join(".claude").join("projects")) else {
        return (0, 0);
    };
    let now = Utc::now();
    let (h5, d7) = (now - Duration::hours(5), now - Duration::days(7));
    let (mut a, mut b) = (0u64, 0u64);
    for ent in walkdir::WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let p = ent.path();
        if p.extension().is_none_or(|x| x != "jsonl") {
            continue;
        }
        let mtime = ent.metadata().ok().and_then(|m| m.modified().ok()).map(DateTime::<Utc>::from);
        if mtime.is_none_or(|t| t < d7) {
            continue;
        }
        let Ok(f) = std::fs::File::open(p) else { continue };
        for line in BufReader::new(f).lines().map_while(Result::ok) {
            if !line.contains("\"usage\"") {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            let Some(ts) = v["timestamp"].as_str().and_then(|t| t.parse::<DateTime<Utc>>().ok()) else {
                continue;
            };
            let u = &v["message"]["usage"];
            let n = u["input_tokens"].as_u64().unwrap_or(0) + u["output_tokens"].as_u64().unwrap_or(0);
            if ts >= d7 {
                b += n;
            }
            if ts >= h5 {
                a += n;
            }
        }
    }
    (a, b)
}
