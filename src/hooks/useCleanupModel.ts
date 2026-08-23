import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { commands, type CleanupModelStatus } from "@/bindings";

export interface CleanupModelState {
  status: CleanupModelStatus | null;
  /** Download progress in percent while installing. */
  progress: number;
}

/**
 * Shared state + actions for the mandatory S1-mini text clean-up model.
 * Used by both the onboarding install page and the dashboard models card —
 * only one of them is ever mounted at a time, so one listener set is enough.
 */
export function useCleanupModel() {
  const [status, setStatus] = useState<CleanupModelStatus | null>(null);
  const [progress, setProgress] = useState(0);

  const refresh = useCallback(async () => {
    try {
      const result = await commands.getCleanupModelStatus();
      if (result.status === "ok") setStatus(result.data);
    } catch (e) {
      console.warn("Failed to get cleanup model status:", e);
    }
  }, []);

  useEffect(() => {
    void refresh();

    const unlistenProgress = listen<{ percentage: number }>(
      "cleanup-model-progress",
      (event) => {
        setProgress(event.payload.percentage);
        setStatus((prev) =>
          prev ? { ...prev, installing: true } : prev,
        );
      },
    );

    const unlistenComplete = listen("cleanup-model-complete", () => {
      setProgress(100);
      void refresh();
    });

    const unlistenFailed = listen<{ error: string }>(
      "cleanup-model-failed",
      () => {
        void refresh();
      },
    );

    return () => {
      unlistenProgress.then((fn) => fn());
      unlistenComplete.then((fn) => fn());
      unlistenFailed.then((fn) => fn());
    };
  }, [refresh]);

  const install = useCallback(async () => {
    setProgress(0);
    try {
      await commands.installCleanupModel();
    } catch (e) {
      console.warn("Failed to start cleanup model install:", e);
      void refresh();
    }
  }, [refresh]);

  return { status, progress, install, refresh };
}
