import { useEffect, useState } from "react";
import { launcherApi } from "../app/api";
import type { LauncherAccountEntry, Notice, ProgressState } from "../app/types";

const XBOX_AVATAR_CACHE_KEY = "launcher-account-xbox-avatars-v1";
const XBOX_AVATAR_CACHE_TTL_MS = 1000 * 60 * 60 * 24 * 30;

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

function buildXboxAvatarUrl(account: LauncherAccountEntry) {
  const xuid = account.xuid?.trim();
  if (xuid) {
    return `https://avatar-ssl.xboxlive.com/users/xuid(${encodeURIComponent(xuid)})/avatarpic-l.png`;
  }

  const gamerTag = account.gamerTag?.trim();
  if (gamerTag) {
    return `https://avatar-ssl.xboxlive.com/users/gt(${encodeURIComponent(gamerTag)})/avatarpic-l.png`;
  }

  return null;
}

function accountHasResolvedXboxIdentity(account: LauncherAccountEntry) {
  return Boolean(account.xuid?.trim() || account.gamerTag?.trim());
}

function normalizeXboxAvatarIdentity(account: LauncherAccountEntry) {
  return {
    xuid: account.xuid?.trim() || undefined,
    gamerTag: account.gamerTag?.trim() || undefined,
  };
}

