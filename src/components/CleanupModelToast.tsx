import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { listen } from "@tauri-apps/api/event";
import { CheckCircle, CircleNotch } from "@phosphor-icons/react";

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
  const [state, setState] = useState<{
    visible: boolean;
    percentage: number;
    speed: number | null;
  }>({ visible: false, percentage: 0, speed: null });

  const lastEvent = useRef<{ at: number; downloaded: number } | null>(null);
  const emaSpeed = useRef<number | null>(null);
  const visibleRef = useRef(false);

  const show = useCallback((percentage: number, speed: number | null) => {
    visibleRef.current = true;
    setState({ visible: true, percentage, speed });
  }, []);

  const hide = useCallback(() => {
    visibleRef.current = false;
    lastEvent.current = null;
    emaSpeed.current = null;
    setState({ visible: false, percentage: 0, speed: null });
  }, []);

  // Mirror the live state into one persistent sonner toast slot.
  useEffect(() => {
    if (!state.visible) {
      toast.dismiss(TOAST_ID);
      return;
    }
    toast.custom(
      () => (
        <div className="flex items-center gap-2 rounded-[7px] bg-stone-800 px-3 py-3 text-sm text-stone-100">
          <CircleNotch className="size-4 shrink-0 animate-spin text-blue-500" />
          <span className="truncate">{t("cleanupToast.downloading")}</span>
          <span className="ms-auto flex shrink-0 items-center gap-2 tabular-nums text-text/50">
            <span>{Math.round(state.percentage)}%</span>
            {state.speed !== null && state.speed > 0 && (
              <span>
                {t("cleanupToast.speed", {
                  speed: (state.speed / 1_048_576).toFixed(1),
                })}
              </span>
            )}
          </span>
        </div>
      ),
      { id: TOAST_ID, duration: Infinity },
    );
  }, [state, t]);

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
      hide();
      toast.success(t("cleanupToast.success"), {
        icon: <CheckCircle weight="fill" className="size-4 text-[#34D399]" />,
      });
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

  // Rendering happens through the sonner portal (toast.custom above).
  return null;
}
