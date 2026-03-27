use crate::{progress::emit_progress, settings};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::Command,
};
use tauri::AppHandle;
use zip::ZipArchive;

const JAVA_RUNTIME_DOWNLOAD_URL: &str =
    "https://api.adoptium.net/v3/binary/latest/21/ga/windows/x64/jre/hotspot/normal/eclipse?project=jdk";

fn suppress_console_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x08000000);
    }
}

pub(super) fn find_game_java_executable() -> Result<PathBuf, String> {
    if cfg!(target_os = "windows") {
        let java = ensure_managed_java_runtime(None)?;
        let javaw = java.with_file_name("javaw.exe");
        if javaw.exists() {
            return Ok(javaw);
        }
        return Ok(java);
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

pub(super) fn find_java_executable() -> Result<PathBuf, String> {
    if cfg!(target_os = "windows") {
        return ensure_managed_java_runtime(None);
    }

    if let Some(java) = discover_java_executable() {
        return Ok(java);
    }

    install_java_runtime(None)
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
    if !cfg!(target_os = "windows") {
        return find_java_executable();
    }

    let install_dir = java_runtime_install_dir();
    if let Some(java) = discover_java_in_directory(&install_dir) {
        return Ok(java);
    }

    install_java_runtime(progress)
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

fn install_java_runtime(progress: Option<(&AppHandle, &str)>) -> Result<PathBuf, String> {
    if !cfg!(target_os = "windows") {
        return Err("Java が見つかりませんでした。Java をインストールしてください。".to_string());
    }

    if let Some((app, operation_id)) = progress {
        emit_progress(
            app,
            operation_id,
            "Java ランタイムを準備中",
            "Java ランタイムのダウンロードを開始しています。",
            12.0,
        );
    }

    let install_dir = java_runtime_install_dir();
    if let Some(java) = discover_java_in_directory(&install_dir) {
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
        .join("temurin-21-runtime.zip");
    download_java_runtime_archive(&archive_path)?;

    if let Some((app, operation_id)) = progress {
        emit_progress(
            app,
            operation_id,
            "Java ランタイムを準備中",
            "Java ランタイムを展開しています。",
            72.0,
        );
    }

    let archive_file = File::open(&archive_path)
        .map_err(|error| format!("{} を開けませんでした: {error}", archive_path.display()))?;
    let mut archive = ZipArchive::new(archive_file)
        .map_err(|error| format!("Java アーカイブを開けませんでした: {error}"))?;
    archive
        .extract(&install_dir)
        .map_err(|error| format!("Java を展開できませんでした: {error}"))?;

    let _ = fs::remove_file(&archive_path);

    discover_java_in_directory(&install_dir).ok_or_else(|| {
        format!(
            "Java の自動インストールは完了しましたが、{} で実行ファイルを見つけられませんでした。",
            install_dir.display()
        )
    })
}

fn download_java_runtime_archive(target_path: &Path) -> Result<(), String> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }

    let escaped_output = target_path.to_string_lossy().replace('"', "``\"");
    let script = format!(
        "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -Uri '{url}' -OutFile \"{out}\" -UseBasicParsing",
        url = JAVA_RUNTIME_DOWNLOAD_URL,
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

fn java_runtime_install_dir() -> PathBuf {
    settings::java_runtime_dir()
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

    let preferred_path = find_executable_recursively(dir, preferred)?;
    if preferred_path.exists() {
        return Some(preferred_path);
    }

    let fallback_path = find_executable_recursively(dir, fallback)?;
    if fallback_path.exists() {
        return Some(fallback_path);
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
