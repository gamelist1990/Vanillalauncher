use crate::models::{
    ActiveLauncherAccount, InstalledMod, LauncherProfile, LauncherSnapshot, LauncherSummary,
};
use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::HashMap,
    env,
    ffi::OsStr,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::UNIX_EPOCH,
};
use toml::Value as TomlValue;
use zip::ZipArchive;

use crate::loaders::is_profile_launch_active;

const WINDOWS_STORE_AUMID: &str =
    "shell:AppsFolder\\Microsoft.4297127D64EC6_8wekyb3d8bbwe!Minecraft";

#[derive(Debug, Clone)]
pub struct CustomProfileDraft {
    pub name: String,
    pub icon: Option<String>,
    pub custom_icon_url: Option<String>,
    pub background_image_url: Option<String>,
    pub game_dir: PathBuf,
    pub last_version_id: String,
}

#[derive(Debug, Clone)]
struct ParsedModMetadata {
    display_name: Option<String>,
    mod_id: Option<String>,
    version: Option<String>,
    description: Option<String>,
    loader: Option<String>,
    authors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrackedModProject {
    pub project_id: String,
    pub file_name: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub curseforge_project_id: Option<u64>,
    #[serde(default)]
    pub curseforge_file_id: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct TrackedModSource {
    pub source: Option<String>,
    pub project_id: String,
    pub curseforge_project_id: Option<u64>,
    pub curseforge_file_id: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct LauncherAccount {
    pub username: Option<String>,
    pub gamer_tag: Option<String>,
    pub profile_id: Option<String>,
    pub access_token: Option<String>,
    pub access_token_expires_at: Option<String>,
    pub client_token: Option<String>,
    pub xuid: Option<String>,
    pub local_id: Option<String>,
    pub user_properties: Option<String>,
}

pub fn load_launcher_snapshot() -> Result<LauncherSnapshot, String> {
    let minecraft_root = minecraft_root()?;
    let profiles_json = read_launcher_profiles(&minecraft_root)?;
    let profiles_value = profiles_json
        .get("profiles")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "launcher_profiles.json の profiles セクションが読み取れません。".to_string()
        })?;

    let mut profiles = Vec::new();

    for (id, value) in profiles_value {
        let profile_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("custom")
            .to_string();
        let name = display_profile_name(
            id,
            value.get("name").and_then(Value::as_str),
            value.get("type").and_then(Value::as_str),
        );
        let icon = value
            .get("icon")
            .and_then(Value::as_str)
            .map(str::to_string);
        let custom_icon_url = value
            .get("customIcon")
            .and_then(Value::as_str)
            .map(str::to_string);
        let background_image_url = value
            .get("backgroundImage")
            .and_then(Value::as_str)
            .map(str::to_string);
        let last_used = value
            .get("lastUsed")
            .and_then(Value::as_str)
            .map(str::to_string);
        let last_version_id = value
            .get("lastVersionId")
            .and_then(Value::as_str)
            .map(str::to_string);
        let modpack_project_id = value
            .get("modpackProjectId")
            .and_then(Value::as_str)
            .map(str::to_string);
        let modpack_version_id = value
            .get("modpackVersionId")
            .and_then(Value::as_str)
            .map(str::to_string);
        let game_dir = resolve_game_dir(
            &minecraft_root,
            value.get("gameDir").and_then(Value::as_str),
        );
        let mods_dir = resolve_managed_mods_dir(&minecraft_root, id, &game_dir)?;
        let tracked_projects = tracked_project_map(&minecraft_root, id);
        let loader = infer_loader(last_version_id.as_deref(), Some(&name)).to_string();
        let loader_version = infer_loader_version(&loader, last_version_id.as_deref());
        let game_version = infer_game_version(last_version_id.as_deref(), Some(&name));
        let mods = read_mods(&mods_dir, Some(&loader), &tracked_projects)?;
        let enabled_mod_count = mods.iter().filter(|mod_file| mod_file.enabled).count();
        let disabled_mod_count = mods.len().saturating_sub(enabled_mod_count);

        profiles.push(LauncherProfile {
            id: id.to_string(),
            name,
            profile_type,
            icon,
            custom_icon_url,
            background_image_url,
            last_used,
            last_version_id,
            game_dir: game_dir.to_string_lossy().to_string(),
            game_version,
            loader,
            loader_version,
            modpack_project_id,
            modpack_version_id,
            launch_active: crate::loaders::is_profile_launch_active(id),
            mod_count: mods.len(),
            enabled_mod_count,
            disabled_mod_count,
            mods,
        });
    }

    profiles.sort_by(|left, right| {
        right
            .last_used
            .cmp(&left.last_used)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });

    let summary = LauncherSummary {
        profile_count: profiles.len(),
        mod_count: profiles.iter().map(|profile| profile.mod_count).sum(),
        enabled_mod_count: profiles
            .iter()
            .map(|profile| profile.enabled_mod_count)
            .sum(),
        disabled_mod_count: profiles
            .iter()
            .map(|profile| profile.disabled_mod_count)
            .sum(),
    };

    let active_account = read_active_launcher_account()?.and_then(|account| {
        account
            .gamer_tag
            .as_ref()
            .or(account.username.as_ref())
            .map(|name| ActiveLauncherAccount {
                username: name.clone(),
                auth_source: "official-launcher".to_string(),
            })
    });

    Ok(LauncherSnapshot {
        minecraft_root: minecraft_root.to_string_lossy().to_string(),
        launcher_available: launcher_available(),
        active_account,
        profiles,
        summary,
    })
}

pub fn find_profile(profile_id: &str) -> Result<LauncherProfile, String> {
    let snapshot = load_launcher_snapshot()?;
    snapshot
        .profiles
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| format!("起動構成 {profile_id} が見つかりません。"))
}

pub fn resolve_profile_path(profile_id: &str, target: &str) -> Result<String, String> {
    let profile = find_profile(profile_id)?;
    let path = match target {
        "game" => PathBuf::from(&profile.game_dir),
        "mods" => resolve_profile_mods_dir(profile_id, &profile.game_dir)?,
        _ => return Err("開く対象が不明です。".to_string()),
    };

    fs::create_dir_all(&path)
        .map_err(|error| format!("{} を準備できませんでした: {error}", path.display()))?;

    Ok(path.to_string_lossy().to_string())
}

