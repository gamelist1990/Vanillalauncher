import { useEffect, useRef } from "react";
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
  // タイマーを notice.id ごとに管理することで、他の通知の追加/削除でタイマーがリセットされないようにする
  const timersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  useEffect(() => {
    const currentIds = new Set(notices.map((n) => n.id));

    // 既に消えた notice のタイマーを削除
    for (const [id, timer] of timersRef.current) {
      if (!currentIds.has(id)) {
        window.clearTimeout(timer);
        timersRef.current.delete(id);
      }
    }

    // 新しい notice にだけタイマーをセット（既存のものはリセットしない）
    for (const notice of notices) {
      if (!timersRef.current.has(notice.id)) {
        const delay = notice.tone === "error" ? 6400 : 4200;
        const timer = window.setTimeout(() => {
          onDismissNotice(notice.id);
          timersRef.current.delete(notice.id);
        }, delay);
        timersRef.current.set(notice.id, timer);
      }
    }
  }, [notices, onDismissNotice]);

  // アンマウント時に残タイマーをすべて解除
  useEffect(() => {
    return () => {
      for (const timer of timersRef.current.values()) {
        window.clearTimeout(timer);
      }
    };
  }, []);

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
            <strong>
              {notice.tone === "success" ? "✓ 完了" : notice.tone === "error" ? "✕ エラー" : "ℹ 案内"}
            </strong>
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
