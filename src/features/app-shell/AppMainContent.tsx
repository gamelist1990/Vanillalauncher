import { compactPath } from "../../app/formatters";
import { loaderGuides } from "../../app/constants";
import type {
  AppSettings,
  InstalledMod,
  LauncherProfile,
  LauncherSnapshot,
  LoaderCatalog,
  LoaderId,
  ModRemoteState,
  ModrinthProject,
  SoftwareStatus,
  ViewMode,
} from "../../app/types";
import { DiscoverView } from "../../views/DiscoverView";
import { LoadersView } from "../../views/LoadersView";
import { ModsView } from "../../views/ModsView";
import { PlayView } from "../../views/PlayView";
import { SettingsView } from "../../views/SettingsView";

type AppMainContentProps = {
  activeView: ViewMode;
  snapshot: LauncherSnapshot | null;
  selectedProfile: LauncherProfile | null;
  launchBusy: boolean;
  busyAction: string | null;
  discoverMode: "mods" | "modpacks";
  searchQuery: string;
  searching: boolean;
  performanceLite: boolean;
  searchResults: ModrinthProject[];
  modRemoteStateMap: Record<string, ModRemoteState>;
  loadingModRemoteStates: boolean;
  activeLoader: LoaderId;
  loaderCatalog: LoaderCatalog | null;
  loadingLoaderCatalog: boolean;
  selectedLoaderGameVersion: string;
  selectedLoaderVersion: string;
  loaderProfileName: string;
  appSettings: AppSettings | null;
  softwareStatus: SoftwareStatus | null;
  loadingSettings: boolean;
  onLaunchSpecificProfile: (profileId: string) => void;
  onOpenProfileMods: (profileId: string) => void;
  onUpdateModpackProfile: (profileId: string) => void;
  onCustomizeProfileVisuals: (profileId: string) => void;
  onImportLocalModpack: () => void;
  onExportProfileModpack: (profileId: string) => void;
  onToggleMod: (mod: InstalledMod) => void;
  onUpdateMod: (mod: InstalledMod) => void;
  onUpdateAllMods: () => void;
  onRemoveMod: (mod: InstalledMod) => void;
  onOpenModSource: (mod: InstalledMod, remoteState?: ModRemoteState) => void;
  onOpenGameDir: () => void;
  onOpenModsDir: () => void;
  onChangeDiscoverMode: (mode: "mods" | "modpacks") => void;
  onChangeSearchQuery: (value: string) => void;
  onSearch: () => void;
  onProjectAction: (project: ModrinthProject) => void;
  onOpenProject: (url: string) => void;
  onSelectLoader: (loader: LoaderId) => void;
  onChangeLoaderVersion: (value: string) => void;
  onChangeLoaderBuildVersion: (value: string) => void;
  onChangeLoaderProfileName: (value: string) => void;
  onInstallLoader: () => void;
  onOpenGuide: (url: string) => void;
  onLaunchOfficial: () => void;
  onToggleTempCache: (enabled: boolean) => void;
  onChangePerformanceLiteMode: (mode: AppSettings["performanceLiteMode"]) => void;
  onEnsureJavaRuntime: () => void;
  onRefreshStatus: () => void;
  onClearTempCache: () => void;
  onExportDebugLog: () => void;
  onOpenTempRoot: () => void;
};

