import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface AudioEnhancementProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

/**
 * RNNoise denoising + automatic gain control on the dictation stream.
 * Removes background noise (chatter, music, fans) and lifts quiet or
 * far-from-microphone speech before the audio reaches VAD and ASR.
 */
export const AudioEnhancement: React.FC<AudioEnhancementProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const enabled = getSetting("audio_enhancement_enabled") ?? true;

  return (
    <ToggleSwitch
      checked={enabled}
      onChange={(enabled) =>
        updateSetting("audio_enhancement_enabled", enabled)
      }
      isUpdating={isUpdating("audio_enhancement_enabled")}
      label={t("settings.advanced.audioEnhancement.title")}
      description={t("settings.advanced.audioEnhancement.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
    />
  );
};
