use crate::{progress::emit_progress, settings};
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::AppHandle;
use zip::ZipArchive;

#[derive(Debug, Clone, Copy)]
struct JavaRuntimeSpec {
    major: u32,
    download_url: &'static str,
}

const MANAGED_JAVA_RUNTIMES: &[JavaRuntimeSpec] = &[
    JavaRuntimeSpec {
        major: 17,
        download_url:
            "https://api.adoptium.net/v3/binary/latest/17/ga/windows/x64/jre/hotspot/normal/eclipse?project=jdk",
    },
    JavaRuntimeSpec {
        major: 21,
        download_url:
            "https://api.adoptium.net/v3/binary/latest/21/ga/windows/x64/jre/hotspot/normal/eclipse?project=jdk",
    },
    JavaRuntimeSpec {
        major: 25,
        download_url:
            "https://api.adoptium.net/v3/binary/latest/25/ga/windows/x64/jre/hotspot/normal/eclipse?project=jdk",
    },
];

fn default_java_runtime() -> JavaRuntimeSpec {
    managed_java_runtime_for_major(21)
}

fn managed_java_runtime_for_major(major: u32) -> JavaRuntimeSpec {
    MANAGED_JAVA_RUNTIMES
        .iter()
        .copied()
        .filter(|runtime| runtime.major >= major)
        .min_by_key(|runtime| runtime.major)
        .or_else(|| MANAGED_JAVA_RUNTIMES.iter().copied().max_by_key(|runtime| runtime.major))
        .expect("managed Java runtime list must not be empty")
}

impl JavaRuntimeSpec {
    fn archive_name(self) -> String {
        format!("temurin-{}-runtime.zip", self.major)
    }

    fn install_dir(self) -> PathBuf {
        settings::java_runtime_dir_for_major(self.major)
    }
}

fn suppress_console_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x08000000);
    }
}

pub(super) fn find_game_java_executable_for_version(
    version_id: &str,
    game_version: Option<&str>,
) -> Result<PathBuf, String> {
    let required_major = required_java_major_for_versions(version_id, game_version);
    let settings = settings::load_settings();
    let selected_major = settings::java_runtime_major_for_mode(&settings.java_runtime_mode);
    if let Some(major) = selected_major {
        if major != required_major {
            crate::app_log::append_log(
                "WARN",
                format!(
                    "Java runtime mode forced Java {} for Minecraft {} / {:?}; auto recommendation was Java {}",
                    major, version_id, game_version, required_major
                ),
            );
        }
    }
    let required_major = selected_major.unwrap_or(required_major);
    let runtime = managed_java_runtime_for_major(required_major);

    if let Some(java) = custom_java_executable()? {
        match java_major_version(&java) {
            Some(major) if major == runtime.major => {}
            Some(major) => {
                crate::app_log::append_log(
                    "WARN",
                    format!(
                        "custom Java {} does not match required Java {} for Minecraft {} / {:?}; using managed Java {}",
                        major,
                        runtime.major,
                        version_id,
                        game_version,
                        runtime.major
                    ),
                );
                return managed_game_java_executable(runtime);
            }
            None => {
                crate::app_log::append_log(
                    "WARN",
                    format!(
                        "custom Java version could not be detected for Minecraft {} / {:?}; using managed Java {}",
                        version_id,
                        game_version,
                        runtime.major
                    ),
                );
                return managed_game_java_executable(runtime);
            }
        }
        if cfg!(target_os = "windows") {
            let javaw = java.with_file_name("javaw.exe");
            if javaw.exists() {
                return Ok(javaw);
            }
        }
        return Ok(java);
    }

    if cfg!(target_os = "windows") {
        return managed_game_java_executable(runtime);
    }

    let java = find_java_executable()?;
    if cfg!(target_os = "windows") {
        let javaw = java.with_file_name("javaw.exe");
        if javaw.exists() {
            return Ok(javaw);
        }
    }

    Ok(java)
}

fn managed_game_java_executable(runtime: JavaRuntimeSpec) -> Result<PathBuf, String> {
    let java = ensure_managed_java_runtime_for(runtime, None)?;
    if cfg!(target_os = "windows") {
        let javaw = java.with_file_name("javaw.exe");
        if javaw.exists() {
            return Ok(javaw);
        }
    }
    Ok(java)
}

