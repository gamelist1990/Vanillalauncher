use crate::{
    loaders::ensure_loader_version_installed,
    minecraft::{
        find_profile, is_mod_archive, minecraft_root, normalize_loader,
        remove_tracked_project_by_file, remove_tracked_project_by_project,
        rename_tracked_project_file, resolve_profile_mods_dir, track_installed_project,
        track_installed_project_with_source, tracked_project_entries,
        update_profile_runtime_and_modpack, upsert_custom_profile, validate_file_name,
        CustomProfileDraft, TrackedModProject, TrackedModSource,
    },
    models::{
        ActionResult, InstallResult, ModRemoteState, ModpackExportResult, ModpackInstallResult,
        ModpackVersionSummary, ModrinthProject, ModrinthVersion,
    },
    progress::emit_progress,
    settings,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    env,
    fs::{self, File},
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};
use tauri::AppHandle;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const MODRINTH_API_BASE: &str = "https://api.modrinth.com/v2";
const MODRINTH_USER_AGENT: &str = "vanillalauncher/0.1.0 (tauri)";
const MODRINTH_CACHE_TTL: Duration = Duration::from_secs(120);
const MODRINTH_VISUAL_CACHE_TTL: Duration = Duration::from_secs(60 * 60 * 24 * 30);
const CURSEFORGE_API_BASE: &str = "https://api.curseforge.com";
const CURSEFORGE_MINECRAFT_GAME_ID: &str = "432";
const CURSEFORGE_MINECRAFT_MOD_CLASS_ID: &str = "6";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedJsonResponse {
    expires_at_unix_ms: i64,
    value: Value,
}

