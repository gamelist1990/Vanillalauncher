import type { LauncherSnapshot, NavigationItem, ViewMode } from "../app/types";

const NAV_COLORS: Record<string, string> = {
  play:     "#22c55e",
  mods:     "#60a5fa",
  discover: "#a78bfa",
  loaders:  "#fb923c",
  settings: "#8b949e",
};

type SidebarProps = {
  activeView: ViewMode;
  compact: boolean;
  items: NavigationItem[];
  open: boolean;
  snapshot: LauncherSnapshot | null;
  onClose: () => void;
  onSelectView: (view: ViewMode) => void;
};

export function Sidebar({
  activeView,
  compact,
  items,
  open,
  snapshot,
  onClose,
  onSelectView,
}: SidebarProps) {
  return (
    <aside
      id="launcher-sidebar"
      className={`sidebar ${compact ? "is-compact" : ""} ${open ? "is-open" : ""}`}
      aria-hidden={compact ? !open : undefined}
    >
      {/* Brand */}
      <div className="sidebar-header">
        <div className="brand-block">
          <div className="brand-mark" />
          <div>
            <strong>VanillaLauncher</strong>
            <span>Java Edition</span>
          </div>
        </div>
        {compact ? (
          <button
            type="button"
            className="sidebar-dismiss"
            onClick={onClose}
            aria-label="ナビゲーションを閉じる"
          >
            ✕
          </button>
        ) : null}
      </div>

      {/* Navigation */}
      <nav className="sidebar-nav">
        {items.map((item) => (
          <button
            key={item.id}
            className={`nav-item ${activeView === item.id ? "is-active" : ""}`}
            style={{ "--nav-accent": NAV_COLORS[item.id] ?? "#8b949e" } as React.CSSProperties}
            onClick={() => {
              onSelectView(item.id);
              if (compact) onClose();
            }}
          >
            <span className="nav-label">
              <strong>{item.label}</strong>
              <span className="nav-kicker">{item.kicker}</span>
            </span>
          </button>
        ))}
      </nav>

      {/* Stats */}
      <div className="sidebar-stats">
        <div className="sidebar-stat-row">
          <span className="sidebar-stat-label">プロファイル</span>
          <span className="sidebar-stat-value">{snapshot?.summary.profileCount ?? 0}</span>
        </div>
        <div className="sidebar-stat-row">
          <span className="sidebar-stat-label">Mod</span>
          <span className="sidebar-stat-value">{snapshot?.summary.modCount ?? 0}</span>
        </div>
        <div className="sidebar-stat-row">
          <span className="sidebar-stat-label">有効</span>
          <span className="sidebar-stat-value sidebar-stat-on">{snapshot?.summary.enabledModCount ?? 0}</span>
        </div>
        <div className="sidebar-stat-row">
          <span className="sidebar-stat-label">無効</span>
          <span className="sidebar-stat-value sidebar-stat-off">{snapshot?.summary.disabledModCount ?? 0}</span>
        </div>
      </div>

      {/* Footer: Minecraft Root */}
      <div className="sidebar-footer">
        <span className="sidebar-label">Minecraft Root</span>
        <code className="sidebar-path">
          {snapshot ? snapshot.minecraftRoot : "読み込み中..."}
        </code>
      </div>
    </aside>
  );
}