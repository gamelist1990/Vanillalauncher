import { useEffect } from "react";
import { ConfirmModal } from "../../components/ConfirmModal";
import { LocalModImportModal } from "../../components/LocalModImportModal";
import { ModpackExportModal } from "../../components/ModpackExportModal";
import { ModpackVersionModal } from "../../components/ModpackVersionModal";
import { ProfileNameModal } from "../../components/ProfileNameModal";
import { ProfileVisualModal } from "../../components/ProfileVisualModal";
import { ProgressDetailModal } from "../../components/ProgressDetailModal";
import type { ProgressSnapshot } from "../../app/types";
import type {
  ConfirmDialogState,
  ModpackExportDialogState,
  ModpackVersionDialogState,
  ProgressDetailDialogState,
  ProfileNameDialogState,
  ProfileVisualDialogState,
} from "./types";

type AppModalsProps = {
  confirmDialog: ConfirmDialogState | null;
  profileVisualDialog: ProfileVisualDialogState | null;
  profileNameDialog: ProfileNameDialogState | null;
  modpackVersionDialog: ModpackVersionDialogState | null;
  modpackExportDialog: ModpackExportDialogState | null;
  progressDetailDialog: ProgressDetailDialogState | null;
  progressDetailSnapshot: ProgressSnapshot | null;
  localModImportOpen: boolean;
  selectedProfileId: string | null;
  busyAction: string | null;
  onCloseConfirmDialog: () => void;
  onConfirmDialog: () => void;
  onCloseProfileVisualDialog: () => void;
  onConfirmProfileVisuals: () => void;
  onChangeProfileVisualName: (value: string) => void;
  onChangeProfileVisualIconUrl: (value: string) => void;
  onChangeProfileVisualBackgroundImageUrl: (value: string) => void;
  onCloseProfileNameDialog: () => void;
  onConfirmProfileName: () => void;
  onChangeProfileName: (value: string) => void;
  onCloseModpackVersionDialog: () => void;
  onConfirmModpackVersionInstall: () => void;
  onSelectModpackVersionId: (value: string) => void;
  onCloseModpackExportDialog: () => void;
  onConfirmModpackExport: () => void;
  onSelectModpackExportFormat: (value: "curseforge" | "modrinth") => void;
  onCloseProgressDetailDialog: () => void;
  onCloseLocalModImport: () => void;
  onLocalModImported: (message: string) => void;
  onLocalModError: (message: string) => void;
};

export function AppModals({
  confirmDialog,
  profileVisualDialog,
  profileNameDialog,
  modpackVersionDialog,
  modpackExportDialog,
  progressDetailDialog,
  progressDetailSnapshot,
  localModImportOpen,
  selectedProfileId,
  busyAction,
  onCloseConfirmDialog,
  onConfirmDialog,
  onCloseProfileVisualDialog,
  onConfirmProfileVisuals,
  onChangeProfileVisualName,
  onChangeProfileVisualIconUrl,
  onChangeProfileVisualBackgroundImageUrl,
  onCloseProfileNameDialog,
  onConfirmProfileName,
  onChangeProfileName,
  onCloseModpackVersionDialog,
  onConfirmModpackVersionInstall,
  onSelectModpackVersionId,
  onCloseModpackExportDialog,
  onConfirmModpackExport,
  onSelectModpackExportFormat,
  onCloseProgressDetailDialog,
  onCloseLocalModImport,
  onLocalModImported,
  onLocalModError,
}: AppModalsProps) {
  // いずれかのモーダルが開いている場合は背面スクロールをロック
  const anyModalOpen =
    confirmDialog !== null ||
    profileVisualDialog !== null ||
    profileNameDialog !== null ||
    modpackVersionDialog !== null ||
    modpackExportDialog !== null ||
    (progressDetailDialog !== null && progressDetailSnapshot !== null) ||
    localModImportOpen;

  useEffect(() => {
    if (!anyModalOpen) {
      return undefined;
    }
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = prev;
    };
  }, [anyModalOpen]);

  return (
    <>
      <ConfirmModal
        open={confirmDialog !== null}
        title={confirmDialog?.title ?? ""}
        description={confirmDialog?.description ?? ""}
        confirmLabel={confirmDialog?.confirmLabel ?? "実行"}
        tone={confirmDialog?.tone}
        onClose={onCloseConfirmDialog}
        onConfirm={onConfirmDialog}
      />

      <ProfileVisualModal
        open={profileVisualDialog !== null}
        profileName={profileVisualDialog?.profileName ?? "外観編集"}
        draftName={profileVisualDialog?.draftName ?? ""}
        iconUrl={profileVisualDialog?.iconUrl ?? ""}
        backgroundImageUrl={profileVisualDialog?.backgroundImageUrl ?? ""}
        onClose={onCloseProfileVisualDialog}
        onConfirm={onConfirmProfileVisuals}
        onChangeName={onChangeProfileVisualName}
        onChangeIconUrl={onChangeProfileVisualIconUrl}
        onChangeBackgroundImageUrl={onChangeProfileVisualBackgroundImageUrl}
      />

      <ProfileNameModal
        open={profileNameDialog !== null}
        profileName={profileNameDialog?.draftName ?? ""}
        onClose={onCloseProfileNameDialog}
        onConfirm={onConfirmProfileName}
        onChangeProfileName={onChangeProfileName}
      />

      <ModpackVersionModal
        open={modpackVersionDialog !== null}
        projectTitle={modpackVersionDialog?.project.title ?? "Modpack"}
        versions={modpackVersionDialog?.versions ?? []}
        selectedVersionId={modpackVersionDialog?.selectedVersionId ?? ""}
        loading={
          modpackVersionDialog !== null &&
          busyAction === `modpack:${modpackVersionDialog.project.projectId}`
        }
        onClose={onCloseModpackVersionDialog}
        onConfirm={onConfirmModpackVersionInstall}
        onSelectVersionId={onSelectModpackVersionId}
      />

      <ModpackExportModal
        open={modpackExportDialog !== null}
        profileName={modpackExportDialog?.profileName ?? "Modpack"}
        selectedFormat={modpackExportDialog?.selectedFormat ?? "curseforge"}
        loading={
          modpackExportDialog !== null &&
          busyAction === `modpack-export:${modpackExportDialog.profileId}`
        }
        onClose={onCloseModpackExportDialog}
        onConfirm={onConfirmModpackExport}
        onSelectFormat={onSelectModpackExportFormat}
      />

      <ProgressDetailModal
        open={progressDetailDialog !== null && progressDetailSnapshot !== null}
        progress={progressDetailSnapshot}
        onClose={onCloseProgressDetailDialog}
      />

      <LocalModImportModal
        openModal={localModImportOpen}
        profileId={selectedProfileId}
        busy={busyAction === "import-local-mod"}
        onClose={onCloseLocalModImport}
        onImported={onLocalModImported}
        onError={onLocalModError}
      />
    </>
  );
}
