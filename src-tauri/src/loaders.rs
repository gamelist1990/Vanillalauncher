use crate::{
    app_log,
    minecraft::{
        ensure_launcher_profiles_file, find_profile, minecraft_root, normalize_loader,
        open_official_launcher, profile_instance_dir, read_active_launcher_account,
        set_java_page_as_last_visited, set_profile_last_used, sync_profile_mods_to_game_dir,
        upsert_custom_profile, CustomProfileDraft,
    },
    models::{
        FabricCatalog, FabricInstallResult, LaunchResult, LoaderCatalog, LoaderInstallResult,
        LoaderVersionSummary, MinecraftVersionSummary,
    },
    progress::emit_progress,
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    env,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};
use tauri::AppHandle;
use zip::ZipArchive;

mod java_runtime;
mod launch_helpers;
mod launch_registry;
mod loader_management;
mod xbox_auth;

use launch_helpers::*;

const FABRIC_META_API_BASE: &str = "https://meta.fabricmc.net/v2";
const FABRIC_USER_AGENT: &str = "vanillalauncher/0.1.0 (loader-install)";
const MOJANG_VERSION_MANIFEST_URL: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const MOJANG_USER_AGENT: &str = "vanillalauncher/0.1.0 (minecraft-launch)";
const QUILT_INSTALLER_DOWNLOAD_URL: &str =
    "https://quiltmc.org/api/v1/download-latest-installer/java-universal";
const QUILT_INSTALLER_METADATA_URL: &str =
    "https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-installer/maven-metadata.xml";
const QUILT_LOADER_METADATA_URL: &str =
    "https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-loader/maven-metadata.xml";
const FORGE_MAVEN_METADATA_URL: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
const NEOFORGE_MAVEN_METADATA_URL: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
const FORGE_MAVEN_BASE: &str = "https://maven.minecraftforge.net";
const NEOFORGE_MAVEN_BASE: &str = "https://maven.neoforged.net/releases";
#[derive(Debug, Deserialize)]
struct FabricGameVersionEntry {
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricLoaderEntry {
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricInstallerEntry {
    url: String,
    version: String,
    stable: bool,
}

#[derive(Debug, Clone)]
struct MavenLoaderEntry {
    minecraft_version: String,
    loader_version: String,
    combined_version: String,
    stable: bool,
}

pub fn is_profile_launch_active(profile_id: &str) -> bool {
    launch_registry::is_profile_launch_active(profile_id)
}

fn suppress_console_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x08000000);
    }
}

fn record_profile_launch(profile_id: &str, pid: u32) -> Result<(), String> {
    launch_registry::record_profile_launch(profile_id, pid)
}

fn clear_profile_launch(profile_id: &str, pid: u32) -> Result<(), String> {
    launch_registry::clear_profile_launch(profile_id, pid)
}

pub async fn get_fabric_catalog(game_version: Option<String>) -> Result<FabricCatalog, String> {
    loader_management::get_fabric_catalog(game_version).await
}

pub async fn get_loader_catalog(
    loader: String,
    game_version: Option<String>,
) -> Result<LoaderCatalog, String> {
    loader_management::get_loader_catalog(loader, game_version).await
}

pub async fn install_fabric_loader(
    app: &AppHandle,
    profile_id: Option<String>,
    minecraft_version: String,
    loader_version: Option<String>,
    profile_name: Option<String>,
    operation_id: Option<String>,
) -> Result<FabricInstallResult, String> {
    loader_management::install_fabric_loader(
        app,
        profile_id,
        minecraft_version,
        loader_version,
        profile_name,
        operation_id,
    )
    .await
}

pub async fn install_loader(
    app: &AppHandle,
    loader: String,
    profile_id: Option<String>,
    minecraft_version: String,
    loader_version: Option<String>,
    profile_name: Option<String>,
    operation_id: Option<String>,
) -> Result<LoaderInstallResult, String> {
    loader_management::install_loader(
        app,
        loader,
        profile_id,
        minecraft_version,
        loader_version,
        profile_name,
        operation_id,
    )
    .await
}

