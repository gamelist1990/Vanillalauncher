use crate::{app_log, models::XboxRpsStateResult, progress::emit_progress};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
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

#[derive(Debug, Clone)]
pub(super) struct CachedXboxAccountHint {
    pub(super) local_id: Option<String>,
    pub(super) username: Option<String>,
    pub(super) display_name: Option<String>,
    pub(super) gamer_tag: Option<String>,
    pub(super) xuid: Option<String>,
    pub(super) source_path: PathBuf,
}

#[derive(Debug, Clone)]
struct TokenbrokerFieldMatch {
    pattern: String,
    value: String,
    gap: Option<usize>,
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
    #[serde(rename = "gtg")]
    gamertag: Option<String>,
    #[serde(rename = "xid")]
    xuid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XboxXstsAuthorizeResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: Option<XboxDisplayClaims>,
}

#[derive(Debug, Clone)]
struct XboxProfileIdentity {
    gamer_tag: Option<String>,
    xuid: Option<String>,
}

#[derive(Debug, Clone)]
struct RpsTicketExchangeResult {
    minecraft_access_token: Option<String>,
    xbox_identity: Option<XboxProfileIdentity>,
}

#[derive(Debug, Deserialize)]
struct LocalLauncherEntitlementsFile {
    #[serde(default)]
    data: HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct LocalLauncherEntitlementPayload {
    #[serde(default)]
    items: Vec<LocalLauncherEntitlementItem>,
}

#[derive(Debug, Deserialize)]
struct LocalLauncherEntitlementItem {
    name: String,
    #[serde(default)]
    source: Option<String>,
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

fn read_cached_xbox_account_hints_raw() -> Result<Vec<CachedXboxAccountHint>, String> {
    if !cfg!(target_os = "windows") {
        return Ok(Vec::new());
    }

    let local_app_data = env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "LOCALAPPDATA が設定されていません。".to_string())?;

    let mut candidates = Vec::new();
    for cache_dir in xbox_token_cache_dirs(&local_app_data) {
        if !cache_dir.exists() {
            continue;
        }

        for entry in fs::read_dir(&cache_dir)
            .map_err(|error| format!("{} を読み込めませんでした: {error}", cache_dir.display()))?
        {
            let entry = entry
                .map_err(|error| format!("TokenBroker キャッシュの確認に失敗しました: {error}"))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("tbres") {
                continue;
            }

            let hint = match parse_cached_xbox_account_hint_file(&path) {
                Ok(hint) => hint,
                Err(error) => {
                    app_log::append_log(
                        "WARN",
                        format!(
                            "failed to parse cached xbox account hint from {}: {error}",
                            path.display()
                        ),
                    );
                    continue;
                }
            };
            let Some(hint) = hint else {
                continue;
            };
            candidates.push(hint);
        }
    }

