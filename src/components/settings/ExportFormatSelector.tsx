import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import type { ExportFormat } from "@/bindings";

interface ExportFormatSelectorProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ExportFormatSelector: React.FC<ExportFormatSelectorProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const selectedFormat = getSetting("export_format") || "markdown";

    const handleFormatSelect = async (format: string) => {
      await updateSetting("export_format", format as ExportFormat);
    };

    const formatOptions = [
      { value: "markdown", label: t("settings.history.exportFormat.markdown") },
      {
        value: "plaintext",
        label: t("settings.history.exportFormat.plainText"),
      },
    ];

    return (
      <SettingContainer
        title={t("settings.history.exportFormat.title")}
        description={t("settings.history.exportFormat.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <Dropdown
          options={formatOptions}
          selectedValue={selectedFormat}
          onSelect={handleFormatSelect}
          placeholder={t("settings.history.exportFormat.placeholder")}
          disabled={isUpdating("export_format")}
        />
      </SettingContainer>
    );
  });

ExportFormatSelector.displayName = "ExportFormatSelector";
