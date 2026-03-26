import { useEffect, useRef } from "react";

type ProfileNameModalProps = {
  open: boolean;
  profileName: string;
  onClose: () => void;
  onConfirm: () => void;
  onChangeProfileName: (value: string) => void;
};

export function ProfileNameModal({
  open,
  profileName,
  onClose,
  onConfirm,
  onChangeProfileName,
}: ProfileNameModalProps) {
  const inputRef = useRef<HTMLInputElement>(null);

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
    window.setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 0);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  if (!open) {
    return null;
  }

  return (
    <div className="modal-layer" role="dialog" aria-modal="true" aria-labelledby="profile-name-title">
      <button
        type="button"
        className="modal-backdrop"
        onClick={onClose}
        aria-label="名前編集ダイアログを閉じる"
      />

      <article className="modal-sheet">
        <div className="modal-copy">
          <span className="section-kicker">名前を編集</span>
          <h3 id="profile-name-title">起動構成の表示名を変更</h3>
          <p>この画面で変更した名前は launcher_profiles.json に保存されます。</p>
        </div>

        <label className="modal-field">
          <span>起動構成名</span>
          <input
            ref={inputRef}
            value={profileName}
            onChange={(event) => onChangeProfileName(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                event.preventDefault();
                onConfirm();
              }
            }}
            placeholder="Fabric 1.21.11"
            autoComplete="off"
          />
        </label>

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
