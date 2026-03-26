import {
  formatDownloads,
  formatIsoDate,
  formatRuntimeSupport,
  formatVersionSupport,
} from "../app/formatters";
import { useState } from "react";
import type { ModrinthProject } from "../app/types";

type DiscoverResultRowProps = {
  project: ModrinthProject;
  mode: "mods" | "modpacks";
  disabled: boolean;
  installState: "install" | "update" | "installed" | "blocked";
  installed: boolean;
  loading: boolean;
  onAction: () => void;
  onOpenProject: () => void;
};

export function DiscoverResultRow({
  project,
  mode,
  disabled,
  installState,
  installed,
  loading,
  onAction,
  onOpenProject,
}: DiscoverResultRowProps) {
  const [previewFailed, setPreviewFailed] = useState(false);
  const [iconFailed, setIconFailed] = useState(false);
  const iconImage = project.iconUrl ?? project.imageUrl ?? null;
  const previewImage = project.imageUrl ?? null;
  const installLabel =
    mode === "modpacks"
      ? loading
        ? "構成を作成中..."
        : "構成を作成"
      : installState === "blocked"
        ? "重複あり"
        : disabled
          ? "Loader 導入後"
        : loading
          ? installState === "update"
            ? "更新中..."
            : "導入中..."
          : installState === "update"
            ? "更新"
            : installState === "installed"
              ? "導入済み"
              : installed
                ? "アンインストール"
                : "インストール";
  const openLabel = project.source === "curseforge" ? "CurseForge を開く" : "Modrinth を開く";

  return (
    <article className="discover-result-row">
      <div className="discover-result-visual">
        {previewImage && !previewFailed ? (
          <img
            src={previewImage}
            alt=""
            className="discover-preview discover-preview-hero"
            loading="lazy"
            decoding="async"
            onError={() => setPreviewFailed(true)}
          />
        ) : (
          <div className="discover-preview-fallback">
            <span>{project.title.slice(0, 1)}</span>
          </div>
        )}
        <div className="discover-preview-overlay" />
        <div className="discover-icon-frame">
          {iconImage && !iconFailed ? (
            <img
              src={iconImage}
              alt=""
              className="project-icon discover-project-icon"
              loading="lazy"
              decoding="async"
              onError={() => setIconFailed(true)}
            />
          ) : (
            <div className="project-icon fallback">{project.title.slice(0, 1)}</div>
          )}
        </div>
        <span
          className={`discover-state-pill ${
            installState === "blocked" || installState === "update" || installed
              ? "is-installed"
              : "is-ready"
          }`}
        >
          {mode === "modpacks"
            ? "新規構成"
            : installState === "blocked"
              ? "重複"
              : installState === "update"
                ? "更新可"
                : installed
                  ? "導入済み"
                  : "おすすめ"}
        </span>
      </div>

      <div className="discover-result-copy">
        <div className="discover-result-heading">
          <div className="discover-identity">
            <strong>{project.title}</strong>
            <span>by {project.author}</span>
          </div>

          <div className="discover-inline-stats">
            <span>{formatDownloads(project.downloads)} DL</span>
            <span>{formatIsoDate(project.updatedAt)}</span>
            {project.followers > 0 ? <span>{formatDownloads(project.followers)} Follow</span> : null}
          </div>
        </div>

        <p className="discover-description">{project.description}</p>

        <div className="discover-result-footer">
          <div className="discover-tags">
            {(project.categories.length > 0 ? project.categories : ["mod"]).slice(0, 5).map((category) => (
              <span className="badge" key={`${project.projectId}-${category}`}>
                {category}
              </span>
            ))}
          </div>

          <div className="discover-mini-meta">
            <span>対応版 {formatVersionSupport(project.versions)}</span>
            <span>{formatRuntimeSupport(project.clientSide, project.serverSide)}</span>
            {project.latestVersion ? <span>最新 {project.latestVersion}</span> : null}
          </div>
        </div>
      </div>

      <div className="discover-result-actions">
        <button className="link-button" type="button" onClick={onOpenProject}>
          {openLabel}
        </button>
        <button
          className={installed ? "danger-button compact" : "play-button compact"}
          type="button"
          disabled={disabled || loading}
          onClick={onAction}
        >
          {installLabel}
        </button>
      </div>
    </article>
  );
}
