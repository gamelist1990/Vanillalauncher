use chrono::Utc;
use std::{env, fs, io::Write, path::PathBuf};

const APP_TEMP_DIR_NAME: &str = "VanillaLauncher";

pub fn log_file_path() -> PathBuf {
    env::temp_dir()
        .join(APP_TEMP_DIR_NAME)
        .join("logs")
        .join("vanillalauncher.log")
}

pub fn append_log(level: &str, message: impl AsRef<str>) {
    let path = log_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let line = format!(
        "{} [{}] {}\n",
        Utc::now().to_rfc3339(),
        level,
        message.as_ref()
    );
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = file.write_all(line.as_bytes());
    }
}

pub fn clear_log() {
    let path = log_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, "");
}
