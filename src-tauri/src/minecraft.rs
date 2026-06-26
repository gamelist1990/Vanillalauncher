use crate::models::{
    ActionResult, ActiveLauncherAccount, InstalledMod, LauncherProfile, LauncherSnapshot,
    LauncherSummary, LocalModAnalysis, LocalModDependency,
};
use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::{HashMap, HashSet, VecDeque},
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
const WINDOWS_STORE_PRODUCT_URI: &str = "ms-windows-store://pdp/?ProductId=9PGW18NPBZV5";
const WINGET_MINECRAFT_LAUNCHER_ID: &str = "Microsoft.MinecraftLauncher";

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
    dependencies: Vec<ParsedModDependency>,
    /// JAR 内のアイコンファイルパス (例: "sodium-icon.png")
    icon_path: Option<String>,
    /// JAR から読み出してキャッシュ済みの base64 Data URI
    icon_data: Option<String>,
}

#[derive(Debug, Clone)]
struct ParsedModDependency {
    mod_id: String,
    requirement: String,
    required: bool,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
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
    #[serde(default)]
    pub auth_source: Option<String>,
    #[serde(default)]
    pub xbox_profile_verified: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileVisualOverride {
    #[serde(default)]
    custom_icon_url: Option<String>,
    #[serde(default)]
    background_image_url: Option<String>,
}

pub fn preferred_launcher_account_display_name(account: &LauncherAccount) -> Option<String> {
    if launcher_account_has_verified_profile(account) || account.xbox_profile_verified {
        if let Some(gamer_tag) = account
            .gamer_tag
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(gamer_tag.to_string());
        }
    }

    if let Some(username) = account
        .username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(local_part) = launcher_account_email_local_part(username) {
            if let Some(stylized) = account
                .gamer_tag
                .as_deref()
                .and_then(|gamer_tag| stylize_unverified_account_name(&local_part, gamer_tag))
            {
                return Some(stylized);
            }

            return Some(local_part);
        }

        return Some(username.to_string());
    }

    account
        .gamer_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn launcher_account_has_verified_profile(account: &LauncherAccount) -> bool {
    account
        .profile_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
}

pub(crate) fn launcher_account_email_local_part(value: &str) -> Option<String> {
    let local_part = value
        .split_once('@')
        .map(|(local_part, _domain)| local_part)
        .unwrap_or(value)
        .trim();

    if local_part.is_empty() {
        None
    } else {
        Some(local_part.to_string())
    }
}

fn launcher_account_username_looks_like_email(account: &LauncherAccount) -> bool {
    account
        .username
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| value.contains('@'))
}