pub fn read_active_launcher_account() -> Result<Option<LauncherAccount>, String> {
    let root = minecraft_root()?;
    for file_name in [
        "launcher_accounts_microsoft_store.json",
        "launcher_accounts.json",
    ] {
        let path = root.join(file_name);
        if !path.exists() {
            continue;
        }

        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("{} を読み込めませんでした: {error}", path.display()))?;
        let value: Value = serde_json::from_str(&contents)
            .map_err(|error| format!("{} を解析できませんでした: {error}", path.display()))?;
        let Some(active_account_id) = value
            .get("activeAccountLocalId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(account) = value
            .get("accounts")
            .and_then(Value::as_object)
            .and_then(|accounts| accounts.get(active_account_id))
            .and_then(Value::as_object)
        else {
            continue;
        };

        let gamer_tag = account
            .get("minecraftProfile")
            .and_then(Value::as_object)
            .and_then(|profile| profile.get("name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let profile_id = account
            .get("minecraftProfile")
            .and_then(Value::as_object)
            .and_then(|profile| profile.get("id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let user_properties = account.get("userProperites").and_then(|value| {
            if value.is_null() {
                None
            } else {
                serde_json::to_string(value).ok()
            }
        });

        return Ok(Some(LauncherAccount {
            username: account
                .get("username")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            gamer_tag,
            profile_id,
            access_token: account
                .get("accessToken")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            access_token_expires_at: account
                .get("accessTokenExpiresAt")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            client_token: value
                .get("mojangClientToken")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            xuid: account
                .get("remoteId")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            local_id: account
                .get("localId")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            user_properties,
        }));
    }

    Ok(None)
}

pub fn resolve_profile_mods_dir(profile_id: &str, game_dir: &str) -> Result<PathBuf, String> {
    let root = minecraft_root()?;
    resolve_managed_mods_dir(&root, profile_id, Path::new(game_dir))
}

pub fn sync_profile_mods_to_game_dir(profile_id: &str) -> Result<(), String> {
    let profile = find_profile(profile_id)?;
    let root = minecraft_root()?;
    let managed_mods_dir =
        resolve_managed_mods_dir(&root, profile_id, Path::new(&profile.game_dir))?;
    let target_mods_dir = Path::new(&profile.game_dir).join("mods");
    fs::create_dir_all(&target_mods_dir).map_err(|error| {
        format!(
            "{} を準備できませんでした: {error}",
            target_mods_dir.display()
        )
    })?;

    if managed_mods_dir == target_mods_dir {
        return Ok(());
    }

    let entries = fs::read_dir(&target_mods_dir).map_err(|error| {
        format!(
            "{} を読み込めませんでした: {error}",
            target_mods_dir.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Mod ファイルの確認に失敗しました: {error}"))?;
        let path = entry.path();
        if path.is_file() && looks_like_mod_file(&path) {
            fs::remove_file(&path)
                .map_err(|error| format!("{} を削除できませんでした: {error}", path.display()))?;
        }
    }

    let managed_entries = fs::read_dir(&managed_mods_dir).map_err(|error| {
        format!(
            "{} を読み込めませんでした: {error}",
            managed_mods_dir.display()
        )
    })?;
    for entry in managed_entries {
        let entry = entry.map_err(|error| format!("Mod ファイルの確認に失敗しました: {error}"))?;
        let source_path = entry.path();
        if !source_path.is_file() || !looks_like_mod_file(&source_path) {
            continue;
        }

        let file_name = source_path
            .file_name()
            .ok_or_else(|| format!("{} のファイル名を解釈できません。", source_path.display()))?;
        fs::copy(&source_path, target_mods_dir.join(file_name)).map_err(|error| {
            format!(
                "{} を {} へ同期できませんでした: {error}",
                source_path.display(),
                target_mods_dir.display()
            )
        })?;
    }

    Ok(())
}

pub fn track_installed_project(
    profile_id: &str,
    project_id: &str,
    file_name: &str,
) -> Result<(), String> {
    track_installed_project_with_source(
        profile_id,
        TrackedModSource {
            source: Some("modrinth".to_string()),
            project_id: project_id.to_string(),
            curseforge_project_id: None,
            curseforge_file_id: None,
        },
        file_name,
    )
}

pub fn track_installed_project_with_source(
    profile_id: &str,
    source: TrackedModSource,
    file_name: &str,
) -> Result<(), String> {
    validate_file_name(file_name)?;
    let root = minecraft_root()?;
    let mut entries = read_tracked_projects(&root, profile_id)?;

    if let Some(existing) = entries
        .iter_mut()
        .find(|entry| {
            entry.file_name == file_name
                || (!source.project_id.is_empty() && entry.project_id == source.project_id)
                || (source.curseforge_project_id.is_some()
                    && source.curseforge_file_id.is_some()
                    && entry.curseforge_project_id == source.curseforge_project_id
                    && entry.curseforge_file_id == source.curseforge_file_id)
        })
    {
        existing.project_id = source.project_id;
        existing.file_name = file_name.to_string();
        existing.source = source.source;
        existing.curseforge_project_id = source.curseforge_project_id;
        existing.curseforge_file_id = source.curseforge_file_id;
    } else {
        entries.push(TrackedModProject {
            project_id: source.project_id,
            file_name: file_name.to_string(),
            source: source.source,
            curseforge_project_id: source.curseforge_project_id,
            curseforge_file_id: source.curseforge_file_id,
        });
    }

    write_tracked_projects(&root, profile_id, &entries)
}

pub fn rename_tracked_project_file(
    profile_id: &str,
    current_file_name: &str,
    next_file_name: &str,
) -> Result<(), String> {
    validate_file_name(current_file_name)?;
    validate_file_name(next_file_name)?;
    let root = minecraft_root()?;
    let mut entries = read_tracked_projects(&root, profile_id)?;
    let mut changed = false;

    for entry in &mut entries {
        if entry.file_name == current_file_name {
            entry.file_name = next_file_name.to_string();
            changed = true;
        }
    }

    if changed {
        write_tracked_projects(&root, profile_id, &entries)?;
    }

    Ok(())
}

pub fn remove_tracked_project_by_file(profile_id: &str, file_name: &str) -> Result<(), String> {
    validate_file_name(file_name)?;
    let root = minecraft_root()?;
    let entries = read_tracked_projects(&root, profile_id)?;
    let original_len = entries.len();
    let filtered: Vec<_> = entries
        .into_iter()
        .filter(|entry| entry.file_name != file_name)
        .collect();

    if filtered.len() != original_len {
        write_tracked_projects(&root, profile_id, &filtered)?;
    }

    Ok(())
}

pub fn remove_tracked_project_by_project(
    profile_id: &str,
    project_id: &str,
) -> Result<Option<String>, String> {
    let root = minecraft_root()?;
    let entries = read_tracked_projects(&root, profile_id)?;
    let original_len = entries.len();
    let removed_file = entries
        .iter()
        .find(|entry| entry.project_id == project_id)
        .map(|entry| entry.file_name.clone());
    let filtered: Vec<_> = entries
        .into_iter()
        .filter(|entry| entry.project_id != project_id)
        .collect();

    if filtered.len() != original_len {
        write_tracked_projects(&root, profile_id, &filtered)?;
    }

    Ok(removed_file)
}

fn profile_storage_dir(minecraft_root: &Path, profile_id: &str) -> PathBuf {
    minecraft_root
        .join(".vanillalauncher")
        .join("profiles")
        .join(sanitize_profile_id(profile_id))
}

pub fn profile_instance_dir(minecraft_root: &Path, loader: &str, profile_name: &str) -> PathBuf {
    minecraft_root
        .join(".vanillalauncher")
        .join("instances")
        .join(normalize_loader(Some(loader)))
        .join(sanitize_profile_id(profile_name))
}

fn tracked_projects_path(minecraft_root: &Path, profile_id: &str) -> PathBuf {
    profile_storage_dir(minecraft_root, profile_id).join("modrinth-projects.json")
}

fn legacy_managed_mods_dir(minecraft_root: &Path, profile_id: &str) -> PathBuf {
    profile_storage_dir(minecraft_root, profile_id).join("mods")
}

fn resolve_managed_mods_dir(
    minecraft_root: &Path,
    profile_id: &str,
    game_dir: &Path,
) -> Result<PathBuf, String> {
    let managed_dir = game_dir.join("mods");
    fs::create_dir_all(&managed_dir)
        .map_err(|error| format!("{} を準備できませんでした: {error}", managed_dir.display()))?;
    seed_mods_from_legacy_dir(
        &legacy_managed_mods_dir(minecraft_root, profile_id),
        &managed_dir,
    )?;
    seed_profile_tracking_from_game_dir(
        game_dir,
        &tracked_projects_path(minecraft_root, profile_id),
    )?;
    Ok(managed_dir)
}

fn seed_mods_from_legacy_dir(legacy_mods_dir: &Path, managed_dir: &Path) -> Result<(), String> {
    if !legacy_mods_dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(&legacy_mods_dir).map_err(|error| {
        format!(
            "{} を読み込めませんでした: {error}",
            legacy_mods_dir.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Mod ファイルの確認に失敗しました: {error}"))?;
        let source_path = entry.path();
        if !source_path.is_file() || !looks_like_mod_file(&source_path) {
            continue;
        }

        let file_name = source_path
            .file_name()
            .ok_or_else(|| format!("{} のファイル名を解釈できません。", source_path.display()))?;
        let target_path = managed_dir.join(file_name);
        if target_path.exists() {
            continue;
        }
        fs::copy(&source_path, &target_path).map_err(|error| {
            format!(
                "{} を {} へコピーできませんでした: {error}",
                source_path.display(),
                target_path.display()
            )
        })?;
    }

    Ok(())
}

fn seed_profile_tracking_from_game_dir(game_dir: &Path, tracked_path: &Path) -> Result<(), String> {
    if tracked_path.exists() {
        return Ok(());
    }

    let legacy_path = game_dir
        .join(".vanillalauncher")
        .join("modrinth-projects.json");
    if !legacy_path.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(&legacy_path)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", legacy_path.display()))?;
    if let Some(parent) = tracked_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }
    fs::write(tracked_path, contents)
        .map_err(|error| format!("{} を保存できませんでした: {error}", tracked_path.display()))
}

fn read_tracked_projects(
    minecraft_root: &Path,
    profile_id: &str,
) -> Result<Vec<TrackedModProject>, String> {
    let path = tracked_projects_path(minecraft_root, profile_id);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("{} を解析できませんでした: {error}", path.display()))
}

fn write_tracked_projects(
    minecraft_root: &Path,
    profile_id: &str,
    entries: &[TrackedModProject],
) -> Result<(), String> {
    let path = tracked_projects_path(minecraft_root, profile_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }

    let serialized = serde_json::to_string_pretty(entries)
        .map_err(|error| format!("Mod 管理情報を保存形式に変換できませんでした: {error}"))?;
    fs::write(&path, serialized)
        .map_err(|error| format!("{} を保存できませんでした: {error}", path.display()))
}

fn tracked_project_map(minecraft_root: &Path, profile_id: &str) -> HashMap<String, String> {
    read_tracked_projects(minecraft_root, profile_id)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (entry.file_name, entry.project_id))
        .collect()
}

pub fn tracked_project_entries(profile_id: &str) -> Result<Vec<TrackedModProject>, String> {
    let root = minecraft_root()?;
    read_tracked_projects(&root, profile_id)
}

pub fn minecraft_root() -> Result<PathBuf, String> {
    if cfg!(target_os = "windows") {
        return env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join(".minecraft"))
            .ok_or_else(|| "APPDATA が設定されていません。".to_string());
    }

    if cfg!(target_os = "macos") {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| {
                path.join("Library")
                    .join("Application Support")
                    .join("minecraft")
            })
            .ok_or_else(|| "HOME が設定されていません。".to_string());
    }

    env::var_os("HOME")
        .map(PathBuf::from)
        .map(|path| path.join(".minecraft"))
        .ok_or_else(|| "HOME が設定されていません。".to_string())
}

pub fn read_launcher_profiles(minecraft_root: &Path) -> Result<Value, String> {
    let launcher_profiles_path = minecraft_root.join("launcher_profiles.json");
    if !launcher_profiles_path.exists() {
        return Ok(serde_json::json!({ "profiles": {}, "settings": {}, "version": 6 }));
    }

    let contents = fs::read_to_string(&launcher_profiles_path).map_err(|error| {
        format!(
            "{} を読み込めませんでした: {error}",
            launcher_profiles_path.display()
        )
    })?;

    serde_json::from_str(&contents).map_err(|error| {
        format!(
            "{} を解析できませんでした: {error}",
            launcher_profiles_path.display()
        )
    })
}

pub fn write_launcher_profiles(minecraft_root: &Path, profiles: &Value) -> Result<(), String> {
    let launcher_profiles_path = minecraft_root.join("launcher_profiles.json");
    let serialized = serde_json::to_string_pretty(profiles).map_err(|error| {
        format!("launcher_profiles.json を保存形式に変換できませんでした: {error}")
    })?;

    fs::write(&launcher_profiles_path, serialized).map_err(|error| {
        format!(
            "{} を保存できませんでした: {error}",
            launcher_profiles_path.display()
        )
    })
}

pub fn ensure_launcher_profiles_file(minecraft_root: &Path) -> Result<(), String> {
    fs::create_dir_all(minecraft_root)
        .map_err(|error| format!("Minecraft ディレクトリを準備できませんでした: {error}"))?;

    let launcher_profiles_path = minecraft_root.join("launcher_profiles.json");
    if !launcher_profiles_path.exists() {
        let empty = serde_json::json!({ "profiles": {}, "settings": {}, "version": 6 });
        write_launcher_profiles(minecraft_root, &empty)?;
    }

    Ok(())
}

pub fn set_profile_last_used(profile_id: &str) -> Result<(), String> {
    let minecraft_root = minecraft_root()?;
    let mut profiles_json = read_launcher_profiles(&minecraft_root)?;
    let profiles_object = profiles_json
        .get_mut("profiles")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            "launcher_profiles.json の profiles セクションが壊れています。".to_string()
        })?;
    let profile = profiles_object
        .get_mut(profile_id)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| format!("起動構成 {profile_id} が launcher_profiles.json にありません。"))?;

    profile.insert(
        "lastUsed".to_string(),
        Value::String(now_timestamp_string()),
    );
    write_launcher_profiles(&minecraft_root, &profiles_json)
}

