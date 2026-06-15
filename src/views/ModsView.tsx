import { useState } from "react";
import { ModListItem } from "../components/ModListItem";
import { getDuplicateModGroups } from "../app/modMatching";
import type { InstalledMod, LauncherProfile, ModRemoteState } from "../app/types";

type ModsViewProps = {
  profile: LauncherProfile | null;
  busyAction: string | null;
  remoteStates: Record<string, ModRemoteState>;
  loadingRemoteStates: boolean;
  remoteFetchDone: number;
  remoteFetchTotal: number;
  lastCheckedAt: number | null;
  onToggle: (mod: InstalledMod) => void;
  onUpdate: (mod: InstalledMod) => void;
  onUpdateAll: () => void;
  onCheckUpdates: () => void;
  onRemove: (mod: InstalledMod) => void;
  onOpenModSource: (mod: InstalledMod, remoteState?: ModRemoteState) => void;
  onOpenGameDir: () => void;
  onOpenModsDir: () => void;
  onImportLocalMod: () => void;
};

export function ModsView({
  profile,
  busyAction,
  remoteStates,
  loadingRemoteStates,
  remoteFetchDone,
  remoteFetchTotal,
  lastCheckedAt,
  onToggle,
  onUpdate,
  onUpdateAll,
  onCheckUpdates,
  onRemove,
  onOpenModSource,
  onOpenGameDir,
  onOpenModsDir,
  onImportLocalMod,
}: ModsViewProps) {
  const [filter, setFilter] = useState<"all" | "enabled" | "disabled">("all");
  const [query, setQuery] = useState("");
  const updatableCount = Object.values(remoteStates).filter((state) => state.updateAvailable).length;
  const trackedCount =
    profile?.mods.filter((mod) => (mod.sourceProjectId ?? "").trim() !== "").length ?? 0;
  const duplicateGroups = getDuplicateModGroups(profile?.mods ?? []);
  const duplicateCount = duplicateGroups.reduce((count, group) => count + group.length, 0);
  const updatingAll = busyAction === "update-all-mods";
  const checkingUpdates = busyAction === "check-mod-updates";
  const totalMods = profile?.mods.length ?? 0;
  const enabledMods = profile?.mods.filter((mod) => mod.enabled).length ?? 0;
  const disabledMods = profile?.mods.filter((mod) => !mod.enabled).length ?? 0;
  const checkedAtLabel = lastCheckedAt
    ? new Date(lastCheckedAt).toLocaleString("ja-JP", {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      })
    : "未確認";

  const filteredMods = (profile?.mods ?? []).filter((mod) => {
    if (filter === "enabled" && !mod.enabled) {
      return false;
    }
    if (filter === "disabled" && mod.enabled) {
      return false;
    }

    const keyword = query.trim().toLowerCase();
    if (!keyword) {
      return true;
    }

    return [
      mod.displayName,
      mod.fileName,
      mod.modId ?? "",
      mod.description ?? "",
      mod.version ?? "",
    ]
      .join(" ")
      .toLowerCase()
      .includes(keyword);
  });

  return (
    <section className="workspace mods-workspace">
      {/* ===== フィルターバー ===== */}
      <div className="toolbar-card mods-toolbar">
        {/* 左: タイトル＋スタッツ */}
        <div className="mods-toolbar-left">
          <div className="mods-stat-chips">
            <span className="mods-stat-chip">
              <span className="mods-stat-num">{totalMods}</span>
              <span className="mods-stat-label">Mod</span>
            </span>
            <span className="mods-stat-chip mods-stat-chip--on">
              <span className="mods-stat-num">{enabledMods}</span>
              <span className="mods-stat-label">有効</span>
            </span>
            <span className="mods-stat-chip mods-stat-chip--off">
              <span className="mods-stat-num">{disabledMods}</span>
              <span className="mods-stat-label">無効</span>
            </span>
            <span className="mods-stat-chip">
              <span className="mods-stat-num">{trackedCount}</span>
              <span className="mods-stat-label">追跡中</span>
            </span>
            <span className="mods-stat-chip">
              <span className="mods-stat-num">{trackedCount}</span>
              <span className="mods-stat-label">追跡中</span>
            </span>
            {updatableCount > 0 ? (
              <span className="mods-stat-chip mods-stat-chip--update">
                <span className="mods-stat-num">{updatableCount}</span>
                <span className="mods-stat-label">更新可能</span>
              </span>
            ) : null}
          </div>
        </div>

        {/* 中: 検索＋フィルター */}
        <div className="mods-toolbar-center">
          <input
            value={query}
            onChange={(event) => setQuery(event.currentTarget.value)}
            className="toolbar-search"
            placeholder="🔍  Mod名・ID・説明で検索..."
          />
          <div className="segmented">
            <button className={filter === "all" ? "is-active" : ""} onClick={() => setFilter("all")}>
              すべて
            </button>
            <button className={filter === "enabled" ? "is-active" : ""} onClick={() => setFilter("enabled")}>
              有効
            </button>
            <button className={filter === "disabled" ? "is-active" : ""} onClick={() => setFilter("disabled")}>
              無効
            </button>
          </div>
        </div>

        {/* 右: アクション */}
        <div className="mods-toolbar-right">
          <div className="mods-update-status">
            {loadingRemoteStates ? (
              <span className="mods-update-label">
                {remoteFetchTotal > 0 ? `${remoteFetchDone} / ${remoteFetchTotal} 取得中` : "チェック中"}
              </span>
            ) : (
              <span className="mods-update-label">最終確認: {checkedAtLabel}</span>
            )}
          </div>
          <button
            className="secondary-button compact"
            onClick={onCheckUpdates}
            disabled={!profile || loadingRemoteStates || checkingUpdates}
          >
            {checkingUpdates ? "チェック中..." : "更新チェック"}
          </button>
          {updatableCount > 0 ? (
            <button
              className="play-button compact"
              onClick={onUpdateAll}
              disabled={!profile || loadingRemoteStates || updatingAll}
            >
              {updatingAll ? "更新中..." : `${updatableCount}件更新`}
            </button>
          ) : null}
          <button className="secondary-button compact" onClick={onImportLocalMod} disabled={!profile}>
            ＋ 追加
          </button>
          <button className="secondary-button compact" onClick={onOpenModsDir} disabled={!profile}>
            Modsフォルダ
          </button>
          <button className="secondary-button compact" onClick={onOpenGameDir} disabled={!profile}>
            ゲームフォルダ
          </button>
        </div>
      </div>

      {/* ===== 重複警告 ===== */}
      {profile && duplicateGroups.length > 0 ? (
        <article className="empty-state warning duplicate-warning">
          <strong>⚠ 重複している Mod を検知しました</strong>
          <p>
            同じ Mod が {duplicateGroups.length} グループ、合計 {duplicateCount} 件見つかりました。
            片方は削除し、バージョン違いなら更新を使って整理してください。
          </p>
        </article>
      ) : null}

      {/* ===== Modリスト ===== */}
      {!profile ? (
        <article className="empty-state">
          <strong>起動構成が未選択です</strong>
          <p>上の一覧から起動構成を選ぶと Mod 管理を始められます。</p>
        </article>
      ) : filteredMods.length === 0 ? (
        <article className="empty-state">
          <strong>表示できる Mod がありません</strong>
          <p>Discover から導入するか、mods フォルダへ .jar を追加するとここに反映されます。</p>
        </article>
      ) : (
        <div className="mod-list">
          {filteredMods.map((mod) => (
            <ModListItem
              key={mod.fileName}
              mod={mod}
              profileLoader={profile.loader}
              remoteState={remoteStates[mod.fileName]}
              busyAction={busyAction}
              onOpenSource={() => onOpenModSource(mod, remoteStates[mod.fileName])}
              onToggle={() => onToggle(mod)}
              onUpdate={() => onUpdate(mod)}
              onRemove={() => onRemove(mod)}
            />
          ))}
        </div>
      )}
    </section>
  );
}
