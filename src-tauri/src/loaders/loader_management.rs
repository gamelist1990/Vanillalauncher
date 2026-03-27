use super::*;

pub async fn get_fabric_catalog(game_version: Option<String>) -> Result<FabricCatalog, String> {
    let client = fabric_client()?;
    let game_versions = client
        .get(format!("{FABRIC_META_API_BASE}/versions/game"))
        .send()
        .await
        .map_err(|error| format!("Fabric 対応バージョン一覧を取得できませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Fabric のゲームバージョン取得に失敗しました: {error}"))?
        .json::<Vec<FabricGameVersionEntry>>()
        .await
        .map_err(|error| format!("Fabric のゲームバージョン一覧を解析できませんでした: {error}"))?;

    let installers = client
        .get(format!("{FABRIC_META_API_BASE}/versions/installer"))
        .send()
        .await
        .map_err(|error| format!("Fabric Installer 一覧を取得できませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Fabric Installer の取得に失敗しました: {error}"))?
        .json::<Vec<FabricInstallerEntry>>()
        .await
        .map_err(|error| format!("Fabric Installer 一覧を解析できませんでした: {error}"))?;

    let latest_installer = installers
        .iter()
        .find(|entry| entry.stable)
        .or_else(|| installers.first())
        .ok_or_else(|| "Fabric Installer が見つかりません。".to_string())?;

    let selected_version = game_version
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            game_versions
                .iter()
                .find(|entry| entry.stable)
                .map(|entry| entry.version.clone())
        })
        .ok_or_else(|| "Fabric の対象 Minecraft バージョンを決定できません。".to_string())?;

    let loader_versions = client
        .get(format!(
            "{FABRIC_META_API_BASE}/versions/loader/{}",
            selected_version
        ))
        .send()
        .await
        .map_err(|error| format!("Fabric Loader 一覧を取得できませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Fabric Loader の取得に失敗しました: {error}"))?
        .json::<Vec<FabricLoaderManifestEntry>>()
        .await
        .map_err(|error| format!("Fabric Loader 一覧を解析できませんでした: {error}"))?;

    let available_loader_versions: Vec<LoaderVersionSummary> = loader_versions
        .iter()
        .take(12)
        .map(|entry| LoaderVersionSummary {
            id: entry.loader.version.clone(),
            stable: entry.loader.stable,
        })
        .collect();

    let recommended_loader = available_loader_versions
        .iter()
        .find(|entry| entry.stable)
        .cloned()
        .or_else(|| available_loader_versions.first().cloned())
        .ok_or_else(|| format!("{selected_version} 向けの Fabric Loader が見つかりません。"))?;

    let mut available_game_versions: Vec<MinecraftVersionSummary> = game_versions
        .into_iter()
        .filter(|entry| {
            !entry.version.ends_with("_unobfuscated") && !entry.version.ends_with("_original")
        })
        .take(28)
        .map(|entry| MinecraftVersionSummary {
            id: entry.version.clone(),
            stable: entry.stable,
            kind: if entry.stable {
                "release".to_string()
            } else {
                "snapshot".to_string()
            },
        })
        .collect();

    if !available_game_versions
        .iter()
        .any(|entry| entry.id == selected_version)
    {
        available_game_versions.insert(
            0,
            MinecraftVersionSummary {
                id: selected_version.clone(),
                stable: false,
                kind: "custom".to_string(),
            },
        );
    }

    Ok(FabricCatalog {
        minecraft_version: selected_version,
        latest_installer: LoaderVersionSummary {
            id: latest_installer.version.clone(),
            stable: latest_installer.stable,
        },
        recommended_loader,
        available_game_versions,
        available_loader_versions,
    })
}

pub async fn get_loader_catalog(
    loader: String,
    game_version: Option<String>,
) -> Result<LoaderCatalog, String> {
    match normalize_loader(Some(&loader)) {
        "fabric" => {
            let catalog = get_fabric_catalog(game_version).await?;
            Ok(LoaderCatalog {
                loader: "fabric".to_string(),
                minecraft_version: catalog.minecraft_version,
                installer_version: catalog.latest_installer,
                recommended_loader: catalog.recommended_loader,
                available_game_versions: catalog.available_game_versions,
                available_loader_versions: catalog.available_loader_versions,
            })
        }
        "quilt" => get_quilt_catalog(game_version).await,
        "forge" => get_forge_catalog(game_version).await,
        "neoforge" => get_neoforge_catalog(game_version).await,
        _ => Err("未対応の Loader です。".to_string()),
    }
}