export function AppMainContent({
  activeView,
  snapshot,
  selectedProfile,
  launchBusy,
  busyAction,
  discoverMode,
  searchQuery,
  searching,
  performanceLite,
  searchResults,
  modRemoteStateMap,
  loadingModRemoteStates,
  activeLoader,
  loaderCatalog,
  loadingLoaderCatalog,
  selectedLoaderGameVersion,
  selectedLoaderVersion,
  loaderProfileName,
  appSettings,
  softwareStatus,
  loadingSettings,
  onLaunchSpecificProfile,
  onOpenProfileMods,
  onUpdateModpackProfile,
  onCustomizeProfileVisuals,
  onImportLocalModpack,
  onExportProfileModpack,
  onToggleMod,
  onUpdateMod,
  onUpdateAllMods,
  onRemoveMod,
  onOpenModSource,
  onOpenGameDir,
  onOpenModsDir,
  onChangeDiscoverMode,
  onChangeSearchQuery,
  onSearch,
  onProjectAction,
  onOpenProject,
  onSelectLoader,
  onChangeLoaderVersion,
  onChangeLoaderBuildVersion,
  onChangeLoaderProfileName,
  onInstallLoader,
  onOpenGuide,
  onLaunchOfficial,
  onToggleTempCache,
  onChangePerformanceLiteMode,
  onEnsureJavaRuntime,
  onRefreshStatus,
  onClearTempCache,
  onExportDebugLog,
  onOpenTempRoot,
}: AppMainContentProps) {
  return (
    <>
      {activeView === "play" ? (
        <PlayView
          profiles={snapshot?.profiles ?? []}
          selectedProfileId={selectedProfile?.id ?? ""}
          launching={launchBusy}
          onLaunchProfile={onLaunchSpecificProfile}
          onOpenProfileMods={onOpenProfileMods}
          onUpdateModpackProfile={onUpdateModpackProfile}
          busyAction={busyAction}
          onCustomizeProfileVisuals={onCustomizeProfileVisuals}
          onImportLocalModpack={onImportLocalModpack}
          onExportProfileModpack={onExportProfileModpack}
        />
      ) : null}

      {activeView === "mods" ? (
        <ModsView
          profile={selectedProfile}
          busyAction={busyAction}
          remoteStates={modRemoteStateMap}
          loadingRemoteStates={loadingModRemoteStates}
          onToggle={onToggleMod}
          onUpdate={onUpdateMod}
          onUpdateAll={onUpdateAllMods}
          onRemove={onRemoveMod}
          onOpenModSource={onOpenModSource}
          onOpenGameDir={onOpenGameDir}
          onOpenModsDir={onOpenModsDir}
        />
      ) : null}

      {activeView === "discover" ? (
        <DiscoverView
          mode={discoverMode}
          profile={selectedProfile}
          searchQuery={searchQuery}
          searching={searching}
          performanceLite={performanceLite}
          busyAction={busyAction}
          results={searchResults}
          onChangeMode={onChangeDiscoverMode}
          onChangeQuery={onChangeSearchQuery}
          onSearch={onSearch}
          onProjectAction={onProjectAction}
          onOpenProject={onOpenProject}
        />
      ) : null}

      {activeView === "loaders" ? (
        <LoadersView
          activeLoader={activeLoader}
          profile={selectedProfile}
          guides={loaderGuides}
          catalog={loaderCatalog}
          loadingCatalog={loadingLoaderCatalog}
          selectedVersion={selectedLoaderGameVersion}
          selectedLoaderVersion={selectedLoaderVersion}
          profileName={loaderProfileName}
          busyAction={busyAction}
          onSelectLoader={onSelectLoader}
          onChangeVersion={onChangeLoaderVersion}
          onChangeLoaderVersion={onChangeLoaderBuildVersion}
          onChangeProfileName={onChangeLoaderProfileName}
          onInstallLoader={onInstallLoader}
          onOpenGuide={onOpenGuide}
          onLaunchOfficial={onLaunchOfficial}
        />
      ) : null}

      {activeView === "settings" ? (
        <SettingsView
          settings={appSettings}
          status={softwareStatus}
          busy={loadingSettings || busyAction === "java-runtime"}
          onToggleTempCache={onToggleTempCache}
          onChangePerformanceLiteMode={onChangePerformanceLiteMode}
          onEnsureJavaRuntime={onEnsureJavaRuntime}
          onRefreshStatus={onRefreshStatus}
          onClearTempCache={onClearTempCache}
          onExportDebugLog={onExportDebugLog}
          onOpenTempRoot={onOpenTempRoot}
        />
      ) : null}

      {!selectedProfile && snapshot ? (
        <article className="empty-state">
          <strong>起動構成がまだありません</strong>
          <p>
            公式 Minecraft Launcher を一度起動すると、
            {` ${compactPath(snapshot.minecraftRoot)} `}
            の内容を読んでこのアプリに表示します。
          </p>
        </article>
      ) : null}
    </>
  );
}
