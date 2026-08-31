import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  CaretDown,
  ClockCounterClockwise,
  Equalizer,
  Funnel,
  GlobeSimple,
} from "@phosphor-icons/react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Search01Icon as MagnifyingGlassIcon } from "@hugeicons/core-free-icons";
import type { ModelCardStatus } from "@/components/onboarding";
import { ModelCard, isMlxModel } from "@/components/onboarding";
import { commands } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { useModelStore } from "@/stores/modelStore";
import { useIsLight } from "@/lib/utils/theme";
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
  const { getSetting } = useSettings();
  const isLight = useIsLight();
  const [switchingModelId, setSwitchingModelId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [filterStreaming, setFilterStreaming] = useState(false);
  const [languageFilter, setLanguageFilter] = useState("all");
  const [sortMode, setSortMode] = useState<SortMode>("accurate");
  // Experimental MLX gate + one-shot runtime diagnostics for the section note.
  const mlxEnabled = getSetting("experimental_mlx_enabled") || false;
  const [mlxRuntimeStatus, setMlxRuntimeStatus] = useState<{
    ready: boolean;
    detail: string;
  } | null>(null);
  useEffect(() => {
    if (!mlxEnabled) {
      setMlxRuntimeStatus(null);
      return;
    }
    let cancelled = false;
    commands
      .getMlxRuntimeInfo()
      .then((result) => {
        if (cancelled) return;
        if (result.status === "ok") {
          setMlxRuntimeStatus({
            ready: result.data.available,
            detail: result.data.status,
          });
          return;
        }
        setMlxRuntimeStatus({ ready: false, detail: result.error });
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setMlxRuntimeStatus({
            ready: false,
            detail: error instanceof Error ? error.message : String(error),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [mlxEnabled]);
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
    const model = models.find((m: ModelInfo) => m.id === modelId);
    if (model && isMlxModel(model) && !getSetting("experimental_mlx_enabled")) {
      // Hard UX gate: running an MLX model requires the experimental opt-in.
      toast.error(t("errors.mlxDisabledTitle"), {
        description: t("errors.mlxDisabledDescription"),
      });
      return;
    }
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
  }, [models, languageFilter, filterStreaming, searchQuery, sortMode]);

  // Split filtered models into downloaded (including custom), available, and
  // experimental-MLX sections
  const { downloadedModels, availableModels, mlxModels } = useMemo(() => {
    const downloaded: ModelInfo[] = [];
    const available: ModelInfo[] = [];
    const mlx: ModelInfo[] = [];

    for (const model of filteredModels) {
      if (isMlxModel(model)) {
        // Experimental MLX lives in its own gated section below; never mixed
        // into the shipped catalog lists.
        mlx.push(model);
        continue;
      }
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
      mlxModels: mlx,
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
        <h1 className={`text-xl font-semibold mb-2 ${isLight ? "text-stone-900" : ""}`}>
          {t("settings.models.title")}
        </h1>
      </div>

      {/* Search bar — filter the catalog by name or description */}
      <label className={`group flex w-full items-center gap-2 rounded-lg px-3 transition-colors duration-150 ${isLight ? "bg-white border border-stone-200 text-stone-500 hover:text-stone-700 focus-within:text-stone-900" : "bg-stone-800 text-stone-400 hover:bg-stone-800 hover:text-stone-100 focus-within:bg-stone-800 focus-within:text-stone-100"}`}>
        <HugeiconsIcon
          icon={MagnifyingGlassIcon}
          size={18}
          className="shrink-0"
        />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder={t("settings.models.searchPlaceholder")}
          className={`min-w-0 flex-1 border-0 bg-transparent py-2 text-sm text-inherit outline-none ring-0 focus:outline-none focus:ring-0 ${isLight ? "placeholder:text-stone-400 group-hover:placeholder:text-stone-600 group-focus-within:placeholder:text-stone-600" : "placeholder:text-stone-400 group-hover:placeholder:text-stone-100 group-focus-within:placeholder:text-stone-100"}`}
        />
      </label>

      <div className="space-y-6">
        {/* Downloaded Models Section — header always visible so filter stays accessible */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <h2 className={`text-sm font-medium ${isLight ? "text-stone-600" : "text-text/60"}`}>
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
                className={`flex items-center justify-center size-8 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
                  isLight ? "bg-white border border-stone-200 text-stone-500 hover:bg-stone-50" : "bg-stone-900 text-text/60 hover:bg-stone-800"
                } ${isRescanning ? "animate-spin" : ""}`}
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
                    : isLight ? "bg-white border border-stone-200 text-stone-500 hover:bg-stone-50" : "bg-stone-900 text-text/60 hover:bg-stone-800"
                }`}
              >
                <Equalizer size={16} />
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
              className={`flex h-7 cursor-pointer items-center gap-2 rounded-lg border-0 px-3 text-[13px] outline-none transition-colors duration-150 ${isLight ? "bg-white border border-stone-200 text-stone-600 hover:bg-stone-50" : "bg-[#36322f] text-text/60 hover:bg-stone-700"}`}
            >
              <Funnel size={16} />
              <span>{t("settings.models.filters.sort")}</span>
              <CaretDown size={12} className={isLight ? "text-stone-400" : "text-text/40"} />
            </MenuTrigger>
            <MenuPopup
              align="start"
              className={`min-w-48 rounded-lg p-1 ${isLight ? "border border-stone-200 bg-white text-stone-900" : "border-none bg-stone-700 text-stone-50"}`}
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
                    className={`flex cursor-pointer select-none items-center gap-1.5 rounded-md px-1.5 py-1 text-[13px] outline-none data-disabled:pointer-events-none data-disabled:opacity-50 ${isLight ? "text-stone-900 data-highlighted:bg-stone-100" : "text-stone-50 data-highlighted:bg-stone-600"}`}
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

        {/* Experimental Apple MLX Section — only while the opt-in is on */}
        {/* {mlxEnabled && mlxModels.length > 0 && (
          <div className="space-y-3">
            <div>
              <h2 className="text-sm font-medium text-text/60">
                {t("settings.models.mlxSectionTitle")}
              </h2>
              <p
                className={`mt-0.5 text-xs ${
                  mlxRuntimeStatus === null
                    ? "text-text/50"
                    : mlxRuntimeStatus.ready
                      ? "text-green-500"
                      : "text-orange-400"
                }`}
              >
                {mlxRuntimeStatus?.detail ??
                  t("settings.models.mlxRuntimeChecking")}
              </p>
            </div>
            {mlxModels.map((model: ModelInfo) => (
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
        )} */}

        {/* Available Models Section */}
        {availableModels.length > 0 && (
          <div className="space-y-3">
            <h2 className={`text-sm font-medium ${isLight ? "text-stone-600" : "text-text/60"}`}>
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
          <div className={`text-center py-8 ${isLight ? "text-stone-500" : "text-text/50"}`}>
            {t("settings.models.noModelsMatch")}
          </div>
        )}
      </div>
    </div>
  );
};
