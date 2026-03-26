import { formatLoader } from "../app/formatters";
import type { InstalledMod, ModRemoteState } from "../app/types";

type ModListItemProps = {
  mod: InstalledMod;
  profileLoader: string;
  remoteState?: ModRemoteState;
  busyAction: string | null;
  onOpenSource: () => void;
  onToggle: () => void;
  onUpdate: () => void;
  onRemove: () => void;
};

function fallbackGlyph(mod: InstalledMod, remoteState?: ModRemoteState) {
  const seed =
    remoteState?.projectTitle ??
    mod.displayName ??
    mod.modId ??
    mod.fileName;
  return seed.slice(0, 1).toUpperCase();
}

export function ModListItem({
  mod,
  profileLoader,
  remoteState,
  busyAction,
  onOpenSource,
  onToggle,
  onUpdate,
  onRemove,
}: ModListItemProps) {
  const updating = busyAction === `update:${mod.fileName}`;
  const toggling = busyAction === `toggle:${mod.fileName}`;
  const removing = busyAction === `remove:${mod.fileName}`;
  const canUpdate = Boolean(remoteState?.updateAvailable && mod.sourceProjectId);
  const isCurseforgeSource =
    remoteState?.source === "curseforge" || mod.sourceProjectId?.startsWith("curseforge:");
  const sourceLabel =
    isCurseforgeSource ? "CurseForge" : remoteState?.source === "modrinth" ? "Modrinth" : null;
  const openSourceTitle = isCurseforgeSource
    ? "mods フォルダを開く"
    : sourceLabel
      ? `${sourceLabel} 情報を開く`
      : "mods フォルダを開く";

  return (
    <article className="mod-card">
      <div className="mod-card-visual">
        <button
          type="button"
          className="mod-icon-button"
          onClick={onOpenSource}
          title={openSourceTitle}
        >
          {remoteState?.iconUrl ? (
            <img
              src={remoteState.iconUrl}
              alt=""
              className="mod-card-icon"
              loading="lazy"
              decoding="async"
            />
          ) : (
            <div className="mod-card-icon mod-card-icon-fallback">
              {fallbackGlyph(mod, remoteState)}
            </div>
          )}
        </button>
      </div>

      <div className="mod-card-body">
        <div className="mod-card-heading">
          <div className="mod-card-title">
            <strong>{remoteState?.projectTitle ?? mod.displayName}</strong>
            <p>{mod.description ?? "説明は取得できませんでした。"}</p>
          </div>

          <div className="mod-card-badges">
            <span className={`badge badge-state ${mod.enabled ? "on" : "off"}`}>
              {mod.enabled ? "有効" : "無効"}
            </span>
            <span className="badge">{formatLoader(mod.loader ?? profileLoader)}</span>
            <span className="badge">{mod.version ?? "バージョン不明"}</span>
            {remoteState?.updateAvailable ? (
              <span className="badge badge-update">更新あり</span>
            ) : null}
          </div>
        </div>

        <div className="mod-card-footer">
          <div className="mod-card-file">
            <span>ファイル</span>
            <strong className="path-text">{mod.fileName}</strong>
          </div>

          <div className="mod-card-actions">
            {canUpdate ? (
              <button
                className="secondary-button accent-button"
                disabled={updating}
                onClick={onUpdate}
              >
                {updating ? "更新中..." : "更新"}
              </button>
            ) : null}
            <button
              className="secondary-button"
              disabled={toggling}
              onClick={onToggle}
            >
              {mod.enabled ? "無効化" : "有効化"}
            </button>
            <button
              className="danger-button"
              disabled={removing}
              onClick={onRemove}
            >
              {removing ? "削除中..." : "削除"}
            </button>
          </div>
        </div>
      </div>
    </article>
  );
}
