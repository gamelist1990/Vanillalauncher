import { useEffect, useState } from "react";
import type { ModrinthProject } from "../app/types";
import {
  formatDownloads,
  formatIsoDate,
  formatVersionSupport,
  formatRuntimeSupport,
} from "../app/formatters";

type ProjectDetailModalProps = {
  project: ModrinthProject | null;
  mode: "mods" | "modpacks";
  installLabel: string;
  installDisabled: boolean;
  installed: boolean;
  loading: boolean;
  onClose: () => void;
  onAction: () => void;
  onOpenProject: () => void;
};

export function ProjectDetailModal({
  project,
  installLabel,
  installDisabled,
  installed,
  loading,
  onClose,
  onAction,
  onOpenProject,
}: ProjectDetailModalProps) {
  const [heroFailed, setHeroFailed] = useState(false);
  const [iconFailed, setIconFailed] = useState(false);

  useEffect(() => {
    if (!project) return undefined;
    setHeroFailed(false);
    setIconFailed(false);
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [project, onClose]);

  if (!project) return null;

  const openLabel =
    project.source === "curseforge" ? "CurseForge を開く" : "Modrinth を開く";

  return (
    <div
      className="modal-layer"
      role="dialog"
      aria-modal="true"
      aria-labelledby="project-detail-title"
    >
      <button
        type="button"
        className="modal-backdrop"
        onClick={onClose}
        aria-label="閉じる"
      />

      <article className="modal-sheet modal-sheet-wide project-detail-sheet">
        {/* ---- 閉じるボタン ---- */}
        <button
          type="button"
          className="project-detail-close"
          onClick={onClose}
          aria-label="閉じる"
        >
          ✕
        </button>

        {/* ---- ヒーロー画像 ---- */}
        {project.imageUrl && !heroFailed ? (
          <div className="project-detail-hero">
            <img
              src={project.imageUrl}
              alt=""
              className="project-detail-hero-img"
              loading="lazy"
              decoding="async"
              onError={() => setHeroFailed(true)}
            />
            <div className="project-detail-hero-overlay" />
          </div>
        ) : (
          <div className="project-detail-hero project-detail-hero-empty" />
        )}

        {/* ---- ヘッダー (アイコン + タイトル) ---- */}
        <div className="project-detail-header">
          {project.iconUrl && !iconFailed ? (
            <img
              src={project.iconUrl}
              alt=""
              className="project-detail-icon"
              loading="lazy"
              decoding="async"
              onError={() => setIconFailed(true)}
            />
          ) : (
            <div className="project-detail-icon project-detail-icon-fallback">
              {project.title.slice(0, 1)}
            </div>
          )}
          <div className="project-detail-identity">
            <h3 id="project-detail-title">{project.title}</h3>
            <span>by {project.author}</span>
          </div>
        </div>

        {/* ---- 統計 ---- */}
        <div className="project-detail-stats">
          <div className="project-detail-stat">
            <span>ダウンロード</span>
            <strong>{formatDownloads(project.downloads)}</strong>
          </div>
          {project.followers > 0 ? (
            <div className="project-detail-stat">
              <span>フォロワー</span>
              <strong>{formatDownloads(project.followers)}</strong>
            </div>
          ) : null}
          {project.updatedAt ? (
            <div className="project-detail-stat">
              <span>最終更新</span>
              <strong>{formatIsoDate(project.updatedAt)}</strong>
            </div>
          ) : null}
          {project.latestVersion ? (
            <div className="project-detail-stat">
              <span>最新バージョン</span>
              <strong>{project.latestVersion}</strong>
            </div>
          ) : null}
        </div>

        {/* ---- 説明 ---- */}
        <p className="project-detail-description">{project.description}</p>

        {/* ---- タグ ---- */}
        {project.categories.length > 0 ? (
          <div className="discover-tags">
            {project.categories.slice(0, 8).map((cat) => (
              <span className="badge" key={cat}>
                {cat}
              </span>
            ))}
          </div>
        ) : null}

        {/* ---- 対応情報 ---- */}
        <div className="project-detail-meta">
          <span>対応バージョン: {formatVersionSupport(project.versions)}</span>
          {project.clientSide || project.serverSide ? (
            <span>{formatRuntimeSupport(project.clientSide, project.serverSide)}</span>
          ) : null}
        </div>

        {/* ---- アクション ---- */}
        <div className="modal-actions">
          <button type="button" className="link-button" onClick={onOpenProject}>
            {openLabel}
          </button>
          <button
            type="button"
            className={installed ? "danger-button" : "play-button"}
            disabled={installDisabled || loading}
            onClick={onAction}
          >
            {loading ? "処理中..." : installLabel}
          </button>
        </div>
      </article>
    </div>
  );
}