fn stylize_unverified_account_name(local_part: &str, gamer_tag: &str) -> Option<String> {
    let local_part = local_part.trim();
    if local_part.is_empty() {
        return None;
    }

    let tokens = gamer_tag
        .split(|character: char| {
            character.is_ascii_whitespace() || character == '_' || character == '-'
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if tokens.len() < 2 {
        return None;
    }

    let local_lower = local_part.to_ascii_lowercase();
    let token_orders = [
        tokens.clone(),
        tokens.iter().rev().copied().collect::<Vec<_>>(),
    ];

    for order in token_orders {
        let joined = order
            .iter()
            .map(|value| value.to_ascii_lowercase())
            .collect::<String>();
        if joined != local_lower {
            continue;
        }

        let Some(prefix) = order.first().copied().filter(|value| {
            value
                .chars()
                .all(|character| !character.is_ascii_lowercase())
        }) else {
            continue;
        };

        if local_part.len() < prefix.len() {
            continue;
        }

        return Some(format!("{prefix}{}", &local_part[prefix.len()..]));
    }

    None
}

#[derive(Debug, Clone)]
struct ParsedLauncherAccountRecord {
    account: LauncherAccount,
    raw: Map<String, Value>,
}

#[derive(Debug, Clone)]
struct ParsedLauncherAccountsFile {
    path: PathBuf,
    root: Value,
    active_account_id: Option<String>,
    client_token: Option<String>,
    accounts: Vec<ParsedLauncherAccountRecord>,
}

pub fn load_launcher_snapshot() -> Result<LauncherSnapshot, String> {
    let minecraft_root = minecraft_root()?;
    let profiles_json = read_launcher_profiles(&minecraft_root)?;
    let visual_overrides = read_profile_visual_overrides(&minecraft_root);
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
        let override_values = visual_overrides.get(id);
        let custom_icon_url = value
            .get("customIcon")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                override_values
                    .and_then(|entry| entry.custom_icon_url.clone())
                    .and_then(normalize_optional_visual_url)
            });
        let background_image_url = value
            .get("backgroundImage")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                override_values
                    .and_then(|entry| entry.background_image_url.clone())
                    .and_then(normalize_optional_visual_url)
            });
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
        let local_id = account.local_id.clone()?;
        preferred_launcher_account_display_name(&account).map(|name| ActiveLauncherAccount {
            local_id,
            username: name,
            auth_source: "official-launcher".to_string(),
            has_java_access: false,
        })
    });

    Ok(LauncherSnapshot {
        minecraft_root: minecraft_root.to_string_lossy().to_string(),
        launcher_available: launcher_available(),
        active_account,
        launcher_accounts: Vec::new(),
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
    let (accounts, active_account_id) = read_launcher_accounts_with_active_id()?;
    let Some(active_account_id) = active_account_id else {
        return Ok(None);
    };

    Ok(accounts
        .into_iter()
        .find(|account| account.local_id.as_deref() == Some(active_account_id.as_str())))
}

pub fn read_launcher_accounts() -> Result<Vec<LauncherAccount>, String> {
    let (accounts, _active_account_id) = read_launcher_accounts_with_active_id()?;
    Ok(accounts)
}

pub fn read_discovered_launcher_accounts() -> Result<Vec<LauncherAccount>, String> {
    let path = discovered_launcher_accounts_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", path.display()))?;
    let mut accounts = serde_json::from_str::<Vec<LauncherAccount>>(&contents)
        .map_err(|error| format!("{} を解析できませんでした: {error}", path.display()))?;
    dedupe_launcher_accounts(&mut accounts);
    Ok(accounts)
}

pub fn merge_discovered_launcher_accounts(accounts: &[LauncherAccount]) -> Result<usize, String> {
    let path = discovered_launcher_accounts_path()?;
    let mut stored = read_discovered_launcher_accounts()?;
    let mut added = 0usize;
    let mut changed = false;

    for incoming in accounts {
        if launcher_account_identity_values(incoming).is_empty() {
            continue;
        }

        if let Some(existing) = stored
            .iter_mut()
            .find(|current| launcher_accounts_match(current, incoming))
        {
            let before = existing.clone();
            merge_launcher_account_fields(existing, incoming);
            if *existing != before {
                changed = true;
            }
            continue;
        }

        stored.push(incoming.clone());
        added += 1;
        changed = true;
    }

    if changed {
        dedupe_launcher_accounts(&mut stored);
        write_discovered_launcher_accounts(&path, &stored)?;
    }

    Ok(added)
}

pub fn remove_microsoft_oauth_launcher_account(local_id: &str) -> Result<LauncherAccount, String> {
    let local_id = local_id.trim();
    if local_id.is_empty() {
        return Err("削除する Microsoft アカウントを指定してください。".to_string());
    }

    let path = discovered_launcher_accounts_path()?;
    let mut discovered_accounts = read_discovered_launcher_accounts()?;
    let Some(index) = discovered_accounts.iter().position(|account| {
        account.local_id.as_deref() == Some(local_id)
            && account.auth_source.as_deref() == Some("microsoft-oauth")
    }) else {
        return Err("Microsoft 経由でログインしたアカウントが見つかりませんでした。".to_string());
    };

    let removed = discovered_accounts.remove(index);
    write_discovered_launcher_accounts(&path, &discovered_accounts)?;

    let target_path = primary_launcher_accounts_path()?;
    if target_path.exists() {
        let mut parsed = parse_launcher_accounts_file(&target_path)?;
        let mut changed = false;

        if let Some(root_object) = parsed.root.as_object_mut() {
            if let Some(accounts) = root_object.get_mut("accounts").and_then(Value::as_object_mut) {
                changed |= accounts.remove(local_id).is_some();
            }

            let active_is_removed = root_object
                .get("activeAccountLocalId")
                .and_then(Value::as_str)
                .map(str::trim)
                == Some(local_id);
            if active_is_removed {
                root_object.insert("activeAccountLocalId".to_string(), Value::String(String::new()));
                changed = true;
            }
        }

        if changed {
            write_launcher_json_file(&parsed.path, &parsed.root)?;
        }
    }

    Ok(removed)
}

pub fn set_active_launcher_account(local_id: &str) -> Result<String, String> {
    let target_path = primary_launcher_accounts_path()?;
    let mut parsed = if target_path.exists() {
        parse_launcher_accounts_file(&target_path)?
    } else {
        ParsedLauncherAccountsFile {
            path: target_path.clone(),
            root: serde_json::json!({
                "accounts": {},
                "activeAccountLocalId": "",
                "mojangClientToken": "",
            }),
            active_account_id: None,
            client_token: None,
            accounts: Vec::new(),
        }
    };

    let mut selected_record = parsed
        .accounts
        .iter()
        .find(|entry| entry.account.local_id.as_deref() == Some(local_id))
        .cloned();
    let mut selected_client_token = parsed.client_token.clone();

    if selected_record.is_none() {
        let root = minecraft_root()?;
        for file_name in [
            "launcher_accounts_microsoft_store.json",
            "launcher_accounts.json",
        ] {
            let path = root.join(file_name);
            if path == parsed.path || !path.exists() {
                continue;
            }
            let source = match parse_launcher_accounts_file(&path) {
                Ok(source) => source,
                Err(_) => continue,
            };
            let source_client_token = source.client_token.clone();
            let Some(record) = source
                .accounts
                .into_iter()
                .find(|entry| entry.account.local_id.as_deref() == Some(local_id))
            else {
                continue;
            };

            let Some(root_object) = parsed.root.as_object_mut() else {
                return Err(format!(
                    "{} のルート形式を解釈できませんでした。",
                    parsed.path.display()
                ));
            };
            let accounts = ensure_json_object_field(root_object, "accounts")?;
            accounts.insert(local_id.to_string(), Value::Object(record.raw.clone()));
            selected_record = Some(record);
            selected_client_token = source_client_token.or(selected_client_token);
            break;
        }
    }

    if selected_record.is_none() {
        let Some(discovered) = read_discovered_launcher_accounts()?
            .into_iter()
            .find(|entry| entry.local_id.as_deref() == Some(local_id))
        else {
            return Err("指定された Launcher アカウントが見つかりませんでした。".to_string());
        };

        let raw = build_launcher_account_record(&discovered);
        let Some(root_object) = parsed.root.as_object_mut() else {
            return Err(format!(
                "{} のルート形式を解釈できませんでした。",
                parsed.path.display()
            ));
        };
        let accounts = ensure_json_object_field(root_object, "accounts")?;
        accounts.insert(local_id.to_string(), Value::Object(raw.clone()));
        selected_client_token = discovered.client_token.clone().or(selected_client_token);
        selected_record = Some(ParsedLauncherAccountRecord {
            account: discovered,
            raw,
        });
    }

    let Some(record) = selected_record else {
        return Err("指定された Launcher アカウントが見つかりませんでした。".to_string());
    };

    let display_name = preferred_launcher_account_display_name(&record.account)
        .unwrap_or_else(|| "アカウント".to_string());

    let Some(root_object) = parsed.root.as_object_mut() else {
        return Err(format!(
            "{} のルート形式を解釈できませんでした。",
            parsed.path.display()
        ));
    };
    if root_object
        .get("mojangClientToken")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        if let Some(client_token) = selected_client_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            root_object.insert(
                "mojangClientToken".to_string(),
                Value::String(client_token.to_string()),
            );
        }
    }
    root_object.insert(
        "activeAccountLocalId".to_string(),
        Value::String(local_id.to_string()),
    );

    write_launcher_json_file(&parsed.path, &parsed.root)?;
    Ok(display_name)
}