static MODRINTH_CLIENT: OnceLock<Client> = OnceLock::new();

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModpackIndex {
    format_version: Option<u32>,
    game: Option<String>,
    version_id: Option<String>,
    name: Option<String>,
    summary: Option<String>,
    files: Vec<ModpackFileEntry>,
    dependencies: HashMap<String, String>,
    #[serde(default, skip_serializing, skip_deserializing)]
    override_prefixes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModpackFileEntry {
    path: String,
    downloads: Vec<String>,
    env: Option<ModpackFileEnv>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModpackFileEnv {
    client: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeManifest {
    name: Option<String>,
    version: Option<String>,
    author: Option<String>,
    #[serde(rename = "manifestType")]
    manifest_type: Option<String>,
    #[serde(rename = "manifestVersion")]
    manifest_version: Option<u32>,
    overrides: Option<String>,
    minecraft: CurseforgeManifestMinecraft,
    #[serde(default)]
    files: Vec<CurseforgeManifestFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeManifestMinecraft {
    version: String,
    #[serde(default, rename = "modLoaders")]
    mod_loaders: Vec<CurseforgeManifestLoader>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeManifestLoader {
    id: String,
    primary: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CurseforgeManifestFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default)]
    required: bool,
    #[serde(default, rename = "isLocked")]
    is_locked: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeWebsiteFileResponse {
    data: CurseforgeWebsiteFile,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeWebsiteFilesResponse {
    data: Vec<CurseforgeWebsiteFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeWebsiteFile {
    id: u64,
    file_name: String,
    display_name: String,
    #[serde(default)]
    game_versions: Vec<String>,
    date_created: Option<String>,
    user: Option<CurseforgeWebsiteUser>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeWebsiteUser {
    display_name: Option<String>,
    twitch_avatar_url: Option<String>,
}

#[derive(Debug, Clone)]
struct CompatibleRelease {
    version: ModrinthVersion,
    file: crate::models::ModrinthFile,
}

pub async fn search_modrinth_mods(
    query: String,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<Vec<ModrinthProject>, String> {
    let normalized_loader = normalize_loader(loader.as_deref()).to_string();
    let trimmed_game_version = game_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let mut projects = search_projects(query.clone(), "mod", loader, game_version).await?;
    if let Some(project) = resolve_curseforge_project_query(
        query.trim(),
        &normalized_loader,
        trimmed_game_version.as_deref(),
    )
    .await?
    {
        projects.insert(0, project);
    } else {
        let curseforge_projects = search_curseforge_projects_official(
            query.trim(),
            &normalized_loader,
            trimmed_game_version.as_deref(),
        )
        .await
        .unwrap_or_default();
        projects.extend(curseforge_projects);
    }

    Ok(projects)
}

pub async fn search_modrinth_modpacks(
    query: String,
    game_version: Option<String>,
) -> Result<Vec<ModrinthProject>, String> {
    search_projects(query, "modpack", None, game_version).await
}

pub async fn get_modrinth_modpack_versions(
    project_id: String,
) -> Result<Vec<ModpackVersionSummary>, String> {
    let client = modrinth_client()?;
    let mut versions = fetch_modpack_versions(client, &project_id).await?;
    versions.sort_by(|left, right| right.published_at.cmp(&left.published_at));

    Ok(versions
        .into_iter()
        .map(|entry| ModpackVersionSummary {
            id: entry.id,
            name: entry.name,
            version_number: entry.version_number,
            game_versions: entry.game_versions,
            published_at: entry.published_at,
        })
        .collect())
}

async fn search_projects(
    query: String,
    project_type: &str,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<Vec<ModrinthProject>, String> {
    let trimmed = normalize_search_query(&query);

    let loader_filter = normalize_loader(loader.as_deref());
    let facets = build_search_facets(project_type, loader_filter, game_version.as_deref())?;
    let client = modrinth_client()?;
    let mut request = client.get(format!("{MODRINTH_API_BASE}/search"));
    request = request.query(&[
        ("limit", "18"),
        (
            "index",
            if trimmed.is_empty() {
                "downloads"
            } else {
                "relevance"
            },
        ),
        ("facets", facets.as_str()),
    ]);

    if !trimmed.is_empty() {
        request = request.query(&[("query", trimmed.as_str())]);
    }

    let cache_key = format!(
        "search|{project_type}|{loader_filter}|{}|{trimmed}",
        game_version
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("-")
    );
    let body = fetch_json_with_cache(
        cache_key,
        request,
        "Modrinth に接続できませんでした",
        "Modrinth 検索に失敗しました",
        "Modrinth のレスポンスを解析できませんでした",
    )
    .await?;

    let hits = body
        .get("hits")
        .and_then(Value::as_array)
        .ok_or_else(|| "Modrinth の検索結果形式が想定と異なります。".to_string())?;

    Ok(hits.iter().filter_map(parse_search_hit).collect())
}

pub async fn install_modrinth_project(
    app: &AppHandle,
    profile_id: String,
    project_id: String,
    operation_id: Option<String>,
) -> Result<InstallResult, String> {
    if let Some(curseforge_project_id) = parse_curseforge_project_id(&project_id) {
        return install_curseforge_project(app, profile_id, curseforge_project_id, operation_id)
            .await;
    }

    let operation_id = operation_id.unwrap_or_else(|| format!("modrinth-install-{project_id}"));
    let profile = find_profile(&profile_id)?;
    let normalized_loader = normalize_loader(Some(&profile.loader));

    if normalized_loader == "vanilla" {
        return Err(
            "この起動構成はまだ Vanilla です。先に Fabric / Forge / NeoForge / Quilt を導入してください。"
                .to_string(),
        );
    }

    emit_progress(
        app,
        &operation_id,
        "Mod を導入中",
        "互換バージョンを確認しています。",
        8.0,
    );
    let client = modrinth_client()?;
    let release = fetch_compatible_release(
        &client,
        &project_id,
        &normalized_loader,
        profile.game_version.as_deref(),
    )
    .await
    .map_err(|error| match error.as_str() {
        "missing-compatible-file" => format!(
            "{} / {} に対応する配布ファイルが見つかりませんでした。",
            profile.loader,
            profile
                .game_version
                .clone()
                .unwrap_or_else(|| "現在の Minecraft バージョン".to_string())
        ),
        _ => error,
    })?;
    let version = &release.version;
    let file = &release.file;

    emit_progress(
        app,
        &operation_id,
        "Mod を導入中",
        format!("{} をダウンロードしています。", file.filename),
        18.0,
    );
    let mut response = client
        .get(&file.url)
        .send()
        .await
        .map_err(|error| format!("Mod ファイルをダウンロードできませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Mod ダウンロードに失敗しました: {error}"))?;
    let total_bytes = response.content_length();
    let mut bytes = Vec::new();
    let mut downloaded_bytes = 0u64;

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("ダウンロードした Mod を読み取れませんでした: {error}"))?
    {
        downloaded_bytes += chunk.len() as u64;
        bytes.extend_from_slice(&chunk);

        let percent = if let Some(total_bytes) = total_bytes.filter(|value| *value > 0) {
            18.0 + ((downloaded_bytes as f64 / total_bytes as f64) * 62.0)
        } else {
            18.0
        };
        emit_progress(
            app,
            &operation_id,
            "Mod を導入中",
            format!("{} を保存用に受信しています。", file.filename),
            percent,
        );
    }

    let mods_dir = resolve_profile_mods_dir(&profile_id, &profile.game_dir)?;
    fs::create_dir_all(&mods_dir)
        .map_err(|error| format!("mods フォルダを準備できませんでした: {error}"))?;

    emit_progress(
        app,
        &operation_id,
        "Mod を導入中",
        "ダウンロードした Mod を構成へ反映しています。",
        90.0,
    );
    let target_path = mods_dir.join(&file.filename);
    fs::write(&target_path, &bytes)
        .map_err(|error| format!("Mod ファイルを保存できませんでした: {error}"))?;
    track_installed_project(&profile_id, &project_id, &file.filename)?;
    emit_progress(
        app,
        &operation_id,
        "Mod を導入中",
        "インストールが完了しました。",
        100.0,
    );

    Ok(InstallResult {
        message: format!("{} に {} を導入しました。", profile.name, version.name),
        file_name: file.filename.clone(),
        version_name: version.version_number.clone(),
    })
}

pub async fn get_profile_mod_remote_states(
    profile_id: String,
) -> Result<Vec<ModRemoteState>, String> {
    let profile = find_profile(&profile_id)?;
    let tracked_entries = tracked_project_entries(&profile_id)?
        .into_iter()
        .map(|entry| (entry.file_name.clone(), entry))
        .collect::<HashMap<_, _>>();
    let tracked_mods: Vec<_> = profile
        .mods
        .iter()
        .filter(|mod_file| tracked_entries.contains_key(&mod_file.file_name))
        .collect();

    if tracked_mods.is_empty() {
        return Ok(Vec::new());
    }

    let client = modrinth_client()?;
    let mut states = Vec::with_capacity(tracked_mods.len());

    for mod_file in tracked_mods {
        let tracked_entry = match tracked_entries.get(&mod_file.file_name) {
            Some(entry) => entry,
            None => continue,
        };
        if let Some(state) =
            build_mod_remote_state(&client, &profile, mod_file, tracked_entry).await?
        {
            states.push(state);
        }
    }

    Ok(states)
}

pub async fn get_profile_mod_remote_state(
    profile_id: String,
    file_name: String,
) -> Result<Option<ModRemoteState>, String> {
    let profile = find_profile(&profile_id)?;
    let tracked_entries = tracked_project_entries(&profile_id)?
        .into_iter()
        .map(|entry| (entry.file_name.clone(), entry))
        .collect::<HashMap<_, _>>();
    let Some(mod_file) = profile
        .mods
        .iter()
        .find(|mod_file| mod_file.file_name == file_name)
    else {
        return Ok(None);
    };
    let Some(tracked_entry) = tracked_entries.get(&mod_file.file_name) else {
        return Ok(None);
    };
    let client = modrinth_client()?;
    build_mod_remote_state(&client, &profile, mod_file, tracked_entry).await
}

pub async fn get_profile_mod_visual_state(
    profile_id: String,
    file_name: String,
) -> Result<Option<ModRemoteState>, String> {
    let profile = find_profile(&profile_id)?;
    let tracked_entries = tracked_project_entries(&profile_id)?
        .into_iter()
        .map(|entry| (entry.file_name.clone(), entry))
        .collect::<HashMap<_, _>>();
    let Some(mod_file) = profile
        .mods
        .iter()
        .find(|mod_file| mod_file.file_name == file_name)
    else {
        return Ok(None);
    };
    let Some(tracked_entry) = tracked_entries.get(&mod_file.file_name) else {
        return Ok(None);
    };
    let client = modrinth_client()?;
    build_mod_visual_state(&client, mod_file, tracked_entry).await
}

async fn build_mod_visual_state(
    client: &Client,
    mod_file: &crate::models::InstalledMod,
    tracked_entry: &TrackedModProject,
) -> Result<Option<ModRemoteState>, String> {
    let source_project_id = mod_file
        .source_project_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| tracked_entry.project_id.clone());

    if source_project_id.trim().is_empty() {
        return Ok(None);
    }

    if let Some(curseforge_project_id) = tracked_entry
        .curseforge_project_id
        .or_else(|| parse_curseforge_project_id(&source_project_id))
    {
        let file_metadata = match tracked_entry.curseforge_file_id {
            Some(file_id) => {
                fetch_curseforge_file_metadata_for_visual_cache(
                    client,
                    curseforge_project_id,
                    file_id,
                )
                .await
                .ok()
            }
            None => None,
        };

        return Ok(Some(ModRemoteState {
            file_name: mod_file.file_name.clone(),
            project_id: source_project_id,
            source: "curseforge".to_string(),
            project_title: Some(mod_file.display_name.clone()),
            project_url: file_metadata
                .as_ref()
                .map(|file| build_curseforge_file_info_url(curseforge_project_id, file.id)),
            icon_url: file_metadata
                .as_ref()
                .and_then(|file| file.user.as_ref())
                .and_then(|user| user.twitch_avatar_url.clone()),
            latest_version: None,
            latest_file_name: None,
            published_at: None,
            update_available: false,
            can_update: false,
        }));
    }

    let modrinth_project_id = source_project_id
        .strip_prefix("modrinth:")
        .unwrap_or(&source_project_id)
        .to_string();
    let project = fetch_project_details_for_visual_cache(client, &modrinth_project_id)
        .await
        .ok();

    Ok(Some(ModRemoteState {
        file_name: mod_file.file_name.clone(),
        project_id: modrinth_project_id,
        source: "modrinth".to_string(),
        project_title: project
            .as_ref()
            .map(|entry| entry.title.clone())
            .or_else(|| Some(mod_file.display_name.clone())),
        project_url: project.as_ref().map(|entry| entry.project_url.clone()),
        icon_url: project.as_ref().and_then(|entry| entry.icon_url.clone()),
        latest_version: None,
        latest_file_name: None,
        published_at: None,
        update_available: false,
        can_update: false,
    }))
}

async fn build_mod_remote_state(
    client: &Client,
    profile: &crate::models::LauncherProfile,
    mod_file: &crate::models::InstalledMod,
    tracked_entry: &TrackedModProject,
) -> Result<Option<ModRemoteState>, String> {
    let source_project_id = mod_file
        .source_project_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| tracked_entry.project_id.clone());

    if source_project_id.trim().is_empty() {
        return Ok(None);
    }

    let default_loader = normalize_loader(Some(&profile.loader)).to_string();
    let loader = mod_file.loader.as_deref().unwrap_or(&default_loader);
    let game_version = profile.game_version.as_deref();
    let installed_file_name = mod_file.file_name.trim_end_matches(".disabled").to_string();
    let visual_state = build_mod_visual_state(client, mod_file, tracked_entry).await?;

    if let Some(curseforge_project_id) = tracked_entry
        .curseforge_project_id
        .or_else(|| parse_curseforge_project_id(&source_project_id))
    {
        let release =
            fetch_latest_curseforge_file(client, curseforge_project_id, loader, game_version)
                .await
                .ok();
        let latest_file_name = release.as_ref().map(|file| file.file_name.clone());
        let latest_version = release.as_ref().map(|file| file.display_name.clone());
        let published_at = release.as_ref().and_then(|file| file.date_created.clone());
        let project_url = release
            .as_ref()
            .map(|file| build_curseforge_file_info_url(curseforge_project_id, file.id));
        let icon_url = visual_state
            .as_ref()
            .and_then(|state| state.icon_url.clone())
            .or_else(|| {
                release
                    .as_ref()
                    .and_then(|file| file.user.as_ref())
                    .and_then(|user| user.twitch_avatar_url.clone())
            });
        let update_available = latest_file_name
            .as_ref()
            .map(|file_name| file_name != &installed_file_name)
            .unwrap_or(false);

        return Ok(Some(ModRemoteState {
            file_name: mod_file.file_name.clone(),
            project_id: source_project_id,
            source: "curseforge".to_string(),
            project_title: visual_state
                .as_ref()
                .and_then(|state| state.project_title.clone())
                .or_else(|| Some(mod_file.display_name.clone())),
            project_url: project_url.or_else(|| {
                visual_state
                    .as_ref()
                    .and_then(|state| state.project_url.clone())
            }),
            icon_url,
            latest_version,
            latest_file_name,
            published_at,
            update_available,
            can_update: release.is_some(),
        }));
    }

    let modrinth_project_id = source_project_id
        .strip_prefix("modrinth:")
        .unwrap_or(&source_project_id)
        .to_string();
    let project = fetch_projects_details(client, &[modrinth_project_id.clone()])
        .await
        .ok()
        .and_then(|mut items| items.remove(&modrinth_project_id));
    let release = fetch_compatible_release(client, &modrinth_project_id, loader, game_version)
        .await
        .ok();
    let latest_file_name = release.as_ref().map(|entry| entry.file.filename.clone());
    let latest_version = release
        .as_ref()
        .map(|entry| entry.version.version_number.clone());
    let published_at = release
        .as_ref()
        .and_then(|entry| entry.version.published_at.clone());
    let update_available = latest_file_name
        .as_ref()
        .map(|file_name| file_name != &installed_file_name)
        .unwrap_or(false);

    Ok(Some(ModRemoteState {
        file_name: mod_file.file_name.clone(),
        project_id: modrinth_project_id,
        source: "modrinth".to_string(),
        project_title: project
            .as_ref()
            .map(|entry| entry.title.clone())
            .or_else(|| {
                visual_state
                    .as_ref()
                    .and_then(|state| state.project_title.clone())
            })
            .or_else(|| Some(mod_file.display_name.clone())),
        project_url: project
            .as_ref()
            .map(|entry| entry.project_url.clone())
            .or_else(|| {
                visual_state
                    .as_ref()
                    .and_then(|state| state.project_url.clone())
            }),
        icon_url: project
            .as_ref()
            .and_then(|entry| entry.icon_url.clone())
            .or_else(|| {
                visual_state
                    .as_ref()
                    .and_then(|state| state.icon_url.clone())
            }),
        latest_version,
        latest_file_name,
        published_at,
        update_available,
        can_update: release.is_some(),
    }))
}

pub async fn update_modrinth_project(
    app: &AppHandle,
    profile_id: String,
    project_id: String,
    file_name: String,
    operation_id: Option<String>,
) -> Result<InstallResult, String> {
    if let Some(curseforge_project_id) = parse_curseforge_project_id(&project_id) {
        return update_curseforge_project(
            app,
            profile_id,
            curseforge_project_id,
            file_name,
            operation_id,
        )
        .await;
    }

    validate_file_name(&file_name)?;
    let operation_id = operation_id.unwrap_or_else(|| format!("modrinth-update-{project_id}"));
    let profile = find_profile(&profile_id)?;
    let normalized_loader = normalize_loader(Some(&profile.loader));

    if normalized_loader == "vanilla" {
        return Err(
            "この起動構成はまだ Vanilla です。先に Fabric / Forge / NeoForge / Quilt を導入してください。"
                .to_string(),
        );
    }

    emit_progress(
        app,
        &operation_id,
        "Mod を更新中",
        "互換バージョンを確認しています。",
        8.0,
    );
    let client = modrinth_client()?;
    let release = fetch_compatible_release(
        &client,
        &project_id,
        &normalized_loader,
        profile.game_version.as_deref(),
    )
    .await
    .map_err(|error| match error.as_str() {
        "missing-compatible-file" => format!(
            "{} / {} に対応する更新候補が見つかりませんでした。",
            profile.loader,
            profile
                .game_version
                .clone()
                .unwrap_or_else(|| "現在の Minecraft バージョン".to_string())
        ),
        _ => error,
    })?;
    let version = &release.version;
    let file = &release.file;

    emit_progress(
        app,
        &operation_id,
        "Mod を更新中",
        format!("{} をダウンロードしています。", file.filename),
        18.0,
    );
    let mut response = client
        .get(&file.url)
        .send()
        .await
        .map_err(|error| format!("更新ファイルをダウンロードできませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Mod 更新のダウンロードに失敗しました: {error}"))?;
    let total_bytes = response.content_length();
    let mut bytes = Vec::new();
    let mut downloaded_bytes = 0u64;

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("ダウンロードした Mod を読み取れませんでした: {error}"))?
    {
        downloaded_bytes += chunk.len() as u64;
        bytes.extend_from_slice(&chunk);

        let percent = if let Some(total_bytes) = total_bytes.filter(|value| *value > 0) {
            18.0 + ((downloaded_bytes as f64 / total_bytes as f64) * 62.0)
        } else {
            18.0
        };
        emit_progress(
            app,
            &operation_id,
            "Mod を更新中",
            format!("{} を保存用に受信しています。", file.filename),
            percent,
        );
    }

    let mods_dir = resolve_profile_mods_dir(&profile_id, &profile.game_dir)?;
    fs::create_dir_all(&mods_dir)
        .map_err(|error| format!("mods フォルダを準備できませんでした: {error}"))?;

    let disabled = file_name.ends_with(".disabled");
    let target_file_name = if disabled {
        format!("{}.disabled", file.filename)
    } else {
        file.filename.clone()
    };
    let current_path = mods_dir.join(&file_name);
    let target_path = mods_dir.join(&target_file_name);

    emit_progress(
        app,
        &operation_id,
        "Mod を更新中",
        "ダウンロードした更新を構成へ反映しています。",
        90.0,
    );
    fs::write(&target_path, &bytes)
        .map_err(|error| format!("更新後の Mod ファイルを保存できませんでした: {error}"))?;

    if current_path.exists() && current_path != target_path {
        fs::remove_file(&current_path)
            .map_err(|error| format!("旧バージョンの Mod を整理できませんでした: {error}"))?;
    }

    track_installed_project(&profile_id, &project_id, &target_file_name)?;
    emit_progress(
        app,
        &operation_id,
        "Mod を更新中",
        "アップデートが完了しました。",
        100.0,
    );

    Ok(InstallResult {
        message: format!("{} の {} を更新しました。", profile.name, file.filename),
        file_name: target_file_name,
        version_name: version.version_number.clone(),
    })
}

async fn install_curseforge_project(
    app: &AppHandle,
    profile_id: String,
    project_id: u64,
    operation_id: Option<String>,
) -> Result<InstallResult, String> {
    let operation_id = operation_id.unwrap_or_else(|| format!("curseforge-install-{project_id}"));
    let profile = find_profile(&profile_id)?;
    let normalized_loader = normalize_loader(Some(&profile.loader));

    if normalized_loader == "vanilla" {
        return Err(
            "この起動構成はまだ Vanilla です。先に Fabric / Forge / NeoForge / Quilt を導入してください。"
                .to_string(),
        );
    }

    emit_progress(
        app,
        &operation_id,
        "Mod を導入中",
        "CurseForge の互換ファイルを確認しています。",
        8.0,
    );
    let client = modrinth_client()?;
    let file = fetch_latest_curseforge_file(
        client,
        project_id,
        normalized_loader,
        profile.game_version.as_deref(),
    )
    .await?;
    let download_url = build_curseforge_download_url(file.id, &file.file_name)?;

    emit_progress(
        app,
        &operation_id,
        "Mod を導入中",
        format!("{} をダウンロードしています。", file.file_name),
        18.0,
    );
    let bytes = client
        .get(&download_url)
        .send()
        .await
        .map_err(|error| format!("CurseForge Mod をダウンロードできませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("CurseForge Mod のダウンロードに失敗しました: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("ダウンロードした Mod を読み取れませんでした: {error}"))?;

    let mods_dir = resolve_profile_mods_dir(&profile_id, &profile.game_dir)?;
    fs::create_dir_all(&mods_dir)
        .map_err(|error| format!("mods フォルダを準備できませんでした: {error}"))?;
    let target_path = mods_dir.join(&file.file_name);
    fs::write(&target_path, &bytes)
        .map_err(|error| format!("Mod ファイルを保存できませんでした: {error}"))?;
    track_installed_project_with_source(
        &profile_id,
        TrackedModSource {
            source: Some("curseforge".to_string()),
            project_id: format!("curseforge:{project_id}"),
            curseforge_project_id: Some(project_id),
            curseforge_file_id: Some(file.id),
        },
        &file.file_name,
    )?;
    emit_progress(
        app,
        &operation_id,
        "Mod を導入中",
        "インストールが完了しました。",
        100.0,
    );

    Ok(InstallResult {
        message: format!("{} に {} を導入しました。", profile.name, file.file_name),
        file_name: file.file_name,
        version_name: file.display_name,
    })
}

async fn update_curseforge_project(
    app: &AppHandle,
    profile_id: String,
    project_id: u64,
    file_name: String,
    operation_id: Option<String>,
) -> Result<InstallResult, String> {
    validate_file_name(&file_name)?;
    let operation_id = operation_id.unwrap_or_else(|| format!("curseforge-update-{project_id}"));
    let profile = find_profile(&profile_id)?;
    let normalized_loader = normalize_loader(Some(&profile.loader));

    if normalized_loader == "vanilla" {
        return Err(
            "この起動構成はまだ Vanilla です。先に Fabric / Forge / NeoForge / Quilt を導入してください。"
                .to_string(),
        );
    }

    emit_progress(
        app,
        &operation_id,
        "Mod を更新中",
        "CurseForge の更新候補を確認しています。",
        8.0,
    );
    let client = modrinth_client()?;
    let file = fetch_latest_curseforge_file(
        client,
        project_id,
        normalized_loader,
        profile.game_version.as_deref(),
    )
    .await?;
    let download_url = build_curseforge_download_url(file.id, &file.file_name)?;

    emit_progress(
        app,
        &operation_id,
        "Mod を更新中",
        format!("{} をダウンロードしています。", file.file_name),
        18.0,
    );
    let bytes = client
        .get(&download_url)
        .send()
        .await
        .map_err(|error| format!("CurseForge 更新ファイルをダウンロードできませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("CurseForge Mod 更新のダウンロードに失敗しました: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("ダウンロードした Mod を読み取れませんでした: {error}"))?;

    let mods_dir = resolve_profile_mods_dir(&profile_id, &profile.game_dir)?;
    fs::create_dir_all(&mods_dir)
        .map_err(|error| format!("mods フォルダを準備できませんでした: {error}"))?;

    let disabled = file_name.ends_with(".disabled");
    let target_file_name = if disabled {
        format!("{}.disabled", file.file_name)
    } else {
        file.file_name.clone()
    };
    let current_path = mods_dir.join(&file_name);
    let target_path = mods_dir.join(&target_file_name);
    fs::write(&target_path, &bytes)
        .map_err(|error| format!("更新後の Mod ファイルを保存できませんでした: {error}"))?;
    if current_path.exists() && current_path != target_path {
        fs::remove_file(&current_path)
            .map_err(|error| format!("旧バージョンの Mod を整理できませんでした: {error}"))?;
    }
    track_installed_project_with_source(
        &profile_id,
        TrackedModSource {
            source: Some("curseforge".to_string()),
            project_id: format!("curseforge:{project_id}"),
            curseforge_project_id: Some(project_id),
            curseforge_file_id: Some(file.id),
        },
        &target_file_name,
    )?;
    emit_progress(
        app,
        &operation_id,
        "Mod を更新中",
        "アップデートが完了しました。",
        100.0,
    );

    Ok(InstallResult {
        message: format!("{} の {} を更新しました。", profile.name, file.file_name),
        file_name: target_file_name,
        version_name: file.display_name,
    })
}

fn loader_label(loader: &str) -> &'static str {
    match normalize_loader(Some(loader)) {
        "fabric" => "Fabric",
        "quilt" => "Quilt",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "vanilla" => "Vanilla",
        _ => "Loader",
    }
}

async fn ensure_loader_with_progress(
    app: &AppHandle,
    operation_id: &str,
    title: &str,
    loader: &str,
    minecraft_version: &str,
    loader_version: Option<&str>,
    start_percent: f64,
    max_wait_percent: f64,
) -> Result<(String, String), String> {
    let label = loader_label(loader);
    emit_progress(
        app,
        operation_id,
        title,
        format!(
            "{} / {} の起動環境を確認しています。必要なランタイムのダウンロードとセットアップを行う場合があります。",
            label, minecraft_version
        ),
        start_percent,
    );

    let done = Arc::new(AtomicBool::new(false));
    let done_for_worker = Arc::clone(&done);
    let app_handle = app.clone();
    let operation_id_owned = operation_id.to_string();
    let title_owned = title.to_string();
    let label_owned = label.to_string();
    let minecraft_version_owned = minecraft_version.to_string();
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let mut wait_percent = (start_percent + 2.0).min(max_wait_percent);
        while !done_for_worker.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_secs(8));
            if done_for_worker.load(Ordering::Relaxed) {
                break;
            }
            let elapsed_seconds = started.elapsed().as_secs();
            emit_progress(
                &app_handle,
                &operation_id_owned,
                &title_owned,
                format!(
                    "{} / {} の起動環境を準備中です。必要なファイルをダウンロードして展開しています（{} 秒経過）。初回は数分かかることがあります。",
                    label_owned,
                    minecraft_version_owned,
                    elapsed_seconds
                ),
                wait_percent,
            );
            if wait_percent < max_wait_percent {
                wait_percent = (wait_percent + 2.0).min(max_wait_percent);
            }
        }
    });

    let result = ensure_loader_version_installed(loader, minecraft_version, loader_version).await;
    done.store(true, Ordering::Relaxed);
    result
}

pub async fn install_modrinth_modpack(
    app: &AppHandle,
    project_id: String,
    version_id: Option<String>,
    operation_id: Option<String>,
    icon_url: Option<String>,
    image_url: Option<String>,
) -> Result<ModpackInstallResult, String> {
    let operation_id = operation_id.unwrap_or_else(|| format!("modpack-install-{project_id}"));
    let client = modrinth_client()?;
    emit_progress(
        app,
        &operation_id,
        "Modpack を準備中",
        "Modpack のバージョン情報を取得しています。",
        6.0,
    );
    let project = fetch_project_details(&client, &project_id).await?;
    let version = fetch_modpack_version(client, &project_id, version_id.as_deref(), None).await?;
    let file = pick_modpack_file(&version)
        .ok_or_else(|| "Modpack の .mrpack ファイルが見つかりませんでした。".to_string())?;

    emit_progress(
        app,
        &operation_id,
        "Modpack を準備中",
        format!("{} をダウンロードしています。", file.filename),
        14.0,
    );
    let pack_bytes = client
        .get(&file.url)
        .send()
        .await
        .map_err(|error| format!("Modpack をダウンロードできませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Modpack のダウンロードに失敗しました: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("Modpack の内容を読み取れませんでした: {error}"))?;

    let root = minecraft_root()?;
    let pack = read_modpack_index(&pack_bytes)?;
    let (loader, minecraft_version, loader_version) = resolve_modpack_dependencies(&pack)?;

    let (version_id, _) = ensure_loader_with_progress(
        app,
        &operation_id,
        "Modpack を準備中",
        &loader,
        &minecraft_version,
        loader_version.as_deref(),
        26.0,
        44.0,
    )
    .await?;

    let game_dir = modpack_instance_dir(&root, &loader, &project.title);
    fs::create_dir_all(&game_dir)
        .map_err(|error| format!("{} を準備できませんでした: {error}", game_dir.display()))?;

    emit_progress(
        app,
        &operation_id,
        "Modpack を準備中",
        "Modpack ファイルと overrides を展開しています。",
        46.0,
    );
    install_modpack_files(app, &operation_id, &client, &pack, &pack_bytes, &game_dir).await?;

    let profile_id = upsert_custom_profile(CustomProfileDraft {
        name: project.title.clone(),
        icon: Some("Grass".to_string()),
        custom_icon_url: icon_url.or(project.icon_url.clone()),
        background_image_url: image_url.or(project.image_url.clone()),
        game_dir: game_dir.clone(),
        last_version_id: version_id.clone(),
    })?;

    track_modpack_projects(&profile_id, &pack)?;
    update_profile_runtime_and_modpack(
        &profile_id,
        &version_id,
        Some(&project_id),
        Some(&version.id),
    )?;

    emit_progress(
        app,
        &operation_id,
        "Modpack を準備中",
        "新しい起動構成を作成しました。",
        100.0,
    );

    Ok(ModpackInstallResult {
        message: format!(
            "{} を新しい起動構成として作成しました。{} を選べばそのまま起動できます。",
            project.title, project.title
        ),
        profile_id,
        profile_name: project.title,
        version_name: version.version_number,
    })
}

pub async fn update_modrinth_modpack_profile(
    app: &AppHandle,
    profile_id: String,
    game_version: Option<String>,
    operation_id: Option<String>,
) -> Result<ModpackInstallResult, String> {
    let profile = find_profile(&profile_id)?;
    let Some(project_id) = profile.modpack_project_id.clone() else {
        return Err("この起動構成には Modpack の追跡情報がありません。".to_string());
    };

    let operation_id = operation_id.unwrap_or_else(|| format!("modpack-update-{profile_id}"));
    let client = modrinth_client()?;
    emit_progress(
        app,
        &operation_id,
        "Modpack を更新中",
        "更新可能な Modpack バージョンを確認しています。",
        6.0,
    );

    let project = fetch_project_details(client, &project_id).await?;
    let version = fetch_modpack_version(
        client,
        &project_id,
        None,
        game_version.as_deref().or(profile.game_version.as_deref()),
    )
    .await?;

    if profile
        .modpack_version_id
        .as_deref()
        .is_some_and(|current| current == version.id)
    {
        return Ok(ModpackInstallResult {
            message: format!("{} はすでに最新です。", profile.name),
            profile_id,
            profile_name: profile.name,
            version_name: version.version_number,
        });
    }

    let file = pick_modpack_file(&version)
        .ok_or_else(|| "Modpack の .mrpack ファイルが見つかりませんでした。".to_string())?;
    emit_progress(
        app,
        &operation_id,
        "Modpack を更新中",
        format!("{} をダウンロードしています。", file.filename),
        16.0,
    );
    let pack_bytes = client
        .get(&file.url)
        .send()
        .await
        .map_err(|error| format!("Modpack をダウンロードできませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Modpack のダウンロードに失敗しました: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("Modpack の内容を読み取れませんでした: {error}"))?;

    let pack = read_modpack_index(&pack_bytes)?;
    let (loader, minecraft_version, loader_version) = resolve_modpack_dependencies(&pack)?;

    let (version_id, _) = ensure_loader_with_progress(
        app,
        &operation_id,
        "Modpack を更新中",
        &loader,
        &minecraft_version,
        loader_version.as_deref(),
        28.0,
        48.0,
    )
    .await?;

    emit_progress(
        app,
        &operation_id,
        "Modpack を更新中",
        "ファイルを更新しています。",
        50.0,
    );
    install_modpack_files(
        app,
        &operation_id,
        client,
        &pack,
        &pack_bytes,
        Path::new(&profile.game_dir),
    )
    .await?;

    track_modpack_projects(&profile_id, &pack)?;
    update_profile_runtime_and_modpack(
        &profile_id,
        &version_id,
        Some(&project_id),
        Some(&version.id),
    )?;

    emit_progress(
        app,
        &operation_id,
        "Modpack を更新中",
        "アップデートが完了しました。",
        100.0,
    );

    Ok(ModpackInstallResult {
        message: format!("{} を更新しました。", project.title),
        profile_id,
        profile_name: profile.name,
        version_name: version.version_number,
    })
}

pub async fn import_local_modpack(
    app: &AppHandle,
    mrpack_path: String,
    profile_name: Option<String>,
    operation_id: Option<String>,
) -> Result<ModpackInstallResult, String> {
    let operation_id = operation_id
        .unwrap_or_else(|| format!("modpack-import-{}", chrono::Local::now().timestamp_millis()));
    let source_path = PathBuf::from(mrpack_path.trim());
    if !source_path.exists() {
        return Err("指定された Modpack アーカイブが見つかりません。".to_string());
    }

    emit_progress(
        app,
        &operation_id,
        "Modpack を取り込み中",
        "ローカル Modpack アーカイブを読み込んでいます。",
        8.0,
    );
    let pack_bytes = fs::read(&source_path)
        .map_err(|error| format!("Modpack アーカイブを読み込めませんでした: {error}"))?;
    let pack = read_modpack_index(&pack_bytes)?;
    let curseforge_manifest = read_curseforge_manifest(&pack_bytes).ok();
    let (loader, minecraft_version, loader_version) = resolve_modpack_dependencies(&pack)?;

    let (version_id, _) = ensure_loader_with_progress(
        app,
        &operation_id,
        "Modpack を取り込み中",
        &loader,
        &minecraft_version,
        loader_version.as_deref(),
        24.0,
        44.0,
    )
    .await?;

    let root = minecraft_root()?;
    let fallback_name = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Imported Modpack")
        .to_string();
    let requested_name = profile_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| pack.name.clone())
        .unwrap_or(fallback_name);

    let game_dir = modpack_instance_dir(&root, &loader, &requested_name);
    fs::create_dir_all(&game_dir)
        .map_err(|error| format!("{} を準備できませんでした: {error}", game_dir.display()))?;

    emit_progress(
        app,
        &operation_id,
        "Modpack を取り込み中",
        "overrides と Mod ファイルを展開しています。",
        46.0,
    );
    let client = modrinth_client()?;
    install_modpack_files(app, &operation_id, client, &pack, &pack_bytes, &game_dir).await?;
    let curseforge_installed_files = if let Some(manifest) = curseforge_manifest.as_ref() {
        install_curseforge_manifest_files(app, &operation_id, &client, manifest, &game_dir).await?
    } else {
        HashMap::new()
    };

    let profile_id = upsert_custom_profile(CustomProfileDraft {
        name: requested_name.clone(),
        icon: Some("Grass".to_string()),
        custom_icon_url: None,
        background_image_url: None,
        game_dir: game_dir.clone(),
        last_version_id: version_id.clone(),
    })?;

    track_modpack_projects(&profile_id, &pack)?;
    if let Some(manifest) = curseforge_manifest.as_ref() {
        track_curseforge_manifest_projects(&profile_id, manifest, &curseforge_installed_files)?;
    }
    update_profile_runtime_and_modpack(&profile_id, &version_id, None, pack.version_id.as_deref())?;

    emit_progress(
        app,
        &operation_id,
        "Modpack を取り込み中",
        "起動構成の作成が完了しました。",
        100.0,
    );

    let import_message = if curseforge_manifest.is_some() {
        format!(
            "{} を CurseForge 互換アーカイブとして取り込みました。",
            requested_name
        )
    } else {
        format!(
            "{} をローカル Modpack アーカイブから取り込みました。",
            requested_name
        )
    };

    Ok(ModpackInstallResult {
        message: import_message,
        profile_id,
        profile_name: requested_name,
        version_name: pack
            .version_id
            .clone()
            .unwrap_or_else(|| "local".to_string()),
    })
}

pub fn export_profile_modpack(
    profile_id: String,
    output_path: String,
    format: String,
) -> Result<ModpackExportResult, String> {
    let profile = find_profile(&profile_id)?;
    let game_dir = PathBuf::from(&profile.game_dir);
    if !game_dir.exists() {
        return Err("起動構成の game directory が見つかりません。".to_string());
    }

    let minecraft_version = profile
        .game_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            profile
                .last_version_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .ok_or_else(|| {
            "Minecraft バージョン情報を決定できないためエクスポートできません。".to_string()
        })?;

    let loader = normalize_loader(Some(&profile.loader)).to_string();
    let format = format.trim().to_lowercase();
    let mut mod_loaders = Vec::new();
    let loader_version = if loader != "vanilla" {
        let loader_version = profile
            .loader_version
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                infer_loader_version_from_last_version_id(
                    &loader,
                    profile.last_version_id.as_deref(),
                )
            });
        let Some(loader_version) = loader_version else {
            return Err(
                "Loader バージョン情報を決定できないためエクスポートできません。".to_string(),
            );
        };
        Some(loader_version)
    } else {
        None
    };

    let output_path = output_path.trim();
    if output_path.is_empty() {
        return Err("書き出し先が指定されていません。".to_string());
    }
    let mut target_path = PathBuf::from(output_path);
    match format.as_str() {
        "curseforge" => {
            if target_path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| !value.eq_ignore_ascii_case("zip"))
                .unwrap_or(true)
            {
                target_path.set_extension("zip");
            }
        }
        "modrinth" => {
            if target_path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| !value.eq_ignore_ascii_case("mrpack"))
                .unwrap_or(true)
            {
                target_path.set_extension("mrpack");
            }
        }
        _ => return Err("書き出し形式が不明です。".to_string()),
    }
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("エクスポート先を作成できませんでした: {error}"))?;
    }
    let file = File::create(&target_path)
        .map_err(|error| format!("{} を作成できませんでした: {error}", target_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let json_options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let message = match format.as_str() {
        "curseforge" => {
            if let Some(loader_version) = loader_version {
                let loader_id = match loader.as_str() {
                    "quilt" => format!("quilt-loader-{loader_version}"),
                    other => format!("{other}-{loader_version}"),
                };
                mod_loaders.push(CurseforgeManifestLoader {
                    id: loader_id,
                    primary: Some(true),
                });
            }
            let tracked_entries = tracked_project_entries(&profile_id)?;
            let files = tracked_entries
                .into_iter()
                .filter_map(|entry| {
                    Some(CurseforgeManifestFile {
                        project_id: entry.curseforge_project_id?,
                        file_id: entry.curseforge_file_id?,
                        required: true,
                        is_locked: false,
                    })
                })
                .collect();
            let pack = CurseforgeManifest {
                name: Some(profile.name.clone()),
                version: profile
                    .modpack_version_id
                    .clone()
                    .or_else(|| profile.last_version_id.clone())
                    .or_else(|| Some("local-export".to_string())),
                author: Some("VanillaLauncher".to_string()),
                manifest_type: Some("minecraftModpack".to_string()),
                manifest_version: Some(1),
                overrides: Some("overrides".to_string()),
                minecraft: CurseforgeManifestMinecraft {
                    version: minecraft_version.clone(),
                    mod_loaders,
                },
                files,
            };
            let pack_json = serde_json::to_vec_pretty(&pack)
                .map_err(|error| format!("manifest.json を生成できませんでした: {error}"))?;
            zip.start_file("manifest.json", json_options)
                .map_err(|error| format!("manifest.json を追加できませんでした: {error}"))?;
            zip.write_all(&pack_json)
                .map_err(|error| format!("manifest.json を書き込めませんでした: {error}"))?;
            add_overrides_entry_to_zip(&mut zip, &game_dir, Path::new("config"))?;
            add_overrides_entry_to_zip(&mut zip, &game_dir, Path::new("resourcepacks"))?;
            add_overrides_entry_to_zip(&mut zip, &game_dir, Path::new("shaderpacks"))?;
            add_overrides_entry_to_zip(&mut zip, &game_dir, Path::new("options.txt"))?;
            add_overrides_entry_to_zip(&mut zip, &game_dir, Path::new("optionsof.txt"))?;
            format!(
                "{} を CurseForge 互換 zip でエクスポートしました。",
                profile.name
            )
        }
        "modrinth" => {
            let mut dependencies = HashMap::new();
            dependencies.insert("minecraft".to_string(), minecraft_version.clone());
            if let Some(loader_version) = loader_version {
                let dependency_key = match loader.as_str() {
                    "fabric" => "fabric-loader",
                    "quilt" => "quilt-loader",
                    other => other,
                };
                dependencies.insert(dependency_key.to_string(), loader_version);
            }
            let pack = ModpackIndex {
                format_version: Some(1),
                game: Some("minecraft".to_string()),
                version_id: profile
                    .modpack_version_id
                    .clone()
                    .or_else(|| profile.last_version_id.clone())
                    .or_else(|| Some("local-export".to_string())),
                name: Some(profile.name.clone()),
                summary: Some("VanillaLauncher でエクスポートした Modpack".to_string()),
                files: Vec::new(),
                dependencies,
                override_prefixes: Vec::new(),
            };
            let pack_json = serde_json::to_vec_pretty(&pack)
                .map_err(|error| format!("modrinth.index.json を生成できませんでした: {error}"))?;
            zip.start_file("modrinth.index.json", json_options)
                .map_err(|error| format!("modrinth.index.json を追加できませんでした: {error}"))?;
            zip.write_all(&pack_json)
                .map_err(|error| format!("modrinth.index.json を書き込めませんでした: {error}"))?;
            add_overrides_entry_to_zip(&mut zip, &game_dir, Path::new("mods"))?;
            add_overrides_entry_to_zip(&mut zip, &game_dir, Path::new("config"))?;
            add_overrides_entry_to_zip(&mut zip, &game_dir, Path::new("resourcepacks"))?;
            add_overrides_entry_to_zip(&mut zip, &game_dir, Path::new("shaderpacks"))?;
            add_overrides_entry_to_zip(&mut zip, &game_dir, Path::new("options.txt"))?;
            add_overrides_entry_to_zip(&mut zip, &game_dir, Path::new("optionsof.txt"))?;
            format!(
                "{} を Modrinth 互換 mrpack でエクスポートしました。",
                profile.name
            )
        }
        _ => unreachable!(),
    };

    let file = zip.finish().map_err(|error| match format.as_str() {
        "modrinth" => format!("mrpack の保存に失敗しました: {error}"),
        _ => format!("CurseForge 互換 zip の保存に失敗しました: {error}"),
    })?;
    let bytes = file
        .metadata()
        .map_err(|error| format!("ファイルサイズを取得できませんでした: {error}"))?
        .len();

    Ok(ModpackExportResult {
        message,
        file_path: target_path.to_string_lossy().to_string(),
        bytes,
    })
}

pub fn uninstall_modrinth_project(
    profile_id: String,
    project_id: String,
) -> Result<ActionResult, String> {
    let profile = find_profile(&profile_id)?;
    let mods_dir = resolve_profile_mods_dir(&profile_id, &profile.game_dir)?;
    let tracked_file_name = remove_tracked_project_by_project(&profile_id, &project_id)?;
    let Some(file_name) = tracked_file_name else {
        return Err(
            "この Mod はまだアプリ管理下で導入されていません。My Mods から手動削除もできます。"
                .to_string(),
        );
    };

    let file_path = mods_dir.join(&file_name);
    if file_path.exists() {
        fs::remove_file(&file_path)
            .map_err(|error| format!("Mod を削除できませんでした: {error}"))?;
    }

    Ok(ActionResult {
        message: format!("{} から {file_name} を削除しました。", profile.name),
        file_name,
    })
}

pub fn set_mod_enabled(
    profile_id: String,
    file_name: String,
    enabled: bool,
) -> Result<ActionResult, String> {
    crate::minecraft::validate_file_name(&file_name)?;
    let profile = find_profile(&profile_id)?;
    let mods_dir = resolve_profile_mods_dir(&profile_id, &profile.game_dir)?;
    fs::create_dir_all(&mods_dir)
        .map_err(|error| format!("mods フォルダを準備できませんでした: {error}"))?;

    let current_path = if enabled {
        if file_name.ends_with(".disabled") {
            mods_dir.join(&file_name)
        } else {
            mods_dir.join(format!("{file_name}.disabled"))
        }
    } else {
        mods_dir.join(&file_name)
    };

    if !current_path.exists() {
        return Err(format!(
            "{} が {} に見つかりません。",
            file_name,
            mods_dir.display()
        ));
    }

    let target_name = if enabled {
        file_name.trim_end_matches(".disabled").to_string()
    } else if file_name.ends_with(".disabled") {
        file_name.clone()
    } else {
        format!("{file_name}.disabled")
    };

    let target_path = mods_dir.join(&target_name);
    fs::rename(&current_path, &target_path)
        .map_err(|error| format!("Mod の有効状態を更新できませんでした: {error}"))?;
    rename_tracked_project_file(&profile_id, &file_name, &target_name)?;

    let message = if enabled {
        format!("{file_name} を再度有効化しました。")
    } else {
        format!("{file_name} を無効化しました。")
    };

    Ok(ActionResult {
        message,
        file_name: target_name,
    })
}

pub fn remove_mod(profile_id: String, file_name: String) -> Result<ActionResult, String> {
    crate::minecraft::validate_file_name(&file_name)?;
    let profile = find_profile(&profile_id)?;
    let mods_dir = resolve_profile_mods_dir(&profile_id, &profile.game_dir)?;
    let file_path = mods_dir.join(&file_name);

    if !file_path.exists() {
        return Err(format!(
            "{} が {} に見つかりません。",
            file_name,
            mods_dir.display()
        ));
    }

    fs::remove_file(&file_path).map_err(|error| format!("Mod を削除できませんでした: {error}"))?;
    remove_tracked_project_by_file(&profile_id, &file_name)?;

    Ok(ActionResult {
        message: format!("{} から {file_name} を削除しました。", profile.name),
        file_name,
    })
}

async fn fetch_projects_details(
    client: &Client,
    project_ids: &[String],
) -> Result<HashMap<String, ModrinthProject>, String> {
    if project_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut sorted_ids = project_ids.to_vec();
    sorted_ids.sort();

    let ids_json = serde_json::to_string(&sorted_ids)
        .map_err(|error| format!("Mod 情報の取得条件を生成できませんでした: {error}"))?;
    let body = fetch_json_with_cache(
        format!("projects|{ids_json}"),
        client
            .get(format!("{MODRINTH_API_BASE}/projects"))
            .query(&[("ids", ids_json.as_str())]),
        "Mod の詳細情報を取得できませんでした",
        "Mod 情報の取得に失敗しました",
        "Mod 情報を解析できませんでした",
    )
    .await?;
    let Some(items) = body.as_array() else {
        return Ok(HashMap::new());
    };

    Ok(items
        .iter()
        .filter_map(parse_project_details)
        .map(|project| (project.project_id.clone(), project))
        .collect())
}

async fn fetch_compatible_release(
    client: &Client,
    project_id: &str,
    loader: &str,
    game_version: Option<&str>,
) -> Result<CompatibleRelease, String> {
    let mut request = client.get(format!("{MODRINTH_API_BASE}/project/{project_id}/version"));

    if loader != "vanilla" {
        let loader_json = serde_json::to_string(&vec![loader.to_string()])
            .map_err(|error| format!("Loader フィルターを生成できませんでした: {error}"))?;
        request = request.query(&[("loaders", loader_json.as_str())]);
    }

    if let Some(game_version) = game_version
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let version_json =
            serde_json::to_string(&vec![game_version.to_string()]).map_err(|error| {
                format!("ゲームバージョンのフィルターを生成できませんでした: {error}")
            })?;
        request = request.query(&[("game_versions", version_json.as_str())]);
    }

    let body = fetch_json_with_cache(
        format!(
            "compatible|{project_id}|{loader}|{}",
            game_version
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("-")
        ),
        request,
        "互換バージョンを取得できませんでした",
        "Modrinth のバージョン取得に失敗しました",
        "バージョン一覧を解析できませんでした",
    )
    .await?;
    let mut versions = parse_version_list(&body);
    versions.sort_by(|left, right| right.published_at.cmp(&left.published_at));

    for version in versions {
        if let Some(file) = pick_mod_file(&version) {
            return Ok(CompatibleRelease { version, file });
        }
    }

    Err("missing-compatible-file".to_string())
}

