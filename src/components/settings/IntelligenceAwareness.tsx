import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface IntelligenceAwarenessProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const IntelligenceAwareness: React.FC<IntelligenceAwarenessProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const enabled = getSetting("intelligence_awareness_enabled") ?? false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(nextEnabled) =>
          updateSetting("intelligence_awareness_enabled", nextEnabled)
        }
        isUpdating={isUpdating("intelligence_awareness_enabled")}
        label={t("settings.advanced.intelligenceAwareness.title")}
        description={t("settings.advanced.intelligenceAwareness.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
