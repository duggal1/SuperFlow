import React from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Badge } from "../../ui/Badge";
import { StatusPing, type StatusPingTone } from "../../ui/StatusPing";
import { useCleanupModel } from "@/hooks/useCleanupModel";
import { useSettings } from "../../../hooks/useSettings";

/** Live status of the optional on-device S1-mini cleanup model, plus the
 * master toggle that gates the whole backend: off (default) means the model
 * is never downloaded, loaded, or run; on means it downloads immediately
 * with live progress and powers text cleanup until switched off again. */
export const AIModelsStatusCard: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const enabled = getSetting("cleanup_model_enabled") ?? false;
  const updating = isUpdating("cleanup_model_enabled");
  const { status, progress } = useCleanupModel();

  const degraded =
    status?.last_run?.lifecycle === "partially_applied" ||
    status?.last_run?.lifecycle === "rejected" ||
    status?.last_run?.lifecycle === "failed";

  const tone: StatusPingTone = !enabled
    ? "rose"
    : status?.active && !degraded && !status.cleaning
      ? "green"
      : degraded ||
          status?.installing ||
          (status?.installed && !status.last_error)
        ? "orange"
        : "rose";

  const badge = !enabled
    ? {
        label: t("settings.aiModels.disabled", { defaultValue: "Off" }),
        variant: "neutral" as const,
      }
    : status?.active && status.cleaning
      ? {
          label: t("settings.aiModels.cleaning", { defaultValue: "Cleaning" }),
          variant: "orange" as const,
        }
      : status?.active && degraded
        ? {
            label: t("settings.aiModels.degraded", {
              defaultValue: "Degraded",
            }),
            variant: "orange" as const,
          }
        : status?.active
          ? { label: t("modelSelector.active"), variant: "green" as const }
          : status?.installing
            ? {
                label: t("settings.aiModels.installing", {
                  defaultValue: "Installing",
                }),
                variant: "orange" as const,
              }
            : status?.installed && !status.last_error
              ? {
                  label: t("settings.aiModels.loading", {
                    defaultValue: "Loading",
                  }),
                  variant: "orange" as const,
                }
              : {
                  label: t("settings.aiModels.unavailable", {
                    defaultValue: "Unavailable",
                  }),
                  variant: "rose" as const,
                };

  return (
    <SettingsGroup title={t("settings.aiModels.title")}>
      <div className="flex min-h-12 items-center justify-between gap-3 px-4 py-2">
        <div className="min-w-0">
          <h3 className="text-sm font-medium">
            {t("settings.aiModels.cleanupModel")}
          </h3>
          <p className="truncate text-xs text-text/50">
            {!enabled
              ? t("settings.aiModels.offByDefault", {
                  defaultValue:
                    "Optional cleanup model for punctuation and grammar. Fully optional.",
                })
              : (status?.model_name ?? t("settings.aiModels.loading"))}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2.5">
          <StatusPing tone={tone} />
          <Badge variant={badge.variant}>{badge.label}</Badge>
          <button
            type="button"
            role="switch"
            aria-checked={enabled}
            aria-label={t("settings.aiModels.cleanupModel")}
            disabled={updating}
            onClick={() => updateSetting("cleanup_model_enabled", !enabled)}
            className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors duration-200 ${
              enabled ? "bg-blue-600" : "bg-stone-700"
            } ${updating ? "cursor-not-allowed opacity-50" : "cursor-pointer"}`}
          >
            <motion.span
              initial={false}
              animate={{ x: enabled ? 22 : 2 }}
              transition={{
                type: "spring",
                stiffness: 500,
                damping: 32,
                mass: 0.6,
              }}
              className="pointer-events-none absolute left-0 top-1/2 -mt-2.5 block size-5 rounded-full bg-stone-50"
            />
          </button>
        </div>
      </div>

      {/* Live install progress: real download percentage streamed from the
          backend while the opt-in install runs. */}
      {enabled && status?.installing && (
        <div className="px-4 pb-3">
          <div className="h-1.5 w-full overflow-hidden rounded-full bg-stone-800">
            <div
              className="h-full rounded-full bg-blue-600 transition-all duration-300"
              style={{ width: `${Math.min(100, progress)}%` }}
            />
          </div>
          <p className="mt-1 text-xs tabular-nums text-text/50">
            {t("onboarding.cleanup.downloading", {
              defaultValue: "Downloading model… {{percentage}}%",
              percentage: Math.round(Math.min(100, progress)),
            })}
          </p>
        </div>
      )}
    </SettingsGroup>
  );
};