    Ok(candidates)
}

pub(super) async fn read_cached_xbox_launcher_accounts(
    launcher_accounts: &[crate::minecraft::LauncherAccount],
    app: Option<&AppHandle>,
    operation_id: Option<&str>,
) -> Result<Vec<crate::minecraft::LauncherAccount>, String> {
    let emit_scan_progress = |detail: String, percent: f64| {
        if let (Some(app), Some(operation_id)) = (app, operation_id) {
            emit_progress(
                app,
                operation_id,
                "Launcher アカウント再検出",
                detail,
                percent,
            );
        }
    };
    let discovered = read_cached_xbox_account_hints_raw()?
        .into_iter()
        .filter_map(|hint| {
            let source_path = hint.source_path.clone();
            let account = super::cached_xbox_account_hint_to_launcher_account(hint)?;
            Some((source_path, account))
        })
        .collect::<Vec<_>>();

    if discovered.is_empty() {
        emit_scan_progress(
            "PC の認証キャッシュからアカウント候補を検出できませんでした。".to_string(),
            88.0,
        );
        return Ok(Vec::new());
    }

    emit_scan_progress(
        format!(
            "PC の認証キャッシュから {} 件の候補ソースを見つけました。照合順を整理しています。",
            discovered.len()
        ),
        36.0,
    );

    let java_access_hints = read_local_launcher_java_access_hints();
    let cached_tokens = read_cached_xbox_identity_tokens()?;
    let max_attempts = cached_tokens.len().clamp(12, 48);
    let (attempts, _used_saved_state) =
        build_prioritized_rps_attempts(&cached_tokens, max_attempts);
    let mut discovered = discovered;
    let mut discovered_indices_by_source = HashMap::<PathBuf, Vec<usize>>::new();
    let mut resolved_sources = HashSet::<PathBuf>::new();

    for (index, (source_path, _account)) in discovered.iter().enumerate() {
        discovered_indices_by_source
            .entry(source_path.clone())
            .or_default()
            .push(index);
    }

    app_log::append_log(
        "INFO",
        format!(
            "resolving cached xbox accounts online discovered_sources={} token_attempts={}",
            discovered_indices_by_source.len(),
            attempts.len()
        ),
    );
    emit_scan_progress(
        format!(
            "オンライン照合を開始します。{} 件の候補ソースに対して最大 {} 通りの認証パターンを試します。",
            discovered_indices_by_source.len(),
            attempts.len()
        ),
        42.0,
    );

    let total_attempts = attempts.len().max(1);
    for (attempt_index, (label, candidate, token)) in attempts.into_iter().enumerate() {
        let Some(indices) = discovered_indices_by_source
            .get(&token.source_path)
            .cloned()
        else {
            continue;
        };
        if resolved_sources.contains(&token.source_path) {
            continue;
        }

        let source_name = token
            .source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("cache.tbres");
        let percent = 42.0 + ((attempt_index as f64 / total_attempts as f64) * 46.0);
        emit_scan_progress(
            format!(
                "オンライン照合 {}/{}: {} を {} で確認しています。",
                attempt_index + 1,
                total_attempts,
                source_name,
                label
            ),
            percent,
        );

        let context = format!(
            "scan_cached_xbox_account:{}:{}",
            token.source_path.display(),
            label
        );
        let Some(exchange) = exchange_rps_ticket_for_minecraft_auth(&candidate, &context).await
        else {
            continue;
        };
        for index in &indices {
            let (_, account) = &mut discovered[*index];
            merge_xbox_identity_into_launcher_account(account, exchange.xbox_identity.as_ref());
        }
        let Some(access_token) = exchange.minecraft_access_token else {
            if exchange.xbox_identity.is_some() {
                app_log::append_log(
                    "INFO",
                    format!(
                        "resolved cached xbox account source={} with Xbox profile only",
                        token.source_path.display()
                    ),
                );
            }
            continue;
        };
        let Some((verified_profile, matched_account, used_fallback)) =
            resolve_verified_minecraft_profile(
                &access_token,
                &format!("{context} -> minecraft/profile"),
                launcher_accounts,
                &java_access_hints,
            )
            .await
        else {
            continue;
        };
        let matched_account = matched_account.or_else(|| {
            match_local_launcher_account_by_xuid(
                launcher_accounts,
                exchange
                    .xbox_identity
                    .as_ref()
                    .and_then(|identity| identity.xuid.as_deref()),
            )
        });

        for index in indices {
            let (_, account) = &mut discovered[index];
            merge_verified_identity_into_launcher_account(
                account,
                &verified_profile,
                matched_account.as_ref(),
            );
        }

        let matched_local_id = matched_account
            .as_ref()
            .and_then(|account| account.local_id.as_deref())
            .unwrap_or("none");
        app_log::append_log(
            "INFO",
            format!(
                "resolved cached xbox account source={} profile={} matched_local_id={} fallback={}",
                token.source_path.display(),
                verified_profile.0,
                matched_local_id,
                used_fallback
            ),
        );
        resolved_sources.insert(token.source_path.clone());
    }

    let collapsed = collapse_cached_xbox_launcher_accounts(
        discovered
            .into_iter()
            .map(|(_source_path, account)| account)
            .collect(),
    );
    app_log::append_log(
        "INFO",
        format!(
            "cached xbox account resolution complete resolved_sources={} collapsed_accounts={}",
            resolved_sources.len(),
            collapsed.len()
        ),
    );
    emit_scan_progress(
        format!(
            "認証キャッシュの整理が完了しました。{} 件のソースから {} 件のアカウント候補へ集約しました。",
            resolved_sources.len(),
            collapsed.len()
        ),
        88.0,
    );

    Ok(collapsed)
}

fn xbox_identity_from_display_claims(
    display_claims: Option<&XboxDisplayClaims>,
) -> Option<XboxProfileIdentity> {
    let claim = display_claims?.xui.first()?;
    let gamer_tag = claim
        .gamertag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let xuid = claim
        .xuid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if gamer_tag.is_none() && xuid.is_none() {
        None
    } else {
        Some(XboxProfileIdentity { gamer_tag, xuid })
    }
}

fn collapse_cached_xbox_launcher_accounts(
    accounts: Vec<crate::minecraft::LauncherAccount>,
) -> Vec<crate::minecraft::LauncherAccount> {
    let mut collapsed = Vec::<crate::minecraft::LauncherAccount>::new();

    for account in accounts {
        if let Some(existing) = collapsed
            .iter_mut()
            .find(|current| cached_launcher_accounts_match(current, &account))
        {
            merge_launcher_account_for_scan(existing, &account);
            continue;
        }
        collapsed.push(account);
    }

    collapsed.sort_by(|left, right| {
        let left_name = left
            .gamer_tag
            .as_deref()
            .or(left.username.as_deref())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let right_name = right
            .gamer_tag
            .as_deref()
            .or(right.username.as_deref())
            .unwrap_or_default()
            .to_ascii_lowercase();
        left_name.cmp(&right_name)
    });
    collapsed
}

fn cached_launcher_accounts_match(
    left: &crate::minecraft::LauncherAccount,
    right: &crate::minecraft::LauncherAccount,
) -> bool {
    let right_keys = super::launcher_account_identity_keys(right);
    if right_keys.is_empty() {
        return false;
    }

    super::launcher_account_identity_keys(left)
        .into_iter()
        .any(|value| right_keys.contains(&value))
}

fn merge_launcher_account_for_scan(
    target: &mut crate::minecraft::LauncherAccount,
    source: &crate::minecraft::LauncherAccount,
) {
    crate::minecraft::merge_launcher_account_fields(target, source);
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

async fn exchange_rps_ticket_for_minecraft_auth(
    ticket: &str,
    context: &str,
) -> Option<RpsTicketExchangeResult> {
    let client = build_mojang_client()?;

    let user_auth_response = post_json_with_retries(
        &client,
        "https://user.auth.xboxlive.com/user/authenticate",
        &serde_json::json!({
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": ticket,
            }
        }),
        context,
        "user/authenticate",
        3,
        true,
    )
    .await?;
    let user_auth = user_auth_response
        .json::<XboxUserAuthenticateResponse>()
        .await
        .ok()?;
    let uhs = user_auth
        .display_claims
        .as_ref()
        .and_then(|claims| claims.xui.first())
        .and_then(|claim| claim.uhs.clone())?;

    let xsts_response = post_json_with_retries(
        &client,
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        &serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [user_auth.token],
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }),
        context,
        "xsts/authorize",
        3,
        true,
    )
    .await?;
    let xsts = xsts_response
        .json::<XboxXstsAuthorizeResponse>()
        .await
        .ok()?;
    let xbox_identity = xbox_identity_from_display_claims(
        xsts.display_claims
            .as_ref()
            .or(user_auth.display_claims.as_ref()),
    );
    let identity_token = format!("XBL3.0 x={uhs};{}", xsts.token);
    let minecraft_access_token = exchange_xbox_token_for_minecraft_access_token(
        &identity_token,
        &format!("{context} -> xbl3"),
    )
    .await;

    Some(RpsTicketExchangeResult {
        minecraft_access_token,
        xbox_identity,
    })
}