pub fn delete_custom_profile(profile_id: &str) -> Result<(), String> {
    if is_profile_launch_active(profile_id) {
        return Err("この起動構成はまだ起動中なので削除できません。".to_string());
    }

    let minecraft_root = minecraft_root()?;
    let mut profiles_json = read_launcher_profiles(&minecraft_root)?;
    let profiles_object = profiles_json
        .get_mut("profiles")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            "launcher_profiles.json の profiles セクションが壊れています。".to_string()
        })?;

    let removed = profiles_object.remove(profile_id);
    let Some(removed_profile) = removed else {
        return Err(format!("起動構成 {profile_id} が見つかりません。"));
    };

    let removed_profile_object = removed_profile.as_object();
    let removed_profile_name = removed_profile_object
        .and_then(|profile| profile.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let removed_last_version_id = removed_profile_object
        .and_then(|profile| profile.get("lastVersionId"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let removed_game_dir = removed_profile_object
        .and_then(|profile| profile.get("gameDir"))
        .and_then(Value::as_str)
        .map(|value| resolve_game_dir(&minecraft_root, Some(value)));

    write_launcher_profiles(&minecraft_root, &profiles_json)?;

    let storage_dir = profile_storage_dir(&minecraft_root, profile_id);
    if storage_dir.exists() {
        fs::remove_dir_all(&storage_dir).map_err(|error| {
            format!("{} を削除できませんでした: {error}", storage_dir.display())
        })?;
    }

    if let Some(game_dir) = removed_game_dir {
        let app_managed_root = minecraft_root.join(".vanillalauncher");
        if game_dir.starts_with(&app_managed_root) && game_dir.exists() {
            fs::remove_dir_all(&game_dir).map_err(|error| {
                format!("{} を削除できませんでした: {error}", game_dir.display())
            })?;
        }
    }

    cleanup_managed_instance_dirs(
        &minecraft_root,
        profile_id,
        removed_profile_name.as_deref(),
        removed_last_version_id.as_deref(),
    )?;

    Ok(())
}

fn cleanup_managed_instance_dirs(
    minecraft_root: &Path,
    profile_id: &str,
    profile_name: Option<&str>,
    last_version_id: Option<&str>,
) -> Result<(), String> {
    let instances_root = minecraft_root.join(".vanillalauncher").join("instances");
    if !instances_root.exists() {
        return Ok(());
    }

    let mut candidates: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    if let Some(name) = profile_name {
        let loader = infer_loader(last_version_id, Some(name));
        candidates.insert(profile_instance_dir(minecraft_root, loader, name));
    }

    for loader in ["fabric", "forge", "neoforge", "quilt", "vanilla"] {
        candidates.insert(
            instances_root
                .join(loader)
                .join(sanitize_profile_id(profile_id)),
        );
    }

    for candidate in candidates {
        if candidate.starts_with(&instances_root) && candidate.exists() {
            fs::remove_dir_all(&candidate).map_err(|error| {
                format!("{} を削除できませんでした: {error}", candidate.display())
            })?;
        }
    }

    Ok(())
}

pub fn update_profile_visuals(
    profile_id: &str,
    custom_icon_url: Option<String>,
    background_image_url: Option<String>,
) -> Result<(), String> {
    let minecraft_root = minecraft_root()?;
    let mut profiles_json = read_launcher_profiles(&minecraft_root)?;
    let profiles_object = profiles_json
        .get_mut("profiles")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            "launcher_profiles.json の profiles セクションが壊れています。".to_string()
        })?;
    let profile = profiles_object
        .get_mut(profile_id)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| format!("起動構成 {profile_id} が launcher_profiles.json にありません。"))?;

    match custom_icon_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            profile.insert("customIcon".to_string(), Value::String(value.to_string()));
        }
        None => {
            profile.remove("customIcon");
        }
    }

    match background_image_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            profile.insert(
                "backgroundImage".to_string(),
                Value::String(value.to_string()),
            );
        }
        None => {
            profile.remove("backgroundImage");
        }
    }

    write_launcher_profiles(&minecraft_root, &profiles_json)
}

