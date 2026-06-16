import { formatLoader } from "../app/formatters";
import { getProjectInstallState } from "../app/modMatching";
import type { ModrinthProject, LauncherProfile } from "../app/types";
import { useEffect, useMemo, useRef, useState } from "react";
import { DiscoverResultRow } from "../components/DiscoverResultRow";
import { ProjectDetailModal } from "../components/ProjectDetailModal";
import {
  DiscoverAdvancedSearchModal,
  type AdvancedFilters,
} from "../components/DiscoverAdvancedSearchModal";

type DiscoverViewProps = {
  mode: "mods" | "modpacks";
  profile: LauncherProfile | null;
  searchQuery: string;
  searching: boolean;
  loadingMore: boolean;
  hasMore: boolean;
  performanceLite: boolean;
  busyAction: string | null;
  results: ModrinthProject[];
  onChangeMode: (mode: "mods" | "modpacks") => void;
  onChangeQuery: (value: string) => void;
  onSearch: () => void;
  onLoadMore: () => void;
  onProjectAction: (project: ModrinthProject) => void;
  onOpenProject: (url: string) => void;
};

const DEFAULT_FILTERS: AdvancedFilters = {
  sortBy: "relevance",
  categories: [],
  environment: "any",
};

export function DiscoverView({
  mode,
  profile,
  searchQuery,
  searching,
  loadingMore,
  hasMore,
  performanceLite,
  busyAction,
  results,
  onChangeMode,
  onChangeQuery,
  onSearch,
  onLoadMore,
  onProjectAction,
  onOpenProject,
}: DiscoverViewProps) {
  const [detailProject, setDetailProject] = useState<ModrinthProject | null>(null);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [advFilters, setAdvFilters] = useState<AdvancedFilters>(DEFAULT_FILTERS);
  const [viewLayout, setViewLayout] = useState<"list" | "grid">("grid");
  const loadMoreRef = useRef<HTMLDivElement | null>(null);

  // advFilters をクライアントサイドで results に適用する
  const filteredResults = useMemo(() => {
    let list = [...results];

    // カテゴリフィルター
    if (advFilters.categories.length > 0) {
      list = list.filter((p) =>
        advFilters.categories.some((cat) => p.categories.includes(cat))
      );
    }

    // 対応環境フィルター
    if (advFilters.environment !== "any") {
      list = list.filter((p) => {
        const isClient = p.clientSide === "required" || p.clientSide === "optional";
        const isServer = p.serverSide === "required" || p.serverSide === "optional";
        if (advFilters.environment === "client") return isClient;
        if (advFilters.environment === "server") return isServer;
        if (advFilters.environment === "both") return isClient && isServer;
        return true;
      });
    }

    // 並び順（バックエンドは最大18件なのでクライアントソートで対応）
    if (advFilters.sortBy !== "relevance") {
      list = [...list].sort((a, b) => {
        if (advFilters.sortBy === "downloads") return b.downloads - a.downloads;
        if (advFilters.sortBy === "follows") return b.followers - a.followers;
        if (advFilters.sortBy === "newest" || advFilters.sortBy === "updated") {
          return (b.updatedAt ?? "").localeCompare(a.updatedAt ?? "");
        }
        return 0;
      });
    }

    return list;
  }, [results, advFilters]);

  const loaderLabel = profile ? formatLoader(profile.loader) : "未選択";
  const gameVersion = profile?.gameVersion ?? "未判定";
  const normalizedQuery = searchQuery.trim() === "0" ? "" : searchQuery.trim();
  const searchingMods = mode === "mods";

  useEffect(() => {
    const node = loadMoreRef.current;
    if (!node || !hasMore || searching || loadingMore) {
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          onLoadMore();
        }
      },
      { root: null, rootMargin: "640px 0px", threshold: 0 },
    );

    observer.observe(node);
    return () => observer.disconnect();
  }, [filteredResults.length, hasMore, loadingMore, onLoadMore, searching]);

  const hasActiveFilters =
    advFilters.sortBy !== "relevance" ||
    advFilters.categories.length > 0 ||
    advFilters.environment !== "any";

  const sortLabel: Record<AdvancedFilters["sortBy"], string> = {
    relevance: "関連度順",
    downloads: "DL数順",
    follows: "フォロワー順",
    newest: "新着順",
    updated: "更新日順",
  };

  return (
    <section className="workspace discover-workspace">

      {/* ===== HERO HEADER ===== */}
      <div className="discover-hero">
        <div className="discover-hero-inner">
          <div className="discover-hero-left">
            <span className="discover-hero-badge">
              {searchingMods ? "🧩 Mods" : "📦 Modpacks"}
            </span>
            <h1 className="discover-hero-title">
              {searchingMods ? "Mod を探す" : "Modpack を探す"}
            </h1>
            <p className="discover-hero-sub">
              {searchingMods
                ? `${loaderLabel} / ${gameVersion} に対応した Mod を Modrinth から検索`
                : "Modrinth の Modpack から新しい起動構成を作成"}
            </p>
          </div>

          <div className="discover-mode-switch">
            <button
              type="button"
              className={`dmb ${mode === "mods" ? "dmb--active" : ""}`}
              onClick={() => onChangeMode("mods")}
            >
              <span>🧩</span> Mods
            </button>
            <button
              type="button"
              className={`dmb ${mode === "modpacks" ? "dmb--active" : ""}`}
              onClick={() => onChangeMode("modpacks")}
            >
              <span>📦</span> Modpacks
            </button>
          </div>
        </div>

        {profile ? (
          <div className="discover-context-strip">
            <span className="dcc dcc--loader">{loaderLabel}</span>
            <span className="dcc">🎮 {gameVersion}</span>
            <span className="dcc">👤 {profile.name}</span>
          </div>
        ) : (
          <div className="discover-context-strip">
            <span className="dcc dcc--warn">⚠ プロファイル未選択</span>
          </div>
        )}
      </div>

      {/* ===== SEARCH TOOLBAR ===== */}
      <div className="discover-toolbar">
        <form
          className="discover-search-form"
          onSubmit={(e) => { e.preventDefault(); onSearch(); }}
        >
          <div className="discover-input-wrap">
            <span className="discover-input-icon">🔍</span>
            <input
              value={searchQuery}
              onChange={(e) => onChangeQuery(e.currentTarget.value)}
              className="discover-search-input"
              placeholder={
                searchingMods
                  ? "Mod名 / CurseForge projectID を入力..."
                  : "Modpack名を検索..."
              }
            />
            {searchQuery.length > 0 && (
              <button
                type="button"
                className="discover-input-clear"
                onClick={() => onChangeQuery("")}
                aria-label="クリア"
              >
                ✕
              </button>
            )}
          </div>

          <button
            type="submit"
            className="play-button"
            disabled={(searchingMods && !profile) || searching}
          >
            {searching ? "検索中…" : normalizedQuery ? "検索" : searchingMods ? "おすすめを見る" : "人気を見る"}
          </button>
        </form>

        <div className="discover-toolbar-extras">
          <button
            type="button"
            className={`discover-adv-btn${hasActiveFilters ? " discover-adv-btn--active" : ""}`}
            onClick={() => setShowAdvanced(true)}
          >
            ⚙ 詳細検索
            {hasActiveFilters && <span className="discover-adv-dot" />}
          </button>

          <div className="discover-layout-btns">
            <button
              type="button"
              className={viewLayout === "list" ? "is-active" : ""}
              onClick={() => setViewLayout("list")}
              title="リスト表示"
            >☰</button>
            <button
              type="button"
              className={viewLayout === "grid" ? "is-active" : ""}
              onClick={() => setViewLayout("grid")}
              title="グリッド表示"
            >⊞</button>
          </div>
        </div>
      </div>

      {/* ===== WARNINGS ===== */}
      {performanceLite && (
        <div className="discover-notice discover-notice--warn">
          <span>⚡</span>
          <span><strong>軽量モード有効</strong> — 低速環境を検知しました。自動取得を抑えて手動検索を優先します。</span>
        </div>
      )}
      {searchingMods && profile?.loader === "vanilla" && (
        <div className="discover-notice discover-notice--warn">
          <span>⚠</span>
          <span><strong>Vanilla 構成には Mod を直接導入できません</strong> — Fabric / Forge / NeoForge / Quilt のいずれかを導入すると対応 Mod を検索できます。</span>
        </div>
      )}

      {/* ===== RESULTS ===== */}
      {filteredResults.length === 0 && !searching ? (
        <div className="discover-empty">
          <div className="discover-empty-icon">{searchingMods ? "🧩" : "📦"}</div>
          <h3 className="discover-empty-title">
            {normalizedQuery ? "検索結果が見つかりませんでした" : searchingMods ? "Mod を検索しましょう" : "Modpack を検索しましょう"}
          </h3>
          <p className="discover-empty-body">
            {normalizedQuery
              ? "別のキーワードや条件を試してみてください。"
              : searchingMods
                ? "空欄のまま検索すると、現在の Loader とバージョンに合う人気 Mod を表示します。"
                : "空欄のまま検索すると、Modrinth で人気の Modpack を表示します。"}
          </p>
          {!normalizedQuery && (
            <button
              type="button"
              className="play-button"
              disabled={(searchingMods && !profile) || searching}
              onClick={onSearch}
            >
              {searchingMods ? "🔥 人気 Mod を見る" : "🔥 人気 Modpack を見る"}
            </button>
          )}
        </div>
      ) : (
        <div className="discover-results-area">
          <div className="discover-results-meta">
            <div className="discover-results-meta-left">
              {searching ? (
                <span className="discover-searching">
                  <span className="discover-spinner" /> 検索中…
                </span>
              ) : (
                <>
                  <strong className="discover-count">{filteredResults.length}</strong>
                  <span className="discover-count-label">
                    {normalizedQuery === "" ? " 件のおすすめ" : " 件の検索結果"}
                  </span>
                  <span className="discover-count-ctx">
                    {searchingMods ? `· ${loaderLabel} / ${gameVersion}` : "· Modrinth"}
                  </span>
                </>
              )}
            </div>
            {hasActiveFilters && !searching && (
              <div className="discover-results-meta-right">
                <span className="discover-filter-active-chip">
                  {sortLabel[advFilters.sortBy]}
                  {advFilters.categories.length > 0 && ` · ${advFilters.categories.length} カテゴリ`}
                  {advFilters.environment !== "any" && ` · ${advFilters.environment}`}
                </span>
              </div>
            )}
          </div>

          {searching && filteredResults.length === 0 ? (
            <div className="discover-loading-state" role="status" aria-live="polite">
              <span className="discover-spinner discover-spinner--large" />
              <strong>読み込み中…</strong>
              <span>ネットワークから候補を取得しています。少し時間がかかる場合があります。</span>
            </div>
          ) : (
            <div className={`discover-list discover-list--${viewLayout}`}>
              {filteredResults.map((project) => {
                const { installedMod, state } = getProjectInstallState(profile, project);
                const actionDisabled = searchingMods
                  ? !profile || profile.loader === "vanilla" || state === "installed" || state === "blocked"
                  : false;

                return (
                  <DiscoverResultRow
                    key={project.projectId}
                    project={project}
                    mode={mode}
                    layout={viewLayout}
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

              <div ref={loadMoreRef} className="discover-load-more-sentinel">
                {loadingMore ? (
                  <span className="discover-searching">
                    <span className="discover-spinner" /> 追加読み込み中…
                  </span>
                ) : hasMore ? (
                  <button
                    type="button"
                    className="secondary-button discover-load-more-button"
                    onClick={onLoadMore}
                    disabled={searching}
                  >
                    さらに読み込む
                  </button>
                ) : filteredResults.length > 0 ? (
                  <span className="discover-load-more-end">すべて表示しました</span>
                ) : null}
              </div>
            </div>
          )}
        </div>
      )}

      {/* ===== DETAIL MODAL ===== */}
      {detailProject && (() => {
        const { installedMod, state } = getProjectInstallState(profile, detailProject);
        const actionDisabled = searchingMods
          ? !profile || profile.loader === "vanilla" || state === "installed" || state === "blocked"
          : false;
        const installLabel =
          mode === "modpacks"
            ? busyAction === `modpack:${detailProject.projectId}` ? "構成を作成中…" : "構成を作成"
            : state === "blocked" ? "重複あり"
            : actionDisabled ? "Loader 導入後"
            : busyAction === `install:${detailProject.projectId}` || busyAction === `uninstall:${detailProject.projectId}` ? "処理中…"
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

      {/* ===== ADVANCED SEARCH MODAL ===== */}
      {showAdvanced && (
        <DiscoverAdvancedSearchModal
          filters={advFilters}
          mode={mode}
          onApply={(f) => { setAdvFilters(f); onSearch(); }}
          onClose={() => setShowAdvanced(false)}
        />
      )}
    </section>
  );
}