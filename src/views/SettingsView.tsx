import { formatBytes } from "../app/formatters";
import type { AppSettings, SoftwareStatus } from "../app/types";

type SettingsViewProps = {
  settings: AppSettings | null;
  status: SoftwareStatus | null;
  busy: boolean;
  onToggleTempCache: (enabled: boolean) => void;
  onChangePerformanceLiteMode: (mode: AppSettings["performanceLiteMode"]) => void;
  onEnsureJavaRuntime: () => void;
  onRefreshStatus: () => void;
  onClearTempCache: () => void;
  onExportDebugLog: () => void;
  onOpenTempRoot: () => void;
};

export function SettingsView({
  settings,
  status,
  busy,
  onToggleTempCache,
  onChangePerformanceLiteMode,
  onEnsureJavaRuntime,
  onRefreshStatus,
  onClearTempCache,
  onExportDebugLog,
  onOpenTempRoot,
}: SettingsViewProps) {
  const liteModeLabel =
    settings?.performanceLiteMode === "on"
      ? "常に有効"
      : settings?.performanceLiteMode === "off"
        ? "常に無効"
        : "自動";

  const healthBadgeTone = settings?.tempCacheEnabled ? "is-healthy" : "is-neutral";
  const healthBadgeLabel = settings?.tempCacheEnabled
    ? "キャッシュ最適化: 稼働中"
    : "キャッシュ最適化: 停止中";

  return (
    <section className="workspace settings-workspace">
      <article className="panel wide settings-panel">
        <header className="settings-header">
          <div className="section-copy">
            <span className="section-kicker">Settings</span>
            <h3>ランチャー環境の調整</h3>
            <p>
              Temp キャッシュ、軽量モード、Java ランタイムを一括で管理し、起動体験を安定化します。
            </p>
          </div>
          <div className={`settings-health ${healthBadgeTone}`}>
            <span>System Health</span>
            <strong>{healthBadgeLabel}</strong>
          </div>
        </header>

        <div className="settings-strip" role="status" aria-live="polite">
          <article>
            <span>Temp キャッシュ</span>
            <strong>{settings?.tempCacheEnabled ? "有効" : "無効"}</strong>
          </article>
          <article>
            <span>軽量モード</span>
            <strong>{liteModeLabel}</strong>
          </article>
          <article>
            <span>キャッシュサイズ</span>
            <strong>{formatBytes(status?.cacheTotalBytes ?? 0)}</strong>
          </article>
          <article>
            <span>キャッシュ件数</span>
            <strong>{status?.cacheFileCount ?? 0}</strong>
          </article>
        </div>

        <div className="settings-grid">
          <article className="settings-card">
            <div className="settings-card-head">
              <span className="section-kicker">Mode Control</span>
              <strong>軽量モード</strong>
            </div>
            <p className="settings-card-text">
              低スペック環境ではアニメーションを抑え、全体の応答性を優先します。
            </p>
            <div className="segmented" role="group" aria-label="軽量モード設定">
              <button
                type="button"
                className={settings?.performanceLiteMode === "auto" ? "is-active" : ""}
                onClick={() => onChangePerformanceLiteMode("auto")}
                disabled={busy || !settings}
              >
                軽量: 自動
              </button>
              <button
                type="button"
                className={settings?.performanceLiteMode === "on" ? "is-active" : ""}
                onClick={() => onChangePerformanceLiteMode("on")}
                disabled={busy || !settings}
              >
                軽量: 常にON
              </button>
              <button
                type="button"
                className={settings?.performanceLiteMode === "off" ? "is-active" : ""}
                onClick={() => onChangePerformanceLiteMode("off")}
                disabled={busy || !settings}
              >
                軽量: 常にOFF
              </button>
            </div>
          </article>

          <article className="settings-card">
            <div className="settings-card-head">
              <span className="section-kicker">Runtime</span>
              <strong>実行環境パス</strong>
            </div>
            <div className="detail-list settings-details">
              <div>
                <span>Java ランタイム</span>
                <strong className="path-text">{status?.javaRuntimeDir ?? "取得中..."}</strong>
              </div>
              <div>
                <span>Temp ルート</span>
                <strong className="path-text">{status?.tempRoot ?? "取得中..."}</strong>
              </div>
            </div>
          </article>
        </div>

        <div className="settings-actions">
          <div className="settings-action-row">
            <span className="section-kicker">Cache</span>
            <button
              className={settings?.tempCacheEnabled ? "danger-button" : "play-button"}
              onClick={() => onToggleTempCache(!settings?.tempCacheEnabled)}
              disabled={busy || !settings}
            >
              {settings?.tempCacheEnabled ? "Temp キャッシュを無効化" : "Temp キャッシュを有効化"}
            </button>
            <button className="secondary-button" onClick={onClearTempCache} disabled={busy}>
              Temp キャッシュをクリア
            </button>
            <button className="secondary-button" onClick={onOpenTempRoot} disabled={!status}>
              Temp フォルダを開く
            </button>
          </div>

          <div className="settings-action-row">
            <span className="section-kicker">Maintenance</span>
            <button className="secondary-button" onClick={onEnsureJavaRuntime} disabled={busy}>
              Java を確認 / 導入
            </button>
            <button className="secondary-button" onClick={onRefreshStatus} disabled={busy}>
              状態を再取得
            </button>
            <button className="play-button" onClick={onExportDebugLog} disabled={busy}>
              デバッグ情報をエクスポート
            </button>
          </div>
        </div>
      </article>
    </section>
  );
}