async fn fetch_project_details(
    client: &Client,
    project_id: &str,
) -> Result<ModrinthProject, String> {
    let body = fetch_json_with_cache(
        format!("project|{project_id}"),
        client.get(format!("{MODRINTH_API_BASE}/project/{project_id}")),
        "Modpack 情報を取得できませんでした",
        "Modpack の取得に失敗しました",
        "Modpack 情報を解析できませんでした",
    )
    .await?;

    parse_project_details(&body).ok_or_else(|| "Modpack 情報の形式が想定と異なります。".to_string())
}

async fn fetch_project_details_for_visual_cache(
    client: &Client,
    project_id: &str,
) -> Result<ModrinthProject, String> {
    let body = fetch_json_with_cache_for_ttl(
        format!("visual-project|{project_id}"),
        client.get(format!("{MODRINTH_API_BASE}/project/{project_id}")),
        "Modpack 情報を取得できませんでした",
        "Modpack の取得に失敗しました",
        "Modpack 情報を解析できませんでした",
        MODRINTH_VISUAL_CACHE_TTL,
    )
    .await?;

    parse_project_details(&body).ok_or_else(|| "Modpack 情報の形式が想定と異なります。".to_string())
}

async fn fetch_modpack_version(
    client: &Client,
    project_id: &str,
    selected_version_id: Option<&str>,
    game_version: Option<&str>,
) -> Result<ModrinthVersion, String> {
    let mut versions = fetch_modpack_versions(client, project_id).await?;
    versions.sort_by(|left, right| right.published_at.cmp(&left.published_at));

    if let Some(version_id) = selected_version_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(found) = versions
            .iter()
            .find(|entry| entry.id == version_id)
            .cloned()
        {
            return Ok(found);
        }

        return Err("選択された Modpack バージョンが見つかりませんでした。".to_string());
    }

    let normalized_game_version = game_version
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(game_version) = normalized_game_version {
        if let Some(found) = versions
            .iter()
            .find(|candidate| {
                candidate
                    .game_versions
                    .iter()
                    .any(|entry| entry.eq_ignore_ascii_case(game_version))
                    && pick_modpack_file(candidate).is_some()
            })
            .cloned()
        {
            return Ok(found);
        }

        return Err(format!(
            "指定バージョン {game_version} に対応する Modpack バージョンが見つかりませんでした。"
        ));
    }

    versions
        .into_iter()
        .find(|candidate| pick_modpack_file(candidate).is_some())
        .ok_or_else(|| "ダウンロード可能な Modpack バージョンが見つかりませんでした。".to_string())
}