pub fn update_profile_name(profile_id: &str, profile_name: &str) -> Result<(), String> {
    let next_name = profile_name.trim();
    if next_name.is_empty() {
        return Err("起動構成名は空欄にできません。".to_string());
    }

    let minecraft_root = minecraft_root()?;
    let mut profiles_json = read_launcher_profiles(&minecraft_root)?;
    let profiles_object = profiles_json
        .get_mut("profiles")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            "launcher_profiles.json の profiles セクションが壊れています。".to_string()
        })?;
    let profile = profiles_object
        .get_mut(profile_id)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| format!("起動構成 {profile_id} が launcher_profiles.json にありません。"))?;

    profile.insert("name".to_string(), Value::String(next_name.to_string()));

    write_launcher_profiles(&minecraft_root, &profiles_json)
}

pub fn update_profile_runtime_and_modpack(
    profile_id: &str,
    last_version_id: &str,
    modpack_project_id: Option<&str>,
    modpack_version_id: Option<&str>,
) -> Result<(), String> {
    let minecraft_root = minecraft_root()?;
    let mut profiles_json = read_launcher_profiles(&minecraft_root)?;
    let profiles_object = profiles_json
        .get_mut("profiles")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            "launcher_profiles.json の profiles セクションが壊れています。".to_string()
        })?;
    let profile = profiles_object
        .get_mut(profile_id)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| format!("起動構成 {profile_id} が launcher_profiles.json にありません。"))?;

    profile.insert(
        "lastVersionId".to_string(),
        Value::String(last_version_id.trim().to_string()),
    );

    match modpack_project_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            profile.insert(
                "modpackProjectId".to_string(),
                Value::String(value.to_string()),
            );
        }
        None => {
            profile.remove("modpackProjectId");
        }
    }

    match modpack_version_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            profile.insert(
                "modpackVersionId".to_string(),
                Value::String(value.to_string()),
            );
        }
        None => {
            profile.remove("modpackVersionId");
        }
    }

    write_launcher_profiles(&minecraft_root, &profiles_json)
}

