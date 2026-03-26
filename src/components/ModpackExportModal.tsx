import { useEffect } from "react";
import type { ModpackExportFormat } from "../app/types";

type ModpackExportModalProps = {
  open: boolean;
  profileName: string;
  selectedFormat: ModpackExportFormat;
  loading: boolean;
  onClose: () => void;
  onConfirm: () => void;
  onSelectFormat: (format: ModpackExportFormat) => void;
};

const formatCopy: Record<
  ModpackExportFormat,
  { label: string; detail: string }
> = {
  curseforge: {
    label: "CurseForge",
    detail: "manifest.json と overrides/ で zip を作成します。",
  },
  modrinth: {
    label: "Modrinth",
    detail: "modrinth.index.json と overrides/ で mrpack を作成します。",
  },
};

export function ModpackExportModal({
  open,
  profileName,
  selectedFormat,
  loading,
  onClose,
  onConfirm,
  onSelectFormat,
}: ModpackExportModalProps) {
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

  if (!open) {
    return null;
  }

  return (
    <div className="modal-layer" role="dialog" aria-modal="true" aria-labelledby="modpack-export-title">
      <button
        type="button"
        className="modal-backdrop"
        onClick={onClose}
        aria-label="書き出し形式モーダルを閉じる"
      />

      <article className="modal-sheet">
        <div className="modal-copy">
          <span className="section-kicker">書き出し形式</span>
          <h3 id="modpack-export-title">{profileName} の書き出し形式を選択</h3>
          <p>どちらの Modpack 形式で保存するか選んでください。</p>
        </div>

        <div className="export-format-list">
          {(["curseforge", "modrinth"] as const).map((format) => (
            <button
              key={format}
              type="button"
              className={`export-format-card ${selectedFormat === format ? "is-selected" : ""}`}
              onClick={() => onSelectFormat(format)}
            >
              <strong>{formatCopy[format].label}</strong>
              <span>{formatCopy[format].detail}</span>
            </button>
          ))}
        </div>

        <div className="modal-actions">
          <button type="button" className="secondary-button" onClick={onClose} disabled={loading}>
            キャンセル
          </button>
          <button type="button" className="play-button" onClick={onConfirm} disabled={loading}>
            {loading ? "準備中..." : "保存先を選ぶ"}
          </button>
        </div>
      </article>
    </div>
  );
}
