use super::*;

pub(super) fn build_launch_arguments(
    profile: &crate::models::LauncherProfile,
    version_id: &str,
    manifest: &MergedVersionManifest,
    auth: &VersionLaunchAuth,
    minecraft_root: &Path,
    native_dir: &Path,
    classpath: &str,
) -> Result<Vec<String>, String> {
    let features = launch_feature_flags();
    let mut replacements = HashMap::new();
    replacements.insert("auth_player_name".to_string(), auth.username.clone());
    replacements.insert("version_name".to_string(), version_id.to_string());
    replacements.insert("game_directory".to_string(), profile.game_dir.clone());
    replacements.insert(
        "assets_root".to_string(),
        minecraft_root.join("assets").to_string_lossy().to_string(),
    );
    replacements.insert(
        "assets_index_name".to_string(),
        manifest.asset_index_name.clone(),
    );
    replacements.insert("auth_uuid".to_string(), auth.uuid.clone());
    replacements.insert("auth_access_token".to_string(), auth.access_token.clone());
    replacements.insert("clientid".to_string(), auth.client_id.clone());
    replacements.insert("auth_xuid".to_string(), auth.xuid.clone());
    replacements.insert("user_properties".to_string(), auth.user_properties.clone());
    replacements.insert("profile_properties".to_string(), "{}".to_string());
    replacements.insert("version_type".to_string(), manifest.version_type.clone());
    replacements.insert(
        "natives_directory".to_string(),
        native_dir.to_string_lossy().to_string(),
    );
    replacements.insert("launcher_name".to_string(), "vanillalauncher".to_string());
    replacements.insert(
        "launcher_version".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    );
    replacements.insert("classpath".to_string(), classpath.to_string());
    replacements.insert(
        "library_directory".to_string(),
        minecraft_root
            .join("libraries")
            .to_string_lossy()
            .to_string(),
    );
    replacements.insert("classpath_separator".to_string(), ";".to_string());
    replacements.insert("resolution_width".to_string(), "1280".to_string());
    replacements.insert("resolution_height".to_string(), "720".to_string());
    replacements.insert(
        "quickPlayPath".to_string(),
        minecraft_root
            .join("quickplay")
            .join("vanillalauncher.json")
            .to_string_lossy()
            .to_string(),
    );
    replacements.insert("user_type".to_string(), auth.user_type.clone());
    replacements.insert("auth_session".to_string(), auth.access_token.clone());

    let mut arguments = Vec::new();

    for argument in collect_argument_values(&manifest.jvm_arguments, &features) {
        arguments.push(replace_placeholders(&argument, &replacements));
    }

    if let Some(logging_argument) = manifest.logging_argument.as_deref() {
        arguments.push(replace_placeholders(logging_argument, &replacements));
    }

    arguments.push(manifest.main_class.clone());

    let game_arguments = if manifest.game_arguments.is_empty() {
        manifest
            .legacy_game_arguments
            .as_deref()
            .map(split_legacy_arguments)
            .unwrap_or_default()
    } else {
        collect_argument_values(&manifest.game_arguments, &features)
    };

    for argument in game_arguments {
        arguments.push(replace_placeholders(&argument, &replacements));
    }

    if arguments.is_empty() {
        return Err("起動引数を生成できませんでした。".to_string());
    }

    Ok(arguments)
}

pub(super) fn build_classpath(
    libraries: &[Value],
    version_jars: &[PathBuf],
    minecraft_root: &Path,
) -> Result<String, String> {
    let features = launch_feature_flags();
    let mut entries = Vec::new();

    for library in libraries {
        if !rule_set_allows(library.get("rules"), &features) {
            continue;
        }

        if let Some(artifact) = resolve_library_artifact_download(library, minecraft_root) {
            if artifact.path.exists() {
                let item = artifact.path.to_string_lossy().to_string();
                if !entries.contains(&item) {
                    entries.push(item);
                }
            }
        }
    }

    for version_jar in version_jars {
        if version_jar.exists() {
            entries.push(version_jar.to_string_lossy().to_string());
        }
    }

    if version_jars.is_empty() {
        return Err(
            "version jar が見つかりません。先に対象バージョンを導入してください。".to_string(),
        );
    }

    Ok(entries.join(";"))
}

