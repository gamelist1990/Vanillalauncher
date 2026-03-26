use crate::{app_log, models::XboxRpsStateResult, progress::emit_progress};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::AppHandle;

use super::MOJANG_USER_AGENT;

#[derive(Debug, Clone)]
pub(super) struct CachedXboxToken {
    pub(super) token: String,
    pub(super) expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) scope: Option<String>,
    pub(super) source_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct XboxRpsLastSuccessState {
    source_path: String,
    variant_label: String,
    relying_party: String,
    ticket_prefix: String,
    expires_at: String,
    saved_at: String,
}

#[derive(Debug, Deserialize)]
struct MinecraftServicesProfile {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct LauncherLoginResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct XboxUserAuthenticateResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: Option<XboxDisplayClaims>,
}

#[derive(Debug, Deserialize)]
struct XboxDisplayClaims {
    xui: Vec<XboxUserClaim>,
}

#[derive(Debug, Deserialize)]
struct XboxUserClaim {
    uhs: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XboxXstsAuthorizeResponse {
    #[serde(rename = "Token")]
    token: String,
}

pub(super) fn read_cached_xbox_identity_tokens() -> Result<Vec<CachedXboxToken>, String> {
    if !cfg!(target_os = "windows") {
        return Ok(Vec::new());
    }

    let local_app_data = env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "LOCALAPPDATA が設定されていません。".to_string())?;

    let mut candidates = Vec::new();
    let mut seen_tokens = HashSet::new();
    for cache_dir in xbox_token_cache_dirs(&local_app_data) {
        if !cache_dir.exists() {
            continue;
        }

        app_log::append_log(
            "INFO",
            format!("scanning xbox token cache dir {}", cache_dir.display()),
        );
        for entry in fs::read_dir(&cache_dir)
            .map_err(|error| format!("{} を読み込めませんでした: {error}", cache_dir.display()))?
        {
            let entry = entry
                .map_err(|error| format!("TokenBroker キャッシュの確認に失敗しました: {error}"))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("tbres") {
                continue;
            }
            if let Some(token) = parse_cached_xbox_token_file(&path)? {
                if !seen_tokens.insert(token.token.clone()) {
                    continue;
                }
                candidates.push(token);
            }
        }
    }

    candidates.sort_by(cached_xbox_token_sort);
    Ok(candidates)
}

pub(super) fn build_prioritized_rps_attempts(
    cached_tokens: &[CachedXboxToken],
    max_attempts: usize,
) -> (Vec<(String, String, CachedXboxToken)>, bool) {
    let mut attempts: Vec<(i32, String, String, CachedXboxToken)> = Vec::new();
    let mut seen = HashSet::new();
    let mut used_saved_state = false;

    if let Some(state) = load_xbox_rps_state() {
        if !is_saved_state_expired(&state) {
            for token in cached_tokens {
                if token.source_path.to_string_lossy() != state.source_path {
                    continue;
                }
                for (label, candidate) in build_xbox_token_variants(&token.token) {
                    if label != state.variant_label || !candidate.contains("t=") {
                        continue;
                    }
                    if seen.insert(candidate.clone()) {
                        attempts.push((10_000, label, candidate, token.clone()));
                        used_saved_state = true;
                    }
                }
            }
        }
    }

    for token in cached_tokens {
        for (label, candidate) in build_xbox_token_variants(&token.token) {
            if !candidate.contains("t=") {
                continue;
            }
            if !seen.insert(candidate.clone()) {
                continue;
            }
            let score = rank_rps_variant(token, &label, &candidate);
            attempts.push((score, label, candidate, token.clone()));
        }
    }

    attempts.sort_by(|left, right| right.0.cmp(&left.0));
    let bounded = attempts
        .into_iter()
        .take(max_attempts)
        .map(|(_score, label, candidate, token)| (label, candidate, token))
        .collect();

    (bounded, used_saved_state)
}