pub fn scan_and_merge_launcher_accounts() -> Result<(usize, usize, usize), String> {
    let target_accounts_path = primary_launcher_accounts_path()?;
    let candidate_paths = discover_launcher_metadata_files(&[
        "launcher_accounts_microsoft_store.json",
        "launcher_accounts.json",
    ])?;

    let scanned_files = candidate_paths.len();
    let mut merged_accounts = 0usize;

    let mut target_accounts_file = if target_accounts_path.exists() {
        parse_launcher_accounts_file(&target_accounts_path)?
    } else {
        ParsedLauncherAccountsFile {
            path: target_accounts_path.clone(),
            root: serde_json::json!({
                "accounts": {},
                "activeAccountLocalId": "",
                "mojangClientToken": "",
            }),
            active_account_id: None,
            client_token: None,
            accounts: Vec::new(),
        }
    };

    let mut known_local_ids = target_accounts_file
        .accounts
        .iter()
        .filter_map(|entry| entry.account.local_id.clone())
        .collect::<HashSet<_>>();

    for path in candidate_paths {
        if path == target_accounts_file.path {
            continue;
        }

        let parsed = match parse_launcher_accounts_file(&path) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        if target_accounts_file.client_token.is_none() && parsed.client_token.is_some() {
            if let Some(root_object) = target_accounts_file.root.as_object_mut() {
                root_object.insert(
                    "mojangClientToken".to_string(),
                    Value::String(parsed.client_token.clone().unwrap_or_default()),
                );
            }
            target_accounts_file.client_token = parsed.client_token.clone();
        }

        for record in parsed.accounts {
            let Some(local_id) = record.account.local_id.clone() else {
                continue;
            };
            if !known_local_ids.insert(local_id.clone()) {
                continue;
            }

            let Some(root_object) = target_accounts_file.root.as_object_mut() else {
                return Err(format!(
                    "{} のルート形式を解釈できませんでした。",
                    target_accounts_file.path.display()
                ));
            };
            let accounts = ensure_json_object_field(root_object, "accounts")?;
            accounts.insert(local_id, Value::Object(record.raw.clone()));
            target_accounts_file.accounts.push(record);
            merged_accounts += 1;
        }
    }

    if !target_accounts_path.exists() || merged_accounts > 0 {
        write_launcher_json_file(&target_accounts_file.path, &target_accounts_file.root)?;
    }

    let target_entitlements_path = primary_launcher_entitlements_path()?;
    let merged_entitlements = scan_and_merge_launcher_entitlements(&target_entitlements_path)?;

    Ok((scanned_files, merged_accounts, merged_entitlements))
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

    if let Some(existing) = entries.iter_mut().find(|entry| {
        entry.file_name == file_name
            || (!source.project_id.is_empty() && entry.project_id == source.project_id)
            || (source.curseforge_project_id.is_some()
                && source.curseforge_file_id.is_some()
                && entry.curseforge_project_id == source.curseforge_project_id
                && entry.curseforge_file_id == source.curseforge_file_id)
    }) {
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

fn read_launcher_accounts_with_active_id() -> Result<(Vec<LauncherAccount>, Option<String>), String>
{
    let root = minecraft_root()?;
    let mut accounts = Vec::new();
    let mut active_account_id = None;

    for file_name in [
        "launcher_accounts_microsoft_store.json",
        "launcher_accounts.json",
    ] {
        let path = root.join(file_name);
        if !path.exists() {
            continue;
        }

        let parsed = parse_launcher_accounts_file(&path)?;
        if active_account_id.is_none() {
            active_account_id = parsed.active_account_id.clone();
        }
        accounts.extend(parsed.accounts.into_iter().map(|entry| entry.account));
    }

    if let Ok(discovered_accounts) = read_discovered_launcher_accounts() {
        for account in &mut accounts {
            if let Some(discovered) = discovered_accounts
                .iter()
                .find(|candidate| launcher_accounts_match(account, candidate))
            {
                merge_launcher_account_fields(account, discovered);
            }
        }
    }

    dedupe_launcher_accounts(&mut accounts);
    Ok((accounts, active_account_id))
}

fn parse_launcher_accounts_file(path: &Path) -> Result<ParsedLauncherAccountsFile, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", path.display()))?;
    let value: Value = serde_json::from_str(&contents)
        .map_err(|error| format!("{} を解析できませんでした: {error}", path.display()))?;
    let active_account_id = value
        .get("activeAccountLocalId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let client_token = value
        .get("mojangClientToken")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);

    let accounts = value
        .get("accounts")
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|(local_id, value)| {
                    let account = value.as_object()?;
                    Some(ParsedLauncherAccountRecord {
                        account: parse_launcher_account_entry(
                            account,
                            client_token.as_deref(),
                            Some(local_id.as_str()),
                        ),
                        raw: account.clone(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(ParsedLauncherAccountsFile {
        path: path.to_path_buf(),
        root: value,
        active_account_id,
        client_token,
        accounts,
    })
}

fn parse_launcher_account_entry(
    account: &Map<String, Value>,
    client_token: Option<&str>,
    local_id_hint: Option<&str>,
) -> LauncherAccount {
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

    LauncherAccount {
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
        client_token: client_token.map(str::to_string),
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
            .map(str::to_string)
            .or_else(|| {
                local_id_hint
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            }),
        user_properties,
        auth_source: account
            .get("authSource")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        xbox_profile_verified: account
            .get("xboxProfileVerified")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn build_launcher_account_record(account: &LauncherAccount) -> Map<String, Value> {
    let mut raw = Map::new();

    if let Some(local_id) = account
        .local_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        raw.insert("localId".to_string(), Value::String(local_id.to_string()));
    }
    if let Some(username) = account
        .username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        raw.insert("username".to_string(), Value::String(username.to_string()));
    }
    if let Some(access_token) = account
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        raw.insert(
            "accessToken".to_string(),
            Value::String(access_token.to_string()),
        );
    }
    if let Some(expires_at) = account
        .access_token_expires_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        raw.insert(
            "accessTokenExpiresAt".to_string(),
            Value::String(expires_at.to_string()),
        );
    }
    if let Some(xuid) = account
        .xuid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        raw.insert("remoteId".to_string(), Value::String(xuid.to_string()));
    }
    if let Some(auth_source) = account
        .auth_source
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        raw.insert("authSource".to_string(), Value::String(auth_source.to_string()));
    }

    let mut profile = Map::new();
    if let Some(profile_id) = account
        .profile_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        profile.insert("id".to_string(), Value::String(profile_id.to_string()));
    }
    if let Some(gamer_tag) = account
        .gamer_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        profile.insert("name".to_string(), Value::String(gamer_tag.to_string()));
    }
    if !profile.is_empty() {
        raw.insert("minecraftProfile".to_string(), Value::Object(profile));
    }

    if let Some(user_properties) = account
        .user_properties
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Ok(parsed) = serde_json::from_str::<Value>(user_properties) {
            raw.insert("userProperites".to_string(), parsed);
        }
    }
    if account.xbox_profile_verified {
        raw.insert("xboxProfileVerified".to_string(), Value::Bool(true));
    }

    raw.insert("type".to_string(), Value::String("Xbox".to_string()));

    raw
}

fn dedupe_launcher_accounts(accounts: &mut Vec<LauncherAccount>) {
    let mut seen = HashSet::new();
    accounts.retain(|account| {
        let key = account
            .local_id
            .clone()
            .or_else(|| account.profile_id.clone())
            .or_else(|| account.xuid.clone())
            .or_else(|| account.username.clone())
            .unwrap_or_else(|| format!("unknown-{}", seen.len()));
        seen.insert(key)
    });
}

fn launcher_accounts_match(left: &LauncherAccount, right: &LauncherAccount) -> bool {
    let right_keys = launcher_account_identity_values(right);
    if right_keys.is_empty() {
        return false;
    }

    launcher_account_identity_values(left)
        .into_iter()
        .any(|value| right_keys.contains(&value))
}

fn launcher_account_identity_values(account: &LauncherAccount) -> Vec<String> {
    let mut values = Vec::new();

    for value in [
        account.local_id.as_deref(),
        account.profile_id.as_deref(),
        account.xuid.as_deref(),
        account.username.as_deref(),
        account.gamer_tag.as_deref(),
    ] {
        let Some(value) = value.map(str::trim).filter(|entry| !entry.is_empty()) else {
            continue;
        };
        values.push(value.to_ascii_lowercase());
    }

    values.sort();
    values.dedup();
    values
}

