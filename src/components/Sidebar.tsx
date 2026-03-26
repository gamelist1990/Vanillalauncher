import type { LauncherSnapshot, NavigationItem, ViewMode } from "../app/types";

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
      <div className="sidebar-header">
        <div className="brand-block">
          <div className="brand-mark" />
          <div>
            <strong>VanillaLauncher</strong>
            <span>Java Mod 管理</span>
          </div>
        </div>

        {compact ? (
          <button
            type="button"
            className="sidebar-dismiss"
            onClick={onClose}
            aria-label="ナビゲーションを閉じる"
          >
            X
          </button>
        ) : null}
      </div>

      <nav className="sidebar-nav">
        {items.map((item) => (
          <button
            key={item.id}
            className={`nav-item ${activeView === item.id ? "is-active" : ""}`}
            onClick={() => {
              onSelectView(item.id);

              if (compact) {
                onClose();
              }
            }}
          >
            <span>{item.kicker}</span>
            <strong>{item.label}</strong>
          </button>
        ))}
      </nav>

      <section className="sidebar-panel">
        <span className="sidebar-label">Minecraft Root</span>
        <code>{snapshot ? snapshot.minecraftRoot : "読み込み中..."}</code>
      </section>

      <section className="sidebar-panel sidebar-panel-grid">
        <article>
          <span>Profiles</span>
          <strong>{snapshot?.summary.profileCount ?? 0}</strong>
        </article>
        <article>
          <span>Mods</span>
          <strong>{snapshot?.summary.modCount ?? 0}</strong>
        </article>
        <article>
          <span>Enabled</span>
          <strong>{snapshot?.summary.enabledModCount ?? 0}</strong>
        </article>
        <article>
          <span>Disabled</span>
          <strong>{snapshot?.summary.disabledModCount ?? 0}</strong>
        </article>
      </section>
    </aside>
  );
}