pub(super) fn persist_xbox_rps_success_state(
    token: &CachedXboxToken,
    label: &str,
    candidate: &str,
) {
    let expires_at = token
        .expires_at
        .unwrap_or_else(chrono::Utc::now)
        .to_rfc3339();
    let state = XboxRpsLastSuccessState {
        source_path: token.source_path.display().to_string(),
        variant_label: label.to_string(),
        relying_party: "rp://api.minecraftservices.com/".to_string(),
        ticket_prefix: candidate.chars().take(24).collect(),
        expires_at,
        saved_at: chrono::Utc::now().to_rfc3339(),
    };

    if let Err(error) = save_xbox_rps_state(&state) {
        app_log::append_log(
            "WARN",
            format!("failed to save xbox-rps state after successful auth: {error}"),
        );
    }
}

pub(super) fn preview_token(token: &str) -> String {
    let prefix: String = token.chars().take(12).collect();
    format!("{prefix}...len={}", token.len())
}

pub(super) async fn exchange_rps_ticket_for_minecraft_access_token(
    ticket: &str,
    context: &str,
) -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent(MOJANG_USER_AGENT)
        .build()
        .ok()?;

    let user_auth_response = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .header("x-xbl-contract-version", "1")
        .json(&serde_json::json!({
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": ticket,
            }
        }))
        .send()
        .await
        .ok()?;
    if !user_auth_response.status().is_success() {
        let status = user_auth_response.status().as_u16();
        let body = user_auth_response
            .text()
            .await
            .unwrap_or_default()
            .replace('\n', " ")
            .replace('\r', " ");
        app_log::append_log(
            "WARN",
            format!(
                "user/authenticate returned status {} context={} body={}",
                status,
                context,
                truncate_log_text(&body, 160)
            ),
        );
        return None;
    }
    let user_auth = user_auth_response
        .json::<XboxUserAuthenticateResponse>()
        .await
        .ok()?;
    let uhs = user_auth
        .display_claims
        .as_ref()
        .and_then(|claims| claims.xui.first())
        .and_then(|claim| claim.uhs.clone())?;

    let xsts_response = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .header("x-xbl-contract-version", "1")
        .json(&serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [user_auth.token],
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }))
        .send()
        .await
        .ok()?;
    if !xsts_response.status().is_success() {
        let status = xsts_response.status().as_u16();
        let body = xsts_response
            .text()
            .await
            .unwrap_or_default()
            .replace('\n', " ")
            .replace('\r', " ");
        app_log::append_log(
            "WARN",
            format!(
                "xsts/authorize returned status {} context={} body={}",
                status,
                context,
                truncate_log_text(&body, 160)
            ),
        );
        return None;
    }
    let xsts = xsts_response
        .json::<XboxXstsAuthorizeResponse>()
        .await
        .ok()?;
    let identity_token = format!("XBL3.0 x={uhs};{}", xsts.token);
    exchange_xbox_token_for_minecraft_access_token(&identity_token, &format!("{context} -> xbl3"))
        .await
}

