import { startTransition, useCallback, useEffect, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { listen } from "@tauri-apps/api/event";
import { launcherApi } from "./app/api";
import { loaderGuides, navigationItems } from "./app/constants";
import {
  defaultLoaderProfileName,
  errorMessage,
} from "./app/formatters";
import { getProjectInstallState } from "./app/modMatching";
import type {
  AppSettings,
  DebugExportResult,
  InstalledMod,
  LoaderCatalog,
  LoaderId,
  LauncherSnapshot,
  ModRemoteState,
  ModpackExportFormat,
  ModpackExportResult,
  ModrinthProject,
  Notice,
  ProgressState,
  SoftwareStatus,
  ViewMode,
} from "./app/types";
import { HeaderBar } from "./components/HeaderBar";
import { HeroPanel } from "./components/HeroPanel";
import { NotificationCenter } from "./components/NotificationCenter";
import { Sidebar } from "./components/Sidebar";
import { AppMainContent } from "./features/app-shell/AppMainContent";
import { AppModals } from "./features/app-shell/AppModals";
import type {
  ConfirmDialogState,
  ModpackExportDialogState,
  ModpackVersionDialogState,
  ProfileNameDialogState,
  ProfileVisualDialogState,
} from "./features/app-shell/types";
import {
  COMPACT_SIDEBAR_MEDIA,
  LOW_END_POLL_MS,
  NORMAL_POLL_MS,
  createNoticeId,
  createOperationId,
  normalizeDiscoverQuery,
  shouldUsePerformanceLiteMode,
} from "./features/app-shell/utils";
import "./App.css";
import "./styles/hero-panel.css";
import "./styles/modals.css";
import "./styles/play-profile.css";
import "./styles/scrollbars.css";

type ModRemoteStateCacheEntry = {
  modSignature: string;
  visualStates: Record<string, ModRemoteState>;
  remoteStates: Record<string, ModRemoteState>;
  checkedAt: number;
};

const MOD_UPDATE_CACHE_WINDOW_MS = 5 * 60 * 1000;

function App() {
  const [snapshot, setSnapshot] = useState<LauncherSnapshot | null>(null);
  const [selectedProfileId, setSelectedProfileId] = useState("");
  const [activeView, setActiveView] = useState<ViewMode>("play");
  const [notices, setNotices] = useState<Notice[]>([]);
  const [progressItems, setProgressItems] = useState<ProgressState[]>([]);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [isCompactSidebar, setIsCompactSidebar] = useState(() =>
    typeof window === "undefined"
      ? false
      : window.matchMedia(COMPACT_SIDEBAR_MEDIA).matches,
  );
  const [sidebarOpen, setSidebarOpen] = useState(false);

  useEffect(() => {
    const handleContextMenu = (event: MouseEvent) => {
      event.preventDefault();
    };

    document.addEventListener("contextmenu", handleContextMenu);

    return () => {
      document.removeEventListener("contextmenu", handleContextMenu);
    };
  }, []);

  const [searchQuery, setSearchQuery] = useState("");
  const [discoverMode, setDiscoverMode] = useState<"mods" | "modpacks">("mods");
  const [searchResults, setSearchResults] = useState<ModrinthProject[]>([]);
  const [searching, setSearching] = useState(false);
  const [modVisualStateMap, setModVisualStateMap] = useState<Record<string, ModRemoteState>>({});
  const [modRemoteStateMap, setModRemoteStateMap] = useState<Record<string, ModRemoteState>>({});
  const [loadingModRemoteStates, setLoadingModRemoteStates] = useState(false);
  const [modUpdateLastCheckedAt, setModUpdateLastCheckedAt] = useState<number | null>(null);
  const [modRemoteStateCacheMap, setModRemoteStateCacheMap] =
    useState<Record<string, ModRemoteStateCacheEntry>>({});
  const modRemoteFetchTokenRef = useRef(0);

  const [activeLoader, setActiveLoader] = useState<LoaderId>("fabric");
  const [loaderCatalog, setLoaderCatalog] = useState<LoaderCatalog | null>(null);
  const [loadingLoaderCatalog, setLoadingLoaderCatalog] = useState(false);
  const [selectedLoaderGameVersion, setSelectedLoaderGameVersion] = useState("");
  const [selectedLoaderVersion, setSelectedLoaderVersion] = useState("");
  const [loaderProfileName, setLoaderProfileName] = useState("");
  const [loaderNameTouched, setLoaderNameTouched] = useState(false);
  const [confirmDialog, setConfirmDialog] = useState<ConfirmDialogState | null>(null);
  const [profileVisualDialog, setProfileVisualDialog] = useState<ProfileVisualDialogState | null>(null);
  const [profileNameDialog, setProfileNameDialog] = useState<ProfileNameDialogState | null>(null);
  const [modpackVersionDialog, setModpackVersionDialog] = useState<ModpackVersionDialogState | null>(null);
  const [modpackExportDialog, setModpackExportDialog] = useState<ModpackExportDialogState | null>(null);
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  const [softwareStatus, setSoftwareStatus] = useState<SoftwareStatus | null>(null);
  const [loadingSettings, setLoadingSettings] = useState(false);
  const [autoPerformanceLite, setAutoPerformanceLite] = useState(() =>
    shouldUsePerformanceLiteMode(),
  );
  const performanceLiteMode = appSettings?.performanceLiteMode ?? "auto";
  const performanceLite =
    performanceLiteMode === "on"
      ? true
      : performanceLiteMode === "off"
        ? false
        : autoPerformanceLite;

  const selectedProfile =
    snapshot?.profiles.find((profile) => profile.id === selectedProfileId) ??
    snapshot?.profiles[0] ??
    null;
  const selectedProfileModSignature =
    selectedProfile?.mods
      .map(
        (mod) =>
          `${mod.fileName}:${mod.version ?? ""}:${mod.sourceProjectId ?? ""}:${mod.modifiedAt ?? ""}`,
      )
      .join("|") ?? "";
  const mergedModStateMap = Object.fromEntries(
    Array.from(
      new Set([
        ...Object.keys(modVisualStateMap),
        ...Object.keys(modRemoteStateMap),
      ]),
    ).map((fileName) => [
      fileName,
      {
        ...(modVisualStateMap[fileName] ?? {}),
        ...(modRemoteStateMap[fileName] ?? {}),
      } as ModRemoteState,
    ]),
  );

  function pushNotice(tone: Notice["tone"], text: string) {
    setNotices((current) => [
      ...current.slice(-3),
      {
        id: createNoticeId(),
        tone,
        text,
      },
    ]);
  }

  const dismissNotice = useCallback((noticeId: string) => {
    setNotices((current) => current.filter((notice) => notice.id !== noticeId));
  }, []);

  function upsertProgress(progress: ProgressState) {
    setProgressItems((current) => {
      const next = current.filter((item) => item.operationId !== progress.operationId);
      return [...next, progress];
    });
  }

  function clearProgress(operationId: string) {
    setProgressItems((current) =>
      current.filter((item) => item.operationId !== operationId),
    );
  }

  function scheduleProgressClear(operationId: string) {
    window.setTimeout(() => {
      clearProgress(operationId);
    }, 680);
  }

  async function ensureXboxAuthStateBeforeLaunch() {
    const operationId = createOperationId("xbox-auth-check");
    upsertProgress({
      operationId,
      title: "Xbox 認証確認",
      detail: "確認を開始しています (0/0)",
      percent: 0,
    });

    try {
      const stateResult = await launcherApi.ensureXboxRpsState(operationId);
      const tried = stateResult.attemptsTried;
      const total = stateResult.totalAttempts;

      if (stateResult.succeeded) {
        if (stateResult.refreshed) {
          pushNotice(
            "success",
            `Xbox 認証状態を更新しました。試行 ${tried}/${total} で起動準備が完了しました。`,
          );
        } else {
          pushNotice(
            "info",
            `保存済みの Xbox 認証状態を利用して起動します。試行 ${tried}/${total}。`,
          );
        }
      } else {
        pushNotice(
          "info",
          `Xbox 認証状態の更新が未完了のため、既存情報で起動を試みます。試行 ${tried}/${total}。`,
        );
      }

      return stateResult;
    } finally {
      scheduleProgressClear(operationId);
    }
  }

  useEffect(() => {
    void refreshLauncher();
    void refreshSettingsState();
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") {
      return undefined;
    }

    const mediaQuery = window.matchMedia(COMPACT_SIDEBAR_MEDIA);

    function syncCompactSidebar(matches: boolean) {
      setIsCompactSidebar(matches);

      if (!matches) {
        setSidebarOpen(false);
      }
    }

    syncCompactSidebar(mediaQuery.matches);

    const handleChange = (event: MediaQueryListEvent) => {
      syncCompactSidebar(event.matches);
    };

    mediaQuery.addEventListener("change", handleChange);

    return () => {
      mediaQuery.removeEventListener("change", handleChange);
    };
  }, []);

  useEffect(() => {
    if (!isCompactSidebar || !sidebarOpen) {
      return undefined;
    }

    const previousOverflow = document.body.style.overflow;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setSidebarOpen(false);
      }
    };

    document.body.style.overflow = "hidden";
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [isCompactSidebar, sidebarOpen]);

  useEffect(() => {
    const unlistenPromise = listen<ProgressState>("launcher-progress", (event) => {
      upsertProgress(event.payload);
    });

    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    void loadLoaderCatalog(activeLoader, selectedProfile?.gameVersion ?? undefined);
  }, [activeLoader, selectedProfile?.gameVersion]);

  useEffect(() => {
    setLoaderNameTouched(false);
  }, [selectedProfile?.id, activeLoader]);

  useEffect(() => {
    setSearchResults([]);
  }, [selectedProfile?.id]);

  useEffect(() => {
    setSearchResults([]);
  }, [discoverMode]);

  useEffect(() => {
    if (!selectedProfile) {
      if (!loaderNameTouched) {
        setLoaderProfileName(
          defaultLoaderProfileName(activeLoader, null, selectedLoaderGameVersion),
        );
      }
      return;
    }

    if (!loaderNameTouched) {
      setLoaderProfileName(
        defaultLoaderProfileName(activeLoader, selectedProfile, selectedLoaderGameVersion),
      );
    }
  }, [activeLoader, selectedLoaderGameVersion, selectedProfile, loaderNameTouched]);

  useEffect(() => {
    if (activeView !== "discover") {
      return;
    }

    if (discoverMode === "mods" && !selectedProfile) {
      return;
    }

    if (normalizeDiscoverQuery(searchQuery) !== "") {
      return;
    }

    if (performanceLite) {
      return;
    }

    void handleSearch();
  }, [activeView, discoverMode, selectedProfile?.id, performanceLite]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return undefined;
    }

    const handleConnectionChange = () => {
      setAutoPerformanceLite(shouldUsePerformanceLiteMode());
    };

    const nav = navigator as Navigator & {
      connection?: { addEventListener?: (event: string, handler: () => void) => void; removeEventListener?: (event: string, handler: () => void) => void };
    };
    const connection = nav.connection;

    connection?.addEventListener?.("change", handleConnectionChange);

    return () => {
      connection?.removeEventListener?.("change", handleConnectionChange);
    };
  }, []);

  useEffect(() => {
    const hasActiveLaunch = snapshot?.profiles.some((profile) => profile.launchActive) ?? false;
    if (!hasActiveLaunch) {
      return undefined;
    }

    const intervalId = window.setInterval(() => {
      void refreshLauncher(selectedProfile?.id);
    }, performanceLite ? LOW_END_POLL_MS : NORMAL_POLL_MS);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [snapshot, selectedProfile?.id, performanceLite]);

  useEffect(() => {
    if (activeView !== "mods" || !selectedProfile) {
      setLoadingModRemoteStates(false);
      setModVisualStateMap({});
      setModRemoteStateMap({});
      setModUpdateLastCheckedAt(null);
      return;
    }

    const cached = modRemoteStateCacheMap[selectedProfile.id];
    if (cached && cached.modSignature === selectedProfileModSignature) {
      setModVisualStateMap(cached.visualStates);
      setModRemoteStateMap(cached.remoteStates);
      setModUpdateLastCheckedAt(cached.checkedAt);
    } else {
      setModVisualStateMap({});
      setModRemoteStateMap({});
      setModUpdateLastCheckedAt(null);
    }

    setLoadingModRemoteStates(false);
  }, [
    activeView,
    selectedProfile?.id,
    selectedProfileModSignature,
    modRemoteStateCacheMap,
  ]);

  async function handleCheckModUpdates() {
    if (!selectedProfile) {
      return;
    }

    const currentProfile = selectedProfile;
    const trackedMods = currentProfile.mods.filter(
      (mod) => (mod.sourceProjectId ?? "").trim() !== "",
    );
    const cacheKey = currentProfile.id;
    const cached = modRemoteStateCacheMap[cacheKey];
    const now = Date.now();

    if (
      cached &&
      cached.modSignature === selectedProfileModSignature &&
      now - cached.checkedAt < MOD_UPDATE_CACHE_WINDOW_MS
    ) {
      setModVisualStateMap(cached.visualStates);
      setModRemoteStateMap(cached.remoteStates);
      setModUpdateLastCheckedAt(cached.checkedAt);
      pushNotice("info", "直近の更新チェック結果を表示しています。必要なら数分後に再確認してください。");
      return;
    }

    if (trackedMods.length === 0) {
      setModVisualStateMap({});
      setModRemoteStateMap({});
      setModUpdateLastCheckedAt(now);
      setModRemoteStateCacheMap((current) => ({
        ...current,
        [cacheKey]: {
          modSignature: selectedProfileModSignature,
          visualStates: {},
          remoteStates: {},
          checkedAt: now,
        },
      }));
      pushNotice("info", "更新チェック対象の Mod はありません。" );
      return;
    }

    const fetchToken = modRemoteFetchTokenRef.current + 1;
    modRemoteFetchTokenRef.current = fetchToken;
    setBusyAction("check-mod-updates");
    setLoadingModRemoteStates(true);

    try {
      const visualStates: Record<string, ModRemoteState> = {};
      await Promise.all(
        trackedMods.map(async (mod) => {
          try {
            const state = await launcherApi.getProfileModVisualState(currentProfile.id, mod.fileName);
            if (state) {
              visualStates[state.fileName] = state;
            }
          } catch {
            // アイコン取得失敗時はグリフ表示のまま進める。
          }
        }),
      );

      const remoteStates: Record<string, ModRemoteState> = {};
      let failedCount = 0;
      const updateQueue = [...trackedMods];
      const workerCount = Math.min(4, trackedMods.length);

      await Promise.allSettled(
        Array.from({ length: workerCount }, async () => {
          while (true) {
            const mod = updateQueue.shift();
            if (!mod) {
              return;
            }

            try {
              const state = await launcherApi.getProfileModRemoteState(currentProfile.id, mod.fileName);
              if (state) {
                remoteStates[state.fileName] = state;
              }
            } catch {
              failedCount += 1;
            }
          }
        }),
      );

      if (modRemoteFetchTokenRef.current !== fetchToken) {
        return;
      }

      const checkedAt = Date.now();
      setModVisualStateMap(visualStates);
      setModRemoteStateMap(remoteStates);
      setModUpdateLastCheckedAt(checkedAt);
      setModRemoteStateCacheMap((current) => ({
        ...current,
        [cacheKey]: {
          modSignature: selectedProfileModSignature,
          visualStates,
          remoteStates,
          checkedAt,
        },
      }));

      if (failedCount === trackedMods.length) {
        pushNotice("error", "Mod の更新情報を取得できませんでした。");
      } else {
        const updatableCount = Object.values(remoteStates).filter((state) => state.updateAvailable).length;
        pushNotice("info", `更新チェックが完了しました。${updatableCount} 件が更新可能です。`);
      }
    } finally {
      if (modRemoteFetchTokenRef.current === fetchToken) {
        setLoadingModRemoteStates(false);
        setBusyAction((current) => (current === "check-mod-updates" ? null : current));
      }
    }
  }

  async function refreshLauncher(preferredProfileId?: string) {
    try {
      const nextSnapshot = await launcherApi.getLauncherState();
      const nextProfile =
        nextSnapshot.profiles.find((profile) => profile.id === preferredProfileId) ??
        nextSnapshot.profiles.find((profile) => profile.id === selectedProfileId) ??
        nextSnapshot.profiles[0];

      startTransition(() => {
        setSnapshot(nextSnapshot);
        setSelectedProfileId(nextProfile?.id ?? "");
      });
    } catch (error) {
      pushNotice(
        "error",
        errorMessage(error, "Minecraft Launcher の状態を読み込めませんでした。"),
      );
    }
  }

  async function refreshSettingsState() {
    setLoadingSettings(true);
    try {
      const [settings, status] = await Promise.all([
        launcherApi.getAppSettings(),
        launcherApi.getSoftwareStatus(),
      ]);
      setAppSettings(settings);
      setSoftwareStatus(status);
    } catch (error) {
      pushNotice("error", errorMessage(error, "設定の取得に失敗しました。"));
    } finally {
      setLoadingSettings(false);
    }
  }

  async function loadLoaderCatalog(loader: LoaderId, preferredGameVersion?: string) {
    setLoadingLoaderCatalog(true);

    try {
      const catalog = await launcherApi.getLoaderCatalog(loader, preferredGameVersion ?? null);
      setLoaderCatalog(catalog);
      setSelectedLoaderGameVersion(catalog.minecraftVersion);
      setSelectedLoaderVersion((currentValue) => {
        if (
          currentValue &&
          catalog.availableLoaderVersions.some((entry) => entry.id === currentValue)
        ) {
          return currentValue;
        }

        return catalog.recommendedLoader.id;
      });
    } catch (error) {
      pushNotice(
        "error",
        errorMessage(
          error,
          `${loaderGuides.find((guide) => guide.id === loader)?.name ?? "Loader"} の導入情報を取得できませんでした。`,
        ),
      );
    } finally {
      setLoadingLoaderCatalog(false);
    }
  }

  async function handleSearch() {
    const normalizedQuery = normalizeDiscoverQuery(searchQuery);
    if (discoverMode === "mods" && !selectedProfile) {
      return;
    }

    setSearching(true);

    try {
      const results =
        discoverMode === "modpacks"
          ? await launcherApi.searchModpacks(normalizedQuery)
          : await launcherApi.searchMods(
              normalizedQuery,
              selectedProfile?.loader,
              selectedProfile?.gameVersion,
            );
      setSearchResults(results);
      setActiveView("discover");

      if (results.length === 0) {
        pushNotice(
          "info",
          normalizedQuery === ""
            ? discoverMode === "modpacks"
              ? "おすすめの Modpack を表示できませんでした。具体的なキーワードでも探せます。"
              : "おすすめ候補を表示できませんでした。条件を変えるか、具体的なキーワードでも探せます。"
            : discoverMode === "modpacks"
              ? "条件に合う Modpack が見つかりませんでした。"
              : "互換条件に合う Mod が見つかりませんでした。",
        );
      }
    } catch (error) {
      pushNotice(
        "error",
        errorMessage(
          error,
          discoverMode === "modpacks"
            ? "Modrinth の Modpack 検索に失敗しました。"
            : "Mod 検索に失敗しました。",
        ),
      );
    } finally {
      setSearching(false);
    }
  }

  async function executeProjectAction(
    project: ModrinthProject,
    installedMod: InstalledMod | null,
  ) {
    if (!selectedProfile) {
      return;
    }

    const installed = installedMod !== null;
    const actionKey = installed ? "uninstall" : "install";

    if (!installed && selectedProfile.loader === "vanilla") {
      pushNotice(
        "info",
        "Vanilla 構成には Mod を直接入れられません。先に Fabric / Forge / NeoForge / Quilt を導入してください。",
      );
      return;
    }

    setBusyAction(`${actionKey}:${project.projectId}`);
    const operationId = createOperationId(actionKey);

    try {
      const result = installed
        ? installedMod?.sourceProjectId
          ? await launcherApi.uninstallProject(selectedProfile.id, project.projectId)
          : await launcherApi.removeMod(selectedProfile.id, installedMod!.fileName)
        : await launcherApi.installProject(
            selectedProfile.id,
            project.projectId,
            operationId,
          );
      pushNotice(
        "success",
        "versionName" in result
          ? `${result.message} バージョン ${result.versionName}。`
          : result.message,
      );
      await refreshLauncher(selectedProfile.id);
    } catch (error) {
      pushNotice(
        "error",
        errorMessage(
          error,
          installed ? "Mod を削除できませんでした。" : "Mod を導入できませんでした。",
        ),
      );
    } finally {
      scheduleProgressClear(operationId);
      setBusyAction(null);
    }
  }

  async function handleProjectAction(project: ModrinthProject) {
    if (discoverMode === "modpacks") {
      try {
        setBusyAction(`modpack-versions:${project.projectId}`);
        const versions = await launcherApi.getModpackVersions(project.projectId);
        if (versions.length === 0) {
          pushNotice("info", "選択できる Modpack バージョンが見つかりませんでした。");
          return;
        }

        setModpackVersionDialog({
          project,
          versions,
          selectedVersionId: versions[0].id,
        });
      } catch (error) {
        pushNotice("error", errorMessage(error, "Modpack の構成作成に失敗しました。"));
      } finally {
        setBusyAction(null);
      }
      return;
    }

    if (!selectedProfile) {
      return;
    }

    const { installedMod, state } = getProjectInstallState(selectedProfile, project);

    if (state === "installed") {
      pushNotice("info", `${project.title} はすでに導入済みです。`);
      return;
    }

    if (state === "blocked") {
      pushNotice(
        "info",
        `${project.title} は既存の Mod と重複しています。My Mods で整理してから導入してください。`,
      );
      return;
    }

    if (
      state === "update" &&
      (installedMod?.sourceProjectId === project.projectId ||
        installedMod?.sourceProjectId === `modrinth:${project.projectId}`)
    ) {
      await handleUpdate(installedMod);
      return;
    }

    await executeProjectAction(project, null);
  }

  async function handleUpdateModpackProfile(profileId: string) {
    const target = snapshot?.profiles.find((entry) => entry.id === profileId);
    if (!target?.modpackProjectId) {
      pushNotice("info", "この構成は Modpack 更新対象ではありません。");
      return;
    }

    setBusyAction(`modpack-update:${profileId}`);
    const operationId = createOperationId("modpack-update");

    try {
      const result = await launcherApi.updateModpackProfile(
        profileId,
        target.gameVersion ?? null,
        operationId,
      );
      pushNotice("success", `${result.message} バージョン ${result.versionName}。`);
      await refreshLauncher(profileId);
    } catch (error) {
      pushNotice("error", errorMessage(error, "Modpack の更新に失敗しました。"));
    } finally {
      scheduleProgressClear(operationId);
      setBusyAction(null);
    }
  }

  async function handleConfirmModpackVersionInstall() {
    if (!modpackVersionDialog) {
      return;
    }

    const { project, selectedVersionId } = modpackVersionDialog;
    if (!selectedVersionId) {
      pushNotice("info", "導入する Modpack バージョンを選択してください。");
      return;
    }

    setModpackVersionDialog(null);
    setBusyAction(`modpack:${project.projectId}`);
    const operationId = createOperationId("modpack");

    try {
      const result = await launcherApi.installModpack(
        project.projectId,
        selectedVersionId,
        operationId,
        project.iconUrl ?? null,
        project.imageUrl ?? null,
      );
      pushNotice("success", `${result.message} バージョン ${result.versionName}。`);
      await refreshLauncher(result.profileId);
      setActiveView("play");
    } catch (error) {
      pushNotice("error", errorMessage(error, "Modpack の構成作成に失敗しました。"));
    } finally {
      scheduleProgressClear(operationId);
      setBusyAction(null);
    }
  }

  async function handleToggle(mod: InstalledMod) {
    if (!selectedProfile) {
      return;
    }

    setBusyAction(`toggle:${mod.fileName}`);

    try {
      const result = await launcherApi.setModEnabled(
        selectedProfile.id,
        mod.fileName,
        !mod.enabled,
      );
      pushNotice("success", result.message);
      await refreshLauncher(selectedProfile.id);
    } catch (error) {
      pushNotice("error", errorMessage(error, "Mod の状態を更新できませんでした。"));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleRemove(mod: InstalledMod) {
    if (!selectedProfile) {
      return;
    }

    setConfirmDialog({
      title: `${mod.displayName} を削除しますか？`,
      description: `${selectedProfile.name} の管理領域からこの Mod ファイルを削除します。元に戻すには再インストールが必要です。`,
      confirmLabel: "削除する",
      tone: "danger",
      onConfirm: async () => {
        setBusyAction(`remove:${mod.fileName}`);

        try {
          const result = await launcherApi.removeMod(selectedProfile.id, mod.fileName);
          pushNotice("success", result.message);
          await refreshLauncher(selectedProfile.id);
        } catch (error) {
          pushNotice("error", errorMessage(error, "Mod を削除できませんでした。"));
        } finally {
          setBusyAction(null);
        }
      },
    });
  }

  async function handleUpdate(mod: InstalledMod) {
    if (!selectedProfile || !mod.sourceProjectId) {
      return;
    }

    setBusyAction(`update:${mod.fileName}`);
    const operationId = createOperationId("update");

    try {
      const result = await launcherApi.updateProject(
        selectedProfile.id,
        mod.sourceProjectId,
        mod.fileName,
        operationId,
      );
      pushNotice(
        "success",
        `${mod.displayName} を更新しました。バージョン ${result.versionName}。`,
      );
      await refreshLauncher(selectedProfile.id);
    } catch (error) {
      pushNotice("error", errorMessage(error, "Mod の更新に失敗しました。"));
    } finally {
      scheduleProgressClear(operationId);
      setBusyAction(null);
    }
  }

  async function handleUpdateAllMods() {
    if (!selectedProfile) {
      return;
    }

    const updatableMods = selectedProfile.mods.filter((mod) => {
      if (!mod.sourceProjectId) {
        return false;
      }

      return modRemoteStateMap[mod.fileName]?.updateAvailable === true;
    });

    if (updatableMods.length === 0) {
      pushNotice("info", "更新可能な Mod は見つかりませんでした。");
      return;
    }

    setBusyAction("update-all-mods");
    let succeeded = 0;
    let failed = 0;

    try {
      for (const mod of updatableMods) {
        if (!mod.sourceProjectId) {
          continue;
        }

        const operationId = createOperationId("update-all");
        try {
          await launcherApi.updateProject(
            selectedProfile.id,
            mod.sourceProjectId,
            mod.fileName,
            operationId,
          );
          succeeded += 1;
        } catch {
          failed += 1;
        } finally {
          scheduleProgressClear(operationId);
        }
      }

      if (succeeded > 0) {
        await refreshLauncher(selectedProfile.id);
      }

      if (failed === 0) {
        pushNotice("success", `${succeeded} 件の Mod を更新しました。`);
      } else if (succeeded > 0) {
        pushNotice("info", `${succeeded} 件更新、${failed} 件は失敗しました。`);
      } else {
        pushNotice("error", "Mod の一括更新に失敗しました。");
      }
    } finally {
      setBusyAction(null);
    }
  }

  async function handleLaunchProfile() {
    if (!selectedProfile) {
      return;
    }

    if (selectedProfile.launchActive) {
      pushNotice("info", `${selectedProfile.name} はまだ起動中です。ゲームの立ち上がりを待ってください。`);
      return;
    }

    setBusyAction("launch");

    try {
      await ensureXboxAuthStateBeforeLaunch();
      const result = await launcherApi.launchProfileDirectly(selectedProfile.id);
      pushNotice("success", result.message);
      await refreshLauncher(selectedProfile.id);
    } catch (error) {
      pushNotice("error", errorMessage(error, "Minecraft Java を直接起動できませんでした。"));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleLaunchSpecificProfile(profileId: string) {
    if (selectedProfile?.id !== profileId) {
      setSelectedProfileId(profileId);
    }

    const profileName =
      snapshot?.profiles.find((entry) => entry.id === profileId)?.name ?? "この構成";

    setBusyAction("launch");

    try {
      await ensureXboxAuthStateBeforeLaunch();
      const result = await launcherApi.launchProfileDirectly(profileId);
      pushNotice("success", result.message);
      await refreshLauncher(profileId);
    } catch (error) {
      pushNotice(
        "error",
        errorMessage(error, `${profileName} を直接起動できませんでした。`),
      );
    } finally {
      setBusyAction(null);
    }
  }

  async function handleOpenOfficialLauncher() {
    if (!selectedProfile) {
      return;
    }

    setBusyAction("launcher");

    try {
      const result = await launcherApi.launchProfileInOfficialLauncher(selectedProfile.id);
      pushNotice("success", result.message);
    } catch (error) {
      pushNotice("error", errorMessage(error, "公式 Launcher を起動できませんでした。"));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleDeleteProfile() {
    if (!selectedProfile) {
      return;
    }

    if (selectedProfile.launchActive) {
      pushNotice("info", "この起動構成は起動中なので削除できません。ゲームを終了してからもう一度試してください。");
      return;
    }

    setConfirmDialog({
      title: `${selectedProfile.name} を削除しますか？`,
      description: "launcher_profiles.json からこの起動構成を削除します。mods 管理情報も一緒に消えますが、ゲーム本体のフォルダは削除しません。",
      confirmLabel: "削除する",
      tone: "danger",
      onConfirm: async () => {
        setBusyAction("delete-profile");

        try {
          const result = await launcherApi.deleteProfile(selectedProfile.id);
          pushNotice("success", result.message);
          await refreshLauncher();
        } catch (error) {
          pushNotice("error", errorMessage(error, "起動構成を削除できませんでした。"));
        } finally {
          setBusyAction(null);
        }
      },
    });
  }

  async function openSelectedPath(target: "game" | "mods") {
    if (!selectedProfile) {
      return;
    }

    try {
      const path = await launcherApi.resolveProfilePath(selectedProfile.id, target);
      await openPath(path);
    } catch (error) {
      pushNotice(
        "error",
        errorMessage(
          error,
          `${target === "game" ? "ゲーム" : "mods"} フォルダを開けませんでした。`,
        ),
      );
    }
  }

  async function handleToggleTempCache(enabled: boolean) {
    try {
      const result = await launcherApi.updateAppSettings(
        enabled,
        appSettings?.performanceLiteMode ?? "auto",
      );
      pushNotice("success", result.message);
      await refreshSettingsState();
    } catch (error) {
      pushNotice("error", errorMessage(error, "設定を更新できませんでした。"));
    }
  }

  async function handleChangePerformanceLiteMode(mode: AppSettings["performanceLiteMode"]) {
    try {
      const result = await launcherApi.updateAppSettings(
        appSettings?.tempCacheEnabled ?? true,
        mode,
      );
      pushNotice("success", result.message);
      await refreshSettingsState();
    } catch (error) {
      pushNotice("error", errorMessage(error, "軽量モード設定を更新できませんでした。"));
    }
  }

  async function handleEnsureJavaRuntime() {
    const operationId = createOperationId("java-runtime");
    setBusyAction("java-runtime");
    try {
      const result = await launcherApi.ensureJavaRuntimeAvailable(operationId);
      pushNotice("success", result.message);
      await refreshSettingsState();
    } catch (error) {
      pushNotice("error", errorMessage(error, "Java の確認または導入に失敗しました。"));
    } finally {
      scheduleProgressClear(operationId);
      setBusyAction(null);
    }
  }

  async function handleClearTempCache() {
    try {
      const result = await launcherApi.clearTempCache();
      pushNotice("success", result.message);
      await refreshSettingsState();
    } catch (error) {
      pushNotice("error", errorMessage(error, "Temp キャッシュをクリアできませんでした。"));
    }
  }

  async function handleExportDebugLog() {
    try {
      const result: DebugExportResult = await launcherApi.exportDebugLog();
      await openPath(result.filePath);
      pushNotice("success", `デバッグ情報を保存しました: ${result.filePath}`);
      await refreshSettingsState();
    } catch (error) {
      pushNotice("error", errorMessage(error, "デバッグ情報を出力できませんでした。"));
    }
  }

  async function handleImportLocalModpack() {
    const selected = await open({
      title: "取り込む Modpack を選択",
      multiple: false,
      filters: [{ name: "Modpack Archive", extensions: ["mrpack", "zip"] }],
    });
    if (!selected || Array.isArray(selected)) {
      pushNotice("info", "取り込みをキャンセルしました。");
      return;
    }
    const operationId = createOperationId("modpack-import");
    setBusyAction("modpack-import");

    try {
      const result = await launcherApi.importLocalModpack(selected, null, operationId);
      pushNotice("success", `${result.message} バージョン ${result.versionName}。`);
      await refreshLauncher(result.profileId);
      setActiveView("play");
    } catch (error) {
      pushNotice("error", errorMessage(error, "ローカル Modpack の取り込みに失敗しました。"));
    } finally {
      scheduleProgressClear(operationId);
      setBusyAction(null);
    }
  }

  async function handleExportProfileModpack(profileId: string) {
    const target = snapshot?.profiles.find((entry) => entry.id === profileId);
    if (!target) {
      return;
    }

    setModpackExportDialog({
      profileId,
      profileName: target.name,
      selectedFormat: "curseforge",
    });
  }

  async function handleConfirmModpackExport() {
    if (!modpackExportDialog) {
      return;
    }

    const { profileId, profileName, selectedFormat } = modpackExportDialog;
    const suggested =
      profileName
        .replace(/[\\/:*?"<>|]/g, "_")
        .trim()
        .replace(/\s+/g, "_") || "modpack";
    const extension = selectedFormat === "modrinth" ? "mrpack" : "zip";
    const filterName = selectedFormat === "modrinth" ? "Modrinth Modpack" : "CurseForge Modpack";
    const selected = await save({
      title: "Modpack の保存先を選択",
      defaultPath: `${suggested}.${extension}`,
      filters: [{ name: filterName, extensions: [extension] }],
    });
    if (!selected) {
      pushNotice("info", "書き出しをキャンセルしました。");
      return;
    }

    setBusyAction(`modpack-export:${profileId}`);
    try {
      const result: ModpackExportResult = await launcherApi.exportProfileModpack(
        profileId,
        selected,
        selectedFormat,
      );
      await openPath(result.filePath);
      pushNotice("success", `${result.message} 保存先: ${result.filePath}`);
      setModpackExportDialog(null);
      await refreshSettingsState();
    } catch (error) {
      pushNotice("error", errorMessage(error, "Modpack のエクスポートに失敗しました。"));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleOpenTempRoot() {
    if (!softwareStatus) {
      return;
    }

    try {
      await openPath(softwareStatus.tempRoot);
    } catch (error) {
      pushNotice("error", errorMessage(error, "Temp フォルダを開けませんでした。"));
    }
  }

  async function handleOpenModSource(mod: InstalledMod, remoteState?: ModRemoteState) {
    if (!selectedProfile) {
      return;
    }

    if (
      remoteState?.source === "curseforge" ||
      mod.sourceProjectId?.startsWith("curseforge:")
    ) {
      try {
        const path = await launcherApi.resolveProfilePath(selectedProfile.id, "mods");
        await openPath(path);
        pushNotice("info", `${mod.displayName} が入っている mods フォルダを開きました。`);
      } catch (error) {
        pushNotice("error", errorMessage(error, "Mod の保存場所を開けませんでした。"));
      }
      return;
    }

    if (remoteState?.projectUrl) {
      await handleOpenGuide(remoteState.projectUrl);
      return;
    }

    if (remoteState?.source === "modrinth" && remoteState?.projectId) {
      await handleOpenGuide(`https://modrinth.com/project/${remoteState.projectId}`);
      return;
    }

    try {
      const path = await launcherApi.resolveProfilePath(selectedProfile.id, "mods");
      await openPath(path);
      pushNotice("info", `${mod.displayName} が入っている mods フォルダを開きました。`);
    } catch (error) {
      pushNotice("error", errorMessage(error, "Mod の保存場所を開けませんでした。"));
    }
  }

  async function handleInstallLoader() {
    if (!selectedLoaderGameVersion) {
      return;
    }

    setBusyAction("loader-install");
    const operationId = createOperationId(`${activeLoader}-install`);

    try {
      const result = await launcherApi.installLoader(
        activeLoader,
        selectedProfile?.id,
        selectedLoaderGameVersion,
        selectedLoaderVersion,
        loaderProfileName.trim() || undefined,
        operationId,
      );
      pushNotice("success", result.message);
      setLoaderNameTouched(false);
      await refreshLauncher(result.profileId);
      setActiveView("play");
    } catch (error) {
      pushNotice(
        "error",
        errorMessage(
          error,
          `${loaderGuides.find((guide) => guide.id === activeLoader)?.name ?? "Loader"} の導入に失敗しました。`,
        ),
      );
    } finally {
      scheduleProgressClear(operationId);
      setBusyAction(null);
    }
  }

  async function handleOpenGuide(url: string) {
    try {
      await openUrl(url);
    } catch (error) {
      pushNotice("error", errorMessage(error, "外部ページを開けませんでした。"));
    }
  }

  async function handleCustomizeProfileVisuals(profileId: string) {
    const targetProfile = snapshot?.profiles.find((entry) => entry.id === profileId);
    if (!targetProfile) {
      return;
    }

    setProfileVisualDialog({
      profileId,
      profileName: targetProfile.name,
      iconUrl: targetProfile.customIconUrl ?? "",
      backgroundImageUrl: targetProfile.backgroundImageUrl ?? "",
    });
  }

  async function handleConfirmProfileVisuals() {
    if (!profileVisualDialog) {
      return;
    }

    const { profileId, iconUrl, backgroundImageUrl } = profileVisualDialog;

    try {
      const result = await launcherApi.updateProfileVisuals(
        profileId,
        iconUrl.trim() === "" ? null : iconUrl.trim(),
        backgroundImageUrl.trim() === "" ? null : backgroundImageUrl.trim(),
      );
      pushNotice("success", result.message);
      setProfileVisualDialog(null);
      await refreshLauncher(profileId);
    } catch (error) {
      pushNotice("error", errorMessage(error, "起動構成の外観を更新できませんでした。"));
    }
  }

  function handleOpenProfileNameDialog() {
    if (!selectedProfile) {
      return;
    }

    setProfileNameDialog({
      profileId: selectedProfile.id,
      draftName: selectedProfile.name,
    });
  }

  async function handleConfirmProfileName() {
    if (!profileNameDialog) {
      return;
    }

    const nextName = profileNameDialog.draftName.trim();
    if (nextName === "") {
      pushNotice("info", "起動構成名を入力してください。");
      return;
    }

    try {
      const result = await launcherApi.updateProfileName(profileNameDialog.profileId, nextName);
      pushNotice("success", result.message);
      setProfileNameDialog(null);
      await refreshLauncher(profileNameDialog.profileId);
    } catch (error) {
      pushNotice("error", errorMessage(error, "起動構成名を更新できませんでした。"));
    }
  }

  async function handleConfirmDialog() {
    const currentDialog = confirmDialog;
    if (!currentDialog) {
      return;
    }

    setConfirmDialog(null);
    await currentDialog.onConfirm();
  }

  function handleSelectView(view: ViewMode) {
    setActiveView(view);

    if (isCompactSidebar) {
      setSidebarOpen(false);
    }
  }

  const launchBusy = busyAction === "launch" || selectedProfile?.launchActive === true;

  return (
    <div
      className={`app-shell ${isCompactSidebar ? "app-shell-compact" : ""} ${
        performanceLite ? "performance-lite" : ""
      }`}
    >
      <button
        type="button"
        className={`sidebar-backdrop ${isCompactSidebar && sidebarOpen ? "is-visible" : ""}`}
        onClick={() => setSidebarOpen(false)}
        aria-label="ナビゲーションを閉じる"
        aria-hidden={!isCompactSidebar || !sidebarOpen}
        tabIndex={!isCompactSidebar || !sidebarOpen ? -1 : 0}
      />

      <Sidebar
        activeView={activeView}
        items={navigationItems}
        snapshot={snapshot}
        compact={isCompactSidebar}
        open={sidebarOpen}
        onClose={() => setSidebarOpen(false)}
        onSelectView={handleSelectView}
      />

      <main className="main-column">
        <HeaderBar
          activeView={activeView}
          compactNavigation={isCompactSidebar}
          profiles={snapshot?.profiles ?? []}
          selectedProfileId={selectedProfile?.id ?? ""}
          sidebarOpen={sidebarOpen}
          onRefresh={() => void refreshLauncher()}
          onSelectProfile={setSelectedProfileId}
          onToggleSidebar={() => setSidebarOpen((current) => !current)}
        />

        <NotificationCenter
          notices={notices}
          progressItems={progressItems}
          onDismissNotice={dismissNotice}
        />

        <AppModals
          confirmDialog={confirmDialog}
          profileVisualDialog={profileVisualDialog}
          profileNameDialog={profileNameDialog}
          modpackVersionDialog={modpackVersionDialog}
          modpackExportDialog={modpackExportDialog}
          busyAction={busyAction}
          onCloseConfirmDialog={() => setConfirmDialog(null)}
          onConfirmDialog={() => void handleConfirmDialog()}
          onCloseProfileVisualDialog={() => setProfileVisualDialog(null)}
          onConfirmProfileVisuals={() => void handleConfirmProfileVisuals()}
          onChangeProfileVisualIconUrl={(value) => {
            setProfileVisualDialog((current) =>
              current ? { ...current, iconUrl: value } : current,
            );
          }}
          onChangeProfileVisualBackgroundImageUrl={(value) => {
            setProfileVisualDialog((current) =>
              current ? { ...current, backgroundImageUrl: value } : current,
            );
          }}
          onCloseProfileNameDialog={() => setProfileNameDialog(null)}
          onConfirmProfileName={() => void handleConfirmProfileName()}
          onChangeProfileName={(value) => {
            setProfileNameDialog((current) =>
              current ? { ...current, draftName: value } : current,
            );
          }}
          onCloseModpackVersionDialog={() => setModpackVersionDialog(null)}
          onConfirmModpackVersionInstall={() => void handleConfirmModpackVersionInstall()}
          onSelectModpackVersionId={(value) => {
            setModpackVersionDialog((current) =>
              current ? { ...current, selectedVersionId: value } : current,
            );
          }}
          onCloseModpackExportDialog={() => setModpackExportDialog(null)}
          onConfirmModpackExport={() => void handleConfirmModpackExport()}
          onSelectModpackExportFormat={(value) => {
            setModpackExportDialog((current) =>
              current ? { ...current, selectedFormat: value as ModpackExportFormat } : current,
            );
          }}
        />

        <HeroPanel
          profile={selectedProfile}
          activeAccount={snapshot?.activeAccount ?? null}
          launcherAvailable={snapshot?.launcherAvailable ?? false}
          busy={launchBusy || busyAction === "delete-profile"}
          openingLauncher={busyAction === "launcher"}
          onLaunch={() => void handleLaunchProfile()}
          onOpenOfficialLauncher={() => void handleOpenOfficialLauncher()}
          onOpenGameDir={() => void openSelectedPath("game")}
          onOpenModsDir={() => void openSelectedPath("mods")}
          onEditProfileName={handleOpenProfileNameDialog}
          onDeleteProfile={() => void handleDeleteProfile()}
        />

        <AppMainContent
          activeView={activeView}
          snapshot={snapshot}
          selectedProfile={selectedProfile}
          launchBusy={launchBusy}
          busyAction={busyAction}
          discoverMode={discoverMode}
          searchQuery={searchQuery}
          searching={searching}
          performanceLite={performanceLite}
          searchResults={searchResults}
          modRemoteStateMap={mergedModStateMap}
          loadingModRemoteStates={loadingModRemoteStates}
          modUpdateLastCheckedAt={modUpdateLastCheckedAt}
          activeLoader={activeLoader}
          loaderCatalog={loaderCatalog}
          loadingLoaderCatalog={loadingLoaderCatalog}
          selectedLoaderGameVersion={selectedLoaderGameVersion}
          selectedLoaderVersion={selectedLoaderVersion}
          loaderProfileName={loaderProfileName}
          appSettings={appSettings}
          softwareStatus={softwareStatus}
          loadingSettings={loadingSettings}
          onLaunchSpecificProfile={(profileId) => void handleLaunchSpecificProfile(profileId)}
          onOpenProfileMods={(profileId) => {
            setSelectedProfileId(profileId);
            setActiveView("mods");
          }}
          onUpdateModpackProfile={(profileId) => void handleUpdateModpackProfile(profileId)}
          onCustomizeProfileVisuals={(profileId) => void handleCustomizeProfileVisuals(profileId)}
          onImportLocalModpack={() => void handleImportLocalModpack()}
          onExportProfileModpack={(profileId) => void handleExportProfileModpack(profileId)}
          onToggleMod={(mod) => void handleToggle(mod)}
          onUpdateMod={(mod) => void handleUpdate(mod)}
          onUpdateAllMods={() => void handleUpdateAllMods()}
          onCheckModUpdates={() => void handleCheckModUpdates()}
          onRemoveMod={(mod) => void handleRemove(mod)}
          onOpenModSource={(mod, remoteState) => void handleOpenModSource(mod, remoteState)}
          onOpenGameDir={() => void openSelectedPath("game")}
          onOpenModsDir={() => void openSelectedPath("mods")}
          onChangeDiscoverMode={setDiscoverMode}
          onChangeSearchQuery={setSearchQuery}
          onSearch={() => void handleSearch()}
          onProjectAction={(project) => void handleProjectAction(project)}
          onOpenProject={(url) => void handleOpenGuide(url)}
          onSelectLoader={(loader) => {
            setActiveLoader(loader);
            setSelectedLoaderVersion("");
            setLoaderNameTouched(false);
          }}
          onChangeLoaderVersion={(value) => {
            setSelectedLoaderGameVersion(value);
            void loadLoaderCatalog(activeLoader, value);
          }}
          onChangeLoaderBuildVersion={setSelectedLoaderVersion}
          onChangeLoaderProfileName={(value) => {
            setLoaderNameTouched(true);
            setLoaderProfileName(value);
          }}
          onInstallLoader={() => void handleInstallLoader()}
          onOpenGuide={(url) => void handleOpenGuide(url)}
          onLaunchOfficial={() => void handleOpenOfficialLauncher()}
          onToggleTempCache={(enabled) => void handleToggleTempCache(enabled)}
          onChangePerformanceLiteMode={(mode) => void handleChangePerformanceLiteMode(mode)}
          onEnsureJavaRuntime={() => void handleEnsureJavaRuntime()}
          onRefreshStatus={() => void refreshSettingsState()}
          onClearTempCache={() => void handleClearTempCache()}
          onExportDebugLog={() => void handleExportDebugLog()}
          onOpenTempRoot={() => void handleOpenTempRoot()}
        />
      </main>
    </div>
  );
}

export default App;
