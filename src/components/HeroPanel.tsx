import { formatLastUsed, formatLoader, heroSubtitle } from "../app/formatters";
import type { ActiveLauncherAccount, LauncherProfile } from "../app/types";

type HeroPanelProps = {
  profile: LauncherProfile | null;
  activeAccount?: ActiveLauncherAccount | null;
  launcherAvailable: boolean;
  busy: boolean;
  openingLauncher: boolean;
  onLaunch: () => void;
  onOpenOfficialLauncher: () => void;
  onOpenGameDir: () => void;
  onOpenModsDir: () => void;
  onEditProfileName: () => void;
  onDeleteProfile: () => void;
};

export function HeroPanel({
  profile,
  activeAccount,
  launcherAvailable,
  busy,
  openingLauncher,
  onLaunch,
  onOpenOfficialLauncher,
  onOpenGameDir,
  onOpenModsDir,
  onEditProfileName,
  onDeleteProfile,
}: HeroPanelProps) {
  const backgroundImage = encodeURI(profile?.backgroundImageUrl ?? "/launcher-hero.jpg");

  return (
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
          <strong className="path-text">
            {activeAccount?.username
              ? `${activeAccount.username} (公式Launcher連携)`
              : "未検出（先に公式Launcherでログイン）"}
          </strong>
        </article>
      </div>
    </section>
  );
}