pub async fn install_fabric_loader(
    app: &AppHandle,
    profile_id: Option<String>,
    minecraft_version: String,
    loader_version: Option<String>,
    profile_name: Option<String>,
    operation_id: Option<String>,
) -> Result<FabricInstallResult, String> {
    let operation_id = operation_id
        .unwrap_or_else(|| format!("fabric-install-{}", chrono::Local::now().timestamp_millis()));
    let requested_game_version = minecraft_version.trim();
    if requested_game_version.is_empty() {
        return Err("Minecraft バージョンを選択してください。".to_string());
    }

    let source_profile = profile_id.as_deref().map(find_profile).transpose()?;
    let resolved_profile_name = resolve_fabric_profile_name(
        source_profile.as_ref(),
        profile_name.as_deref(),
        requested_game_version,
    );
    emit_progress(
        app,
        &operation_id,
        "Fabric を導入中",
        format!(
            "Vanilla {} の必要ファイルを確認しています。",
            requested_game_version
        ),
        8.0,
    );
    let app_clone = app.clone();
    let op_id = operation_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        ensure_managed_java_runtime(Some((&app_clone, op_id.as_str())))
    })
    .await
    .map_err(|error| format!("Java ランタイム準備に失敗しました: {error}"))??;
    let (version_id, resolved_loader_version) =
        ensure_fabric_version_installed(requested_game_version, loader_version.as_deref()).await?;
    let result = finalize_loader_install(
        app,
        &operation_id,
        source_profile.as_ref(),
        resolved_profile_name,
        "fabric",
        requested_game_version,
        &resolved_loader_version,
        version_id,
    )
    .await?;

    Ok(FabricInstallResult {
        message: result.message,
        profile_id: result.profile_id,
        profile_name: result.profile_name,
        version_id: result.version_id,
        minecraft_version: result.minecraft_version,
        loader_version: result.loader_version,
    })
}

pub async fn install_loader(
    app: &AppHandle,
    loader: String,
    profile_id: Option<String>,
    minecraft_version: String,
    loader_version: Option<String>,
    profile_name: Option<String>,
    operation_id: Option<String>,
) -> Result<LoaderInstallResult, String> {
    match normalize_loader(Some(&loader)) {
        "fabric" => {
            let result = install_fabric_loader(
                app,
                profile_id,
                minecraft_version,
                loader_version,
                profile_name,
                operation_id,
            )
            .await?;

            Ok(LoaderInstallResult {
                message: result.message,
                loader: "fabric".to_string(),
                profile_id: result.profile_id,
                profile_name: result.profile_name,
                version_id: result.version_id,
                minecraft_version: result.minecraft_version,
                loader_version: result.loader_version,
            })
        }
        "quilt" => {
            install_quilt_loader(
                app,
                profile_id,
                minecraft_version,
                loader_version,
                profile_name,
                operation_id,
            )
            .await
        }
        "forge" => {
            install_forge_loader(
                app,
                profile_id,
                minecraft_version,
                loader_version,
                profile_name,
                operation_id,
            )
            .await
        }
        "neoforge" => {
            install_neoforge_loader(
                app,
                profile_id,
                minecraft_version,
                loader_version,
                profile_name,
                operation_id,
            )
            .await
        }
        _ => Err("未対応の Loader です。".to_string()),
    }
}

pub async fn ensure_loader_version_installed(
    loader: &str,
    minecraft_version: &str,
    loader_version: Option<&str>,
) -> Result<(String, String), String> {
    match normalize_loader(Some(loader)) {
        "fabric" => ensure_fabric_version_installed(minecraft_version, loader_version).await,
        "quilt" => ensure_quilt_version_installed(minecraft_version, loader_version).await,
        "forge" => ensure_forge_version_installed(minecraft_version, loader_version).await,
        "neoforge" => ensure_neoforge_version_installed(minecraft_version, loader_version).await,
        "vanilla" => {
            let root = minecraft_root()?;
            ensure_launcher_profiles_file(&root)?;
            ensure_version_ready(&root, minecraft_version).await?;
            Ok((minecraft_version.to_string(), minecraft_version.to_string()))
        }
        _ => Err("未対応の Loader です。".to_string()),
    }
}

async fn ensure_fabric_version_installed(
    minecraft_version: &str,
    loader_version: Option<&str>,
) -> Result<(String, String), String> {
    let root = minecraft_root()?;
    ensure_launcher_profiles_file(&root)?;
    ensure_version_ready(&root, minecraft_version).await?;
    let catalog = get_fabric_catalog(Some(minecraft_version.to_string())).await?;
    let requested_loader_version = loader_version
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let resolved_loader_version = requested_loader_version
        .filter(|requested| {
            catalog
                .available_loader_versions
                .iter()
                .any(|entry| entry.id == *requested)
        })
        .map(str::to_string)
        .unwrap_or_else(|| catalog.recommended_loader.id.clone());
    let version_id = format!("fabric-loader-{resolved_loader_version}-{minecraft_version}");

    run_fabric_installer(
        &download_fabric_installer(&catalog.latest_installer.id).await?,
        &root,
        minecraft_version,
        &resolved_loader_version,
    )?;
    ensure_version_ready(&root, &version_id).await?;

    Ok((version_id, resolved_loader_version))
}

async fn ensure_quilt_version_installed(
    minecraft_version: &str,
    loader_version: Option<&str>,
) -> Result<(String, String), String> {
    let root = minecraft_root()?;
    ensure_launcher_profiles_file(&root)?;
    ensure_version_ready(&root, minecraft_version).await?;
    let catalog = get_quilt_catalog(Some(minecraft_version.to_string())).await?;
    let resolved_loader_version = loader_version
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| catalog.recommended_loader.id.clone());
    let version_id = run_quilt_installer_in_stage(
        &download_quilt_installer(&catalog.installer_version.id).await?,
        &root,
        minecraft_version,
        &resolved_loader_version,
    )?;
    ensure_version_ready(&root, &version_id).await?;

    Ok((version_id, resolved_loader_version))
}