pub(crate) fn merge_launcher_account_fields(
    target: &mut LauncherAccount,
    source: &LauncherAccount,
) {
    merge_launcher_account_option(&mut target.username, &source.username);
    merge_launcher_account_gamer_tag(target, source);
    merge_launcher_account_option(&mut target.profile_id, &source.profile_id);
    merge_launcher_account_option(&mut target.access_token, &source.access_token);
    merge_launcher_account_option(
        &mut target.access_token_expires_at,
        &source.access_token_expires_at,
    );
    merge_launcher_account_option(&mut target.client_token, &source.client_token);
    merge_launcher_account_option(&mut target.xuid, &source.xuid);
    merge_launcher_account_option(&mut target.local_id, &source.local_id);
    merge_launcher_account_option(&mut target.user_properties, &source.user_properties);
    merge_launcher_account_option(&mut target.auth_source, &source.auth_source);
    target.xbox_profile_verified |= source.xbox_profile_verified;
}

fn merge_launcher_account_gamer_tag(target: &mut LauncherAccount, source: &LauncherAccount) {
    let Some(source_gamer_tag) = source
        .gamer_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };

    let target_has_verified_profile = launcher_account_has_verified_profile(target);
    let source_has_verified_profile = launcher_account_has_verified_profile(source);
    let target_username_is_email = launcher_account_username_looks_like_email(target);
    let should_replace = target
        .gamer_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
        || (source_has_verified_profile
            && (!target_has_verified_profile
                || (target_username_is_email && !target.xbox_profile_verified)))
        || (source.xbox_profile_verified
            && !target_has_verified_profile
            && !target.xbox_profile_verified);

    if should_replace {
        target.gamer_tag = Some(source_gamer_tag.to_string());
    }
}

fn merge_launcher_account_option(target: &mut Option<String>, source: &Option<String>) {
    if target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        return;
    }

    if let Some(value) = source
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        *target = Some(value.to_string());
    }
}

fn primary_launcher_accounts_path() -> Result<PathBuf, String> {
    let root = minecraft_root()?;
    let microsoft_store = root.join("launcher_accounts_microsoft_store.json");
    if microsoft_store.exists() {
        return Ok(microsoft_store);
    }

    let classic = root.join("launcher_accounts.json");
    if classic.exists() {
        return Ok(classic);
    }

    Ok(microsoft_store)
}

fn primary_launcher_entitlements_path() -> Result<PathBuf, String> {
    let root = minecraft_root()?;
    let microsoft_store = root.join("launcher_entitlements_microsoft_store.json");
    if microsoft_store.exists() {
        return Ok(microsoft_store);
    }

    let classic = root.join("launcher_entitlements.json");
    if classic.exists() {
        return Ok(classic);
    }

    Ok(microsoft_store)
}

fn discovered_launcher_accounts_path() -> Result<PathBuf, String> {
    let root = minecraft_root()?;
    Ok(root
        .join(".vanillalauncher")
        .join("discovered-launcher-accounts.json"))
}

fn write_launcher_json_file(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|error| {
        format!(
            "{} を保存形式へ変換できませんでした: {error}",
            path.display()
        )
    })?;
    fs::write(path, text)
        .map_err(|error| format!("{} を更新できませんでした: {error}", path.display()))
}

fn write_discovered_launcher_accounts(
    path: &Path,
    accounts: &[LauncherAccount],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(accounts).map_err(|error| {
        format!(
            "{} を保存形式へ変換できませんでした: {error}",
            path.display()
        )
    })?;
    fs::write(path, text)
        .map_err(|error| format!("{} を更新できませんでした: {error}", path.display()))
}

fn ensure_json_object_field<'a>(
    object: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>, String> {
    let value = object
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value
        .as_object_mut()
        .ok_or_else(|| format!("{key} をオブジェクトとして扱えませんでした。"))
}

fn discover_launcher_metadata_files(file_names: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    roots.push(minecraft_root()?);
    if let Some(appdata) = env::var_os("APPDATA") {
        roots.push(PathBuf::from(appdata).join(".minecraft"));
    }
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(&local_app_data).join("Packages"));
        roots.push(PathBuf::from(local_app_data).join("Microsoft"));
    }

    let wanted = file_names
        .iter()
        .map(|entry| entry.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let mut found = Vec::new();

    for root in roots {
        if root.exists() && visited.insert(root.clone()) {
            queue.push_back((root, 0usize));
        }
    }

    while let Some((dir, depth)) = queue.pop_front() {
        if found.len() >= 64 {
            break;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            if file_type.is_file() {
                let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if wanted.contains(&file_name) {
                    found.push(path);
                }
                continue;
            }

            if !file_type.is_dir() || depth >= 6 {
                continue;
            }

            if should_descend_launcher_scan_dir(&path, depth + 1) && visited.insert(path.clone()) {
                queue.push_back((path, depth + 1));
            }
        }
    }

    found.sort();
    found.dedup();
    Ok(found)
}

fn should_descend_launcher_scan_dir(path: &Path, depth: usize) -> bool {
    if depth <= 2 {
        return true;
    }

    path.file_name()
        .and_then(|value| value.to_str())
        .map(|value| {
            let lower = value.to_ascii_lowercase();
            lower.contains("minecraft")
                || lower.contains("launcher")
                || lower.contains("mojang")
                || lower.contains("microsoft")
                || lower.contains("xbox")
                || lower.contains("game")
                || lower.contains("roaming")
                || lower.contains("localcache")
                || lower.contains("4297127d64ec6")
        })
        .unwrap_or(false)
}

