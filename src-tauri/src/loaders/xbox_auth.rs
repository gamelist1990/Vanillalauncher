use crate::{app_log, models::XboxRpsStateResult, progress::emit_progress};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};
use tauri::AppHandle;
use tokio::sync::Mutex;

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

static XBOX_AUTH_EXCHANGE_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn xbox_auth_exchange_mutex() -> &'static Mutex<()> {
    XBOX_AUTH_EXCHANGE_MUTEX.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone)]
struct TokenbrokerFieldMatch {
    pattern: String,
    value: String,
    gap: Option<usize>,
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
pub(super) struct XboxProfileIdentity {
    pub(super) gamer_tag: Option<String>,
    pub(super) xuid: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct RpsTicketExchangeResult {
    pub(super) minecraft_access_token: Option<String>,
    pub(super) xbox_identity: Option<XboxProfileIdentity>,
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct SecureLaunchTokenCacheFile {
    version: u32,
    #[serde(default)]
    entries: HashMap<String, SecureLaunchTokenCacheEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SecureLaunchTokenCacheEntry {
    access_token: String,
    #[serde(default)]
    expires_at: Option<String>,
    username: String,
    uuid: String,
    #[serde(default)]
    xuid: Option<String>,
    #[serde(default)]
    local_id: Option<String>,
    #[serde(default)]
    user_properties: Option<String>,
    #[serde(default)]
    user_type: Option<String>,
    saved_at: String,
}

#[derive(Debug, Clone)]
pub(super) struct SecureLaunchToken {
    pub(super) access_token: String,
    pub(super) expires_at: Option<String>,
    pub(super) username: String,
    pub(super) uuid: String,
    pub(super) xuid: String,
    pub(super) user_properties: String,
    pub(super) user_type: String,
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
    let attempts = build_prioritized_rps_attempts(&cached_tokens, max_attempts);
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
            resolve_verified_minecraft_profile_with_xbox_identity(
                &access_token,
                &format!("{context} -> minecraft/profile"),
                launcher_accounts,
                &java_access_hints,
                exchange.xbox_identity.as_ref(),
            )
            .await
        else {
            continue;
        };

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
) -> Vec<(String, String, CachedXboxToken)> {
    build_prioritized_rps_attempts_for_account(cached_tokens, max_attempts, None)
}

pub(super) fn build_prioritized_rps_attempts_for_account(
    cached_tokens: &[CachedXboxToken],
    max_attempts: usize,
    preferred_account: Option<&crate::minecraft::LauncherAccount>,
) -> Vec<(String, String, CachedXboxToken)> {
    let mut attempts: Vec<(i32, String, String, CachedXboxToken)> = Vec::new();
    let mut seen = HashSet::new();
    let preferred_source_scores = preferred_cached_xbox_source_scores(preferred_account);

    for token in cached_tokens {
        for (label, candidate) in build_xbox_token_variants(&token.token) {
            if !candidate.contains("t=") {
                continue;
            }
            if !seen.insert(candidate.clone()) {
                continue;
            }
            let score = rank_rps_variant(token, &label, &candidate)
                + preferred_source_scores
                    .get(&token.source_path)
                    .copied()
                    .unwrap_or_default();
            attempts.push((score, label, candidate, token.clone()));
        }
    }

    attempts.sort_by(|left, right| right.0.cmp(&left.0));
    let bounded = attempts
        .into_iter()
        .take(max_attempts)
        .map(|(_score, label, candidate, token)| (label, candidate, token))
        .collect();

    bounded
}

fn preferred_cached_xbox_source_scores(
    preferred_account: Option<&crate::minecraft::LauncherAccount>,
) -> HashMap<PathBuf, i32> {
    let Some(preferred_account) = preferred_account else {
        return HashMap::new();
    };

    let hints = match read_cached_xbox_account_hints_raw() {
        Ok(hints) => hints,
        Err(error) => {
            app_log::append_log(
                "WARN",
                format!(
                    "failed to read cached xbox account hints while prioritizing attempts: {error}"
                ),
            );
            return HashMap::new();
        }
    };
    let scores = preferred_cached_xbox_source_scores_from_hints(Some(preferred_account), &hints);
    if !scores.is_empty() {
        app_log::append_log(
            "INFO",
            format!(
                "prioritizing cached Xbox sources for selected account matched_sources={}",
                scores.len()
            ),
        );
    }
    scores
}

fn preferred_cached_xbox_source_scores_from_hints(
    preferred_account: Option<&crate::minecraft::LauncherAccount>,
    hints: &[CachedXboxAccountHint],
) -> HashMap<PathBuf, i32> {
    let Some(preferred_account) = preferred_account else {
        return HashMap::new();
    };

    let mut scores = HashMap::new();
    for hint in hints {
        let score = cached_xbox_account_hint_match_score(hint, preferred_account);
        if score <= 0 {
            continue;
        }
        scores
            .entry(hint.source_path.clone())
            .and_modify(|current: &mut i32| *current = (*current).max(score))
            .or_insert(score);
    }
    scores
}

fn cached_xbox_account_hint_match_score(
    hint: &CachedXboxAccountHint,
    preferred_account: &crate::minecraft::LauncherAccount,
) -> i32 {
    let preferred_display_name =
        crate::minecraft::preferred_launcher_account_display_name(preferred_account);
    let mut score = 0;

    if account_identity_matches(
        hint.local_id.as_deref(),
        preferred_account.local_id.as_deref(),
    ) {
        score += 5_000;
    }
    if account_identity_matches(hint.xuid.as_deref(), preferred_account.xuid.as_deref()) {
        score += 4_200;
    }
    if account_identity_matches(
        hint.username.as_deref(),
        preferred_account.username.as_deref(),
    ) {
        score += 3_200;
    }
    if account_identity_matches(
        hint.gamer_tag.as_deref(),
        preferred_account.gamer_tag.as_deref(),
    ) {
        score += 2_600;
    }
    if account_identity_matches(
        hint.display_name.as_deref(),
        preferred_account.gamer_tag.as_deref(),
    ) {
        score += 2_200;
    }
    if account_identity_matches(
        hint.display_name.as_deref(),
        preferred_display_name.as_deref(),
    ) {
        score += 1_800;
    }

    score
}

fn account_identity_matches(left: Option<&str>, right: Option<&str>) -> bool {
    let Some(left) = normalize_account_identity_value(left) else {
        return false;
    };
    let Some(right) = normalize_account_identity_value(right) else {
        return false;
    };
    left == right
}

fn normalize_account_identity_value(value: Option<&str>) -> Option<String> {
    let value = value.map(str::trim).filter(|entry| !entry.is_empty())?;
    if value.len() >= 32
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() || character == '-')
    {
        return Some(normalize_uuid_value(value).to_ascii_lowercase());
    }
    Some(value.to_ascii_lowercase())
}

pub(super) fn preview_token(token: &str) -> String {
    let prefix: String = token.chars().take(12).collect();
    format!("{prefix}...len={}", token.len())
}

pub(super) async fn exchange_rps_ticket_for_minecraft_auth(
    ticket: &str,
    context: &str,
) -> Option<RpsTicketExchangeResult> {
    let _exchange_guard = xbox_auth_exchange_mutex().lock().await;
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

pub(super) async fn ensure_xbox_rps_state(
    app: Option<&AppHandle>,
    operation_id: Option<&str>,
) -> Result<XboxRpsStateResult, String> {
    let state_path = String::new();
    let used_saved_state = false;

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
            state_path: state_path.clone(),
            used_saved_state,
            refreshed: false,
            succeeded: false,
            attempts_tried: 0,
            total_attempts: 0,
            source_path: None,
            variant_label: None,
        });
    }

