import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";
import { SettingContainer } from "../ui/SettingContainer";

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
          <div className="flex flex-col items-end gap-2">
            <div className="flex items-center gap-2">
              <Input
                type="text"
                className="max-w-40"
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
                variant="primary"
                size="md"
              >
                {t("settings.advanced.customWords.add")}
              </Button>
            </div>
            {customWords.length > 0 && (
              <div className="flex max-w-72 flex-wrap justify-end gap-1">
                {customWords.map((word) => (
                  <button
                    type="button"
                    key={word}
                    onClick={() => handleRemoveWord(word)}
                    disabled={isUpdating("custom_words")}
                    className="inline-flex cursor-pointer items-center gap-1.5 rounded-md border border-stone-700 bg-stone-800 px-2 py-1 text-xs leading-none text-stone-100 transition-colors hover:bg-stone-700 disabled:cursor-not-allowed disabled:opacity-50"
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
          </div>
        </SettingContainer>
      </>
    );
  },
);