fn scan_and_merge_launcher_entitlements(target_path: &Path) -> Result<usize, String> {
    let mut merged = 0usize;
    let mut target_value = if target_path.exists() {
        let contents = fs::read_to_string(target_path).map_err(|error| {
            format!("{} を読み込めませんでした: {error}", target_path.display())
        })?;
        serde_json::from_str::<Value>(&contents)
            .map_err(|error| format!("{} を解析できませんでした: {error}", target_path.display()))?
    } else {
        serde_json::json!({ "data": {}, "formatVersion": 1 })
    };

    let candidate_paths = discover_launcher_metadata_files(&[
        "launcher_entitlements_microsoft_store.json",
        "launcher_entitlements.json",
    ])?;

    for path in candidate_paths {
        if path == target_path {
            continue;
        }

        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(_) => continue,
        };
        let value = match serde_json::from_str::<Value>(&contents) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(source_data) = value.get("data").and_then(Value::as_object) else {
            continue;
        };
        let Some(target_root) = target_value.as_object_mut() else {
            return Err(format!(
                "{} のルート形式を解釈できませんでした。",
                target_path.display()
            ));
        };
        let target_data = ensure_json_object_field(target_root, "data")?;

        for (key, payload) in source_data {
            if target_data.contains_key(key) {
                continue;
            }
            target_data.insert(key.clone(), payload.clone());
            merged += 1;
        }
    }

    if !target_path.exists() || merged > 0 {
        write_launcher_json_file(target_path, &target_value)?;
    }

    Ok(merged)
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
    let _ = clear_profile_visual_override(&minecraft_root, profile_id);

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

    let resolved_custom_icon_url = profile
        .get("customIcon")
        .and_then(Value::as_str)
        .map(str::to_string);
    let resolved_background_image_url = profile
        .get("backgroundImage")
        .and_then(Value::as_str)
        .map(str::to_string);

    write_launcher_profiles(&minecraft_root, &profiles_json)?;
    upsert_profile_visual_override(
        &minecraft_root,
        profile_id,
        resolved_custom_icon_url,
        resolved_background_image_url,
    )
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
    let settings = crate::settings::load_settings();
    if settings.official_launcher_auto_install && !launcher_available() {
        ensure_official_launcher_available(false)?;
    }

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
        return legacy_launcher_candidates().iter().any(|path| path.exists())
            || windows_store_launcher_package_installed();
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

pub fn official_launcher_installer_path() -> PathBuf {
    crate::settings::temp_root_dir()
        .join("official-launcher")
        .join("MinecraftLauncher.winget")
}

pub fn ensure_official_launcher_available_with_progress(
    app: &tauri::AppHandle,
    operation_id: Option<String>,
    reinstall: bool,
) -> Result<ActionResult, String> {
    let operation_id = operation_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("official-launcher-{}", chrono::Local::now().timestamp_millis()));

    crate::progress::emit_progress(
        app,
        &operation_id,
        "公式 Launcher を準備中",
        if reinstall {
            "公式 Minecraft Launcher を再インストールします。"
        } else {
            "公式 Minecraft Launcher の導入状態を確認しています。"
        },
        10.0,
    );

    let result = ensure_official_launcher_available(reinstall);

    match &result {
        Ok(_) => crate::progress::emit_progress(
            app,
            &operation_id,
            "公式 Launcher を準備中",
            "公式 Minecraft Launcher の準備が完了しました。",
            100.0,
        ),
        Err(error) => crate::progress::emit_progress(
            app,
            &operation_id,
            "公式 Launcher を準備中",
            format!("公式 Minecraft Launcher の準備に失敗しました: {error}"),
            100.0,
        ),
    }

    result
}

pub fn ensure_official_launcher_available(reinstall: bool) -> Result<ActionResult, String> {
    if !cfg!(target_os = "windows") {
        return Err(
            "公式 Minecraft Launcher の自動導入は現在 Windows のみ対応です。macOS / Linux では公式サイトまたは各ストアから導入してください。"
                .to_string(),
        );
    }

    if launcher_available() && !reinstall {
        return Ok(ActionResult {
            message: "公式 Minecraft Launcher は既に利用可能です。".to_string(),
            file_name: official_launcher_installer_path().to_string_lossy().to_string(),
        });
    }

    if let Some(parent) = official_launcher_installer_path().parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }

    crate::app_log::append_log(
        "INFO",
        if reinstall {
            "installing official Minecraft Launcher via winget with reinstall requested"
        } else {
            "installing official Minecraft Launcher via winget"
        },
    );

    let winget_result = run_winget_minecraft_launcher_install(reinstall);
    match winget_result {
        Ok(message) => Ok(ActionResult {
            message,
            file_name: official_launcher_installer_path().to_string_lossy().to_string(),
        }),
        Err(error) => {
            crate::app_log::append_log(
                "WARN",
                format!("winget Minecraft Launcher install failed: {error}"),
            );
            open_minecraft_launcher_store_page()?;
            Ok(ActionResult {
                message: format!(
                    "winget で公式 Minecraft Launcher を導入できなかったため、Microsoft Store の公式ページを開きました。表示された画面からインストールしてください。詳細: {error}"
                ),
                file_name: WINDOWS_STORE_PRODUCT_URI.to_string(),
            })
        }
    }
}

fn run_winget_minecraft_launcher_install(reinstall: bool) -> Result<String, String> {
    let mut command = std::process::Command::new("winget");
    if reinstall {
        command.args([
            "install",
            "--id",
            WINGET_MINECRAFT_LAUNCHER_ID,
            "-e",
            "--source",
            "msstore",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--force",
        ]);
    } else {
        command.args([
            "install",
            "--id",
            WINGET_MINECRAFT_LAUNCHER_ID,
            "-e",
            "--source",
            "msstore",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ]);
    }

    suppress_console_window(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("winget を実行できませんでした: {error}"))?;

    if output.status.success() {
        return Ok(if reinstall {
            "公式 Minecraft Launcher の再インストールを開始しました。".to_string()
        } else {
            "公式 Minecraft Launcher のインストールを開始しました。".to_string()
        });
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(format!("winget が失敗しました: {}{}", stdout, stderr))
}

fn open_minecraft_launcher_store_page() -> Result<(), String> {
    if cfg!(target_os = "windows") {
        std::process::Command::new("explorer.exe")
            .arg(WINDOWS_STORE_PRODUCT_URI)
            .spawn()
            .map_err(|error| format!("Microsoft Store を開けませんでした: {error}"))?;
        return Ok(());
    }
    Err("Microsoft Store ページを開けるのは Windows のみです。".to_string())
}

fn windows_store_launcher_package_installed() -> bool {
    if !cfg!(target_os = "windows") {
        return false;
    }

    let mut command = std::process::Command::new("powershell.exe");
    command.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        "if (Get-AppxPackage -Name 'Microsoft.4297127D64EC6' -ErrorAction SilentlyContinue) { exit 0 } else { exit 1 }",
    ]);
    suppress_console_window(&mut command);
    command.output().map(|output| output.status.success()).unwrap_or(false)
}

fn suppress_console_window(command: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
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

    let saved_custom_icon_url = profile
        .get("customIcon")
        .and_then(Value::as_str)
        .map(str::to_string);
    let saved_background_image_url = profile
        .get("backgroundImage")
        .and_then(Value::as_str)
        .map(str::to_string);

    profiles_object.insert(profile_id.clone(), Value::Object(profile));
    write_launcher_profiles(&minecraft_root, &profiles_json)?;
    let _ = upsert_profile_visual_override(
        &minecraft_root,
        &profile_id,
        saved_custom_icon_url,
        saved_background_image_url,
    );
    seed_profile_preferences_from_vanilla(&minecraft_root, &draft.game_dir)?;
    Ok(profile_id)
}

fn profile_visual_overrides_path(minecraft_root: &Path) -> PathBuf {
    minecraft_root
        .join(".vanillalauncher")
        .join("profile-visual-overrides.json")
}

