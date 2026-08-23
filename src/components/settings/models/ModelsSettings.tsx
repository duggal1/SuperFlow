import React, { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  CaretDown,
  ClockCounterClockwise,
  Equalizer,
  Funnel,
  GlobeSimple,
  Translate,
} from "@phosphor-icons/react";
import type { ModelCardStatus } from "@/components/onboarding";
import { ModelCard } from "@/components/onboarding";
import { useModelStore } from "@/stores/modelStore";
import {
  Dropdown,
  Menu,
  MenuPopup,
  MenuRadioGroup,
  MenuRadioItem,
  MenuTrigger,
} from "@/components/ui/Dropdown";
import {
  getLanguageFlag,
  MODEL_CAPABILITY_LANGUAGES,
  supportsLanguageCode,
} from "@/lib/constants/languages.ts";
import type { ModelInfo } from "@/bindings";

// check if model supports a language based on its supported_languages list
const modelSupportsLanguage = (model: ModelInfo, langCode: string): boolean => {
  return supportsLanguageCode(model.supported_languages, langCode);
};

// Filter modes for the model catalog.
type SortMode = "default" | "accurate" | "fastest" | "ratio";

// Accuracy-to-speed ratio via harmonic mean: a model must be BOTH fast and
// accurate to rank high — strong on one axis alone drags the score down.
const accuracySpeedRatio = (model: ModelInfo): number => {
  const accuracy = Math.min(1, Math.max(0, model.accuracy_score));
  const speed = Math.min(1, Math.max(0, model.speed_score));
  if (accuracy <= 0 || speed <= 0) return 0;
  return (2 * accuracy * speed) / (accuracy + speed);
};

// Sort menu options, in display order.
const SORT_OPTIONS: { value: SortMode; labelKey: string }[] = [
  { value: "default", labelKey: "settings.models.filters.sortDefault" },
  { value: "accurate", labelKey: "settings.models.filters.sortAccurate" },
  { value: "fastest", labelKey: "settings.models.filters.sortFastest" },
  { value: "ratio", labelKey: "settings.models.filters.sortRatio" },
];

