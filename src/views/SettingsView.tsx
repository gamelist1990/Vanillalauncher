import { formatBytes } from "../app/formatters";
import type { AppSettings, SoftwareStatus } from "../app/types";

type SettingsViewProps = {
  settings: AppSettings | null;
  status: SoftwareStatus | null;
  busy: boolean;
  onToggleTempCache: (enabled: boolean) => void;
  onToggleOfflineMode: (enabled: boolean) => void;
  onChangeOfflineUsername: (username: string) => void;
  onToggleOfficialLauncherAutoInstall: (enabled: boolean) => void;
  onEnsureOfficialLauncher: (reinstall?: boolean) => void;
  onEnsureJavaRuntime: () => void;
  onChangeJavaRuntimeMode: (mode: NonNullable<AppSettings["javaRuntimeMode"]>) => void;
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
  onToggleOfficialLauncherAutoInstall,
  onEnsureOfficialLauncher,
  onEnsureJavaRuntime,
  onChangeJavaRuntimeMode,
  onSelectCustomJavaPath,
  onClearCustomJavaPath,
  onRefreshStatus,
  onClearTempCache,
  onExportDebugLog,
  onOpenTempRoot,
}: SettingsViewProps) {
  const healthBadgeTone = settings?.tempCacheEnabled ? "is-healthy" : "is-neutral";
  const healthBadgeLabel = settings?.tempCacheEnabled
    ? "キャッシュ最適化: 稼働中"
    : "キャッシュ最適化: 停止中";
  const customJavaPath = settings?.customJavaPath ?? status?.customJavaPath ?? "";
  const javaRuntimeMode = settings?.javaRuntimeMode ?? status?.javaRuntimeMode ?? "auto";
  const officialLauncherAutoInstall = settings?.officialLauncherAutoInstall ?? false;

  return (
    <section className="workspace settings-workspace">
      {/* ===== ページヘッダー ===== */}
      <div className="panel settings-header-card">
        <div className="settings-header-top">
          <div>
            <p className="eyebrow">Settings</p>
            <h2 className="header-title">ランチャー設定</h2>
            <p className="header-subtitle">
              Temp キャッシュ・Java ランタイムを管理します
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
            <span>公式 Launcher</span>
            <strong>{status?.officialLauncherAvailable ? "検出済み" : "未検出"}</strong>
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

      {/* ===== 公式 Launcher ===== */}
      <div className="settings-section">
        <h3 className="settings-section-title">🟩 公式 Minecraft Launcher</h3>
        <div className="settings-field panel">
          <div className="settings-field-header">
            <div>
              <p className="settings-field-label">公式 Launcher の自動導入</p>
              <p className="settings-field-desc">
                公式 Launcher が見つからない場合に自動導入を試みます。既に導入済みの場合は「再インストール」で入れ直せます。
              </p>
            </div>
          </div>
          <div className="segmented" role="group" aria-label="公式 Launcher 自動導入設定">
            <button
              type="button"
              className={!officialLauncherAutoInstall ? "is-active" : ""}
              onClick={() => onToggleOfficialLauncherAutoInstall(false)}
              disabled={busy || !settings}
            >
              手動
            </button>
            <button
              type="button"
              className={officialLauncherAutoInstall ? "is-active" : ""}
              onClick={() => onToggleOfficialLauncherAutoInstall(true)}
              disabled={busy || !settings}
            >
              自動導入
            </button>
          </div>
          <div className="settings-path-list" style={{ marginTop: 14 }}>
            <div className="settings-path-row">
              <span className="settings-path-label">導入状態</span>
              <code className="settings-path-value">
                {status?.officialLauncherAvailable ? "公式 Launcher を検出しました" : "公式 Launcher が見つかりません"}
              </code>
            </div>
            <div className="settings-path-row">
              <span className="settings-path-label">導入メモ</span>
              <code className="settings-path-value">
                {status?.officialLauncherInstaller ?? "取得中..."}
              </code>
            </div>
          </div>
          <div className="settings-field-actions">
            <button className="secondary-button" onClick={() => onEnsureOfficialLauncher(false)} disabled={busy}>
              公式 Launcher を確認 / 導入
            </button>
            <button className="danger-button" onClick={() => onEnsureOfficialLauncher(true)} disabled={busy}>
              公式 Launcher を再インストール
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
          <div className="settings-field-header" style={{ marginBottom: 14 }}>
            <div>
              <p className="settings-field-label">JVM 選択</p>
              <p className="settings-field-desc">
                通常は自動推奨です。必要な場合だけ Java 17 / 21 / 25 を固定できます。
              </p>
            </div>
          </div>
          <div className="segmented" role="group" aria-label="JVM 選択">
            <button
              type="button"
              className={javaRuntimeMode === "auto" ? "is-active" : ""}
              onClick={() => onChangeJavaRuntimeMode("auto")}
              disabled={busy || !settings}
            >
              自動
            </button>
            <button
              type="button"
              className={javaRuntimeMode === "java17" ? "is-active" : ""}
              onClick={() => onChangeJavaRuntimeMode("java17")}
              disabled={busy || !settings}
            >
              Java 17
            </button>
            <button
              type="button"
              className={javaRuntimeMode === "java21" ? "is-active" : ""}
              onClick={() => onChangeJavaRuntimeMode("java21")}
              disabled={busy || !settings}
            >
              Java 21
            </button>
            <button
              type="button"
              className={javaRuntimeMode === "java25" ? "is-active" : ""}
              onClick={() => onChangeJavaRuntimeMode("java25")}
              disabled={busy || !settings}
            >
              Java 25
            </button>
          </div>
          <div className="settings-path-list">
            <div className="settings-path-row">
              <span className="settings-path-label">JVM モード</span>
              <code className="settings-path-value">
                {javaRuntimeMode === "auto"
                  ? "自動（Minecraft バージョンに合わせる）"
                  : javaRuntimeMode === "java17"
                    ? "Java 17 に固定"
                    : javaRuntimeMode === "java21"
                      ? "Java 21 に固定"
                      : "Java 25 に固定"}
              </code>
            </div>
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
