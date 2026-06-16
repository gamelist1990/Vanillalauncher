import { useEffect, useMemo, useRef, useState } from "react";
import { launcherApi } from "../app/api";
import type { LoaderVersionSummary, MinecraftVersionSummary } from "../app/types";

type VersionKindFilter = "all" | "release" | "snapshot";
export type VersionTab = "minecraft" | "loader";

type MinecraftVersionSelectModalProps = {
  open: boolean;
  loaderName: string;
  gameVersions: MinecraftVersionSummary[];
  loaderVersions: LoaderVersionSummary[];
  selectedGameVersion: string;
  selectedLoaderVersion: string;
  initialTab?: VersionTab;
  loading: boolean;
  onClose: () => void;
  onSelectGameVersion: (value: string) => void;
  onSelectLoaderVersion: (value: string) => void;
};

type VersionArtwork = {
  title: string;
  fileName: string;
  sourceUrl: string;
};

type CachedArtwork = VersionArtwork & {
  cachedAt: number;
};

type VersionArtworkGroup = {
  artwork: VersionArtwork;
  versions: MinecraftVersionSummary[];
};

type ActiveArtworkKey = string | null;

type VersionCatalogWorkerState = {
  filteredGameVersions: MinecraftVersionSummary[];
  filteredLoaderVersions: LoaderVersionSummary[];
  groupedGameVersions: VersionArtworkGroup[];
};

type VersionCatalogWorkerResponse = VersionCatalogWorkerState & {
  requestId: number;
};

const ARTWORK_CACHE_KEY = "minecraft-version-art-v1";
const ARTWORK_CACHE_MAX_AGE_MS = 1000 * 60 * 60 * 24 * 30;
const WIKI_FILE_BASE_URL = "https://minecraft.wiki/w/Special:Redirect/file/";

/** Runtime in-memory artwork cache – avoids repeated Temp cache reads per session */
const artworkRuntimeCache = new Map<string, VersionArtwork>();
/** In-memory parsed copy of Temp cache artwork data – avoids repeated JSON.parse */
let artworkTempCacheParsed: Record<string, CachedArtwork> | null = null;
let artworkTempCacheLoadPromise: Promise<void> | null = null;
/** URL build cache – avoids repeated encodeURIComponent/decodeURIComponent */
const wikiUrlCache = new Map<string, string>();

const FALLBACK_ARTWORK: VersionArtwork = {
  title: "Minecraft",
  fileName: "Minecraft_Key_Art_2024.jpg",
  sourceUrl: "https://minecraft.wiki/w/File:Minecraft_Key_Art_2024.jpg",
};

