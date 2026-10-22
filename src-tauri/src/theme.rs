use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

#[path = "theme_files.rs"]
mod files;
pub use files::DEFAULT_FILES;

#[derive(Serialize, Debug)]
pub struct Theme {
    pub name: String,
    pub colors: Map<String, Value>,
    pub font: String,
    /// {frame, scale, characterY, deskY}
    pub sprite: Value,
    /// slot -> data URL
    pub slots: BTreeMap<String, String>,
    pub errors: Vec<String>,
}

fn mime(name: &str) -> Option<&'static str> {
    Some(match name.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    })
}

fn base64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

fn data_url(path: &Path) -> Option<String> {
    let m = mime(&path.to_string_lossy())?;
    Some(format!("data:{m};base64,{}", base64(&std::fs::read(path).ok()?)))
}

/// Ghi de `t` bang theme.json (da parse) cua thu muc `dir`: mau tung khoa, font, sprite tung khoa, slot tung cai.
fn overlay(t: &mut Theme, v: &Value, dir: &Path) {
    if let Some(c) = v["colors"].as_object() {
        t.colors.extend(c.clone());
    }
    if let Some(f) = v["font"].as_str() {
        t.font = f.into();
    }
    if let (Some(s), Some(base)) = (v["sprite"].as_object(), t.sprite.as_object_mut()) {
        base.extend(s.clone());
    }
    let Some(slots) = v["slots"].as_object() else { return };
    let canon = dir.canonicalize().ok();
    for (slot, file) in slots {
        let f = dir.join(file.as_str().unwrap_or_default());
        // Chi nhan file nam trong thu muc theme (chan "../").
        let inside = matches!((f.canonicalize(), &canon), (Ok(p), Some(d)) if p.starts_with(d));
        match inside.then(|| data_url(&f)).flatten() {
            Some(u) => {
                t.slots.insert(slot.clone(), u);
            }
            None => t.errors.push(format!("{slot}: khong doc duoc {}", file.as_str().unwrap_or_default())),
        }
    }
}

/// Theme mac dinh (nhung trong app) roi ghi de bang `<root>/themes/<name>` neu co.
pub fn load_theme(root: &Path, name: &str) -> Theme {
    let get = |f: &str| DEFAULT_FILES.iter().find(|(n, _)| *n == f).map(|(_, c)| *c);
    let base: Value = serde_json::from_str(get("theme.json").unwrap_or("{}")).unwrap_or_default();
    let mut t = Theme {
        name: name.into(),
        colors: base["colors"].as_object().cloned().unwrap_or_default(),
        font: base["font"].as_str().unwrap_or_default().into(),
        sprite: base["sprite"].clone(),
        slots: BTreeMap::new(),
        errors: vec![],
    };
    for (slot, file) in base["slots"].as_object().into_iter().flatten() {
        let f = file.as_str().unwrap_or_default();
        if let (Some(m), Some(c)) = (mime(f), get(f)) {
            t.slots.insert(slot.clone(), format!("data:{m};base64,{}", base64(c.as_bytes())));
        }
    }
    let dir = root.join("themes").join(name);
    match std::fs::read_to_string(dir.join("theme.json")) {
        Ok(s) => match serde_json::from_str::<Value>(&s) {
            Ok(v) => overlay(&mut t, &v, &dir),
            Err(e) => t.errors.push(format!("theme.json: {e}")),
        },
        Err(_) if name == "default" => {}
        Err(_) => t.errors.push(format!("khong co theme {name}, dung mac dinh")),
    }
    t
}

#[tauri::command]
pub fn theme_load(root: String, name: String) -> Theme {
    load_theme(Path::new(&root), &name)
}

