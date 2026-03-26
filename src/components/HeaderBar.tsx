import { useState } from "react";
import { viewTitle } from "../app/formatters";
import type { LauncherProfile, ViewMode } from "../app/types";
import { DropdownSelect } from "./DropdownSelect";

type HeaderBarProps = {
  activeView: ViewMode;
  compactNavigation: boolean;
  profiles: LauncherProfile[];
  selectedProfileId: string;
  sidebarOpen: boolean;
  onRefresh: () => void;
  onSelectProfile: (profileId: string) => void;
  onToggleSidebar: () => void;
};

export function HeaderBar({
  activeView,
  compactNavigation,
  profiles,
  selectedProfileId,
  sidebarOpen,
  onRefresh,
  onSelectProfile,
  onToggleSidebar,
}: HeaderBarProps) {
  const [isProfileListOpen, setIsProfileListOpen] = useState(false);
  const profileOptions = profiles.map((profile) => ({
    value: profile.id,
    label: profile.name,
  }));

  return (
    <header className="header-bar">
      <div className="header-main">
        {compactNavigation ? (
          <button
            type="button"
            className={`menu-toggle ${sidebarOpen ? "is-open" : ""}`}
            onClick={onToggleSidebar}
            aria-label={sidebarOpen ? "ナビゲーションを閉じる" : "ナビゲーションを開く"}
            aria-controls="launcher-sidebar"
            aria-expanded={sidebarOpen}
          >
            <span className="menu-toggle-box" aria-hidden="true">
              <span />
              <span />
              <span />
            </span>
          </button>
        ) : null}

        <div className="header-copy">
          <p className="eyebrow">Minecraft Java Edition</p>
          <h1>{viewTitle(activeView)}</h1>
        </div>
      </div>

      <div className="header-actions">
        <label className="header-select">
          <span>選択中の起動構成</span>
          <DropdownSelect
            value={selectedProfileId}
            options={profileOptions}
            open={isProfileListOpen}
            disabled={profiles.length === 0}
            emptyLabel="起動構成がありません"
            menuLabel="起動構成一覧"
            onOpenChange={setIsProfileListOpen}
            onChange={onSelectProfile}
          />
        </label>

        <button className="secondary-button" onClick={onRefresh}>
          再読み込み
        </button>
      </div>
    </header>
  );
}
