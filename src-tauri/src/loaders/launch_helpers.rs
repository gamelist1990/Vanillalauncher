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
    replacements.insert("user_type".to_string(), "msa".to_string());
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
    app_log::append_log(
        "INFO",
        format!(
            "resolve_launch_auth account_present={} launcher_access_token_present={}",
            account.is_some(),
            account
                .as_ref()
                .and_then(|item| item.access_token.as_ref())
                .is_some()
        ),
    );
    let mut username = account
        .as_ref()
        .and_then(|account| account.gamer_tag.clone())
        .or_else(|| {
            account
                .as_ref()
                .and_then(|account| account.username.clone())
        })
        .unwrap_or_else(|| "Player".to_string());
    let raw_access_token = account
        .as_ref()
        .and_then(|account| account.access_token.clone())
        .unwrap_or_else(|| "0".to_string());
    let expires_at = account
        .as_ref()
        .and_then(|account| account.access_token_expires_at.as_deref());

    let launcher_profile = if raw_access_token != "0"
        && !xbox_auth::is_access_token_expired(&raw_access_token, expires_at)
    {
        app_log::append_log("INFO", "trying launcher_accounts access token");
        xbox_auth::fetch_minecraft_profile_for_token(&raw_access_token).await
    } else {
        app_log::append_log(
            "INFO",
            "launcher_accounts access token missing or expired; trying Xbox cache",
        );
        None
    };

    let cached_xbox_tokens = if launcher_profile.is_none() {
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
        let mut resolved_access_token: Option<String> = None;
        let mut resolved_profile: Option<(String, String)> = None;
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
                app_log::append_log("INFO", "Xbox token exchanged via /launcher/login");
                xbox_auth::persist_xbox_rps_success_state(
                    &xbox_token,
                    &variant_label,
                    &candidate_token,
                );
                resolved_profile = xbox_auth::fetch_minecraft_profile_for_token(&access_token).await;
                resolved_access_token = Some(access_token);
                break;
            }

            std::thread::sleep(Duration::from_millis(180));
        }
        if resolved_access_token.is_none() {
            app_log::append_log("WARN", "all cached Xbox token exchanges failed");
        }
        resolved_access_token.map(|token| (token, resolved_profile))
    } else {
        None
    };

    let (access_token, verified_profile, mode) = if let Some(profile) = launcher_profile {
        (
            raw_access_token,
            Some(profile),
            "direct-account".to_string(),
        )
    } else if let Some((token, profile)) = xbox_profile {
        (token, profile, "direct-xbox-cache".to_string())
    } else {
        ("0".to_string(), None, "direct-runtime".to_string())
    };

    let client_id = account
        .as_ref()
        .and_then(|account| account.local_id.clone())
        .or_else(|| {
            account
                .as_ref()
                .and_then(|account| account.client_token.clone())
        })
        .unwrap_or_else(|| "vanillalauncher".to_string());
    let xuid = account
        .as_ref()
        .and_then(|account| account.xuid.clone())
        .unwrap_or_else(|| "0".to_string());
    let uuid = verified_profile
        .as_ref()
        .map(|profile| xbox_auth::normalize_uuid_value(&profile.0))
        .or_else(|| {
            account
                .as_ref()
                .and_then(|account| account.profile_id.clone())
                .map(|value| xbox_auth::normalize_uuid_value(&value))
        })
        .or_else(|| {
            account
                .as_ref()
                .and_then(|account| account.access_token.as_ref())
                .and_then(|token| xbox_auth::uuid_from_access_token(token))
        })
        .or_else(|| {
            account
                .as_ref()
                .and_then(|account| account.local_id.clone())
                .map(|value| xbox_auth::normalize_uuid_value(&value))
        })
        .unwrap_or_else(|| "00000000000000000000000000000000".to_string());

    if let Some(profile) = verified_profile.as_ref() {
        if !profile.1.trim().is_empty() {
            username = profile.1.clone();
        }
    }
    let user_properties = account
        .as_ref()
        .and_then(|account| account.user_properties.clone())
        .unwrap_or_else(|| "{}".to_string());

    Ok(VersionLaunchAuth {
        username,
        uuid,
        access_token,
        client_id,
        xuid,
        user_properties,
        mode,
    })
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
