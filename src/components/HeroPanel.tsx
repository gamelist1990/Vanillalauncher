import { useState } from "react";
import { formatLastUsed, formatLoader, heroSubtitle } from "../app/formatters";
import type {
  ActiveLauncherAccount,
  LauncherAccountEntry,
  LauncherProfile,
  ProgressState,
} from "../app/types";
import { LauncherAccountModal } from "./LauncherAccountModal";

type HeroPanelProps = {
  profile: LauncherProfile | null;
  activeAccount?: ActiveLauncherAccount | null;
  launcherAccounts: LauncherAccountEntry[];
  launcherAvailable: boolean;
  busy: boolean;
  openingLauncher: boolean;
  switchingAccountLocalId?: string | null;
  scanningAccounts: boolean;
  scanProgress?: ProgressState | null;
  onLaunch: () => void;
  onOpenOfficialLauncher: () => void;
  onSelectLauncherAccount: (localId: string) => Promise<boolean>;
  onScanLauncherAccounts: () => void;
  onOpenGameDir: () => void;
  onOpenModsDir: () => void;
  onEditProfileName: () => void;
  onDeleteProfile: () => void;
};

export function HeroPanel({
  profile,
  activeAccount,
  launcherAccounts,
  launcherAvailable,
  busy,
  openingLauncher,
  switchingAccountLocalId,
  scanningAccounts,
  scanProgress,
  onLaunch,
  onOpenOfficialLauncher,
  onSelectLauncherAccount,
  onScanLauncherAccounts,
  onOpenGameDir,
  onOpenModsDir,
  onEditProfileName,
  onDeleteProfile,
}: HeroPanelProps) {
  const [accountPanelOpen, setAccountPanelOpen] = useState(false);
  const backgroundImage = encodeURI(profile?.backgroundImageUrl ?? "/launcher-hero.jpg");
  const accountSummary = activeAccount?.username
    ? activeAccount.username
    : "アカウント未検出";
  const accountStatus = activeAccount?.hasJavaAccess
    ? "Minecraft Java ライセンス確認済み"
    : activeAccount
      ? "Java 版ライセンス未確認"
      : "アカウントデータ未検出";
  const accountTone = activeAccount?.hasJavaAccess
    ? "Java 利用可"
    : activeAccount
      ? "Java 未確認"
      : "未検出";

  async function handleSelectAccount(localId: string) {
    const shouldClose = await onSelectLauncherAccount(localId);
    if (shouldClose) {
      setAccountPanelOpen(false);
    }
  }

  return (
    <>
      <section
        className="hero-panel"
        style={{
          backgroundImage:
            `linear-gradient(135deg, rgba(255, 255, 255, 0.96) 0%, rgba(255, 255, 255, 0.8) 48%, rgba(232, 246, 236, 0.82) 100%), url("${backgroundImage}")`,
        }}
      >
        <div className="hero-copy">
          <p className="eyebrow">選択中の起動構成</p>
          <h2
            className={profile ? "hero-title-editable" : undefined}
            onDoubleClick={() => {
              if (profile) {
                onEditProfileName();
              }
            }}
            title={profile ? "ダブルクリックで名前を編集" : undefined}
          >
            {profile?.name ?? "起動構成が選ばれていません"}
          </h2>
          <p className="hero-text">{heroSubtitle(profile)}</p>

          <div className="hero-tags">
            <span className={`badge badge-loader badge-${profile?.loader ?? "vanilla"}`}>
              {formatLoader(profile?.loader ?? "vanilla")}
            </span>
            <span className="badge">{profile?.gameVersion ?? "バージョン未判定"}</span>
            <span className="badge">最終起動 {formatLastUsed(profile?.lastUsed)}</span>
          </div>

          <div className="hero-actions">
            <button className="play-button" onClick={onLaunch} disabled={!profile || busy}>
              {busy ? "Minecraft を起動中..." : "Minecraft Java を起動"}
            </button>
            <button
              className="secondary-button"
              onClick={onOpenOfficialLauncher}
              disabled={!profile || !launcherAvailable || openingLauncher}
            >
              {openingLauncher ? "Launcher 起動中..." : "公式 Launcher"}
            </button>
            <button className="secondary-button" onClick={onOpenGameDir} disabled={!profile}>
              ゲームフォルダ
            </button>
            <button className="secondary-button" onClick={onOpenModsDir} disabled={!profile}>
              この構成の mods
            </button>
            <button className="danger-button" onClick={onDeleteProfile} disabled={!profile || busy}>
              起動構成を削除
            </button>
          </div>
        </div>

        <div className="hero-summary">
          <article>
            <span>Enabled</span>
            <strong>{profile?.enabledModCount ?? 0}</strong>
          </article>
          <article>
            <span>Disabled</span>
            <strong>{profile?.disabledModCount ?? 0}</strong>
          </article>
          <article>
            <span>Game Dir</span>
            <strong className="path-text">{profile ? profile.gameDir : "-"}</strong>
          </article>
          <article>
            <span>Last Version ID</span>
            <strong className="path-text">{profile?.lastVersionId ?? "未設定"}</strong>
          </article>
          <article>
            <span>Account</span>
            <button
              type="button"
              className="hero-account-trigger"
              onClick={() => setAccountPanelOpen(true)}
            >
              <span className="hero-account-copy">
                <span className="hero-account-head">
                  <strong className="hero-account-title">
                    <span>{accountSummary}</span>
                  </strong>
                  <span className={`hero-account-pill ${activeAccount?.hasJavaAccess ? "is-owned" : activeAccount ? "is-neutral" : ""}`}>
                    {accountTone}
                  </span>
                </span>
                <span className="hero-account-status">{accountStatus}</span>
              </span>
            </button>
          </article>
        </div>
      </section>

      <LauncherAccountModal
        open={accountPanelOpen}
        accounts={launcherAccounts}
        switchingLocalId={switchingAccountLocalId}
        scanning={scanningAccounts}
        scanProgress={scanProgress}
        onClose={() => setAccountPanelOpen(false)}
        onSelectAccount={(localId) => void handleSelectAccount(localId)}
        onScanAccounts={onScanLauncherAccounts}
        onOpenOfficialLauncher={onOpenOfficialLauncher}
      />
    </>
  );
}
