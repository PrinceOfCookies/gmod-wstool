use gma_lite::Builder;
use serde_json::Value;
use std::path::{Path, PathBuf};
use wildmatch::WildMatch;

use super::client;

const DEFAULT_IGNORES: &[&str] = &[
    "*thumbs.db",
    "*desktop.ini",
    ".git*",
    ".ds_store",
    "*/.ds_store",
];

pub fn pack_to_temp_dir(title: String, src_dir: &Path) -> Result<PathBuf, String> {
    if !src_dir.is_dir() {
        return Err("Content path must be an existing folder.".into());
    }

    let json_path = src_dir.join("addon.json");
    let user_ignores = if let Ok(raw) = std::fs::read_to_string(&json_path) {
        serde_json::from_str::<Value>(&raw)
            .ok()
            .and_then(|meta| meta.get("ignore")?.as_array().cloned())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_lowercase())
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let matchers: Vec<WildMatch> = DEFAULT_IGNORES
        .iter()
        .map(|s| s.to_string())
        .chain(user_ignores)
        .map(|p| WildMatch::new(&p))
        .collect();

    let steam_id64 = client().user().steam_id().raw() as i64;

    let mut builder = Builder::new(&title, steam_id64);
    builder.set_author(client().friends().name());

    let mut count = 0usize;
    collect_files(src_dir, src_dir, &matchers, &mut builder, &mut count)?;
    if count == 0 {
        return Err("No files to pack (everything was ignored or the folder is empty).".into());
    }

    // write the gma into its own dir so Steam uploads only this file
    let out_dir = std::env::temp_dir().join(format!("gmod-wstool-pack-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("Create temp dir failed: {e}"))?;

    let gma_path = out_dir.join("gmod-wstool.gma");
    let file = std::fs::File::create(&gma_path).map_err(|e| format!("Create gma failed: {e}"))?;
    builder
        .write_to(file)
        .map_err(|e| format!("Pack failed: {e}"))?;

    Ok(out_dir)
}

fn collect_files(
    root: &Path,
    dir: &Path,
    matchers: &[WildMatch],
    builder: &mut Builder,
    count: &mut usize,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Read dir failed: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let ft = entry
            .file_type()
            .map_err(|e| format!("File type failed: {e}"))?;
        if ft.is_dir() {
            collect_files(root, &path, matchers, builder, count)?;
        } else if ft.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| "path escaped root".to_string())?
                .to_string_lossy()
                .replace('\\', "/")
                .to_lowercase();

            // addon.json is metadata, never goes in the gma
            if rel == "addon.json" {
                continue;
            }
            if matchers.iter().any(|m| m.matches(&rel)) {
                continue;
            }
            if !crate::whitelist::is_allowed(&rel) {
                return Err(format!("File {rel} is not allowed, add it to ignore list"));
            }

            let bytes = std::fs::read(&path).map_err(|e| format!("Read {rel} failed: {e}"))?;
            builder.file_from_bytes(rel, bytes);
            *count += 1;
        }
    }
    Ok(())
}