export const ModelsSettings: React.FC = () => {
  const { t } = useTranslation();
  const [switchingModelId, setSwitchingModelId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [filterStreaming, setFilterStreaming] = useState(false);
  const [filterTranslation, setFilterTranslation] = useState(false);
  const [languageFilter, setLanguageFilter] = useState("all");
  const [sortMode, setSortMode] = useState<SortMode>("accurate");
  const {
    models,
    currentModel,
    downloadingModels,
    downloadProgress,
    downloadStats,
    verifyingModels,
    extractingModels,
    loading,
    isRescanning,
    downloadModel,
    cancelDownload,
    selectModel,
    deleteModel,
    rescanLocalModels,
  } = useModelStore();

  // Language filter options for the shared Dropdown, each prefixed with its flag
  const languageOptions = useMemo(
    () =>
      MODEL_CAPABILITY_LANGUAGES.map((lang) => ({
        value: lang.value,
        label: lang.label,
        icon: getLanguageFlag(lang.value) ? (
          <span aria-hidden="true">{getLanguageFlag(lang.value)}</span>
        ) : undefined,
      })),
    [],
  );

  const getModelStatus = (modelId: string): ModelCardStatus => {
    if (modelId in extractingModels) {
      return "extracting";
    }
    if (modelId in verifyingModels) {
      return "verifying";
    }
    if (modelId in downloadingModels) {
      return "downloading";
    }
    if (switchingModelId === modelId) {
      return "switching";
    }
    if (modelId === currentModel) {
      return "active";
    }
    const model = models.find((m: ModelInfo) => m.id === modelId);
    if (model?.is_downloaded) {
      return "available";
    }
    return "downloadable";
  };

  const getDownloadProgress = (modelId: string): number | undefined => {
    const progress = downloadProgress[modelId];
    return progress?.percentage;
  };

  const getDownloadSpeed = (modelId: string): number | undefined => {
    const stats = downloadStats[modelId];
    return stats?.speed;
  };

  const handleModelSelect = async (modelId: string) => {
    setSwitchingModelId(modelId);
    try {
      await selectModel(modelId);
    } finally {
      setSwitchingModelId(null);
    }
  };

  const handleModelDownload = async (modelId: string) => {
    await downloadModel(modelId);
  };

  const handleModelDelete = async (modelId: string) => {
    const model = models.find((m: ModelInfo) => m.id === modelId);
    const modelName = model?.name || modelId;
    const isActive = modelId === currentModel;

    const confirmed = await ask(
      isActive
        ? t("settings.models.deleteActiveConfirm", { modelName })
        : t("settings.models.deleteConfirm", { modelName }),
      {
        title: t("settings.models.deleteTitle"),
        kind: "warning",
      },
    );

    if (confirmed) {
      try {
        await deleteModel(modelId);
      } catch (err) {
        console.error(`Failed to delete model ${modelId}:`, err);
      }
    }
  };

  const handleModelCancel = async (modelId: string) => {
    try {
      await cancelDownload(modelId);
    } catch (err) {
      console.error(`Failed to cancel download for ${modelId}:`, err);
    }
  };

  // Filter models by search query (name + description), language filter, and
  // toggles, then sort by the selected filter mode (default keeps catalog order).
  const filteredModels = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    const filtered = models.filter((model: ModelInfo) => {
      if (languageFilter !== "all") {
        if (!modelSupportsLanguage(model, languageFilter)) return false;
      }
      if (filterStreaming && !model.supports_streaming) return false;
      if (filterTranslation && !model.supports_translation) return false;

      if (q) {
        const haystack = `${model.name} ${model.description}`.toLowerCase();
        if (!haystack.includes(q)) return false;
      }
      return true;
    });

    if (sortMode === "default") return filtered;

    const sorted = [...filtered];
    if (sortMode === "accurate") {
      sorted.sort((a, b) => b.accuracy_score - a.accuracy_score);
    } else if (sortMode === "fastest") {
      sorted.sort((a, b) => b.speed_score - a.speed_score);
    } else {
      sorted.sort((a, b) => accuracySpeedRatio(b) - accuracySpeedRatio(a));
    }
    return sorted;
  }, [
    models,
    languageFilter,
    filterStreaming,
    filterTranslation,
    searchQuery,
    sortMode,
  ]);

  // Split filtered models into downloaded (including custom) and available sections
  const { downloadedModels, availableModels } = useMemo(() => {
    const downloaded: ModelInfo[] = [];
    const available: ModelInfo[] = [];

    for (const model of filteredModels) {
      if (
        model.is_custom ||
        model.is_downloaded ||
        model.id in downloadingModels ||
        model.id in extractingModels
      ) {
        downloaded.push(model);
      } else {
        available.push(model);
      }
    }

    // Sort: active model first, then non-custom, then custom at the bottom
    downloaded.sort((a, b) => {
      if (a.id === currentModel) return -1;
      if (b.id === currentModel) return 1;
      if (a.is_custom !== b.is_custom) return a.is_custom ? 1 : -1;
      return 0;
    });

    return {
      downloadedModels: downloaded,
      availableModels: available,
    };
  }, [filteredModels, downloadingModels, extractingModels, currentModel]);

  if (loading) {
    return (
      <div className="max-w-3xl w-full mx-auto">
        <div className="flex items-center justify-center py-16">
          <div className="w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full animate-spin" />
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-3xl w-full mx-auto space-y-4">
      <div className="mb-4">
        <h1 className="text-xl font-semibold mb-2">
          {t("settings.models.title")}
        </h1>
        <p className="text-sm text-text/60">
          {t("settings.models.description")}
        </p>
      </div>

      {/* Search bar — filter the catalog by name or description */}
      <input
        type="text"
        value={searchQuery}
        onChange={(e) => setSearchQuery(e.target.value)}
        placeholder={t("settings.models.searchPlaceholder")}
        className="w-full px-3 py-2 text-sm bg-stone-900 border border-stone-700 rounded-lg focus:outline-none placeholder:text-text/40"
      />

      <div className="space-y-6">
        {/* Downloaded Models Section — header always visible so filter stays accessible */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-medium text-text/60">
              {t("settings.models.yourModels")}
            </h2>
            <div className="flex items-center gap-2">
              {/* Rescan local sources for models added outside SuperFlow */}
              <button
                type="button"
                onClick={() => rescanLocalModels()}
                disabled={isRescanning}
                title={t("settings.models.rescan.tooltip")}
                aria-label={t("settings.models.rescan.tooltip")}
                className={`flex items-center justify-center size-8 rounded-lg bg-stone-900 text-text/60 hover:bg-stone-800 transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
                  isRescanning ? "animate-spin" : ""
                }`}
              >
                <ClockCounterClockwise size={16} />
              </button>

              <button
                type="button"
                onClick={() => setFilterStreaming((enabled) => !enabled)}
                title={t("settings.models.filters.streaming")}
                aria-label={t("settings.models.filters.streaming")}
                aria-pressed={filterStreaming}
                className={`flex items-center justify-center size-8 rounded-lg transition-colors ${
                  filterStreaming
                    ? "bg-blue-600/20 text-blue-500 hover:bg-blue-600/30"
                    : "bg-stone-900 text-text/60 hover:bg-stone-800"
                }`}
              >
                <Equalizer size={16} />
              </button>
              <button
                type="button"
                onClick={() => setFilterTranslation((enabled) => !enabled)}
                title={t("settings.models.filters.translation")}
                aria-label={t("settings.models.filters.translation")}
                aria-pressed={filterTranslation}
                className={`flex items-center justify-center size-8 rounded-lg transition-colors ${
                  filterTranslation
                    ? "bg-blue-600/20 text-blue-500 hover:bg-blue-600/30"
                    : "bg-stone-900 text-text/60 hover:bg-stone-800"
                }`}
              >
                <Translate size={16} />
              </button>

              {/* Language filter — shared Dropdown with flag prefixes */}
              <Dropdown
                options={[
                  {
                    value: "all",
                    label: t("settings.models.filters.allTranslate"),
                    icon: <GlobeSimple size={14} weight="regular" />,
                  },
                  ...languageOptions,
                ]}
                selectedValue={languageFilter}
                onSelect={(value) => setLanguageFilter(value)}
                placeholder={t("settings.models.filters.allTranslate")}
                searchable
                searchPlaceholder={t(
                  "settings.general.language.searchPlaceholder",
                )}
                emptyLabel={t("settings.general.language.noResults")}
                className="h-8 min-w-0 max-w-[132px] text-sm"
              />
            </div>
          </div>

          {/* Filter — below the toggle row, icon prefix + label, shared radio-dot menu */}
          <Menu>
            <MenuTrigger
              aria-label={t("settings.models.filters.sort")}
              title={t("settings.models.filters.sort")}
              className="flex h-7 cursor-pointer items-center gap-2 rounded-lg border border-[#37332f] bg-[#302c29] px-3 text-[13px] text-text/60 outline-none transition-colors duration-200 hover:border-[#44403c] hover:bg-[#292524]"
            >
              <Funnel size={16} />
              <span>{t("settings.models.filters.sort")}</span>
              <CaretDown size={12} className="text-text/40" />
            </MenuTrigger>
            <MenuPopup
              align="start"
              className="min-w-48 rounded-lg border-none bg-stone-700 p-1 text-stone-50"
            >
              <MenuRadioGroup
                value={sortMode}
                onValueChange={(value) =>
                  setSortMode(String(value) as SortMode)
                }
              >
                {SORT_OPTIONS.map((option) => (
                  <MenuRadioItem
                    key={option.value}
                    value={option.value}
                    className="flex cursor-pointer select-none items-center gap-1.5 rounded-md px-1.5 py-1 text-[13px] text-stone-50 outline-none data-disabled:pointer-events-none data-disabled:opacity-50 data-highlighted:bg-stone-600"
                  >
                    {t(option.labelKey)}
                  </MenuRadioItem>
                ))}
              </MenuRadioGroup>
            </MenuPopup>
          </Menu>

          {downloadedModels.map((model: ModelInfo) => (
            <ModelCard
              key={model.id}
              model={model}
              status={getModelStatus(model.id)}
              onSelect={handleModelSelect}
              onDownload={handleModelDownload}
              onDelete={handleModelDelete}
              onCancel={handleModelCancel}
              downloadProgress={getDownloadProgress(model.id)}
              downloadSpeed={getDownloadSpeed(model.id)}
              showRecommended={false}
            />
          ))}
        </div>

        {/* Available Models Section */}
        {availableModels.length > 0 && (
          <div className="space-y-3">
            <h2 className="text-sm font-medium text-text/60">
              {t("settings.models.availableModels")}
            </h2>
            {availableModels.map((model: ModelInfo) => (
              <ModelCard
                key={model.id}
                model={model}
                status={getModelStatus(model.id)}
                onSelect={handleModelSelect}
                onDownload={handleModelDownload}
                onDelete={handleModelDelete}
                onCancel={handleModelCancel}
                downloadProgress={getDownloadProgress(model.id)}
                downloadSpeed={getDownloadSpeed(model.id)}
                showRecommended={true}
              />
            ))}
          </div>
        )}
        {filteredModels.length === 0 && (
          <div className="text-center py-8 text-text/50">
            {t("settings.models.noModelsMatch")}
          </div>
        )}
      </div>
    </div>
  );
};