pub fn set_java_page_as_last_visited() -> Result<(), String> {
    let root = minecraft_root()?;
    let ui_state_path = root.join("launcher_ui_state_microsoft_store.json");
    if !ui_state_path.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(&ui_state_path).map_err(|error| {
        format!(
            "{} を読み込めませんでした: {error}",
            ui_state_path.display()
        )
    })?;
    let mut value: Value = serde_json::from_str(&contents).map_err(|error| {
        format!(
            "{} を解析できませんでした: {error}",
            ui_state_path.display()
        )
    })?;

    let settings_slot = value
        .get_mut("data")
        .and_then(Value::as_object_mut)
        .and_then(|data| data.get_mut("UiSettings"));

    let Some(settings_slot) = settings_slot else {
        return Ok(());
    };

    let Some(settings_text) = settings_slot.as_str() else {
        return Ok(());
    };

    let mut settings: Value = serde_json::from_str(settings_text)
        .map_err(|error| format!("UiSettings を解析できませんでした: {error}"))?;

    if let Some(settings_object) = settings.as_object_mut() {
        settings_object.insert(
            "lastVisitedPage".to_string(),
            Value::String("java".to_string()),
        );
    }

    *settings_slot = Value::String(
        serde_json::to_string(&settings)
            .map_err(|error| format!("UiSettings を保存形式に変換できませんでした: {error}"))?,
    );

    let serialized = serde_json::to_string_pretty(&value)
        .map_err(|error| format!("UI 状態を保存形式に変換できませんでした: {error}"))?;

    fs::write(&ui_state_path, serialized).map_err(|error| {
        format!(
            "{} を保存できませんでした: {error}",
            ui_state_path.display()
        )
    })
}

pub fn open_official_launcher() -> Result<String, String> {
    if cfg!(target_os = "windows") {
        for path in legacy_launcher_candidates() {
            if path.exists() {
                std::process::Command::new(&path).spawn().map_err(|error| {
                    format!("{} を起動できませんでした: {error}", path.display())
                })?;
                return Ok("legacy".to_string());
            }
        }

        std::process::Command::new("explorer.exe")
            .arg(WINDOWS_STORE_AUMID)
            .spawn()
            .map_err(|error| format!("Minecraft Launcher を起動できませんでした: {error}"))?;
        return Ok("windows-store".to_string());
    }

    if cfg!(target_os = "macos") {
        std::process::Command::new("open")
            .arg("-a")
            .arg("Minecraft Launcher")
            .spawn()
            .map_err(|error| format!("Minecraft Launcher を起動できませんでした: {error}"))?;
        return Ok("macos".to_string());
    }

    std::process::Command::new("minecraft-launcher")
        .spawn()
        .map_err(|error| format!("minecraft-launcher を起動できませんでした: {error}"))?;
    Ok("linux".to_string())
}

pub fn launcher_available() -> bool {
    if cfg!(target_os = "windows") {
        return true;
    }

    if cfg!(target_os = "macos") {
        return std::process::Command::new("open")
            .arg("-Ra")
            .arg("Minecraft Launcher")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
    }

    std::process::Command::new("sh")
        .arg("-lc")
        .arg("command -v minecraft-launcher")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn upsert_custom_profile(draft: CustomProfileDraft) -> Result<String, String> {
    let minecraft_root = minecraft_root()?;
    ensure_launcher_profiles_file(&minecraft_root)?;
    let mut profiles_json = read_launcher_profiles(&minecraft_root)?;
    let profiles_object = profiles_json
        .get_mut("profiles")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            "launcher_profiles.json の profiles セクションが壊れています。".to_string()
        })?;

    let existing_profile_id = profiles_object.iter().find_map(|(profile_id, value)| {
        let profile = value.as_object()?;
        let last_version_matches = profile
            .get("lastVersionId")
            .and_then(Value::as_str)
            .map(|last_version_id| last_version_id == draft.last_version_id)
            .unwrap_or(false);
        let game_dir_matches = resolve_game_dir(
            &minecraft_root,
            profile.get("gameDir").and_then(Value::as_str),
        ) == draft.game_dir;

        if last_version_matches && game_dir_matches {
            Some(profile_id.to_string())
        } else {
            None
        }
    });

    let profile_id = existing_profile_id.unwrap_or_else(|| {
        let base = sanitize_profile_id(&draft.name);
        let timestamp = Local::now().timestamp_millis();
        format!("{base}-{timestamp}")
    });
    let existing_profile = profiles_object
        .get(&profile_id)
        .and_then(Value::as_object)
        .cloned();

    let now = now_timestamp_string();
    let mut profile = Map::new();
    profile.insert("created".to_string(), Value::String(now.clone()));
    profile.insert(
        "icon".to_string(),
        Value::String(draft.icon.unwrap_or_else(|| "Grass".to_string())),
    );
    profile.insert("lastUsed".to_string(), Value::String(now));
    profile.insert(
        "lastVersionId".to_string(),
        Value::String(draft.last_version_id),
    );
    profile.insert("name".to_string(), Value::String(draft.name));
    profile.insert("type".to_string(), Value::String("custom".to_string()));
    if let Some(background_image_url) = draft.background_image_url {
        profile.insert(
            "backgroundImage".to_string(),
            Value::String(background_image_url),
        );
    } else if let Some(existing_profile) = existing_profile
        .as_ref()
        .and_then(|value| value.get("backgroundImage"))
        .cloned()
    {
        profile.insert("backgroundImage".to_string(), existing_profile);
    }
    if let Some(custom_icon_url) = draft.custom_icon_url {
        profile.insert("customIcon".to_string(), Value::String(custom_icon_url));
    } else if let Some(existing_profile) = existing_profile
        .as_ref()
        .and_then(|value| value.get("customIcon"))
        .cloned()
    {
        profile.insert("customIcon".to_string(), existing_profile);
    }

    if draft.game_dir != minecraft_root {
        profile.insert(
            "gameDir".to_string(),
            Value::String(draft.game_dir.to_string_lossy().to_string()),
        );
    }

    profiles_object.insert(profile_id.clone(), Value::Object(profile));
    write_launcher_profiles(&minecraft_root, &profiles_json)?;
    seed_profile_preferences_from_vanilla(&minecraft_root, &draft.game_dir)?;
    Ok(profile_id)
}