async fn ensure_forge_version_installed(
    minecraft_version: &str,
    loader_version: Option<&str>,
) -> Result<(String, String), String> {
    let root = minecraft_root()?;
    ensure_launcher_profiles_file(&root)?;
    ensure_version_ready(&root, minecraft_version).await?;
    let entries = fetch_forge_loader_entries().await?;
    let resolved_loader_version = loader_version
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            entries
                .iter()
                .find(|entry| entry.minecraft_version == minecraft_version && entry.stable)
                .map(|entry| entry.loader_version.clone())
        })
        .or_else(|| {
            entries
                .iter()
                .find(|entry| entry.minecraft_version == minecraft_version)
                .map(|entry| entry.loader_version.clone())
        })
        .ok_or_else(|| {
            format!("Minecraft {minecraft_version} に対応する Forge が見つかりません。")
        })?;
    let combined_version = resolve_maven_combined_version(
        &entries,
        minecraft_version,
        &resolved_loader_version,
        "Forge",
    )?;
    let version_id = run_staged_installer(
        &download_forge_installer(&combined_version).await?,
        &root,
        "forge",
        &["--installClient"],
        &combined_version,
    )?;
    ensure_version_ready(&root, &version_id).await?;

    Ok((version_id, resolved_loader_version))
}

async fn ensure_neoforge_version_installed(
    minecraft_version: &str,
    loader_version: Option<&str>,
) -> Result<(String, String), String> {
    let root = minecraft_root()?;
    ensure_launcher_profiles_file(&root)?;
    ensure_version_ready(&root, minecraft_version).await?;
    let entries = fetch_neoforge_loader_entries().await?;
    let resolved_loader_version = loader_version
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            entries
                .iter()
                .find(|entry| entry.minecraft_version == minecraft_version && entry.stable)
                .map(|entry| entry.loader_version.clone())
        })
        .or_else(|| {
            entries
                .iter()
                .find(|entry| entry.minecraft_version == minecraft_version)
                .map(|entry| entry.loader_version.clone())
        })
        .ok_or_else(|| {
            format!("Minecraft {minecraft_version} に対応する NeoForge が見つかりません。")
        })?;
    let version_id = run_staged_installer(
        &download_neoforge_installer(&resolved_loader_version).await?,
        &root,
        "neoforge",
        &["--install-client"],
        &resolved_loader_version,
    )?;
    ensure_version_ready(&root, &version_id).await?;

    Ok((version_id, resolved_loader_version))
}

async fn get_quilt_catalog(game_version: Option<String>) -> Result<LoaderCatalog, String> {
    let installer_version = fetch_maven_release_version(QUILT_INSTALLER_METADATA_URL).await?;
    let available_loader_versions = fetch_quilt_loader_versions().await?;
    let recommended_loader = available_loader_versions
        .iter()
        .find(|entry| entry.stable)
        .cloned()
        .or_else(|| available_loader_versions.first().cloned())
        .ok_or_else(|| "Quilt Loader 一覧を取得できませんでした。".to_string())?;
    let (available_game_versions, selected_version) =
        build_available_game_versions(None, game_version).await?;

    Ok(LoaderCatalog {
        loader: "quilt".to_string(),
        minecraft_version: selected_version,
        installer_version: LoaderVersionSummary {
            id: installer_version.clone(),
            stable: is_stable_loader_version(&installer_version),
        },
        recommended_loader,
        available_game_versions,
        available_loader_versions,
    })
}

async fn get_forge_catalog(game_version: Option<String>) -> Result<LoaderCatalog, String> {
    let entries = fetch_forge_loader_entries().await?;
    build_maven_loader_catalog("forge", &entries, game_version).await
}

async fn get_neoforge_catalog(game_version: Option<String>) -> Result<LoaderCatalog, String> {
    let entries = fetch_neoforge_loader_entries().await?;
    build_maven_loader_catalog("neoforge", &entries, game_version).await
}

async fn build_maven_loader_catalog(
    loader: &str,
    entries: &[MavenLoaderEntry],
    game_version: Option<String>,
) -> Result<LoaderCatalog, String> {
    let supported_game_versions: Vec<String> = unique_values(
        entries
            .iter()
            .map(|entry| entry.minecraft_version.clone())
            .collect(),
    );
    let (available_game_versions, selected_version) =
        build_available_game_versions(Some(supported_game_versions), game_version).await?;
    let available_loader_versions = unique_loader_versions(
        entries
            .iter()
            .filter(|entry| entry.minecraft_version == selected_version)
            .map(|entry| LoaderVersionSummary {
                id: entry.loader_version.clone(),
                stable: entry.stable,
            })
            .collect(),
    );
    let recommended_loader = available_loader_versions
        .iter()
        .find(|entry| entry.stable)
        .cloned()
        .or_else(|| available_loader_versions.first().cloned())
        .ok_or_else(|| {
            format!(
                "{selected_version} 向けの {} Loader が見つかりません。",
                loader_display_name(loader)
            )
        })?;

    Ok(LoaderCatalog {
        loader: loader.to_string(),
        minecraft_version: selected_version,
        installer_version: recommended_loader.clone(),
        recommended_loader,
        available_game_versions,
        available_loader_versions,
    })
}

