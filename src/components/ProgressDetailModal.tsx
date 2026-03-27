import { useEffect, useMemo, useState } from "react";
import {
  formatClockTime,
  formatDateTime,
  formatDurationMs,
  formatRelativeMs,
} from "../app/formatters";
import type { ProgressSnapshot } from "../app/types";

type ProgressDetailModalProps = {
  open: boolean;
  progress: ProgressSnapshot | null;
  onClose: () => void;
};

function estimateRemainingMs(progress: ProgressSnapshot, referenceTime: number) {
  if (progress.status === "completed") {
    return 0;
  }

  // Require a small amount of progress/time before estimating to avoid noise.
  const MIN_ELAPSED_MS = 2000;
  const MIN_PERCENT = 1;

  const elapsed = referenceTime - progress.startedAt;
  if (progress.percent < MIN_PERCENT || elapsed < MIN_ELAPSED_MS) {
    return null;
  }

  // Build ordered samples (oldest -> newest) and include the current reference point.
  const samples: { t: number; p: number }[] = progress.history.map((h) => ({ t: h.timestamp, p: h.percent }));
  // Ensure we have a starting sample; fall back to startedAt if history is empty.
  if (samples.length === 0) {
    samples.push({ t: progress.startedAt, p: 0 });
  }

  // Append the latest observed point (referenceTime, current percent).
  const last = samples[samples.length - 1];
  if (referenceTime !== last.t || progress.percent !== last.p) {
    samples.push({ t: referenceTime, p: progress.percent });
  }

  // Calculate instantaneous speeds (percent per ms) between consecutive samples.
  const speeds: number[] = [];
  for (let i = 1; i < samples.length; i++) {
    const dt = samples[i].t - samples[i - 1].t;
    const dp = samples[i].p - samples[i - 1].p;
    if (dt > 0 && dp > 0) {
      speeds.push(dp / dt);
    }
  }

  if (speeds.length === 0) {
    return null;
  }

  // Use the median speed to reduce sensitivity to spikes/outliers.
  speeds.sort((a, b) => a - b);
  const mid = Math.floor(speeds.length / 2);
  const medianSpeed = speeds.length % 2 === 1 ? speeds[mid] : (speeds[mid - 1] + speeds[mid]) / 2;

  if (!Number.isFinite(medianSpeed) || medianSpeed <= 0) {
    return null;
  }

  const remainingPercent = Math.max(0, 100 - progress.percent);
  const estimatedRemainingMs = remainingPercent / medianSpeed;

  if (!Number.isFinite(estimatedRemainingMs) || estimatedRemainingMs < 0) {
    return null;
  }

  return estimatedRemainingMs;
}

export function ProgressDetailModal({
  open,
  progress,
  onClose,
}: ProgressDetailModalProps) {
  const [now, setNow] = useState(() => Date.now());

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
    if (!open || !progress || progress.status !== "active") {
      return undefined;
    }

    const timer = window.setInterval(() => {
      setNow(Date.now());
    }, 1000);

    return () => {
      window.clearInterval(timer);
    };
  }, [open, progress]);

  useEffect(() => {
    if (!open) {
      return;
    }

    setNow(Date.now());
  }, [open, progress?.operationId]);

  const derived = useMemo(() => {
    if (!progress) {
      return null;
    }

    const referenceTime =
      progress.status === "active"
        ? now
        : progress.completedAt ?? progress.updatedAt;
    const elapsedMs = Math.max(0, referenceTime - progress.startedAt);
    const remainingMs = estimateRemainingMs(progress, referenceTime);
    const etaTimestamp = remainingMs !== null ? referenceTime + remainingMs : null;
    const statusLabel =
      progress.status === "completed"
        ? "完了"
        : progress.status === "active"
          ? "進行中"
          : "通知終了";

    return {
      elapsedMs,
      etaTimestamp,
      history: [...progress.history].reverse(),
      referenceTime,
      remainingMs,
      statusLabel,
    };
  }, [now, progress]);

  if (!open || !progress || !derived) {
    return null;
  }

  return (
    <div
      className="modal-layer"
      role="dialog"
      aria-modal="true"
      aria-labelledby="progress-detail-title"
    >
      <button
        type="button"
        className="modal-backdrop"
        onClick={onClose}
        aria-label="進捗詳細を閉じる"
      />

      <article className="modal-sheet modal-sheet-wide progress-detail-sheet">
        <div className="modal-copy">
          <span className="section-kicker">インストール進捗</span>
          <h3 id="progress-detail-title">{progress.title}</h3>
          <p>{progress.detail}</p>
        </div>

        <section className="progress-detail-hero">
          <div className="progress-detail-primary">
            <div className="progress-detail-status-row">
              <span className={`progress-status-pill is-${progress.status}`}>
                {derived.statusLabel}
              </span>
              <strong>{Math.round(progress.percent)}%</strong>
            </div>

            <div className="progress-track progress-track-large" aria-hidden="true">
              <div
                className="progress-fill"
                style={{ width: `${Math.max(4, Math.min(100, progress.percent))}%` }}
              />
            </div>

            <p className="progress-detail-caption">
              ETA は現在の進捗速度からの推定です。更新頻度が低い処理は再計算待ちになります。
            </p>
          </div>

          <div className="progress-detail-metrics">
            <article>
              <span>経過時間</span>
              <strong>{formatDurationMs(derived.elapsedMs)}</strong>
            </article>
            <article>
              <span>推定残り</span>
              <strong>
                {derived.remainingMs === null
                  ? progress.status === "completed"
                    ? "完了"
                    : "計測中"
                  : formatDurationMs(derived.remainingMs)}
              </strong>
            </article>
            <article>
              <span>ETA</span>
              <strong>
                {derived.etaTimestamp === null
                  ? progress.status === "completed"
                    ? "到達済み"
                    : "計測中"
                  : formatDateTime(derived.etaTimestamp)}
              </strong>
            </article>
            <article>
              <span>開始</span>
              <strong>{formatClockTime(progress.startedAt)}</strong>
            </article>
            <article>
              <span>最終更新</span>
              <strong>{formatRelativeMs(derived.referenceTime - progress.updatedAt)}</strong>
              <small>{formatClockTime(progress.updatedAt)}</small>
            </article>
            <article>
              <span>履歴件数</span>
              <strong>{progress.history.length} 件</strong>
              <small>初回記録 {formatDateTime(progress.startedAt)}</small>
            </article>
          </div>
        </section>

        <section className="progress-detail-section">
          <div className="progress-detail-section-head">
            <div>
              <span className="section-kicker">現在の状況</span>
              <h4>ダウンロードとセットアップの流れ</h4>
            </div>
            <p>
              進捗通知で受け取ったステップを時系列で表示しています。直近の更新ほど上に並びます。
            </p>
          </div>

          <ol className="progress-timeline">
            {derived.history.map((entry) => (
              <li className="progress-timeline-item" key={entry.id}>
                <span className="progress-timeline-marker" aria-hidden="true" />
                <div className="progress-timeline-copy">
                  <div className="progress-timeline-head">
                    <strong>{entry.title}</strong>
                    <span>
                      {Math.round(entry.percent)}% | {formatClockTime(entry.timestamp)}
                    </span>
                  </div>
                  <p>{entry.detail}</p>
                </div>
              </li>
            ))}
          </ol>
        </section>

        <div className="modal-actions">
          <button type="button" className="secondary-button" onClick={onClose}>
            閉じる
          </button>
        </div>
      </article>
    </div>
  );
}
