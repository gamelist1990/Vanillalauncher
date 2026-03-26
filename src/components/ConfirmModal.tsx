import { useEffect } from "react";

type ConfirmModalProps = {
  open: boolean;
  title: string;
  description: string;
  confirmLabel: string;
  tone?: "danger" | "accent";
  onClose: () => void;
  onConfirm: () => void;
};

export function ConfirmModal({
  open,
  title,
  description,
  confirmLabel,
  tone = "danger",
  onClose,
  onConfirm,
}: ConfirmModalProps) {
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
    <div className="modal-layer" role="dialog" aria-modal="true" aria-labelledby="confirm-title">
      <button
        type="button"
        className="modal-backdrop"
        onClick={onClose}
        aria-label="確認ダイアログを閉じる"
      />

      <article className="modal-sheet">
        <div className="modal-copy">
          <span className="section-kicker">確認</span>
          <h3 id="confirm-title">{title}</h3>
          <p>{description}</p>
        </div>

        <div className="modal-actions">
          <button type="button" className="secondary-button" onClick={onClose}>
            キャンセル
          </button>
          <button
            type="button"
            className={tone === "danger" ? "danger-button" : "play-button"}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </article>
    </div>
  );
}
