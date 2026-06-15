import { useState } from "react";
import { viewTitle } from "../app/formatters";
import type { LauncherProfile, ViewMode } from "../app/types";
import { DropdownSelect } from "./DropdownSelect";

// ビュー別のサブタイトル
const VIEW_SUBTITLES: Partial<Record<ViewMode, string>> = {
  play:     "プロファイルを選択して起動",
  mods:     "Modの有効・無効・管理",
  discover: "Modrinthから新しいModを探す",
  loaders:  "Fabric / Forge / NeoForge / Quilt",
  settings: "ランチャーの設定",
};

// ビュー別のアクセントカラー
const VIEW_ACCENT: Partial<Record<ViewMode, string>> = {
  play:     "#22c55e",
  mods:     "#60a5fa",
  discover: "#a78bfa",
  loaders:  "#fb923c",
  settings: "#8b949e",
};

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

  const accent = VIEW_ACCENT[activeView] ?? "#22c55e";
  const subtitle = VIEW_SUBTITLES[activeView];

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
          <div
            className="header-accent-bar"
            style={{ background: accent }}
            aria-hidden="true"
          />
          <div className="header-title-group">
            <h1 className="header-title">{viewTitle(activeView)}</h1>
            {subtitle ? (
              <p className="header-subtitle">{subtitle}</p>
            ) : null}
          </div>
        </div>
      </div>

      <div className="header-actions">
        {/* プロファイルセレクター */}
        {profiles.length > 0 ? (
          <div className="header-profile-selector">
            <span className="header-profile-label">起動構成</span>
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
          </div>
        ) : null}

        {/* リフレッシュボタン */}
        <button
          type="button"
          className="header-icon-btn"
          onClick={onRefresh}
          title="再読み込み"
          aria-label="データを再読み込み"
        >
          ↻
        </button>
      </div>
    </header>
  );
}