async fn install_quilt_loader(
    app: &AppHandle,
    profile_id: Option<String>,
    minecraft_version: String,
    loader_version: Option<String>,
    profile_name: Option<String>,
    operation_id: Option<String>,
) -> Result<LoaderInstallResult, String> {
    let operation_id = operation_id
        .unwrap_or_else(|| format!("quilt-install-{}", chrono::Local::now().timestamp_millis()));
    let requested_game_version = minecraft_version.trim();
    if requested_game_version.is_empty() {
        return Err("Minecraft バージョンを選択してください。".to_string());
    }

    let source_profile = profile_id.as_deref().map(find_profile).transpose()?;
    let resolved_profile_name = resolve_loader_profile_name(
        source_profile.as_ref(),
        profile_name.as_deref(),
        "quilt",
        requested_game_version,
    );
    emit_progress(
        app,
        &operation_id,
        "Quilt を導入中",
        format!(
            "Vanilla {} の必要ファイルを確認しています。",
            requested_game_version
        ),
        8.0,
    );
    let app_clone = app.clone();
    let op_id = operation_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        ensure_managed_java_runtime(Some((&app_clone, op_id.as_str())))
    })
    .await
    .map_err(|error| format!("Java ランタイム準備に失敗しました: {error}"))??;
    let (version_id, resolved_loader_version) =
        ensure_quilt_version_installed(requested_game_version, loader_version.as_deref()).await?;

    finalize_loader_install(
        app,
        &operation_id,
        source_profile.as_ref(),
        resolved_profile_name,
        "quilt",
        requested_game_version,
        &resolved_loader_version,
        version_id,
    )
    .await
}

async fn install_forge_loader(
    app: &AppHandle,
    profile_id: Option<String>,
    minecraft_version: String,
    loader_version: Option<String>,
    profile_name: Option<String>,
    operation_id: Option<String>,
) -> Result<LoaderInstallResult, String> {
    let operation_id = operation_id
        .unwrap_or_else(|| format!("forge-install-{}", chrono::Local::now().timestamp_millis()));
    let requested_game_version = minecraft_version.trim();
    if requested_game_version.is_empty() {
        return Err("Minecraft バージョンを選択してください。".to_string());
    }

    let source_profile = profile_id.as_deref().map(find_profile).transpose()?;
    let resolved_profile_name = resolve_loader_profile_name(
        source_profile.as_ref(),
        profile_name.as_deref(),
        "forge",
        requested_game_version,
    );
    emit_progress(
        app,
        &operation_id,
        "Forge を導入中",
        format!(
            "Vanilla {} の必要ファイルを確認しています。",
            requested_game_version
        ),
        8.0,
    );
    let app_clone = app.clone();
    let op_id = operation_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        ensure_managed_java_runtime(Some((&app_clone, op_id.as_str())))
    })
    .await
    .map_err(|error| format!("Java ランタイム準備に失敗しました: {error}"))??;
    let (version_id, resolved_loader_version) =
        ensure_forge_version_installed(requested_game_version, loader_version.as_deref()).await?;

    finalize_loader_install(
        app,
        &operation_id,
        source_profile.as_ref(),
        resolved_profile_name,
        "forge",
        requested_game_version,
        &resolved_loader_version,
        version_id,
    )
    .await
}

async fn install_neoforge_loader(
    app: &AppHandle,
    profile_id: Option<String>,
    minecraft_version: String,
    loader_version: Option<String>,
    profile_name: Option<String>,
    operation_id: Option<String>,
) -> Result<LoaderInstallResult, String> {
    let operation_id = operation_id.unwrap_or_else(|| {
        format!(
            "neoforge-install-{}",
            chrono::Local::now().timestamp_millis()
        )
    });
    let requested_game_version = minecraft_version.trim();
    if requested_game_version.is_empty() {
        return Err("Minecraft バージョンを選択してください。".to_string());
    }

    let source_profile = profile_id.as_deref().map(find_profile).transpose()?;
    let resolved_profile_name = resolve_loader_profile_name(
        source_profile.as_ref(),
        profile_name.as_deref(),
        "neoforge",
        requested_game_version,
    );
    emit_progress(
        app,
        &operation_id,
        "NeoForge を導入中",
        format!(
            "Vanilla {} の必要ファイルを確認しています。",
            requested_game_version
        ),
        8.0,
    );
    let app_clone = app.clone();
    let op_id = operation_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        ensure_managed_java_runtime(Some((&app_clone, op_id.as_str())))
    })
    .await
    .map_err(|error| format!("Java ランタイム準備に失敗しました: {error}"))??;
    let (version_id, resolved_loader_version) =
        ensure_neoforge_version_installed(requested_game_version, loader_version.as_deref())
            .await?;

    finalize_loader_install(
        app,
        &operation_id,
        source_profile.as_ref(),
        resolved_profile_name,
        "neoforge",
        requested_game_version,
        &resolved_loader_version,
        version_id,
    )
    .await
}

