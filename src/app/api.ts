import { invoke } from "@tauri-apps/api/core";
import type {
  ActionResult,
  AppSettings,
  DebugExportResult,
  FabricCatalog,
  FabricInstallResult,
  InstallResult,
  LaunchResult,
  LocalModAnalysis,
  LoaderCatalog,
  LoaderId,
  LoaderInstallResult,
  LauncherSnapshot,
  ModRemoteState,
  ModpackExportResult,
  ModpackExportFormat,
  ModpackInstallResult,
  ModpackVersionSummary,
  ModrinthProject,
  SoftwareStatus,
  XboxRpsStateResult,
  XboxSignInResult,
} from "./types";

export const launcherApi = {
  getLauncherState: () => invoke<LauncherSnapshot>("get_launcher_state"),
  searchMods: (
    query: string,
    loader?: string | null,
    gameVersion?: string | null,
    limit?: number,
    offset?: number,
  ) =>
    invoke<ModrinthProject[]>("search_modrinth_mods", {
      query,
      loader,
      gameVersion,
      limit,
      offset,
    }),
  searchModpacks: (query: string, gameVersion?: string | null, limit?: number, offset?: number) =>
    invoke<ModrinthProject[]>("search_modrinth_modpacks", {
      query,
      gameVersion,
      limit,
      offset,
    }),
  getModpackVersions: (projectId: string) =>
    invoke<ModpackVersionSummary[]>("get_modrinth_modpack_versions", {
      projectId,
    }),
  installProject: (profileId: string, projectId: string, operationId?: string) =>
    invoke<InstallResult>("install_modrinth_project", { profileId, projectId, operationId }),
  getProfileModRemoteStates: (profileId: string) =>
    invoke<ModRemoteState[]>("get_profile_mod_remote_states", { profileId }),
  getProfileModRemoteState: (profileId: string, fileName: string) =>
    invoke<ModRemoteState | null>("get_profile_mod_remote_state", { profileId, fileName }),
  getProfileModVisualState: (profileId: string, fileName: string) =>
    invoke<ModRemoteState | null>("get_profile_mod_visual_state", { profileId, fileName }),
  updateProject: (profileId: string, projectId: string, fileName: string, operationId?: string) =>
    invoke<InstallResult>("update_modrinth_project", {
      profileId,
      projectId,
      fileName,
      operationId,
    }),
  installModpack: (
    projectId: string,
    versionId?: string | null,
    operationId?: string,
    iconUrl?: string | null,
    imageUrl?: string | null,
  ) =>
    invoke<ModpackInstallResult>("install_modrinth_modpack", {
      projectId,
      versionId,
      operationId,
      iconUrl,
      imageUrl,
    }),
  updateModpackProfile: (profileId: string, gameVersion?: string | null, operationId?: string) =>
    invoke<ModpackInstallResult>("update_modrinth_modpack_profile", {
      profileId,
      gameVersion,
      operationId,
    }),
  importLocalModpack: (mrpackPath: string, profileName?: string | null, operationId?: string) =>
    invoke<ModpackInstallResult>("import_local_modpack", {
      mrpackPath,
      profileName,
      operationId,
    }),
  exportProfileModpack: (profileId: string, outputPath: string, format: ModpackExportFormat) =>
    invoke<ModpackExportResult>("export_profile_modpack", { profileId, outputPath, format }),
  deleteProfile: (profileId: string) =>
    invoke<ActionResult>("delete_profile", { profileId }),
  updateProfileVisuals: (
    profileId: string,
    customIconUrl?: string | null,
    backgroundImageUrl?: string | null,
  ) =>
    invoke<ActionResult>("update_profile_visuals", {
      profileId,
      customIconUrl,
      backgroundImageUrl,
    }),
  updateProfileName: (profileId: string, profileName: string) =>
    invoke<ActionResult>("update_profile_name", {
      profileId,
      profileName,
    }),
  setActiveLauncherAccount: (localId: string) =>
    invoke<ActionResult>("set_active_launcher_account", {
      localId,
    }),
  logoutMicrosoftLauncherAccount: (localId: string) =>
    invoke<ActionResult>("logout_microsoft_launcher_account", {
      localId,
    }),
  scanLauncherAccounts: (operationId?: string) =>
    invoke<ActionResult>("scan_launcher_accounts", { operationId }),
  uninstallProject: (profileId: string, projectId: string) =>
    invoke<ActionResult>("uninstall_modrinth_project", { profileId, projectId }),
  setModEnabled: (profileId: string, fileName: string, enabled: boolean) =>
    invoke<ActionResult>("set_mod_enabled", { profileId, fileName, enabled }),
  importLocalMod: (profileId: string, modPath: string) =>
    invoke<ActionResult>("import_local_mod", { profileId, modPath }),
  analyzeLocalMod: (profileId: string, modPath: string) =>
    invoke<LocalModAnalysis>("analyze_local_mod", { profileId, modPath }),
  importCheckedLocalMod: (profileId: string, modPath: string) =>
    invoke<ActionResult>("import_checked_local_mod", { profileId, modPath }),
  removeMod: (profileId: string, fileName: string) =>
    invoke<ActionResult>("remove_mod", { profileId, fileName }),
  resolveProfilePath: (profileId: string, target: "game" | "mods") =>
    invoke<string>("resolve_profile_path", { profileId, target }),
  getLoaderCatalog: (loader: LoaderId, gameVersion?: string | null) =>
    invoke<LoaderCatalog>("get_loader_catalog", { loader, gameVersion }),
  installLoader: (
    loader: LoaderId,
    profileId: string | undefined,
    minecraftVersion: string,
    loaderVersion?: string,
    profileName?: string,
    operationId?: string,
  ) =>
    invoke<LoaderInstallResult>("install_loader", {
      loader,
      profileId,
      minecraftVersion,
      loaderVersion,
      profileName,
      operationId,
    }),
  getFabricCatalog: (gameVersion?: string | null) =>
    invoke<FabricCatalog>("get_fabric_catalog", { gameVersion }),
  installFabricLoader: (
    profileId: string | undefined,
    minecraftVersion: string,
    loaderVersion?: string,
    profileName?: string,
    operationId?: string,
  ) =>
    invoke<FabricInstallResult>("install_fabric_loader", {
      profileId,
      minecraftVersion,
      loaderVersion,
      profileName,
      operationId,
    }),
  ensureXboxRpsState: (operationId?: string) =>
    invoke<XboxRpsStateResult>("ensure_xbox_rps_state", { operationId }),
  startXboxSignIn: (operationId?: string) =>
    invoke<XboxSignInResult>("start_xbox_sign_in", { operationId }),
  launchProfileDirectly: (profileId: string) =>
    invoke<LaunchResult>("launch_profile_directly", { profileId }),
  launchProfileInOfficialLauncher: (profileId: string) =>
    invoke<LaunchResult>("launch_profile_in_official_launcher", { profileId }),
  getAppSettings: () => invoke<AppSettings>("get_app_settings"),
  updateAppSettings: (
    tempCacheEnabled: boolean,
    performanceLiteMode: AppSettings["performanceLiteMode"],
    customJavaPath?: string | null,
    offlineModeEnabled?: boolean,
    offlineUsername?: string | null,
    officialLauncherAutoInstall?: boolean,
  ) =>
    invoke<ActionResult>("update_app_settings", {
      tempCacheEnabled,
      performanceLiteMode,
      customJavaPath,
      offlineModeEnabled,
      offlineUsername,
      officialLauncherAutoInstall,
    }),
  ensureJavaRuntimeAvailable: (operationId?: string) =>
    invoke<ActionResult>("ensure_java_runtime_available", { operationId }),
  ensureOfficialLauncherAvailable: (operationId?: string, reinstall?: boolean) =>
    invoke<ActionResult>("ensure_official_launcher_available", { operationId, reinstall }),
  clearTempCache: () => invoke<ActionResult>("clear_temp_cache"),
  readTempCacheJson: (key: string) => invoke<string | null>("read_temp_cache_json", { key }),
  writeTempCacheJson: (key: string, jsonText: string) =>
    invoke<ActionResult>("write_temp_cache_json", { key, jsonText }),
  getSoftwareStatus: () => invoke<SoftwareStatus>("get_software_status"),
  exportDebugLog: () => invoke<DebugExportResult>("export_debug_log"),
};