pub(super) fn find_java_executable() -> Result<PathBuf, String> {
    if let Some(java) = custom_java_executable()? {
        return Ok(java);
    }

    if cfg!(target_os = "windows") {
        return ensure_managed_java_runtime(None);
    }

    if let Some(java) = discover_java_executable() {
        return Ok(java);
    }

    install_java_runtime(default_java_runtime(), None)
}

fn java_major_version(java: &Path) -> Option<u32> {
    let mut command = Command::new(java);
    command.arg("-version");
    suppress_console_window(&mut command);
    let output = command.output().ok()?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    for token in text.split(|character: char| !character.is_ascii_alphanumeric() && character != '.') {
        if token.is_empty() {
            continue;
        }
        if let Some(rest) = token.strip_prefix("1.") {
            if let Some(value) = rest.split('.').next().and_then(|part| part.parse::<u32>().ok()) {
                return Some(value);
            }
        }
        if let Some(value) = token.split('.').next().and_then(|part| part.parse::<u32>().ok()) {
            if value > 0 {
                return Some(value);
            }
        }
    }

    None
}

fn required_java_major_for_versions(version_id: &str, game_version: Option<&str>) -> u32 {
    game_version
        .and_then(required_java_major_for_minecraft_version)
        .or_else(|| required_java_major_for_minecraft_version(version_id))
        .unwrap_or(default_java_runtime().major)
}

fn required_java_major_for_minecraft_version(value: &str) -> Option<u32> {
    extract_minecraft_version_candidate(value).map(required_java_major_for_minecraft_candidate)
}

#[derive(Debug, Clone, Copy)]
struct VersionNumberCandidate {
    major: u32,
    minor: Option<u32>,
    patch: Option<u32>,
}

fn extract_minecraft_version_candidate(value: &str) -> Option<VersionNumberCandidate> {
    for token in value.split(|character: char| !character.is_ascii_digit() && character != '.') {
        let trimmed = token.trim_matches('.');
        if trimmed.is_empty() || !trimmed.chars().any(|character| character.is_ascii_digit()) {
            continue;
        }
        let parts = trimmed
            .split('.')
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<u32>().ok())
            .collect::<Vec<_>>();
        let Some(major) = parts.first().copied() else {
            continue;
        };
        let candidate = VersionNumberCandidate {
            major,
            minor: parts.get(1).copied(),
            patch: parts.get(2).copied(),
        };

        if looks_like_minecraft_version(candidate) {
            return Some(candidate);
        }
    }

    None
}

fn looks_like_minecraft_version(version: VersionNumberCandidate) -> bool {
    if version.major == 1 {
        return version.minor.is_some_and(|minor| minor <= 99);
    }

    (2..=25).contains(&version.major)
}

fn required_java_major_for_minecraft_candidate(version: VersionNumberCandidate) -> u32 {
    if version.major >= 26 {
        return 25;
    }

    if version.major >= 2 {
        return 21;
    }

    let minor = version.minor.unwrap_or_default();
    let patch = version.patch.unwrap_or_default();

    if minor >= 26 {
        25
    } else if minor >= 21 || (minor == 20 && patch >= 5) {
        21
    } else if minor >= 18 {
        17
    } else {
        default_java_runtime().major
    }
}

fn custom_java_executable() -> Result<Option<PathBuf>, String> {
    let settings = settings::load_settings();
    let Some(path) = settings.custom_java_path.as_deref() else {
        return Ok(None);
    };
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    settings::validate_custom_java_path(trimmed).map(Some)
}

pub(super) fn ensure_java_runtime_available_with_progress(
    app: &AppHandle,
    operation_id: Option<String>,
) -> Result<crate::models::ActionResult, String> {
    let operation_id = operation_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("java-runtime-{}", chrono::Local::now().timestamp_millis()));
    let java_path = ensure_managed_java_runtime(Some((app, operation_id.as_str())))?;
    emit_progress(
        app,
        &operation_id,
        "Java ランタイムを準備中",
        "Java ランタイムの確認が完了しました。",
        100.0,
    );

    Ok(crate::models::ActionResult {
        message: "Java ランタイムを確認しました。".to_string(),
        file_name: java_path.to_string_lossy().to_string(),
    })
}

