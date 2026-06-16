import { useState } from "react";

export type AdvancedFilters = {
  sortBy: "relevance" | "downloads" | "follows" | "newest" | "updated";
  categories: string[];
  environment: "any" | "client" | "server" | "both";
};

type Props = {
  filters: AdvancedFilters;
  mode: "mods" | "modpacks";
  onApply: (filters: AdvancedFilters) => void;
  onClose: () => void;
};

const SORT_OPTIONS: { value: AdvancedFilters["sortBy"]; label: string; icon: string }[] = [
  { value: "relevance", label: "関連度", icon: "✦" },
  { value: "downloads", label: "DL数", icon: "⬇" },
  { value: "follows", label: "フォロワー", icon: "★" },
  { value: "newest", label: "新着", icon: "🆕" },
  { value: "updated", label: "更新日", icon: "🔄" },
];

const MOD_CATEGORIES = [
  "adventure", "cursed", "decoration", "economy", "equipment",
  "food", "game-mechanics", "library", "magic", "management",
  "minigame", "mobs", "optimization", "social", "storage",
  "technology", "transportation", "utility", "worldgen",
];

const MODPACK_CATEGORIES = [
  "adventure", "challenge", "combat", "exploration", "kitchen-sink",
  "lightweight", "magic", "multiplayer", "optimization", "quests",
  "sci-fi", "skyblock", "small", "tech", "vanilla-plus",
];

const ENV_OPTIONS: { value: AdvancedFilters["environment"]; label: string; icon: string }[] = [
  { value: "any",    label: "すべて",       icon: "🌐" },
  { value: "client", label: "クライアント",  icon: "💻" },
  { value: "server", label: "サーバー",      icon: "🖥" },
  { value: "both",   label: "両対応",        icon: "⚡" },
];

const DEFAULT_FILTERS: AdvancedFilters = {
  sortBy: "relevance",
  categories: [],
  environment: "any",
};

export function DiscoverAdvancedSearchModal({ filters, mode, onApply, onClose }: Props) {
  const [local, setLocal] = useState<AdvancedFilters>({ ...filters });
  const categories = mode === "mods" ? MOD_CATEGORIES : MODPACK_CATEGORIES;

  const toggleCategory = (cat: string) =>
    setLocal((prev) => ({
      ...prev,
      categories: prev.categories.includes(cat)
        ? prev.categories.filter((c) => c !== cat)
        : [...prev.categories, cat],
    }));

  const handleApply = () => {
    onApply(local);
    onClose();
  };

  const handleReset = () => setLocal({ ...DEFAULT_FILTERS });

  return (
    <div
      className="modal-layer"
      onClick={onClose}
      role="presentation"
    >
      <div className="modal-backdrop" aria-hidden="true" />
      <div
        className="modal-panel adv-search-panel"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="詳細検索"
      >
        {/* ヘッダー */}
        <div className="modal-header">
          <div className="modal-header-left">
            <span className="adv-search-header-icon">⚙</span>
            <h3 className="modal-title">詳細検索</h3>
          </div>
          <button className="modal-close" type="button" onClick={onClose} aria-label="閉じる">
            ✕
          </button>
        </div>

        {/* ボディ */}
        <div className="adv-search-body">

          {/* 並び順 */}
          <section className="adv-section">
            <h4 className="adv-section-title">並び順</h4>
            <div className="adv-option-grid">
              {SORT_OPTIONS.map((opt) => (
                <button
                  key={opt.value}
                  type="button"
                  className={`adv-option-btn ${local.sortBy === opt.value ? "is-active" : ""}`}
                  onClick={() => setLocal((p) => ({ ...p, sortBy: opt.value }))}
                >
                  <span className="adv-option-icon">{opt.icon}</span>
                  {opt.label}
                </button>
              ))}
            </div>
          </section>

          {/* 対応環境 (Modsのみ) */}
          {mode === "mods" && (
            <section className="adv-section">
              <h4 className="adv-section-title">対応環境</h4>
              <div className="adv-option-grid">
                {ENV_OPTIONS.map((opt) => (
                  <button
                    key={opt.value}
                    type="button"
                    className={`adv-option-btn ${local.environment === opt.value ? "is-active" : ""}`}
                    onClick={() => setLocal((p) => ({ ...p, environment: opt.value }))}
                  >
                    <span className="adv-option-icon">{opt.icon}</span>
                    {opt.label}
                  </button>
                ))}
              </div>
            </section>
          )}

          {/* カテゴリ */}
          <section className="adv-section">
            <h4 className="adv-section-title">
              カテゴリ
              {local.categories.length > 0 && (
                <span className="adv-section-badge">{local.categories.length}件</span>
              )}
            </h4>
            <div className="adv-category-wrap">
              {categories.map((cat) => (
                <button
                  key={cat}
                  type="button"
                  className={`adv-cat-chip ${
                    local.categories.includes(cat) ? "is-active" : ""
                  }`}
                  onClick={() => toggleCategory(cat)}
                >
                  {cat}
                </button>
              ))}
            </div>
          </section>
        </div>

        {/* フッター */}
        <div className="adv-search-footer">
          <button type="button" className="link-button" onClick={handleReset}>
            リセット
          </button>
          <div className="adv-footer-actions">
            <button type="button" className="adv-cancel-btn" onClick={onClose}>
              キャンセル
            </button>
            <button type="button" className="play-button" onClick={handleApply}>
              適用する
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