    let preferred_account = match crate::minecraft::read_active_launcher_account() {
        Ok(account) => account,
        Err(error) => {
            app_log::append_log(
                "WARN",
                format!(
                    "failed to read active launcher account while probing xbox-rps state: {error}"
                ),
            );
            None
        }
    };
    let bounded =
        build_prioritized_rps_attempts_for_account(&cached_tokens, 12, preferred_account.as_ref());
    let total_attempts = bounded.len();

    if total_attempts == 0 {
        emit_auth_progress(
            "Xbox 認証確認",
            "候補が 0 件のため、既存情報で続行します (0/0)".to_string(),
            100.0,
        );
        return Ok(XboxRpsStateResult {
            message: "有効な Xbox RPS 候補を構築できませんでした。".to_string(),
            state_path: state_path.clone(),
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

        if let Some(exchange) = exchange_rps_ticket_for_minecraft_auth(&candidate, &context).await {
            let Some(access_token) = exchange.minecraft_access_token else {
                continue;
            };
            let verification_context = format!("{context} -> minecraft/profile");
            if resolve_verified_minecraft_profile_with_xbox_identity(
                &access_token,
                &verification_context,
                &launcher_accounts,
                &java_access_hints,
                exchange.xbox_identity.as_ref(),
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

                return Ok(XboxRpsStateResult {
                message:
                    "Xbox RPS state を検証し、Minecraft Java へアクセスできる候補を保存しました。"
                        .to_string(),
                state_path: state_path.clone(),
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

        tokio::time::sleep(Duration::from_millis(700)).await;
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
        state_path,
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

pub(super) fn access_token_expiry_rfc3339(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = decode_base64_url(payload)?;
    let value = serde_json::from_slice::<Value>(&decoded).ok()?;
    let expiry = value.get("exp").and_then(Value::as_i64)?;
    let expires_at = chrono::DateTime::<chrono::Utc>::from_timestamp(expiry, 0)?;
    Some(expires_at.to_rfc3339())
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

fn match_local_launcher_account_with_xbox_identity(
    accounts: &[crate::minecraft::LauncherAccount],
    verified_profile: Option<&(String, String)>,
    token: Option<&str>,
    xbox_identity: Option<&XboxProfileIdentity>,
) -> Option<crate::minecraft::LauncherAccount> {
    match_local_launcher_account(accounts, verified_profile, token).or_else(|| {
        match_local_launcher_account_by_xuid(
            accounts,
            xbox_identity.and_then(|identity| identity.xuid.as_deref()),
        )
    })
}

fn resolve_verified_profile_or_hint(
    verified_profile: Option<(String, String)>,
    matched_account: Option<crate::minecraft::LauncherAccount>,
    context: &str,
    java_access_hints: &HashMap<String, bool>,
) -> Option<(
    (String, String),
    Option<crate::minecraft::LauncherAccount>,
    bool,
)> {
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
    resolve_verified_minecraft_profile_with_xbox_identity(
        token,
        context,
        accounts,
        java_access_hints,
        None,
    )
    .await
}

pub(super) async fn resolve_verified_minecraft_profile_with_xbox_identity(
    token: &str,
    context: &str,
    accounts: &[crate::minecraft::LauncherAccount],
    java_access_hints: &HashMap<String, bool>,
    xbox_identity: Option<&XboxProfileIdentity>,
) -> Option<(
    (String, String),
    Option<crate::minecraft::LauncherAccount>,
    bool,
)> {
    let verified_profile = fetch_minecraft_profile_for_token(token, context).await;
    let matched_account = match_local_launcher_account_with_xbox_identity(
        accounts,
        verified_profile.as_ref(),
        Some(token),
        xbox_identity,
    );

    resolve_verified_profile_or_hint(
        verified_profile,
        matched_account,
        context,
        java_access_hints,
    )
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

fn secure_launch_token_cache_path() -> PathBuf {
    env::temp_dir()
        .join("VanillaLauncher")
        .join("launch-auth-cache.bin")
}

fn secure_launch_token_cache_keys(
    account: Option<&crate::minecraft::LauncherAccount>,
) -> Vec<String> {
    let Some(account) = account else {
        return Vec::new();
    };

    let mut keys = Vec::new();
    for value in [account.local_id.as_deref(), account.xuid.as_deref()] {
        let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
            continue;
        };
        let normalized = value.to_ascii_lowercase();
        if !keys.contains(&normalized) {
            keys.push(normalized);
        }
    }

    keys
}

fn load_secure_launch_token_cache() -> Result<SecureLaunchTokenCacheFile, String> {
    let path = secure_launch_token_cache_path();
    if !path.exists() {
        return Ok(SecureLaunchTokenCacheFile {
            version: 1,
            entries: HashMap::new(),
        });
    }

    let encrypted = fs::read(&path)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", path.display()))?;
    if encrypted.is_empty() {
        return Ok(SecureLaunchTokenCacheFile {
            version: 1,
            entries: HashMap::new(),
        });
    }

    let decrypted = decrypt_windows_dpapi_blob(&encrypted)?;
    serde_json::from_slice::<SecureLaunchTokenCacheFile>(&decrypted).map_err(|error| {
        format!(
            "{} の認証キャッシュを解析できませんでした: {error}",
            path.display()
        )
    })
}

fn save_secure_launch_token_cache(cache: &SecureLaunchTokenCacheFile) -> Result<(), String> {
    let path = secure_launch_token_cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を作成できませんでした: {error}", parent.display()))?;
    }

    let payload = serde_json::to_vec(cache)
        .map_err(|error| format!("認証キャッシュを JSON 化できませんでした: {error}"))?;
    let encrypted = encrypt_windows_dpapi_blob(&payload)?;
    fs::write(&path, encrypted)
        .map_err(|error| format!("{} を保存できませんでした: {error}", path.display()))
}

pub(super) fn read_secure_launch_token(
    account: Option<&crate::minecraft::LauncherAccount>,
) -> Option<SecureLaunchToken> {
    let keys = secure_launch_token_cache_keys(account);
    if keys.is_empty() {
        return None;
    }

    let cache = match load_secure_launch_token_cache() {
        Ok(cache) => cache,
        Err(error) => {
            app_log::append_log(
                "WARN",
                format!("failed to read secure launch token cache: {error}"),
            );
            return None;
        }
    };

    for key in keys {
        let Some(entry) = cache.entries.get(&key) else {
            continue;
        };
        return Some(SecureLaunchToken {
            access_token: entry.access_token.clone(),
            expires_at: entry.expires_at.clone(),
            username: entry.username.clone(),
            uuid: normalize_uuid_value(&entry.uuid),
            xuid: entry.xuid.clone().unwrap_or_default(),
            user_properties: entry
                .user_properties
                .clone()
                .unwrap_or_else(|| "{}".to_string()),
            user_type: entry.user_type.clone().unwrap_or_else(|| "msa".to_string()),
        });
    }

    None
}

pub(super) fn persist_secure_launch_token(
    account: Option<&crate::minecraft::LauncherAccount>,
    token: &SecureLaunchToken,
) -> Result<(), String> {
    let keys = secure_launch_token_cache_keys(account);
    if keys.is_empty() {
        return Ok(());
    }

    let mut cache =
        load_secure_launch_token_cache().unwrap_or_else(|_| SecureLaunchTokenCacheFile {
            version: 1,
            entries: HashMap::new(),
        });
    cache.version = 1;

    let local_id = account
        .and_then(|account| account.local_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let xuid = account
        .and_then(|account| account.xuid.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| Some(token.xuid.trim().to_string()).filter(|value| !value.is_empty()));
    let entry = SecureLaunchTokenCacheEntry {
        access_token: token.access_token.clone(),
        expires_at: token.expires_at.clone(),
        username: token.username.clone(),
        uuid: normalize_uuid_value(&token.uuid),
        xuid,
        local_id,
        user_properties: Some(token.user_properties.clone()),
        user_type: Some(token.user_type.clone()),
        saved_at: chrono::Utc::now().to_rfc3339(),
    };

    for key in keys {
        cache.entries.insert(key, entry.clone());
    }

    save_secure_launch_token_cache(&cache)
}

pub(super) fn clear_secure_launch_token(account: Option<&crate::minecraft::LauncherAccount>) {
    let keys = secure_launch_token_cache_keys(account);
    if keys.is_empty() {
        return;
    }

    let mut cache = match load_secure_launch_token_cache() {
        Ok(cache) => cache,
        Err(error) => {
            app_log::append_log(
                "WARN",
                format!("failed to clear secure launch token cache: {error}"),
            );
            return;
        }
    };
    let mut changed = false;
    for key in keys {
        changed |= cache.entries.remove(&key).is_some();
    }
    if !changed {
        return;
    }

    if let Err(error) = save_secure_launch_token_cache(&cache) {
        app_log::append_log(
            "WARN",
            format!("failed to save secure launch token cache: {error}"),
        );
    }
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

fn encrypt_windows_dpapi_blob(plain: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(target_os = "windows")]
    {
        use std::{ptr, slice};
        use windows_sys::Win32::{
            Foundation::LocalFree,
            Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB},
        };

        unsafe {
            let input = CRYPT_INTEGER_BLOB {
                cbData: plain.len() as u32,
                pbData: plain.as_ptr() as *mut u8,
            };
            let mut output = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: ptr::null_mut(),
            };

            let success = CryptProtectData(
                &input,
                ptr::null(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                &mut output,
            );
            if success == 0 {
                return Err("Windows の保護トークンを暗号化できませんでした。".to_string());
            }

            let bytes = slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = LocalFree(output.pbData as *mut core::ffi::c_void);
            Ok(bytes)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = plain;
        Err("Windows 以外では認証キャッシュ暗号化に対応していません。".to_string())
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
                    tokio::time::sleep(Duration::from_millis(retry_wait_millis(attempt))).await;
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
            tokio::time::sleep(Duration::from_millis(wait_ms)).await;
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
                    tokio::time::sleep(Duration::from_millis(retry_wait_millis(attempt))).await;
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
            tokio::time::sleep(Duration::from_millis(retry_wait_millis(attempt))).await;
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
    fn matches_launcher_account_from_xbox_identity_when_profile_lookup_is_missing() {
        let accounts = vec![crate::minecraft::LauncherAccount {
            gamer_tag: Some("PEXkoukunn".to_string()),
            xuid: Some("2535457379922907".to_string()),
            local_id: Some("saved-local-id".to_string()),
            ..Default::default()
        }];
        let xbox_identity = XboxProfileIdentity {
            gamer_tag: Some("PEXkoukunn".to_string()),
            xuid: Some("2535457379922907".to_string()),
        };

        let matched = match_local_launcher_account_with_xbox_identity(
            &accounts,
            None,
            None,
            Some(&xbox_identity),
        )
        .expect("expected launcher account to match xbox identity");

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
    fn scores_cached_sources_for_selected_account() {
        let preferred_account = crate::minecraft::LauncherAccount {
            username: Some("pexkurann@gmail.com".to_string()),
            gamer_tag: Some("PEXkoukunn".to_string()),
            local_id: Some("00037FFE1DB0472E".to_string()),
            xuid: Some("2535457379922907".to_string()),
            ..Default::default()
        };
        let matching_source = PathBuf::from("C:\\cache\\matching.tbres");
        let other_source = PathBuf::from("C:\\cache\\other.tbres");
        let hints = vec![
            CachedXboxAccountHint {
                username: Some("pexkurann@gmail.com".to_string()),
                gamer_tag: Some("PEXkoukunn".to_string()),
                local_id: Some("00037FFE1DB0472E".to_string()),
                xuid: Some("2535457379922907".to_string()),
                display_name: Some("PEXkoukunn".to_string()),
                source_path: matching_source.clone(),
            },
            CachedXboxAccountHint {
                username: Some("isseidas@gmail.com".to_string()),
                gamer_tag: Some("PC My".to_string()),
                local_id: Some("00037FFE1DB0472E79".to_string()),
                xuid: Some("2535457379922000".to_string()),
                display_name: Some("PC My".to_string()),
                source_path: other_source.clone(),
            },
        ];

        let scores =
            preferred_cached_xbox_source_scores_from_hints(Some(&preferred_account), &hints);

        assert!(scores.get(&matching_source).copied().unwrap_or_default() > 0);
        assert!(!scores.contains_key(&other_source));
    }

    #[test]
    fn matches_account_identities_case_insensitively() {
        assert!(account_identity_matches(
            Some("PEXKURANN@GMAIL.COM"),
            Some("pexkurann@gmail.com"),
        ));
        assert!(account_identity_matches(
            Some("C53F907D-0AD2-42C6-99C3-3994A3C1CAA4"),
            Some("c53f907d0ad242c699c33994a3c1caa4"),
        ));
        assert!(!account_identity_matches(
            Some("isseidas@gmail.com"),
            Some("pexkurann@gmail.com"),
        ));
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
