import { formatLoader } from "../app/formatters";
import type { LauncherProfile } from "../app/types";

function resolveLoaderIconPath(loader: string) {
  switch (loader.toLowerCase()) {
    case "vanilla":
      return "/vanilla.png";
    case "fabric":
      return "/Fabric.png";
    case "forge":
      return "/Forge.png";
    case "neoforge":
      return "/NeoForge.png";
    case "quilt":
      return "/Quilt.png";
    default:
      return "/tauri.svg";
  }
}

type PlayViewProps = {
  profiles: LauncherProfile[];
  selectedProfileId: string;
  launching: boolean;
  busyAction: string | null;
  onLaunchProfile: (profileId: string) => void;
  onOpenProfileMods: (profileId: string) => void;
  onUpdateModpackProfile: (profileId: string) => void;
  onCustomizeProfileVisuals: (profileId: string) => void;
  onImportLocalModpack: () => void;
  onExportProfileModpack: (profileId: string) => void;
};

export function PlayView({
  profiles,
  selectedProfileId,
  launching,
  busyAction,
  onLaunchProfile,
  onOpenProfileMods,
  onUpdateModpackProfile,
  onCustomizeProfileVisuals,
  onImportLocalModpack,
  onExportProfileModpack,
}: PlayViewProps) {
  const selectedProfile = profiles.find((entry) => entry.id === selectedProfileId) ?? null;
  const importing = busyAction === "modpack-import";
  const exporting =
    selectedProfile !== null && busyAction === `modpack-export:${selectedProfile.id}`;

  return (
    <section className="workspace play-workspace">
      <article className="panel">
        <div className="panel-heading">
          <span>起動構成カード</span>
          <div className="header-actions" onClick={(event) => event.stopPropagation()}>
            <button className="secondary-button compact" onClick={onImportLocalModpack} disabled={importing}>
              {importing ? "取込中..." : "読み込み"}
            </button>
            <button
              className="secondary-button compact"
              onClick={() => {
                if (selectedProfile) {
                  onExportProfileModpack(selectedProfile.id);
                }
              }}
              disabled={!selectedProfile || exporting}
            >
              {exporting ? "書出中..." : "書き出し"}
            </button>
          </div>
        </div>

        <div className="play-profile-cards">
          {profiles.map((entry) => {
            const background = encodeURI(entry.backgroundImageUrl ?? "/launcher-hero.jpg");
            const icon = entry.customIconUrl ?? resolveLoaderIconPath(entry.loader);
            const selected = selectedProfileId === entry.id;

            return (
              <article
                key={entry.id}
                className={`play-profile-card ${selected ? "is-selected" : ""}`}
                onClick={() => onOpenProfileMods(entry.id)}
              >
                <div
                  className="play-profile-visual"
                  style={{
                    backgroundImage: `linear-gradient(180deg, rgba(15, 18, 24, 0.1), rgba(15, 18, 24, 0.64)), url("${background}")`,
                  }}
                >
                  <div className="play-profile-top">
                    <span className="badge">{entry.gameVersion ?? "未判定"}</span>
                    <span className="badge">{formatLoader(entry.loader)}</span>
                  </div>

                  <div className="play-profile-icon-wrap">
                    <img src={icon} alt="" className="play-profile-icon" />
                  </div>

                  <div className="play-profile-overlay-actions" onClick={(event) => event.stopPropagation()}>
                    <button
                      className="play-button compact"
                      onClick={() => onLaunchProfile(entry.id)}
                      disabled={launching}
                    >
                      {launching && selected ? "起動中..." : "プレイ"}
                    </button>
                    <button
                      className="secondary-button compact"
                      onClick={() => onCustomizeProfileVisuals(entry.id)}
                      disabled={launching}
                    >
                      外観
                    </button>
                    {entry.modpackProjectId ? (
                      <button
                        className="secondary-button compact"
                        onClick={() => onUpdateModpackProfile(entry.id)}
                        disabled={busyAction === `modpack-update:${entry.id}`}
                      >
                        {busyAction === `modpack-update:${entry.id}` ? "更新中..." : "パック更新"}
                      </button>
                    ) : null}
                  </div>
                </div>

                <div className="play-profile-copy">
                  <strong>{entry.name}</strong>
                  <span>{entry.modCount} Mods</span>
                </div>
              </article>
            );
          })}
        </div>
      </article>
    </section>
  );
}
