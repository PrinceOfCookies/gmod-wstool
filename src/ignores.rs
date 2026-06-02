use arc_swap::ArcSwap;
use std::sync::{Arc, LazyLock, Mutex};
use wildmatch::WildMatch;

pub const STORAGE_KEY: &str = "ignore_patterns";

static MATCHERS: LazyLock<ArcSwap<Vec<WildMatch>>> =
    LazyLock::new(|| ArcSwap::from_pointee(Vec::new()));
static LATEST: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

fn compile(patterns: &[String]) -> Vec<WildMatch> {
    patterns.iter().map(|p| WildMatch::new(p)).collect()
}

pub fn seed(stored: Option<String>) {
    let Some(raw) = stored else { return };
    let Ok(patterns) = serde_json::from_str::<Vec<String>>(&raw) else {
        return;
    };
    MATCHERS.store(Arc::new(compile(&patterns)));
    *LATEST.lock().unwrap() = patterns;
}

pub fn to_storage_string() -> String {
    let patterns = LATEST.lock().unwrap().clone();
    serde_json::to_string(&patterns).unwrap_or_default()
}

pub fn get() -> Vec<String> {
    LATEST.lock().unwrap().clone()
}

pub fn set(patterns: Vec<String>) {
    let cleaned: Vec<String> = patterns
        .into_iter()
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .collect();
    MATCHERS.store(Arc::new(compile(&cleaned)));
    *LATEST.lock().unwrap() = cleaned;
}