const UPDATE_ARTWORKS: Array<{
  title: string;
  fileName: string;
  sourceUrl: string;
  matches: RegExp[];
}> = [
  {
    title: "Chaos Cubed",
    fileName: "Chaos_Cubed_Key_Art.png",
    sourceUrl: "https://minecraft.wiki/w/File:Chaos_Cubed_Key_Art.png",
    matches: [/^26\.2(?:\b|-)/, /^26w/i],
  },
  {
    title: "Mounts of Mayhem",
    fileName: "Mounts_of_Mayhem_Key_Art.png",
    sourceUrl: "https://minecraft.wiki/w/File:Mounts_of_Mayhem_Key_Art.png",
    matches: [/^26\.1(?:\b|-)/],
  },
  {
    title: "The Copper Age",
    fileName: "The_Copper_Age_Key_Art.png",
    sourceUrl: "https://minecraft.wiki/w/File:The_Copper_Age_Key_Art.png",
    matches: [/^1\.21\.(?:9|10|11)(?:\b|-)/, /^25w(?:3[1-7]|4[1-6])/i],
  },
  {
    title: "Chase the Skies",
    fileName: "Chase_the_Skies_Key_Art.jpg",
    sourceUrl: "https://minecraft.wiki/w/File:Chase_the_Skies_Key_Art.jpg",
    matches: [/^1\.21\.(?:6|7|8)(?:\b|-)/, /^25w(?:1[5-9]|2[0-1])/i],
  },
  {
    title: "Spring to Life",
    fileName: "Spring_to_Life_Key_Art.jpg",
    sourceUrl: "https://minecraft.wiki/w/File:Spring_to_Life_Key_Art.jpg",
    matches: [/^1\.21\.5(?:\b|-)/, /^25w(?:0[2-9]|10|14)/i],
  },
  {
    title: "The Garden Awakens",
    fileName: "The_Garden_Awakens_Key_Art.png",
    sourceUrl: "https://minecraft.wiki/w/File:The_Garden_Awakens_Key_Art.png",
    matches: [/^1\.21\.4(?:\b|-)/, /^24w4[4-6]/i],
  },
  {
    title: "Bundles of Bravery",
    fileName: "Bundles_of_Bravery_Key_Art.png",
    sourceUrl: "https://minecraft.wiki/w/File:Bundles_of_Bravery_Key_Art.png",
    matches: [/^1\.21\.(?:2|3)(?:\b|-)/, /^24w3[3-9]/i, /^24w40/i],
  },
  {
    title: "Tricky Trials",
    fileName: "Tricky_Trials_Key_Art.png",
    sourceUrl: "https://minecraft.wiki/w/File:Tricky_Trials_Key_Art.png",
    matches: [/^1\.21(?:\.1)?(?:\b|-)/, /^24w(?:1[8-9]|2[0-1])/i],
  },
  {
    title: "Trails & Tales",
    fileName: "Trails_&_Tales_key_art.png",
    sourceUrl: "https://minecraft.wiki/w/File:Trails_%26_Tales_key_art.png",
    matches: [/^1\.20(?:\.[0-6])?(?:\b|-)/, /^23w/i, /^24w(?:0[3-9]|1[0-4])/i],
  },
  {
    title: "The Wild Update",
    fileName: "Wild_key_art.png",
    sourceUrl: "https://minecraft.wiki/w/File:Wild_key_art.png",
    matches: [/^1\.19(?:\.[0-4])?(?:\b|-)/, /^22w/i],
  },
  {
    title: "Caves & Cliffs",
    fileName: "Caves_&_Cliffs_Part_II.png",
    sourceUrl: "https://minecraft.wiki/w/File:Caves_%26_Cliffs_Part_II.png",
    matches: [/^1\.18(?:\.[0-2])?(?:\b|-)/, /^21w(?:3[7-9]|4[0-4])/i],
  },
  {
    title: "Caves & Cliffs Part I",
    fileName: "Caves_&_Cliffs_cover_art.png",
    sourceUrl: "https://minecraft.wiki/w/File:Caves_%26_Cliffs_cover_art.png",
    matches: [/^1\.17(?:\.1)?(?:\b|-)/, /^21w(?:0[3-9]|1[0-9]|20)/i],
  },
  {
    title: "Nether Update",
    fileName: "NetherUpdateArtwork.png",
    sourceUrl: "https://minecraft.wiki/w/File:NetherUpdateArtwork.png",
    matches: [/^1\.16(?:\.[0-5])?(?:\b|-)/, /^20w/i],
  },
  {
    title: "Buzzy Bees",
    fileName: "Buzzy_Bees.png",
    sourceUrl: "https://minecraft.wiki/w/File:Buzzy_Bees.png",
    matches: [/^1\.15(?:\.[0-2])?(?:\b|-)/, /^19w(?:3[4-9]|4[0-6])/i],
  },
  {
    title: "Village & Pillage",
    fileName: "Village_&_Pillage_banner.png",
    sourceUrl: "https://minecraft.wiki/w/File:Village_%26_Pillage_banner.png",
    matches: [/^1\.14(?:\.[0-4])?(?:\b|-)/, /^18w/i, /^19w(?:0[2-9]|1[0-4])/i],
  },
];

