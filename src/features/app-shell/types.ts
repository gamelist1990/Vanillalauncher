import type { ModpackVersionSummary, ModrinthProject } from "../../app/types";
import type { ModpackExportFormat } from "../../app/types";

export type ConfirmDialogState = {
  title: string;
  description: string;
  confirmLabel: string;
  tone?: "danger" | "accent";
  onConfirm: () => Promise<void> | void;
};

export type ProfileVisualDialogState = {
  profileId: string;
  profileName: string;
  iconUrl: string;
  backgroundImageUrl: string;
};

export type ProfileNameDialogState = {
  profileId: string;
  draftName: string;
};

export type ModpackVersionDialogState = {
  project: ModrinthProject;
  versions: ModpackVersionSummary[];
  selectedVersionId: string;
};

export type ModpackExportDialogState = {
  profileId: string;
  profileName: string;
  selectedFormat: ModpackExportFormat;
};
