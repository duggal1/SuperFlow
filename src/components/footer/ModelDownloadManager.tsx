import { useEffect, useMemo, useRef, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { CaretUp, DownloadSimple, Stop, Trash } from "@phosphor-icons/react";
import { useTranslation } from "react-i18next";
import { getTranslatedModelName } from "@/lib/utils/modelTranslation";
import { useModelStore } from "@/stores/modelStore";
import { useIsLight } from "@/lib/utils/theme";

const clampPercentage = (value: number | undefined): number =>
  Math.max(0, Math.min(100, Math.round(value ?? 0)));

export default function ModelDownloadManager() {
  const { t } = useTranslation();
  const {
    models,
    downloadingModels,
    downloadProgress,
    downloadStats,
    downloadModel,
    cancelDownload,
    deleteModel,
  } = useModelStore();
  const [open, setOpen] = useState(false);
  const [trackedIds, setTrackedIds] = useState<string[]>([]);
  const [pendingIds, setPendingIds] = useState<string[]>([]);
  const rootRef = useRef<HTMLDivElement>(null);

  const activeIds = useMemo(
    () => Object.keys(downloadingModels),
    [downloadingModels],
  );

  useEffect(() => {
    if (activeIds.length === 0) return;
    setTrackedIds((current) => Array.from(new Set([...activeIds, ...current])));
  }, [activeIds]);

  useEffect(() => {
    if (!open) return;
    const closeOnOutsideClick = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", closeOnOutsideClick);
    return () => document.removeEventListener("mousedown", closeOnOutsideClick);
  }, [open]);

  const visibleIds = useMemo(
    () => Array.from(new Set([...activeIds, ...trackedIds])),
    [activeIds, trackedIds],
  );

  const averageProgress =
    activeIds.length === 0
      ? 0
      : Math.round(
          activeIds.reduce(
            (total, id) =>
              total + clampPercentage(downloadProgress[id]?.percentage),
            0,
          ) / activeIds.length,
        );

  const runForId = async (modelId: string, action: () => Promise<boolean>) => {
    setPendingIds((current) => [...current, modelId]);
    try {
      await action();
    } finally {
      setPendingIds((current) => current.filter((id) => id !== modelId));
    }
  };

  const stopAll = async () => {
    await Promise.all(
      activeIds.map((modelId) =>
        runForId(modelId, () => cancelDownload(modelId)),
      ),
    );
  };

  const removeModel = async (modelId: string, modelName: string) => {
    const confirmed = await ask(
      t("settings.models.deleteConfirm", { modelName }),
      { title: t("settings.models.deleteTitle"), kind: "warning" },
    );
    if (!confirmed) return;
    await runForId(modelId, () => deleteModel(modelId));
    setTrackedIds((current) => current.filter((id) => id !== modelId));
  };

  const isLight = useIsLight();
  return (
    <div ref={rootRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((current) => !current)}
        aria-expanded={open}
        className={`group flex h-7 items-center gap-2 rounded-[6px] px-2 transition-colors duration-150 hover:underline hover:decoration-dotted hover:underline-offset-4 ${isLight ? "text-stone-500 decoration-stone-400 hover:text-stone-900" : "text-text/60 decoration-stone-500 hover:text-text"}`}
      >
        <span className="relative flex size-4 items-center justify-center">
          <DownloadSimple className="size-4" />
          {activeIds.length > 0 && (
            <span className="absolute -right-0.5 -top-0.5 size-1.5 rounded-full bg-blue-500" />
          )}
        </span>
        <span className="max-w-40 truncate">
          {activeIds.length > 0
            ? t("footer.downloadManager.active", {
                count: activeIds.length,
                progress: averageProgress,
              })
            : t("footer.downloadManager.title")}
        </span>
        <CaretUp
          className={`size-3 transition-transform duration-150 ${open ? "rotate-180" : ""}`}
        />
      </button>

      {open && (
        <div
          className={`absolute bottom-full left-0 z-50 mb-2 w-[360px] overflow-hidden rounded-[8px] p-1.5 shadow-none ${
            isLight
              ? "bg-white border border-stone-200/70 text-stone-900"
              : "bg-[#363230] border border-white/[0.06] text-text"
          }`}
        >
          <div className="flex items-center justify-between px-3 pb-2 pt-2.5">
            <div>
              <p
                className={`text-[13px] font-medium ${isLight ? "text-stone-900" : ""}`}
              >
                {t("footer.downloadManager.title")}
              </p>
              <p
                className={`text-[11px] ${isLight ? "text-stone-500" : "text-text/50"}`}
              >
                {activeIds.length > 0
                  ? t("footer.downloadManager.backgroundActive", {
                      count: activeIds.length,
                    })
                  : t("footer.downloadManager.backgroundIdle")}
              </p>
            </div>
            {activeIds.length > 1 && (
              <button
                type="button"
                onClick={stopAll}
                className="rounded-[4px] px-2 py-1 text-[11px] text-rose-400 transition-colors hover:bg-rose-500/10 hover:text-rose-300"
              >
                {t("footer.downloadManager.stopAll")}
              </button>
            )}
          </div>

          <div className="max-h-72 space-y-0.5 overflow-y-auto">
            {visibleIds.length === 0 ? (
              <div
                className={`px-3 py-5 text-center text-xs ${isLight ? "text-stone-500" : "text-text/45"}`}
              >
                {t("footer.downloadManager.empty")}
              </div>
            ) : (
              visibleIds.map((modelId) => {
                const model = models.find(
                  (candidate) => candidate.id === modelId,
                );
                const active = modelId in downloadingModels;
                const pending = pendingIds.includes(modelId);
                const progress = clampPercentage(
                  downloadProgress[modelId]?.percentage,
                );
                const speed = downloadStats[modelId]?.speed ?? 0;
                const modelName = model
                  ? getTranslatedModelName(model, t)
                  : modelId;

                return (
                    <div
                    key={modelId}
                    className={`rounded-[6px] px-2.5 py-2 transition-colors ${isLight ? "hover:bg-stone-100" : "hover:bg-white/[0.04]"}`}
                  >
                    <div className="flex items-start gap-3">
                      <div className="min-w-0 flex-1">
                        <p
                          className={`truncate text-xs ${isLight ? "text-stone-900" : "text-text/90"}`}
                        >
                          {modelName}
                        </p>
                        <div
                          className={`mt-0.5 flex items-center gap-2 text-[11px] ${isLight ? "text-stone-500" : "text-text/45"}`}
                        >
                          <span>
                            {active
                              ? t("footer.downloadManager.progress", {
                                  progress,
                                })
                              : model?.is_downloaded
                                ? t("footer.downloadManager.downloaded")
                                : t("footer.downloadManager.stopped")}
                          </span>
                          {active && speed > 0 && (
                            <span>
                              {t("modelSelector.downloadSpeed", {
                                speed: speed.toFixed(1),
                              })}
                            </span>
                          )}
                        </div>
                      </div>

                      {active ? (
                        <button
                          type="button"
                          disabled={pending}
                          onClick={() =>
                            runForId(modelId, () => cancelDownload(modelId))
                          }
                          aria-label={t("modelSelector.cancelDownload")}
                          className={`flex size-7 shrink-0 items-center justify-center rounded-[4px] transition-colors disabled:opacity-40 ${isLight ? "text-stone-500 hover:bg-rose-400 hover:text-white" : "text-text/55 hover:bg-rose-700 hover:text-white"}`}
                        >
                          <Stop className="size-3.5" weight="fill" />
                        </button>
                      ) : model?.is_downloaded ? (
                        <button
                          type="button"
                          disabled={pending}
                          onClick={() => removeModel(modelId, modelName)}
                          aria-label={t("modelSelector.deleteModel", {
                            modelName,
                          })}
                          className={`flex size-7 shrink-0 items-center justify-center rounded-[4px] transition-colors disabled:opacity-40 ${isLight ? "text-stone-500 hover:bg-rose-400 hover:text-white" : "text-text/55 hover:bg-rose-700 hover:text-white"}`}
                        >
                          <Trash className="size-3.5" />
                        </button>
                      ) : (
                        <button
                          type="button"
                          disabled={pending}
                          onClick={() =>
                            runForId(modelId, () => downloadModel(modelId))
                          }
                          aria-label={t("footer.downloadManager.download", {
                            modelName,
                          })}
                          className={`flex size-7 shrink-0 items-center justify-center rounded-[4px] transition-colors disabled:opacity-40 ${isLight ? "text-stone-500 hover:bg-blue-50 hover:text-blue-600" : "text-text/55 hover:bg-blue-500/10 hover:text-blue-400"}`}
                        >
                          <DownloadSimple className="size-3.5" />
                        </button>
                      )}
                    </div>

                    {active && (
                      <div
                        className={`mt-2 h-1 overflow-hidden rounded-full ${isLight ? "bg-stone-200" : "bg-stone-700"}`}
                      >
                        <div
                          className="h-full rounded-full bg-blue-500 transition-[width] duration-200"
                          style={{ width: `${progress}%` }}
                        />
                      </div>
                    )}
                  </div>
                );
              })
            )}
          </div>
        </div>
      )}
    </div>
  );
}