pub(super) fn prepare_native_directory(
    minecraft_root: &Path,
    version_id: &str,
    libraries: &[Value],
) -> Result<PathBuf, String> {
    let features = launch_feature_flags();
    let native_dir = minecraft_root
        .join("bin")
        .join(format!("vanillalauncher-{version_id}"));

    if native_dir.exists() {
        let _ = fs::remove_dir_all(&native_dir);
    }
    fs::create_dir_all(&native_dir)
        .map_err(|error| format!("{} を準備できませんでした: {error}", native_dir.display()))?;

    for library in libraries {
        if !rule_set_allows(library.get("rules"), &features) {
            continue;
        }
        let Some(download) = resolve_library_native_download(library, minecraft_root) else {
            continue;
        };
        if !download.path.exists() {
            continue;
        }
        extract_native_archive(&download.path, &native_dir)?;
    }

    Ok(native_dir)
}

pub(super) fn extract_native_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = File::open(archive_path)
        .map_err(|error| format!("{} を開けませんでした: {error}", archive_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("{} を展開できませんでした: {error}", archive_path.display()))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("native ファイルを読み取れませんでした: {error}"))?;
        let entry_name = entry.name().replace('\\', "/");
        if entry.is_dir() || entry_name.starts_with("META-INF/") || entry_name.ends_with(".sha1") {
            continue;
        }

        let output_path = target_dir.join(&entry_name);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
        }

        let mut output = File::create(&output_path).map_err(|error| {
            format!("{} を作成できませんでした: {error}", output_path.display())
        })?;
        std::io::copy(&mut entry, &mut output).map_err(|error| {
            format!("{} を書き込めませんでした: {error}", output_path.display())
        })?;
        output.flush().map_err(|error| {
            format!("{} を保存できませんでした: {error}", output_path.display())
        })?;
    }

    Ok(())
}