pub(super) fn ensure_managed_java_runtime(
    progress: Option<(&AppHandle, &str)>,
) -> Result<PathBuf, String> {
    let settings = settings::load_settings();
    let runtime = settings::java_runtime_major_for_mode(&settings.java_runtime_mode)
        .map(managed_java_runtime_for_major)
        .unwrap_or_else(default_java_runtime);
    ensure_managed_java_runtime_for(runtime, progress)
}

fn ensure_managed_java_runtime_for(
    runtime: JavaRuntimeSpec,
    progress: Option<(&AppHandle, &str)>,
) -> Result<PathBuf, String> {
    if !cfg!(target_os = "windows") {
        return find_java_executable();
    }

    let install_dir = java_runtime_install_dir(runtime);
    if let Some(java) = discover_java_in_directory(&install_dir) {
        if java_major_version(&java).is_some_and(|major| major >= runtime.major) {
            return Ok(java);
        }
        crate::app_log::append_log(
            "WARN",
            format!(
                "managed Java runtime in {} is older than required {}; reinstalling",
                install_dir.display(),
                runtime.major
            ),
        );
        let _ = fs::remove_dir_all(&install_dir);
    }

    install_java_runtime(runtime, progress)
}

fn discover_java_executable() -> Option<PathBuf> {
    if let Some(java_home) = std::env::var_os("JAVA_HOME") {
        let candidate = PathBuf::from(java_home)
            .join("bin")
            .join(if cfg!(target_os = "windows") {
                "javaw.exe"
            } else {
                "java"
            });
        if candidate.exists() {
            return Some(candidate);
        }
    }

    if cfg!(target_os = "windows") {
        let mut command = Command::new("where");
        command.arg("java");
        suppress_console_window(&mut command);
        let output = command.output().ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(path) = stdout.lines().find(|line| !line.trim().is_empty()) {
                return Some(PathBuf::from(path.trim()));
            }
        }
    } else {
        let output = Command::new("sh")
            .arg("-lc")
            .arg("command -v java")
            .output()
            .ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(path) = stdout.lines().find(|line| !line.trim().is_empty()) {
                return Some(PathBuf::from(path.trim()));
            }
        }
    }

    let fallback = if cfg!(target_os = "windows") {
        PathBuf::from("javaw.exe")
    } else {
        PathBuf::from("java")
    };
    let mut probe_command = Command::new(&fallback);
    probe_command.arg("-version");
    suppress_console_window(&mut probe_command);
    let probe = probe_command.output().ok()?;

    if probe.status.success() || !probe.stderr.is_empty() {
        Some(fallback)
    } else {
        None
    }
}

fn install_java_runtime(
    runtime: JavaRuntimeSpec,
    progress: Option<(&AppHandle, &str)>,
) -> Result<PathBuf, String> {
    if !cfg!(target_os = "windows") {
        return Err("Java が見つかりませんでした。Java をインストールしてください。".to_string());
    }

    if let Some((app, operation_id)) = progress {
        emit_progress(
            app,
            operation_id,
            "Java ランタイムを準備中",
            format!("Java {} ランタイムのダウンロードを開始しています。", runtime.major),
            12.0,
        );
    }

    let install_dir = java_runtime_install_dir(runtime);
    if let Some(java) = discover_java_in_directory(&install_dir) {
        if java_major_version(&java).is_some_and(|major| major >= runtime.major) {
            if let Some((app, operation_id)) = progress {
                emit_progress(
                    app,
                    operation_id,
                    "Java ランタイムを準備中",
                    "既存の Java ランタイムを使用します。",
                    100.0,
                );
            }
            return Ok(java);
        }
    }

    if install_dir.exists() {
        fs::remove_dir_all(&install_dir).map_err(|error| {
            format!("{} を更新できませんでした: {error}", install_dir.display())
        })?;
    }

    if let Some(parent) = install_dir.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }

    let archive_path = install_dir
        .parent()
        .unwrap_or(install_dir.as_path())
        .join(runtime.archive_name());
    download_java_runtime_archive(runtime, &archive_path)?;

    if let Some((app, operation_id)) = progress {
        emit_progress(
            app,
            operation_id,
            "Java ランタイムを準備中",
            "Java ランタイムを展開しています。",
            72.0,
        );
    }

    extract_java_runtime_archive(&archive_path, &install_dir, progress)?;

    let _ = fs::remove_file(&archive_path);

    discover_java_in_directory(&install_dir).ok_or_else(|| {
        format!(
            "Java の自動インストールは完了しましたが、{} で実行ファイルを見つけられませんでした。",
            install_dir.display()
        )
    })
}