pub(super) async fn ensure_xbox_rps_state(
    app: Option<&AppHandle>,
    operation_id: Option<&str>,
) -> Result<XboxRpsStateResult, String> {
    let state_path = xbox_rps_state_path();
    let mut used_saved_state = false;

    let emit_auth_progress = |title: &str, detail: String, percent: f64| {
        if let (Some(app), Some(operation_id)) = (app, operation_id) {
            emit_progress(app, operation_id, title, detail, percent);
        }
    };

    let cached_tokens = read_cached_xbox_identity_tokens()?;
    if cached_tokens.is_empty() {
        emit_auth_progress(
            "Xbox 認証確認",
            "試行対象が見つからなかったため、認証確認を完了しました (0/0)".to_string(),
            100.0,
        );
        return Ok(XboxRpsStateResult {
            message: "利用可能な TokenBroker 候補が見つかりませんでした。".to_string(),
            state_path: state_path.display().to_string(),
            used_saved_state,
            refreshed: false,
            succeeded: false,
            attempts_tried: 0,
            total_attempts: 0,
            source_path: None,
            variant_label: None,
        });
    }

    let (bounded, used_saved) = build_prioritized_rps_attempts(&cached_tokens, 12);
    used_saved_state = used_saved;
    let total_attempts = bounded.len();

    if total_attempts == 0 {
        emit_auth_progress(
            "Xbox 認証確認",
            "候補が 0 件のため、既存情報で続行します (0/0)".to_string(),
            100.0,
        );
        return Ok(XboxRpsStateResult {
            message: "有効な Xbox RPS 候補を構築できませんでした。".to_string(),
            state_path: state_path.display().to_string(),
            used_saved_state,
            refreshed: false,
            succeeded: false,
            attempts_tried: 0,
            total_attempts: 0,
            source_path: None,
            variant_label: None,
        });
    }

    emit_auth_progress(
        "Xbox 認証確認",
        format!("試行を開始します (0/{total_attempts})"),
        0.0,
    );

    for (index, (label, candidate, token)) in bounded.into_iter().enumerate() {
        let attempts_tried = index + 1;
        let context = format!(
            "ensure_xbox_rps_state:{}:{}",
            token.source_path.display(),
            label
        );
        app_log::append_log(
            "INFO",
            format!(
                "probing xbox-rps state attempt={} source={} variant={} preview={}",
                index + 1,
                token.source_path.display(),
                label,
                preview_token(&candidate)
            ),
        );

        if exchange_rps_ticket_for_minecraft_access_token(&candidate, &context)
            .await
            .is_some()
        {
            let percent = (attempts_tried as f64 / total_attempts as f64) * 100.0;
            emit_auth_progress(
                "Xbox 認証確認",
                format!("試行 {attempts_tried}/{total_attempts} が成功しました"),
                percent,
            );

            persist_xbox_rps_success_state(&token, &label, &candidate);

            return Ok(XboxRpsStateResult {
                message: "Xbox RPS state を検証し、利用可能な候補を保存しました。".to_string(),
                state_path: state_path.display().to_string(),
                used_saved_state,
                refreshed: true,
                succeeded: true,
                attempts_tried,
                total_attempts,
                source_path: Some(token.source_path.display().to_string()),
                variant_label: Some(label),
            });
        }

        let percent = (attempts_tried as f64 / total_attempts as f64) * 100.0;
        emit_auth_progress(
            "Xbox 認証確認",
            format!("試行 {attempts_tried}/{total_attempts} を完了。次の候補を確認します"),
            percent,
        );

        std::thread::sleep(Duration::from_millis(700));
    }

    emit_auth_progress(
        "Xbox 認証確認",
        format!(
            "全試行 {total_attempts}/{total_attempts} を完了しましたが有効候補は見つかりませんでした"
        ),
        100.0,
    );

    Ok(XboxRpsStateResult {
        message: "有効な Xbox RPS state を更新できませんでした。".to_string(),
        state_path: state_path.display().to_string(),
        used_saved_state,
        refreshed: false,
        succeeded: false,
        attempts_tried: total_attempts,
        total_attempts,
        source_path: None,
        variant_label: None,
    })
}

pub(super) fn is_access_token_expired(token: &str, expires_at: Option<&str>) -> bool {
    if let Some(expiry_text) = expires_at {
        if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(expiry_text) {
            return chrono::Utc::now() >= parsed.with_timezone(&chrono::Utc);
        }
    }

    let Some(payload) = token.split('.').nth(1) else {
        return false;
    };
    let Some(decoded) = decode_base64_url(payload) else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<Value>(&decoded) else {
        return false;
    };
    let Some(expiry) = value.get("exp").and_then(Value::as_i64) else {
        return false;
    };

    chrono::Utc::now().timestamp() >= expiry
}

pub(super) async fn fetch_minecraft_profile_for_token(token: &str) -> Option<(String, String)> {
    let client = match reqwest::Client::builder()
        .user_agent(MOJANG_USER_AGENT)
        .build()
    {
        Ok(client) => client,
        Err(_) => return None,
    };

    let response = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await;

    let Ok(result) = response else {
        return None;
    };
    if !result.status().is_success() {
        return None;
    }

    result
        .json::<MinecraftServicesProfile>()
        .await
        .ok()
        .map(|profile| (profile.id, profile.name))
}

