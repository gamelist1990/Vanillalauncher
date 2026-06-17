import { useEffect, useState } from "react";
import { launcherApi } from "../app/api";
import type { LauncherAccountEntry, Notice, ProgressState } from "../app/types";

const XBOX_AVATAR_CACHE_KEY = "launcher-account-xbox-avatars-v1";
const XBOX_AVATAR_CACHE_TTL_MS = 1000 * 60 * 60 * 24 * 30;
const XBOX_AVATAR_FALLBACK_URL = "/default-avatar.svg";

const UNSAFE_XBOX_AVATAR_URL_PATTERNS = [
  "avatar-ssl.xboxlive.com/users/",
];

type CachedXboxAvatar = {
  url: string;
  cachedAt: number;
  gamerTag?: string;
  xuid?: string;
};

let xboxAvatarTempCache: Record<string, CachedXboxAvatar> | null = null;
let xboxAvatarTempCacheLoadPromise: Promise<void> | null = null;

function xboxAvatarCacheId(account: LauncherAccountEntry) {
  return account.xuid?.trim() || account.localId || account.gamerTag?.trim() || account.username;
}

function readOptionalString(source: object, keys: string[]) {
  const record = source as Record<string, unknown>;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return null;
}

function isUnsafeXboxAvatarUrl(url: string) {
  return UNSAFE_XBOX_AVATAR_URL_PATTERNS.some((pattern) => url.includes(pattern));
}

function withXboxAvatarDisplaySize(url: string) {
  if (!url.includes("images-eds.xboxlive.com/image")) {
    return url;
  }

  try {
    const sizedUrl = new URL(url);
    if (!sizedUrl.searchParams.has("format")) {
      sizedUrl.searchParams.set("format", "png");
    }
    if (!sizedUrl.searchParams.has("w")) {
      sizedUrl.searchParams.set("w", "208");
    }
    if (!sizedUrl.searchParams.has("h")) {
      sizedUrl.searchParams.set("h", "208");
    }
    return sizedUrl.toString();
  } catch {
    return url;
  }
}

function normalizeXboxAvatarUrl(rawUrl: string | null) {
  if (!rawUrl || isUnsafeXboxAvatarUrl(rawUrl)) {
    return null;
  }
  return withXboxAvatarDisplaySize(rawUrl);
}

function normalizeMinecraftProfileInput(value: string | null | undefined) {
  const input = value?.trim();
  if (!input) {
    return null;
  }

  const compactUuid = input.replace(/-/g, "");
  if (/^[0-9a-fA-F]{32}$/.test(compactUuid)) {
    return compactUuid.toLowerCase();
  }

  if (/^[A-Za-z0-9_]{3,16}$/.test(input)) {
    return input;
  }

  return null;
}

function buildMinecraftAvatarUrl(account: LauncherAccountEntry) {
  const profileInput =
    normalizeMinecraftProfileInput(account.username) ??
    normalizeMinecraftProfileInput(account.localId);

  if (!profileInput) {
    return null;
  }

  return `https://api.minecraftapi.net/v3/profile/${encodeURIComponent(profileInput)}/avatar?size=128&overlay=true`;
}

function buildXboxAvatarUrl(account: LauncherAccountEntry) {
  const explicitUrl = normalizeXboxAvatarUrl(readOptionalString(account, [
    "gameDisplayPicRaw",
    "GameDisplayPicRaw",
    "game_display_pic_raw",
    "displayPicRaw",
    "DisplayPicRaw",
    "display_pic_raw",
    "appDisplayPicRaw",
    "AppDisplayPicRaw",
    "app_display_pic_raw",
    "gameDisplayPictureResizeUri",
    "GameDisplayPictureResizeUri",
    "game_display_picture_resize_uri",
    "appDisplayPictureResizeUri",
    "AppDisplayPictureResizeUri",
    "app_display_picture_resize_uri",
    "profilePicture",
    "profilePictureUrl",
    "profile_picture",
    "profile_picture_url",
    "avatarUrl",
    "avatar_url",
  ]));

  return explicitUrl ?? buildMinecraftAvatarUrl(account);
}

function normalizeXboxAvatarIdentity(account: LauncherAccountEntry) {
  return {
    xuid: account.xuid?.trim() || undefined,
    gamerTag: account.gamerTag?.trim() || undefined,
  };
}

