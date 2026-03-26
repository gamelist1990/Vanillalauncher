import { useEffect, useMemo, useRef, useState } from "react";
import { formatIsoDate } from "../app/formatters";
import type { ModpackVersionSummary } from "../app/types";

type ModpackVersionModalProps = {
  open: boolean;
  projectTitle: string;
  versions: ModpackVersionSummary[];
  selectedVersionId: string;
  loading: boolean;
  onClose: () => void;
  onConfirm: () => void;
  onSelectVersionId: (value: string) => void;
};

export function ModpackVersionModal({
  open,
  projectTitle,
  versions,
  selectedVersionId,
  loading,
  onClose,
  onConfirm,
  onSelectVersionId,
}: ModpackVersionModalProps) {
  const [selectedGameVersion, setSelectedGameVersion] = useState("all");
  const [isGameVersionListOpen, setIsGameVersionListOpen] = useState(false);
  const [isVersionListOpen, setIsVersionListOpen] = useState(false);
  const gameVersionSelectRef = useRef<HTMLDivElement>(null);
  const versionSelectRef = useRef<HTMLDivElement>(null);

  const gameVersionOptions = useMemo(() => {
    const values = new Set<string>();
    for (const entry of versions) {
      for (const gameVersion of entry.gameVersions) {
        if (gameVersion.trim() !== "") {
          values.add(gameVersion.trim());
        }
      }
    }

    return Array.from(values).sort((left, right) => right.localeCompare(left, undefined, { numeric: true }));
  }, [versions]);

  const filteredVersions = useMemo(() => {
    if (selectedGameVersion === "all") {
      return versions;
    }

    return versions.filter((entry) =>
      entry.gameVersions.some((gameVersion) => gameVersion === selectedGameVersion),
    );
  }, [versions, selectedGameVersion]);

  const selectedVersion = useMemo(
    () => filteredVersions.find((entry) => entry.id === selectedVersionId) ?? null,
    [filteredVersions, selectedVersionId],
  );

  const selectedVersionLabel = selectedVersion
    ? `${selectedVersion.versionNumber} | MC ${selectedVersion.gameVersions.slice(0, 2).join("/") || "不明"} | ${formatIsoDate(selectedVersion.publishedAt)}`
    : "配布バージョンを選択";
  const selectedGameVersionLabel =
    selectedGameVersion === "all" ? "すべて" : selectedGameVersion;

  useEffect(() => {
    if (!open) {
      return undefined;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [open, onClose]);

  useEffect(() => {
    if (!open) {
      return;
    }

    setSelectedGameVersion("all");
    setIsGameVersionListOpen(false);
    setIsVersionListOpen(false);
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }

    setIsVersionListOpen(false);
  }, [open, selectedGameVersion]);

  useEffect(() => {
    if (!isVersionListOpen) {
      return undefined;
    }

    const handlePointerDown = (event: MouseEvent) => {
      if (!versionSelectRef.current?.contains(event.target as Node)) {
        setIsVersionListOpen(false);
      }
    };

    window.addEventListener("mousedown", handlePointerDown);
    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
    };
  }, [isVersionListOpen]);

  useEffect(() => {
    if (!isGameVersionListOpen) {
      return undefined;
    }

    const handlePointerDown = (event: MouseEvent) => {
      if (!gameVersionSelectRef.current?.contains(event.target as Node)) {
        setIsGameVersionListOpen(false);
      }
    };

    window.addEventListener("mousedown", handlePointerDown);
    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
    };
  }, [isGameVersionListOpen]);

  useEffect(() => {
    if (filteredVersions.length === 0) {
      return;
    }

    if (!filteredVersions.some((entry) => entry.id === selectedVersionId)) {
      onSelectVersionId(filteredVersions[0].id);
    }
  }, [filteredVersions, onSelectVersionId, selectedVersionId]);

  if (!open) {
    return null;
  }

  return (
    <div className="modal-layer" role="dialog" aria-modal="true" aria-labelledby="modpack-version-title">
      <button
        type="button"
        className="modal-backdrop"
        onClick={onClose}
        aria-label="Modpack バージョン選択を閉じる"
      />

      <article className="modal-sheet modal-sheet-wide">
        <div className="modal-copy">
          <span className="section-kicker">Modpack 構成作成</span>
          <h3 id="modpack-version-title">{projectTitle} のバージョンを選択</h3>
          <p>この Modpack が配布しているバージョンから、作成する構成の基準を選びます。</p>
        </div>

        <label className="modal-field">
          <span>Minecraft バージョンで絞り込み</span>
          <div className="modal-select-shell" ref={gameVersionSelectRef}>
            <button
              type="button"
              className="modal-select-trigger"
              onClick={() => setIsGameVersionListOpen((current) => !current)}
              disabled={loading || gameVersionOptions.length === 0}
            >
              <span>{selectedGameVersionLabel}</span>
              <span aria-hidden="true" className={`modal-select-arrow ${isGameVersionListOpen ? "is-open" : ""}`}>
                v
              </span>
            </button>

            {isGameVersionListOpen ? (
              <div className="modal-select-menu" role="listbox" aria-label="Minecraft バージョン一覧">
                <button
                  type="button"
                  className={`modal-select-option ${selectedGameVersion === "all" ? "is-selected" : ""}`}
                  onClick={() => {
                    setSelectedGameVersion("all");
                    setIsGameVersionListOpen(false);
                  }}
                >
                  すべて
                </button>
                {gameVersionOptions.map((entry) => (
                  <button
                    type="button"
                    key={entry}
                    className={`modal-select-option ${selectedGameVersion === entry ? "is-selected" : ""}`}
                    onClick={() => {
                      setSelectedGameVersion(entry);
                      setIsGameVersionListOpen(false);
                    }}
                  >
                    {entry}
                  </button>
                ))}
              </div>
            ) : null}
          </div>
        </label>

        <label className="modal-field">
          <span>配布バージョン</span>
          <div className="modal-select-shell" ref={versionSelectRef}>
            <button
              type="button"
              className="modal-select-trigger"
              onClick={() => setIsVersionListOpen((current) => !current)}
              disabled={loading || filteredVersions.length === 0}
            >
              <span>{selectedVersionLabel}</span>
              <span aria-hidden="true" className={`modal-select-arrow ${isVersionListOpen ? "is-open" : ""}`}>
                v
              </span>
            </button>

            {isVersionListOpen ? (
              <div className="modal-select-menu" role="listbox" aria-label="配布バージョン一覧">
                {filteredVersions.map((entry) => {
                  const optionLabel = `${entry.versionNumber} | MC ${entry.gameVersions.slice(0, 2).join("/") || "不明"} | ${formatIsoDate(entry.publishedAt)}`;
                  const selected = entry.id === selectedVersionId;

                  return (
                    <button
                      type="button"
                      key={entry.id}
                      className={`modal-select-option ${selected ? "is-selected" : ""}`}
                      onClick={() => {
                        onSelectVersionId(entry.id);
                        setIsVersionListOpen(false);
                      }}
                    >
                      {optionLabel}
                    </button>
                  );
                })}
              </div>
            ) : null}
          </div>
        </label>

        <div className="modal-actions">
          <button type="button" className="secondary-button" onClick={onClose} disabled={loading}>
            キャンセル
          </button>
          <button
            type="button"
            className="play-button"
            onClick={onConfirm}
            disabled={loading || filteredVersions.length === 0 || !selectedVersionId}
          >
            {loading ? "構成を作成中..." : "このバージョンで構成を作成"}
          </button>
        </div>
      </article>
    </div>
  );
}