pub(super) async fn resolve_launch_auth() -> Result<VersionLaunchAuth, String> {
    let account = read_active_launcher_account()?;
    let launcher_accounts = read_launcher_accounts()?;
    let java_access_hints = xbox_auth::read_local_launcher_java_access_hints();
    app_log::append_log(
        "INFO",
        format!(
            "resolve_launch_auth account_present={} launcher_accounts={} launcher_access_token_present={}",
            account.is_some(),
            launcher_accounts.len(),
            account
                .as_ref()
                .and_then(|item| item.access_token.as_ref())
                .is_some()
        ),
    );
    let raw_access_token = account
        .as_ref()
        .and_then(|account| account.access_token.clone())
        .unwrap_or_else(|| "0".to_string());
    let expires_at = account
        .as_ref()
        .and_then(|account| account.access_token_expires_at.as_deref());
    let active_account_profile = account
        .as_ref()
        .and_then(xbox_auth::launcher_account_profile);

    let direct_account_auth = if raw_access_token != "0"
        && !xbox_auth::is_access_token_expired(&raw_access_token, expires_at)
    {
        app_log::append_log("INFO", "trying launcher_accounts access token");
        if let Some((profile, matched_account, used_local_hint)) =
            xbox_auth::resolve_verified_minecraft_profile(
                &raw_access_token,
                "launcher_accounts access token",
                &launcher_accounts,
                &java_access_hints,
            )
            .await
        {
            let resolved_account = matched_account.or_else(|| account.clone());
            if launch_auth_matches_selected_account(
                account.as_ref(),
                Some(&profile),
                resolved_account.as_ref(),
            ) {
                Some((
                    raw_access_token.clone(),
                    profile,
                    resolved_account,
                    if used_local_hint {
                        "direct-account-local".to_string()
                    } else {
                        "direct-account".to_string()
                    },
                ))
            } else {
                app_log::append_log(
                    "WARN",
                    "launcher_accounts access token resolved to a different account; ignoring",
                );
                None
            }
        } else if let Some(active_account) = account.clone().filter(|entry| {
            xbox_auth::launcher_account_has_java_access_hint(entry, &java_access_hints)
        }) {
            active_account_profile.clone().map(|profile| {
                app_log::append_log(
                    "INFO",
                    format!(
                        "launcher_accounts access token fell back to local entitlement hint xuid={}",
                        active_account.xuid.as_deref().unwrap_or("unknown")
                    ),
                );
                (
                    raw_access_token.clone(),
                    profile,
                    Some(active_account),
                    "direct-account-local".to_string(),
                )
            })
        } else {
            None
        }
    } else {
        app_log::append_log(
            "INFO",
            "launcher_accounts access token missing or expired; trying Xbox cache",
        );
        None
    };

    let cached_xbox_tokens = if direct_account_auth.is_none() {
        match xbox_auth::read_cached_xbox_identity_tokens() {
            Ok(tokens) => {
                if tokens.is_empty() {
                    app_log::append_log("WARN", "no usable cached Xbox token found");
                } else {
                    app_log::append_log(
                        "INFO",
                        format!("found {} cached Xbox token candidates", tokens.len()),
                    );
                }
                tokens
            }
            Err(error) => {
                app_log::append_log(
                    "ERROR",
                    format!("read_cached_xbox_identity_tokens failed: {error}"),
                );
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    let xbox_profile = if !cached_xbox_tokens.is_empty() {
        let mut resolved_auth: Option<(
            String,
            (String, String),
            Option<crate::minecraft::LauncherAccount>,
            String,
        )> = None;
        let mut attempted_tokens = HashSet::new();
        let (planned_attempts, used_saved_state) =
            xbox_auth::build_prioritized_rps_attempts(&cached_xbox_tokens, 10);
        app_log::append_log(
            "INFO",
            format!(
                "prepared {} prioritized xbox-rps attempts (saved-state-priority={})",
                planned_attempts.len(),
                used_saved_state
            ),
        );

        for (variant_label, candidate_token, xbox_token) in planned_attempts {
            if !attempted_tokens.insert(candidate_token.clone()) {
                continue;
            }
            app_log::append_log(
                "INFO",
                format!(
                    "trying cached Xbox token source={} scope={} variant={} preview={}",
                    xbox_token.source_path.display(),
                    xbox_token.scope.as_deref().unwrap_or("unknown"),
                    variant_label,
                    xbox_auth::preview_token(&candidate_token)
                ),
            );
            let context = format!("{} [{}]", xbox_token.source_path.display(), variant_label);
            let exchange_result = xbox_auth::exchange_rps_ticket_for_minecraft_access_token(
                &candidate_token,
                &context,
            )
            .await;
            if let Some(access_token) = exchange_result {
                let verification_context = format!("{context} -> minecraft/profile");
                if let Some((profile, matched_account, used_local_hint)) =
                    xbox_auth::resolve_verified_minecraft_profile(
                        &access_token,
                        &verification_context,
                        &launcher_accounts,
                        &java_access_hints,
                    )
                    .await
                {
                    if !launch_auth_matches_selected_account(
                        account.as_ref(),
                        Some(&profile),
                        matched_account.as_ref(),
                    ) {
                        app_log::append_log(
                            "WARN",
                            format!(
                                "cached Xbox token resolved to another account; skipping source={} variant={}",
                                xbox_token.source_path.display(),
                                variant_label
                            ),
                        );
                        std::thread::sleep(Duration::from_millis(180));
                        continue;
                    }
                    app_log::append_log("INFO", "Xbox token exchanged via /launcher/login");
                    xbox_auth::persist_xbox_rps_success_state(
                        &xbox_token,
                        &variant_label,
                        &candidate_token,
                    );
                    resolved_auth = Some((
                        access_token,
                        profile,
                        matched_account,
                        if used_local_hint {
                            "direct-xbox-cache-local".to_string()
                        } else {
                            "direct-xbox-cache".to_string()
                        },
                    ));
                    break;
                }

                app_log::append_log(
                    "WARN",
                    format!(
                        "cached Xbox token exchanged but Minecraft Java access could not be verified source={} variant={}",
                        xbox_token.source_path.display(),
                        variant_label
                    ),
                );
            }

            std::thread::sleep(Duration::from_millis(180));
        }
        if resolved_auth.is_none() {
            app_log::append_log("WARN", "all cached Xbox token exchanges failed");
        }
        resolved_auth
    } else {
        None
    };

    let (access_token, verified_profile, resolved_account, mode) =
        if let Some((token, profile, matched_account, mode)) = direct_account_auth {
            (token, Some(profile), matched_account, mode)
        } else if let Some((token, profile, matched_account, mode)) = xbox_profile {
            (token, Some(profile), matched_account, mode)
        } else if account.is_some() {
            app_log::append_log(
                "INFO",
                "using selected launcher account as offline fallback",
            );
            (
                "0".to_string(),
                None,
                account.clone(),
                "direct-offline-selected".to_string(),
            )
        } else {
            ("0".to_string(), None, None, "direct-runtime".to_string())
        };

    let metadata_account =
        if mode.starts_with("direct-account") || mode == "direct-offline-selected" {
            account.as_ref().or(resolved_account.as_ref())
        } else {
            resolved_account.as_ref()
        };

    let client_id = metadata_account
        .and_then(|account| account.local_id.clone())
        .or_else(|| metadata_account.and_then(|account| account.client_token.clone()))
        .unwrap_or_else(|| "vanillalauncher".to_string());
    let xuid = metadata_account
        .and_then(|account| account.xuid.clone())
        .unwrap_or_else(|| "0".to_string());
    let uuid = verified_profile
        .as_ref()
        .map(|profile| profile.0.clone())
        .or_else(|| {
            metadata_account
                .and_then(|account| account.profile_id.clone())
                .map(|value| xbox_auth::normalize_uuid_value(&value))
        })
        .or_else(|| xbox_auth::uuid_from_access_token(&access_token))
        .or_else(|| {
            metadata_account
                .and_then(|account| account.local_id.clone())
                .map(|value| xbox_auth::normalize_uuid_value(&value))
        })
        .unwrap_or_else(|| "00000000000000000000000000000000".to_string());

    let username = verified_profile
        .as_ref()
        .map(|profile| profile.1.clone())
        .or_else(|| {
            metadata_account.and_then(crate::minecraft::preferred_launcher_account_display_name)
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Player".to_string());
    let user_properties = metadata_account
        .and_then(|account| account.user_properties.clone())
        .unwrap_or_else(|| "{}".to_string());

    if mode == "direct-offline-selected" {
        let offline_username = offline_username_for_account(metadata_account.or(account.as_ref()));
        let offline_uuid = offline_uuid_for_username(&offline_username);
        app_log::append_log(
            "INFO",
            format!(
                "offline fallback identity username={} uuid={}",
                offline_username, offline_uuid
            ),
        );
        return Ok(VersionLaunchAuth {
            username: offline_username,
            uuid: offline_uuid,
            access_token: "0".to_string(),
            client_id,
            xuid: "0".to_string(),
            user_properties: "{}".to_string(),
            user_type: "legacy".to_string(),
            mode,
        });
    }

    Ok(VersionLaunchAuth {
        username,
        uuid,
        access_token,
        client_id,
        xuid,
        user_properties,
        user_type: "msa".to_string(),
        mode,
    })
}

fn launch_auth_matches_selected_account(
    selected_account: Option<&crate::minecraft::LauncherAccount>,
    verified_profile: Option<&(String, String)>,
    resolved_account: Option<&crate::minecraft::LauncherAccount>,
) -> bool {
    let Some(selected_account) = selected_account else {
        return true;
    };

    if let Some(resolved_account) = resolved_account {
        if launcher_accounts_share_identity(selected_account, resolved_account) {
            return true;
        }
    }

    let Some((profile_id, profile_name)) = verified_profile else {
        return false;
    };
    let normalized_profile_id = xbox_auth::normalize_uuid_value(profile_id);
    let normalized_profile_name = profile_name.trim();

    selected_account
        .profile_id
        .as_deref()
        .map(xbox_auth::normalize_uuid_value)
        .is_some_and(|candidate| candidate == normalized_profile_id)
        || selected_account
            .gamer_tag
            .as_deref()
            .map(str::trim)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(normalized_profile_name))
}

fn launcher_accounts_share_identity(
    left: &crate::minecraft::LauncherAccount,
    right: &crate::minecraft::LauncherAccount,
) -> bool {
    let right_keys = launcher_account_identity_keys(right);
    launcher_account_identity_keys(left)
        .into_iter()
        .any(|key| right_keys.contains(&key))
}

fn launcher_account_identity_keys(account: &crate::minecraft::LauncherAccount) -> Vec<String> {
    let mut keys = Vec::new();

    for value in [
        account.local_id.as_deref(),
        account.profile_id.as_deref(),
        account.xuid.as_deref(),
        account.username.as_deref(),
        account.gamer_tag.as_deref(),
    ] {
        let Some(value) = value.map(str::trim).filter(|entry| !entry.is_empty()) else {
            continue;
        };
        let normalized = if value.contains('-') && value.len() >= 32 {
            xbox_auth::normalize_uuid_value(value)
        } else {
            value.to_ascii_lowercase()
        };
        if !keys.contains(&normalized) {
            keys.push(normalized);
        }
    }

    keys
}

fn offline_username_for_account(account: Option<&crate::minecraft::LauncherAccount>) -> String {
    let fallback_local_id = account.and_then(|entry| entry.local_id.as_deref());
    let preferred_name =
        account.and_then(crate::minecraft::preferred_launcher_account_display_name);
    let preferred_name_ref = preferred_name.as_deref();
    let candidates = [preferred_name_ref, fallback_local_id];

    for candidate in candidates.into_iter().flatten() {
        let sanitized = sanitize_offline_username(candidate);
        if !sanitized.is_empty() {
            return sanitized;
        }
    }

    let suffix = fallback_local_id
        .map(|value| {
            value
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .take(6)
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "0000".to_string());
    sanitize_offline_username(&format!("Player{suffix}"))
}

fn sanitize_offline_username(value: &str) -> String {
    let mut sanitized = String::new();
    let mut last_was_separator = false;

    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            sanitized.push(character);
            last_was_separator = false;
        } else if (character.is_ascii_whitespace() || character == '-' || character == '.')
            && !sanitized.is_empty()
            && !last_was_separator
        {
            sanitized.push('_');
            last_was_separator = true;
        }

        if sanitized.len() >= 16 {
            break;
        }
    }

    while sanitized.ends_with('_') {
        sanitized.pop();
    }

    if sanitized.len() >= 3 {
        sanitized
    } else {
        let mut fallback = "Player".to_string();
        if !sanitized.is_empty() {
            fallback.push('_');
            fallback.push_str(&sanitized);
        }
        fallback.chars().take(16).collect()
    }
}

fn offline_uuid_for_username(username: &str) -> String {
    let mut digest = md5::compute(format!("OfflinePlayer:{username}")).0;
    digest[6] = (digest[6] & 0x0f) | 0x30;
    digest[8] = (digest[8] & 0x3f) | 0x80;

    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

pub(super) fn fabric_client() -> Result<Client, String> {
    loader_http_client()
}

pub(super) fn loader_http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(FABRIC_USER_AGENT)
        .build()
        .map_err(|error| format!("HTTP クライアントを作成できませんでした: {error}"))
}

pub(super) fn mojang_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(MOJANG_USER_AGENT)
        .build()
        .map_err(|error| format!("HTTP クライアントを作成できませんでした: {error}"))
}

pub(super) async fn fetch_official_version_manifest() -> Result<MojangVersionManifest, String> {
    mojang_client()?
        .get(MOJANG_VERSION_MANIFEST_URL)
        .send()
        .await
        .map_err(|error| format!("Minecraft バージョン一覧を取得できませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Minecraft バージョン一覧の取得に失敗しました: {error}"))?
        .json::<MojangVersionManifest>()
        .await
        .map_err(|error| format!("Minecraft バージョン一覧を解析できませんでした: {error}"))
}

pub(super) async fn download_to_path(
    client: &Client,
    url: &str,
    path: &Path,
) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }

    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("{url} を取得できませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("{url} の取得に失敗しました: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("{url} の内容を読み取れませんでした: {error}"))?;

    fs::write(path, &bytes)
        .map_err(|error| format!("{} を保存できませんでした: {error}", path.display()))
}

