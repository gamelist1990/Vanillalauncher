use crate::settings;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf, process::Command};

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ActiveLaunchRegistry {
    profiles: HashMap<String, u32>,
}

pub(super) fn is_profile_launch_active(profile_id: &str) -> bool {
    refresh_active_launch_registry()
        .map(|registry| registry.profiles.contains_key(profile_id))
        .unwrap_or(false)
}

pub(super) fn record_profile_launch(profile_id: &str, pid: u32) -> Result<(), String> {
    let mut registry = load_active_launch_registry()?;
    registry.profiles.insert(profile_id.to_string(), pid);
    save_active_launch_registry(&registry)
}

pub(super) fn clear_profile_launch(profile_id: &str, pid: u32) -> Result<(), String> {
    let mut registry = load_active_launch_registry()?;
    let should_remove = registry
        .profiles
        .get(profile_id)
        .copied()
        .map(|current| current == pid)
        .unwrap_or(false);

    if should_remove {
        registry.profiles.remove(profile_id);
        save_active_launch_registry(&registry)?;
    }

    Ok(())
}

fn launch_registry_path() -> PathBuf {
    settings::temp_root_dir().join("active-launches.json")
}

fn load_active_launch_registry() -> Result<ActiveLaunchRegistry, String> {
    let path = launch_registry_path();
    if !path.exists() {
        return Ok(ActiveLaunchRegistry::default());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("{} を解析できませんでした: {error}", path.display()))
}

fn save_active_launch_registry(registry: &ActiveLaunchRegistry) -> Result<(), String> {
    fs::create_dir_all(settings::temp_root_dir()).map_err(|error| {
        format!(
            "{} を準備できませんでした: {error}",
            settings::temp_root_dir().display()
        )
    })?;
    let path = launch_registry_path();
    let text = serde_json::to_string_pretty(registry)
        .map_err(|error| format!("起動状態を書き出しできませんでした: {error}"))?;
    fs::write(&path, text)
        .map_err(|error| format!("{} を保存できませんでした: {error}", path.display()))
}

fn is_process_running(pid: u32) -> bool {
    if cfg!(target_os = "windows") {
        let mut command = Command::new("tasklist");
        command.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW
            command.creation_flags(0x08000000);
        }

        let output = command.output();

        return output
            .ok()
            .map(|result| {
                let text = String::from_utf8_lossy(&result.stdout);
                text.contains(&format!(",\"{pid}\",")) || text.contains(&format!("\"{pid}\""))
            })
            .unwrap_or(false);
    }

    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn refresh_active_launch_registry() -> Result<ActiveLaunchRegistry, String> {
    let mut registry = load_active_launch_registry()?;
    let mut changed = false;

    registry.profiles.retain(|_, pid| {
        let keep = is_process_running(*pid);
        if !keep {
            changed = true;
        }
        keep
    });

    if changed {
        save_active_launch_registry(&registry)?;
    }

    Ok(registry)
}