async fn finalize_loader_install(
    app: &AppHandle,
    operation_id: &str,
    source_profile: Option<&crate::models::LauncherProfile>,
    profile_name: String,
    loader: &str,
    minecraft_version: &str,
    loader_version: &str,
    version_id: String,
) -> Result<LoaderInstallResult, String> {
    let root = minecraft_root()?;
    let progress_title = format!("{} を導入中", loader_display_name(loader));
    emit_progress(
        app,
        operation_id,
        &progress_title,
        "起動構成と必要ファイルを仕上げています。",
        78.0,
    );
    let game_dir = profile_instance_dir(&root, loader, &profile_name);
    fs::create_dir_all(game_dir.join("mods"))
        .map_err(|error| format!("mods フォルダを準備できませんでした: {error}"))?;

    let profile_id = upsert_custom_profile(CustomProfileDraft {
        name: profile_name.clone(),
        icon: source_profile
            .as_ref()
            .and_then(|profile| profile.icon.clone())
            .or_else(|| Some("Grass".to_string())),
        custom_icon_url: source_profile
            .as_ref()
            .and_then(|profile| profile.custom_icon_url.clone()),
        background_image_url: source_profile
            .as_ref()
            .and_then(|profile| profile.background_image_url.clone()),
        game_dir,
        last_version_id: version_id.clone(),
    })?;
    ensure_version_ready(&root, &version_id).await?;
    emit_progress(
        app,
        operation_id,
        &progress_title,
        format!("{} 構成の準備が完了しました。", loader_display_name(loader)),
        100.0,
    );

    Ok(LoaderInstallResult {
        message: format!(
            "{} {} を Minecraft {} に導入し、{} を作成しました。必要な Vanilla 本体も先に揃えています。",
            loader_display_name(loader),
            loader_version,
            minecraft_version,
            profile_name
        ),
        loader: loader.to_string(),
        profile_id,
        profile_name,
        version_id,
        minecraft_version: minecraft_version.to_string(),
        loader_version: loader_version.to_string(),
    })
}

fn loader_display_name(loader: &str) -> &'static str {
    match loader {
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "quilt" => "Quilt",
        _ => "Loader",
    }
}

async fn build_available_game_versions(
    supported_versions: Option<Vec<String>>,
    preferred_game_version: Option<String>,
) -> Result<(Vec<MinecraftVersionSummary>, String), String> {
    let manifest = fetch_official_version_manifest().await?;
    let preferred = preferred_game_version
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let supported_lookup = supported_versions.map(unique_values);

    let mut available_game_versions = Vec::new();
    for entry in manifest.versions {
        if entry.id.ends_with("_unobfuscated") || entry.id.ends_with("_original") {
            continue;
        }

        if let Some(values) = supported_lookup.as_ref() {
            if !values.iter().any(|value| value == &entry.id) {
                continue;
            }
        }

        available_game_versions.push(MinecraftVersionSummary {
            id: entry.id.clone(),
            stable: entry.version_type == "release",
            kind: entry.version_type,
        });

        if available_game_versions.len() >= 28 {
            break;
        }
    }

    let selected_version = preferred
        .clone()
        .or_else(|| {
            available_game_versions
                .iter()
                .find(|entry| entry.stable)
                .map(|entry| entry.id.clone())
        })
        .or_else(|| {
            available_game_versions
                .first()
                .map(|entry| entry.id.clone())
        })
        .ok_or_else(|| "対象の Minecraft バージョンを決定できません。".to_string())?;

    if !available_game_versions
        .iter()
        .any(|entry| entry.id == selected_version)
    {
        available_game_versions.insert(
            0,
            MinecraftVersionSummary {
                id: selected_version.clone(),
                stable: false,
                kind: "custom".to_string(),
            },
        );
    }

    Ok((available_game_versions, selected_version))
}

async fn fetch_quilt_loader_versions() -> Result<Vec<LoaderVersionSummary>, String> {
    let metadata = fetch_text(QUILT_LOADER_METADATA_URL, "Quilt Loader 一覧").await?;
    let versions = parse_xml_tag_values(&metadata, "version");
    let summaries = unique_loader_versions(
        versions
            .into_iter()
            .rev()
            .take(12)
            .map(|version| LoaderVersionSummary {
                stable: is_stable_loader_version(&version),
                id: version,
            })
            .collect(),
    );

    if summaries.is_empty() {
        return Err("Quilt Loader 一覧を取得できませんでした。".to_string());
    }

    Ok(summaries)
}

async fn fetch_forge_loader_entries() -> Result<Vec<MavenLoaderEntry>, String> {
    let metadata = fetch_text(FORGE_MAVEN_METADATA_URL, "Forge のバージョン一覧").await?;
    Ok(parse_xml_tag_values(&metadata, "version")
        .into_iter()
        .rev()
        .filter_map(|combined| {
            let (raw_minecraft_version, loader_version) = combined.split_once('-')?;
            Some(MavenLoaderEntry {
                minecraft_version: normalize_catalog_game_version(raw_minecraft_version),
                loader_version: loader_version.to_string(),
                stable: is_stable_loader_version(&loader_version),
                combined_version: combined,
            })
        })
        .collect())
}