export function MinecraftVersionSelectModal({
  open,
  loaderName,
  gameVersions,
  loaderVersions,
  selectedGameVersion,
  selectedLoaderVersion,
  initialTab = "minecraft",
  loading,
  onClose,
  onSelectGameVersion,
  onSelectLoaderVersion,
}: MinecraftVersionSelectModalProps) {
  const [tab, setTab] = useState<VersionTab>("minecraft");
  const [query, setQuery] = useState("");
  const [kindFilter, setKindFilter] = useState<VersionKindFilter>("all");
  const [stableLoaderOnly, setStableLoaderOnly] = useState(false);
  const [imageErrors, setImageErrors] = useState<Record<string, boolean>>({});
  const [activeArtworkKey, setActiveArtworkKey] = useState<ActiveArtworkKey>(null);
  const [artworkCacheVersion, setArtworkCacheVersion] = useState(0);
  const [workerState, setWorkerState] = useState<VersionCatalogWorkerState>({
    filteredGameVersions: [],
    filteredLoaderVersions: [],
    groupedGameVersions: [],
  });
  const [workerPreparing, setWorkerPreparing] = useState(false);
  const workerRef = useRef<Worker | null>(null);
  const workerRequestIdRef = useRef(0);

  // クエリ文字列は毎レンダーで重複計算するのではなく一度だけ計算する
  const normalizedQuery = useMemo(() => query.trim().toLowerCase(), [query]);

  const selectedGame = useMemo(
    () => open ? gameVersions.find((entry) => entry.id === selectedGameVersion) ?? null : null,
    [gameVersions, open, selectedGameVersion],
  );
  const selectedLoader = useMemo(
    () => open ? loaderVersions.find((entry) => entry.id === selectedLoaderVersion) ?? null : null,
    [loaderVersions, open, selectedLoaderVersion],
  );
  const selectedArtwork = useMemo(
    () => open ? resolveArtworkForVersion(selectedGameVersion || selectedGame?.id || "") : FALLBACK_ARTWORK,
    [artworkCacheVersion, open, selectedGame?.id, selectedGameVersion],
  );

  const filteredGameVersions = workerState.filteredGameVersions;
  const filteredLoaderVersions = workerState.filteredLoaderVersions;
  const groupedGameVersions = workerState.groupedGameVersions;

  const activeGameVersionGroup = useMemo(() => {
    if (groupedGameVersions.length === 0) {
      return null;
    }

    const selectedGroup = groupedGameVersions.find((group) =>
      group.versions.some((entry) => entry.id === selectedGameVersion),
    );

    if (!activeArtworkKey) {
      return selectedGroup ?? groupedGameVersions[0];
    }

    return groupedGameVersions.find((group) => getArtworkKey(group.artwork) === activeArtworkKey)
      ?? selectedGroup
      ?? groupedGameVersions[0];
  }, [activeArtworkKey, groupedGameVersions, selectedGameVersion]);

  useEffect(() => {
    if (!open) {
      setWorkerPreparing(false);
      setWorkerState({
        filteredGameVersions: [],
        filteredLoaderVersions: [],
        groupedGameVersions: [],
      });
      return;
    }

    if (!workerRef.current) {
      workerRef.current = new Worker(
        new URL("../workers/versionCatalogWorker.ts", import.meta.url),
        { type: "module" },
      );
    }

    const requestId = workerRequestIdRef.current + 1;
    workerRequestIdRef.current = requestId;
    setWorkerPreparing(true);

    const worker = workerRef.current;
    const handleMessage = (event: MessageEvent<VersionCatalogWorkerResponse>) => {
      if (event.data.requestId !== workerRequestIdRef.current) {
        return;
      }

      setWorkerState({
        filteredGameVersions: event.data.filteredGameVersions,
        filteredLoaderVersions: event.data.filteredLoaderVersions,
        groupedGameVersions: event.data.groupedGameVersions,
      });
      setWorkerPreparing(false);
    };

    worker.addEventListener("message", handleMessage);
    worker.postMessage({
      requestId,
      gameVersions,
      loaderVersions,
      query,
      kindFilter,
      stableLoaderOnly,
    });

    return () => {
      worker.removeEventListener("message", handleMessage);
    };
  }, [gameVersions, kindFilter, loaderVersions, open, query, stableLoaderOnly]);

  useEffect(() => {
    return () => {
      workerRef.current?.terminate();
      workerRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (!open) {
      return undefined;
    }

    // モーダル表示中は背面スクロールをロック
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      document.body.style.overflow = prevOverflow;
    };
  }, [onClose, open]);

  useEffect(() => {
    if (!open) {
      return;
    }

    void loadArtworkTempCacheOnce().then(() => {
      setArtworkCacheVersion((current) => current + 1);
    });

    setTab(initialTab);
    setQuery("");
    setKindFilter("all");
    setStableLoaderOnly(false);
    setActiveArtworkKey(null);
  }, [initialTab, open]);

  if (!open) {
    return null;
  }

  const selectedGameKindLabel = selectedGame ? renderVersionKind(selectedGame) : "未選択";
  const selectedLoaderKindLabel = selectedLoader?.stable ? "安定版" : "通常版";

  return (
    <div className="modal-layer" role="dialog" aria-modal="true" aria-labelledby="minecraft-version-modal-title">
      <button
        type="button"
        className="modal-backdrop"
        onClick={onClose}
        aria-label="バージョン選択を閉じる"
      />

      <article className="modal-sheet modal-sheet-wide version-picker-sheet">
        <div className="version-picker-compact-header">
          <div className="version-picker-mini-art" aria-hidden="true">
            {!imageErrors[selectedArtwork.fileName] ? (
              <img
                src={getWikiFileImageUrl(selectedArtwork.fileName)}
                alt=""
                loading="lazy"
                onError={() => setImageErrors((current) => ({ ...current, [selectedArtwork.fileName]: true }))}
              />
            ) : (
              <span>Minecraft</span>
            )}
          </div>
          <div className="modal-copy version-picker-copy">
            <span className="section-kicker">Version Catalog</span>
            <h3 id="minecraft-version-modal-title">Minecraft / {loaderName} バージョンを選択</h3>
            <div className="version-picker-selected-row">
              <span>Minecraft: <strong>{selectedGameVersion || "未選択"}</strong> / {selectedGameKindLabel}</span>
              <span>{loaderName}: <strong>{selectedLoaderVersion || "未選択"}</strong> / {selectedLoaderKindLabel}</span>
            </div>
            <a className="version-picker-source" href={selectedArtwork.sourceUrl} target="_blank" rel="noreferrer">
              画像参照: {selectedArtwork.title}
            </a>
          </div>
        </div>

        <div className="version-picker-tabs" role="tablist" aria-label="選択対象">
          <button
            type="button"
            className={tab === "minecraft" ? "is-active" : ""}
            onClick={() => setTab("minecraft")}
          >
            Minecraft バージョン
          </button>
          <button
            type="button"
            className={tab === "loader" ? "is-active" : ""}
            onClick={() => setTab("loader")}
          >
            {loaderName} Loader
          </button>
        </div>

        <div className="version-picker-toolbar">
          <label className="version-picker-search">
            <span>検索</span>
            <input
              value={query}
              onChange={(event) => setQuery(event.currentTarget.value)}
              placeholder={tab === "minecraft" ? "例: 1.21 / 25w / rc" : "例: 0.19 / build"}
              autoFocus
            />
          </label>

          {tab === "minecraft" ? (
            <div className="version-picker-filter-group" aria-label="Minecraft バージョン種別">
              {(["all", "release", "snapshot"] as const).map((entry) => (
                <button
                  type="button"
                  key={entry}
                  className={kindFilter === entry ? "is-active" : ""}
                  onClick={() => setKindFilter(entry)}
                >
                  {entry === "all" ? "すべて" : entry === "release" ? "リリース" : "スナップショット"}
                </button>
              ))}
            </div>
          ) : (
            <label className="version-picker-check">
              <input
                type="checkbox"
                checked={stableLoaderOnly}
                onChange={(event) => setStableLoaderOnly(event.currentTarget.checked)}
              />
              安定版のみ
            </label>
          )}
        </div>

        {tab === "minecraft" ? (
          <div className="version-browser" role="listbox" aria-label="Minecraft バージョン一覧">
            <aside className="version-browser-rail" aria-label="アップデート一覧">
              <div className="version-browser-rail-title">
                <strong>アップデート</strong>
                <span>{filteredGameVersions.length} 件</span>
              </div>
              <div className="version-group-list">
                {groupedGameVersions.map((group) => {
                  const active = activeGameVersionGroup
                    ? getArtworkKey(activeGameVersionGroup.artwork) === getArtworkKey(group.artwork)
                    : false;
                  const selectedInGroup = group.versions.some((entry) => entry.id === selectedGameVersion);

                  return (
                    <button
                      type="button"
                      key={getArtworkKey(group.artwork)}
                      className={`version-group-button ${active ? "is-active" : ""} ${selectedInGroup ? "has-selected" : ""}`}
                      onClick={() => setActiveArtworkKey(getArtworkKey(group.artwork))}
                    >
                      <span className="version-group-thumb" aria-hidden="true">
                        {!imageErrors[group.artwork.fileName] ? (
                          <img
                            src={getWikiFileImageUrl(group.artwork.fileName)}
                            alt=""
                            loading="lazy"
                            onError={() => setImageErrors((current) => ({ ...current, [group.artwork.fileName]: true }))}
                          />
                        ) : (
                          <span>MC</span>
                        )}
                      </span>
                      <span className="version-group-copy">
                        <strong>
                          {(group.versions.find((v) => v.kind === "release") ??
                            group.versions[0]).id}
                        </strong>
                        <small>{group.artwork.title} · {group.versions.length} 件</small>
                      </span>
                    </button>
                  );
                })}
              </div>
            </aside>

            <section className="version-choice-panel">
              {workerPreparing ? (
                <div className="version-empty-state">バージョン一覧をバックグラウンドで準備しています...</div>
              ) : activeGameVersionGroup ? (
                <>
                  <div className="version-choice-header">
                    <div className="version-choice-header-banner" aria-hidden="true">
                      {!imageErrors[activeGameVersionGroup.artwork.fileName] ? (
                        <img
                          src={getWikiFileImageUrl(activeGameVersionGroup.artwork.fileName)}
                          alt=""
                          loading="lazy"
                          onError={() => setImageErrors((current) => ({ ...current, [activeGameVersionGroup.artwork.fileName]: true }))}
                        />
                      ) : null}
                    </div>
                    <div className="version-choice-header-info">
                      <div>
                        <span className="section-kicker">
                          {activeGameVersionGroup.artwork.title}
                        </span>
                        <h4>
                          {(activeGameVersionGroup.versions.find((v) => v.kind === "release") ??
                            activeGameVersionGroup.versions[0]).id}
                        </h4>
                        <p>{activeGameVersionGroup.versions.length} 件のバージョンから選択</p>
                      </div>
                      <a href={activeGameVersionGroup.artwork.sourceUrl} target="_blank" rel="noreferrer">
                        Key art
                      </a>
                    </div>
                  </div>

                  <div className="version-choice-list">
                    <div className="version-chip-grid">
                      {activeGameVersionGroup.versions.map((entry) => {
                        const selected = entry.id === selectedGameVersion;
                        const isRelease = entry.kind === "release";

                        return (
                          <button
                            type="button"
                            key={entry.id}
                            className={`version-chip ${selected ? "is-selected" : ""} ${isRelease ? "version-chip-stable" : ""}`}
                            onClick={() => onSelectGameVersion(entry.id)}
                            disabled={loading}
                            aria-pressed={selected}
                          >
                            <strong>{entry.id}</strong>
                            <small>{renderVersionKind(entry)}{selected ? " ✓ 選択中" : ""}</small>
                          </button>
                        );
                      })}
                    </div>
                  </div>
                </>
              ) : (
                <div className="version-empty-state">該当する Minecraft バージョンがありません。</div>
              )}
            </section>
          </div>
        ) : (
          <div className="loader-version-list" role="listbox" aria-label={`${loaderName} Loader バージョン一覧`}>
            <div className="version-chip-grid">
              {filteredLoaderVersions.map((entry) => {
                const selected = entry.id === selectedLoaderVersion;
                const isStable = entry.stable;

                return (
                  <button
                    type="button"
                    key={entry.id}
                    className={`version-chip ${selected ? "is-selected" : ""} ${isStable ? "version-chip-stable" : ""}`}
                    onClick={() => onSelectLoaderVersion(entry.id)}
                    disabled={loading}
                    aria-pressed={selected}
                  >
                    <strong>{entry.id}</strong>
                    <small>{isStable ? "✓ 安定版" : "通常版"}{selected ? " · 選択中" : ""}</small>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        <div className="modal-actions">
          <button type="button" className="secondary-button" onClick={onClose}>
            閉じる
          </button>
        </div>
      </article>
    </div>
  );
}

function renderVersionKind(version: MinecraftVersionSummary) {
  if (version.kind === "release") {
    return "リリース";
  }

  if (version.kind === "snapshot") {
    return "スナップショット";
  }

  return version.kind || "通常版";
}

function getWikiFileImageUrl(fileName: string): string {
  const cached = wikiUrlCache.get(fileName);
  if (cached) return cached;
  const url = `${WIKI_FILE_BASE_URL}${encodeURIComponent(decodeURIComponent(fileName))}`;
  wikiUrlCache.set(fileName, url);
  return url;
}

function getArtworkKey(artwork: VersionArtwork) {
  return `${artwork.title}:${artwork.fileName}`;
}

function resolveArtworkForVersion(versionId: string): VersionArtwork {
  const normalizedVersionId = versionId.trim();

  if (!normalizedVersionId) {
    return FALLBACK_ARTWORK;
  }

  // 1. インメモリキャッシュ（最速 – Temp キャッシュ読み取り不要）
  const memHit = artworkRuntimeCache.get(normalizedVersionId);
  if (memHit) return memHit;

  // 2. Temp キャッシュ（セッション間持続）
  const tempHit = readArtworkCache(normalizedVersionId);
  if (tempHit) {
    artworkRuntimeCache.set(normalizedVersionId, tempHit);
    return tempHit;
  }

  // 3. 正規表現マッチ（初回のみ）
  const matched = UPDATE_ARTWORKS.find((entry) =>
    entry.matches.some((pattern) => pattern.test(normalizedVersionId)),
  );
  const artwork = matched
    ? {
        title: matched.title,
        fileName: matched.fileName,
        sourceUrl: matched.sourceUrl,
      }
    : FALLBACK_ARTWORK;

  artworkRuntimeCache.set(normalizedVersionId, artwork);
  writeArtworkCache(normalizedVersionId, artwork);
  return artwork;
}

function readArtworkCache(versionId: string): VersionArtwork | null {
  try {
    if (artworkTempCacheParsed === null) {
      void loadArtworkTempCacheOnce();
      return null;
    }

    const entry = artworkTempCacheParsed[versionId];
    if (!entry) {
      return null;
    }

    if (Date.now() - entry.cachedAt > ARTWORK_CACHE_MAX_AGE_MS) {
      delete artworkTempCacheParsed[versionId];
      persistArtworkTempCache();
      return null;
    }

    return {
      title: entry.title,
      fileName: entry.fileName,
      sourceUrl: entry.sourceUrl,
    };
  } catch {
    artworkTempCacheParsed = {};
    return null;
  }
}

function writeArtworkCache(versionId: string, artwork: VersionArtwork) {
  try {
    if (artworkTempCacheParsed === null) {
      artworkTempCacheParsed = {};
    }
    artworkTempCacheParsed[versionId] = { ...artwork, cachedAt: Date.now() };
    persistArtworkTempCache();
  } catch {
    // キャッシュに失敗しても選択 UI は通常どおり動かす
  }
}

function loadArtworkTempCacheOnce() {
  if (artworkTempCacheLoadPromise) {
    return artworkTempCacheLoadPromise;
  }

  artworkTempCacheLoadPromise = launcherApi
    .readTempCacheJson(ARTWORK_CACHE_KEY)
    .then((raw) => {
      artworkTempCacheParsed = raw ? (JSON.parse(raw) as Record<string, CachedArtwork>) : {};
      for (const [versionId, artwork] of Object.entries(artworkTempCacheParsed)) {
        if (Date.now() - artwork.cachedAt <= ARTWORK_CACHE_MAX_AGE_MS) {
          artworkRuntimeCache.set(versionId, {
            title: artwork.title,
            fileName: artwork.fileName,
            sourceUrl: artwork.sourceUrl,
          });
        }
      }
    })
    .catch(() => {
      artworkTempCacheParsed = {};
    });

  return artworkTempCacheLoadPromise;
}

function persistArtworkTempCache() {
  if (artworkTempCacheParsed === null) {
    return;
  }

  void launcherApi.writeTempCacheJson(
    ARTWORK_CACHE_KEY,
    JSON.stringify(artworkTempCacheParsed),
  );
}
