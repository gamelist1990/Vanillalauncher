type VersionKindFilter = "all" | "release" | "snapshot";

type MinecraftVersionSummary = {
  id: string;
  stable: boolean;
  kind: string;
};

type LoaderVersionSummary = {
  id: string;
  stable: boolean;
};

type VersionArtwork = {
  title: string;
  fileName: string;
  sourceUrl: string;
};

type VersionArtworkGroup = {
  artwork: VersionArtwork;
  versions: MinecraftVersionSummary[];
};

type VersionCatalogWorkerRequest = {
  requestId: number;
  gameVersions: MinecraftVersionSummary[];
  loaderVersions: LoaderVersionSummary[];
  query: string;
  kindFilter: VersionKindFilter;
  stableLoaderOnly: boolean;
};

type VersionCatalogWorkerResponse = {
  requestId: number;
  filteredGameVersions: MinecraftVersionSummary[];
  filteredLoaderVersions: LoaderVersionSummary[];
  groupedGameVersions: VersionArtworkGroup[];
};

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

const workerArtworkCache = new Map<string, VersionArtwork>();

self.onmessage = (event: MessageEvent<VersionCatalogWorkerRequest>) => {
  const {
    requestId,
    gameVersions,
    loaderVersions,
    query,
    kindFilter,
    stableLoaderOnly,
  } = event.data;
  const normalizedQuery = query.trim().toLowerCase();

  const filteredGameVersions = gameVersions.filter((entry) => {
    if (kindFilter !== "all" && entry.kind !== kindFilter) {
      return false;
    }

    if (normalizedQuery && !entry.id.toLowerCase().includes(normalizedQuery)) {
      return false;
    }

    return true;
  });

  const filteredLoaderVersions = loaderVersions.filter((entry) => {
    if (stableLoaderOnly && !entry.stable) {
      return false;
    }

    if (normalizedQuery && !entry.id.toLowerCase().includes(normalizedQuery)) {
      return false;
    }

    return true;
  });

  const groups = new Map<string, VersionArtworkGroup>();
  for (const entry of filteredGameVersions) {
    const artwork = resolveArtworkForVersionInWorker(entry.id);
    const key = `${artwork.title}:${artwork.fileName}`;
    const current = groups.get(key);

    if (current) {
      current.versions.push(entry);
    } else {
      groups.set(key, {
        artwork,
        versions: [entry],
      });
    }
  }

  const response: VersionCatalogWorkerResponse = {
    requestId,
    filteredGameVersions,
    filteredLoaderVersions,
    groupedGameVersions: Array.from(groups.values()),
  };

  self.postMessage(response);
};

function resolveArtworkForVersionInWorker(versionId: string): VersionArtwork {
  const normalizedVersionId = versionId.trim();

  if (!normalizedVersionId) {
    return FALLBACK_ARTWORK;
  }

  const cached = workerArtworkCache.get(normalizedVersionId);
  if (cached) {
    return cached;
  }

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

  workerArtworkCache.set(normalizedVersionId, artwork);
  return artwork;
}

export {};