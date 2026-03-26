import { useEffect } from "react";

type ProfileVisualModalProps = {
  open: boolean;
  title: string;
  description: string;
  iconUrl: string;
  backgroundImageUrl: string;
  onClose: () => void;
  onConfirm: () => void;
  onChangeIconUrl: (value: string) => void;
  onChangeBackgroundImageUrl: (value: string) => void;
};

export function ProfileVisualModal({
  open,
  title,
  description,
  iconUrl,
  backgroundImageUrl,
  onClose,
  onConfirm,
  onChangeIconUrl,
  onChangeBackgroundImageUrl,
}: ProfileVisualModalProps) {
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
    <div className="modal-layer" role="dialog" aria-modal="true" aria-labelledby="profile-visual-title">
      <button
        type="button"
        className="modal-backdrop"
        onClick={onClose}
        aria-label="外観編集ダイアログを閉じる"
      />

      <article className="modal-sheet modal-sheet-wide">
        <div className="modal-copy">
          <span className="section-kicker">外観編集</span>
          <h3 id="profile-visual-title">{title}</h3>
          <p>{description}</p>
        </div>

        <div className="visual-modal-fields">
          <label className="modal-field">
            <span>カードアイコン画像 URL (空欄でリセット)</span>
            <input
              value={iconUrl}
              onChange={(event) => onChangeIconUrl(event.currentTarget.value)}
              placeholder="https://..."
              autoComplete="off"
            />
          </label>

          <label className="modal-field">
            <span>カード壁紙 URL (空欄でリセット)</span>
            <input
              value={backgroundImageUrl}
              onChange={(event) => onChangeBackgroundImageUrl(event.currentTarget.value)}
              placeholder="https://..."
              autoComplete="off"
            />
          </label>
        </div>

        <div className="modal-actions">
          <button type="button" className="secondary-button" onClick={onClose}>
            キャンセル
          </button>
          <button type="button" className="play-button" onClick={onConfirm}>
            保存する
          </button>
        </div>
      </article>
    </div>
  );
}