fn download_java_runtime_archive(runtime: JavaRuntimeSpec, target_path: &Path) -> Result<(), String> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }

    let escaped_output = target_path.to_string_lossy().replace('"', "``\"");
    let script = format!(
        "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -Uri '{url}' -OutFile \"{out}\" -UseBasicParsing",
        url = runtime.download_url,
        out = escaped_output
    );
    let mut command = Command::new("powershell");
    command
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(script);
    suppress_console_window(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("Java をダウンロードできませんでした: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if !stderr.trim().is_empty() {
        stderr.trim().to_string()
    } else if !stdout.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        "原因不明のエラー".to_string()
    };
    Err(format!("Java のダウンロードに失敗しました: {detail}"))
}

fn extract_java_runtime_archive(
    archive_path: &Path,
    install_dir: &Path,
    progress: Option<(&AppHandle, &str)>,
) -> Result<(), String> {
    let archive_file = File::open(archive_path)
        .map_err(|error| format!("{} を開けませんでした: {error}", archive_path.display()))?;
    let mut archive = ZipArchive::new(archive_file)
        .map_err(|error| format!("Java アーカイブを開けませんでした: {error}"))?;
    let total_entries = archive.len().max(1);

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Java アーカイブ内のファイルを読み取れませんでした: {error}"))?;
        let Some(enclosed_name) = entry.enclosed_name().map(|path| path.to_owned()) else {
            continue;
        };
        let output_path = install_dir.join(enclosed_name);

        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(|error| {
                format!("{} を作成できませんでした: {error}", output_path.display())
            })?;
        } else {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!("{} を作成できませんでした: {error}", parent.display())
                })?;
            }

            let mut output_file = File::create(&output_path).map_err(|error| {
                format!("{} を作成できませんでした: {error}", output_path.display())
            })?;
            io::copy(&mut entry, &mut output_file).map_err(|error| {
                format!("{} を展開できませんでした: {error}", output_path.display())
            })?;
        }

        if let Some((app, operation_id)) = progress {
            let extracted = index + 1;
            let percent = 72.0 + ((extracted as f64 / total_entries as f64) * 24.0);
            emit_progress(
                app,
                operation_id,
                "Java ランタイムを準備中",
                format!(
                    "Java ランタイムを展開しています。{} / {} ファイルを処理しました。",
                    extracted, total_entries
                ),
                percent.min(96.0),
            );
        }
    }

    Ok(())
}

fn java_runtime_install_dir(runtime: JavaRuntimeSpec) -> PathBuf {
    runtime.install_dir()
}

fn discover_java_in_directory(dir: &Path) -> Option<PathBuf> {
    if !dir.exists() {
        return None;
    }

    let preferred = if cfg!(target_os = "windows") {
        "java.exe"
    } else {
        "java"
    };
    let fallback = if cfg!(target_os = "windows") {
        "javaw.exe"
    } else {
        "java"
    };

    if let Some(preferred_path) = find_executable_recursively(dir, preferred) {
        if preferred_path.exists() {
            return Some(preferred_path);
        }
    }

    if let Some(fallback_path) = find_executable_recursively(dir, fallback) {
        if fallback_path.exists() {
            return Some(fallback_path);
        }
    }

    None
}

fn find_executable_recursively(dir: &Path, file_name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;

    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_executable_recursively(&path, file_name) {
                return Some(found);
            }
            continue;
        }

        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(file_name))
        {
            return Some(path);
        }
    }

    None
}
