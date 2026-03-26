import type { InstalledMod, LauncherProfile, ModrinthProject } from "./types";

function normalizeMatchToken(value: string) {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "");
}

function sameProjectId(installedProjectId?: string | null, projectId?: string | null) {
  if (!installedProjectId || !projectId) {
    return false;
  }

  return (
    installedProjectId === projectId ||
    installedProjectId === `modrinth:${projectId}` ||
    projectId === `modrinth:${installedProjectId}`
  );
}

export function findInstalledProjectMod(
  profile: LauncherProfile | null,
  project: ModrinthProject,
) {
  if (!profile) {
    return null;
  }

  const slug = project.slug.toLowerCase();
  const normalizedTitle = normalizeMatchToken(project.title);

  return (
    profile.mods.find((mod) => {
      if (sameProjectId(mod.sourceProjectId, project.projectId)) {
        return true;
      }

      const fileName = mod.fileName.toLowerCase();
      const modId = mod.modId?.toLowerCase() ?? "";
      const normalizedName = normalizeMatchToken(mod.displayName);

      return fileName.includes(slug) || modId === slug || normalizedName === normalizedTitle;
    }) ?? null
  );
}

export function getProjectInstallState(
  profile: LauncherProfile | null,
  project: ModrinthProject,
) {
  const installedMod = findInstalledProjectMod(profile, project);

  if (!installedMod) {
    return {
      installedMod: null as InstalledMod | null,
      state: "install" as const,
    };
  }

  if (!sameProjectId(installedMod.sourceProjectId, project.projectId)) {
    return {
      installedMod,
      state: "blocked" as const,
    };
  }

  const installedVersion = installedMod.version?.trim();
  const latestVersion = project.latestVersion?.trim();
  const sameVersion = Boolean(
    installedVersion && latestVersion && installedVersion === latestVersion,
  );

  return {
    installedMod,
    state: sameVersion ? ("installed" as const) : ("update" as const),
  };
}

export function getDuplicateModGroups(mods: InstalledMod[]) {
  const groups = new Map<string, InstalledMod[]>();

  for (const mod of mods) {
    const key =
      mod.sourceProjectId?.trim() ||
      mod.modId?.trim() ||
      normalizeMatchToken(mod.displayName || mod.fileName);

    const bucket = groups.get(key) ?? [];
    bucket.push(mod);
    groups.set(key, bucket);
  }

  return Array.from(groups.values()).filter((group) => group.length > 1);
}
