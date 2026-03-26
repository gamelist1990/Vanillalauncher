import { ConfirmModal } from "../../components/ConfirmModal";
import { ModpackExportModal } from "../../components/ModpackExportModal";
import { ModpackVersionModal } from "../../components/ModpackVersionModal";
import { ProfileNameModal } from "../../components/ProfileNameModal";
import { ProfileVisualModal } from "../../components/ProfileVisualModal";
import type {
  ConfirmDialogState,
  ModpackExportDialogState,
  ModpackVersionDialogState,
  ProfileNameDialogState,
  ProfileVisualDialogState,
} from "./types";

type AppModalsProps = {
  confirmDialog: ConfirmDialogState | null;
  profileVisualDialog: ProfileVisualDialogState | null;
  profileNameDialog: ProfileNameDialogState | null;
  modpackVersionDialog: ModpackVersionDialogState | null;
  modpackExportDialog: ModpackExportDialogState | null;
  busyAction: string | null;
  onCloseConfirmDialog: () => void;
  onConfirmDialog: () => void;
  onCloseProfileVisualDialog: () => void;
  onConfirmProfileVisuals: () => void;
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
};

export function AppModals({
  confirmDialog,
  profileVisualDialog,
  profileNameDialog,
  modpackVersionDialog,
  modpackExportDialog,
  busyAction,
  onCloseConfirmDialog,
  onConfirmDialog,
  onCloseProfileVisualDialog,
  onConfirmProfileVisuals,
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
}: AppModalsProps) {
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
        title={profileVisualDialog?.profileName ?? "外観編集"}
        description="カードの見た目を変えます。空欄にすると既定のアイコンや壁紙へ戻ります。"
        iconUrl={profileVisualDialog?.iconUrl ?? ""}
        backgroundImageUrl={profileVisualDialog?.backgroundImageUrl ?? ""}
        onClose={onCloseProfileVisualDialog}
        onConfirm={onConfirmProfileVisuals}
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
    </>
  );
}
