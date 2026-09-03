import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";
import { SettingContainer } from "../ui/SettingContainer";
import { useIsLight } from "../../lib/utils/theme";

interface CustomWordsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

const normalizeCustomWord = (word: string) =>
  word
    .replace(/[<>"']/g, "")
    .replace(/\s+/g, " ")
    .trim();

export const CustomWords: React.FC<CustomWordsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const isLight = useIsLight();
    const [newWord, setNewWord] = useState("");
    const customWords = getSetting("custom_words") || [];
    const normalizedWord = normalizeCustomWord(newWord);

    const handleAddWord = () => {
      if (normalizedWord && normalizedWord.length <= 50) {
        if (customWords.includes(normalizedWord)) {
          toast.error(
            t("settings.advanced.customWords.duplicate", {
              word: normalizedWord,
            }),
          );
          return;
        }
        updateSetting("custom_words", [...customWords, normalizedWord]);
        setNewWord("");
      }
    };

    const handleRemoveWord = (wordToRemove: string) => {
      updateSetting(
        "custom_words",
        customWords.filter((word) => word !== wordToRemove),
      );
    };

    const handleKeyPress = (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleAddWord();
      }
    };

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.customWords.title")}
          description={t("settings.advanced.customWords.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="flex items-center gap-2">
            <Input
              type="text"
              className={`flex-1 max-w-52 !rounded-lg placeholder:text-stone-500 py-1 ${isLight ? "!bg-stone-100/80 !border-0" : "!bg-stone-800 !border-stone-700"}`}
              value={newWord}
              onChange={(e) => setNewWord(e.target.value)}
              onKeyDown={handleKeyPress}
              placeholder={t("settings.advanced.customWords.placeholder")}
              variant="compact"
              disabled={isUpdating("custom_words")}
            />
            <Button
              onClick={handleAddWord}
              disabled={
                !normalizedWord ||
                normalizedWord.length > 50 ||
                isUpdating("custom_words")
              }
              variant="secondary"
              size="sm"
              className={
                isLight
                  ? "!bg-white !border !border-stone-200/70 hover:!bg-stone-100/80 hover:!border-0 !rounded-lg"
                  : ""
              }
            >
              {t("settings.advanced.customWords.add")}
            </Button>
          </div>
        </SettingContainer>
        {customWords.length > 0 && (
          <div className="flex w-full flex-wrap justify-start gap-1.5 px-4 pt-3 pb-3">
            {customWords.map((word) => (
              <button
                type="button"
                key={word}
                onClick={() => handleRemoveWord(word)}
                disabled={isUpdating("custom_words")}
                className={`inline-flex cursor-pointer items-center gap-1.5 rounded-lg border-0 px-4 py-1.5 text-xs leading-none transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${isLight ? "bg-stone-100/80 text-stone-700 hover:bg-stone-100" : "bg-stone-800 text-stone-100 hover:bg-stone-700"}`}
                aria-label={t("settings.advanced.customWords.remove", {
                  word,
                })}
              >
                <span>{word}</span>
                <svg
                  className="h-3 w-3"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            ))}
          </div>
        )}
      </>
    );
  },
);
