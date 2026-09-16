import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface CodeIntelligenceProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const CodeIntelligence: React.FC<CodeIntelligenceProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const enabled = getSetting("code_intelligence_enabled") ?? true;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(nextEnabled) =>
          updateSetting("code_intelligence_enabled", nextEnabled)
        }
        isUpdating={isUpdating("code_intelligence_enabled")}
        label={t("settings.advanced.codeIntelligence.title")}
        description={t("settings.advanced.codeIntelligence.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
