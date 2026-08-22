import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface SmartFileReferencesProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const SmartFileReferences: React.FC<SmartFileReferencesProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const enabled = getSetting("smart_file_references_enabled") ?? true;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(nextEnabled) =>
          updateSetting("smart_file_references_enabled", nextEnabled)
        }
        isUpdating={isUpdating("smart_file_references_enabled")}
        label={t("settings.advanced.fileReferences.title")}
        description={t("settings.advanced.fileReferences.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
