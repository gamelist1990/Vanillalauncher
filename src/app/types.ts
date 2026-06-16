export type ViewMode = "play" | "mods" | "discover" | "loaders" | "settings";

export type Notice = {
  id: string;
  tone: "success" | "error" | "info";
  text: string;
};

export type LoaderId = "fabric" | "forge" | "neoforge" | "quilt";

export type ProgressState = {
  operationId: string;
  title: string;
  detail: string;
  percent: number;
};

export type ProgressStatus = "active" | "completed" | "inactive";

export type ProgressHistoryEntry = {
  id: string;
  timestamp: number;
  title: string;
  detail: string;
  percent: number;
};

export type ProgressSnapshot = ProgressState & {
  status: ProgressStatus;
  startedAt: number;
  updatedAt: number;
  completedAt: number | null;
  history: ProgressHistoryEntry[];
};

export type LauncherSnapshot = {
  minecraftRoot: string;
  launcherAvailable: boolean;
  activeAccount?: ActiveLauncherAccount | null;
  launcherAccounts: LauncherAccountEntry[];
  profiles: LauncherProfile[];
  summary: LauncherSummary;
};

export type ActiveLauncherAccount = {
  localId: string;
  username: string;
  authSource: string;
  hasJavaAccess: boolean;
};

export type LauncherAccountEntry = {
  localId: string;
  username: string;
  gamerTag?: string | null;
  xuid?: string | null;
  microsoftUsername?: string | null;
  authSource: string;
  hasJavaAccess: boolean;
  isActive: boolean;
  isSelectable: boolean;
  canLogout: boolean;
};

export type LauncherSummary = {
  profileCount: number;
  modCount: number;
  enabledModCount: number;
  disabledModCount: number;
};

export type LauncherProfile = {
  id: string;
  name: string;
  profileType: string;
  icon?: string | null;
  customIconUrl?: string | null;
  backgroundImageUrl?: string | null;
  lastUsed?: string | null;
  lastVersionId?: string | null;
  gameDir: string;
  gameVersion?: string | null;
  loader: string;
  loaderVersion?: string | null;
  modpackProjectId?: string | null;
  modpackVersionId?: string | null;
  launchActive: boolean;
  modCount: number;
  enabledModCount: number;
  disabledModCount: number;
  mods: InstalledMod[];
};

export type InstalledMod = {
  fileName: string;
  displayName: string;
  sourceProjectId?: string | null;
  modId?: string | null;
  version?: string | null;
  description?: string | null;
  loader?: string | null;
  authors: string[];
  enabled: boolean;
  sizeBytes: number;
  modifiedAt?: number | null;
  /** JAR から抽出したアイコンの base64 Data URI */
  iconData?: string | null;
};

export type ModRemoteState = {
  fileName: string;
  projectId: string;
  source: "modrinth" | "curseforge";
  projectTitle?: string | null;
  projectUrl?: string | null;
  iconUrl?: string | null;
  latestVersion?: string | null;
  latestFileName?: string | null;
  publishedAt?: string | null;
  updateAvailable: boolean;
  canUpdate: boolean;
};

export type ModrinthProject = {
  projectId: string;
  source: "modrinth" | "curseforge";
  slug: string;
  title: string;
  author: string;
  description: string;
  downloads: number;
  followers: number;
  categories: string[];
  versions: string[];
  iconUrl?: string | null;
  imageUrl?: string | null;
  latestVersion?: string | null;
  updatedAt?: string | null;
  clientSide?: string | null;
  serverSide?: string | null;
  projectUrl: string;
};

export type InstallResult = {
  message: string;
  fileName: string;
  versionName: string;
};

export type ModpackInstallResult = {
  message: string;
  profileId: string;
  profileName: string;
  versionName: string;
};

export type ModpackExportResult = {
  message: string;
  filePath: string;
  bytes: number;
};

export type ModpackExportFormat = "curseforge" | "modrinth";

export type ModpackVersionSummary = {
  id: string;
  name: string;
  versionNumber: string;
  gameVersions: string[];
  publishedAt?: string | null;
};

export type ActionResult = {
  message: string;
  fileName: string;
};

export type LocalModDependency = {
  modId: string;
  requirement: string;
  required: boolean;
  satisfied: boolean;
  note: string;
};

export type LocalModAnalysis = {
  filePath: string;
  fileName: string;
  displayName: string;
  modId?: string | null;
  version?: string | null;
  description?: string | null;
  loader?: string | null;
  authors: string[];
  compatible: boolean;
  action: "install" | "replace" | "skip" | "reject" | string;
  severity: "ok" | "warning" | "error" | string;
  summary: string;
  dependencies: LocalModDependency[];
  existingFileName?: string | null;
  existingVersion?: string | null;
  /** JAR から抽出したアイコンの base64 Data URI */
  iconData?: string | null;
};

export type LaunchResult = {
  message: string;
  launchMode: string;
};

export type XboxRpsStateResult = {
  message: string;
  statePath: string;
  usedSavedState: boolean;
  refreshed: boolean;
  succeeded: boolean;
  attemptsTried: number;
  totalAttempts: number;
  sourcePath?: string | null;
  variantLabel?: string | null;
};

export type XboxSignInResult = {
  message: string;
  openedXboxSignIn: boolean;
  refreshed: boolean;
  succeeded: boolean;
  accountCount: number;
};

export type LoaderVersionSummary = {
  id: string;
  stable: boolean;
};

export type MinecraftVersionSummary = {
  id: string;
  stable: boolean;
  kind: string;
};

export type FabricCatalog = {
  minecraftVersion: string;
  latestInstaller: LoaderVersionSummary;
  recommendedLoader: LoaderVersionSummary;
  availableGameVersions: MinecraftVersionSummary[];
  availableLoaderVersions: LoaderVersionSummary[];
};

export type FabricInstallResult = {
  message: string;
  profileId: string;
  profileName: string;
  versionId: string;
  minecraftVersion: string;
  loaderVersion: string;
};

export type LoaderCatalog = {
  loader: LoaderId;
  minecraftVersion: string;
  installerVersion: LoaderVersionSummary | null;
  recommendedLoader: LoaderVersionSummary | null;
  availableGameVersions: MinecraftVersionSummary[];
  availableLoaderVersions: LoaderVersionSummary[];
};

export type LoaderInstallResult = {
  message: string;
  loader: LoaderId;
  profileId: string;
  profileName: string;
  versionId: string;
  minecraftVersion: string;
  loaderVersion: string;
};

export type NavigationItem = {
  id: ViewMode;
  label: string;
  kicker: string;
};

export type AppSettings = {
  tempCacheEnabled: boolean;
  performanceLiteMode: "auto" | "on" | "off";
  customJavaPath?: string | null;
  offlineModeEnabled?: boolean;
  offlineUsername?: string | null;
  officialLauncherAutoInstall?: boolean;
};

export type SoftwareStatus = {
  tempRoot: string;
  cacheDir: string;
  settingsPath: string;
  javaRuntimeDir: string;
  customJavaPath?: string | null;
  officialLauncherAvailable?: boolean;
  officialLauncherInstaller?: string;
  tempCacheEnabled: boolean;
  cacheFileCount: number;
  cacheTotalBytes: number;
  debugExportDir: string;
};

export type DebugExportResult = {
  filePath: string;
  bytes: number;
};

export type LoaderGuide = {
  id: LoaderId;
  name: string;
  kicker: string;
  description: string;
  detail: string;
  url: string;
  automation: "full" | "guide";
};