async fn fetch_modpack_versions(
    client: &Client,
    project_id: &str,
) -> Result<Vec<ModrinthVersion>, String> {
    let body = fetch_json_with_cache(
        format!("modpack-version|{project_id}"),
        client.get(format!("{MODRINTH_API_BASE}/project/{project_id}/version")),
        "Modpack のバージョン一覧を取得できませんでした",
        "Modpack のバージョン取得に失敗しました",
        "Modpack のバージョン一覧を解析できませんでした",
    )
    .await?;

    Ok(parse_version_list(&body)
        .into_iter()
        .filter(|candidate| pick_modpack_file(candidate).is_some())
        .collect())
}

fn pick_modpack_file(version: &ModrinthVersion) -> Option<crate::models::ModrinthFile> {
    version
        .files
        .iter()
        .find(|candidate| {
            candidate.primary.unwrap_or(false) && candidate.filename.ends_with(".mrpack")
        })
        .cloned()
        .or_else(|| {
            version
                .files
                .iter()
                .find(|candidate| candidate.filename.ends_with(".mrpack"))
                .cloned()
        })
}

fn read_modpack_index(bytes: &[u8]) -> Result<ModpackIndex, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|error| format!("Modpack アーカイブを開けませんでした: {error}"))?;
    if let Some(entry_index) =
        find_zip_entry_index_by_file_name(&mut archive, "modrinth.index.json")?
    {
        let mut entry = archive
            .by_index(entry_index)
            .map_err(|error| format!("modrinth.index.json を読めませんでした: {error}"))?;
        let mut contents = String::new();
        entry
            .read_to_string(&mut contents)
            .map_err(|error| format!("modrinth.index.json を読み込めませんでした: {error}"))?;
        let mut index: ModpackIndex = serde_json::from_str(&contents)
            .map_err(|error| format!("modrinth.index.json を解析できませんでした: {error}"))?;
        if index.override_prefixes.is_empty() {
            index.override_prefixes = vec!["overrides".to_string(), "client-overrides".to_string()];
        }
        return Ok(index);
    }

    if let Some(entry_index) = find_zip_entry_index_by_file_name(&mut archive, "manifest.json")? {
        let mut entry = archive
            .by_index(entry_index)
            .map_err(|error| format!("manifest.json を読めませんでした: {error}"))?;
        let mut contents = String::new();
        entry
            .read_to_string(&mut contents)
            .map_err(|error| format!("manifest.json を読み込めませんでした: {error}"))?;
        let manifest: CurseforgeManifest = serde_json::from_str(&contents)
            .map_err(|error| format!("manifest.json を解析できませんでした: {error}"))?;
        return Ok(map_curseforge_manifest_to_modpack_index(manifest));
    }

    Err("modrinth.index.json または manifest.json が見つかりませんでした。Modpack 形式のアーカイブか確認してください。".to_string())
}

