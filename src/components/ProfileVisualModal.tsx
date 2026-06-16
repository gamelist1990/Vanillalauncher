import { useEffect } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";

const IMAGE_FILTERS = [
  { name: "画像ファイル", extensions: ["png", "jpg", "jpeg", "webp", "gif", "avif"] },
];

type ProfileVisualModalProps = {
  open: boolean;
  profileName: string;
  draftName: string;
  iconUrl: string;
  backgroundImageUrl: string;
  onClose: () => void;
  onConfirm: () => void;
  onChangeName: (value: string) => void;
  onChangeIconUrl: (value: string) => void;
  onChangeBackgroundImageUrl: (value: string) => void;
};

export function ProfileVisualModal({
  open,
  profileName,
  draftName,
  iconUrl,
  backgroundImageUrl,
  onClose,
  onConfirm,
  onChangeName,
  onChangeIconUrl,
  onChangeBackgroundImageUrl,
}: ProfileVisualModalProps) {
  useEffect(() => {
    if (!open) {
      return undefined;
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  if (!open) return null;

  const iconSrc = iconUrl ? convertFileSrc(iconUrl) : undefined;
  const bgSrc = backgroundImageUrl ? convertFileSrc(backgroundImageUrl) : undefined;
  const displayName = draftName.trim() || profileName;

  async function pickIcon() {
    const selected = await openFileDialog({
      title: "アイコン画像を選択",
      filters: IMAGE_FILTERS,
      multiple: false,
    });
    if (typeof selected === "string") onChangeIconUrl(selected);
  }

  async function pickBackground() {
    const selected = await openFileDialog({
      title: "カード壁紙を選択",
      filters: IMAGE_FILTERS,
      multiple: false,
    });
    if (typeof selected === "string") onChangeBackgroundImageUrl(selected);
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
          <span className="section-kicker">プロフィールのカスタマイズ</span>
          <h3 id="profile-visual-title">{profileName} の外観を変更</h3>
        </div>

        {/* ライブプレビューカード */}
        <div
          className="visual-preview-card"
          style={bgSrc ? { backgroundImage: `url(${bgSrc})` } : undefined}
        >
          <div className="visual-preview-icon-wrap">
            {iconSrc ? (
              <img src={iconSrc} alt="アイコンプレビュー" className="visual-preview-icon" />
            ) : (
              <div className="visual-preview-icon-fallback">
                {displayName.slice(0, 2).toUpperCase()}
              </div>
            )}
          </div>
          <div className="visual-preview-copy">
            <span className="visual-preview-name">{displayName}</span>
            <span className="visual-preview-sub">プレビュー</span>
          </div>
        </div>

        <div className="visual-modal-fields">
          {/* 名前フィールド */}
          <label className="modal-field">
            <span>プロフィール名</span>
            <input
              value={draftName}
              onChange={(event) => onChangeName(event.currentTarget.value)}
              placeholder={profileName}
              autoComplete="off"
            />
          </label>

          {/* アイコン選択 */}
          <div className="visual-file-field">
            <div className="visual-file-label-row">
              <span className="visual-file-label">アイコン画像</span>
              {iconUrl && (
                <button
                  type="button"
                  className="visual-file-clear"
                  onClick={() => onChangeIconUrl("")}
                >
                  リセット
                </button>
              )}
            </div>
            <button
              type="button"
              className="visual-file-pick"
              onClick={() => void pickIcon()}
            >
              <span className="visual-file-pick-icon">🖼️</span>
              <span>{iconUrl ? "別のファイルを選ぶ" : "ファイルを選ぶ（PNG / JPG / WEBP）"}</span>
            </button>
            {iconUrl && <p className="visual-file-path">{iconUrl}</p>}
          </div>

          {/* 壁紙選択 */}
          <div className="visual-file-field">
            <div className="visual-file-label-row">
              <span className="visual-file-label">カード壁紙</span>
              {backgroundImageUrl && (
                <button
                  type="button"
                  className="visual-file-clear"
                  onClick={() => onChangeBackgroundImageUrl("")}
                >
                  リセット
                </button>
              )}
            </div>
            <button
              type="button"
              className="visual-file-pick"
              onClick={() => void pickBackground()}
            >
              <span className="visual-file-pick-icon">🌄</span>
              <span>{backgroundImageUrl ? "別のファイルを選ぶ" : "ファイルを選ぶ（PNG / JPG / WEBP）"}</span>
            </button>
            {backgroundImageUrl && <p className="visual-file-path">{backgroundImageUrl}</p>}
          </div>
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