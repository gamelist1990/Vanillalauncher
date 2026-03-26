import { formatLoader } from "../app/formatters";
import { getProjectInstallState } from "../app/modMatching";
import type { ModrinthProject, LauncherProfile } from "../app/types";
import { DiscoverResultRow } from "../components/DiscoverResultRow";

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
  const loaderLabel = profile ? formatLoader(profile.loader) : "未選択";
  const gameVersion = profile?.gameVersion ?? "未判定";
  const normalizedQuery = searchQuery.trim() === "0" ? "" : searchQuery.trim();
  const searchingMods = mode === "mods";

  return (
    <section className="workspace discover-workspace">
      <div className="toolbar-card discover-toolbar">
        <div className="section-copy">
          <span className="section-kicker">Discover</span>
          <h3>{searchingMods ? "直感的に Mod を探して入れる" : "Modpack から新しい構成を作る"}</h3>
          <p>
            {searchingMods
              ? "選択中の起動構成に合わせて、Loader と Minecraft バージョンの条件をそのまま検索に使います。Modrinth 検索に加えて、CurseForge の projectID を入れれば直接導入もできます。"
              : "Modrinth の modpack を探して、その pack 専用の新しい起動構成をこのランチャー内に作成できます。"}
          </p>
        </div>

        <div className="discover-scope">
          <article>
            <span>{searchingMods ? "対象条件" : "動作"}</span>
            <strong>
              {searchingMods ? `${loaderLabel} / ${gameVersion}` : "新しい modpack 構成を作成"}
            </strong>
          </article>
          <article>
            <span>{searchingMods ? "起動構成" : "選択中の構成"}</span>
            <strong>
              {searchingMods
                ? profile?.name ?? "起動構成を選択してください"
                : profile?.name ?? "未選択でも作成できます"}
            </strong>
          </article>
        </div>
      </div>

      <div className="toolbar-card">
        <div className="section-copy">
          <span className="section-kicker">対象</span>
          <h3>探す内容を切り替える</h3>
          <p>Mods は今の構成へ追加、Modpacks は新しい構成を作成します。</p>
        </div>

        <div className="toolbar-actions">
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
      </div>

      <form
        className="toolbar-card discover-search-shell"
        onSubmit={(event) => {
          event.preventDefault();
          onSearch();
        }}
      >
        <label className="discover-search-field">
          <span>検索キーワード</span>
          <input
            value={searchQuery}
            onChange={(event) => onChangeQuery(event.currentTarget.value)}
            placeholder={
              searchingMods
                ? "空欄か 0 でおすすめ / Sodium / Iris / JourneyMap / CurseForge の projectID..."
                : "空欄か 0 で人気の modpack / Fabulously Optimized / Better MC..."
            }
          />
        </label>
        <button className="play-button compact" disabled={(searchingMods && !profile) || searching}>
          {searching
            ? "読込中..."
            : normalizedQuery
              ? "検索"
              : searchingMods
                ? "おすすめを見る"
                : "人気 pack を見る"}
        </button>
      </form>

      {performanceLite ? (
        <article className="empty-state warning">
          <strong>軽量モードを有効化しています</strong>
          <p>
            低速回線または低スペック環境を検知したため、自動取得を抑えて手動検索を優先しています。
          </p>
        </article>
      ) : null}

      {searchingMods && profile?.loader === "vanilla" ? (
        <article className="empty-state warning">
          <strong>Vanilla 構成には Mod を直接導入できません</strong>
          <p>
            いまは Vanilla 構成です。Fabric / Forge / NeoForge / Quilt のいずれかを導入すると、
            対応する Mod だけを絞ってそのままインストールできます。
          </p>
        </article>
      ) : null}

      {results.length === 0 && !searching ? (
        <article className="empty-state">
          <strong>{searchingMods ? "おすすめか検索結果がここに並びます" : "おすすめの Modpack がここに並びます"}</strong>
          <p>
            {searchingMods
              ? "空欄のまま検索すると、現在の Loader とバージョンに合う人気 Mod を先に表示します。"
              : "空欄のまま検索すると、Modrinth で人気の Modpack を先に表示します。"}
          </p>
        </article>
      ) : (
        <div className="discover-list">
          <div className="discover-results-header">
            <strong>
              {normalizedQuery === "" ? "おすすめ" : "検索結果"} {results.length} 件
            </strong>
            <span>
              {searchingMods
                ? `${loaderLabel} / ${gameVersion} に合うものだけを表示しています。CurseForge は projectID 指定にも対応します。`
                : "選んだ Modpack ごとに新しい起動構成を作成します。"}
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
              />
            );
          })}
        </div>
      )}
    </section>
  );
}