function loadXboxAvatarTempCacheOnce() {
  if (xboxAvatarTempCacheLoadPromise) {
    return xboxAvatarTempCacheLoadPromise;
  }

  xboxAvatarTempCacheLoadPromise = launcherApi
    .readTempCacheJson(XBOX_AVATAR_CACHE_KEY)
    .then((raw) => {
      xboxAvatarTempCache = raw ? (JSON.parse(raw) as Record<string, CachedXboxAvatar>) : {};
      const now = Date.now();
      for (const [key, entry] of Object.entries(xboxAvatarTempCache)) {
        if (!entry.url || isUnsafeXboxAvatarUrl(entry.url) || now - entry.cachedAt > XBOX_AVATAR_CACHE_TTL_MS) {
          delete xboxAvatarTempCache[key];
        }
      }
    })
    .catch(() => {
      xboxAvatarTempCache = {};
    });

  return xboxAvatarTempCacheLoadPromise;
}

function persistXboxAvatarTempCache() {
  if (xboxAvatarTempCache === null) {
    return;
  }
  void launcherApi.writeTempCacheJson(XBOX_AVATAR_CACHE_KEY, JSON.stringify(xboxAvatarTempCache));
}

function resolveCachedXboxAvatarUrl(account: LauncherAccountEntry) {
  const directUrl = buildXboxAvatarUrl(account);
  if (!directUrl) {
    return null;
  }

  if (xboxAvatarTempCache === null) {
    void loadXboxAvatarTempCacheOnce();
    return directUrl;
  }

  const cacheId = xboxAvatarCacheId(account);
  const cached = xboxAvatarTempCache[cacheId];
  const identity = normalizeXboxAvatarIdentity(account);
  const now = Date.now();
  if (
    cached &&
    cached.xuid === identity.xuid &&
    cached.gamerTag === identity.gamerTag &&
    now - cached.cachedAt <= XBOX_AVATAR_CACHE_TTL_MS
  ) {
    return cached.url;
  }

  xboxAvatarTempCache[cacheId] = {
    url: directUrl,
    cachedAt: now,
    ...identity,
  };
  persistXboxAvatarTempCache();
  return directUrl;
}

function invalidateCachedXboxAvatarUrl(account: LauncherAccountEntry, failedUrl: string | null) {
  if (xboxAvatarTempCache === null || !failedUrl) {
    return;
  }

  const cacheId = xboxAvatarCacheId(account);
  if (xboxAvatarTempCache[cacheId]?.url === failedUrl) {
    delete xboxAvatarTempCache[cacheId];
    persistXboxAvatarTempCache();
  }
}

// ─── AccountRow: 個人情報保護付きアカウント行 ───────────────────────────────
type AccountRowProps = {
  account: LauncherAccountEntry;
  selected: boolean;
  switching: boolean;
  sourceLabel: string;
  sourceClass: string;
  canSelect: boolean;
  canLogout: boolean;
  onSelect: () => void;
  onLogout?: () => void;
};

function AccountAvatar({ account, className = "acct-mgr-avatar" }: { account: LauncherAccountEntry | null; className?: string }) {
  const [avatarUrl, setAvatarUrl] = useState<string | null>(() => account ? resolveCachedXboxAvatarUrl(account) : null);
  const [avatarFailed, setAvatarFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setAvatarFailed(false);

    if (!account) {
      setAvatarUrl(null);
      return () => {
        cancelled = true;
      };
    }

    void loadXboxAvatarTempCacheOnce().then(() => {
      if (!cancelled) {
        setAvatarUrl(resolveCachedXboxAvatarUrl(account));
      }
    });

    return () => {
      cancelled = true;
    };
  }, [
    account?.localId,
    account?.gamerTag,
    account?.xuid,
    account?.avatarUrl,
    account?.profilePicture,
    account?.profilePictureUrl,
    account?.displayPicRaw,
    account?.appDisplayPicRaw,
    account?.gameDisplayPicRaw,
    account?.appDisplayPictureResizeUri,
    account?.gameDisplayPictureResizeUri,
  ]);

  const src = avatarUrl ?? XBOX_AVATAR_FALLBACK_URL;

  return (
    <span className={`${className} ${account?.hasJavaAccess ? "is-owned" : ""}`} aria-hidden="true">
      {!avatarFailed ? (
        <img
          className="acct-mgr-avatar-image"
          src={src}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          onError={() => {
            if (account && avatarUrl) {
              invalidateCachedXboxAvatarUrl(account, avatarUrl);
              setAvatarUrl(null);
              return;
            }
            setAvatarFailed(true);
          }}
        />
      ) : (
        <span className="acct-mgr-avatar-fallback" aria-hidden="true">
          <span className="acct-mgr-avatar-fallback-head" />
          <span className="acct-mgr-avatar-fallback-body" />
        </span>
      )}
    </span>
  );
}