pub(super) async fn exchange_rps_ticket_for_minecraft_access_token(
    ticket: &str,
    context: &str,
) -> Option<String> {
    exchange_rps_ticket_for_minecraft_auth(ticket, context)
        .await?
        .minecraft_access_token
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

    let launcher_accounts = match crate::minecraft::read_launcher_accounts() {
        Ok(accounts) => accounts,
        Err(error) => {
            app_log::append_log(
                "WARN",
                format!("failed to read launcher accounts while probing xbox-rps state: {error}"),
            );
            Vec::new()
        }
    };
    let java_access_hints = read_local_launcher_java_access_hints();

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

        if let Some(access_token) =
            exchange_rps_ticket_for_minecraft_access_token(&candidate, &context).await
        {
            let verification_context = format!("{context} -> minecraft/profile");
            if resolve_verified_minecraft_profile(
                &access_token,
                &verification_context,
                &launcher_accounts,
                &java_access_hints,
            )
            .await
            .is_none()
            {
                app_log::append_log(
                    "WARN",
                    format!(
                        "xbox-rps state candidate did not produce verified Minecraft Java access context={}",
                        context
                    ),
                );
            } else {
                let percent = (attempts_tried as f64 / total_attempts as f64) * 100.0;
                emit_auth_progress(
                "Xbox 認証確認",
                format!(
                    "試行 {attempts_tried}/{total_attempts} で Minecraft Java へのアクセスを確認しました"
                ),
                percent,
            );

                persist_xbox_rps_success_state(&token, &label, &candidate);

                return Ok(XboxRpsStateResult {
                message:
                    "Xbox RPS state を検証し、Minecraft Java へアクセスできる候補を保存しました。"
                        .to_string(),
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
        message:
            "Minecraft Java へのアクセス権が確認できる Xbox RPS state を更新できませんでした。"
                .to_string(),
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

pub(super) async fn fetch_minecraft_profile_for_token(
    token: &str,
    context: &str,
) -> Option<(String, String)> {
    let client = build_mojang_client()?;
    let max_attempts = 3;

    for attempt in 1..=max_attempts {
        let response = client
            .get("https://api.minecraftservices.com/minecraft/profile")
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await;

        let result = match response {
            Ok(result) => result,
            Err(error) => {
                app_log::append_log(
                    "WARN",
                    format!(
                        "minecraft/profile request failed attempt={}/{} context={} error={}",
                        attempt, max_attempts, context, error
                    ),
                );
                if attempt < max_attempts {
                    std::thread::sleep(Duration::from_millis(retry_wait_millis(attempt)));
                    continue;
                }
                return None;
            }
        };

        if result.status().is_success() {
            return result
                .json::<MinecraftServicesProfile>()
                .await
                .ok()
                .map(|profile| (profile.id, profile.name));
        }

        let status = result.status().as_u16();
        let body = result
            .text()
            .await
            .unwrap_or_default()
            .replace('\n', " ")
            .replace('\r', " ");
        app_log::append_log(
            "WARN",
            format!(
                "minecraft/profile returned status {} attempt={}/{} context={} body={}",
                status,
                attempt,
                max_attempts,
                context,
                truncate_log_text(&body, 160)
            ),
        );

        if attempt < max_attempts && should_retry_http_status(status) {
            std::thread::sleep(Duration::from_millis(retry_wait_millis(attempt)));
            continue;
        }

        return None;
    }

    None
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

pub(super) fn launcher_account_profile(
    account: &crate::minecraft::LauncherAccount,
) -> Option<(String, String)> {
    let profile_id = account
        .profile_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let profile_name = account
        .gamer_tag
        .as_deref()
        .or(account.username.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    Some((normalize_uuid_value(profile_id), profile_name.to_string()))
}

pub(super) fn match_local_launcher_account(
    accounts: &[crate::minecraft::LauncherAccount],
    verified_profile: Option<&(String, String)>,
    token: Option<&str>,
) -> Option<crate::minecraft::LauncherAccount> {
    if let Some(profile_id) = verified_profile.map(|profile| normalize_uuid_value(&profile.0)) {
        if let Some(account) = accounts.iter().find(|account| {
            account
                .profile_id
                .as_deref()
                .map(normalize_uuid_value)
                .is_some_and(|candidate| candidate == profile_id)
        }) {
            return Some(account.clone());
        }
    }

    let token_uuid = token.and_then(uuid_from_access_token);
    token_uuid.and_then(|uuid| {
        accounts
            .iter()
            .find(|account| {
                account
                    .profile_id
                    .as_deref()
                    .map(normalize_uuid_value)
                    .is_some_and(|candidate| candidate == uuid)
            })
            .cloned()
    })
}

fn match_local_launcher_account_by_xuid(
    accounts: &[crate::minecraft::LauncherAccount],
    xuid: Option<&str>,
) -> Option<crate::minecraft::LauncherAccount> {
    let xuid = xuid.map(str::trim).filter(|value| !value.is_empty())?;

    accounts
        .iter()
        .find(|account| {
            account
                .xuid
                .as_deref()
                .map(str::trim)
                .is_some_and(|candidate| candidate == xuid)
        })
        .cloned()
}

pub(super) fn read_local_launcher_java_access_hints() -> HashMap<String, bool> {
    let Ok(root) = crate::minecraft::minecraft_root() else {
        return HashMap::new();
    };

    let mut hints = HashMap::new();
    for file_name in [
        "launcher_entitlements_microsoft_store.json",
        "launcher_entitlements.json",
    ] {
        let path = root.join(file_name);
        if !path.exists() {
            continue;
        }

        if let Err(error) = merge_local_launcher_java_access_hints(&mut hints, &path) {
            app_log::append_log(
                "WARN",
                format!(
                    "failed to read launcher entitlement hints from {}: {error}",
                    path.display()
                ),
            );
        }
    }

    hints
}

pub(super) fn launcher_account_has_java_access_hint(
    account: &crate::minecraft::LauncherAccount,
    hints: &HashMap<String, bool>,
) -> bool {
    account
        .xuid
        .as_deref()
        .and_then(|xuid| hints.get(xuid))
        .copied()
        .unwrap_or(false)
}

pub(super) async fn resolve_verified_minecraft_profile(
    token: &str,
    context: &str,
    accounts: &[crate::minecraft::LauncherAccount],
    java_access_hints: &HashMap<String, bool>,
) -> Option<(
    (String, String),
    Option<crate::minecraft::LauncherAccount>,
    bool,
)> {
    let verified_profile = fetch_minecraft_profile_for_token(token, context).await;
    let matched_account =
        match_local_launcher_account(accounts, verified_profile.as_ref(), Some(token));

    if let Some((profile_id, profile_name)) = verified_profile {
        return Some((
            (normalize_uuid_value(&profile_id), profile_name),
            matched_account,
            false,
        ));
    }

    let matched_account = matched_account?;
    if !launcher_account_has_java_access_hint(&matched_account, java_access_hints) {
        return None;
    }

    let fallback_profile = launcher_account_profile(&matched_account)?;
    app_log::append_log(
        "INFO",
        format!(
            "using launcher entitlement hint for context={} profile={} xuid={}",
            context,
            fallback_profile.0,
            matched_account.xuid.as_deref().unwrap_or("unknown")
        ),
    );

    Some((fallback_profile, Some(matched_account), true))
}

fn merge_verified_identity_into_launcher_account(
    target: &mut crate::minecraft::LauncherAccount,
    verified_profile: &(String, String),
    matched_account: Option<&crate::minecraft::LauncherAccount>,
) {
    target.profile_id = Some(normalize_uuid_value(&verified_profile.0));
    target.gamer_tag = Some(verified_profile.1.clone());

    let Some(matched_account) = matched_account else {
        return;
    };

    if target
        .xuid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        target.xuid = matched_account
            .xuid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }
}

fn merge_xbox_identity_into_launcher_account(
    target: &mut crate::minecraft::LauncherAccount,
    xbox_identity: Option<&XboxProfileIdentity>,
) {
    let Some(xbox_identity) = xbox_identity else {
        return;
    };

    if let Some(gamer_tag) = xbox_identity
        .gamer_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        target.gamer_tag = Some(gamer_tag.to_string());
        target.xbox_profile_verified = true;
    }

    if target
        .xuid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        target.xuid = xbox_identity
            .xuid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }
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
    if path.contains("minecraftlauncher") {
        score += 220;
    }
    if path.contains("windowsstore") {
        score += 140;
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
    let mut candidates = vec![
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
    ];

    let packages_root = base.join("Packages");
    if let Ok(entries) = fs::read_dir(&packages_root) {
        for entry in entries.flatten() {
            let package_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if !looks_like_auth_related_package(&package_name) {
                continue;
            }

            candidates.push(entry.path().join("AC").join("TokenBroker").join("Cache"));
        }
    }

    candidates.sort();
    candidates.dedup();
    candidates
}

fn looks_like_auth_related_package(package_name: &str) -> bool {
    package_name.contains("xbox")
        || package_name.contains("gaming")
        || package_name.contains("minecraft")
        || package_name.contains("store")
        || package_name.contains("identity")
}

fn parse_cached_xbox_token_file(path: &Path) -> Result<Option<CachedXboxToken>, String> {
    let (payload, expires_at) = read_tokenbroker_payload(path)?;
    if payload.trim().is_empty() {
        return Ok(None);
    }
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

fn parse_cached_xbox_account_hint_file(
    path: &Path,
) -> Result<Option<CachedXboxAccountHint>, String> {
    let (payload, _expires_at) = read_tokenbroker_payload(path)?;
    Ok(extract_cached_xbox_account_hint(&payload, path))
}

fn read_tokenbroker_payload(
    path: &Path,
) -> Result<(String, Option<chrono::DateTime<chrono::Utc>>), String> {
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
        return Ok((String::new(), None));
    };
    let expires_at = system_defined
        .get("Expiration")
        .and_then(|value| value.get("Value"))
        .and_then(Value::as_str)
        .and_then(parse_tokenbroker_filetime);

    let decrypted =
        decrypt_windows_dpapi_blob(&decode_base64(response_blob).ok_or_else(|| {
            format!(
                "{} の ResponseBytes を復号前に解釈できませんでした。",
                path.display()
            )
        })?)?;

    Ok((String::from_utf8_lossy(&decrypted).into_owned(), expires_at))
}

fn extract_cached_xbox_account_hint(
    payload: &str,
    source_path: &Path,
) -> Option<CachedXboxAccountHint> {
    if payload.trim().is_empty() {
        return None;
    }

    let segments = payload
        .split('\0')
        .map(clean_tokenbroker_segment)
        .collect::<Vec<_>>();
    let username_match = extract_tokenbroker_segment_match(
        &segments,
        &[
            "UserName",
            "WA_UserName",
            "preferred_username",
            "login_hint",
        ],
    )
    .or_else(|| extract_marker_match(payload, "preferred_username="))
    .or_else(|| extract_marker_match(payload, "login_hint="));
    let display_name_match = extract_tokenbroker_segment_match(&segments, &["DisplayName"])
        .filter(|item| !looks_like_generic_account_display_name(&item.value));
    let gamer_tag_match = extract_tokenbroker_segment_match(
        &segments,
        &["Gamertag", "GamerTag", "PublicGamerTag", "XboxGamerTag"],
    );
    let xuid_match = extract_tokenbroker_segment_match(&segments, &["XUID", "xuid"]);
    let local_id_match =
        extract_tokenbroker_segment_match(&segments, &["UID", "WA_Id", "home_account_id"])
            .or_else(|| extract_marker_match(payload, "home_account_id="))
            .or_else(|| xuid_match.clone())
            .or_else(|| username_match.clone());
    let fallback_display_match = extract_tokenbroker_segment_match(&segments, &["WAP_DisplayName"])
        .filter(|item| !looks_like_generic_account_display_name(&item.value));

    let username = username_match.as_ref().map(|item| item.value.clone());
    let display_name = display_name_match.as_ref().map(|item| item.value.clone());
    let gamer_tag = gamer_tag_match.as_ref().map(|item| item.value.clone());
    let xuid = xuid_match.as_ref().map(|item| item.value.clone());
    let local_id = local_id_match.as_ref().map(|item| item.value.clone());
    let fallback_display = fallback_display_match
        .as_ref()
        .map(|item| item.value.clone());

    if local_id.is_none()
        && username.is_none()
        && display_name.is_none()
        && gamer_tag.is_none()
        && xuid.is_none()
        && fallback_display.is_none()
    {
        return None;
    }

    let hint = CachedXboxAccountHint {
        local_id,
        username,
        display_name: display_name.or(fallback_display),
        gamer_tag,
        xuid,
        source_path: source_path.to_path_buf(),
    };
    log_cached_xbox_account_hint_match(
        source_path,
        &hint,
        local_id_match.as_ref(),
        username_match.as_ref(),
        display_name_match
            .as_ref()
            .or(fallback_display_match.as_ref()),
        gamer_tag_match.as_ref(),
        xuid_match.as_ref(),
    );

    Some(hint)
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

fn extract_marker_match(payload: &str, marker: &str) -> Option<TokenbrokerFieldMatch> {
    extract_marker_value(payload, marker).map(|value| TokenbrokerFieldMatch {
        pattern: format!("marker:{marker}"),
        value,
        gap: None,
    })
}

fn extract_tokenbroker_segment_match(
    segments: &[String],
    keys: &[&str],
) -> Option<TokenbrokerFieldMatch> {
    for (index, segment) in segments.iter().enumerate() {
        if segment.is_empty() {
            continue;
        }
        if !keys.iter().any(|key| segment.eq_ignore_ascii_case(key)) {
            continue;
        }

        for (offset, candidate) in segments.iter().skip(index + 1).take(16).enumerate() {
            let value = candidate.trim();
            if value.is_empty() {
                continue;
            }
            if keys.iter().any(|key| value.eq_ignore_ascii_case(key)) {
                continue;
            }
            return Some(TokenbrokerFieldMatch {
                pattern: format!("segment:{}", segment),
                value: value.to_string(),
                gap: Some(offset + 1),
            });
        }
    }

    None
}

fn log_cached_xbox_account_hint_match(
    source_path: &Path,
    hint: &CachedXboxAccountHint,
    local_id_match: Option<&TokenbrokerFieldMatch>,
    username_match: Option<&TokenbrokerFieldMatch>,
    display_name_match: Option<&TokenbrokerFieldMatch>,
    gamer_tag_match: Option<&TokenbrokerFieldMatch>,
    xuid_match: Option<&TokenbrokerFieldMatch>,
) {
    let fields = [
        ("local_id", hint.local_id.as_deref(), local_id_match),
        ("username", hint.username.as_deref(), username_match),
        (
            "display_name",
            hint.display_name.as_deref(),
            display_name_match,
        ),
        ("gamer_tag", hint.gamer_tag.as_deref(), gamer_tag_match),
        ("xuid", hint.xuid.as_deref(), xuid_match),
    ];
    let summary = fields
        .into_iter()
        .filter_map(|(label, value, matched)| {
            let value = value?;
            let matched = matched?;
            Some(format!(
                "{}={} via {}",
                label,
                preview_account_hint_value(value),
                describe_tokenbroker_match_pattern(matched)
            ))
        })
        .collect::<Vec<_>>();

    if summary.is_empty() {
        return;
    }

    app_log::append_log(
        "INFO",
        format!(
            "cached xbox account hint matched path={} {}",
            source_path.display(),
            summary.join(" | ")
        ),
    );
}

fn describe_tokenbroker_match_pattern(matched: &TokenbrokerFieldMatch) -> String {
    match matched.gap {
        Some(gap) => format!("{} gap={}", matched.pattern, gap),
        None => matched.pattern.clone(),
    }
}

fn preview_account_hint_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "(empty)".to_string();
    }

    if let Some((local, domain)) = trimmed.split_once('@') {
        let prefix: String = local.chars().take(2).collect();
        return format!("{prefix}***@{domain}");
    }

    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= 8 {
        return trimmed.to_string();
    }

    let head: String = chars.iter().take(4).copied().collect();
    let tail: String = chars
        .iter()
        .rev()
        .take(2)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}...{tail}")
}

fn clean_tokenbroker_segment(segment: &str) -> String {
    segment
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

fn looks_like_generic_account_display_name(value: &str) -> bool {
    let lowered = value.trim().to_ascii_lowercase();
    lowered.is_empty()
        || lowered == "microsoft account"
        || lowered == "microsoft アカウント"
        || lowered.contains("cloudexperiencehost")
        || lowered.starts_with("@{microsoft.")
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
    let client = build_mojang_client()?;

    let max_attempts = 4;
    for attempt in 1..=max_attempts {
        let response = client
            .post("https://api.minecraftservices.com/launcher/login")
            .json(&serde_json::json!({
                "platform": "ONESTORE",
                "xtoken": token,
            }))
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                app_log::append_log(
                    "WARN",
                    format!(
                        "/launcher/login request failed attempt={}/{} context={} error={}",
                        attempt, max_attempts, context, error
                    ),
                );
                if attempt < max_attempts {
                    std::thread::sleep(Duration::from_millis(retry_wait_millis(attempt)));
                    continue;
                }
                return None;
            }
        };

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

        if should_retry_http_status(status) && attempt < max_attempts {
            let wait_ms = retry_wait_millis(attempt);
            app_log::append_log(
                "INFO",
                format!(
                    "/launcher/login retry scheduled after {}ms context={}",
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

fn merge_local_launcher_java_access_hints(
    hints: &mut HashMap<String, bool>,
    path: &Path,
) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", path.display()))?;
    let file = serde_json::from_str::<LocalLauncherEntitlementsFile>(&text)
        .map_err(|error| format!("{} を解析できませんでした: {error}", path.display()))?;

    for (signer_id, payload_value) in file.data {
        let Some(payload) = parse_local_launcher_entitlement_payload(payload_value) else {
            continue;
        };
        let grants_java_access = payload
            .items
            .iter()
            .any(local_launcher_entitlement_grants_java_access);
        hints
            .entry(signer_id)
            .and_modify(|current| *current = *current || grants_java_access)
            .or_insert(grants_java_access);
    }

    Ok(())
}

fn parse_local_launcher_entitlement_payload(
    value: Value,
) -> Option<LocalLauncherEntitlementPayload> {
    match value {
        Value::String(text) => serde_json::from_str::<LocalLauncherEntitlementPayload>(&text).ok(),
        Value::Object(map) => {
            serde_json::from_value::<LocalLauncherEntitlementPayload>(Value::Object(map)).ok()
        }
        _ => None,
    }
}

fn local_launcher_entitlement_grants_java_access(item: &LocalLauncherEntitlementItem) -> bool {
    matches!(
        item.name.to_ascii_lowercase().as_str(),
        "product_minecraft" | "game_minecraft" | "product_minecraft_java" | "game_minecraft_java"
    ) && entitlement_source_grants_java_access(item.source.as_deref())
}

fn entitlement_source_grants_java_access(source: Option<&str>) -> bool {
    let source = source
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_ascii_lowercase();
    !matches!(source.as_str(), "" | "trial" | "demo" | "expired" | "none")
}

fn build_mojang_client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(MOJANG_USER_AGENT)
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .build()
        .ok()
}

fn should_retry_http_status(status: u16) -> bool {
    status == 408 || status == 425 || status == 429 || status >= 500
}

fn retry_wait_millis(attempt: usize) -> u64 {
    400 * attempt as u64
}

async fn post_json_with_retries(
    client: &reqwest::Client,
    url: &str,
    body: &serde_json::Value,
    context: &str,
    label: &str,
    max_attempts: usize,
    include_xbl_contract_header: bool,
) -> Option<reqwest::Response> {
    for attempt in 1..=max_attempts {
        let mut request = client.post(url);
        if include_xbl_contract_header {
            request = request.header("x-xbl-contract-version", "1");
        }

        let response = request.json(body).send().await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                app_log::append_log(
                    "WARN",
                    format!(
                        "{} request failed attempt={}/{} context={} error={}",
                        label, attempt, max_attempts, context, error
                    ),
                );
                if attempt < max_attempts {
                    std::thread::sleep(Duration::from_millis(retry_wait_millis(attempt)));
                    continue;
                }
                return None;
            }
        };

        if response.status().is_success() {
            return Some(response);
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
                "{} returned status {} attempt={}/{} context={} body={}",
                label,
                status,
                attempt,
                max_attempts,
                context,
                truncate_log_text(&body, 160)
            ),
        );

        if attempt < max_attempts && should_retry_http_status(status) {
            std::thread::sleep(Duration::from_millis(retry_wait_millis(attempt)));
            continue;
        }

        return None;
    }

    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_launcher_account_from_verified_profile() {
        let accounts = vec![crate::minecraft::LauncherAccount {
            gamer_tag: Some("PlayerOne".to_string()),
            profile_id: Some("c53f907d0ad242c699c33994a3c1caa4".to_string()),
            ..Default::default()
        }];
        let verified_profile = (
            "c53f907d-0ad2-42c6-99c3-3994a3c1caa4".to_string(),
            "PlayerOne".to_string(),
        );

        let matched = match_local_launcher_account(&accounts, Some(&verified_profile), None)
            .expect("expected launcher account to match verified profile");

        assert_eq!(matched.gamer_tag.as_deref(), Some("PlayerOne"));
    }

    #[test]
    fn matches_launcher_account_from_xuid() {
        let accounts = vec![crate::minecraft::LauncherAccount {
            gamer_tag: Some("PEXkoukunn".to_string()),
            xuid: Some("2535457379922907".to_string()),
            local_id: Some("saved-local-id".to_string()),
            ..Default::default()
        }];

        let matched = match_local_launcher_account_by_xuid(&accounts, Some("2535457379922907"))
            .expect("expected launcher account to match xuid");

        assert_eq!(matched.local_id.as_deref(), Some("saved-local-id"));
    }

    #[test]
    fn parses_java_launcher_entitlement_payload() {
        let payload = parse_local_launcher_entitlement_payload(Value::String(
            r#"{
                "items": [
                    { "name": "product_minecraft_bedrock", "source": "TRIAL" },
                    { "name": "game_minecraft", "source": "PURCHASE" }
                ]
            }"#
            .to_string(),
        ))
        .expect("expected entitlement payload to parse");

        assert!(payload
            .items
            .iter()
            .any(local_launcher_entitlement_grants_java_access));
    }

    #[test]
    fn trial_entitlement_does_not_grant_java_access() {
        let payload = parse_local_launcher_entitlement_payload(Value::String(
            r#"{
                "items": [
                    { "name": "game_minecraft", "source": "TRIAL" }
                ]
            }"#
            .to_string(),
        ))
        .expect("expected entitlement payload to parse");

        assert!(!payload
            .items
            .iter()
            .any(local_launcher_entitlement_grants_java_access));
    }

    #[test]
    fn extracts_cached_account_hint_from_tokenbroker_payload() {
        let payload = "\u{0b}DisplayName\0\0\0\0\0\0PC My\0WA_UserName\0\0\0\0\0\0isseidas@gmail.com\0UID\0\0\0\0\0\000037FFE1DB0472E\0XUID\0\0\0\0\0\0123456789\0";

        let hint = extract_cached_xbox_account_hint(payload, Path::new("cache.tbres"))
            .expect("expected cached account hint");

        assert_eq!(hint.display_name.as_deref(), Some("PC My"));
        assert_eq!(hint.username.as_deref(), Some("isseidas@gmail.com"));
        assert_eq!(hint.local_id.as_deref(), Some("00037FFE1DB0472E"));
        assert_eq!(hint.xuid.as_deref(), Some("123456789"));
    }

    #[test]
    fn ignores_generic_provider_display_name_when_extracting_account_hint() {
        let payload = "WAP_DisplayName\0\0Microsoft アカウント\0WA_UserName\0\0user@example.com\0WA_Id\0\0abc123\0";

        let hint = extract_cached_xbox_account_hint(payload, Path::new("cache.tbres"))
            .expect("expected cached account hint");

        assert_eq!(hint.display_name, None);
        assert_eq!(hint.username.as_deref(), Some("user@example.com"));
        assert_eq!(hint.local_id.as_deref(), Some("abc123"));
    }

    #[test]
    fn merges_verified_identity_into_detected_launcher_account() {
        let mut detected = crate::minecraft::LauncherAccount {
            username: Some("isseidas@gmail.com".to_string()),
            gamer_tag: Some("PC My".to_string()),
            local_id: Some("00037FFE1DB0472E".to_string()),
            ..Default::default()
        };
        let matched = crate::minecraft::LauncherAccount {
            gamer_tag: Some("PEXkoukunn".to_string()),
            profile_id: Some("c53f907d0ad242c699c33994a3c1caa4".to_string()),
            xuid: Some("2535457379922907".to_string()),
            local_id: Some("60b756e5db1d4abaa16908b50dcaa086".to_string()),
            ..Default::default()
        };

        merge_verified_identity_into_launcher_account(
            &mut detected,
            &(
                "c53f907d-0ad2-42c6-99c3-3994a3c1caa4".to_string(),
                "PEXkoukunn".to_string(),
            ),
            Some(&matched),
        );

        assert_eq!(
            detected.profile_id.as_deref(),
            Some("c53f907d0ad242c699c33994a3c1caa4")
        );
        assert_eq!(detected.xuid.as_deref(), Some("2535457379922907"));
        assert_eq!(detected.username.as_deref(), Some("isseidas@gmail.com"));
        assert_eq!(detected.gamer_tag.as_deref(), Some("PEXkoukunn"));
    }

    #[test]
    fn merges_verified_xbox_profile_into_detected_launcher_account() {
        let mut detected = crate::minecraft::LauncherAccount {
            username: Some("pexkurann@gmail.com".to_string()),
            local_id: Some("00037FFE1DB0472E".to_string()),
            ..Default::default()
        };

        merge_xbox_identity_into_launcher_account(
            &mut detected,
            Some(&XboxProfileIdentity {
                gamer_tag: Some("PEXkurann".to_string()),
                xuid: Some("2535457379922907".to_string()),
            }),
        );

        assert_eq!(detected.gamer_tag.as_deref(), Some("PEXkurann"));
        assert_eq!(detected.xuid.as_deref(), Some("2535457379922907"));
        assert!(detected.xbox_profile_verified);
    }
}
