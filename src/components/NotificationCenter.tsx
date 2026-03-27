import { useEffect } from "react";
import type { Notice, ProgressState } from "../app/types";

type NotificationCenterProps = {
  notices: Notice[];
  progressItems: ProgressState[];
  onDismissNotice: (noticeId: string) => void;
  onOpenProgress: (operationId: string) => void;
};

export function NotificationCenter({
  notices,
  progressItems,
  onDismissNotice,
  onOpenProgress,
}: NotificationCenterProps) {
  useEffect(() => {
    if (notices.length === 0) {
      return;
    }

    const timers = notices.map((notice) =>
      window.setTimeout(
        () => onDismissNotice(notice.id),
        notice.tone === "error" ? 6400 : 4200,
      ),
    );

    return () => {
      timers.forEach((timer) => window.clearTimeout(timer));
    };
  }, [notices, onDismissNotice]);

  if (notices.length === 0 && progressItems.length === 0) {
    return null;
  }

  return (
    <aside className="notification-viewport" aria-live="polite">
      {progressItems.map((item) => (
        <button
          className="progress-toast progress-toast-button"
          key={item.operationId}
          type="button"
          onClick={() => onOpenProgress(item.operationId)}
          aria-label={`${item.title} の詳細な進捗を開く`}
        >
          <div className="progress-toast-head">
            <span className="progress-toast-kicker">進行中</span>
            <strong>{Math.round(item.percent)}%</strong>
          </div>
          <strong className="progress-toast-title">{item.title}</strong>
          <p className="progress-toast-detail">{item.detail}</p>
          <div className="progress-track" aria-hidden="true">
            <div
              className="progress-fill"
              style={{ width: `${Math.max(4, Math.min(100, item.percent))}%` }}
            />
          </div>
          <div className="progress-toast-foot">
            <span>経過時間・ETA・履歴を表示</span>
            <span className="progress-toast-open">詳細を開く</span>
          </div>
        </button>
      ))}

      {notices.map((notice) => (
        <article className={`toast-item toast-${notice.tone}`} key={notice.id}>
          <div className="toast-copy">
            <strong>{notice.tone === "success" ? "完了" : notice.tone === "error" ? "エラー" : "案内"}</strong>
            <p>{notice.text}</p>
          </div>
          <button
            className="toast-dismiss"
            type="button"
            aria-label="通知を閉じる"
            onClick={() => onDismissNotice(notice.id)}
          >
            ×
          </button>
        </article>
      ))}
    </aside>
  );
}
