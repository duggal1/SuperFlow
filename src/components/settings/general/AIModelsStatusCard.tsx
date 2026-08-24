import React from "react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Badge } from "../../ui/Badge";
import { StatusPing, type StatusPingTone } from "../../ui/StatusPing";
import { useCleanupModel } from "@/hooks/useCleanupModel";

/** Live backend status of the mandatory on-device cleanup model. */
export const AIModelsStatusCard: React.FC = () => {
  const { t } = useTranslation();
  const { status } = useCleanupModel();
  const degraded =
    status?.last_run?.lifecycle === "partially_applied" ||
    status?.last_run?.lifecycle === "rejected" ||
    status?.last_run?.lifecycle === "failed";

  const tone: StatusPingTone =
    status?.active && !degraded && !status.cleaning
      ? "green"
      : degraded ||
          status?.installing ||
          (status?.installed && !status.last_error)
        ? "orange"
        : "rose";

  const badge =
    status?.active && status.cleaning
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
                label: t("settings.aiModels.installing"),
                variant: "orange" as const,
              }
            : status?.installed && !status.last_error
              ? {
                  label: t("settings.aiModels.loading"),
                  variant: "orange" as const,
                }
              : {
                  label: t("settings.aiModels.unavailable"),
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
            {status?.model_name ?? t("settings.aiModels.loading")}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2.5">
          <StatusPing tone={tone} />
          <Badge variant={badge.variant}>{badge.label}</Badge>
        </div>
      </div>
    </SettingsGroup>
  );
};