fn seed_profile_preferences_from_vanilla(
    minecraft_root: &Path,
    game_dir: &Path,
) -> Result<(), String> {
    if game_dir == minecraft_root {
        return Ok(());
    }

    fs::create_dir_all(game_dir)
        .map_err(|error| format!("{} を準備できませんでした: {error}", game_dir.display()))?;

    let source_options = vanilla_options_path(minecraft_root);
    if !source_options.exists() {
        return Ok(());
    }

    let source_contents = fs::read_to_string(&source_options).map_err(|error| {
        format!(
            "{} を読み込めませんでした: {error}",
            source_options.display()
        )
    })?;
    let target_options = game_dir.join("options.txt");
    let target_contents = if target_options.exists() {
        fs::read_to_string(&target_options).map_err(|error| {
            format!(
                "{} を読み込めませんでした: {error}",
                target_options.display()
            )
        })?
    } else {
        String::new()
    };

    let merged_contents = merge_vanilla_options(&target_contents, &source_contents);
    if merged_contents != target_contents {
        fs::write(&target_options, merged_contents).map_err(|error| {
            format!(
                "{} を更新できませんでした: {error}",
                target_options.display()
            )
        })?;
    }

    Ok(())
}

fn vanilla_options_path(minecraft_root: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        if let Some(appdata) = env::var_os("APPDATA") {
            return PathBuf::from(appdata).join(".minecraft").join("options.txt");
        }
    }

    minecraft_root.join("options.txt")
}

fn merge_vanilla_options(existing_contents: &str, vanilla_contents: &str) -> String {
    let mut seed_values = vanilla_contents
        .lines()
        .filter_map(parse_option_line)
        .filter(|(key, _)| should_seed_option_key(key))
        .collect::<HashMap<_, _>>();

    if seed_values.is_empty() {
        return existing_contents.to_string();
    }

    let mut merged_lines = Vec::new();
    let mut changed = false;

    for line in existing_contents.lines() {
        if let Some((key, _)) = parse_option_line(line) {
            if should_seed_option_key(&key) {
                if let Some(new_value) = seed_values.remove(&key) {
                    let replacement = format!("{key}:{new_value}");
                    if line.trim() != replacement {
                        changed = true;
                    }
                    merged_lines.push(replacement);
                    continue;
                }
            }
        }

        merged_lines.push(line.to_string());
    }

    if !seed_values.is_empty() {
        changed = true;
        for (key, value) in seed_values {
            merged_lines.push(format!("{key}:{value}"));
        }
    }

    if !changed {
        return existing_contents.to_string();
    }

    let mut merged = merged_lines.join("\n");
    merged.push('\n');
    merged
}

fn parse_option_line(line: &str) -> Option<(String, String)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim();
    if key.is_empty() {
        return None;
    }

    Some((key.to_string(), value.trim().to_string()))
}

fn should_seed_option_key(key: &str) -> bool {
    key.starts_with("key_")
        || key.starts_with("mouse")
        || key.starts_with("soundCategory_")
        || matches!(
            key,
            "lang"
                | "fov"
                | "fovEffectScale"
        )
}

pub fn normalize_loader(loader: Option<&str>) -> &'static str {
    match loader.unwrap_or_default().to_lowercase().as_str() {
        "fabric" => "fabric",
        "forge" => "forge",
        "neoforge" => "neoforge",
        "quilt" => "quilt",
        _ => "vanilla",
    }
}

pub fn infer_loader(last_version_id: Option<&str>, profile_name: Option<&str>) -> &'static str {
    let source = format!(
        "{} {}",
        last_version_id.unwrap_or_default().to_lowercase(),
        profile_name.unwrap_or_default().to_lowercase()
    );

    if source.contains("neoforge") {
        "neoforge"
    } else if source.contains("fabric") {
        "fabric"
    } else if source.contains("quilt") {
        "quilt"
    } else if source.contains("forge") {
        "forge"
    } else {
        "vanilla"
    }
}

pub fn infer_loader_version(loader: &str, last_version_id: Option<&str>) -> Option<String> {
    let value = last_version_id?.to_lowercase();

    match loader {
        "fabric" => value
            .strip_prefix("fabric-loader-")
            .and_then(|rest| rest.split_once('-').map(|(version, _)| version.to_string())),
        "quilt" => value
            .strip_prefix("quilt-loader-")
            .and_then(|rest| rest.split_once('-').map(|(version, _)| version.to_string())),
        "forge" => {
            if let Some((_, version)) = value.split_once("-forge-") {
                Some(version.to_string())
            } else {
                value.strip_prefix("forge-").map(str::to_string)
            }
        }
        "neoforge" => {
            if let Some((_, version)) = value.split_once("-neoforge-") {
                Some(version.to_string())
            } else {
                value.strip_prefix("neoforge-").map(str::to_string)
            }
        }
        _ => None,
    }
}

