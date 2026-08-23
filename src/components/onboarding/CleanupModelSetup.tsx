import React, { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Broom, CircleNotch, Cpu, HardDrives } from "@phosphor-icons/react";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import SuperFlowTextLogo from "@/components/icons/SuperFlowTextLogo";
import { useCleanupModel } from "@/hooks/useCleanupModel";

/** Approximate download size shown on the card before install. */
const CLEANUP_MODEL_SIZE_MB = 462;

interface CleanupModelSetupProps {
  /** Advance to the dashboard. Only callable after the model is installed. */
  onComplete: () => void;
}

/**
 * Mandatory clean-up model install step. Sits between model selection and the
 * dashboard; there is no skip — dictation quality depends on this model and it
 * cannot be disabled later. Runs locally, pinned to Apple GPU (Metal).
 */
export const CleanupModelSetup: React.FC<CleanupModelSetupProps> = ({
  onComplete,
}) => {
  const { t } = useTranslation();
  const { status, progress, install } = useCleanupModel();
  const completedRef = useRef(false);

  const installed = status?.installed ?? false;

  // Hand control to the dashboard shortly after "Activated" appears so the
  // user actually sees the terminal state instead of an instant jump.
  useEffect(() => {
    if (!installed || completedRef.current) return;
    completedRef.current = true;
    const timer = setTimeout(onComplete, 900);
    return () => clearTimeout(timer);
  }, [installed, onComplete]);

  const handleInstall = () => {
    void install();
  };

  return (
    <div className="flex h-screen w-screen flex-col items-center justify-center gap-6 bg-neutral-900 p-6">
      <div className="flex flex-col items-center gap-2">
        <SuperFlowTextLogo size={22} />
        <p className="text-text/70 max-w-md font-medium mx-auto text-center">
          {t("onboarding.cleanup.subtitle")}
        </p>
      </div>

      <div className="flex w-full max-w-md flex-col items-center gap-4">
        {/* Status header: live ping dot + model name + state badge */}
        <div className="flex items-center gap-2 self-start">
          <span className="relative flex size-1.5">
            <span
              className={`absolute inline-flex h-full w-full animate-ping rounded-[1.5px] opacity-75 ${
                installed
                  ? "bg-green-500"
                  : status?.installing
                    ? "bg-orange-500"
                    : "bg-rose-500"
              }`}
            />
            <span
              className={`relative inline-flex size-1.5 rounded-[1.5px] ${
                installed
                  ? "bg-green-500"
                  : status?.installing
                    ? "bg-orange-500"
                    : "bg-rose-500"
              }`}
            />
          </span>
          <h2 className="text-sm font-medium tracking-tight text-stone-50">
            {t("onboarding.cleanup.cardTitle")}
          </h2>
          {installed && (
            <Badge variant="green">{t("onboarding.cleanup.activated")}</Badge>
          )}
        </div>

        <div
          className={`w-full rounded-lg border bg-surface p-4 transition-colors duration-200 ${
            installed ? "border-blue-600/60 bg-blue-600/10" : "border-transparent"
          }`}
        >
          <div className="flex items-start gap-4">
            <Broom size={32} className="shrink-0 text-stone-100" />
            <div className="min-w-0 flex-1">
              <div className="flex items-center justify-between gap-2">
                <h3 className="text-sm font-medium tracking-tight text-stone-50">
                  {t("onboarding.cleanup.cardTitle")}
                </h3>
                {installed ? (
                  <Badge variant="green">{t("onboarding.cleanup.activated")}</Badge>
                ) : (
                  <Badge variant="blue">{t("onboarding.cleanup.required")}</Badge>
                )}
              </div>
              <p className="mt-1 text-sm leading-relaxed text-stone-400">
                {t("onboarding.cleanup.description")}
              </p>

              <div className="mt-2.5 flex items-center gap-3 text-xs text-text/50">
                <span className="flex items-center gap-1">
                  <HardDrives className="w-3.5 h-3.5" />
                  <span>{CLEANUP_MODEL_SIZE_MB} MB</span>
                </span>
                <span className="flex items-center gap-1">
                  <Cpu className="w-3.5 h-3.5" />
                  <span>{t("onboarding.cleanup.runsOnMetal")}</span>
                </span>
              </div>

              {/* Action area */}
              <div className="mt-3">
                {installed ? (
                  <div className="flex items-center gap-2 text-sm text-stone-400">
                    <CircleNotch className="size-3.5 animate-spin" />
                    <span>{t("onboarding.cleanup.startingEngine")}</span>
                  </div>
                ) : status?.installing ? (
                  <div>
                    <div className="w-full h-1.5 bg-stone-800 rounded-full overflow-hidden">
                      <div
                        className="h-full bg-blue-600 rounded-full transition-all duration-300"
                        style={{ width: `${Math.min(100, progress)}%` }}
                      />
                    </div>
                    <p className="text-xs text-text/50 mt-1 tabular-nums">
                      {t("onboarding.cleanup.downloading", {
                        percentage: Math.round(Math.min(100, progress)),
                      })}
                    </p>
                  </div>
                ) : (
                  <Button onClick={handleInstall} variant="primary">
                    {t("onboarding.cleanup.install")}
                  </Button>
                )}
              </div>

              {status?.last_error && !status.installing && (
                <p className="mt-2 text-xs leading-relaxed text-[#FF5C5C]">
                  {t("onboarding.cleanup.error", {
                    error: status.last_error,
                  })}
                </p>
              )}
            </div>
          </div>
        </div>

        <p className="max-w-md text-center text-xs leading-relaxed text-text/40">
          {t("onboarding.cleanup.alwaysActiveNote")}
        </p>
      </div>
    </div>
  );
};
