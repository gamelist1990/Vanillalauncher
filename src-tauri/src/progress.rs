use crate::models::ProgressUpdate;
use tauri::{AppHandle, Emitter};

pub const LAUNCHER_PROGRESS_EVENT: &str = "launcher-progress";

pub fn emit_progress(
    app: &AppHandle,
    operation_id: &str,
    title: &str,
    detail: impl Into<String>,
    percent: f64,
) {
    let clamped_percent = percent.clamp(0.0, 100.0);
    let payload = ProgressUpdate {
        operation_id: operation_id.to_string(),
        title: title.to_string(),
        detail: detail.into(),
        percent: clamped_percent,
    };
    let _ = app.emit(LAUNCHER_PROGRESS_EVENT, payload);
}