fn read_profile_visual_overrides(minecraft_root: &Path) -> HashMap<String, ProfileVisualOverride> {
    let path = profile_visual_overrides_path(minecraft_root);
    if !path.exists() {
        return HashMap::new();
    }

    let Ok(contents) = fs::read_to_string(&path) else {
        return HashMap::new();
    };

    serde_json::from_str::<HashMap<String, ProfileVisualOverride>>(&contents)
        .unwrap_or_else(|_| HashMap::new())
}

fn write_profile_visual_overrides(
    minecraft_root: &Path,
    overrides: &HashMap<String, ProfileVisualOverride>,
) -> Result<(), String> {
    let path = profile_visual_overrides_path(minecraft_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }

    let serialized = serde_json::to_string_pretty(overrides)
        .map_err(|error| format!("外観の補助設定を保存形式に変換できませんでした: {error}"))?;
    fs::write(&path, serialized)
        .map_err(|error| format!("{} を保存できませんでした: {error}", path.display()))
}

fn normalize_optional_visual_url(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn upsert_profile_visual_override(
    minecraft_root: &Path,
    profile_id: &str,
    custom_icon_url: Option<String>,
    background_image_url: Option<String>,
) -> Result<(), String> {
    let normalized_custom_icon_url = custom_icon_url.and_then(normalize_optional_visual_url);
    let normalized_background_image_url =
        background_image_url.and_then(normalize_optional_visual_url);

    let mut overrides = read_profile_visual_overrides(minecraft_root);
    if normalized_custom_icon_url.is_none() && normalized_background_image_url.is_none() {
        overrides.remove(profile_id);
    } else {
        overrides.insert(
            profile_id.to_string(),
            ProfileVisualOverride {
                custom_icon_url: normalized_custom_icon_url,
                background_image_url: normalized_background_image_url,
            },
        );
    }

    write_profile_visual_overrides(minecraft_root, &overrides)
}

fn clear_profile_visual_override(minecraft_root: &Path, profile_id: &str) -> Result<(), String> {
    let mut overrides = read_profile_visual_overrides(minecraft_root);
    if overrides.remove(profile_id).is_none() {
        return Ok(());
    }
    write_profile_visual_overrides(minecraft_root, &overrides)
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
            return PathBuf::from(appdata)
                .join(".minecraft")
                .join("options.txt");
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
        || matches!(key, "lang" | "fov" | "fovEffectScale")
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
            dependencies: Vec::new(),
            icon_path: None,
            icon_data: None,
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
            icon_data: parsed.icon_data,
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

/// JAR アーカイブからアイコン画像を読み出し base64 Data URI に変換する。
/// archive はすでに開いているため JAR を2回開かない。
fn extract_icon_data(archive: &mut ZipArchive<File>, icon_path: &str) -> Option<String> {
    use std::io::Read;
    let mut entry = archive.by_name(icon_path).ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).ok()?;
    let ext = std::path::Path::new(icon_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png",
    };
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    Some(format!("data:{};base64,{}", mime, b64))
}

fn read_mod_metadata(path: &Path) -> Option<ParsedModMetadata> {
    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;

    let mut metadata = if let Some(m) = read_fabric_mod_json(&mut archive) {
        m
    } else if let Some(m) = read_quilt_mod_json(&mut archive) {
        m
    } else if let Some(m) = read_forge_mods_toml(&mut archive) {
        m
    } else {
        read_manifest_metadata(&mut archive)?
    };

    // アイコンを同じ archive から一度だけ読んでキャッシュする
    if let Some(ref icon_path) = metadata.icon_path.clone() {
        metadata.icon_data = extract_icon_data(&mut archive, icon_path);
    }

    Some(metadata)
}

pub fn analyze_local_mod(profile_id: &str, mod_path: &str) -> Result<LocalModAnalysis, String> {
    let source_path = PathBuf::from(mod_path.trim());
    if !source_path.exists() {
        return Err("指定された Mod ファイルが見つかりません。".to_string());
    }

    let file_name = source_path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| "ファイル名を取得できませんでした。".to_string())?
        .to_string();

    if !is_mod_archive(&file_name) {
        return Err("対応していないファイル形式です。.jar ファイルを指定してください。".to_string());
    }

    validate_file_name(&file_name)?;

    let profile = find_profile(profile_id)?;
    let parsed = read_mod_metadata(&source_path).ok_or_else(|| {
        format!("{} から Mod メタデータを読み取れませんでした。", file_name)
    })?;

    let profile_loader = normalize_loader(Some(&profile.loader));
    let mod_loader = parsed.loader.as_deref().map(|value| normalize_loader(Some(value)));
    let game_version = profile.game_version.as_deref().unwrap_or_default();
    let mods_dir = resolve_profile_mods_dir(profile_id, &profile.game_dir)?;
    let tracked_projects = tracked_project_map(&minecraft_root()?, profile_id);
    let existing_mods = read_mods(&mods_dir, Some(profile_loader), &tracked_projects)?;

    let existing = parsed.mod_id.as_deref().and_then(|mod_id| {
        existing_mods
            .iter()
            .find(|installed| installed.mod_id.as_deref() == Some(mod_id))
    });

    let mut dependencies = Vec::new();
    let mut problems = Vec::new();

    if profile_loader == "vanilla" {
        problems.push("Vanilla 構成には Mod を直接導入できません。Loader を導入してください。".to_string());
    }

    if let Some(loader) = mod_loader {
        let loader_ok = loader == profile_loader
            || (profile_loader == "quilt" && loader == "fabric")
            || (profile_loader == "neoforge" && loader == "forge");
        if !loader_ok {
            problems.push(format!(
                "Loader が一致しません: このJarは {} 用ですが、現在の構成は {} です。",
                loader, profile_loader
            ));
        }
    }

    for dep in &parsed.dependencies {
        let (satisfied, note) = dependency_note(dep, profile_loader, game_version, &existing_mods);
        if dep.required && !satisfied {
            problems.push(note.clone());
        }
        dependencies.push(LocalModDependency {
            mod_id: dep.mod_id.clone(),
            requirement: dep.requirement.clone(),
            required: dep.required,
            satisfied,
            note,
        });
    }

    let version_cmp = existing
        .and_then(|installed| installed.version.as_deref())
        .zip(parsed.version.as_deref())
        .map(|(old, new)| compare_versions(new, old));

    let compatible = problems.is_empty();
    let action = if !compatible {
        "reject".to_string()
    } else if existing.is_some() {
        match version_cmp.unwrap_or(1) {
            value if value > 0 => "replace".to_string(),
            value if value == 0 => "skip".to_string(),
            _ => "reject".to_string(),
        }
    } else {
        "install".to_string()
    };

    let display_name = parsed
        .display_name
        .clone()
        .unwrap_or_else(|| display_name_for_file(&file_name));

    let summary = if !compatible {
        problems.join(" ")
    } else if action == "replace" {
        format!(
            "{} は既存Modより新しいバージョンです。既存ファイルを置き換えます。",
            display_name
        )
    } else if action == "skip" {
        format!("{} は既に同じバージョンが導入済みです。", display_name)
    } else if action == "reject" {
        format!("{} は既存Modより古い可能性があるため却下します。", display_name)
    } else {
        format!("{} は現在の構成に導入できます。", display_name)
    };

    let severity = if compatible && action != "reject" {
        "ok"
    } else {
        "error"
    }
    .to_string();

    // アイコンは read_mod_metadata 内で既に base64 変換済み (JAR を2回開かない)
    let icon_data = parsed.icon_data;

    Ok(LocalModAnalysis {
        file_path: source_path.to_string_lossy().to_string(),
        file_name,
        display_name,
        mod_id: parsed.mod_id,
        version: parsed.version,
        description: parsed.description.and_then(trim_text),
        loader: parsed.loader,
        authors: parsed.authors,
        compatible,
        action,
        severity,
        summary,
        dependencies,
        existing_file_name: existing.map(|mod_file| mod_file.file_name.clone()),
        existing_version: existing.and_then(|mod_file| mod_file.version.clone()),
        icon_data,
    })
}