pub fn infer_game_version(
    last_version_id: Option<&str>,
    profile_name: Option<&str>,
) -> Option<String> {
    if let Some(value) = last_version_id {
        if let Some(version) = extract_game_version_from_version_id(value) {
            return Some(version);
        }
    }

    profile_name.and_then(extract_game_version_from_text)
}

pub fn resolve_game_dir(minecraft_root: &Path, configured_path: Option<&str>) -> PathBuf {
    let Some(configured_path) = configured_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return minecraft_root.to_path_buf();
    };

    let candidate = PathBuf::from(configured_path);
    if candidate.is_absolute() {
        candidate
    } else {
        minecraft_root.join(candidate)
    }
}

pub fn validate_file_name(file_name: &str) -> Result<(), String> {
    if file_name.trim().is_empty() {
        return Err("Mod ファイル名が空です。".to_string());
    }

    let path = Path::new(file_name);
    if path.components().count() != 1 {
        return Err("Mod 操作ではネストされたパスを使えません。".to_string());
    }

    Ok(())
}

pub fn is_mod_archive(file_name: &str) -> bool {
    file_name.ends_with(".jar")
}

pub fn display_name_for_file(file_name: &str) -> String {
    file_name
        .trim_end_matches(".disabled")
        .trim_end_matches(".jar")
        .replace(['_', '-'], " ")
}

pub fn now_timestamp_string() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%z").to_string()
}

fn read_mods(
    mods_dir: &Path,
    profile_loader: Option<&str>,
    tracked_projects: &HashMap<String, String>,
) -> Result<Vec<InstalledMod>, String> {
    if !mods_dir.exists() {
        return Ok(Vec::new());
    }

    let mut mods = Vec::new();
    let entries = fs::read_dir(mods_dir)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", mods_dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("Mod ファイルの確認に失敗しました: {error}"))?;
        let path = entry.path();
        if !path.is_file() || !looks_like_mod_file(&path) {
            continue;
        }

        let metadata = entry.metadata().map_err(|error| {
            format!("{} のメタデータを読めませんでした: {error}", path.display())
        })?;
        let file_name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| format!("{} のファイル名を解釈できません。", path.display()))?
            .to_string();
        let enabled = !file_name.ends_with(".disabled");
        let parsed = read_mod_metadata(&path).unwrap_or_else(|| ParsedModMetadata {
            display_name: None,
            mod_id: None,
            version: None,
            description: None,
            loader: None,
            authors: Vec::new(),
        });

        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|timestamp| timestamp.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs());

        mods.push(InstalledMod {
            file_name: file_name.clone(),
            display_name: parsed
                .display_name
                .unwrap_or_else(|| display_name_for_file(&file_name)),
            source_project_id: tracked_projects.get(&file_name).cloned(),
            mod_id: parsed.mod_id,
            version: parsed.version,
            description: parsed.description.and_then(trim_text),
            loader: parsed
                .loader
                .or_else(|| profile_loader.map(str::to_string))
                .filter(|value| value != "vanilla"),
            authors: parsed.authors,
            enabled,
            size_bytes: metadata.len(),
            modified_at,
        });
    }

    mods.sort_by(|left, right| {
        right.modified_at.cmp(&left.modified_at).then_with(|| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
        })
    });

    Ok(mods)
}

fn looks_like_mod_file(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(|file_name| is_mod_archive(file_name) || file_name.ends_with(".jar.disabled"))
        .unwrap_or(false)
}

fn read_mod_metadata(path: &Path) -> Option<ParsedModMetadata> {
    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;

    if let Some(metadata) = read_fabric_mod_json(&mut archive) {
        return Some(metadata);
    }

    if let Some(metadata) = read_quilt_mod_json(&mut archive) {
        return Some(metadata);
    }

    if let Some(metadata) = read_forge_mods_toml(&mut archive) {
        return Some(metadata);
    }

    read_manifest_metadata(&mut archive)
}

fn read_fabric_mod_json(archive: &mut ZipArchive<File>) -> Option<ParsedModMetadata> {
    let value = read_json_entry(archive, "fabric.mod.json")?;

    Some(ParsedModMetadata {
        display_name: value
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string),
        mod_id: value.get("id").and_then(Value::as_str).map(str::to_string),
        version: value_to_string(value.get("version")),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string),
        loader: Some("fabric".to_string()),
        authors: authors_from_json(value.get("authors")),
    })
}

fn read_quilt_mod_json(archive: &mut ZipArchive<File>) -> Option<ParsedModMetadata> {
    let value = read_json_entry(archive, "quilt.mod.json")?;
    let loader = value.get("quilt_loader")?;
    let metadata = loader.get("metadata");

    let authors = metadata
        .and_then(|item| item.get("contributors"))
        .map(contributors_from_json)
        .unwrap_or_default();

    Some(ParsedModMetadata {
        display_name: metadata
            .and_then(|item| item.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string),
        mod_id: loader.get("id").and_then(Value::as_str).map(str::to_string),
        version: value_to_string(loader.get("version")),
        description: metadata
            .and_then(|item| item.get("description"))
            .and_then(Value::as_str)
            .map(str::to_string),
        loader: Some("quilt".to_string()),
        authors,
    })
}

fn read_forge_mods_toml(archive: &mut ZipArchive<File>) -> Option<ParsedModMetadata> {
    let value = read_toml_entry(archive, "META-INF/mods.toml")?;
    let mods = value.get("mods")?.as_array()?;
    let mod_info = mods.first()?;
    let mod_loader = value
        .get("modLoader")
        .and_then(TomlValue::as_str)
        .unwrap_or_default()
        .to_lowercase();
    let loader = if mod_loader.contains("lowcodefml") || mod_loader.contains("javafml") {
        Some("forge".to_string())
    } else {
        None
    };

    Some(ParsedModMetadata {
        display_name: mod_info
            .get("displayName")
            .and_then(TomlValue::as_str)
            .map(str::to_string),
        mod_id: mod_info
            .get("modId")
            .and_then(TomlValue::as_str)
            .map(str::to_string),
        version: toml_value_to_string(mod_info.get("version")),
        description: mod_info
            .get("description")
            .and_then(TomlValue::as_str)
            .map(str::to_string),
        loader,
        authors: mod_info
            .get("authors")
            .and_then(TomlValue::as_str)
            .map(split_authors)
            .unwrap_or_default(),
    })
}