async fn fetch_neoforge_loader_entries() -> Result<Vec<MavenLoaderEntry>, String> {
    let metadata = fetch_text(NEOFORGE_MAVEN_METADATA_URL, "NeoForge のバージョン一覧").await?;
    Ok(parse_xml_tag_values(&metadata, "version")
        .into_iter()
        .rev()
        .filter_map(|combined| {
            let minecraft_version = parse_neoforge_catalog_game_version(&combined)?;
            Some(MavenLoaderEntry {
                minecraft_version,
                loader_version: combined.clone(),
                stable: is_stable_loader_version(&combined),
                combined_version: combined,
            })
        })
        .collect())
}

fn parse_neoforge_catalog_game_version(version: &str) -> Option<String> {
    let prefix = version.split('-').next().unwrap_or(version);
    let mut segments = prefix.split('.');
    let major = segments.next()?;
    let minor = segments.next()?;

    if !major.chars().all(|character| character.is_ascii_digit())
        || !minor.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }

    Some(format!("1.{major}.{minor}"))
}

fn normalize_catalog_game_version(value: &str) -> String {
    if value.starts_with("1.") {
        return value.to_string();
    }

    let mut segments = value.split('.');
    let major = segments.next().unwrap_or_default();
    let minor = segments.next().unwrap_or_default();
    if major.chars().all(|character| character.is_ascii_digit())
        && minor.chars().all(|character| character.is_ascii_digit())
        && !major.is_empty()
        && !minor.is_empty()
    {
        format!("1.{major}.{minor}")
    } else {
        value.to_string()
    }
}

fn resolve_maven_combined_version(
    entries: &[MavenLoaderEntry],
    minecraft_version: &str,
    loader_version: &str,
    loader_name: &str,
) -> Result<String, String> {
    entries
        .iter()
        .find(|entry| {
            entry.minecraft_version == minecraft_version && entry.loader_version == loader_version
        })
        .map(|entry| entry.combined_version.clone())
        .ok_or_else(|| {
            format!(
                "{loader_name} {loader_version} は Minecraft {minecraft_version} に対応していません。"
            )
        })
}

async fn fetch_maven_release_version(metadata_url: &str) -> Result<String, String> {
    let metadata = fetch_text(metadata_url, "Maven metadata").await?;
    parse_xml_tag_values(&metadata, "release")
        .into_iter()
        .next()
        .or_else(|| parse_xml_tag_values(&metadata, "latest").into_iter().next())
        .ok_or_else(|| "最新版を特定できませんでした。".to_string())
}

async fn fetch_text(url: &str, label: &str) -> Result<String, String> {
    loader_http_client()?
        .get(url)
        .send()
        .await
        .map_err(|error| format!("{label}を取得できませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("{label}の取得に失敗しました: {error}"))?
        .text()
        .await
        .map_err(|error| format!("{label}を読み取れませんでした: {error}"))
}

fn parse_xml_tag_values(text: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut values = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find(&open) {
        let content_start = start + open.len();
        let next = &remaining[content_start..];
        let Some(end) = next.find(&close) else {
            break;
        };
        let value = next[..end].trim();
        if !value.is_empty() {
            values.push(value.to_string());
        }
        remaining = &next[end + close.len()..];
    }

    values
}

fn unique_values(values: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        if !unique.iter().any(|item| item == &value) {
            unique.push(value);
        }
    }
    unique
}

fn unique_loader_versions(values: Vec<LoaderVersionSummary>) -> Vec<LoaderVersionSummary> {
    let mut unique = Vec::new();
    for value in values {
        if !unique
            .iter()
            .any(|item: &LoaderVersionSummary| item.id == value.id)
        {
            unique.push(value);
        }
    }
    unique
}

fn is_stable_loader_version(value: &str) -> bool {
    let lower = value.to_lowercase();
    !(lower.contains("alpha") || lower.contains("beta") || lower.contains("pre"))
}

async fn download_quilt_installer(installer_version: &str) -> Result<PathBuf, String> {
    let cache_dir = loader_cache_dir()?;
    let target_path = cache_dir.join(format!("quilt-installer-{installer_version}.jar"));

    if !target_path.exists() {
        download_binary(
            QUILT_INSTALLER_DOWNLOAD_URL,
            &target_path,
            "Quilt Installer",
        )
        .await?;
    }

    Ok(target_path)
}

async fn download_forge_installer(combined_version: &str) -> Result<PathBuf, String> {
    let cache_dir = loader_cache_dir()?;
    let target_path = cache_dir.join(format!("forge-{combined_version}-installer.jar"));
    let url = format!(
        "{FORGE_MAVEN_BASE}/net/minecraftforge/forge/{combined_version}/forge-{combined_version}-installer.jar"
    );

    if !target_path.exists() {
        download_binary(&url, &target_path, "Forge Installer").await?;
    }

    Ok(target_path)
}

async fn download_neoforge_installer(version: &str) -> Result<PathBuf, String> {
    let cache_dir = loader_cache_dir()?;
    let target_path = cache_dir.join(format!("neoforge-{version}-installer.jar"));
    let url = format!(
        "{NEOFORGE_MAVEN_BASE}/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
    );

    if !target_path.exists() {
        download_binary(&url, &target_path, "NeoForge Installer").await?;
    }

    Ok(target_path)
}

fn loader_cache_dir() -> Result<PathBuf, String> {
    let cache_dir = std::env::temp_dir().join("vanillalauncher");
    fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("一時フォルダを準備できませんでした: {error}"))?;
    Ok(cache_dir)
}

