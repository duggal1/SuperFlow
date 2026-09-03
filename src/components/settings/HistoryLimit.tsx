import React from "react";
import { CaretDown, CaretUp } from "@phosphor-icons/react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";
import { useIsLight } from "../../lib/utils/theme";

interface HistoryLimitProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const HistoryLimit: React.FC<HistoryLimitProps> = ({
  descriptionMode = "inline",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const isLight = useIsLight();

  const historyLimit = getSetting("history_limit") ?? 5;

  const handleChange = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(event.target.value, 10);
    if (!isNaN(value) && value >= 0) {
      updateSetting("history_limit", value);
    }
  };

  const updateHistoryLimit = (change: number) => {
    const nextValue = Math.min(1000, Math.max(0, historyLimit + change));
    updateSetting("history_limit", nextValue);
  };

  return (
    <SettingContainer
      title={t("settings.debug.historyLimit.title")}
      description={t("settings.debug.historyLimit.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="horizontal"
    >
      <div className="flex items-center space-x-2">
        <div className="relative">
          <Input
            type="number"
            min="0"
            max="1000"
            value={historyLimit}
            onChange={handleChange}
            disabled={isUpdating("history_limit")}
            className="w-20 appearance-none pr-7 [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
          />
          <div className="absolute inset-y-px right-px flex w-6 flex-col overflow-hidden rounded-r-[7px]">
            <button
              type="button"
              onClick={() => updateHistoryLimit(1)}
              disabled={isUpdating("history_limit") || historyLimit >= 1000}
              aria-label="Increase history limit"
              className={`flex flex-1 items-center justify-center py-0.5 transition-colors ${isLight ? "bg-white text-stone-600 hover:bg-stone-100 disabled:text-stone-300" : "bg-stone-800 text-stone-100 hover:bg-stone-700 disabled:text-stone-600"}`}
            >
              <CaretUp size={10} weight="bold" />
            </button>
            <button
              type="button"
              onClick={() => updateHistoryLimit(-1)}
              disabled={isUpdating("history_limit") || historyLimit <= 0}
              aria-label="Decrease history limit"
              className={`flex flex-1 items-center justify-center py-0.5 transition-colors ${isLight ? "bg-white text-stone-600 hover:bg-stone-100 disabled:text-stone-300" : "bg-stone-800 text-stone-100 hover:bg-stone-700 disabled:text-stone-600"}`}
            >
              <CaretDown size={10} weight="bold" />
            </button>
          </div>
        </div>
        <span className="text-sm text-text">
          {t("settings.debug.historyLimit.entries")}
        </span>
      </div>
    </SettingContainer>
  );
};