fn read_curseforge_manifest(bytes: &[u8]) -> Result<CurseforgeManifest, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|error| format!("Modpack アーカイブを開けませんでした: {error}"))?;
    let Some(entry_index) = find_zip_entry_index_by_file_name(&mut archive, "manifest.json")?
    else {
        return Err("manifest.json がありません。".to_string());
    };
    let mut entry = archive
        .by_index(entry_index)
        .map_err(|error| format!("manifest.json を読めませんでした: {error}"))?;
    let mut contents = String::new();
    entry
        .read_to_string(&mut contents)
        .map_err(|error| format!("manifest.json を読み込めませんでした: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("manifest.json を解析できませんでした: {error}"))
}

fn find_zip_entry_index_by_file_name<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    file_name: &str,
) -> Result<Option<usize>, String> {
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("アーカイブ内ファイルを読み取れませんでした: {error}"))?;
        let normalized = entry.name().replace('\\', "/");
        let current = normalized.rsplit('/').next().unwrap_or_default().trim();
        if current.eq_ignore_ascii_case(file_name) {
            return Ok(Some(index));
        }
    }

    Ok(None)
}

fn map_curseforge_manifest_to_modpack_index(manifest: CurseforgeManifest) -> ModpackIndex {
    let mut dependencies = HashMap::new();
    dependencies.insert("minecraft".to_string(), manifest.minecraft.version.clone());

    let loader_id = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|entry| entry.primary.unwrap_or(false))
        .or_else(|| manifest.minecraft.mod_loaders.first())
        .map(|entry| entry.id.as_str());

    if let Some(loader_id) = loader_id {
        if let Some((dependency_key, dependency_version)) = parse_curseforge_loader(loader_id) {
            dependencies.insert(dependency_key.to_string(), dependency_version);
        }
    }

    let files = manifest
        .files
        .into_iter()
        .filter(|file| file.required)
        .map(|file| ModpackFileEntry {
            path: format!("mods/curseforge-{}-{}.jar", file.project_id, file.file_id),
            downloads: Vec::new(),
            env: None,
        })
        .collect();

    ModpackIndex {
        format_version: Some(1),
        game: Some("minecraft".to_string()),
        version_id: manifest.version,
        name: manifest.name,
        summary: Some("CurseForge 形式の Modpack を取り込みました。".to_string()),
        files,
        dependencies,
        override_prefixes: vec![manifest
            .overrides
            .unwrap_or_else(|| "overrides".to_string())],
    }
}

