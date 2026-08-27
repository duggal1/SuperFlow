import React from "react";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface ShowLiveStreamingProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const ShowLiveStreaming: React.FC<ShowLiveStreamingProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const enabled = getSetting("show_live_streaming") ?? true;

  return (
    <ToggleSwitch
      checked={enabled}
      onChange={(enabled) => updateSetting("show_live_streaming", enabled)}
      isUpdating={isUpdating("show_live_streaming")}
      label="Show live streaming"
      description="UI component that renders streaming."
      descriptionMode={descriptionMode}
      grouped={grouped}
    />
  );
};