pub async fn ensure_loader_version_installed(
    loader: &str,
    minecraft_version: &str,
    loader_version: Option<&str>,
) -> Result<(String, String), String> {
    loader_management::ensure_loader_version_installed(loader, minecraft_version, loader_version)
        .await
}

pub async fn ensure_xbox_rps_state(
    app: Option<&AppHandle>,
    operation_id: Option<&str>,
) -> Result<crate::models::XboxRpsStateResult, String> {
    xbox_auth::ensure_xbox_rps_state(app, operation_id).await
}

pub async fn launch_profile_directly(
    _app: &AppHandle,
    profile_id: String,
) -> Result<LaunchResult, String> {
    let profile = find_profile(&profile_id)?;
    app_log::append_log(
        "INFO",
        format!(
            "launch_profile_directly start profile_id={} name={}",
            profile.id, profile.name
        ),
    );
    if is_profile_launch_active(&profile_id) {
        return Err(format!(
            "{} はまだ起動中です。ゲームが立ち上がるまで少し待ってください。",
            profile.name
        ));
    }

    let root = minecraft_root()?;
    ensure_launcher_profiles_file(&root)?;
    sync_profile_mods_to_game_dir(&profile_id)?;

    let version_id = resolve_launch_version_id(&profile).await?;
    let launch_context = prepare_direct_launch(&root, &profile, &version_id).await?;
    set_profile_last_used(&profile_id)?;

    if launch_context.auth_mode == "direct-runtime" {
        app_log::append_log(
            "WARN",
            format!(
                "direct launch fallback profile_id={} version_id={} auth_mode={}",
                profile.id, version_id, launch_context.auth_mode
            ),
        );
        let launch_mode = open_official_launcher()?;
        return Ok(LaunchResult {
            message: format!(
                "{} は有効な公式認証トークンを取得できなかったため、公式 Minecraft Launcher で起動しました。公式Launcher側でログイン更新後にそのままプレイしてください。",
                profile.name
            ),
            launch_mode,
        });
    }

    let mut command = Command::new(&launch_context.java_path);
    command
        .args(&launch_context.arguments)
        .current_dir(&launch_context.game_dir)
        .stdin(Stdio::null());
    suppress_console_window(&mut command);
    if cfg!(not(debug_assertions)) {
        command.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let child = command
        .spawn()
        .map_err(|error| format!("Minecraft Java を直接起動できませんでした: {error}"))?;
    track_profile_launch(profile_id.clone(), child)?;
    app_log::append_log(
        "INFO",
        format!(
            "direct launch spawned profile_id={} version_id={} auth_mode={}",
            profile_id, version_id, launch_context.auth_mode
        ),
    );

    let auth_detail = match launch_context.auth_mode.as_str() {
        "direct-account" => "ログイン状態を引き継いで直接起動しています。",
        _ => "保存済みのランチャー環境を利用して直接起動しています。",
    };

    Ok(LaunchResult {
        message: format!(
            "{} を Java から直接起動しました。{}",
            profile.name, auth_detail
        ),
        launch_mode: launch_context.auth_mode,
    })
}

pub fn launch_profile_in_official_launcher(profile_id: String) -> Result<LaunchResult, String> {
    let profile = find_profile(&profile_id)?;
    set_profile_last_used(&profile_id)?;
    sync_profile_mods_to_game_dir(&profile_id)?;
    let _ = set_java_page_as_last_visited();
    let launch_mode = open_official_launcher()?;

    Ok(LaunchResult {
        message: format!(
            "公式 Minecraft Launcher を開きました。{} を先頭に出るよう更新しています。",
            profile.name
        ),
        launch_mode,
    })
}

fn track_profile_launch(profile_id: String, mut child: Child) -> Result<(), String> {
    let pid = child.id();
    record_profile_launch(&profile_id, pid)?;

    std::thread::spawn(move || {
        let _ = child.wait();
        let _ = clear_profile_launch(&profile_id, pid);
    });

    Ok(())
}

#[derive(Debug, Clone)]
struct DirectLaunchContext {
    java_path: PathBuf,
    game_dir: PathBuf,
    arguments: Vec<String>,
    auth_mode: String,
}

#[derive(Debug, Clone)]
struct MergedVersionManifest {
    main_class: String,
    version_type: String,
    asset_index_name: String,
    asset_index_download: Option<VersionDownload>,
    logging_argument: Option<String>,
    logging_file: Option<VersionDownload>,
    libraries: Vec<Value>,
    version_jars: Vec<PathBuf>,
    jvm_arguments: Vec<Value>,
    game_arguments: Vec<Value>,
    legacy_game_arguments: Option<String>,
}

#[derive(Debug, Clone)]
struct VersionDownload {
    url: String,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct VersionLaunchAuth {
    username: String,
    uuid: String,
    access_token: String,
    client_id: String,
    xuid: String,
    user_properties: String,
    mode: String,
}

#[derive(Debug, Deserialize)]
struct MojangVersionManifest {
    latest: MojangLatestVersions,
    versions: Vec<MojangVersionEntry>,
}

#[derive(Debug, Deserialize)]
struct MojangLatestVersions {
    release: String,
    snapshot: String,
}

#[derive(Debug, Deserialize)]
struct MojangVersionEntry {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
    url: String,
}

async fn prepare_direct_launch(
    minecraft_root: &Path,
    profile: &crate::models::LauncherProfile,
    version_id: &str,
) -> Result<DirectLaunchContext, String> {
    fs::create_dir_all(Path::new(&profile.game_dir))
        .map_err(|error| format!("{} を準備できませんでした: {error}", profile.game_dir))?;

    let merged = ensure_version_ready(minecraft_root, version_id).await?;
    let java_path = find_game_java_executable()?;
    let auth = launch_helpers::resolve_launch_auth().await?;
    let native_dir =
        launch_helpers::prepare_native_directory(minecraft_root, version_id, &merged.libraries)?;
    let classpath =
        launch_helpers::build_classpath(&merged.libraries, &merged.version_jars, minecraft_root)?;
    let arguments = launch_helpers::build_launch_arguments(
        profile,
        version_id,
        &merged,
        &auth,
        minecraft_root,
        &native_dir,
        &classpath,
    )?;

    Ok(DirectLaunchContext {
        java_path,
        game_dir: PathBuf::from(&profile.game_dir),
        arguments,
        auth_mode: auth.mode,
    })
}

async fn resolve_launch_version_id(
    profile: &crate::models::LauncherProfile,
) -> Result<String, String> {
    let version_id = profile
        .last_version_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| profile.game_version.as_deref())
        .ok_or_else(|| "起動対象の Minecraft バージョンを決定できませんでした。".to_string())?;

    match version_id {
        "latest-release" => Ok(fetch_official_version_manifest().await?.latest.release),
        "latest-snapshot" => Ok(fetch_official_version_manifest().await?.latest.snapshot),
        other => Ok(other.to_string()),
    }
}

async fn ensure_version_ready(
    minecraft_root: &Path,
    version_id: &str,
) -> Result<MergedVersionManifest, String> {
    let client = mojang_client()?;
    let manifests = load_version_manifest_chain(&client, minecraft_root, version_id).await?;
    ensure_version_jars(&client, minecraft_root, &manifests).await?;
    let merged = merge_version_manifests(minecraft_root, &manifests)?;
    ensure_libraries(&client, &merged.libraries, minecraft_root).await?;

    if let Some(logging_file) = merged.logging_file.as_ref() {
        download_to_path(&client, &logging_file.url, &logging_file.path).await?;
    }

    if let Some(asset_index) = merged.asset_index_download.as_ref() {
        download_to_path(&client, &asset_index.url, &asset_index.path).await?;
        ensure_asset_objects(&client, &asset_index.path, minecraft_root).await?;
    }

    Ok(merged)
}

async fn load_version_manifest_chain(
    client: &Client,
    minecraft_root: &Path,
    version_id: &str,
) -> Result<Vec<Value>, String> {
    let mut manifests = Vec::new();
    let mut current_version_id = version_id.to_string();

    loop {
        let manifest =
            ensure_version_manifest_file(client, minecraft_root, &current_version_id).await?;
        current_version_id = manifest
            .get("inheritsFrom")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_default();
        manifests.push(manifest);

        if current_version_id.is_empty() {
            break;
        }
    }

    Ok(manifests)
}

async fn ensure_version_manifest_file(
    client: &Client,
    minecraft_root: &Path,
    version_id: &str,
) -> Result<Value, String> {
    let path = version_json_path(minecraft_root, version_id);
    if path.exists() {
        return read_json_file(&path);
    }

    let manifest = fetch_official_version_manifest().await?;
    let Some(entry) = manifest
        .versions
        .into_iter()
        .find(|entry| entry.id == version_id)
    else {
        return Err(format!("{version_id} のバージョン情報が見つかりません。"));
    };

    let response = client
        .get(&entry.url)
        .send()
        .await
        .map_err(|error| format!("{version_id} のバージョン情報を取得できませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("{version_id} のバージョン情報取得に失敗しました: {error}"))?;
    let body = response
        .text()
        .await
        .map_err(|error| format!("{version_id} のバージョン情報を読み取れませんでした: {error}"))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }
    fs::write(&path, body)
        .map_err(|error| format!("{} を保存できませんでした: {error}", path.display()))?;

    read_json_file(&path)
}

fn merge_version_manifests(
    minecraft_root: &Path,
    manifests: &[Value],
) -> Result<MergedVersionManifest, String> {
    let mut merged = MergedVersionManifest {
        main_class: String::new(),
        version_type: "release".to_string(),
        asset_index_name: String::new(),
        asset_index_download: None,
        logging_argument: None,
        logging_file: None,
        libraries: Vec::new(),
        version_jars: Vec::new(),
        jvm_arguments: Vec::new(),
        game_arguments: Vec::new(),
        legacy_game_arguments: None,
    };

    for manifest in manifests.iter().rev() {
        if let Some(main_class) = manifest.get("mainClass").and_then(Value::as_str) {
            merged.main_class = main_class.to_string();
        }
        if let Some(version_type) = manifest.get("type").and_then(Value::as_str) {
            merged.version_type = version_type.to_string();
        }
        if let Some(assets) = manifest.get("assets").and_then(Value::as_str) {
            merged.asset_index_name = assets.to_string();
        }
        if let Some(asset_index) = manifest.get("assetIndex").and_then(Value::as_object) {
            if let Some(id) = asset_index.get("id").and_then(Value::as_str) {
                merged.asset_index_name = id.to_string();
                if let Some(url) = asset_index.get("url").and_then(Value::as_str) {
                    merged.asset_index_download = Some(VersionDownload {
                        url: url.to_string(),
                        path: minecraft_root
                            .join("assets")
                            .join("indexes")
                            .join(format!("{id}.json")),
                    });
                }
            }
        }
        if let Some(libraries) = manifest.get("libraries").and_then(Value::as_array) {
            merged.libraries.extend(libraries.iter().cloned());
        }
        if let Some(arguments) = manifest.get("arguments").and_then(Value::as_object) {
            if let Some(jvm) = arguments.get("jvm").and_then(Value::as_array) {
                merged.jvm_arguments.extend(jvm.iter().cloned());
            }
            if let Some(game) = arguments.get("game").and_then(Value::as_array) {
                merged.game_arguments.extend(game.iter().cloned());
            }
        }
        if let Some(game_arguments) = manifest.get("minecraftArguments").and_then(Value::as_str) {
            merged.legacy_game_arguments = Some(game_arguments.to_string());
        }
        if let Some(logging) = manifest
            .get("logging")
            .and_then(|value| value.get("client"))
            .and_then(Value::as_object)
        {
            if let Some(argument) = logging.get("argument").and_then(Value::as_str) {
                merged.logging_argument = Some(argument.to_string());
            }
            if let Some(file) = logging.get("file").and_then(Value::as_object) {
                let file_id = file.get("id").and_then(Value::as_str);
                let file_url = file.get("url").and_then(Value::as_str);
                if let (Some(id), Some(url)) = (file_id, file_url) {
                    merged.logging_file = Some(VersionDownload {
                        url: url.to_string(),
                        path: minecraft_root.join("assets").join("log_configs").join(id),
                    });
                }
            }
        }

        if let Some(version_id) = manifest.get("id").and_then(Value::as_str) {
            let version_jar = version_jar_path(minecraft_root, version_id);
            if version_jar.exists() && !merged.version_jars.contains(&version_jar) {
                merged.version_jars.push(version_jar);
            }
        }
    }

    if merged.main_class.trim().is_empty() {
        return Err("起動に必要な mainClass を決定できませんでした。".to_string());
    }
    if merged.asset_index_name.trim().is_empty() {
        return Err("起動に必要な asset index を決定できませんでした。".to_string());
    }
    if merged.version_jars.is_empty() {
        return Err("起動に必要な version jar が見つかりませんでした。".to_string());
    }

    Ok(merged)
}

async fn ensure_version_jars(
    client: &Client,
    minecraft_root: &Path,
    manifests: &[Value],
) -> Result<(), String> {
    for manifest in manifests {
        let Some(version_id) = manifest.get("id").and_then(Value::as_str) else {
            continue;
        };
        let jar_path = version_jar_path(minecraft_root, version_id);
        if jar_path.exists() {
            continue;
        }
        let Some(download) = manifest
            .get("downloads")
            .and_then(|value| value.get("client"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        let Some(url) = download.get("url").and_then(Value::as_str) else {
            continue;
        };
        download_to_path(client, url, &jar_path).await?;
    }

    Ok(())
}

async fn ensure_libraries(
    client: &Client,
    libraries: &[Value],
    minecraft_root: &Path,
) -> Result<(), String> {
    let features = launch_feature_flags();

    for library in libraries {
        if !rule_set_allows(library.get("rules"), &features) {
            continue;
        }

        if let Some(artifact) =
            launch_helpers::resolve_library_artifact_download(library, minecraft_root)
        {
            download_to_path(client, &artifact.url, &artifact.path).await?;
        }

        if let Some(native) = resolve_library_native_download(library, minecraft_root) {
            download_to_path(client, &native.url, &native.path).await?;
        }
    }

    Ok(())
}

async fn ensure_asset_objects(
    client: &Client,
    asset_index_path: &Path,
    minecraft_root: &Path,
) -> Result<(), String> {
    let value = read_json_file(asset_index_path)?;
    let Some(objects) = value.get("objects").and_then(Value::as_object) else {
        return Ok(());
    };

    for entry in objects.values() {
        let Some(hash) = entry.get("hash").and_then(Value::as_str) else {
            continue;
        };
        let Some(prefix) = hash.get(0..2) else {
            continue;
        };
        let target_path = minecraft_root
            .join("assets")
            .join("objects")
            .join(prefix)
            .join(hash);
        let url = format!("https://resources.download.minecraft.net/{prefix}/{hash}");
        download_to_path(client, &url, &target_path).await?;
    }

    Ok(())
}

fn find_game_java_executable() -> Result<PathBuf, String> {
    java_runtime::find_game_java_executable()
}

fn find_java_executable() -> Result<PathBuf, String> {
    java_runtime::find_java_executable()
}

pub fn ensure_java_runtime_available_with_progress(
    app: &AppHandle,
    operation_id: Option<String>,
) -> Result<crate::models::ActionResult, String> {
    java_runtime::ensure_java_runtime_available_with_progress(app, operation_id)
}

fn ensure_managed_java_runtime(progress: Option<(&AppHandle, &str)>) -> Result<PathBuf, String> {
    java_runtime::ensure_managed_java_runtime(progress)
}

#[derive(Debug, Deserialize)]
struct FabricLoaderManifestEntry {
    loader: FabricLoaderEntry,
}
