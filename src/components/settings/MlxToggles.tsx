import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

type WarmState =
  | { phase: "idle" }
  | { phase: "warming" }
  | { phase: "ok"; detail: string }
  | { phase: "fail"; detail: string };

/// Ultra-clean experimental card for the optional native Apple-Silicon MLX
/// engine. Lives in Advanced → Experimental group. Enabling it:
///   1. persists the setting (backend re-seeds MLX model entries live),
///   2. immediately pre-warms Python/Metal so first use has zero cold start.
/// Purely additive — shipped engines are untouched.
export const MlxExperimentalCard: React.FC = React.memo(() => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const enabled = getSetting("experimental_mlx_enabled") || false;
  const [warm, setWarm] = useState<WarmState>({ phase: "idle" });

  const handleToggle = async (on: boolean) => {
    await updateSetting("experimental_mlx_enabled", on);
    if (!on) {
      setWarm({ phase: "idle" });
      return;
    }

    // Zero cold start: pull the Metal dylibs into memory right now.
    setWarm({ phase: "warming" });
    try {
      const result = await commands.warmMlxRuntime();
      if (result.status === "ok" && result.data.ok) {
        setWarm({ phase: "ok", detail: result.data.detail });
        toast.success(t("settings.advanced.mlxCard.warmToastOk"), {
          description: result.data.detail,
        });
      } else {
        const detail =
          result.status === "ok" ? result.data.detail : result.error;
        setWarm({ phase: "fail", detail });
        toast.error(t("settings.advanced.mlxCard.warmToastFail"), {
          description: detail,
        });
      }
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e);
      setWarm({ phase: "fail", detail });
      toast.error(t("settings.advanced.mlxCard.warmToastFail"), {
        description: detail,
      });
    }
  };

  return (
    <div className="rounded-xl border border-stone-800 bg-stone-900/60 p-4">
      {/* Header row: title + switch */}
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <h3 className="text-sm font-semibold text-text">
            {t("settings.advanced.mlxCard.title")}
          </h3>
        </div>
        <ToggleSwitch
          bare
          checked={enabled}
          onChange={handleToggle}
          isUpdating={isUpdating("experimental_mlx_enabled")}
          label={t("settings.advanced.mlxCard.title")}
          description=""
        />
      </div>

      {/* Warm status line */}
      {enabled && (
        <p
          className={`mt-2 text-xs ${
            warm.phase === "ok"
              ? "text-green-500"
              : warm.phase === "fail"
                ? "text-orange-400"
                : "text-text/50"
          }`}
        >
          {warm.phase === "idle" && t("settings.advanced.mlxCard.warmIdle")}
          {warm.phase === "warming" &&
            t("settings.advanced.mlxCard.warming")}
          {warm.phase === "ok" && warm.detail}
          {warm.phase === "fail" && warm.detail}
        </p>
      )}
    </div>
  );
});