pub fn import_checked_local_mod(profile_id: &str, mod_path: &str) -> Result<ActionResult, String> {
    let analysis = analyze_local_mod(profile_id, mod_path)?;
    if !analysis.compatible || analysis.action == "reject" || analysis.action == "skip" {
        return Err(analysis.summary);
    }

    let source_path = PathBuf::from(&analysis.file_path);
    let profile = find_profile(profile_id)?;
    let mods_dir = resolve_profile_mods_dir(profile_id, &profile.game_dir)?;
    fs::create_dir_all(&mods_dir)
        .map_err(|error| format!("mods フォルダを準備できませんでした: {error}"))?;

    if let Some(existing_file_name) = analysis.existing_file_name.as_deref() {
        validate_file_name(existing_file_name)?;
        let existing_path = mods_dir.join(existing_file_name);
        if existing_path.exists() {
            fs::remove_file(&existing_path)
                .map_err(|error| format!("既存 Mod を置き換えられませんでした: {error}"))?;
        }
    }

    let target_path = mods_dir.join(&analysis.file_name);
    if target_path.exists() {
        return Err(format!("{} はすでに存在します。", analysis.file_name));
    }

    fs::copy(&source_path, &target_path)
        .map_err(|error| format!("Mod ファイルをコピーできませんでした: {error}"))?;

    let message = if analysis.action == "replace" {
        format!("{} を更新しました。", analysis.display_name)
    } else {
        format!("{} を {} に追加しました。", analysis.display_name, profile.name)
    };

    Ok(ActionResult {
        message,
        file_name: analysis.file_name,
    })
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
        dependencies: fabric_dependencies_from_json(&value),
        icon_path: value.get("icon").and_then(Value::as_str).map(str::to_string),
        icon_data: None, // read_mod_metadata で一括処理
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
        dependencies: quilt_dependencies_from_json(loader),
        icon_path: metadata.and_then(|m| m.get("icon")).and_then(Value::as_str).map(str::to_string),
        icon_data: None,
    })
}

fn read_forge_mods_toml(archive: &mut ZipArchive<File>) -> Option<ParsedModMetadata> {
    let value = read_toml_entry(archive, "META-INF/mods.toml")
        .or_else(|| read_toml_entry(archive, "META-INF/neoforge.mods.toml"))?;
    let mods = value.get("mods")?.as_array()?;
    let mod_info = mods.first()?;
    let mod_loader = value
        .get("modLoader")
        .and_then(TomlValue::as_str)
        .unwrap_or_default()
        .to_lowercase();
    let dependencies = forge_dependencies_from_toml(&value);
    let loader = if dependencies.iter().any(|dep| dep.mod_id == "neoforge")
        || mod_loader.contains("neoforge")
    {
        Some("neoforge".to_string())
    } else if mod_loader.contains("lowcodefml") || mod_loader.contains("javafml") {
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
        dependencies,
        icon_path: mod_info
            .get("logoFile")
            .and_then(TomlValue::as_str)
            .map(str::to_string),
        icon_data: None,
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
        dependencies: Vec::new(),
        icon_path: None,
        icon_data: None,
    })
}

fn fabric_dependencies_from_json(value: &Value) -> Vec<ParsedModDependency> {
    let mut deps = Vec::new();
    for (section, required) in [("depends", true), ("recommends", false), ("suggests", false)] {
        if let Some(map) = value.get(section).and_then(Value::as_object) {
            for (mod_id, requirement) in map {
                deps.push(ParsedModDependency {
                    mod_id: mod_id.clone(),
                    requirement: value_to_string(Some(requirement)).unwrap_or_else(|| "*".to_string()),
                    required,
                });
            }
        }
    }
    deps
}