async fn download_binary(url: &str, target_path: &Path, label: &str) -> Result<(), String> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{} を準備できませんでした: {error}", parent.display()))?;
    }

    let bytes = loader_http_client()?
        .get(url)
        .send()
        .await
        .map_err(|error| format!("{label} をダウンロードできませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("{label} のダウンロードに失敗しました: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("{label} の内容を読み取れませんでした: {error}"))?;

    fs::write(target_path, &bytes)
        .map_err(|error| format!("{} を保存できませんでした: {error}", target_path.display()))
}

fn run_quilt_installer_in_stage(
    installer_path: &Path,
    minecraft_root: &Path,
    minecraft_version: &str,
    loader_version: &str,
) -> Result<String, String> {
    let stage_root = prepare_stage_root("quilt")?;
    let java = find_java_executable()?;
    let mut command = Command::new(java);
    command
        .arg("-jar")
        .arg(installer_path)
        .arg("install")
        .arg("client")
        .arg(minecraft_version)
        .arg(loader_version)
        .arg(format!("--install-dir={}", stage_root.display()))
        .arg("--no-profile")
        .current_dir(&stage_root);
    suppress_console_window(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("Quilt Installer を実行できませんでした: {error}"))?;

    if !output.status.success() {
        return Err(installer_error_detail("Quilt Installer", &output));
    }

    let version_id =
        detect_installed_version_id(&stage_root, "quilt", minecraft_version, loader_version)?;
    copy_stage_artifacts(&stage_root, minecraft_root, &version_id)?;
    let _ = fs::remove_dir_all(&stage_root);
    Ok(version_id)
}

fn run_staged_installer(
    installer_path: &Path,
    minecraft_root: &Path,
    loader: &str,
    installer_args: &[&str],
    version_hint: &str,
) -> Result<String, String> {
    let stage_root = prepare_stage_root(loader)?;
    ensure_launcher_profiles_file(&stage_root)?;
    let java = find_java_executable()?;
    let mut command = Command::new(java);
    command.arg("-jar").arg(installer_path);
    for argument in installer_args {
        command.arg(argument);
    }
    command.arg(&stage_root).current_dir(&stage_root);
    suppress_console_window(&mut command);
    let output = command.output().map_err(|error| {
        format!(
            "{} Installer を実行できませんでした: {error}",
            loader_display_name(loader)
        )
    })?;

    if !output.status.success() {
        return Err(installer_error_detail(
            &format!("{} Installer", loader_display_name(loader)),
            &output,
        ));
    }

    let version_id = detect_installed_version_id(&stage_root, loader, "", version_hint)?;
    copy_stage_artifacts(&stage_root, minecraft_root, &version_id)?;
    let _ = fs::remove_dir_all(&stage_root);
    Ok(version_id)
}

fn prepare_stage_root(loader: &str) -> Result<PathBuf, String> {
    let stage_root = std::env::temp_dir().join("vanillalauncher").join(format!(
        "{loader}-stage-{}",
        chrono::Local::now().timestamp_millis()
    ));
    fs::create_dir_all(&stage_root)
        .map_err(|error| format!("一時導入先を準備できませんでした: {error}"))?;
    Ok(stage_root)
}

fn detect_installed_version_id(
    stage_root: &Path,
    loader: &str,
    minecraft_version: &str,
    loader_version: &str,
) -> Result<String, String> {
    let versions_dir = stage_root.join("versions");
    let entries = fs::read_dir(&versions_dir)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", versions_dir.display()))?;
    let mut candidates = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| format!("導入結果を確認できませんでした: {error}"))?;
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if matches_version_candidate(loader, &name, minecraft_version, loader_version) {
            candidates.push(name);
        }
    }

    candidates.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    candidates.into_iter().next().ok_or_else(|| {
        format!(
            "{} の導入は完了しましたが、version ディレクトリを特定できませんでした。",
            loader_display_name(loader)
        )
    })
}

fn matches_version_candidate(
    loader: &str,
    version_id: &str,
    minecraft_version: &str,
    loader_version: &str,
) -> bool {
    let lower = version_id.to_lowercase();
    let loader_version = loader_version.to_lowercase();
    let minecraft_version = minecraft_version.to_lowercase();

    match loader {
        "quilt" => {
            lower.starts_with("quilt-loader-")
                && (minecraft_version.is_empty() || lower.contains(&minecraft_version))
                && (loader_version.is_empty() || lower.contains(&loader_version))
        }
        "forge" => {
            lower.contains("forge")
                && (loader_version.is_empty() || lower.contains(&loader_version))
                && (minecraft_version.is_empty() || lower.contains(&minecraft_version))
        }
        "neoforge" => {
            lower.contains("neoforge")
                && (loader_version.is_empty() || lower.contains(&loader_version))
                && (minecraft_version.is_empty() || lower.contains(&minecraft_version))
        }
        _ => false,
    }
}

