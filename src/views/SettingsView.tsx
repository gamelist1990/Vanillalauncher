import { formatBytes } from "../app/formatters";
import type { AppSettings, SoftwareStatus } from "../app/types";

type SettingsViewProps = {
  settings: AppSettings | null;
  status: SoftwareStatus | null;
  busy: boolean;
  onToggleTempCache: (enabled: boolean) => void;
  onChangePerformanceLiteMode: (mode: AppSettings["performanceLiteMode"]) => void;
  onEnsureJavaRuntime: () => void;
  onSelectCustomJavaPath: () => void;
  onClearCustomJavaPath: () => void;
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
  onSelectCustomJavaPath,
  onClearCustomJavaPath,
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
  const customJavaPath = settings?.customJavaPath ?? status?.customJavaPath ?? "";

  return (
    <section className="workspace settings-workspace">
      {/* ===== ページヘッダー ===== */}
      <div className="panel settings-header-card">
        <div className="settings-header-top">
          <div>
            <p className="eyebrow">Settings</p>
            <h2 className="header-title">ランチャー設定</h2>
            <p className="header-subtitle">
              Temp キャッシュ・軽量モード・Java ランタイムを管理します
            </p>
          </div>
          <div className={`settings-health-badge ${healthBadgeTone}`}>
            <span className="settings-health-label">System</span>
            <strong>{healthBadgeLabel}</strong>
          </div>
        </div>

        {/* ステータスチップ */}
        <div className="settings-stat-row">
          <div className="settings-stat">
            <span>Temp キャッシュ</span>
            <strong className={settings?.tempCacheEnabled ? "stat-on" : "stat-off"}>
              {settings?.tempCacheEnabled ? "有効" : "無効"}
            </strong>
          </div>
          <div className="settings-stat">
            <span>軽量モード</span>
            <strong>{liteModeLabel}</strong>
          </div>
          <div className="settings-stat">
            <span>キャッシュサイズ</span>
            <strong>{formatBytes(status?.cacheTotalBytes ?? 0)}</strong>
          </div>
          <div className="settings-stat">
            <span>キャッシュ件数</span>
            <strong>{status?.cacheFileCount ?? 0}</strong>
          </div>
        </div>
      </div>

      {/* ===== 軽量モード ===== */}
      <div className="settings-section">
        <h3 className="settings-section-title">⚡ パフォーマンス</h3>
        <div className="settings-field panel">
          <div className="settings-field-header">
            <div>
              <p className="settings-field-label">軽量モード</p>
              <p className="settings-field-desc">
                低スペック環境ではアニメーションを抑え、全体の応答性を優先します。
              </p>
            </div>
          </div>
          <div className="segmented" role="group" aria-label="軽量モード設定">
            <button
              type="button"
              className={settings?.performanceLiteMode === "auto" ? "is-active" : ""}
              onClick={() => onChangePerformanceLiteMode("auto")}
              disabled={busy || !settings}
            >
              自動
            </button>
            <button
              type="button"
              className={settings?.performanceLiteMode === "on" ? "is-active" : ""}
              onClick={() => onChangePerformanceLiteMode("on")}
              disabled={busy || !settings}
            >
              常にON
            </button>
            <button
              type="button"
              className={settings?.performanceLiteMode === "off" ? "is-active" : ""}
              onClick={() => onChangePerformanceLiteMode("off")}
              disabled={busy || !settings}
            >
              常にOFF
            </button>
          </div>
        </div>
      </div>

      {/* ===== Temp キャッシュ ===== */}
      <div className="settings-section">
        <h3 className="settings-section-title">🗂 Temp キャッシュ</h3>
        <div className="panel settings-field">
          <div className="settings-field-header">
            <div>
              <p className="settings-field-label">キャッシュの有効 / 無効</p>
              <p className="settings-field-desc">
                Temp キャッシュを有効にすると起動が高速化されます。
              </p>
            </div>
          </div>
          <div className="settings-field-actions">
            <button
              className={settings?.tempCacheEnabled ? "danger-button" : "play-button"}
              onClick={() => onToggleTempCache(!settings?.tempCacheEnabled)}
              disabled={busy || !settings}
            >
              {settings?.tempCacheEnabled ? "キャッシュを無効化" : "キャッシュを有効化"}
            </button>
            <button className="secondary-button" onClick={onClearTempCache} disabled={busy}>
              キャッシュをクリア
            </button>
            <button className="secondary-button" onClick={onOpenTempRoot} disabled={!status}>
              Temp フォルダを開く
            </button>
          </div>
        </div>
      </div>

      {/* ===== 実行環境 ===== */}
      <div className="settings-section">
        <h3 className="settings-section-title">☕ 実行環境</h3>
        <div className="panel">
          <div className="settings-path-list">
            <div className="settings-path-row">
              <span className="settings-path-label">Java ランタイム</span>
              <code className="settings-path-value">{status?.javaRuntimeDir ?? "取得中..."}</code>
            </div>
            <div className="settings-path-row">
              <span className="settings-path-label">直接指定 Java</span>
              <code className="settings-path-value">
                {customJavaPath || "未指定（内蔵 Java を使用）"}
              </code>
            </div>
            <div className="settings-path-row">
              <span className="settings-path-label">Temp ルート</span>
              <code className="settings-path-value">{status?.tempRoot ?? "取得中..."}</code>
            </div>
          </div>
          <div className="settings-field-actions">
            <button className="secondary-button" onClick={onEnsureJavaRuntime} disabled={busy}>
              Java を確認 / 導入
            </button>
            <button className="secondary-button" onClick={onSelectCustomJavaPath} disabled={busy}>
              Java パスを指定
            </button>
            <button className="danger-button" onClick={onClearCustomJavaPath} disabled={busy || !customJavaPath}>
              Java 指定を解除
            </button>
            <button className="secondary-button" onClick={onRefreshStatus} disabled={busy}>
              状態を再取得
            </button>
            <button className="play-button" onClick={onExportDebugLog} disabled={busy}>
              デバッグ情報をエクスポート
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}
