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
      <div className="loader-feature">
        <div className="loader-feature-copy">
          <span className="section-kicker">Loader 自動導入</span>
          <h3>{activeGuide?.name ?? formatLoader(activeLoader)} をそのまま追加する</h3>
          <p>
            Vanilla 本体の不足分を先に揃えた上で、選んだ Loader の version と起動構成をまとめて追加します。
            今の構成の `gameDir` を引き継いだまま派生構成を増やせます。
          </p>
        </div>

        <div className="loader-feature-meta">
          <article>
            <span>対象構成</span>
            <strong>{profile?.name ?? "未選択"}</strong>
          </article>
          <article>
            <span>選択中 Loader</span>
            <strong>{activeGuide?.name ?? formatLoader(activeLoader)}</strong>
          </article>
          <article>
            <span>Installer</span>
            <strong>{catalog?.installerVersion.id ?? "読込中"}</strong>
          </article>
        </div>
      </div>

      <div className="toolbar-card">
        <div className="section-copy">
          <span className="section-kicker">切り替え</span>
          <h3>導入する Loader を選ぶ</h3>
          <p>{activeGuide?.detail ?? "導入したい Loader を選ぶと、互換バージョンをここに読み込みます。"}</p>
        </div>

        <div className="toolbar-actions">
          <div className="segmented loader-picker">
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

      <div className="loader-grid">
        <article className="loader-panel fabric-panel">
          <div className="loader-panel-shell">
            <div className="loader-panel-main">
              <div className="panel-heading">
                <span>{activeGuide?.name ?? formatLoader(activeLoader)} セットアップ</span>
                <small>{loadingCatalog ? "同期中..." : "自動導入可能"}</small>
              </div>

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
                  <span>{activeGuide?.name ?? formatLoader(activeLoader)} Loader</span>
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

                <label className="full">
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
                  className="play-button compact"
                  type="button"
                  onClick={onInstallLoader}
                  disabled={!selectedVersion || !selectedLoaderVersion || installing || loadingCatalog}
                >
                  {installing
                    ? `${activeGuide?.name ?? formatLoader(activeLoader)} を導入中...`
                    : `${activeGuide?.name ?? formatLoader(activeLoader)} を導入`}
                </button>
                <button className="secondary-button" type="button" onClick={onLaunchOfficial}>
                  公式 Launcher を開く
                </button>
              </div>
            </div>

            <aside className="loader-panel-sidebar">
              <div className="loader-sidebar-copy">
                <span className="section-kicker">今回の内容</span>
                <h4>{activeGuide?.name ?? formatLoader(activeLoader)} を追加</h4>
                <p>
                  Version 追加と起動構成作成をまとめて行います。元の構成は残しながら、
                  Loader 付きの派生構成を増やせます。
                </p>
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
            </aside>
          </div>
        </article>

        {standbyGuides.map((guide) => (
          <article className="loader-panel" key={guide.id}>
            <div className="panel-heading">
              <span>{guide.name}</span>
              <small>{guide.kicker}</small>
            </div>

            <p className="loader-description">{guide.description}</p>
            <p className="loader-detail">{guide.detail}</p>

            <div className="loader-panel-footer">
              <span>
                {profile
                  ? `現在の構成: ${profile.name}`
                  : "起動構成を選ぶと派生先を確認しやすくなります。"}
              </span>
              <button className="secondary-button" type="button" onClick={() => onSelectLoader(guide.id)}>
                {guide.name} を選ぶ
              </button>
            </div>
          </article>
        ))}
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