pub(super) fn read_json_file(path: &Path) -> Result<Value, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("{} を解析できませんでした: {error}", path.display()))
}

pub(super) fn version_json_path(minecraft_root: &Path, version_id: &str) -> PathBuf {
    minecraft_root
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.json"))
}

pub(super) fn version_jar_path(minecraft_root: &Path, version_id: &str) -> PathBuf {
    minecraft_root
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.jar"))
}

pub(super) fn launch_feature_flags() -> HashMap<&'static str, bool> {
    HashMap::from([
        ("is_demo_user", false),
        ("has_custom_resolution", true),
        ("has_quick_plays_support", false),
        ("is_quick_play_singleplayer", false),
        ("is_quick_play_multiplayer", false),
        ("is_quick_play_realms", false),
    ])
}

pub(super) fn collect_argument_values(
    values: &[Value],
    features: &HashMap<&'static str, bool>,
) -> Vec<String> {
    values
        .iter()
        .flat_map(|value| match value {
            Value::String(text) => vec![text.to_string()],
            Value::Object(object) if rule_set_allows(object.get("rules"), features) => {
                match object.get("value") {
                    Some(Value::String(text)) => vec![text.to_string()],
                    Some(Value::Array(items)) => items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect(),
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        })
        .collect()
}

pub(super) fn split_legacy_arguments(value: &str) -> Vec<String> {
    value.split_whitespace().map(str::to_string).collect()
}

pub(super) fn replace_placeholders(value: &str, replacements: &HashMap<String, String>) -> String {
    replacements
        .iter()
        .fold(value.to_string(), |current, (key, replacement)| {
            current.replace(&format!("${{{key}}}"), replacement)
        })
}

pub(super) fn rule_set_allows(
    rules_value: Option<&Value>,
    features: &HashMap<&'static str, bool>,
) -> bool {
    let Some(rules) = rules_value.and_then(Value::as_array) else {
        return true;
    };

    let mut allowed = false;

    for rule in rules {
        let Some(object) = rule.as_object() else {
            continue;
        };
        if !rule_matches_environment(object, features) {
            continue;
        }

        allowed = object
            .get("action")
            .and_then(Value::as_str)
            .map(|action| action == "allow")
            .unwrap_or(true);
    }

    allowed
}

pub(super) fn rule_matches_environment(
    rule: &serde_json::Map<String, Value>,
    features: &HashMap<&'static str, bool>,
) -> bool {
    if let Some(os) = rule.get("os").and_then(Value::as_object) {
        if let Some(name) = os.get("name").and_then(Value::as_str) {
            let current_os = if cfg!(target_os = "windows") {
                "windows"
            } else if cfg!(target_os = "macos") {
                "osx"
            } else {
                "linux"
            };
            if name != current_os {
                return false;
            }
        }
        if let Some(arch) = os.get("arch").and_then(Value::as_str) {
            let current_arch = if cfg!(target_pointer_width = "32") {
                "x86"
            } else {
                "x86_64"
            };
            if arch != current_arch {
                return false;
            }
        }
    }

    if let Some(required_features) = rule.get("features").and_then(Value::as_object) {
        for (key, expected) in required_features {
            let expected_value = expected.as_bool().unwrap_or(false);
            if features.get(key.as_str()).copied().unwrap_or(false) != expected_value {
                return false;
            }
        }
    }

    true
}

pub(super) fn resolve_library_artifact_download(
    library: &Value,
    minecraft_root: &Path,
) -> Option<VersionDownload> {
    if let Some(artifact) = library
        .get("downloads")
        .and_then(|value| value.get("artifact"))
        .and_then(Value::as_object)
    {
        let path = artifact.get("path").and_then(Value::as_str)?;
        let url = artifact.get("url").and_then(Value::as_str)?;
        return Some(VersionDownload {
            url: url.to_string(),
            path: minecraft_root.join("libraries").join(path),
        });
    }

    let base_url = library.get("url").and_then(Value::as_str)?;
    let library_name = library.get("name").and_then(Value::as_str)?;
    let path = maven_library_path(library_name, None)?;
    Some(VersionDownload {
        url: format!("{base_url}{path}"),
        path: minecraft_root.join("libraries").join(path),
    })
}

pub(super) fn resolve_library_native_download(
    library: &Value,
    minecraft_root: &Path,
) -> Option<VersionDownload> {
    let classifier_name = resolve_native_classifier_name(library)?;
    if let Some(classifier) = library
        .get("downloads")
        .and_then(|value| value.get("classifiers"))
        .and_then(|value| value.get(&classifier_name))
        .and_then(Value::as_object)
    {
        let path = classifier.get("path").and_then(Value::as_str)?;
        let url = classifier.get("url").and_then(Value::as_str)?;
        return Some(VersionDownload {
            url: url.to_string(),
            path: minecraft_root.join("libraries").join(path),
        });
    }

    let base_url = library.get("url").and_then(Value::as_str)?;
    let library_name = library.get("name").and_then(Value::as_str)?;
    let path = maven_library_path(library_name, Some(&classifier_name))?;
    Some(VersionDownload {
        url: format!("{base_url}{path}"),
        path: minecraft_root.join("libraries").join(path),
    })
}

pub(super) fn resolve_native_classifier_name(library: &Value) -> Option<String> {
    let natives = library.get("natives")?.as_object()?;
    let raw = natives
        .get("windows")
        .and_then(Value::as_str)
        .or_else(|| natives.values().find_map(Value::as_str))?;
    let arch = if cfg!(target_pointer_width = "32") {
        "32"
    } else {
        "64"
    };
    Some(raw.replace("${arch}", arch))
}

pub(super) fn maven_library_path(coordinates: &str, classifier: Option<&str>) -> Option<String> {
    let mut parts = coordinates.split(':');
    let group = parts.next()?;
    let artifact = parts.next()?;
    let version = parts.next()?;
    let extension = parts
        .next()
        .and_then(|part| part.strip_prefix('@'))
        .unwrap_or("jar");

    let file_name = if let Some(classifier) = classifier {
        format!("{artifact}-{version}-{classifier}.{extension}")
    } else {
        format!("{artifact}-{version}.{extension}")
    };

    Some(format!(
        "{}/{}/{}/{}",
        group.replace('.', "/"),
        artifact,
        version,
        file_name
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        launch_auth_matches_selected_account, offline_username_for_account,
        offline_uuid_for_username,
    };
    use crate::minecraft::LauncherAccount;

    fn launcher_account(
        local_id: &str,
        username: &str,
        gamer_tag: Option<&str>,
        profile_id: Option<&str>,
        xuid: Option<&str>,
    ) -> LauncherAccount {
        LauncherAccount {
            username: Some(username.to_string()),
            gamer_tag: gamer_tag.map(str::to_string),
            profile_id: profile_id.map(str::to_string),
            access_token: None,
            access_token_expires_at: None,
            client_token: None,
            xuid: xuid.map(str::to_string),
            local_id: Some(local_id.to_string()),
            user_properties: None,
            xbox_profile_verified: false,
        }
    }

    #[test]
    fn selected_account_accepts_matching_resolved_account() {
        let selected = launcher_account(
            "selected-local",
            "kurann@example.com",
            Some("Kurann PEX"),
            Some("11112222333344445555666677778888"),
            Some("42"),
        );
        let resolved = launcher_account(
            "selected-local",
            "kurann@example.com",
            Some("Kurann PEX"),
            Some("11112222333344445555666677778888"),
            Some("42"),
        );

        assert!(launch_auth_matches_selected_account(
            Some(&selected),
            Some(&(
                "11112222333344445555666677778888".to_string(),
                "Kurann PEX".to_string()
            )),
            Some(&resolved),
        ));
    }

    #[test]
    fn selected_account_rejects_other_resolved_account() {
        let selected = launcher_account(
            "selected-local",
            "kurann@example.com",
            Some("Kurann PEX"),
            Some("11112222333344445555666677778888"),
            Some("42"),
        );
        let resolved = launcher_account(
            "other-local",
            "pexkoukunn@example.com",
            Some("PEXkoukunn"),
            Some("aaaaaaaa111122223333444455556666"),
            Some("84"),
        );

        assert!(!launch_auth_matches_selected_account(
            Some(&selected),
            Some(&(
                "aaaaaaaa111122223333444455556666".to_string(),
                "PEXkoukunn".to_string()
            )),
            Some(&resolved),
        ));
    }

    #[test]
    fn selected_account_accepts_matching_verified_profile_without_resolved_account() {
        let selected = launcher_account(
            "selected-local",
            "kurann@example.com",
            Some("Kurann PEX"),
            Some("11112222333344445555666677778888"),
            None,
        );

        assert!(launch_auth_matches_selected_account(
            Some(&selected),
            Some(&(
                "11112222-3333-4444-5555-666677778888".to_string(),
                "Kurann PEX".to_string()
            )),
            None,
        ));
    }

    #[test]
    fn offline_username_prefers_email_local_part_when_java_profile_is_missing() {
        let selected = launcher_account(
            "selected-local",
            "pexkurann@gmail.com",
            Some("Kurann PEX"),
            None,
            None,
        );

        assert_eq!(
            offline_username_for_account(Some(&selected)),
            "PEXkurann".to_string()
        );
    }

    #[test]
    fn offline_username_uses_gamertag_when_verified_profile_exists() {
        let selected = launcher_account(
            "selected-local",
            "pexkurann@gmail.com",
            Some("PEXkoukunn"),
            Some("11112222333344445555666677778888"),
            None,
        );

        assert_eq!(
            offline_username_for_account(Some(&selected)),
            "PEXkoukunn".to_string()
        );
    }

    #[test]
    fn offline_uuid_is_deterministic_and_hex() {
        let left = offline_uuid_for_username("Kurann_PEX");
        let right = offline_uuid_for_username("Kurann_PEX");

        assert_eq!(left, right);
        assert_eq!(left.len(), 32);
        assert!(left.chars().all(|character| character.is_ascii_hexdigit()));
    }
}
