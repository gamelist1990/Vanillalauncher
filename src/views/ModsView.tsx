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
      <div className="toolbar-card">
        <div className="section-copy">
          <span className="section-kicker">Mod 管理</span>
          <h3>導入済み Mod</h3>
          <p>
            起動構成ごとに分離された Mod を表示します。Modrinth と CurseForge の追跡 Mod は、
            ここからリンク確認や更新チェックもまとめて扱えます。
          </p>

          <div className="mods-summary-strip" role="list" aria-label="Mod の概要">
            <article className="mods-summary-item" role="listitem">
              <span>この構成の mods</span>
              <strong>{totalMods}</strong>
            </article>
            <article className="mods-summary-item" role="listitem">
              <span>有効 / 無効</span>
              <strong>
                {enabledMods} / {disabledMods}
              </strong>
            </article>
            <article className="mods-summary-item" role="listitem">
              <span>追跡中</span>
              <strong>{trackedCount}</strong>
            </article>
          </div>
        </div>

        <div className="toolbar-actions">
          <div className="segmented">
            <button
              className={filter === "all" ? "is-active" : ""}
              onClick={() => setFilter("all")}
            >
              すべて
            </button>
            <button
              className={filter === "enabled" ? "is-active" : ""}
              onClick={() => setFilter("enabled")}
            >
              有効
            </button>
            <button
              className={filter === "disabled" ? "is-active" : ""}
              onClick={() => setFilter("disabled")}
            >
              無効
            </button>
          </div>

          <input
            value={query}
            onChange={(event) => setQuery(event.currentTarget.value)}
            className="toolbar-search"
            placeholder="Mod 名や ID で絞り込み"
          />

          <div className="mods-remote-status">
            <span>更新ステータス</span>
            <strong>
              {loadingRemoteStates
                ? remoteFetchTotal > 0
                  ? `${remoteFetchDone} / ${remoteFetchTotal} 取得中`
                  : "チェックしています"
                : `${updatableCount} 件更新可能`}
            </strong>
            <small>最終確認: {checkedAtLabel}</small>
            <button
              className="secondary-button"
              onClick={onCheckUpdates}
              disabled={!profile || loadingRemoteStates || checkingUpdates}
            >
              {checkingUpdates ? "更新チェック中..." : "更新チェック"}
            </button>
            <button
              className="play-button mods-bulk-update"
              onClick={onUpdateAll}
              disabled={!profile || loadingRemoteStates || checkingUpdates || updatableCount === 0 || updatingAll}
            >
              {updatingAll ? "すべて更新中..." : "すべて更新"}
            </button>
          </div>

          <button className="secondary-button" onClick={onOpenModsDir} disabled={!profile}>
            この構成の mods
          </button>
          <button className="secondary-button" onClick={onOpenGameDir} disabled={!profile}>
            ゲームフォルダ
          </button>
        </div>
      </div>

      {profile && duplicateGroups.length > 0 ? (
        <article className="empty-state warning duplicate-warning">
          <strong>重複している Mod を検知しました</strong>
          <p>
            同じ Mod が {duplicateGroups.length} グループ、合計 {duplicateCount} 件見つかりました。
            片方は削除し、バージョン違いなら更新を使って整理してください。
          </p>
        </article>
      ) : null}

      {!profile ? (
        <article className="empty-state">
          <strong>起動構成が未選択です</strong>
          <p>上の一覧から起動構成を選ぶと Mod 管理を始められます。</p>
        </article>
      ) : filteredMods.length === 0 ? (
        <article className="empty-state">
          <strong>表示できる Mod がありません</strong>
          <p>Discover から導入するか、mods フォルダへ `.jar` を追加するとここに反映されます。</p>
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
