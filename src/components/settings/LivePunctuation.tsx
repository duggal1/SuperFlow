import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { Dropdown } from "../ui/Dropdown";
import { useSettings } from "../../hooks/useSettings";

interface LivePunctuationProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const LivePunctuation: React.FC<LivePunctuationProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const enabled = getSetting("live_punctuation_enabled") ?? false;
    const style = getSetting("punctuation_style") ?? "informal";

    return (
      <div className={grouped ? "flex flex-col gap-1" : ""}>
        <ToggleSwitch
          checked={enabled}
          onChange={(nextEnabled) =>
            updateSetting("live_punctuation_enabled", nextEnabled)
          }
          isUpdating={isUpdating("live_punctuation_enabled")}
          label={t("settings.advanced.livePunctuation.title")}
          description={t("settings.advanced.livePunctuation.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />
        {enabled && (
          <div className="flex items-center justify-end pr-1">
            <Dropdown
              options={[
                {
                  value: "informal",
                  label: t("settings.advanced.livePunctuation.informal"),
                },
                {
                  value: "formal",
                  label: t("settings.advanced.livePunctuation.formal"),
                },
              ]}
              selectedValue={style}
              onSelect={(next) =>
                updateSetting(
                  "punctuation_style",
                  next as "informal" | "formal",
                )
              }
            />
          </div>
        )}
      </div>
    );
  },
);