fn quilt_dependencies_from_json(loader: &Value) -> Vec<ParsedModDependency> {
    loader
        .get("depends")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(ParsedModDependency {
                        mod_id: item.get("id")?.as_str()?.to_string(),
                        requirement: value_to_string(item.get("versions")).unwrap_or_else(|| "*".to_string()),
                        required: true,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn forge_dependencies_from_toml(value: &TomlValue) -> Vec<ParsedModDependency> {
    let mut deps = Vec::new();
    let Some(table) = value.as_table() else {
        return deps;
    };

    for (key, item) in table {
        if !key.starts_with("dependencies") {
            continue;
        }
        if let Some(items) = item.as_array() {
            for dep in items {
                let Some(mod_id) = dep.get("modId").and_then(TomlValue::as_str) else {
                    continue;
                };
                let required = dep
                    .get("mandatory")
                    .and_then(TomlValue::as_bool)
                    .or_else(|| {
                        dep.get("type")
                            .and_then(TomlValue::as_str)
                            .map(|value| value == "required")
                    })
                    .unwrap_or(true);
                deps.push(ParsedModDependency {
                    mod_id: mod_id.to_string(),
                    requirement: dep
                        .get("versionRange")
                        .and_then(TomlValue::as_str)
                        .unwrap_or("*")
                        .to_string(),
                    required,
                });
            }
        }
    }
    deps
}

fn dependency_note(
    dep: &ParsedModDependency,
    profile_loader: &str,
    game_version: &str,
    installed_mods: &[InstalledMod],
) -> (bool, String) {
    let id = dep.mod_id.as_str();
    if id == "minecraft" {
        let ok = version_req_matches(&dep.requirement, game_version);
        return (
            ok,
            if ok {
                format!("Minecraft {} は条件 {} を満たしています。", game_version, dep.requirement)
            } else {
                format!("Minecraft {} は条件 {} を満たしていません。", game_version, dep.requirement)
            },
        );
    }

    if ["fabricloader", "fabric", "forge", "neoforge", "quilt_loader", "quilt"].contains(&id) {
        let ok = match id {
            "fabricloader" | "fabric" => profile_loader == "fabric" || profile_loader == "quilt",
            "forge" => profile_loader == "forge" || profile_loader == "neoforge",
            "neoforge" => profile_loader == "neoforge",
            "quilt_loader" | "quilt" => profile_loader == "quilt",
            _ => false,
        };
        return (
            ok,
            if ok {
                format!("Loader 条件 {} {} を満たしています。", id, dep.requirement)
            } else {
                format!("Loader 条件 {} {} を満たしていません。", id, dep.requirement)
            },
        );
    }

    // fabric-api のサブモジュール（fabric-rendering-fluids-v1 等）は
    // Fabric API 本体に同梱されているため、常に satisfied として扱う
    if id.starts_with("fabric-") {
        return (
            true,
            format!("Fabric API サブモジュール {} は Fabric API に同梱されています。", id),
        );
    }

    let installed = installed_mods.iter().any(|item| item.mod_id.as_deref() == Some(id));
    (
        installed || !dep.required,
        if installed {
            format!("依存 Mod {} は導入済みです。", id)
        } else if dep.required {
            format!("必須依存 Mod {} が不足しています。", id)
        } else {
            format!("任意依存 Mod {} は未導入です。", id)
        },
    )
}

fn version_req_matches(requirement: &str, current: &str) -> bool {
    let requirement = requirement.trim();
    if requirement.is_empty() || requirement == "*" || current.is_empty() {
        return true;
    }
    if requirement.contains(current) {
        return true;
    }

    let range = requirement.trim_matches(|c| c == '[' || c == ']' || c == '(' || c == ')');
    for part in range.split(',').map(str::trim).filter(|value| !value.is_empty()) {
        if let Some(min) = part.strip_prefix(">=") {
            if compare_versions(current, min) < 0 {
                return false;
            }
        } else if let Some(max) = part.strip_prefix("<=") {
            if compare_versions(current, max) > 0 {
                return false;
            }
        } else if let Some(min) = part.strip_prefix('>') {
            if compare_versions(current, min) <= 0 {
                return false;
            }
        } else if let Some(max) = part.strip_prefix('<') {
            if compare_versions(current, max) >= 0 {
                return false;
            }
        } else if !part.ends_with('-') && compare_versions(current, part) != 0 {
            return false;
        }
    }
    true
}

fn compare_versions(left: &str, right: &str) -> i32 {
    let left_parts = numeric_version_parts(left);
    let right_parts = numeric_version_parts(right);
    let len = left_parts.len().max(right_parts.len());
    for index in 0..len {
        let left_value = *left_parts.get(index).unwrap_or(&0);
        let right_value = *right_parts.get(index).unwrap_or(&0);
        if left_value > right_value {
            return 1;
        }
        if left_value < right_value {
            return -1;
        }
    }
    0
}

fn numeric_version_parts(value: &str) -> Vec<u64> {
    value
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
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
    let text = match value {
        Some(Value::String(text)) => Some(text.to_string()),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        _ => None,
    }?;

    normalize_mod_metadata_value(&text)
}

fn toml_value_to_string(value: Option<&TomlValue>) -> Option<String> {
    let text = match value {
        Some(TomlValue::String(text)) => Some(text.to_string()),
        Some(TomlValue::Integer(number)) => Some(number.to_string()),
        Some(TomlValue::Float(number)) => Some(number.to_string()),
        Some(TomlValue::Boolean(flag)) => Some(flag.to_string()),
        _ => None,
    }?;

    normalize_mod_metadata_value(&text)
}

fn normalize_mod_metadata_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || is_unresolved_metadata_placeholder(trimmed) {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn is_unresolved_metadata_placeholder(value: &str) -> bool {
    value.starts_with("${") && value.ends_with('}')
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

#[cfg(test)]
mod tests {
    use super::{
        merge_launcher_account_fields, preferred_launcher_account_display_name, LauncherAccount,
    };

    fn launcher_account(
        username: Option<&str>,
        gamer_tag: Option<&str>,
        profile_id: Option<&str>,
    ) -> LauncherAccount {
        LauncherAccount {
            username: username.map(str::to_string),
            gamer_tag: gamer_tag.map(str::to_string),
            profile_id: profile_id.map(str::to_string),
            access_token: None,
            access_token_expires_at: None,
            client_token: None,
            xuid: None,
            local_id: Some("local-id".to_string()),
            user_properties: None,
            auth_source: None,
            xbox_profile_verified: false,
        }
    }

    #[test]
    fn preferred_display_name_uses_email_local_part_without_java_profile() {
        let account = launcher_account(Some("pexkurann@gmail.com"), Some("Kurann PEX"), None);

        assert_eq!(
            preferred_launcher_account_display_name(&account).as_deref(),
            Some("PEXkurann")
        );
    }

    #[test]
    fn preferred_display_name_uses_gamertag_when_java_profile_exists() {
        let account = launcher_account(
            Some("pexkurann@gmail.com"),
            Some("PEXkoukunn"),
            Some("11112222333344445555666677778888"),
        );

        assert_eq!(
            preferred_launcher_account_display_name(&account).as_deref(),
            Some("PEXkoukunn")
        );
    }

    #[test]
    fn preferred_display_name_uses_verified_xbox_profile_without_java_profile() {
        let mut account = launcher_account(Some("pexkurann@gmail.com"), Some("CoolDragon99"), None);
        account.xbox_profile_verified = true;

        assert_eq!(
            preferred_launcher_account_display_name(&account).as_deref(),
            Some("CoolDragon99")
        );
    }

    #[test]
    fn verified_profile_name_replaces_stale_detected_hint() {
        let mut target = launcher_account(
            Some("isseidas@gmail.com"),
            Some("PC My"),
            Some("c53f907d0ad242c699c33994a3c1caa4"),
        );
        let source = launcher_account(
            Some("isseidas@gmail.com"),
            Some("PEXkoukunn"),
            Some("c53f907d0ad242c699c33994a3c1caa4"),
        );

        merge_launcher_account_fields(&mut target, &source);

        assert_eq!(target.gamer_tag.as_deref(), Some("PEXkoukunn"));
    }

    #[test]
    fn official_verified_profile_name_is_not_replaced_by_stale_discovered_hint() {
        let mut target = launcher_account(
            Some("Hotissei2019"),
            Some("PEXkoukunn"),
            Some("c53f907d0ad242c699c33994a3c1caa4"),
        );
        let source = launcher_account(
            Some("isseidas@gmail.com"),
            Some("PC My"),
            Some("c53f907d0ad242c699c33994a3c1caa4"),
        );

        merge_launcher_account_fields(&mut target, &source);

        assert_eq!(target.gamer_tag.as_deref(), Some("PEXkoukunn"));
    }
}
