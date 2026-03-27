import type { LauncherProfile, LoaderId, ViewMode } from "./types";

export function viewTitle(view: ViewMode) {
  switch (view) {
    case "play":
      return "Play";
    case "mods":
      return "My Mods";
    case "discover":
      return "Discover";
    case "loaders":
      return "Loader";
    case "settings":
      return "Settings";
  }
}

export function formatLoader(loader: string) {
  switch (loader) {
    case "fabric":
      return "Fabric";
    case "forge":
      return "Forge";
    case "neoforge":
      return "NeoForge";
    case "quilt":
      return "Quilt";
    default:
      return "Vanilla";
  }
}

export function formatBytes(value: number) {
  if (value < 1024) {
    return `${value} B`;
  }

  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }

  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatDownloads(value: number) {
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1)}M`;
  }

  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(1)}K`;
  }

  return `${value}`;
}

export function formatTimestamp(timestamp?: number | null) {
  if (!timestamp) {
    return "更新時刻なし";
  }

  return new Intl.DateTimeFormat("ja-JP", {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(timestamp * 1000);
}

export function formatIsoDate(value?: string | null) {
  if (!value) {
    return "更新日不明";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("ja-JP", {
    year: "numeric",
    month: "numeric",
    day: "numeric",
  }).format(date);
}

export function formatDateTime(value: number) {
  return new Intl.DateTimeFormat("ja-JP", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(value);
}

export function formatClockTime(value: number) {
  return new Intl.DateTimeFormat("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(value);
}

export function formatDurationMs(value: number) {
  const totalSeconds = Math.max(0, Math.round(value / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return minutes > 0 ? `${hours}時間${minutes}分` : `${hours}時間`;
  }

  if (minutes > 0) {
    return seconds > 0 ? `${minutes}分${seconds}秒` : `${minutes}分`;
  }

  return `${seconds}秒`;
}

export function formatRelativeMs(value: number) {
  const totalSeconds = Math.max(0, Math.round(value / 1000));

  if (totalSeconds < 5) {
    return "たった今";
  }

  if (totalSeconds < 60) {
    return `${totalSeconds}秒前`;
  }

  const totalMinutes = Math.floor(totalSeconds / 60);
  if (totalMinutes < 60) {
    return `${totalMinutes}分前`;
  }

  const totalHours = Math.floor(totalMinutes / 60);
  if (totalHours < 24) {
    return `${totalHours}時間前`;
  }

  const totalDays = Math.floor(totalHours / 24);
  return `${totalDays}日前`;
}

export function formatLastUsed(value?: string | null) {
  if (!value) {
    return "未使用";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("ja-JP", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

export function formatVersionSupport(versions: string[]) {
  const uniqueVersions = Array.from(new Set(versions.filter(Boolean)));
  if (uniqueVersions.length === 0) {
    return "対応版未取得";
  }

  if (uniqueVersions.length <= 2) {
    return uniqueVersions.join(" / ");
  }

  return `${uniqueVersions[0]} + ${uniqueVersions.length - 1}`;
}

export function compactPath(path: string) {
  if (path.length <= 44) {
    return path;
  }

  return `${path.slice(0, 18)}...${path.slice(-20)}`;
}

export function formatRuntimeSupport(clientSide?: string | null, serverSide?: string | null) {
  const clientSupported = clientSide === "required" || clientSide === "optional";
  const serverSupported = serverSide === "required" || serverSide === "optional";

  if (clientSupported && serverSupported) {
    return "クライアント / サーバー";
  }

  if (clientSupported) {
    return "クライアント";
  }

  if (serverSupported) {
    return "サーバー";
  }

  return "対応情報なし";
}

export function errorMessage(error: unknown, fallback: string) {
  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object" && "message" in error) {
    const { message } = error as { message?: unknown };
    if (typeof message === "string") {
      return message;
    }
  }

  return fallback;
}

export function defaultFabricProfileName(
  profile: LauncherProfile | null,
  minecraftVersion: string | undefined,
) {
  return defaultLoaderProfileName("fabric", profile, minecraftVersion);
}

export function defaultLoaderProfileName(
  loader: LoaderId,
  profile: LauncherProfile | null,
  minecraftVersion: string | undefined,
) {
  const version = minecraftVersion?.trim();
  const loaderLabel = formatLoader(loader);
  const fallbackName = version ? `${loaderLabel} ${version}` : `${loaderLabel} Profile`;

  if (!profile) {
    return fallbackName;
  }

  if (profile.name === "最新リリース" || profile.name === "最新スナップショット") {
    return fallbackName;
  }

  return `${profile.name} / ${loaderLabel}`;
}

export function heroSubtitle(profile: LauncherProfile | null) {
  if (!profile) {
    return "公式 Minecraft Launcher の起動構成を読み込むと、ここから Play と Mod 管理をまとめて進められます。";
  }

  const version = profile.gameVersion ?? "バージョン未判定";
  const loader = formatLoader(profile.loader);
  const loaderVersion = profile.loaderVersion ? ` ${profile.loaderVersion}` : "";
  return `${loader}${loaderVersion} / ${version} の構成です。ゲームフォルダ、導入済み Mod、ローダー状態をこのアプリからまとめて確認できます。`;
}
