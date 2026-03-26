use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSnapshot {
    pub minecraft_root: String,
    pub launcher_available: bool,
    pub active_account: Option<ActiveLauncherAccount>,
    pub profiles: Vec<LauncherProfile>,
    pub summary: LauncherSummary,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ActiveLauncherAccount {
    pub username: String,
    pub auth_source: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSummary {
    pub profile_count: usize,
    pub mod_count: usize,
    pub enabled_mod_count: usize,
    pub disabled_mod_count: usize,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LauncherProfile {
    pub id: String,
    pub name: String,
    pub profile_type: String,
    pub icon: Option<String>,
    pub custom_icon_url: Option<String>,
    pub background_image_url: Option<String>,
    pub last_used: Option<String>,
    pub last_version_id: Option<String>,
    pub game_dir: String,
    pub game_version: Option<String>,
    pub loader: String,
    pub loader_version: Option<String>,
    pub modpack_project_id: Option<String>,
    pub modpack_version_id: Option<String>,
    pub launch_active: bool,
    pub mod_count: usize,
    pub enabled_mod_count: usize,
    pub disabled_mod_count: usize,
    pub mods: Vec<InstalledMod>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub file_name: String,
    pub display_name: String,
    pub source_project_id: Option<String>,
    pub mod_id: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub loader: Option<String>,
    pub authors: Vec<String>,
    pub enabled: bool,
    pub size_bytes: u64,
    pub modified_at: Option<u64>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModRemoteState {
    pub file_name: String,
    pub project_id: String,
    pub source: String,
    pub project_title: Option<String>,
    pub project_url: Option<String>,
    pub icon_url: Option<String>,
    pub latest_version: Option<String>,
    pub latest_file_name: Option<String>,
    pub published_at: Option<String>,
    pub update_available: bool,
    pub can_update: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub message: String,
    pub file_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub message: String,
    pub file_name: String,
    pub version_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackInstallResult {
    pub message: String,
    pub profile_id: String,
    pub profile_name: String,
    pub version_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackExportResult {
    pub message: String,
    pub file_path: String,
    pub bytes: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModpackVersionSummary {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub message: String,
    pub launch_mode: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XboxRpsStateResult {
    pub message: String,
    pub state_path: String,
    pub used_saved_state: bool,
    pub refreshed: bool,
    pub succeeded: bool,
    pub attempts_tried: usize,
    pub total_attempts: usize,
    pub source_path: Option<String>,
    pub variant_label: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressUpdate {
    pub operation_id: String,
    pub title: String,
    pub detail: String,
    pub percent: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricCatalog {
    pub minecraft_version: String,
    pub latest_installer: LoaderVersionSummary,
    pub recommended_loader: LoaderVersionSummary,
    pub available_game_versions: Vec<MinecraftVersionSummary>,
    pub available_loader_versions: Vec<LoaderVersionSummary>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersionSummary {
    pub id: String,
    pub stable: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersionSummary {
    pub id: String,
    pub stable: bool,
    pub kind: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricInstallResult {
    pub message: String,
    pub profile_id: String,
    pub profile_name: String,
    pub version_id: String,
    pub minecraft_version: String,
    pub loader_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderCatalog {
    pub loader: String,
    pub minecraft_version: String,
    pub installer_version: LoaderVersionSummary,
    pub recommended_loader: LoaderVersionSummary,
    pub available_game_versions: Vec<MinecraftVersionSummary>,
    pub available_loader_versions: Vec<LoaderVersionSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderInstallResult {
    pub message: String,
    pub loader: String,
    pub profile_id: String,
    pub profile_name: String,
    pub version_id: String,
    pub minecraft_version: String,
    pub loader_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthProject {
    pub project_id: String,
    pub source: String,
    pub slug: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub downloads: u64,
    pub followers: u64,
    pub categories: Vec<String>,
    pub versions: Vec<String>,
    pub icon_url: Option<String>,
    pub image_url: Option<String>,
    pub latest_version: Option<String>,
    pub updated_at: Option<String>,
    pub client_side: Option<String>,
    pub server_side: Option<String>,
    pub project_url: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub published_at: Option<String>,
    pub files: Vec<ModrinthFile>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    pub primary: Option<bool>,
}