pub(super) fn uuid_from_access_token(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = decode_base64_url(payload)?;
    let value: Value = serde_json::from_slice(&decoded).ok()?;
    value
        .get("profiles")
        .and_then(|profiles| profiles.get("mc"))
        .and_then(Value::as_str)
        .map(normalize_uuid_value)
}

pub(super) fn normalize_uuid_value(value: &str) -> String {
    value
        .chars()
        .filter(|character| *character != '-')
        .collect()
}

fn xbox_rps_state_path() -> PathBuf {
    env::temp_dir()
        .join("VanillaLauncher")
        .join("xbox-rps-last-success.json")
}

fn load_xbox_rps_state() -> Option<XboxRpsLastSuccessState> {
    let path = xbox_rps_state_path();
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str::<XboxRpsLastSuccessState>(&text).ok()
}

fn save_xbox_rps_state(state: &XboxRpsLastSuccessState) -> Result<(), String> {
    let path = xbox_rps_state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を作成できませんでした: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(state)
        .map_err(|error| format!("state を JSON 化できませんでした: {error}"))?;
    fs::write(&path, text)
        .map_err(|error| format!("{} を保存できませんでした: {error}", path.display()))
}

fn is_saved_state_expired(state: &XboxRpsLastSuccessState) -> bool {
    let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(&state.expires_at) else {
        return true;
    };
    expires_at.with_timezone(&chrono::Utc) <= chrono::Utc::now()
}

fn rank_rps_variant(token: &CachedXboxToken, label: &str, candidate: &str) -> i32 {
    let mut score = cached_xbox_token_score(token);
    let label_score = match label {
        "from-t-marker" => 220,
        "from-t-marker-no-prefix" => 180,
        "through-ampersand" => 130,
        "through-ampersand-no-prefix" => 90,
        "raw" => -120,
        _ => 0,
    };
    score += label_score;

    if candidate.contains("EwD4A+pv") || candidate.contains("EwDoA+pv") {
        score += 260;
    }
    if candidate.contains("GwAmAru") {
        score -= 260;
    }

    score
}

fn cached_xbox_token_sort(left: &CachedXboxToken, right: &CachedXboxToken) -> std::cmp::Ordering {
    right
        .expires_at
        .cmp(&left.expires_at)
        .then_with(|| cached_xbox_token_score(right).cmp(&cached_xbox_token_score(left)))
        .then_with(|| left.source_path.cmp(&right.source_path))
}

fn cached_xbox_token_score(token: &CachedXboxToken) -> i32 {
    let scope = token
        .scope
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let path = token.source_path.to_string_lossy().to_ascii_lowercase();
    let mut score = 0;
    if path.contains("xboxidentityprovider") {
        score += 400;
    }
    if path.contains("gamingapp") {
        score += 250;
    }
    if scope.contains("xbox") || scope.contains("sisu") {
        score += 500;
    }
    if scope.contains("ssl.live.com") {
        score += 120;
    }
    if scope.contains("passport.net") {
        score += 60;
    }
    score
}

fn xbox_token_cache_dirs(local_app_data: &std::ffi::OsStr) -> Vec<PathBuf> {
    let base = PathBuf::from(local_app_data);
    vec![
        base.join("Packages")
            .join("Microsoft.XboxIdentityProvider_8wekyb3d8bbwe")
            .join("AC")
            .join("TokenBroker")
            .join("Cache"),
        base.join("Packages")
            .join("Microsoft.GamingApp_8wekyb3d8bbwe")
            .join("AC")
            .join("TokenBroker")
            .join("Cache"),
        base.join("Microsoft").join("TokenBroker").join("Cache"),
    ]
}

