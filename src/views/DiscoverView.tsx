import { formatLoader } from "../app/formatters";
import { getProjectInstallState } from "../app/modMatching";
import type { ModrinthProject, LauncherProfile } from "../app/types";
import { useState } from "react";
import { DiscoverResultRow } from "../components/DiscoverResultRow";
import { ProjectDetailModal } from "../components/ProjectDetailModal";

type DiscoverViewProps = {
  mode: "mods" | "modpacks";
  profile: LauncherProfile | null;
  searchQuery: string;
  searching: boolean;
  performanceLite: boolean;
  busyAction: string | null;
  results: ModrinthProject[];
  onChangeMode: (mode: "mods" | "modpacks") => void;
  onChangeQuery: (value: string) => void;
  onSearch: () => void;
  onProjectAction: (project: ModrinthProject) => void;
  onOpenProject: (url: string) => void;
};

export function DiscoverView({
  mode,
  profile,
  searchQuery,
  searching,
  performanceLite,
  busyAction,
  results,
  onChangeMode,
  onChangeQuery,
  onSearch,
  onProjectAction,
  onOpenProject,
}: DiscoverViewProps) {
  const [detailProject, setDetailProject] = useState<ModrinthProject | null>(null);

  const loaderLabel = profile ? formatLoader(profile.loader) : "未選択";
  const gameVersion = profile?.gameVersion ?? "未判定";
  const normalizedQuery = searchQuery.trim() === "0" ? "" : searchQuery.trim();
  const searchingMods = mode === "mods";

  return (
    <section className="workspace discover-workspace">
      {/* ===== ヘッダー ===== */}
      <div className="discover-header panel">
        <div className="discover-header-top">
          <div>
            <p className="eyebrow">Discover</p>
            <h2 className="header-title">
              {searchingMods ? "Mod を探す" : "Modpack を探す"}
            </h2>
            <p className="header-subtitle">
              {searchingMods
                ? `${loaderLabel} / ${gameVersion} に対応した Mod を Modrinth から検索します`
                : "Modrinth の Modpack から新しい起動構成を作成します"}
            </p>
          </div>

          {/* Mode switcher */}
          <div className="segmented">
            <button
              type="button"
              className={mode === "mods" ? "is-active" : ""}
              onClick={() => onChangeMode("mods")}
            >
              Mods
            </button>
            <button
              type="button"
              className={mode === "modpacks" ? "is-active" : ""}
              onClick={() => onChangeMode("modpacks")}
            >
              Modpacks
            </button>
          </div>
        </div>

        {/* ===== 検索バー ===== */}
        <form
          className="discover-search-bar"
          onSubmit={(event) => {
            event.preventDefault();
            onSearch();
          }}
        >
          <input
            value={searchQuery}
            onChange={(event) => onChangeQuery(event.currentTarget.value)}
            className="discover-search-input"
            placeholder={
              searchingMods
                ? "🔍  Mod名を検索  /  CurseForge の projectID も使えます..."
                : "🔍  Modpack名を検索..."
            }
          />
          <button
            type="submit"
            className="play-button"
            disabled={(searchingMods && !profile) || searching}
          >
            {searching
              ? "検索中..."
              : normalizedQuery
                ? "検索"
                : searchingMods
                  ? "おすすめを見る"
                  : "人気 Pack を見る"}
          </button>
        </form>

        {/* 構成情報チップ */}
        {profile ? (
          <div className="discover-context-chips">
            <span className="badge badge-loader">{loaderLabel}</span>
            <span className="badge">{gameVersion}</span>
            <span className="badge">{profile.name}</span>
          </div>
        ) : null}
      </div>

      {/* ===== 警告 ===== */}
      {performanceLite ? (
        <article className="empty-state warning">
          <strong>⚡ 軽量モード有効</strong>
          <p>低速環境を検知しました。自動取得を抑えて手動検索を優先します。</p>
        </article>
      ) : null}

      {searchingMods && profile?.loader === "vanilla" ? (
        <article className="empty-state warning">
          <strong>⚠ Vanilla 構成には Mod を直接導入できません</strong>
          <p>Fabric / Forge / NeoForge / Quilt のいずれかを導入すると対応 Mod を検索できます。</p>
        </article>
      ) : null}

      {/* ===== 結果リスト ===== */}
      {results.length === 0 && !searching ? (
        <article className="empty-state">
          <strong>{searchingMods ? "検索結果がここに並びます" : "Modpack の検索結果がここに並びます"}</strong>
          <p>
            {searchingMods
              ? "空欄のまま検索すると、現在の Loader とバージョンに合う人気 Mod を表示します。"
              : "空欄のまま検索すると、Modrinth で人気の Modpack を表示します。"}
          </p>
        </article>
      ) : (
        <div className="discover-results">
          <div className="discover-results-header">
            <strong>
              {normalizedQuery === "" ? "おすすめ" : "検索結果"}{" "}
              {results.length} 件
            </strong>
            <span>
              {searchingMods
                ? `${loaderLabel} / ${gameVersion} に対応したものを表示中`
                : "選択した Pack から新しい起動構成を作成します"}
            </span>
          </div>

          {results.map((project) => {
            const { installedMod, state } = getProjectInstallState(profile, project);
            const actionDisabled = searchingMods
              ? !profile || profile.loader === "vanilla" || state === "installed" || state === "blocked"
              : false;

            return (
              <DiscoverResultRow
                key={project.projectId}
                project={project}
                mode={mode}
                disabled={actionDisabled}
                installState={searchingMods ? state : "install"}
                installed={searchingMods ? Boolean(installedMod) : false}
                loading={
                  busyAction === `install:${project.projectId}` ||
                  busyAction === `uninstall:${project.projectId}` ||
                  busyAction === `modpack:${project.projectId}` ||
                  busyAction === `modpack-versions:${project.projectId}`
                }
                onAction={() => onProjectAction(project)}
                onOpenProject={() => onOpenProject(project.projectUrl)}
                onOpenDetail={() => setDetailProject(project)}
              />
            );
          })}

          {/* ===== 詳細モーダル ===== */}
          {(() => {
            if (!detailProject) return null;
            const { installedMod, state } = getProjectInstallState(profile, detailProject);
            const actionDisabled = searchingMods
              ? !profile || profile.loader === "vanilla" || state === "installed" || state === "blocked"
              : false;
            const installLabel =
              mode === "modpacks"
                ? busyAction === `modpack:${detailProject.projectId}` ? "構成を作成中..." : "構成を作成"
                : state === "blocked" ? "重複あり"
                : actionDisabled ? "Loader 導入後"
                : busyAction === `install:${detailProject.projectId}` || busyAction === `uninstall:${detailProject.projectId}` ? "処理中..."
                : state === "update" ? "更新"
                : state === "installed" ? "導入済み"
                : Boolean(installedMod) ? "アンインストール"
                : "インストール";
            return (
              <ProjectDetailModal
                project={detailProject}
                mode={mode}
                installLabel={installLabel}
                installDisabled={actionDisabled}
                installed={searchingMods ? Boolean(installedMod) : false}
                loading={
                  busyAction === `install:${detailProject.projectId}` ||
                  busyAction === `uninstall:${detailProject.projectId}` ||
                  busyAction === `modpack:${detailProject.projectId}`
                }
                onClose={() => setDetailProject(null)}
                onAction={() => { onProjectAction(detailProject); setDetailProject(null); }}
                onOpenProject={() => onOpenProject(detailProject.projectUrl)}
              />
            );
          })()}
        </div>
      )}
    </section>
  );
}