function buildLegacyXboxAvatarUrl(gamerTag?: string | null) {
  const trimmed = gamerTag?.trim();
  if (!trimmed) {
    return null;
  }
  return `https://avatar-ssl.xboxlive.com/users/gt(${encodeURIComponent(trimmed)})/avatarpic-l.png`;
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
        if (!entry.url || (!entry.xuid && !entry.gamerTag) || now - entry.cachedAt > XBOX_AVATAR_CACHE_TTL_MS) {
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
  if (!accountHasResolvedXboxIdentity(account)) {
    return null;
  }

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

function MaskText({ text, label }: { text: string; label: string }) {
  const [revealed, setRevealed] = useState(false);
  return (
    <button
      type="button"
      className={`acct-mgr-mask-btn ${revealed ? "is-revealed" : ""}`}
      onClick={(e) => { e.stopPropagation(); setRevealed((v) => !v); }}
      title={revealed ? "隠す" : `${label}を表示`}
      aria-label={revealed ? `${label}を隠す` : `${label}を表示`}
    >
      <span className="acct-mgr-mask-text">{text}</span>
      <span className="acct-mgr-mask-toggle">{revealed ? "隠す" : "表示"}</span>
    </button>
  );
}

function AccountRow({ account, selected, switching, sourceLabel, sourceClass, canSelect, canLogout, onSelect, onLogout }: AccountRowProps) {
  // Minecraft スキン顔アイコン（Java 確認済みは緑アバター、未確認はグレー）
  const avatarEmoji = account.hasJavaAccess ? "🟩" : "⬜";
  const initials = account.username.slice(0, 2).toUpperCase();
  const [avatarUrl, setAvatarUrl] = useState<string | null>(() => resolveCachedXboxAvatarUrl(account));
  const [avatarFailed, setAvatarFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setAvatarFailed(false);
    void loadXboxAvatarTempCacheOnce().then(() => {
      if (!cancelled) {
        setAvatarUrl(resolveCachedXboxAvatarUrl(account));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [account.localId, account.gamerTag, account.xuid]);

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
        <span className={`acct-mgr-avatar ${account.hasJavaAccess ? "is-owned" : ""}`} aria-hidden="true">
          {avatarUrl && !avatarFailed ? (
            <img
              className="acct-mgr-avatar-image"
              src={avatarUrl}
              alt=""
              loading="lazy"
              referrerPolicy="no-referrer"
              onError={() => setAvatarFailed(true)}
            />
          ) : (
            <>
              <span className="acct-mgr-avatar-emoji">{avatarEmoji}</span>
              <span className="acct-mgr-avatar-initials">{initials}</span>
            </>
          )}
        </span>
      </button>

      {/* メイン情報（クリック非依存） */}
      <span className="acct-mgr-row-body">
        <span className="acct-mgr-row-name">
          {account.username}
          {account.hasJavaAccess && (
            <span className="acct-mgr-java-badge" title="Java Edition 所有確認済み">✓ Java</span>
          )}
        </span>

        {/* Microsoft メール・ID は保護表示 */}
        {account.microsoftUsername ? (
          <span className="acct-mgr-row-private">
            <MaskText text={account.microsoftUsername} label="Microsoft アカウント" />
          </span>
        ) : (
          <span className="acct-mgr-row-sub">Microsoft アカウント未取得</span>
        )}

        {account.gamerTag ? (
          <span className="acct-mgr-row-private">
            <MaskText text={account.gamerTag} label="ゲーマータグ" />
          </span>
        ) : null}

        <span className="acct-mgr-row-tags">
          <span className={`acct-mgr-tag ${sourceClass}`}>
            {sourceLabel}
          </span>
        </span>
      </span>

      {/* 右側ステータス（切替ボタン兼用） */}
      <span className="acct-mgr-row-status">
        <span className="acct-mgr-row-status-actions">
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

type AccountManagerTab = "manage" | "login";

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
  accountNotices = [],
  offlineModeEnabled,
  offlineUsername,
  switchingLocalId,
  scanning = false,
  xboxLoggingIn = false,
  scanProgress = null,
  loginProgress = null,
  interactionDisabled = false,
  onClose,
  onDismissAccountNotice,
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

  const [activeTab, setActiveTab] = useState<AccountManagerTab>("manage");

  useEffect(() => {
    if (xboxLoggingIn) {
      setActiveTab("login");
    }
  }, [xboxLoggingIn]);

  if (!open) return null;

  const javaReadyCount = accounts.filter((a) => a.hasJavaAccess).length;
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

  const renderAccountRows = (
    sectionAccounts: LauncherAccountEntry[],
    emptyMessage: string,
    sourceLabel: string,
    sourceClass: string,
  ) => sectionAccounts.length === 0 ? (
    <div className="acct-mgr-section-empty">{emptyMessage}</div>
  ) : (
    sectionAccounts.map((account) => {
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

  return (
    <div className="modal-layer" role="dialog" aria-modal="true" aria-labelledby="acct-mgr-title">
      <button type="button" className="modal-backdrop" onClick={onClose} aria-label="閉じる" />

      <article className="modal-sheet modal-sheet-wide acct-mgr-sheet">

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

        {/* ===== アカウントモーダル内専用 Notify ===== */}
        {accountNotices.length > 0 && (
          <div className="acct-mgr-notices" aria-live="polite">
            {accountNotices.map((notice) => (
              <div key={notice.id} className={`acct-mgr-notice is-${notice.tone}`}>
                <span className="acct-mgr-notice-icon" aria-hidden="true">
                  {notice.tone === "success" ? "✓" : notice.tone === "error" ? "!" : "i"}
                </span>
                <span className="acct-mgr-notice-text">{notice.text}</span>
                {onDismissAccountNotice && (
                  <button
                    type="button"
                    className="acct-mgr-notice-close"
                    onClick={() => onDismissAccountNotice(notice.id)}
                    aria-label="通知を閉じる"
                  >
                    ✕
                  </button>
                )}
              </div>
            ))}
          </div>
        )}

        {/* ===== 完全タブ分離: 管理 / ログイン ===== */}
        <nav className="acct-mgr-tabs" role="tablist" aria-label="アカウント操作">
          <button
            type="button"
            role="tab"
            id="acct-tab-manage"
            aria-controls="acct-panel-manage"
            aria-selected={activeTab === "manage"}
            className={`acct-mgr-tab-btn ${activeTab === "manage" ? "is-active" : ""}`}
            onClick={() => setActiveTab("manage")}
          >
            <span className="acct-mgr-tab-icon" aria-hidden="true">🧾</span>
            <span>
              <strong>アカウント管理</strong>
              <small>{accounts.length} 件の候補を整理・切替</small>
            </span>
          </button>
          <button
            type="button"
            role="tab"
            id="acct-tab-login"
            aria-controls="acct-panel-login"
            aria-selected={activeTab === "login"}
            className={`acct-mgr-tab-btn ${activeTab === "login" ? "is-active" : ""}`}
            onClick={() => setActiveTab("login")}
          >
            <span className="acct-mgr-tab-icon" aria-hidden="true">🔐</span>
            <span>
              <strong>アカウントログイン</strong>
              <small>Microsoft サインイン専用</small>
            </span>
          </button>
        </nav>

        {activeTab === "manage" ? (
          <section
            id="acct-panel-manage"
            role="tabpanel"
            aria-labelledby="acct-tab-manage"
            className="acct-mgr-tab-panel"
          >
            {/* ===== アカウントマネージャー: ログイン済み / PC探索を分離 ===== */}
            <div className={`acct-mgr-list ${scanning ? "is-busy" : ""}`}>
              {accounts.length === 0 ? (
                <div className="acct-mgr-empty">
                  <span className="acct-mgr-empty-icon">🔍</span>
                  <strong>アカウントが見つかりません</strong>
                  <span>「アカウントログイン」タブから Microsoft アカウントを追加するか、このタブで PC から再検出してください。</span>
                </div>
              ) : (
                <>
                  <section className="acct-mgr-section">
                    <div className="acct-mgr-section-head">
                      <strong>Microsoft ログイン済み</strong>
                      <span>このランチャーで認証・Java 所有確認済み</span>
                    </div>
                    {renderAccountRows(microsoftAccounts, "Microsoft 経由でログインしたアカウントはまだありません。", "Microsoft ログイン", "tag-ms")}
                  </section>

                  <section className="acct-mgr-section">
                    <div className="acct-mgr-section-head">
                      <strong>PC から検出</strong>
                      <span>公式 Launcher / PC 内キャッシュから見つかった候補</span>
                    </div>
                    {renderAccountRows(pcScanAccounts, "PC 探索で見つかった追加候補はありません。", "PC から検出", "tag-pc")}
                  </section>

                  <section className="acct-mgr-section">
                    <div className="acct-mgr-section-head">
                      <strong>公式 Launcher 保存済み</strong>
                      <span>公式 Launcher 側に保存されているアカウント</span>
                    </div>
                    {renderAccountRows(launcherAccounts, "公式 Launcher 保存済みアカウントはありません。", "Launcher 保存済み", "tag-launcher")}
                  </section>
                </>
              )}
            </div>

            {/* ===== 管理アクション ===== */}
            <div className="acct-mgr-actions">
              <div className="acct-mgr-sub-actions">
                <button
                  type="button"
                  className="acct-mgr-btn-sub"
                  onClick={onScanAccounts}
                  disabled={busy}
                >
                  {scanning ? "🔍 検出中…" : "🔍 PC から再検出"}
                </button>
                <button
                  type="button"
                  className="acct-mgr-btn-sub"
                  onClick={onOpenOfficialLauncher}
                  disabled={interactionDisabled}
                >
                  🚀 公式 Launcher を開く
                </button>
              </div>

              <div className="acct-mgr-mode-row">
                <span className="acct-mgr-mode-label">起動モード</span>
                <div className="segmented acct-mgr-mode-seg" role="group" aria-label="起動モード">
                  <button
                    type="button"
                    className={!offlineModeEnabled ? "is-active" : ""}
                    onClick={() => onToggleOfflineMode(false)}
                    disabled={busy}
                  >
                    🌐 オンライン
                  </button>
                  <button
                    type="button"
                    className={offlineModeEnabled ? "is-active" : ""}
                    onClick={() => onToggleOfflineMode(true)}
                    disabled={busy}
                  >
                    ✈ オフライン
                  </button>
                </div>
              </div>

              {offlineModeEnabled && (
                <div className="acct-mgr-offline-field">
                  <label className="acct-mgr-offline-label" htmlFor="acct-mgr-offline-name">
                    オフラインユーザー名
                  </label>
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
          </section>
        ) : (
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
                <span className="acct-mgr-login-title">Microsoft アカウントでログイン</span>
                <span className="acct-mgr-login-desc">ここはログイン専用です。既存アカウントの切替・PC 再検出は「アカウント管理」タブに分離しました。</span>
              </div>
              <button
                type="button"
                className="acct-mgr-btn-ms"
                onClick={onXboxLogin}
                disabled={offlineModeEnabled || busy}
              >
                <MicrosoftIcon />
                {xboxLoggingIn ? (
                  <span>Xboxログイン中…</span>
                ) : (
                  <span>Microsoft でサインイン</span>
                )}
              </button>
              {offlineModeEnabled && (
                <p className="acct-mgr-login-hint">オフラインモード中は Microsoft ログインできません。管理タブでオンラインに切り替えてください。</p>
              )}
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