fn read_manifest_metadata(archive: &mut ZipArchive<File>) -> Option<ParsedModMetadata> {
    let manifest = read_text_entry(archive, "META-INF/MANIFEST.MF")?;
    let values = parse_manifest(&manifest);

    Some(ParsedModMetadata {
        display_name: values
            .get("Implementation-Title")
            .cloned()
            .or_else(|| values.get("Specification-Title").cloned()),
        mod_id: None,
        version: values
            .get("Implementation-Version")
            .cloned()
            .or_else(|| values.get("Specification-Version").cloned()),
        description: values.get("Implementation-Vendor").cloned(),
        loader: None,
        authors: values
            .get("Implementation-Vendor")
            .map(|value| vec![value.clone()])
            .unwrap_or_default(),
    })
}

fn read_json_entry(archive: &mut ZipArchive<File>, name: &str) -> Option<Value> {
    let text = read_text_entry(archive, name)?;
    serde_json::from_str(&text).ok()
}

fn read_toml_entry(archive: &mut ZipArchive<File>, name: &str) -> Option<TomlValue> {
    let text = read_text_entry(archive, name)?;
    toml::from_str(&text).ok()
}

fn read_text_entry(archive: &mut ZipArchive<File>, name: &str) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    Some(text)
}

fn parse_manifest(text: &str) -> HashMap<String, String> {
    let mut resolved: HashMap<String, String> = HashMap::new();
    let mut current_key: Option<String> = None;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some(key) = current_key.as_ref() {
                if let Some(existing) = resolved.get_mut(key) {
                    existing.push_str(rest);
                }
            }
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };

        let key = key.trim().to_string();
        current_key = Some(key.clone());
        resolved.insert(key, value.trim().to_string());
    }

    resolved
}

fn value_to_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.to_string()),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        _ => None,
    }
}

fn toml_value_to_string(value: Option<&TomlValue>) -> Option<String> {
    match value {
        Some(TomlValue::String(text)) => Some(text.to_string()),
        Some(TomlValue::Integer(number)) => Some(number.to_string()),
        Some(TomlValue::Float(number)) => Some(number.to_string()),
        Some(TomlValue::Boolean(flag)) => Some(flag.to_string()),
        _ => None,
    }
}

fn authors_from_json(value: Option<&Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };

    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text.to_string()),
                Value::Object(object) => object
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                _ => None,
            })
            .collect(),
        Value::String(text) => vec![text.to_string()],
        _ => Vec::new(),
    }
}

fn contributors_from_json(value: &Value) -> Vec<String> {
    match value {
        Value::Object(map) => map.keys().cloned().collect(),
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Value::String(text) => vec![text.to_string()],
        _ => Vec::new(),
    }
}

fn split_authors(text: &str) -> Vec<String> {
    text.split([',', ';', '&'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn trim_text(text: String) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.replace('\n', " "))
    }
}

fn display_profile_name(
    id: &str,
    configured_name: Option<&str>,
    profile_type: Option<&str>,
) -> String {
    if let Some(name) = configured_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return name.to_string();
    }

    match profile_type.unwrap_or_default() {
        "latest-release" => "最新リリース".to_string(),
        "latest-snapshot" => "最新スナップショット".to_string(),
        _ if looks_like_generated_profile_id(id) => "無題の構成".to_string(),
        _ => id.to_string(),
    }
}

fn looks_like_generated_profile_id(value: &str) -> bool {
    value.len() >= 24 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn extract_game_version_from_version_id(value: &str) -> Option<String> {
    let lower = value.to_lowercase();

    if lower == "latest-release" || lower == "latest-snapshot" {
        return None;
    }

    if let Some(rest) = lower.strip_prefix("fabric-loader-") {
        return rest
            .split_once('-')
            .map(|(_, game_version)| game_version.to_string());
    }

    if let Some(rest) = lower.strip_prefix("quilt-loader-") {
        return rest
            .split_once('-')
            .map(|(_, game_version)| game_version.to_string());
    }

    if let Some((game_version, _)) = lower.split_once("-forge-") {
        return Some(game_version.to_string());
    }

    if let Some((game_version, _)) = lower.split_once("-neoforge-") {
        return Some(game_version.to_string());
    }

    if let Some(rest) = lower.strip_prefix("neoforge-") {
        if let Some(version) = parse_neoforge_minecraft_version(rest) {
            return Some(version);
        }
    }

    extract_game_version_from_text(&lower)
}

fn extract_game_version_from_text(value: &str) -> Option<String> {
    let weekly = weekly_snapshot_regex()
        .find_iter(value)
        .last()
        .map(|capture| capture.as_str().to_string());
    if weekly.is_some() {
        return weekly;
    }

    release_version_regex()
        .find_iter(value)
        .filter_map(|capture| {
            let candidate = capture.as_str();
            if looks_like_loader_version(candidate) {
                None
            } else {
                Some(candidate.to_string())
            }
        })
        .last()
}

fn looks_like_loader_version(value: &str) -> bool {
    if value.starts_with("0.") {
        return true;
    }

    value.matches('.').count() >= 2 && !value.starts_with("1.")
}

fn parse_neoforge_minecraft_version(value: &str) -> Option<String> {
    let mut parts = value.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;

    if major.len() != 2
        || !major.chars().all(|character| character.is_ascii_digit())
        || !minor.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }

    Some(format!("1.{major}.{minor}"))
}

fn sanitize_profile_id(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let cleaned = sanitized.trim_matches('-').to_string();
    if cleaned.is_empty() {
        "profile".to_string()
    } else {
        cleaned
    }
}

fn legacy_launcher_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        candidates.push(
            PathBuf::from(program_files_x86)
                .join("Minecraft Launcher")
                .join("MinecraftLauncher.exe"),
        );
    }

    if let Some(program_files) = env::var_os("ProgramFiles") {
        candidates.push(
            PathBuf::from(program_files)
                .join("Minecraft Launcher")
                .join("MinecraftLauncher.exe"),
        );
    }

    candidates
}

fn weekly_snapshot_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\b\d{2}w\d{2}[a-z]\b").expect("weekly snapshot regex"))
}

fn release_version_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\b\d+\.\d+(?:\.\d+)?(?:-(?:pre|rc)-?\d+|(?:-snapshot-\d+))?\b")
            .expect("release version regex")
    })
}
