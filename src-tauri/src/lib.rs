mod app_log;
mod loaders;
mod minecraft;
mod models;
mod modrinth;
mod progress;
mod settings;

use models::{
    ActionResult, FabricCatalog, FabricInstallResult, InstallResult, LaunchResult,
    LauncherSnapshot, LoaderCatalog, LoaderInstallResult, ModRemoteState, ModpackExportResult,
    ModpackInstallResult, ModpackVersionSummary, ModrinthProject, XboxRpsStateResult,
};
use settings::{AppSettings, DebugExportResult, PerformanceLiteMode, SoftwareStatus};

#[tauri::command]
fn get_launcher_state() -> Result<LauncherSnapshot, String> {
    minecraft::load_launcher_snapshot()
}

#[tauri::command]
async fn search_modrinth_mods(
    query: String,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<Vec<ModrinthProject>, String> {
    modrinth::search_modrinth_mods(query, loader, game_version).await
}

#[tauri::command]
async fn search_modrinth_modpacks(
    query: String,
    game_version: Option<String>,
) -> Result<Vec<ModrinthProject>, String> {
    modrinth::search_modrinth_modpacks(query, game_version).await
}
#[tauri::command]
async fn import_local_modpack(
    app: tauri::AppHandle,
    mrpack_path: String,
    profile_name: Option<String>,
    operation_id: Option<String>,
) -> Result<ModpackInstallResult, String> {
    modrinth::import_local_modpack(&app, mrpack_path, profile_name, operation_id).await
}

#[tauri::command]
fn export_profile_modpack(
    profile_id: String,
    output_path: String,
    format: String,
) -> Result<ModpackExportResult, String> {
    modrinth::export_profile_modpack(profile_id, output_path, format)
}

#[tauri::command]
async fn get_modrinth_modpack_versions(
    project_id: String,
) -> Result<Vec<ModpackVersionSummary>, String> {
    modrinth::get_modrinth_modpack_versions(project_id).await
}

#[tauri::command]
async fn install_modrinth_project(
    app: tauri::AppHandle,
    profile_id: String,
    project_id: String,
    operation_id: Option<String>,
) -> Result<InstallResult, String> {
    modrinth::install_modrinth_project(&app, profile_id, project_id, operation_id).await
}

#[tauri::command]
async fn get_profile_mod_remote_states(profile_id: String) -> Result<Vec<ModRemoteState>, String> {
    modrinth::get_profile_mod_remote_states(profile_id).await
}

#[tauri::command]
async fn get_profile_mod_remote_state(
    profile_id: String,
    file_name: String,
) -> Result<Option<ModRemoteState>, String> {
    modrinth::get_profile_mod_remote_state(profile_id, file_name).await
}

#[tauri::command]
async fn get_profile_mod_visual_state(
    profile_id: String,
    file_name: String,
) -> Result<Option<ModRemoteState>, String> {
    modrinth::get_profile_mod_visual_state(profile_id, file_name).await
}

#[tauri::command]
async fn update_modrinth_project(
    app: tauri::AppHandle,
    profile_id: String,
    project_id: String,
    file_name: String,
    operation_id: Option<String>,
) -> Result<InstallResult, String> {
    modrinth::update_modrinth_project(&app, profile_id, project_id, file_name, operation_id).await
}

#[tauri::command]
async fn install_modrinth_modpack(
    app: tauri::AppHandle,
    project_id: String,
    version_id: Option<String>,
    operation_id: Option<String>,
    icon_url: Option<String>,
    image_url: Option<String>,
) -> Result<ModpackInstallResult, String> {
    modrinth::install_modrinth_modpack(
        &app,
        project_id,
        version_id,
        operation_id,
        icon_url,
        image_url,
    )
    .await
}

#[tauri::command]
async fn update_modrinth_modpack_profile(
    app: tauri::AppHandle,
    profile_id: String,
    game_version: Option<String>,
    operation_id: Option<String>,
) -> Result<ModpackInstallResult, String> {
    modrinth::update_modrinth_modpack_profile(&app, profile_id, game_version, operation_id).await
}

#[tauri::command]
fn delete_profile(profile_id: String) -> Result<ActionResult, String> {
    minecraft::delete_custom_profile(&profile_id)?;
    Ok(ActionResult {
        message: "起動構成を削除しました。".to_string(),
        file_name: profile_id,
    })
}

#[tauri::command]
fn update_profile_visuals(
    profile_id: String,
    custom_icon_url: Option<String>,
    background_image_url: Option<String>,
) -> Result<ActionResult, String> {
    minecraft::update_profile_visuals(&profile_id, custom_icon_url, background_image_url)?;
    Ok(ActionResult {
        message: "起動構成の見た目を更新しました。".to_string(),
        file_name: profile_id,
    })
}

#[tauri::command]
fn update_profile_name(profile_id: String, profile_name: String) -> Result<ActionResult, String> {
    minecraft::update_profile_name(&profile_id, &profile_name)?;
    Ok(ActionResult {
        message: "起動構成名を更新しました。".to_string(),
        file_name: profile_id,
    })
}

#[tauri::command]
fn uninstall_modrinth_project(
    profile_id: String,
    project_id: String,
) -> Result<ActionResult, String> {
    modrinth::uninstall_modrinth_project(profile_id, project_id)
}

#[tauri::command]
fn set_mod_enabled(
    profile_id: String,
    file_name: String,
    enabled: bool,
) -> Result<ActionResult, String> {
    modrinth::set_mod_enabled(profile_id, file_name, enabled)
}

#[tauri::command]
fn remove_mod(profile_id: String, file_name: String) -> Result<ActionResult, String> {
    modrinth::remove_mod(profile_id, file_name)
}

#[tauri::command]
fn resolve_profile_path(profile_id: String, target: String) -> Result<String, String> {
    minecraft::resolve_profile_path(&profile_id, &target)
}

#[tauri::command]
async fn get_fabric_catalog(game_version: Option<String>) -> Result<FabricCatalog, String> {
    loaders::get_fabric_catalog(game_version).await
}

#[tauri::command]
async fn get_loader_catalog(
    loader: String,
    game_version: Option<String>,
) -> Result<LoaderCatalog, String> {
    loaders::get_loader_catalog(loader, game_version).await
}

#[tauri::command]
async fn install_fabric_loader(
    app: tauri::AppHandle,
    profile_id: Option<String>,
    minecraft_version: String,
    loader_version: Option<String>,
    profile_name: Option<String>,
    operation_id: Option<String>,
) -> Result<FabricInstallResult, String> {
    loaders::install_fabric_loader(
        &app,
        profile_id,
        minecraft_version,
        loader_version,
        profile_name,
        operation_id,
    )
    .await
}

#[tauri::command]
async fn install_loader(
    app: tauri::AppHandle,
    loader: String,
    profile_id: Option<String>,
    minecraft_version: String,
    loader_version: Option<String>,
    profile_name: Option<String>,
    operation_id: Option<String>,
) -> Result<LoaderInstallResult, String> {
    loaders::install_loader(
        &app,
        loader,
        profile_id,
        minecraft_version,
        loader_version,
        profile_name,
        operation_id,
    )
    .await
}

#[tauri::command]
async fn launch_profile_directly(
    app: tauri::AppHandle,
    profile_id: String,
) -> Result<LaunchResult, String> {
    loaders::launch_profile_directly(&app, profile_id).await
}

#[tauri::command]
fn launch_profile_in_official_launcher(profile_id: String) -> Result<LaunchResult, String> {
    loaders::launch_profile_in_official_launcher(profile_id)
}

#[tauri::command]
fn get_app_settings() -> Result<AppSettings, String> {
    Ok(settings::get_app_settings())
}

#[tauri::command]
fn update_app_settings(
    temp_cache_enabled: bool,
    performance_lite_mode: PerformanceLiteMode,
) -> Result<ActionResult, String> {
    settings::update_app_settings(temp_cache_enabled, performance_lite_mode)
}

#[tauri::command]
fn ensure_java_runtime_available(
    app: tauri::AppHandle,
    operation_id: Option<String>,
) -> Result<ActionResult, String> {
    loaders::ensure_java_runtime_available_with_progress(&app, operation_id)
}

#[tauri::command]
fn clear_temp_cache() -> Result<ActionResult, String> {
    settings::clear_temp_cache()
}

#[tauri::command]
fn get_software_status() -> Result<SoftwareStatus, String> {
    settings::get_software_status()
}

#[tauri::command]
fn export_debug_log() -> Result<DebugExportResult, String> {
    settings::export_debug_log()
}

#[tauri::command]
fn clear_log() -> Result<ActionResult, String> {
    app_log::clear_log();
    Ok(ActionResult {
        message: "ログファイルをクリアしました".to_string(),
        file_name: "vanillalauncher.log".to_string(),
    })
}

#[tauri::command]
async fn ensure_xbox_rps_state(
    app: tauri::AppHandle,
    operation_id: Option<String>,
) -> Result<XboxRpsStateResult, String> {
    loaders::ensure_xbox_rps_state(Some(&app), operation_id.as_deref()).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // アプリケーション起動時にログファイルをクリア
    app_log::clear_log();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_launcher_state,
            search_modrinth_mods,
            search_modrinth_modpacks,
            get_modrinth_modpack_versions,
            install_modrinth_project,
            get_profile_mod_remote_states,
            get_profile_mod_remote_state,
            get_profile_mod_visual_state,
            update_modrinth_project,
            install_modrinth_modpack,
            update_modrinth_modpack_profile,
            import_local_modpack,
            export_profile_modpack,
            delete_profile,
            update_profile_visuals,
            update_profile_name,
            uninstall_modrinth_project,
            set_mod_enabled,
            remove_mod,
            resolve_profile_path,
            get_fabric_catalog,
            get_loader_catalog,
            install_fabric_loader,
            install_loader,
            launch_profile_directly,
            launch_profile_in_official_launcher,
            get_app_settings,
            update_app_settings,
            ensure_java_runtime_available,
            clear_temp_cache,
            get_software_status,
            export_debug_log,
            clear_log,
            ensure_xbox_rps_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
