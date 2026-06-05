use egui::{ColorImage, TextureHandle};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use steamworks::Client;

pub mod callbacks;
pub mod download;
pub mod items;
pub mod pack;
mod state;
pub mod update;

use crate::ctx;
pub use state::get_state;

static CLIENT: OnceLock<Client> = OnceLock::new();
static BOOTSTRAP_STATUS: OnceLock<Mutex<String>> = OnceLock::new();
const GMOD_APP_ID: u32 = 4000;
const STEAM_BOOT_TIMEOUT: Duration = Duration::from_secs(45);
const STEAM_PROCESS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const STEAM_RETRY_INTERVAL: Duration = Duration::from_secs(2);
const STEAM_FORCE_LAUNCH_AFTER: Duration = Duration::from_secs(6);
const STEAM_POST_LAUNCH_SETTLE: Duration = Duration::from_secs(12);
const STEAM_DENIED_RETRY_INTERVAL: Duration = Duration::from_secs(8);

pub fn client() -> &'static Client {
    CLIENT.get().expect("Steam client not initialized")
}

pub fn bootstrap_status() -> String {
    BOOTSTRAP_STATUS
        .get_or_init(|| Mutex::new("Checking Steam...".into()))
        .lock()
        .unwrap()
        .clone()
}

pub fn init_client() -> Result<(), String> {
    if CLIENT.get().is_some() {
        set_bootstrap_status("Connected to Steam.");
        return Ok(());
    }

    if !steam_is_running() {
        return bootstrap_steam_and_retry("Steam is not running.".into());
    }

    match init_client_once() {
        Ok(()) => {
            set_bootstrap_status("Connected to Steam.");
            Ok(())
        }
        Err(initial_error) => bootstrap_steam_and_retry(initial_error),
    }
}

fn init_client_once() -> Result<(), String> {
    if CLIENT.get().is_none() {
        let client = Client::init_app(GMOD_APP_ID).map_err(|e| e.to_string())?;
        if CLIENT.set(client).is_ok() {
            state::init_state_events();
        }
    }
    Ok(())
}

fn bootstrap_steam_and_retry(initial_error: String) -> Result<(), String> {
    let mut last_error = initial_error;
    let mut launched_steam = false;
    let mut saw_running_process = steam_is_running();
    let mut process_seen_at = if saw_running_process {
        Some(Instant::now() - STEAM_POST_LAUNCH_SETTLE)
    } else {
        None
    };
    let mut next_retry_delay = STEAM_RETRY_INTERVAL;

    if !saw_running_process {
        set_bootstrap_status("Opening Steam...");
        launch_steam()?;
        launched_steam = true;
        next_retry_delay = STEAM_PROCESS_POLL_INTERVAL;
        set_bootstrap_status("Waiting for Steam to open...");
    } else {
        set_bootstrap_status("Waiting for Steam...");
    }

    let started_waiting = Instant::now();
    let deadline = started_waiting + STEAM_BOOT_TIMEOUT;
    while Instant::now() < deadline {
        if launched_steam && !saw_running_process {
            if steam_is_running() {
                saw_running_process = true;
                process_seen_at = Some(Instant::now());
                next_retry_delay = STEAM_PROCESS_POLL_INTERVAL;
                set_bootstrap_status("Steam opened. Waiting for sign-in...");
            } else {
                thread::sleep(STEAM_PROCESS_POLL_INTERVAL);
                continue;
            }
        }

        if let Some(seen_at) = process_seen_at {
            let settle_elapsed = seen_at.elapsed();
            if settle_elapsed < STEAM_POST_LAUNCH_SETTLE {
                set_bootstrap_status(status_message_for(&last_error, launched_steam));
                thread::sleep(
                    (STEAM_POST_LAUNCH_SETTLE - settle_elapsed).min(STEAM_PROCESS_POLL_INTERVAL),
                );
                continue;
            }
        }

        thread::sleep(next_retry_delay);
        match init_client_once() {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = error;
                next_retry_delay = retry_delay_for(&last_error);
                if !launched_steam && started_waiting.elapsed() >= STEAM_FORCE_LAUNCH_AFTER {
                    set_bootstrap_status("Opening Steam...");
                    launch_steam()?;
                    launched_steam = true;
                    saw_running_process = false;
                    process_seen_at = None;
                    next_retry_delay = STEAM_PROCESS_POLL_INTERVAL;
                    set_bootstrap_status("Waiting for Steam to open...");
                } else {
                    set_bootstrap_status(status_message_for(&last_error, launched_steam));
                }
                saw_running_process |= steam_is_running();
            }
        }
    }

    if launched_steam {
        Err(format!(
            "Steam was launched, but gmod-wstool could not initialize it within {} seconds.\n\nLast Steam error: {}\n\nIf Steam just opened, finish signing in first. Steam must be logged into an account that owns Garry's Mod.",
            STEAM_BOOT_TIMEOUT.as_secs(),
            last_error
        ))
    } else if saw_running_process {
        Err(format!(
            "Steam looked like it was already running, but gmod-wstool still could not initialize it within {} seconds.\n\nLast Steam error: {}\n\nMake sure Steam is fully signed in on the account that owns Garry's Mod, then retry.",
            STEAM_BOOT_TIMEOUT.as_secs(),
            last_error
        ))
    } else {
        Err(last_error)
    }
}

