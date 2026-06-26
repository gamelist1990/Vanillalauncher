import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { launcherApi } from "../app/api";
import type { LocalModAnalysis } from "../app/types";

type LocalModImportModalProps = {
  openModal: boolean;
  profileId?: string | null;
  busy: boolean;
  onClose: () => void;
  onImported: (message: string) => void;
  onError: (message: string) => void;
};

function modActionLabel(action: string) {
  switch (action) {
    case "install":
      return "新規導入";
    case "replace":
      return "更新置き換え";
    case "skip":
      return "導入済み";
    case "reject":
      return "却下";
    default:
      return action;
  }
}

export function LocalModImportModal({
  openModal,
  profileId,
  busy,
  onClose,
  onImported,
  onError,
}: LocalModImportModalProps) {
  const [analyses, setAnalyses] = useState<LocalModAnalysis[]>([]);
  const [analyzing, setAnalyzing] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const analysisCacheRef = useRef<Map<string, LocalModAnalysis>>(new Map());

  const installable = useMemo(
    () => analyses.filter((item) => item.compatible && ["install", "replace"].includes(item.action)),
    [analyses],
  );

  useEffect(() => {
    if (!openModal) return undefined;
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !analyzing && !installing) onClose();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [openModal, analyzing, installing, onClose]);

  // Tauri v2 ネイティブ drag & drop イベントで .jar ファイルのパスを取得
  useEffect(() => {
    if (!openModal || !profileId) return undefined;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "over") {
          setDragOver(true);
        } else if (event.payload.type === "leave") {
          setDragOver(false);
        } else if (event.payload.type === "drop") {
          setDragOver(false);
          const jarPaths = event.payload.paths.filter((p: string) => p.toLowerCase().endsWith(".jar"));
          if (jarPaths.length > 0) {
            void analyzePaths(jarPaths);
          }
        }
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => {
      unlisten?.();
    };
  }, [openModal, profileId]);

  useEffect(() => {
    if (!openModal) {
      setAnalyses([]);
      setAnalyzing(false);
      setInstalling(false);
      setDragOver(false);
      analysisCacheRef.current.clear();
    }
  }, [openModal]);

  async function analyzePaths(paths: string[]) {
    if (!profileId || paths.length === 0) return;
    setAnalyzing(true);
    try {
      const results: LocalModAnalysis[] = [];
      const uniquePaths = Array.from(new Set(paths.filter((path) => path.toLowerCase().endsWith(".jar"))));
      const pendingPaths: string[] = [];

      for (const path of uniquePaths) {
        const cacheKey = `${profileId}:${path}`;
        const cached = analysisCacheRef.current.get(cacheKey);
        if (cached) {
          results.push(cached);
        } else {
          pendingPaths.push(path);
        }
      }

      const queue = [...pendingPaths];
      const workerCount = Math.min(4, queue.length);
      await Promise.all(
        Array.from({ length: workerCount }, async () => {
          while (queue.length > 0) {
            const path = queue.shift();
            if (!path) return;
            try {
              const result = await launcherApi.analyzeLocalMod(profileId, path);
              analysisCacheRef.current.set(`${profileId}:${path}`, result);
              results.push(result);
            } catch (error) {
              onError(error instanceof Error ? error.message : String(error));
            }
          }
        }),
      );

      setAnalyses((current) => {
        const merged = [...current];
        for (const result of results) {
          const index = merged.findIndex((item) => item.filePath === result.filePath);
          if (index >= 0) merged[index] = result;
          else merged.push(result);
        }
        return merged;
      });
    } finally {
      setAnalyzing(false);
    }
  }

  async function chooseFiles() {
    const selected = await open({
      title: "追加する Mod Jar を選択",
      multiple: true,
      filters: [{ name: "Minecraft Mod Jar", extensions: ["jar"] }],
    });
    if (!selected) return;
    await analyzePaths(Array.isArray(selected) ? selected : [selected]);
  }

  async function importInstallable() {
    if (!profileId || installable.length === 0) return;
    setInstalling(true);
    let success = 0;
    let failed = 0;
    try {
      for (const item of installable) {
        try {
          const result = await launcherApi.importCheckedLocalMod(profileId, item.filePath);
          onImported(result.message);
          success += 1;
        } catch (error) {
          failed += 1;
          onError(error instanceof Error ? error.message : String(error));
        }
      }
      if (success > 0) {
        onImported(failed > 0 ? `${success} 件導入、${failed} 件は失敗しました。` : `${success} 件の Mod を導入しました。`);
        onClose();
      }
    } finally {
      setInstalling(false);
    }
  }

  if (!openModal) return null;

  return (
    <div className="modal-layer" role="dialog" aria-modal="true" aria-labelledby="local-mod-import-title">
      <button
        type="button"
        className="modal-backdrop"
        onClick={() => !analyzing && !installing && onClose()}
        aria-label="Mod 追加モーダルを閉じる"
      />

      <article className="modal-sheet modal-sheet-wide local-mod-import-sheet">
        <div className="modal-copy">
          <span className="section-kicker">Local Mod Import</span>
          <h3 id="local-mod-import-title">Jar を解析して安全に追加</h3>
          <p>
            .jar をここにドロップ、または選択してください。Jar 内の fabric.mod.json / quilt.mod.json /
            mods.toml / neoforge.mods.toml を読み取り、Loader・Minecraftバージョン・依存関係・既存Modとのバージョン差を確認します。
          </p>
        </div>

        <button
          type="button"
          className={`local-mod-dropzone ${dragOver ? "is-dragover" : ""}`}
          onClick={chooseFiles}
          onDragOver={(event) => {
            event.preventDefault();
            setDragOver(true);
          }}
          onDragLeave={() => setDragOver(false)}
          onDrop={(event) => {
            event.preventDefault();
            setDragOver(false);
            const paths = Array.from(event.dataTransfer.files)
              .map((file) => "path" in file ? String((file as File & { path?: string }).path ?? "") : "")
              .filter(Boolean);
            void analyzePaths(paths);
          }}
          disabled={!profileId || analyzing || installing || busy}
        >
          <input ref={fileInputRef} type="file" accept=".jar" multiple hidden />
          <strong>{analyzing ? "解析中..." : "Jar をドロップ / クリックして選択"}</strong>
          <span>対応していないJar・必須依存が不足しているJar・古い更新Jarは却下します。</span>
        </button>

        {analyses.length > 0 ? (
          <div className="local-mod-analysis-list">
            {analyses.map((item) => (
              <article className={`local-mod-analysis-card is-${item.severity}`} key={item.filePath}>
                <div className="local-mod-analysis-head">
                  {item.iconData ? (
                    <img src={item.iconData} alt="" className="local-mod-icon" loading="lazy" decoding="async" />
                  ) : null}
                  <div>
                    <strong>{item.displayName}</strong>
                    <span>{item.modId ?? "modId不明"} / {item.version ?? "version不明"}</span>
                  </div>
                  <span className={`local-mod-action-pill is-${item.action}`}>{modActionLabel(item.action)}</span>
                </div>
                <p>{item.summary}</p>
                <div className="local-mod-meta-grid">
                  <span>Loader: {item.loader ?? "不明"}</span>
                  {item.existingFileName ? <span>既存: {item.existingFileName}</span> : null}
                  {item.existingVersion ? <span>既存Version: {item.existingVersion}</span> : null}
                </div>
                {item.dependencies.length > 0 ? (
                  <div className="local-mod-deps">
                    {item.dependencies.map((dep) => (
                      <span className={dep.satisfied ? "is-ok" : dep.required ? "is-ng" : "is-warn"} key={`${item.filePath}-${dep.modId}`}>
                        {dep.satisfied ? "✓" : dep.required ? "!" : "?"} {dep.modId} {dep.requirement}
                      </span>
                    ))}
                  </div>
                ) : null}
              </article>
            ))}
          </div>
        ) : null}

        <div className="modal-actions">
          <button type="button" className="secondary-button" onClick={onClose} disabled={analyzing || installing}>
            閉じる
          </button>
          <button
            type="button"
            className="play-button"
            onClick={importInstallable}
            disabled={installable.length === 0 || analyzing || installing || busy}
          >
            {installing ? "導入中..." : `${installable.length} 件を追加 / 更新`}
          </button>
        </div>
      </article>
    </div>
  );
}
