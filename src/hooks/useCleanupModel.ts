import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { commands, events, type CleanupModelStatus } from "@/bindings";

export interface CleanupModelState {
  status: CleanupModelStatus | null;
  /** Download progress in percent while installing. */
  progress: number;
}

/**
 * Shared state + actions for the optional S1-mini text clean-up model
 * (default off). Used by the model status card — progress events stream
 * whenever an opt-in install is running.
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
        setStatus((prev) => (prev ? { ...prev, installing: true } : prev));
      },
    );

    const unlistenComplete = listen("cleanup-model-complete", () => {
      setProgress(100);
      void refresh();
    });

    const unlistenReady = listen("cleanup-model-ready", () => {
      void refresh();
    });

    const unlistenRunStatus = events.cleanupRunStatusEvent.listen(() => {
      void refresh();
    });

    const unlistenProgressState = events.cleanupProgressEvent.listen(() => {
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
      unlistenReady.then((fn) => fn());
      unlistenRunStatus.then((fn) => fn());
      unlistenProgressState.then((fn) => fn());
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