fn parse_cached_xbox_token_file(path: &Path) -> Result<Option<CachedXboxToken>, String> {
    let raw = fs::read(path)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", path.display()))?;
    let utf16: Vec<u16> = raw
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    let mut text = String::from_utf16_lossy(&utf16);
    while text.ends_with('\u{0}') {
        text.pop();
    }

    let value: Value = serde_json::from_str(&text)
        .map_err(|error| format!("{} を解析できませんでした: {error}", path.display()))?;
    let system_defined = value
        .get("TBDataStoreObject")
        .and_then(|value| value.get("ObjectData"))
        .and_then(|value| value.get("SystemDefinedProperties"))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "{} の TokenBroker 情報を解釈できませんでした。",
                path.display()
            )
        })?;

    let response_blob = system_defined
        .get("ResponseBytes")
        .and_then(|value| value.get("Value"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let Some(response_blob) = response_blob else {
        return Ok(None);
    };

    let decrypted =
        decrypt_windows_dpapi_blob(&decode_base64(response_blob).ok_or_else(|| {
            format!(
                "{} の ResponseBytes を復号前に解釈できませんでした。",
                path.display()
            )
        })?)?;
    let payload = String::from_utf8_lossy(&decrypted);
    let scope = extract_cached_token_scope(&payload);
    let Some(token) = extract_cached_xbox_token(&payload) else {
        app_log::append_log(
            "INFO",
            format!("tbres {} did not contain Xbox token marker", path.display()),
        );
        return Ok(None);
    };
    if token.is_empty() {
        return Ok(None);
    }

    let expires_at = system_defined
        .get("Expiration")
        .and_then(|value| value.get("Value"))
        .and_then(Value::as_str)
        .and_then(parse_tokenbroker_filetime);
    if expires_at
        .as_ref()
        .is_some_and(|value| *value <= chrono::Utc::now())
    {
        if let Some(expires_at) = expires_at.as_ref() {
            app_log::append_log(
                "INFO",
                format!(
                    "tbres {} had expired token at {}",
                    path.display(),
                    expires_at.to_rfc3339()
                ),
            );
        }
        return Ok(None);
    }

    app_log::append_log(
        "INFO",
        format!(
            "tbres {} yielded Xbox token exp={} scope={} preview={}",
            path.display(),
            expires_at
                .as_ref()
                .map(|value| value.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_string()),
            scope.as_deref().unwrap_or("unknown"),
            preview_token(&token)
        ),
    );
    Ok(Some(CachedXboxToken {
        token,
        expires_at,
        scope,
        source_path: path.to_path_buf(),
    }))
}

fn extract_cached_token_scope(payload: &str) -> Option<String> {
    extract_marker_value(payload, "scope=").or_else(|| extract_marker_value(payload, "WA_Scope"))
}

fn extract_cached_xbox_token(payload: &str) -> Option<String> {
    let raw = extract_marker_value(payload, "WTRes_Token")?;
    build_xbox_token_variants(&raw)
        .into_iter()
        .find(|(_, token)| token.contains("t="))
        .map(|(_, token)| token)
        .or(Some(raw))
}

fn extract_marker_value(payload: &str, marker: &str) -> Option<String> {
    let start = payload.find(marker)?;
    let tail = &payload[start + marker.len()..];
    let mut seen_content = false;
    let mut value = String::new();

    for character in tail.chars() {
        if !seen_content {
            if character == '\0' || character.is_control() || character.is_whitespace() {
                continue;
            }
            seen_content = true;
        }

        if character == '\0' || character.is_control() || character.is_whitespace() {
            break;
        }

        value.push(character);
    }

    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn build_xbox_token_variants(raw_token: &str) -> Vec<(String, String)> {
    let mut variants = Vec::new();
    let mut seen = HashSet::new();
    let cleaned = raw_token.trim().trim_matches('\0').to_string();

    push_token_variant(&mut variants, &mut seen, "raw", cleaned.clone());

    if let Some(index) = cleaned.find("t=") {
        let from_t = cleaned[index..].to_string();
        push_token_variant(&mut variants, &mut seen, "from-t-marker", from_t.clone());

        if let Some(stripped) = from_t.strip_prefix("t=") {
            push_token_variant(
                &mut variants,
                &mut seen,
                "from-t-marker-no-prefix",
                stripped.to_string(),
            );
        }

        if let Some(ampersand) = from_t.find('&') {
            let through_ampersand = from_t[..ampersand].to_string();
            push_token_variant(
                &mut variants,
                &mut seen,
                "through-ampersand",
                through_ampersand.clone(),
            );
            if let Some(stripped) = through_ampersand.strip_prefix("t=") {
                push_token_variant(
                    &mut variants,
                    &mut seen,
                    "through-ampersand-no-prefix",
                    stripped.to_string(),
                );
            }
        }
    }

    variants
}

fn push_token_variant(
    variants: &mut Vec<(String, String)>,
    seen: &mut HashSet<String>,
    label: &str,
    token: String,
) {
    if token.is_empty() || !seen.insert(token.clone()) {
        return;
    }
    variants.push((label.to_string(), token));
}

fn parse_tokenbroker_filetime(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let bytes = decode_base64(value)?;
    if bytes.len() < 8 {
        return None;
    }
    let filetime = i64::from_le_bytes(bytes[..8].try_into().ok()?);
    if filetime <= 0 {
        return None;
    }
    let unix_seconds = (filetime / 10_000_000) - 11_644_473_600;
    chrono::DateTime::<chrono::Utc>::from_timestamp(unix_seconds, 0)
}

fn decrypt_windows_dpapi_blob(encrypted: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(target_os = "windows")]
    {
        use std::{ptr, slice};
        use windows_sys::Win32::{
            Foundation::LocalFree,
            Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB},
        };

        unsafe {
            let input = CRYPT_INTEGER_BLOB {
                cbData: encrypted.len() as u32,
                pbData: encrypted.as_ptr() as *mut u8,
            };
            let mut output = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: ptr::null_mut(),
            };

            let success = CryptUnprotectData(
                &input,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                &mut output,
            );
            if success == 0 {
                return Err("Windows の保護トークンを復号できませんでした。".to_string());
            }

            let bytes = slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = LocalFree(output.pbData as *mut core::ffi::c_void);
            Ok(bytes)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = encrypted;
        Err("Windows 以外では TokenBroker の復号に対応していません。".to_string())
    }
}

