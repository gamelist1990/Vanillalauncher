import { useState } from "react";
import { formatLoader } from "../app/formatters";
import type {
  LauncherProfile,
  LoaderCatalog,
  LoaderGuide,
  LoaderId,
  LoaderVersionSummary,
  MinecraftVersionSummary,
} from "../app/types";
import { DropdownSelect } from "../components/DropdownSelect";

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
  const [openDropdown, setOpenDropdown] = useState<"version" | "loader" | null>(null);
  const activeGuide = guides.find((guide) => guide.id === activeLoader);
  const standbyGuides = guides.filter((guide) => guide.id !== activeLoader);
  const installing = busyAction === "loader-install";
  const versionOptions = (catalog?.availableGameVersions ?? []).map((version) => ({
    value: version.id,
    label: renderVersionLabel(version),
  }));
  const loaderOptions = (catalog?.availableLoaderVersions ?? []).map((loader) => ({
    value: loader.id,
    label: renderLoaderLabel(loader),
  }));

  return (
    <section className="workspace loaders-workspace">
      {/* ===== ヘッダー ===== */}
      <div className="loader-header panel">
        <div className="loader-header-top">
          <div>
            <p className="eyebrow">Loaders</p>
            <h2 className="header-title">Loader を導入する</h2>
            <p className="header-subtitle">
              Vanilla 本体を補完しながら、選択した Loader のバージョンと起動構成をまとめて追加します
            </p>
          </div>
          {/* Loader 切り替え */}
          <div className="segmented">
            {guides.map((guide) => (
              <button
                key={guide.id}
                type="button"
                className={guide.id === activeLoader ? "is-active" : ""}
                onClick={() => onSelectLoader(guide.id)}
              >
                {guide.name}
              </button>
            ))}
          </div>
        </div>

        {/* 状態チップ */}
        <div className="loader-status-chips">
          <span className="badge">対象構成: {profile?.name ?? "未選択"}</span>
          <span className="badge badge-loader">{activeGuide?.name ?? formatLoader(activeLoader)}</span>
          <span className="badge">Installer: {catalog?.installerVersion.id ?? "読込中"}</span>
          {loadingCatalog ? <span className="badge">同期中...</span> : null}
        </div>
      </div>

      {/* ===== メインコンテンツ: 2列 ===== */}
      <div className="loaders-main-grid">
        {/* 左: セットアップフォーム */}
        <div className="loader-setup-card panel">
          <div className="panel-heading">
            <h3>{activeGuide?.name ?? formatLoader(activeLoader)} セットアップ</h3>
            <span className="badge badge-loader">自動導入可能</span>
          </div>

          <p className="loader-description">
            {activeGuide?.detail ?? "導入したい Loader を選ぶと、互換バージョンをここに読み込みます。"}
          </p>

          <div className="loader-form">
            <label>
              <span>Minecraft バージョン</span>
              <DropdownSelect
                value={selectedVersion}
                options={versionOptions}
                open={openDropdown === "version"}
                disabled={loadingCatalog}
                emptyLabel="Minecraft バージョンを選択"
                menuLabel="Minecraft バージョン一覧"
                onOpenChange={(open) => setOpenDropdown(open ? "version" : null)}
                onChange={onChangeVersion}
              />
            </label>

            <label>
              <span>{activeGuide?.name ?? formatLoader(activeLoader)} Loader バージョン</span>
              <DropdownSelect
                value={selectedLoaderVersion}
                options={loaderOptions}
                open={openDropdown === "loader"}
                disabled={loadingCatalog}
                emptyLabel={`${activeGuide?.name ?? formatLoader(activeLoader)} Loader を選択`}
                menuLabel={`${activeGuide?.name ?? formatLoader(activeLoader)} Loader 一覧`}
                onOpenChange={(open) => setOpenDropdown(open ? "loader" : null)}
                onChange={onChangeLoaderVersion}
              />
            </label>

            <label>
              <span>作成する構成名</span>
              <input
                value={profileName}
                onChange={(event) => onChangeProfileName(event.currentTarget.value)}
                placeholder={`${activeGuide?.name ?? formatLoader(activeLoader)} ${selectedVersion || "Profile"}`}
              />
            </label>
          </div>

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

        {/* 右: サイドバー情報 + 他のローダー */}
        <div className="loader-sidebar-stack">
          {/* インストール情報 */}
          <div className="panel">
            <div className="panel-heading">
              <h3>導入の内容</h3>
            </div>
            <div className="loader-notes">
              <div>
                <span>導入先</span>
                <strong>{profile?.name ?? "Minecraft Root 直下"}</strong>
              </div>
              <div>
                <span>推奨 Loader</span>
                <strong>{catalog?.recommendedLoader.id ?? "読込中"}</strong>
              </div>
              <div>
                <span>現在の Loader</span>
                <strong>{formatLoader(profile?.loader ?? "vanilla")}</strong>
              </div>
              <div>
                <span>動作</span>
                <strong>Version 追加 + 起動構成作成</strong>
              </div>
            </div>
          </div>

          {/* 他のローダー */}
          {standbyGuides.map((guide) => (
            <div className="loader-alt-card panel" key={guide.id}>
              <div className="loader-alt-header">
                <div>
                  <p className="loader-alt-name">{guide.name}</p>
                  <p className="loader-detail">{guide.kicker}</p>
                </div>
                <button
                  className="secondary-button compact"
                  type="button"
                  onClick={() => onSelectLoader(guide.id)}
                >
                  切り替え
                </button>
              </div>
              <p className="loader-description">{guide.description}</p>
            </div>
          ))}
        </div>
      </div>
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
