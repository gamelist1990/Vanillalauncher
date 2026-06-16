import { formatLoader } from "../app/formatters";
import type {
  LauncherProfile,
  LoaderCatalog,
  LoaderGuide,
  LoaderId,
  LoaderVersionSummary,
  MinecraftVersionSummary,
} from "../app/types";
import { useState } from "react";
import { MinecraftVersionSelectModal, type VersionTab } from "../components/MinecraftVersionSelectModal";

type LoadersViewProps = {
  activeLoader: LoaderId;
  profile: LauncherProfile | null;
  guides: LoaderGuide[];
  catalog: LoaderCatalog | null;
  loadingCatalog: boolean;
  selectedVersion: string;
  selectedLoaderVersion: string;
  profileName: string;
  busyAction: string | null;
  onSelectLoader: (loader: LoaderId) => void;
  onChangeVersion: (value: string) => void;
  onChangeLoaderVersion: (value: string) => void;
  onChangeProfileName: (value: string) => void;
  onInstallLoader: () => void;
  onOpenGuide: (url: string) => void;
  onLaunchOfficial: () => void;
};

export function LoadersView({
  activeLoader,
  profile,
  guides,
  catalog,
  loadingCatalog,
  selectedVersion,
  selectedLoaderVersion,
  profileName,
  busyAction,
  onSelectLoader,
  onChangeVersion,
  onChangeLoaderVersion,
  onChangeProfileName,
  onInstallLoader,
  onOpenGuide,
  onLaunchOfficial,
}: LoadersViewProps) {
  const [versionModalOpen, setVersionModalOpen] = useState(false);
  const [versionModalTab, setVersionModalTab] = useState<VersionTab>("minecraft");
  const activeGuide = guides.find((guide) => guide.id === activeLoader);
  const installing = busyAction === "loader-install";
  const selectedGameVersionSummary = catalog?.availableGameVersions.find(
    (version) => version.id === selectedVersion,
  );
  const selectedLoaderVersionSummary = catalog?.availableLoaderVersions.find(
    (loader) => loader.id === selectedLoaderVersion,
  );

  return (
    <section className="workspace loaders-workspace">
      {/* ===== ページヘッダー ===== */}
      <header className="loader-page-header">
        <div>
          <p className="eyebrow">Loaders</p>
          <h2 className="header-title">Loader を導入する</h2>
          <p className="header-subtitle">
            Vanilla 本体を補完しながら、選択した Loader のバージョンと起動構成をまとめて追加します
          </p>
        </div>
        <div className="loader-status-chips">
          <span className="badge">対象構成: {profile?.name ?? "未選択"}</span>
          <span className="badge badge-loader">{activeGuide?.name ?? formatLoader(activeLoader)}</span>
          <span className="badge">Installer: {catalog?.installerVersion?.id ?? "読込中"}</span>
          {loadingCatalog ? <span className="badge">同期中...</span> : null}
        </div>
      </header>

      {/* ===== メインレイアウト ===== */}
      <div className="loaders-layout">

        {/* ── 左レール: Loader選択 + 導入内容 ── */}
        <aside className="loaders-rail panel">
          <div className="loader-rail-section">
            <p className="loader-rail-label">Loader を選択</p>
            <div className="loader-select-list">
              {guides.map((guide) => (
                <button
                  key={guide.id}
                  type="button"
                  className={`loader-select-item ${guide.id === activeLoader ? "is-active" : ""}`}
                  onClick={() => onSelectLoader(guide.id)}
                >
                  <span className="loader-select-body">
                    <strong>{guide.name}</strong>
                    <small>{guide.kicker}</small>
                  </span>
                  {guide.id === activeLoader && (
                    <span className="loader-select-check" aria-hidden="true">✓</span>
                  )}
                </button>
              ))}
            </div>
          </div>


        </aside>

        {/* ── 右メイン: アクティブLoader情報 + フォーム ── */}
        <div className="loader-main-panel">

          {/* Loader info card */}
          <div className="loader-active-info panel">
            <div className="loader-active-info-header">
              <div>
                <span className="section-kicker">選択中の Loader</span>
                <h3 className="loader-active-name">
                  {activeGuide?.name ?? formatLoader(activeLoader)}
                </h3>
                <p className="loader-description">
                  {activeGuide?.detail ?? "導入したい Loader を選ぶと、互換バージョンをここに読み込みます。"}
                </p>
              </div>
            </div>
          </div>

          {/* Setup form card */}
          {catalog && catalog.availableLoaderVersions.length === 0 ? (
            <div className="loader-unsupported panel">
              <p className="loader-unsupported-msg">
                {activeGuide?.name ?? formatLoader(activeLoader)} は、選択中の Minecraft バージョン向けの対応バージョンがありません。<br />
                別の Minecraft バージョンを選ぶか、他の Loader をお試しください。
              </p>
            </div>
          ) : (
          <div className="loader-form-card panel">
            <div className="loader-version-pickers">
              <label>
                <span>Minecraft バージョン</span>
                <button
                  type="button"
                  className="loader-version-trigger"
                  disabled={loadingCatalog}
                  onClick={() => {
                    setVersionModalTab("minecraft");
                    setVersionModalOpen(true);
                  }}
                >
                  <span>
                    <strong>{selectedVersion || "バージョンを選択"}</strong>
                    <small>
                      {selectedGameVersionSummary
                        ? renderVersionLabel(selectedGameVersionSummary)
                        : "一覧から選択"}
                    </small>
                  </span>
                  <span aria-hidden="true">選択</span>
                </button>
              </label>

              <label>
                <span>{activeGuide?.name ?? formatLoader(activeLoader)} Loader バージョン</span>
                <button
                  type="button"
                  className="loader-version-trigger"
                  disabled={loadingCatalog}
                  onClick={() => {
                    setVersionModalTab("loader");
                    setVersionModalOpen(true);
                  }}
                >
                  <span>
                    <strong>
                      {selectedLoaderVersion ||
                        `${activeGuide?.name ?? formatLoader(activeLoader)} Loader を選択`}
                    </strong>
                    <small>
                      {selectedLoaderVersionSummary
                        ? renderLoaderLabel(selectedLoaderVersionSummary)
                        : "一覧から選択"}
                    </small>
                  </span>
                  <span aria-hidden="true">選択</span>
                </button>
              </label>
            </div>

            <hr className="loader-form-sep" />

            <label className="loader-form-name">
              <span>作成する構成名</span>
              <input
                value={profileName}
                onChange={(event) => onChangeProfileName(event.currentTarget.value)}
                placeholder={`${activeGuide?.name ?? formatLoader(activeLoader)} ${selectedVersion || "Profile"}`}
              />
            </label>

            <div className="loader-actions">
              <button
                className="play-button"
                type="button"
                onClick={onInstallLoader}
                disabled={!selectedVersion || !selectedLoaderVersion || installing || loadingCatalog}
              >
                {installing
                  ? `${activeGuide?.name ?? formatLoader(activeLoader)} を導入中...`
                  : `${activeGuide?.name ?? formatLoader(activeLoader)} を導入`}
              </button>
              <button className="secondary-button" type="button" onClick={onLaunchOfficial}>
                公式 Launcher
              </button>
              <button
                className="secondary-button"
                type="button"
                disabled={!activeGuide}
                onClick={() => activeGuide && onOpenGuide(activeGuide.url)}
              >
                公式ページ
              </button>
            </div>
          </div>
          )}
        </div>
      </div>

      {versionModalOpen ? (
        <MinecraftVersionSelectModal
          open={versionModalOpen}
          loaderName={activeGuide?.name ?? formatLoader(activeLoader)}
          gameVersions={catalog?.availableGameVersions ?? []}
          loaderVersions={catalog?.availableLoaderVersions ?? []}
          selectedGameVersion={selectedVersion}
          selectedLoaderVersion={selectedLoaderVersion}
          initialTab={versionModalTab}
          loading={loadingCatalog}
          onClose={() => setVersionModalOpen(false)}
          onSelectGameVersion={onChangeVersion}
          onSelectLoaderVersion={onChangeLoaderVersion}
        />
      ) : null}
    </section>
  );
}

function renderVersionLabel(version: MinecraftVersionSummary) {
  if (version.kind === "release") {
    return `${version.id} (リリース)`;
  }

  if (version.kind === "snapshot") {
    return `${version.id} (スナップショット)`;
  }

  return version.id;
}

function renderLoaderLabel(loader: LoaderVersionSummary) {
  return loader.stable ? `${loader.id} (安定版)` : loader.id;
}