fn parse_curseforge_loader(loader_id: &str) -> Option<(&'static str, String)> {
    let normalized = loader_id.trim().to_lowercase();
    for (prefix, key) in [
        ("fabric-", "fabric-loader"),
        ("quilt-loader-", "quilt-loader"),
        ("quilt-", "quilt-loader"),
        ("neoforge-", "neoforge"),
        ("forge-", "forge"),
    ] {
        if let Some(value) = normalized.strip_prefix(prefix) {
            if !value.trim().is_empty() {
                return Some((key, value.to_string()));
            }
        }
    }
    None
}

fn resolve_modpack_dependencies(
    pack: &ModpackIndex,
) -> Result<(String, String, Option<String>), String> {
    let minecraft_version = pack.dependencies.get("minecraft").cloned().ok_or_else(|| {
        "この Modpack には Minecraft バージョン依存情報がありません。".to_string()
    })?;

    for dependency_key in ["fabric-loader", "forge", "neoforge", "quilt-loader"] {
        if let Some(version) = pack.dependencies.get(dependency_key) {
            let loader = match dependency_key {
                "fabric-loader" => "fabric",
                "quilt-loader" => "quilt",
                other => other,
            };
            return Ok((loader.to_string(), minecraft_version, Some(version.clone())));
        }
    }

    Ok(("vanilla".to_string(), minecraft_version, None))
}

fn modpack_instance_dir(minecraft_root: &Path, loader: &str, profile_name: &str) -> PathBuf {
    let _ = loader;
    minecraft_root
        .join(".vanillalauncher")
        .join("instances")
        .join("modpacks")
        .join(
            profile_name
                .chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() {
                        character.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
                .trim_matches('-')
                .to_string(),
        )
}

async fn install_modpack_files(
    app: &AppHandle,
    operation_id: &str,
    client: &Client,
    pack: &ModpackIndex,
    pack_bytes: &[u8],
    game_dir: &Path,
) -> Result<(), String> {
    for (index, file) in pack.files.iter().enumerate() {
        if !should_install_on_client(file.env.as_ref()) {
            continue;
        }

        let Some(download_url) = file.downloads.first() else {
            continue;
        };
        let target_path = safe_join_instance_path(game_dir, &file.path)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
        }

        let percent = 46.0 + (((index + 1) as f64 / pack.files.len().max(1) as f64) * 38.0);
        emit_progress(
            app,
            operation_id,
            "Modpack を準備中",
            format!("{} を取得しています。", file.path),
            percent,
        );
        let bytes = client
            .get(download_url)
            .send()
            .await
            .map_err(|error| format!("{} をダウンロードできませんでした: {error}", file.path))?
            .error_for_status()
            .map_err(|error| format!("{} のダウンロードに失敗しました: {error}", file.path))?
            .bytes()
            .await
            .map_err(|error| format!("{} の内容を読み取れませんでした: {error}", file.path))?;
        fs::write(&target_path, &bytes).map_err(|error| {
            format!("{} を保存できませんでした: {error}", target_path.display())
        })?;
    }

    let override_prefixes = if pack.override_prefixes.is_empty() {
        vec!["overrides".to_string(), "client-overrides".to_string()]
    } else {
        pack.override_prefixes.clone()
    };

    for prefix in override_prefixes {
        extract_overrides_to(pack_bytes, game_dir, &prefix)?;
    }

    Ok(())
}

async fn install_curseforge_manifest_files(
    app: &AppHandle,
    operation_id: &str,
    client: &Client,
    manifest: &CurseforgeManifest,
    game_dir: &Path,
) -> Result<HashMap<(u64, u64), String>, String> {
    let mods_dir = game_dir.join("mods");
    fs::create_dir_all(&mods_dir)
        .map_err(|error| format!("{} を準備できませんでした: {error}", mods_dir.display()))?;

    let required_files: Vec<_> = manifest.files.iter().filter(|file| file.required).collect();
    let total = required_files.len().max(1);
    let mut installed = HashMap::new();

    for (index, file) in required_files.into_iter().enumerate() {
        let percent = 52.0 + (((index + 1) as f64 / total as f64) * 40.0);
        emit_progress(
            app,
            operation_id,
            "Modpack を準備中",
            format!(
                "CurseForge Mod {} / {} を取得しています。",
                index + 1,
                total
            ),
            percent,
        );

        let metadata =
            fetch_curseforge_file_metadata(client, file.project_id, file.file_id).await?;
        let download_url = build_curseforge_download_url(file.file_id, &metadata.file_name)?;
        let target_path = mods_dir.join(&metadata.file_name);
        let bytes = client
            .get(&download_url)
            .send()
            .await
            .map_err(|error| {
                format!(
                    "{} をダウンロードできませんでした: {error}",
                    metadata.file_name
                )
            })?
            .error_for_status()
            .map_err(|error| {
                format!(
                    "{} のダウンロードに失敗しました: {error}",
                    metadata.file_name
                )
            })?
            .bytes()
            .await
            .map_err(|error| {
                format!(
                    "{} の内容を読み取れませんでした: {error}",
                    metadata.file_name
                )
            })?;

        fs::write(&target_path, &bytes).map_err(|error| {
            format!("{} を保存できませんでした: {error}", target_path.display())
        })?;
        installed.insert((file.project_id, file.file_id), metadata.file_name);
    }

    Ok(installed)
}

fn should_install_on_client(env: Option<&ModpackFileEnv>) -> bool {
    let Some(env) = env else {
        return true;
    };

    env.client.as_deref().unwrap_or("required") != "unsupported"
}

fn track_modpack_projects(profile_id: &str, pack: &ModpackIndex) -> Result<(), String> {
    for file in &pack.files {
        if !should_install_on_client(file.env.as_ref()) {
            continue;
        }

        let normalized_path = file.path.replace('\\', "/");
        if !normalized_path.starts_with("mods/") {
            continue;
        }

        let file_name = Path::new(&normalized_path)
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("{} のファイル名を解釈できませんでした。", file.path))?;

        if !is_mod_archive(file_name) {
            continue;
        }

        let Some(download_url) = file.downloads.first() else {
            continue;
        };
        let Some(project_id) = parse_modrinth_project_id_from_download_url(download_url) else {
            continue;
        };

        track_installed_project(profile_id, &project_id, file_name)?;
    }

    Ok(())
}

fn track_curseforge_manifest_projects(
    profile_id: &str,
    manifest: &CurseforgeManifest,
    installed_files: &HashMap<(u64, u64), String>,
) -> Result<(), String> {
    for file in &manifest.files {
        if !file.required {
            continue;
        }

        let file_name = installed_files
            .get(&(file.project_id, file.file_id))
            .cloned()
            .unwrap_or_else(|| format!("curseforge-{}-{}.jar", file.project_id, file.file_id));
        track_installed_project_with_source(
            profile_id,
            TrackedModSource {
                source: Some("curseforge".to_string()),
                project_id: format!("curseforge:{}", file.project_id),
                curseforge_project_id: Some(file.project_id),
                curseforge_file_id: Some(file.file_id),
            },
            &file_name,
        )?;
    }

    Ok(())
}

fn parse_modrinth_project_id_from_download_url(url: &str) -> Option<String> {
    let marker = "/data/";
    let start = url.find(marker)? + marker.len();
    let project_id = url[start..].split('/').next()?.trim();
    if project_id.is_empty() {
        return None;
    }

    if !project_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        return None;
    }

    Some(project_id.to_string())
}

fn infer_loader_version_from_last_version_id(
    loader: &str,
    last_version_id: Option<&str>,
) -> Option<String> {
    let value = last_version_id?.trim();
    if value.is_empty() {
        return None;
    }
    match loader {
        "fabric" => {
            let marker = "fabric-loader-";
            let rest = value.strip_prefix(marker)?;
            let (loader_version, _) = rest.rsplit_once('-')?;
            Some(loader_version.to_string())
        }
        "quilt" => {
            let marker = "quilt-loader-";
            let rest = value.strip_prefix(marker)?;
            let (loader_version, _) = rest.rsplit_once('-')?;
            Some(loader_version.to_string())
        }
        "forge" | "neoforge" => {
            let marker = format!("{loader}-");
            let rest = value.strip_prefix(&marker)?;
            let (_, loader_version) = rest.split_once('-')?;
            Some(loader_version.to_string())
        }
        _ => None,
    }
}