function AccountRow({ account, selected, switching, sourceLabel, sourceClass, canSelect, canLogout, onSelect, onLogout }: AccountRowProps) {
  return (
    <div
      className={[
        "acct-mgr-row",
        selected ? "is-active" : "",
        sourceClass === "tag-pc" ? "is-detected" : "",
        !canSelect ? "is-locked" : "",
      ].filter(Boolean).join(" ")}
    >
      {/* アバター + 行クリック領域（切替用） */}
      <button
        type="button"
        className="acct-mgr-row-select"
        onClick={onSelect}
        disabled={!canSelect}
        aria-pressed={selected}
        aria-label={`${account.username} を選択`}
        tabIndex={canSelect ? 0 : -1}
      >
        <AccountAvatar account={account} />
      </button>

      {/* ユーザー名 */}
      <span className="acct-mgr-row-name">{account.username}</span>

      {/* Java 所有バッジ */}
      {account.hasJavaAccess && (
        <span className="acct-mgr-java-badge" title="Java Edition 所有確認済み">✓ Java</span>
      )}

      {/* ソースタグ */}
      <span className={`acct-mgr-tag ${sourceClass}`}>{sourceLabel}</span>

      {/* アクションエリア */}
      <span className="acct-mgr-row-actions">
        {selected ? (
          <span className="acct-mgr-status-active">使用中</span>
        ) : switching ? (
          <span className="acct-mgr-status-switching">切替中…</span>
        ) : (
          <button
            type="button"
            className="acct-mgr-status-switch-btn"
            onClick={onSelect}
            disabled={!canSelect}
          >
            {sourceClass === "tag-pc" ? "取り込む" : "切替"}
          </button>
        )}
        {canLogout && onLogout && (
          <button
            type="button"
            className="acct-mgr-status-logout-btn"
            onClick={(event) => { event.stopPropagation(); onLogout(); }}
          >
            ログアウト
          </button>
        )}
      </span>
    </div>
  );
}

type LauncherAccountModalProps = {
  open: boolean;
  accounts: LauncherAccountEntry[];
  accountNotices?: Notice[];
  offlineModeEnabled: boolean;
  offlineUsername: string;
  switchingLocalId?: string | null;
  scanning?: boolean;
  xboxLoggingIn?: boolean;
  scanProgress?: ProgressState | null;
  loginProgress?: ProgressState | null;
  interactionDisabled?: boolean;
  onClose: () => void;
  onDismissAccountNotice?: (noticeId: string) => void;
  onSelectAccount: (localId: string) => void;
  onLogoutMicrosoftAccount: (localId: string) => void;
  onScanAccounts: () => void;
  onXboxLogin: () => void;
  onToggleOfflineMode: (enabled: boolean) => void;
  onChangeOfflineUsername: (username: string) => void;
  onOpenOfficialLauncher: () => void;
};

type AccountManagerTab = "accounts" | "login" | "settings";
type AccountFilterType = "all" | "ready" | "active" | "detected";

// Microsoft アイコン SVG
function MicrosoftIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 21 21" fill="none" aria-hidden="true">
      <rect x="1" y="1" width="9" height="9" fill="#f25022"/>
      <rect x="11" y="1" width="9" height="9" fill="#7fba00"/>
      <rect x="1" y="11" width="9" height="9" fill="#00a4ef"/>
      <rect x="11" y="11" width="9" height="9" fill="#ffb900"/>
    </svg>
  );
}

