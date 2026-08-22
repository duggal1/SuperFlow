import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface TechLexiconProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const TechLexicon: React.FC<TechLexiconProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const enabled = getSetting("tech_lexicon_enabled") ?? true;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(nextEnabled) =>
          updateSetting("tech_lexicon_enabled", nextEnabled)
        }
        isUpdating={isUpdating("tech_lexicon_enabled")}
        label={t("settings.advanced.techLexicon.title")}
        description={t("settings.advanced.techLexicon.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
