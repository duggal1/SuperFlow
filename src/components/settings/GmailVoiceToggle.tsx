import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { ToggleSwitch } from "../ui/ToggleSwitch";

interface GmailVoiceToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const GmailVoiceToggle: React.FC<GmailVoiceToggleProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const enabled = getSetting("experimental_gmail_voice_enabled") ?? false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(nextEnabled) =>
          updateSetting("experimental_gmail_voice_enabled", nextEnabled)
        }
        isUpdating={isUpdating("experimental_gmail_voice_enabled")}
        label={t("settings.advanced.gmailVoice.label")}
        description={t("settings.advanced.gmailVoice.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