export function LauncherAccountModal({
  open,
  accounts,
  offlineModeEnabled,
  offlineUsername,
  switchingLocalId,
  scanning = false,
  xboxLoggingIn = false,
  scanProgress = null,
  loginProgress = null,
  interactionDisabled = false,
  onClose,
  onSelectAccount,
  onLogoutMicrosoftAccount,
  onScanAccounts,
  onXboxLogin,
  onToggleOfflineMode,
  onChangeOfflineUsername,
  onOpenOfficialLauncher,
}: LauncherAccountModalProps) {
  // スクロールロック
  useEffect(() => {
    if (!open) return undefined;
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      document.body.style.overflow = prev;
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [open, onClose]);

  const [activeTab, setActiveTab] = useState<AccountManagerTab>("accounts");
  const [accountQuery, setAccountQuery] = useState("");
  const [accountFilter, setAccountFilter] = useState<AccountFilterType>("all");

  useEffect(() => {
    if (xboxLoggingIn) {
      setActiveTab("login");
    }
  }, [xboxLoggingIn]);

  if (!open) return null;

  const javaReadyCount = accounts.filter((a) => a.hasJavaAccess).length;
  const detectedCount = accounts.filter((a) => a.authSource === "pc-scan").length;
  const selectedAccount = accounts.find((account) => account.isActive) ?? null;
  const scanPercent = scanProgress ? Math.round(scanProgress.percent) : null;
  const scanDetail = scanProgress?.detail ?? "Launcher 保存先と認証キャッシュを確認しています。";
  const loginPercent = loginProgress ? Math.round(loginProgress.percent) : null;
  const loginTitle = loginProgress?.title ?? "Microsoft ログイン";
  const loginDetail = loginProgress?.detail ?? "Microsoft アカウントのログイン処理を準備しています。";
  const busy = scanning || xboxLoggingIn || interactionDisabled;
  const microsoftAccounts = accounts.filter((account) => account.authSource === "microsoft-oauth");
  const pcScanAccounts = accounts.filter((account) => account.authSource === "pc-scan");
  const launcherAccounts = accounts.filter(
    (account) => account.authSource !== "microsoft-oauth" && account.authSource !== "pc-scan",
  );
  const normalizedQuery = accountQuery.trim().toLowerCase();
  const onlineModeLabel = offlineModeEnabled ? `オフライン: ${offlineUsername || "Player"}` : "オンライン起動";
  const activeSourceLabel = selectedAccount
    ? selectedAccount.authSource === "microsoft-oauth"
      ? "Microsoft"
      : selectedAccount.authSource === "pc-scan"
        ? "PC 検出"
        : "Launcher"
    : offlineModeEnabled
      ? "Offline"
      : "未選択";

  const filterAccounts = (sectionAccounts: LauncherAccountEntry[]) => sectionAccounts.filter((account) => {
    const matchesQuery = !normalizedQuery || [
      account.username,
      account.microsoftUsername ?? "",
      account.gamerTag ?? "",
      account.localId,
    ].some((value) => value.toLowerCase().includes(normalizedQuery));

    if (!matchesQuery) return false;
    if (accountFilter === "ready") return account.hasJavaAccess;
    if (accountFilter === "active") return account.isActive;
    if (accountFilter === "detected") return account.authSource === "pc-scan";
    return true;
  });

  const renderAccountRows = (
    sectionAccounts: LauncherAccountEntry[],
    emptyMessage: string,
    sourceLabel: string,
    sourceClass: string,
  ) => {
    const visibleAccounts = filterAccounts(sectionAccounts);

    return visibleAccounts.length === 0 ? (
      <div className="acct-mgr-section-empty">
        {sectionAccounts.length === 0 ? emptyMessage : "検索・フィルター条件に一致するアカウントはありません。"}
      </div>
    ) : (
      visibleAccounts.map((account) => {
      const selected = account.isActive;
      const switching = switchingLocalId === account.localId;
      const canSelect = !offlineModeEnabled && !busy && !switching && !selected;
      const canLogout = account.canLogout && !busy;

      return (
        <AccountRow
          key={account.localId}
          account={account}
          selected={selected}
          switching={switching}
          sourceLabel={sourceLabel}
          sourceClass={sourceClass}
          canSelect={canSelect}
          canLogout={canLogout}
          onSelect={() => canSelect && onSelectAccount(account.localId)}
          onLogout={() => onLogoutMicrosoftAccount(account.localId)}
        />
      );
      })
    );
  };

  return (
    <div className="modal-layer" role="dialog" aria-modal="true" aria-labelledby="acct-mgr-title">
      <button type="button" className="modal-backdrop" onClick={onClose} aria-label="閉じる" />

      <article className="modal-sheet modal-sheet-wide acct-mgr-sheet acct-mgr-pro-sheet">

        {/* ===== ヘッダー ===== */}
        <header className="acct-mgr-header">
          <div className="acct-mgr-header-left">
            <span className="acct-mgr-icon" aria-hidden="true">👤</span>
            <div>
              <h3 id="acct-mgr-title" className="acct-mgr-title">アカウント管理</h3>
              <p className="acct-mgr-subtitle">
                {javaReadyCount > 0
                  ? `${javaReadyCount} 件の Java 利用可能アカウント`
                  : "アカウントを追加してゲームを起動"}
              </p>
            </div>
          </div>
          <button type="button" className="acct-mgr-close" onClick={onClose} aria-label="閉じる">✕</button>
        </header>

        {/* ===== スキャン進行中バナー ===== */}
        {scanning && (
          <div className="acct-mgr-scan-banner" aria-live="polite">
            <div className="acct-mgr-scan-bar-wrap">
              <div
                className={`acct-mgr-scan-bar ${scanPercent !== null ? "is-det" : ""}`}
                style={scanPercent !== null ? { width: `${Math.min(100, Math.max(6, scanPercent))}%` } : undefined}
              />
            </div>
            <span className="acct-mgr-scan-label">
              {scanPercent !== null ? `${scanPercent}% — ` : ""}{scanDetail}
            </span>
          </div>
        )}

        {/* ===== 3タブナビ ===== */}
        <nav className="acct-mgr-tabs" role="tablist" aria-label="アカウント操作">
          <button
            type="button" role="tab"
            id="acct-tab-accounts"
            aria-controls="acct-panel-accounts"
            aria-selected={activeTab === "accounts"}
            className={`acct-mgr-tab-btn ${activeTab === "accounts" ? "is-active" : ""}`}
            onClick={() => setActiveTab("accounts")}
          >
            <span className="acct-mgr-tab-icon" aria-hidden="true">👤</span>
            <span><strong>アカウント</strong><small>{accounts.length} 件</small></span>
          </button>
          <button
            type="button" role="tab"
            id="acct-tab-login"
            aria-controls="acct-panel-login"
            aria-selected={activeTab === "login"}
            className={`acct-mgr-tab-btn ${activeTab === "login" ? "is-active" : ""}`}
            onClick={() => setActiveTab("login")}
          >
            <span className="acct-mgr-tab-icon" aria-hidden="true">＋</span>
            <span><strong>ログイン追加</strong><small>Microsoft サインイン</small></span>
          </button>
          <button
            type="button" role="tab"
            id="acct-tab-settings"
            aria-controls="acct-panel-settings"
            aria-selected={activeTab === "settings"}
            className={`acct-mgr-tab-btn ${activeTab === "settings" ? "is-active" : ""}`}
            onClick={() => setActiveTab("settings")}
          >
            <span className="acct-mgr-tab-icon" aria-hidden="true">⚙</span>
            <span><strong>設定</strong><small>起動モード</small></span>
          </button>
        </nav>

        {/* ===== アカウントタブ ===== */}
        {activeTab === "accounts" && (
          <section
            id="acct-panel-accounts"
            role="tabpanel"
            aria-labelledby="acct-tab-accounts"
            className="acct-mgr-tab-panel acct-mgr-accounts-panel"
          >
            <div className="acct-mgr-command-center">
              <div className="acct-mgr-current-card">
                <span className="acct-mgr-current-kicker">現在使用するアカウント</span>
                <div className="acct-mgr-current-main">
                  {offlineModeEnabled ? (
                    <span className="acct-mgr-current-avatar" aria-hidden="true">✈</span>
                  ) : (
                    <AccountAvatar account={selectedAccount} className="acct-mgr-current-avatar" />
                  )}
                  <div className="acct-mgr-current-copy">
                    <strong>{offlineModeEnabled ? offlineUsername || "Player" : selectedAccount?.username ?? "アカウント未選択"}</strong>
                    <span>
                      {offlineModeEnabled
                        ? "オフライン起動中 — Microsoft 認証は使いません"
                        : selectedAccount?.hasJavaAccess
                          ? "Java Edition 利用可能 / すぐ起動できます"
                          : selectedAccount
                            ? "Java ライセンス未確認 — 必要なら Microsoft ログインしてください"
                            : "ログイン追加または PC 再検出でアカウントを選択してください"}
                    </span>
                  </div>
                </div>
                <div className="acct-mgr-current-meta">
                  <span>{activeSourceLabel}</span>
                  <span>{offlineModeEnabled ? "Offline" : "Online"}</span>
                  <span>{javaReadyCount} Java OK</span>
                </div>
              </div>

            </div>

            {/* フィルターバー */}
            <div className="acct-mgr-filterbar">
              <input
                className="acct-mgr-search-input"
                value={accountQuery}
                onChange={(e) => setAccountQuery(e.target.value)}
                placeholder="名前 / メール / ゲーマータグで検索"
                aria-label="アカウント検索"
              />
              <div className="acct-mgr-filter-chips" role="group" aria-label="絞り込み">
                {([
                  ["all", "すべて"],
                  ["ready", "Java OK"],
                  ["active", "使用中"],
                  ["detected", "PC 検出"],
                ] as const).map(([value, label]) => (
                  <button
                    key={value}
                    type="button"
                    className={`acct-mgr-chip ${accountFilter === value ? "is-active" : ""}`}
                    onClick={() => setAccountFilter(value)}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>

            {/* アカウントリスト */}
            <div className={`acct-mgr-list acct-mgr-list-modern ${scanning ? "is-busy" : ""}`}>
              {accounts.length === 0 ? (
                <div className="acct-mgr-empty">
                  <span className="acct-mgr-empty-icon">👤</span>
                  <strong>アカウントが見つかりません</strong>
                  <span>「ログイン追加」タブから Microsoft アカウントを追加するか、設定タブで PC から再検出してください。</span>
                </div>
              ) : (
                <>
                  <section className="acct-mgr-section">
                    <div className="acct-mgr-section-head">
                      <strong>Microsoft ログイン済み</strong>
                      <span>{microsoftAccounts.length} 件</span>
                    </div>
                    {renderAccountRows(microsoftAccounts, "Microsoft 経由でログインしたアカウントはまだありません。", "Microsoft", "tag-ms")}
                  </section>
                  <section className="acct-mgr-section">
                    <div className="acct-mgr-section-head">
                      <strong>PC から検出</strong>
                      <span>{pcScanAccounts.length} 件</span>
                    </div>
                    {renderAccountRows(pcScanAccounts, "PC 探索で見つかった追加候補はありません。", "PC 検出", "tag-pc")}
                  </section>
                  <section className="acct-mgr-section">
                    <div className="acct-mgr-section-head">
                      <strong>公式 Launcher 保存済み</strong>
                      <span>{launcherAccounts.length} 件</span>
                    </div>
                    {renderAccountRows(launcherAccounts, "公式 Launcher 保存済みアカウントはありません。", "Launcher", "tag-launcher")}
                  </section>
                </>
              )}
            </div>
          </section>
        )}

        {/* ===== ログイン追加タブ ===== */}
        {activeTab === "login" && (
          <section
            id="acct-panel-login"
            role="tabpanel"
            aria-labelledby="acct-tab-login"
            className="acct-mgr-tab-panel acct-mgr-login-tab-panel"
          >
            <div className="acct-mgr-login-card">
              <div className="acct-mgr-login-card-icon" aria-hidden="true">
                <MicrosoftIcon />
              </div>
              <div className="acct-mgr-login-panel">
                <span className="acct-mgr-login-title">Microsoft アカウントでサインイン</span>
                <span className="acct-mgr-login-desc">ブラウザーが開きます。Microsoft アカウントでログインすると、Java 版の所有確認が完了します。</span>
              </div>
              <button
                type="button"
                className="acct-mgr-btn-ms"
                onClick={onXboxLogin}
                disabled={offlineModeEnabled || busy}
              >
                <MicrosoftIcon />
                {xboxLoggingIn ? <span>ログイン処理中…</span> : <span>Microsoft でサインイン</span>}
              </button>
              {offlineModeEnabled && (
                <p className="acct-mgr-login-hint">オフラインモード中は Microsoft ログインできません。設定タブでオンラインに切り替えてください。</p>
              )}
            </div>
          </section>
        )}

        {/* ===== 設定タブ ===== */}
        {activeTab === "settings" && (
          <section
            id="acct-panel-settings"
            role="tabpanel"
            aria-labelledby="acct-tab-settings"
            className="acct-mgr-tab-panel acct-mgr-settings-panel"
          >
            {/* 現在のアカウント概要 */}
            <div className="acct-settings-card">
              <span className="acct-settings-label">現在の起動アカウント</span>
              <div className="acct-settings-current">
                <strong>{selectedAccount?.username ?? "未選択"}</strong>
                <span className={`acct-settings-status ${selectedAccount?.hasJavaAccess ? "is-ok" : ""}`}>
                  {selectedAccount?.hasJavaAccess ? "✓ Java Edition 利用可能" : onlineModeLabel}
                </span>
              </div>
              <div className="acct-settings-stats">
                <span><b>{accounts.length}</b> 件登録</span>
                <span><b className="is-ok">{javaReadyCount}</b> Java OK</span>
                <span><b className="is-info">{detectedCount}</b> PC 検出</span>
              </div>
            </div>

            {/* 起動モード */}
            <div className="acct-settings-card">
              <span className="acct-settings-label">起動モード</span>
              <div className="acct-mgr-mode-seg" role="group" aria-label="起動モード">
                <button type="button" className={!offlineModeEnabled ? "is-active" : ""} onClick={() => onToggleOfflineMode(false)} disabled={busy}>
                  🌐 オンライン
                </button>
                <button type="button" className={offlineModeEnabled ? "is-active" : ""} onClick={() => onToggleOfflineMode(true)} disabled={busy}>
                  ✈ オフライン
                </button>
              </div>
              {offlineModeEnabled && (
                <div className="acct-mgr-offline-field">
                  <label className="acct-mgr-offline-label" htmlFor="acct-mgr-offline-name">オフラインユーザー名</label>
                  <input
                    id="acct-mgr-offline-name"
                    className="acct-mgr-offline-input"
                    value={offlineUsername}
                    onChange={(e) => onChangeOfflineUsername(e.target.value)}
                    placeholder="例: Player"
                    maxLength={16}
                    disabled={busy}
                  />
                </div>
              )}
            </div>

            {/* PC 再検出 */}
            <div className="acct-settings-card">
              <span className="acct-settings-label">PC アカウント検出</span>
              <p className="acct-settings-desc">公式 Minecraft Launcher のキャッシュを再スキャンして、アカウント候補を取り込みます。</p>
              <div className="acct-settings-actions">
                <button type="button" className="acct-mgr-btn-sub" onClick={onScanAccounts} disabled={busy}>
                  {scanning ? "🔍 検出中…" : "🔍 PC から再検出"}
                </button>
                <button type="button" className="acct-mgr-btn-sub" onClick={onOpenOfficialLauncher} disabled={interactionDisabled}>
                  🚀 公式 Launcher を開く
                </button>
              </div>
            </div>
          </section>
        )}

      </article>

      {/* ===== Microsoft / Xbox ログイン専用モーダル ===== */}
      {xboxLoggingIn && (
        <div className="acct-login-layer" role="dialog" aria-modal="true" aria-labelledby="acct-login-title">
          <article className="acct-login-dialog">
            <div className="acct-login-orb" aria-hidden="true">
              <MicrosoftIcon />
            </div>
            <p className="acct-login-kicker">Microsoft アカウント</p>
            <h4 id="acct-login-title" className="acct-login-title">{loginTitle}</h4>
            <p className="acct-login-detail">{loginDetail}</p>
            <div className="acct-login-progress" aria-label="ログイン進行状況">
              <div
                className={`acct-login-progress-bar ${loginPercent !== null ? "is-det" : ""}`}
                style={loginPercent !== null ? { width: `${Math.min(100, Math.max(5, loginPercent))}%` } : undefined}
              />
            </div>
            <div className="acct-login-meta">
              <span>{loginPercent !== null ? `${loginPercent}%` : "処理中"}</span>
              <span>ブラウザーでサインインを完了してください</span>
            </div>
          </article>
        </div>
      )}
    </div>
  );
}