fn add_overrides_entry_to_zip(
    zip: &mut ZipWriter<File>,
    game_dir: &Path,
    entry: &Path,
) -> Result<(), String> {
    let source = game_dir.join(entry);
    if !source.exists() {
        return Ok(());
    }

    if source.is_file() {
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let target = format!("overrides/{}", entry.to_string_lossy().replace('\\', "/"));
        zip.start_file(target, options)
            .map_err(|error| format!("mrpack へファイルを追加できませんでした: {error}"))?;
        let bytes = fs::read(&source)
            .map_err(|error| format!("{} を読み込めませんでした: {error}", source.display()))?;
        zip.write_all(&bytes)
            .map_err(|error| format!("mrpack へ書き込めませんでした: {error}"))?;
        return Ok(());
    }

    add_overrides_directory_to_zip(zip, game_dir, &source)
}

fn add_overrides_directory_to_zip(
    zip: &mut ZipWriter<File>,
    game_dir: &Path,
    current: &Path,
) -> Result<(), String> {
    let entries = fs::read_dir(current)
        .map_err(|error| format!("{} を読み取れませんでした: {error}", current.display()))?;

    for entry in entries {
        let entry = entry
            .map_err(|error| format!("ディレクトリエントリを読み取れませんでした: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            add_overrides_directory_to_zip(zip, game_dir, &path)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }

        let relative = path.strip_prefix(game_dir).map_err(|error| {
            format!(
                "{} の相対パスを解決できませんでした: {error}",
                path.display()
            )
        })?;
        let target = format!(
            "overrides/{}",
            relative.to_string_lossy().replace('\\', "/")
        );
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file(target, options)
            .map_err(|error| format!("mrpack へファイルを追加できませんでした: {error}"))?;
        let bytes = fs::read(&path)
            .map_err(|error| format!("{} を読み込めませんでした: {error}", path.display()))?;
        zip.write_all(&bytes)
            .map_err(|error| format!("mrpack へ書き込めませんでした: {error}"))?;
    }

    Ok(())
}

fn extract_overrides_to(pack_bytes: &[u8], game_dir: &Path, prefix: &str) -> Result<(), String> {
    let cursor = std::io::Cursor::new(pack_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|error| format!("Modpack アーカイブを開けませんでした: {error}"))?;
    let normalized_prefix = format!("{}/", prefix.trim_matches('/'));

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Modpack 内のファイルを読み取れませんでした: {error}"))?;
        let entry_name = entry.name().replace('\\', "/");
        if !entry_name.starts_with(&normalized_prefix) {
            continue;
        }

        let relative = &entry_name[normalized_prefix.len()..];
        if relative.is_empty() {
            continue;
        }

        let target_path = safe_join_instance_path(game_dir, relative)?;
        if entry.is_dir() {
            fs::create_dir_all(&target_path).map_err(|error| {
                format!("{} を作成できませんでした: {error}", target_path.display())
            })?;
            continue;
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
        }

        let mut output = File::create(&target_path).map_err(|error| {
            format!("{} を作成できませんでした: {error}", target_path.display())
        })?;
        std::io::copy(&mut entry, &mut output).map_err(|error| {
            format!("{} を書き込めませんでした: {error}", target_path.display())
        })?;
        output.flush().map_err(|error| {
            format!("{} を保存できませんでした: {error}", target_path.display())
        })?;
    }

    Ok(())
}

fn safe_join_instance_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative_path = Path::new(relative);
    let mut safe = PathBuf::new();

    for component in relative_path.components() {
        match component {
            Component::Normal(segment) => safe.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(format!("不正なパスを含むため展開できません: {relative}"));
            }
        }
    }

    Ok(root.join(safe))
}

fn build_search_facets(
    project_type: &str,
    loader: &str,
    game_version: Option<&str>,
) -> Result<String, String> {
    let mut facets = vec![vec![format!("project_type:{project_type}")]];

    if loader != "vanilla" {
        facets.push(vec![format!("categories:{loader}")]);
    }

    if let Some(game_version) = game_version
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        facets.push(vec![format!("versions:{game_version}")]);
    }

    serde_json::to_string(&facets)
        .map_err(|error| format!("検索フィルターを生成できませんでした: {error}"))
}