fn copy_stage_artifacts(
    stage_root: &Path,
    minecraft_root: &Path,
    version_id: &str,
) -> Result<(), String> {
    let source_version_dir = stage_root.join("versions").join(version_id);
    let target_version_dir = minecraft_root.join("versions").join(version_id);

    if target_version_dir.exists() {
        let _ = fs::remove_dir_all(&target_version_dir);
    }
    sync_directory(&source_version_dir, &target_version_dir)?;

    let source_libraries_dir = stage_root.join("libraries");
    if source_libraries_dir.exists() {
        sync_directory(&source_libraries_dir, &minecraft_root.join("libraries"))?;
    }

    Ok(())
}

fn sync_directory(source: &Path, target: &Path) -> Result<(), String> {
    if !source.exists() {
        return Err(format!("{} が見つかりません。", source.display()));
    }

    fs::create_dir_all(target)
        .map_err(|error| format!("{} を準備できませんでした: {error}", target.display()))?;

    let entries = fs::read_dir(source)
        .map_err(|error| format!("{} を読み込めませんでした: {error}", source.display()))?;

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("ファイルコピーの確認に失敗しました: {error}"))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if source_path.is_dir() {
            sync_directory(&source_path, &target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!("{} を準備できませんでした: {error}", parent.display())
                })?;
            }
            fs::copy(&source_path, &target_path).map_err(|error| {
                format!(
                    "{} を {} へコピーできませんでした: {error}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn installer_error_detail(label: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if !stderr.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        stdout.trim().to_string()
    };

    if detail.is_empty() {
        format!("{label} が失敗しました。")
    } else {
        format!("{label} が失敗しました: {detail}")
    }
}

fn resolve_loader_profile_name(
    source_profile: Option<&crate::models::LauncherProfile>,
    requested_name: Option<&str>,
    loader: &str,
    minecraft_version: &str,
) -> String {
    if let Some(name) = requested_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return name.to_string();
    }

    let loader_name = loader_display_name(loader);

    if let Some(profile) = source_profile {
        if profile.name == "最新リリース" || profile.name == "最新スナップショット"
        {
            return format!("{loader_name} {minecraft_version}");
        }

        return format!("{} / {loader_name}", profile.name);
    }

    format!("{loader_name} {minecraft_version}")
}

async fn download_fabric_installer(installer_version: &str) -> Result<PathBuf, String> {
    let client = fabric_client()?;
    let installers = client
        .get(format!("{FABRIC_META_API_BASE}/versions/installer"))
        .send()
        .await
        .map_err(|error| format!("Fabric Installer 一覧を取得できませんでした: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Fabric Installer の取得に失敗しました: {error}"))?
        .json::<Vec<FabricInstallerEntry>>()
        .await
        .map_err(|error| format!("Fabric Installer 一覧を解析できませんでした: {error}"))?;

    let installer = installers
        .into_iter()
        .find(|entry| entry.version == installer_version)
        .ok_or_else(|| format!("Fabric Installer {installer_version} が見つかりません。"))?;

    let cache_dir = std::env::temp_dir().join("vanillalauncher");
    fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("一時フォルダを準備できませんでした: {error}"))?;
    let target_path = cache_dir.join(format!("fabric-installer-{}.jar", installer.version));

    if !target_path.exists() {
        let bytes = client
            .get(&installer.url)
            .send()
            .await
            .map_err(|error| format!("Fabric Installer をダウンロードできませんでした: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Fabric Installer のダウンロードに失敗しました: {error}"))?
            .bytes()
            .await
            .map_err(|error| format!("Fabric Installer の内容を読み取れませんでした: {error}"))?;

        fs::write(&target_path, &bytes)
            .map_err(|error| format!("Fabric Installer を保存できませんでした: {error}"))?;
    }

    Ok(target_path)
}

fn run_fabric_installer(
    installer_path: &Path,
    minecraft_root: &Path,
    minecraft_version: &str,
    loader_version: &str,
) -> Result<(), String> {
    ensure_launcher_profiles_file(minecraft_root)?;
    let java = find_java_executable()?;
    let mut command = Command::new(java);
    command
        .arg("-jar")
        .arg(installer_path)
        .arg("client")
        .arg("-dir")
        .arg(minecraft_root)
        .arg("-mcversion")
        .arg(minecraft_version)
        .arg("-loader")
        .arg(loader_version)
        .arg("-noprofile");
    suppress_console_window(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("Fabric Installer を実行できませんでした: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };

        return Err(if detail.is_empty() {
            "Fabric Installer が失敗しました。".to_string()
        } else {
            format!("Fabric Installer が失敗しました: {detail}")
        });
    }

    let version_dir = minecraft_root.join("versions").join(format!(
        "fabric-loader-{}-{}",
        loader_version, minecraft_version
    ));
    if !version_dir.exists() {
        return Err(format!(
            "Fabric Installer は完了しましたが、{} が作成されませんでした。",
            version_dir.display()
        ));
    }

    Ok(())
}

fn resolve_fabric_profile_name(
    source_profile: Option<&crate::models::LauncherProfile>,
    requested_name: Option<&str>,
    minecraft_version: &str,
) -> String {
    if let Some(name) = requested_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return name.to_string();
    }

    if let Some(profile) = source_profile {
        if profile.name == "最新リリース" || profile.name == "最新スナップショット"
        {
            return format!("Fabric {minecraft_version}");
        }

        return format!("{} / Fabric", profile.name);
    }

    format!("Fabric {minecraft_version}")
}
