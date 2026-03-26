use crate::models::ActionResult;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashSet,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

const APP_TEMP_DIR_NAME: &str = "VanillaLauncher";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PerformanceLiteMode {
    Auto,
    On,
    Off,
}

fn default_performance_lite_mode() -> PerformanceLiteMode {
    PerformanceLiteMode::Auto
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub temp_cache_enabled: bool,
    #[serde(default = "default_performance_lite_mode")]
    pub performance_lite_mode: PerformanceLiteMode,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareStatus {
    pub temp_root: String,
    pub cache_dir: String,
    pub settings_path: String,
    pub java_runtime_dir: String,
    pub app_log_path: String,
    pub temp_cache_enabled: bool,
    pub cache_file_count: usize,
    pub cache_total_bytes: u64,
    pub debug_export_dir: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DebugExportResult {
    pub file_path: String,
    pub bytes: u64,
}

pub fn temp_root_dir() -> PathBuf {
    env::temp_dir().join(APP_TEMP_DIR_NAME)
}

pub fn java_runtime_dir() -> PathBuf {
    env::temp_dir()
        .join("vanillalauncher")
        .join("java-runtime")
        .join("temurin-21")
}

pub fn cache_dir() -> PathBuf {
    temp_root_dir().join("modrinth-cache")
}

pub fn debug_export_dir() -> PathBuf {
    temp_root_dir().join("debug-exports")
}

pub fn settings_path() -> PathBuf {
    temp_root_dir().join("settings.json")
}

pub fn default_settings() -> AppSettings {
    AppSettings {
        temp_cache_enabled: true,
        performance_lite_mode: PerformanceLiteMode::Auto,
    }
}

pub fn load_settings() -> AppSettings {
    let path = settings_path();
    if !path.exists() {
        return default_settings();
    }

    let contents = match fs::read_to_string(&path) {
        Ok(value) => value,
        Err(_) => return default_settings(),
    };

    serde_json::from_str::<AppSettings>(&contents).unwrap_or_else(|_| default_settings())
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    ensure_dir_exists(&temp_root_dir())?;
    let path = settings_path();
    let text = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("設定を書き出しできませんでした: {error}"))?;
    fs::write(&path, text)
        .map_err(|error| format!("{} を保存できませんでした: {error}", path.display()))
}

pub fn get_app_settings() -> AppSettings {
    load_settings()
}

pub fn update_app_settings(
    temp_cache_enabled: bool,
    performance_lite_mode: PerformanceLiteMode,
) -> Result<ActionResult, String> {
    let settings = AppSettings {
        temp_cache_enabled,
        performance_lite_mode,
    };
    save_settings(&settings)?;

    Ok(ActionResult {
        message: "設定を更新しました。".to_string(),
        file_name: "settings.json".to_string(),
    })
}

pub fn clear_temp_cache() -> Result<ActionResult, String> {
    let cache = cache_dir();
    if cache.exists() {
        fs::remove_dir_all(&cache)
            .map_err(|error| format!("{} を削除できませんでした: {error}", cache.display()))?;
    }
    ensure_dir_exists(&cache)?;

    Ok(ActionResult {
        message: "Temp キャッシュをクリアしました。".to_string(),
        file_name: cache.to_string_lossy().to_string(),
    })
}

pub fn get_software_status() -> Result<SoftwareStatus, String> {
    ensure_dir_exists(&temp_root_dir())?;
    ensure_dir_exists(&cache_dir())?;
    ensure_dir_exists(&debug_export_dir())?;

    let settings = load_settings();
    let (count, bytes) = dir_size_and_count(&cache_dir())?;

    Ok(SoftwareStatus {
        temp_root: temp_root_dir().to_string_lossy().to_string(),
        cache_dir: cache_dir().to_string_lossy().to_string(),
        settings_path: settings_path().to_string_lossy().to_string(),
        java_runtime_dir: java_runtime_dir().to_string_lossy().to_string(),
        app_log_path: crate::app_log::log_file_path()
            .to_string_lossy()
            .to_string(),
        temp_cache_enabled: settings.temp_cache_enabled,
        cache_file_count: count,
        cache_total_bytes: bytes,
        debug_export_dir: debug_export_dir().to_string_lossy().to_string(),
    })
}

pub fn export_debug_log() -> Result<DebugExportResult, String> {
    ensure_dir_exists(&debug_export_dir())?;

    let status = get_software_status()?;
    let snapshot = crate::minecraft::load_launcher_snapshot().ok();
    let now = Utc::now();
    let file_name = format!("vanillalauncher-debug-{}.zip", now.format("%Y%m%d-%H%M%S"));
    let target = debug_export_dir().join(file_name);

    let payload = serde_json::to_vec_pretty(&json!({
        "exportedAt": now.to_rfc3339(),
        "softwareStatus": status,
        "launcherSummary": snapshot.as_ref().map(|item| &item.summary),
        "profiles": snapshot.as_ref().map(|item| item.profiles.iter().map(|profile| json!({
            "id": profile.id,
            "name": profile.name,
            "loader": profile.loader,
            "gameVersion": profile.game_version,
            "modCount": profile.mod_count,
            "enabledModCount": profile.enabled_mod_count,
            "disabledModCount": profile.disabled_mod_count,
        })).collect::<Vec<_>>()),
    }))
    .map_err(|error| format!("デバッグ情報を生成できませんでした: {error}"))?;

    let file = fs::File::create(&target)
        .map_err(|error| format!("{} を作成できませんでした: {error}", target.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    write_zip_bytes(&mut zip, "debug.json", &payload, options)?;
    add_file_to_zip(
        &mut zip,
        &crate::app_log::log_file_path(),
        "logs/vanillalauncher.log",
        options,
    )?;

    let mut seen = HashSet::new();
    for path in collect_official_launcher_logs()? {
        let name = format!(
            "logs/official/{}",
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("launcher.log")
        );
        if !seen.insert(name.clone()) {
            continue;
        }
        add_file_to_zip(&mut zip, &path, &name, options)?;
    }

    zip.finish()
        .map_err(|error| format!("{} を保存できませんでした: {error}", target.display()))?;

    let bytes = fs::metadata(&target)
        .map_err(|error| {
            format!(
                "{} のサイズを確認できませんでした: {error}",
                target.display()
            )
        })?
        .len();

    Ok(DebugExportResult {
        file_path: target.to_string_lossy().to_string(),
        bytes,
    })
}

fn write_zip_bytes(
    zip: &mut ZipWriter<fs::File>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<(), String> {
    zip.start_file(name, options)
        .map_err(|error| format!("{name} を ZIP に追加できませんでした: {error}"))?;
    zip.write_all(bytes)
        .map_err(|error| format!("{name} を ZIP に書き込めませんでした: {error}"))
}

fn add_file_to_zip(
    zip: &mut ZipWriter<fs::File>,
    source: &Path,
    name: &str,
    options: SimpleFileOptions,
) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }

    let bytes = fs::read(source)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", source.display()))?;
    write_zip_bytes(zip, name, &bytes, options)
}

fn collect_official_launcher_logs() -> Result<Vec<PathBuf>, String> {
    let minecraft_root = crate::minecraft::minecraft_root()?;
    let mut logs = Vec::new();
    for name in [
        "launcher_log.txt",
        "launcher_cef_log.txt",
        "launcher_accounts_microsoft_store.json",
        "launcher_msa_credentials_microsoft_store.bin",
        "launcher_entitlements_microsoft_store.json",
        "launcher_product_state_microsoft_store.json",
        "launcher_ui_state_microsoft_store.json",
    ] {
        let path = minecraft_root.join(name);
        if path.exists() {
            logs.push(path);
        }
    }

    if let Ok(entries) = fs::read_dir(&minecraft_root) {
        let mut rotated_logs = entries
            .filter_map(|entry| entry.ok().map(|value| value.path()))
            .filter(|path| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.starts_with("launcher_log") && name.ends_with(".txt"))
            })
            .collect::<Vec<_>>();
        rotated_logs.sort();
        logs.extend(rotated_logs);
    }

    logs.sort();
    logs.dedup();
    Ok(logs)
}

fn ensure_dir_exists(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("{} を準備できませんでした: {error}", path.display()))
}

fn dir_size_and_count(path: &Path) -> Result<(usize, u64), String> {
    if !path.exists() {
        return Ok((0, 0));
    }

    let mut file_count = 0usize;
    let mut total_bytes = 0u64;

    for entry in fs::read_dir(path)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", path.display()))?
    {
        let entry =
            entry.map_err(|error| format!("Temp キャッシュの走査に失敗しました: {error}"))?;
        let metadata = entry.metadata().map_err(|error| {
            format!(
                "{} の情報を取得できませんでした: {error}",
                entry.path().display()
            )
        })?;
        if metadata.is_file() {
            file_count += 1;
            total_bytes += metadata.len();
        }
    }

    Ok((file_count, total_bytes))
}