/// Ten cac theme trong `<root>/themes`, luon co "default".
#[tauri::command]
pub fn theme_list(root: String) -> Vec<String> {
    let mut out = vec!["default".to_string()];
    for e in std::fs::read_dir(Path::new(&root).join("themes")).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if e.path().is_dir() && !out.contains(&n) {
            out.push(n);
        }
    }
    out[1..].sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cd-theme-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// PNG 1x1 hop le.
    const PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, 0x49, 0x48, 0x44, 0x52, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1F, 0x15, 0xC4,
        0x89, 0, 0, 0, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0, 1, 0, 0, 5, 0, 1, 0x0D, 0x0A, 0x2D, 0xB4, 0, 0, 0, 0, 0x49, 0x45, 0x4E, 0x44, 0xAE,
        0x42, 0x60, 0x82,
    ];

    #[test]
    fn theme_default_has_core_slots() {
        let root = tmp("default");
        let t = load_theme(&root, "default");
        assert!(t.errors.is_empty(), "{:?}", t.errors);
        for s in ["wall", "desk", "floor.wood", "floor.carpet", "decor.plant"] {
            assert!(t.slots[s].starts_with("data:image/svg+xml;base64,"), "{s}");
        }
        assert!(t.slots.keys().filter(|k| k.starts_with("character.") && k.ends_with(".typing")).count() >= 4);
        for c in ["bg", "room", "border", "text", "muted", "error", "shadow"] {
            assert!(t.colors[c].is_string(), "{c}"); // moi mau component dung deu phai co trong theme
        }
        assert_eq!(t.sprite["frame"], 32);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn theme_partial_override() {
        let root = tmp("partial");
        let d = root.join("themes/xanh");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("a.png"), PNG).unwrap();
        fs::write(d.join("theme.json"), r##"{"colors":{"bg":"#000011"},"slots":{"desk":"a.png"}}"##).unwrap();
        let t = load_theme(&root, "xanh");
        assert!(t.errors.is_empty(), "{:?}", t.errors);
        assert_eq!(t.name, "xanh");
        assert_eq!(t.colors["bg"], "#000011");
        assert!(t.colors["text"].is_string()); // mau khong ghi de van con
        assert!(t.slots["desk"].starts_with("data:image/png;base64,iVBOR"));
        assert!(t.slots["wall"].starts_with("data:image/svg+xml")); // slot khong ghi de lay mac dinh
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn theme_broken_falls_back() {
        let root = tmp("broken");
        fs::create_dir_all(root.join("themes/hong")).unwrap();
        fs::write(root.join("themes/hong/theme.json"), "{ hong").unwrap();
        let t = load_theme(&root, "hong");
        assert!(!t.errors.is_empty());
        assert!(t.slots.contains_key("desk"));
        let t = load_theme(&root, "khong-co");
        assert!(!t.errors.is_empty());
        assert!(t.slots.contains_key("desk"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn theme_rejects_path_escape() {
        let root = tmp("escape");
        fs::create_dir_all(root.join("themes/x")).unwrap();
        fs::write(root.join("secret.png"), PNG).unwrap();
        fs::write(root.join("themes/x/theme.json"), r#"{"slots":{"desk":"../../secret.png","wall":"missing.png"}}"#).unwrap();
        let t = load_theme(&root, "x");
        assert!(t.slots["desk"].starts_with("data:image/svg+xml")); // giu mac dinh
        assert!(t.slots["wall"].starts_with("data:image/svg+xml"));
        assert_eq!(t.errors.len(), 2, "{:?}", t.errors);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn data_url_png_and_svg() {
        let root = tmp("dataurl");
        fs::write(root.join("a.png"), PNG).unwrap();
        fs::write(root.join("b.svg"), "<svg/>").unwrap();
        fs::write(root.join("c.txt"), "x").unwrap();
        assert!(data_url(&root.join("a.png")).unwrap().starts_with("data:image/png;base64,iVBORw0KGgo"));
        assert_eq!(data_url(&root.join("b.svg")).unwrap(), "data:image/svg+xml;base64,PHN2Zy8+");
        assert!(data_url(&root.join("c.txt")).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn theme_list_always_has_default() {
        let root = tmp("list");
        assert_eq!(theme_list(root.to_string_lossy().into()), vec!["default"]);
        fs::create_dir_all(root.join("themes/xanh")).unwrap();
        fs::create_dir_all(root.join("themes/default")).unwrap();
        assert_eq!(theme_list(root.to_string_lossy().into()), vec!["default", "xanh"]);
        let _ = fs::remove_dir_all(&root);
    }
}
