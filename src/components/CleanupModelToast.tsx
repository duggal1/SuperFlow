import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { Sonner, type SonnerState } from "./toast";

interface ProgressPayload {
  downloaded: number;
  total: number;
  percentage: number;
}

const TOAST_ID = "cleanup-model-install";

/**
 * Global bottom-right indicator for the mandatory clean-up model install.
 * Backend-driven: appears the moment progress events start streaming
 * (auto-install at launch or explicit install), shows real throughput
 * computed from consecutive events, flips to success on completion.
 */
export function CleanupModelToast({ enabled = true }: { enabled?: boolean }) {
  const { t } = useTranslation();
  const [sonner, setSonner] = useState<SonnerState | null>(null);

  const lastEvent = useRef<{ at: number; downloaded: number } | null>(null);
  const emaSpeed = useRef<number | null>(null);
  const visibleRef = useRef(false);

  const show = useCallback(
    (percentage: number, speed: number | null) => {
      visibleRef.current = true;
      const speedText =
        speed !== null && speed > 0
          ? ` · ${(speed / 1_048_576).toFixed(1)} MB/s`
          : "";
      setSonner({
        kind: "loading",
        message: `${t("cleanupToast.downloading")} ${Math.round(percentage)}%${speedText}`,
        id: TOAST_ID,
      });
    },
    [t],
  );

  const hide = useCallback(() => {
    visibleRef.current = false;
    lastEvent.current = null;
    emaSpeed.current = null;
    setSonner(null);
  }, []);

  useEffect(() => {
    if (!enabled && visibleRef.current) hide();

    const unlistenProgress = listen<ProgressPayload>(
      "cleanup-model-progress",
      (event) => {
        if (!enabled) return;
        const { downloaded } = event.payload;
        const now = performance.now();

        // Real throughput: exponential moving average over raw byte deltas.
        if (lastEvent.current) {
          const dt = (now - lastEvent.current.at) / 1000;
          if (dt > 0.2) {
            const instant = (downloaded - lastEvent.current.downloaded) / dt;
            emaSpeed.current =
              emaSpeed.current === null
                ? instant
                : emaSpeed.current * 0.7 + instant * 0.3;
          }
        }
        lastEvent.current = { at: now, downloaded };

        show(event.payload.percentage, emaSpeed.current);
      },
    );

    const unlistenComplete = listen("cleanup-model-complete", () => {
      if (!enabled || !visibleRef.current) return;
      setSonner({ kind: "success", message: t("cleanupToast.success"), id: `${TOAST_ID}-done` });
      window.setTimeout(hide, 3000);
    });

    const unlistenFailed = listen<{ error: string }>(
      "cleanup-model-failed",
      () => {
        if (!visibleRef.current) return;
        hide();
      },
    );

    return () => {
      unlistenProgress.then((fn) => fn());
      unlistenComplete.then((fn) => fn());
      unlistenFailed.then((fn) => fn());
    };
  }, [enabled, hide, show, t]);

  return <Sonner sonner={sonner} />;
}