fn steam_is_running() -> bool {
    #[cfg(target_os = "linux")]
    {
        return command_succeeds("pgrep", &["-x", "steam"])
            || command_succeeds("pgrep", &["-f", "/Steam/ubuntu12_32/steam"])
            || command_succeeds("pgrep", &["-x", "steam.sh"]);
    }

    #[cfg(target_os = "windows")]
    {
        return Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq steam.exe"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains("steam.exe"))
            .unwrap_or(false);
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        false
    }
}

fn launch_steam() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        return try_launch(&[
            ("steam", &[]),
            ("/usr/bin/steam", &[]),
            ("xdg-open", &["steam://open/main"]),
            ("/usr/bin/steam", &["-silent"]),
            ("steam", &["-silent"]),
            ("flatpak", &["run", "com.valvesoftware.Steam"]),
        ]);
    }

    #[cfg(target_os = "windows")]
    {
        return try_launch(&[
            ("cmd", &["/C", "start", "", "steam://open/main"]),
            ("steam", &[]),
        ]);
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Err("Automatic Steam launch is not supported on this platform yet.".into())
    }
}

fn try_launch(candidates: &[(&str, &[&str])]) -> Result<(), String> {
    let mut errors = Vec::new();

    for (program, args) in candidates {
        match Command::new(program)
            .args(*args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => errors.push(format!("{program}: {error}")),
        }
    }

    if errors.is_empty() {
        Err("Steam is not running and no supported Steam launcher command was found.".into())
    } else {
        Err(format!(
            "Steam is not running and automatic launch failed: {}",
            errors.join(" | ")
        ))
    }
}

fn command_succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn retry_delay_for(error: &str) -> Duration {
    if error.to_ascii_lowercase().contains("denied appid") {
        STEAM_DENIED_RETRY_INTERVAL
    } else {
        STEAM_RETRY_INTERVAL
    }
}

fn status_message_for(error: &str, launched_steam: bool) -> &'static str {
    let lower = error.to_ascii_lowercase();
    if lower.contains("denied appid") {
        "Steam opened. Waiting for sign-in..."
    } else if launched_steam {
        "Waiting for Steam to finish starting..."
    } else {
        "Waiting for Steam..."
    }
}

fn set_bootstrap_status(message: impl Into<String>) {
    *BOOTSTRAP_STATUS
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .unwrap() = message.into();
}

pub struct NameAvatar {
    pub name: String,
    pub avatar: TextureHandle,
}

pub fn name_avatar() -> NameAvatar {
    let steam_id = client().user().steam_id();
    let friends = client().friends();
    let user = friends.get_friend(steam_id);

    let avatar = user
        .large_avatar()
        .map(|pixels| {
            // Steam large avatars are 184x184, RGBA format
            let size = 184;
            ColorImage::from_rgba_unmultiplied([size, size], &pixels)
        })
        .unwrap_or_else(|| ColorImage::example());

    let avatar = ctx().load_texture("steam_avatar", avatar, egui::TextureOptions::LINEAR);

    NameAvatar {
        name: friends.name(),
        avatar,
    }
}