async fn fetch_curseforge_file_metadata(
    client: &Client,
    project_id: u64,
    file_id: u64,
) -> Result<CurseforgeWebsiteFile, String> {
    let response = client
        .get(format!(
            "https://www.curseforge.com/api/v1/mods/{project_id}/files/{file_id}"
        ))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| format!("CurseForge ファイル情報を取得できませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("CurseForge ファイル情報の取得に失敗しました: {error}"))?;

    let body: CurseforgeWebsiteFileResponse = response
        .json()
        .await
        .map_err(|error| format!("CurseForge ファイル情報を解析できませんでした: {error}"))?;

    Ok(body.data)
}

async fn fetch_curseforge_file_metadata_for_visual_cache(
    client: &Client,
    project_id: u64,
    file_id: u64,
) -> Result<CurseforgeWebsiteFile, String> {
    let body = fetch_json_with_cache_for_ttl(
        format!("visual-curseforge-file|{project_id}|{file_id}"),
        client
            .get(format!(
                "https://www.curseforge.com/api/v1/mods/{project_id}/files/{file_id}"
            ))
            .header("Accept", "application/json"),
        "CurseForge ファイル情報を取得できませんでした",
        "CurseForge ファイル情報の取得に失敗しました",
        "CurseForge ファイル情報を解析できませんでした",
        MODRINTH_VISUAL_CACHE_TTL,
    )
    .await?;

    serde_json::from_value::<CurseforgeWebsiteFileResponse>(body)
        .map(|entry| entry.data)
        .map_err(|error| format!("CurseForge ファイル情報を解析できませんでした: {error}"))
}

async fn fetch_curseforge_files(
    client: &Client,
    project_id: u64,
) -> Result<Vec<CurseforgeWebsiteFile>, String> {
    let response = client
        .get(format!(
            "https://www.curseforge.com/api/v1/mods/{project_id}/files?pageSize=50&index=0"
        ))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| format!("CurseForge ファイル一覧を取得できませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("CurseForge ファイル一覧の取得に失敗しました: {error}"))?;

    let body: CurseforgeWebsiteFilesResponse = response
        .json()
        .await
        .map_err(|error| format!("CurseForge ファイル一覧を解析できませんでした: {error}"))?;

    Ok(body.data)
}

async fn fetch_latest_curseforge_file(
    client: &Client,
    project_id: u64,
    loader: &str,
    game_version: Option<&str>,
) -> Result<CurseforgeWebsiteFile, String> {
    let files = fetch_curseforge_files(client, project_id).await?;
    files
        .into_iter()
        .find(|file| {
            curseforge_file_matches_loader(file, loader)
                && game_version
                    .is_none_or(|version| curseforge_file_matches_game_version(file, version))
        })
        .ok_or_else(|| "CurseForge に互換ファイルが見つかりませんでした。".to_string())
}

async fn resolve_curseforge_project_query(
    query: &str,
    loader: &str,
    game_version: Option<&str>,
) -> Result<Option<ModrinthProject>, String> {
    let Some(project_id) = parse_curseforge_query_project_id(query) else {
        return Ok(None);
    };
    let client = modrinth_client()?;
    let file = fetch_latest_curseforge_file(client, project_id, loader, game_version).await?;
    Ok(Some(ModrinthProject {
        project_id: format!("curseforge:{project_id}"),
        source: "curseforge".to_string(),
        slug: project_id.to_string(),
        title: format!("CurseForge #{project_id}"),
        author: file
            .user
            .as_ref()
            .and_then(|user| user.display_name.clone())
            .unwrap_or_else(|| "作者不明".to_string()),
        description: format!("CurseForge 互換ファイル: {}", file.display_name),
        downloads: 0,
        followers: 0,
        categories: file.game_versions.clone(),
        versions: file.game_versions.clone(),
        icon_url: file
            .user
            .as_ref()
            .and_then(|user| user.twitch_avatar_url.clone()),
        image_url: None,
        latest_version: Some(file.display_name.clone()),
        updated_at: file.date_created.clone(),
        client_side: None,
        server_side: None,
        project_url: build_curseforge_file_info_url(project_id, file.id),
    }))
}

async fn search_curseforge_projects_official(
    query: &str,
    loader: &str,
    game_version: Option<&str>,
) -> Result<Vec<ModrinthProject>, String> {
    let Some(api_key) = curseforge_api_key() else {
        return Ok(Vec::new());
    };

    let client = modrinth_client()?;
    let mut request = client
        .get(format!("{CURSEFORGE_API_BASE}/v1/mods/search"))
        .header("Accept", "application/json")
        .header("x-api-key", api_key)
        .query(&[
            ("gameId", CURSEFORGE_MINECRAFT_GAME_ID),
            ("classId", CURSEFORGE_MINECRAFT_MOD_CLASS_ID),
            ("pageSize", "18"),
        ]);

    if !query.is_empty() {
        request = request.query(&[("searchFilter", query)]);
    } else {
        request = request.query(&[("sortField", "2")]);
    }

    if let Some(game_version) = game_version {
        request = request.query(&[("gameVersion", game_version)]);
        if let Some(mod_loader_type) = curseforge_loader_type(loader) {
            request = request.query(&[("modLoaderType", mod_loader_type)]);
        }
    }

    let body: Value = request
        .send()
        .await
        .map_err(|error| format!("CurseForge 検索に接続できませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("CurseForge 検索に失敗しました: {error}"))?
        .json()
        .await
        .map_err(|error| format!("CurseForge の検索結果を解析できませんでした: {error}"))?;

    Ok(body
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(parse_curseforge_search_hit)
        .collect())
}

fn parse_curseforge_project_id(value: &str) -> Option<u64> {
    value
        .strip_prefix("curseforge:")
        .and_then(|text| text.parse::<u64>().ok())
}

fn parse_curseforge_query_project_id(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(project_id) = parse_curseforge_project_id(trimmed) {
        return Some(project_id);
    }

    if trimmed.chars().all(|character| character.is_ascii_digit()) {
        return trimmed.parse::<u64>().ok();
    }

    None
}

fn curseforge_file_matches_loader(file: &CurseforgeWebsiteFile, loader: &str) -> bool {
    if loader == "vanilla" {
        return true;
    }

    let expected = match loader {
        "fabric" => "fabric",
        "forge" => "forge",
        "neoforge" => "neoforge",
        "quilt" => "quilt",
        other => other,
    };

    file.game_versions
        .iter()
        .any(|entry| entry.eq_ignore_ascii_case(expected))
}

fn curseforge_file_matches_game_version(file: &CurseforgeWebsiteFile, game_version: &str) -> bool {
    file.game_versions
        .iter()
        .any(|entry| entry.eq_ignore_ascii_case(game_version))
}

fn build_curseforge_file_info_url(project_id: u64, file_id: u64) -> String {
    format!("https://www.curseforge.com/api/v1/mods/{project_id}/files/{file_id}")
}

fn curseforge_loader_type(loader: &str) -> Option<&'static str> {
    match loader {
        "forge" => Some("1"),
        "fabric" => Some("4"),
        "quilt" => Some("5"),
        "neoforge" => Some("6"),
        _ => None,
    }
}

fn build_curseforge_download_url(file_id: u64, file_name: &str) -> Result<String, String> {
    let prefix = file_id / 1000;
    let suffix = file_id % 1000;
    let encoded_name = file_name.replace(' ', "%20").replace('+', "%2B");
    Ok(format!(
        "https://edge.forgecdn.net/files/{prefix}/{suffix:03}/{encoded_name}"
    ))
}

async fn fetch_json_with_cache(
    cache_key: String,
    request: reqwest::RequestBuilder,
    connect_error_label: &str,
    status_error_label: &str,
    parse_error_label: &str,
) -> Result<Value, String> {
    fetch_json_with_cache_for_ttl(
        cache_key,
        request,
        connect_error_label,
        status_error_label,
        parse_error_label,
        MODRINTH_CACHE_TTL,
    )
    .await
}

async fn fetch_json_with_cache_for_ttl(
    cache_key: String,
    request: reqwest::RequestBuilder,
    connect_error_label: &str,
    status_error_label: &str,
    parse_error_label: &str,
    ttl: Duration,
) -> Result<Value, String> {
    if let Some(value) = get_cached_json(&cache_key) {
        return Ok(value);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("{connect_error_label}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("{status_error_label}: {error}"))?;

    let body: Value = response
        .json()
        .await
        .map_err(|error| format!("{parse_error_label}: {error}"))?;
    put_cached_json(cache_key, body.clone(), ttl);
    Ok(body)
}

fn get_cached_json(cache_key: &str) -> Option<Value> {
    if !settings::get_app_settings().temp_cache_enabled {
        return None;
    }

    let path = cache_file_path(cache_key);
    if !path.exists() {
        return None;
    }

    let contents = fs::read_to_string(&path).ok()?;
    let entry = serde_json::from_str::<CachedJsonResponse>(&contents).ok()?;
    let now = chrono::Utc::now().timestamp_millis();
    if entry.expires_at_unix_ms <= now {
        let _ = fs::remove_file(&path);
        return None;
    }

    Some(entry.value)
}

fn put_cached_json(cache_key: String, value: Value, ttl: Duration) {
    if !settings::get_app_settings().temp_cache_enabled {
        return;
    }

    let cache_dir = settings::cache_dir();
    if fs::create_dir_all(&cache_dir).is_err() {
        return;
    }

    let path = cache_file_path(&cache_key);
    let now = chrono::Utc::now().timestamp_millis();
    let expires_at = now + ttl.as_millis() as i64;
    let entry = CachedJsonResponse {
        expires_at_unix_ms: expires_at,
        value,
    };
    if let Ok(text) = serde_json::to_string(&entry) {
        let _ = fs::write(path, text);
    }
}

fn cache_file_path(cache_key: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    cache_key.hash(&mut hasher);
    let digest = hasher.finish();
    settings::cache_dir().join(format!("modrinth-cache-{digest:016x}.json"))
}

fn modrinth_client() -> Result<&'static Client, String> {
    if let Some(client) = MODRINTH_CLIENT.get() {
        return Ok(client);
    }

    let client = Client::builder()
        .user_agent(MODRINTH_USER_AGENT)
        .build()
        .map_err(|error| format!("HTTP クライアントを作成できませんでした: {error}"))?;

    let _ = MODRINTH_CLIENT.set(client);
    MODRINTH_CLIENT
        .get()
        .ok_or_else(|| "HTTP クライアントの初期化に失敗しました。".to_string())
}

fn normalize_search_query(value: &str) -> String {
    match value.trim() {
        "" | "0" => String::new(),
        other => other.to_string(),
    }
}

fn curseforge_api_key() -> Option<String> {
    env::var("CURSEFORGE_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_search_hit(hit: &Value) -> Option<ModrinthProject> {
    let project_id = value_to_string(hit.get("project_id"))?;
    let slug = value_to_string(hit.get("slug")).unwrap_or_else(|| project_id.clone());
    let title = value_to_string(hit.get("title")).unwrap_or_else(|| slug.clone());
    let author = value_to_string(hit.get("author")).unwrap_or_else(|| "作者不明".to_string());
    let project_type =
        value_to_string(hit.get("project_type")).unwrap_or_else(|| "project".to_string());
    let description = value_to_string(hit.get("description"))
        .map(|text| collapse_whitespace(&text))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "説明はまだ取得できません。".to_string());
    let display_categories = value_to_string_vec(hit.get("display_categories"));
    let categories = if display_categories.is_empty() {
        value_to_string_vec(hit.get("categories"))
    } else {
        display_categories
    };

    Some(ModrinthProject {
        project_id,
        source: "modrinth".to_string(),
        slug: slug.clone(),
        title,
        author,
        description,
        downloads: value_to_u64(hit.get("downloads")),
        followers: value_to_u64(hit.get("follows")),
        categories,
        versions: value_to_string_vec(hit.get("versions")),
        icon_url: value_to_string(hit.get("icon_url")),
        image_url: value_to_string(hit.get("featured_gallery"))
            .or_else(|| first_gallery_image(hit.get("gallery"))),
        latest_version: value_to_string(hit.get("latest_version")),
        updated_at: value_to_string(hit.get("date_modified"))
            .or_else(|| value_to_string(hit.get("date_created"))),
        client_side: value_to_string(hit.get("client_side")),
        server_side: value_to_string(hit.get("server_side")),
        project_url: format!("https://modrinth.com/{project_type}/{slug}"),
    })
}

fn parse_curseforge_search_hit(hit: &Value) -> Option<ModrinthProject> {
    let project_id = value_to_u64(hit.get("id"));
    if project_id == 0 {
        return None;
    }

    let slug = value_to_string(hit.get("slug")).unwrap_or_else(|| project_id.to_string());
    let title = value_to_string(hit.get("name")).unwrap_or_else(|| slug.clone());
    let author = hit
        .get("authors")
        .and_then(Value::as_array)
        .and_then(|authors| authors.first())
        .and_then(|author| value_to_string(author.get("name")))
        .or_else(|| value_to_string(hit.get("author")))
        .unwrap_or_else(|| "作者不明".to_string());
    let description = value_to_string(hit.get("summary"))
        .map(|text| collapse_whitespace(&text))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "説明はまだ取得できません。".to_string());
    let categories = hit
        .get("categories")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| value_to_string(item.get("name")))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let icon_url = hit.get("logo").and_then(Value::as_object).and_then(|logo| {
        value_to_string(logo.get("thumbnailUrl")).or_else(|| value_to_string(logo.get("url")))
    });
    let project_url = hit
        .get("links")
        .and_then(Value::as_object)
        .and_then(|links| value_to_string(links.get("websiteUrl")))
        .unwrap_or_else(|| format!("https://www.curseforge.com/api/v1/mods/{project_id}"));

    Some(ModrinthProject {
        project_id: format!("curseforge:{project_id}"),
        source: "curseforge".to_string(),
        slug,
        title,
        author,
        description,
        downloads: value_to_u64(hit.get("downloadCount")),
        followers: 0,
        categories,
        versions: hit
            .get("latestFilesIndexes")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| value_to_string(item.get("gameVersion")))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        icon_url,
        image_url: None,
        latest_version: hit
            .get("latestFilesIndexes")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| value_to_string(item.get("filename"))),
        updated_at: value_to_string(hit.get("dateModified"))
            .or_else(|| value_to_string(hit.get("dateReleased"))),
        client_side: None,
        server_side: None,
        project_url,
    })
}

fn parse_project_details(value: &Value) -> Option<ModrinthProject> {
    let project_id = value_to_string(value.get("id"))?;
    let slug = value_to_string(value.get("slug")).unwrap_or_else(|| project_id.clone());
    let title = value_to_string(value.get("title")).unwrap_or_else(|| slug.clone());
    let project_type =
        value_to_string(value.get("project_type")).unwrap_or_else(|| "project".to_string());
    let author = value_to_string(value.get("author"))
        .or_else(|| value_to_string(value.get("team")))
        .unwrap_or_else(|| "作者不明".to_string());
    let description = value_to_string(value.get("description"))
        .map(|text| collapse_whitespace(&text))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "説明はまだ取得できません。".to_string());

    Some(ModrinthProject {
        project_id,
        source: "modrinth".to_string(),
        slug: slug.clone(),
        title,
        author,
        description,
        downloads: value_to_u64(value.get("downloads")),
        followers: value_to_u64(value.get("followers")),
        categories: value_to_string_vec(value.get("categories")),
        versions: value_to_string_vec(value.get("game_versions")),
        icon_url: value_to_string(value.get("icon_url")),
        image_url: value_to_string(value.get("featured_gallery"))
            .or_else(|| first_gallery_image(value.get("gallery"))),
        latest_version: value_to_string(value.get("latest_version")),
        updated_at: value_to_string(value.get("updated"))
            .or_else(|| value_to_string(value.get("published"))),
        client_side: value_to_string(value.get("client_side")),
        server_side: value_to_string(value.get("server_side")),
        project_url: format!("https://modrinth.com/{project_type}/{slug}"),
    })
}

fn parse_version_list(body: &Value) -> Vec<ModrinthVersion> {
    body.as_array()
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(parse_version_entry)
        .collect()
}

fn parse_version_entry(value: &Value) -> Option<ModrinthVersion> {
    let id = value_to_string(value.get("id"))?;
    let name = value_to_string(value.get("name"))
        .or_else(|| value_to_string(value.get("version_number")))?;
    let version_number = value_to_string(value.get("version_number"))
        .or_else(|| value_to_string(value.get("name")))
        .unwrap_or_else(|| name.clone());
    let files = value
        .get("files")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(parse_version_file)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(ModrinthVersion {
        id,
        name,
        version_number,
        game_versions: value_to_string_vec(value.get("game_versions")),
        published_at: value_to_string(value.get("date_published"))
            .or_else(|| value_to_string(value.get("published"))),
        files,
    })
}

fn parse_version_file(value: &Value) -> Option<crate::models::ModrinthFile> {
    Some(crate::models::ModrinthFile {
        url: value_to_string(value.get("url"))?,
        filename: value_to_string(value.get("filename"))
            .or_else(|| value_to_string(value.get("name")))?,
        primary: value.get("primary").and_then(Value::as_bool),
    })
}

fn pick_mod_file(version: &ModrinthVersion) -> Option<crate::models::ModrinthFile> {
    version
        .files
        .iter()
        .find(|candidate| candidate.primary.unwrap_or(false) && is_mod_archive(&candidate.filename))
        .cloned()
        .or_else(|| {
            version
                .files
                .iter()
                .find(|candidate| is_mod_archive(&candidate.filename))
                .cloned()
        })
}

fn first_gallery_image(value: Option<&Value>) -> Option<String> {
    let items = value?.as_array()?;
    items.iter().find_map(|item| match item {
        Value::String(text) => Some(text.to_string()),
        Value::Object(object) => object
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    })
}

fn value_to_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.trim().to_string()),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        _ => None,
    }
}

fn value_to_u64(value: Option<&Value>) -> u64 {
    match value {
        Some(Value::Number(number)) => number.as_u64().unwrap_or_default(),
        Some(Value::String(text)) => text.parse::<u64>().unwrap_or_default(),
        _ => 0,
    }
}

fn value_to_string_vec(value: Option<&Value>) -> Vec<String> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| match item {
            Value::String(text) => Some(text.trim().to_string()),
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
        .collect()
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