async fn exchange_xbox_token_for_minecraft_access_token(
    token: &str,
    context: &str,
) -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent(MOJANG_USER_AGENT)
        .build()
        .ok()?;

    let max_attempts = 4;
    for attempt in 1..=max_attempts {
        let response = client
            .post("https://api.minecraftservices.com/launcher/login")
            .json(&serde_json::json!({
                "platform": "ONESTORE",
                "xtoken": token,
            }))
            .send()
            .await
            .ok()?;

        if response.status().is_success() {
            return response
                .json::<LauncherLoginResponse>()
                .await
                .ok()
                .map(|parsed| parsed.access_token);
        }

        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .unwrap_or_default()
            .replace('\n', " ")
            .replace('\r', " ");
        app_log::append_log(
            "WARN",
            format!(
                "/launcher/login returned status {} attempt={}/{} context={} body={}",
                status,
                attempt,
                max_attempts,
                context,
                truncate_log_text(&body, 160)
            ),
        );

        if status == 429 && attempt < max_attempts {
            let wait_ms = 900 * attempt as u64;
            app_log::append_log(
                "INFO",
                format!(
                    "rate limited by /launcher/login; waiting {}ms before retry context={}",
                    wait_ms, context
                ),
            );
            std::thread::sleep(Duration::from_millis(wait_ms));
            continue;
        }

        return None;
    }

    None
}

fn truncate_log_text(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        return trimmed.to_string();
    }
    let shortened: String = trimmed.chars().take(limit).collect();
    format!("{shortened}...")
}

fn decode_base64_url(value: &str) -> Option<Vec<u8>> {
    let mut normalized = value.replace('-', "+").replace('_', "/");
    while normalized.len() % 4 != 0 {
        normalized.push('=');
    }
    decode_base64(&normalized)
}

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;

    for character in value.chars() {
        if character == '=' {
            break;
        }
        let index = alphabet.find(character)? as u32;
        buffer = (buffer << 6) | index;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
        }
    }

    Some(output)
}
