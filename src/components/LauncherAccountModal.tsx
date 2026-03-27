import { useEffect } from "react";
import type { LauncherAccountEntry, ProgressState } from "../app/types";

type LauncherAccountModalProps = {
  open: boolean;
  accounts: LauncherAccountEntry[];
  switchingLocalId?: string | null;
  scanning?: boolean;
  scanProgress?: ProgressState | null;
  interactionDisabled?: boolean;
  onClose: () => void;
  onSelectAccount: (localId: string) => void;
  onScanAccounts: () => void;
  onOpenOfficialLauncher: () => void;
};

export function LauncherAccountModal({
  open,
  accounts,
  switchingLocalId,
  scanning = false,
  scanProgress = null,
  interactionDisabled = false,
  onClose,
  onSelectAccount,
  onScanAccounts,
  onOpenOfficialLauncher,
}: LauncherAccountModalProps) {
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

  const launcherSavedCount = accounts.filter((account) => account.authSource === "official-launcher").length;
  const javaReadyCount = accounts.filter((account) => account.hasJavaAccess).length;
  const scanPercent = scanProgress ? Math.round(scanProgress.percent) : null;
  const scanTitle = scanProgress?.title ?? "PC 内を再検出しています";
  const scanDetail = scanProgress?.detail ?? "Launcher 保存先と認証キャッシュを確認しています。完了したらこの一覧をその場で更新します。";

  return (
    <div className="modal-layer" role="dialog" aria-modal="true" aria-labelledby="account-panel-title">
      <button
        type="button"
        className="modal-backdrop"
        onClick={onClose}
        aria-label="アカウントパネルを閉じる"
      />

      <article className="modal-sheet modal-sheet-wide launcher-account-sheet">
        <div className="modal-copy">
          <span className="section-kicker">Accounts</span>
          <h3 id="account-panel-title">Launcher アカウント</h3>
          <p>Launcher 保存済みと PC で見つかった候補をまとめて保持します。PC から検出したアカウントも、選ぶと Launcher の切替候補へ取り込みます。</p>
        </div>

        {scanning ? (
          <div className="launcher-account-scan-status" aria-live="polite">
            <div className="launcher-account-scan-copy">
              <strong>{scanTitle}</strong>
              <span>完了したらこの一覧をその場で更新します。</span>
            </div>
            <div
              className={`launcher-account-scan-progress ${scanProgress ? "is-determinate" : ""}`}
              role="progressbar"
              aria-label="Launcher アカウントの再検出進行状況"
              aria-valuetext={scanDetail}
              aria-valuenow={scanPercent ?? undefined}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              <span className="launcher-account-scan-progress-track" aria-hidden="true">
                <span
                  className="launcher-account-scan-progress-bar"
                  style={
                    scanPercent !== null
                      ? { width: `${Math.min(100, Math.max(10, scanPercent))}%` }
                      : undefined
                  }
                />
              </span>
              <span className="launcher-account-scan-progress-meta">
                <span className="launcher-account-scan-progress-note">{scanDetail}</span>
                {scanPercent !== null ? (
                  <span className="launcher-account-scan-progress-value">{scanPercent}%</span>
                ) : null}
              </span>
            </div>
          </div>
        ) : null}

        <div className={`launcher-account-overview ${scanning ? "is-scanning" : ""}`}>
          <article>
            <span>Detected</span>
            <strong>{accounts.length}</strong>
          </article>
          <article>
            <span>Launcher Saved</span>
            <strong>{launcherSavedCount}</strong>
          </article>
          <article>
            <span>Java Ready</span>
            <strong>{javaReadyCount}</strong>
          </article>
        </div>

        <div className={`launcher-account-list ${scanning ? "is-scanning" : ""}`}>
          {accounts.length > 0 ? (
            accounts.map((account) => {
              const switching = switchingLocalId === account.localId;
              const selected = account.isActive;
              const fromPcScan = account.authSource === "pc-scan";
              const identityLine = account.microsoftUsername
                ? account.microsoftUsername
                : "Microsoft アカウント名は未取得";
              const actionLabel = selected
                ? "選択中"
                : switching
                  ? "切替中..."
                  : fromPcScan
                    ? "取り込んで選択"
                    : "切替";
              const sourceLabel = fromPcScan ? "PC から検出" : "Launcher 保存済み";
              const ownershipLabel = account.hasJavaAccess ? "Java 利用可" : "Java 未確認";

              const rowContent = (
                <span className="launcher-account-row-copy">
                  <span className="launcher-account-row-head">
                    <strong className="launcher-account-name">
                      <span>{account.username}</span>
                      {account.hasJavaAccess ? (
                        <span className="launcher-account-inline-check is-owned" aria-hidden="true">
                          ✓
                        </span>
                      ) : null}
                    </strong>
                    <span className={`launcher-account-row-action ${selected ? "is-active" : fromPcScan ? "is-muted" : ""}`}>
                      {actionLabel}
                    </span>
                  </span>
                  <span>{identityLine}</span>
                  <span className="launcher-account-meta">
                    <span className={`launcher-account-tag ${fromPcScan ? "is-detected" : "is-launcher"}`}>
                      {sourceLabel}
                    </span>
                    <span className={`launcher-account-tag ${account.hasJavaAccess ? "is-owned" : "is-neutral"}`}>
                      {ownershipLabel}
                    </span>
                  </span>
                  {fromPcScan ? (
                    <span className="launcher-account-row-note">
                      このアカウントは PC の認証キャッシュから見つかりました。選ぶと Launcher の切替候補へ取り込みます。
                    </span>
                  ) : null}
                </span>
              );

              return (
                <button
                  key={account.localId}
                  type="button"
                  className={`launcher-account-row is-selectable ${selected ? "is-active" : ""} ${fromPcScan ? "is-detected-only" : ""} ${account.hasJavaAccess ? "is-java-owned" : "is-java-missing"}`}
                  onClick={() => onSelectAccount(account.localId)}
                  disabled={scanning || interactionDisabled || switching || selected}
                >
                  {rowContent}
                </button>
              );
            })
          ) : (
            <div className="launcher-account-empty">
              <strong>保持済みアカウントがありません</strong>
              <span>PC から再検出するか、公式 Launcher で一度ログインしてください。</span>
            </div>
          )}
        </div>

        <div className="modal-actions">
          <button
            type="button"
            className="secondary-button"
            onClick={onScanAccounts}
            disabled={scanning || interactionDisabled}
          >
            {scanning ? "検出中..." : "PC から再検出"}
          </button>
          <button type="button" className="secondary-button" onClick={onClose}>
            閉じる
          </button>
          <button
            type="button"
            className="secondary-button"
            onClick={onOpenOfficialLauncher}
            disabled={interactionDisabled}
          >
            公式 Launcher を開く
          </button>
        </div>
      </article>
    </div>
  );
}
