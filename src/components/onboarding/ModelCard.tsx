import React from "react";
import { useTranslation } from "react-i18next";
import {
  Equalizer,
  GlobeSimple,
  HardDrives,
  Translate,
} from "@phosphor-icons/react";
import type { ModelInfo } from "@/bindings";
import { formatModelSize } from "../../lib/utils/format";
import {
  getTranslatedModelDescription,
  getTranslatedModelName,
} from "../../lib/utils/modelTranslation";
import {
  getLanguageLabel,
  getUniqueCapabilityLanguages,
} from "../../lib/constants/languages";
import { Badge, type BadgeVariant } from "../ui/Badge";
import { Button } from "../ui/Button";
import { useIsLight } from "@/lib/utils/theme";

// Accuracy tier badge — green at/above 85%, orange from 70–85%, rose below.
// No score means no badge: unknown is not the same as inaccurate.
const getAccuracyBadge = (
  model: ModelInfo,
): { percent: number; variant: BadgeVariant } | null => {
  const accuracy = Math.min(1, Math.max(0, model.accuracy_score));
  if (accuracy <= 0) return null;
  const percent = Math.round(accuracy * 100);
  if (percent >= 85) return { percent, variant: "green" };
  if (percent >= 70) return { percent, variant: "orange" };
  return { percent, variant: "rose" };
};

// Get display text for model's language support
const getLanguageDisplayText = (
  supportedLanguages: string[],
  t: (key: string, options?: Record<string, unknown>) => string,
): string => {
  const capabilityLanguages = getUniqueCapabilityLanguages(supportedLanguages);
  if (capabilityLanguages.length === 1) {
    const langCode = capabilityLanguages[0];
    const langName = getLanguageLabel(langCode) || langCode;
    return t("modelSelector.capabilities.languageOnly", { language: langName });
  }
  return t("modelSelector.capabilities.languageCount", {
    total: capabilityLanguages.length,
  });
};

// Legacy = a blob (Url-sourced) .bin/ONNX model, kept runnable but no longer the
// advertised download (catalog GGUFs supersede it).
export const isLegacySource = (model: ModelInfo): boolean =>
  typeof model.source === "object" && "Url" in model.source;

export type ModelCardStatus =
  | "downloadable"
  | "downloading"
  | "verifying"
  | "extracting"
  | "switching"
  | "active"
  | "available";

interface ModelCardProps {
  model: ModelInfo;
  variant?: "default" | "featured";
  status?: ModelCardStatus;
  disabled?: boolean;
  className?: string;
  onSelect: (modelId: string) => void;
  onDownload?: (modelId: string) => void;
  onDelete?: (modelId: string) => void;
  onCancel?: (modelId: string) => void;
  downloadProgress?: number;
  downloadSpeed?: number; // MB/s
  showRecommended?: boolean;
  /** Overrides the catalog flag with a spec-computed recommendation. */
  recommended?: boolean;
}

/** Stable id-prefix check for seeded experimental MLX descriptors. */
export const isMlxModel = (model: ModelInfo): boolean =>
  model.id.startsWith("superflow-mlx/");

const ModelCard: React.FC<ModelCardProps> = ({
  model,
  variant = "default",
  status = "downloadable",
  disabled = false,
  className = "",
  onSelect,
  onDownload,
  onDelete,
  onCancel,
  downloadProgress,
  downloadSpeed,
  showRecommended = true,
  recommended,
}) => {
  const { t } = useTranslation();
  const isLight = useIsLight();
  const isFeatured = variant === "featured";
  // The active model is already loaded — re-selecting it just reloads it for no
  // gain, so it is deliberately not clickable.
  const isClickable = status === "available" || status === "downloadable";

  // Get translated model name and description
  const displayName = getTranslatedModelName(model, t);
  const displayDescription = getTranslatedModelDescription(model, t);
  const showModelSize =
    status === "downloadable" || status === "available" || status === "active";
  const formattedModelSize = formatModelSize(Number(model.size_mb));
  const capabilityLanguages = getUniqueCapabilityLanguages(
    model.supported_languages,
  );
  const accuracyBadge = getAccuracyBadge(model);

  const baseClasses = `group flex flex-col rounded-[10px] border bg-surface px-4 py-3 gap-2 text-left transition-colors duration-200 ${isLight ? "border-stone-200/80" : "border-transparent"}`;

  const getVariantClasses = () => {
    if (status === "active") {
      return "border-blue-600/60 bg-blue-600/10";
    }
    if (isFeatured) {
      return "border-blue-600/30";
    }
    return "";
  };

  const getInteractiveClasses = () => {
    if (!isClickable) return "";
    if (disabled) return "opacity-50 cursor-not-allowed";
    return isLight
      ? "cursor-pointer hover:border-stone-300 hover:bg-stone-50 active:scale-[0.99]"
      : "cursor-pointer hover:border-stone-800 hover:bg-stone-850 active:scale-[0.99]";
  };

  const handleClick = () => {
    if (!isClickable || disabled) return;
    if (status === "downloadable" && onDownload) {
      onDownload(model.id);
    } else {
      onSelect(model.id);
    }
  };

  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete?.(model.id);
  };

  return (
    <div
      onClick={handleClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" && isClickable) handleClick();
      }}
      role={isClickable ? "button" : undefined}
      tabIndex={isClickable ? 0 : undefined}
      className={[
        baseClasses,
        getVariantClasses(),
        getInteractiveClasses(),
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      {/* Top section: name/description + score bars */}
      <div className="flex justify-between items-center w-full">
        <div className="flex flex-col items-start flex-1 min-w-0">
          <div className="flex items-center gap-3 flex-wrap">
            <h3
              className={`text-base font-semibold ${isLight ? "text-stone-900" : "text-text"} ${isClickable ? "group-hover:text-blue-500" : ""} transition-colors`}
            >
              {displayName}
            </h3>
            {showRecommended && (recommended ?? model.is_recommended) && (
              <Badge variant="fuchsia">{t("onboarding.recommended")}</Badge>
            )}
            {isMlxModel(model) && (
              <Badge variant="fuchsia">{t("settings.models.mlxBadge")}</Badge>
            )}
            {status === "active" && (
              <Badge variant="blue">{t("modelSelector.active")}</Badge>
            )}
            {accuracyBadge && (
              <Badge
                variant={accuracyBadge.variant}
                className="px-2.5 text-[10px] leading-none"
              >
                {t("onboarding.modelCard.accuracyPercent", {
                  percent: accuracyBadge.percent,
                })}
              </Badge>
            )}
            {model.is_custom && (
              <Badge variant="neutral">{t("modelSelector.custom")}</Badge>
            )}
            {isLegacySource(model) && (
              <Badge variant="neutral">{t("modelSelector.legacy")}</Badge>
            )}
            {status === "switching" && (
              <Badge variant="neutral">{t("modelSelector.switching")}</Badge>
            )}
          </div>
          {!isMlxModel(model) && (
            <p
              className={`text-sm leading-relaxed ${isLight ? "text-stone-500" : "text-text/60"}`}
            >
              {displayDescription}
            </p>
          )}
        </div>
        {(model.accuracy_score > 0 || model.speed_score > 0) && (
          <div className="hidden sm:flex items-center ms-4">
            <div className="space-y-1">
              {model.accuracy_score > 0 && (
                <div className="flex items-center gap-2">
                  <p
                    className={`text-xs w-24 text-end ${isLight ? "text-stone-500" : "text-text/60"}`}
                  >
                    {t("onboarding.modelCard.accuracy")}
                  </p>
                  <div
                    className={`w-16 h-1.5 rounded-full overflow-hidden ${isLight ? "bg-stone-200" : "bg-stone-800"}`}
                  >
                    <div
                      className="h-full bg-blue-600 rounded-full"
                      style={{ width: `${model.accuracy_score * 100}%` }}
                    />
                  </div>
                </div>
              )}
              {model.speed_score > 0 && (
                <div className="flex items-center gap-2">
                  <p
                    className={`text-xs w-24 text-end ${isLight ? "text-stone-500" : "text-text/60"}`}
                  >
                    {t("onboarding.modelCard.speed")}
                  </p>
                  <div
                    className={`w-16 h-1.5 rounded-full overflow-hidden ${isLight ? "bg-stone-200" : "bg-stone-800"}`}
                  >
                    <div
                      className="h-full bg-blue-600 rounded-full"
                      style={{ width: `${model.speed_score * 100}%` }}
                    />
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      <hr
        className={`w-full ${isLight ? "border-stone-200" : "border-stone-700"}`}
      />

      {/* Bottom row: tags + action buttons (full width) */}
      <div className="flex items-center gap-3 w-full -mb-0.5 mt-0.5 h-5">
        {capabilityLanguages.length > 0 && (
          <div
            className={`flex items-center gap-1 text-xs ${isLight ? "text-stone-500" : "text-text/50"}`}
            title={
              capabilityLanguages.length === 1
                ? t("modelSelector.capabilities.singleLanguage")
                : t("modelSelector.capabilities.languageSelection")
            }
          >
            <GlobeSimple className="w-3.5 h-3.5" />
            <span>{getLanguageDisplayText(model.supported_languages, t)}</span>
          </div>
        )}
        {model.supports_streaming && (
          <div
            className={`flex items-center gap-1 text-xs ${isLight ? "text-stone-500" : "text-text/50"}`}
            title={t("modelSelector.capabilities.streaming")}
          >
            <Equalizer className="w-3.5 h-3.5" />
            <span>{t("modelSelector.streaming")}</span>
          </div>
        )}
        {model.supports_translation && (
          <div
            className={`flex items-center gap-1 text-xs ${isLight ? "text-stone-500" : "text-text/50"}`}
            title={t("modelSelector.capabilities.translation")}
          >
            <Translate className="w-3.5 h-3.5" />
            <span>{t("modelSelector.capabilities.translate")}</span>
          </div>
        )}
        {showModelSize && (
          <span
            className={`flex items-center gap-1.5 ms-auto text-xs ${isLight ? "text-stone-500" : "text-text/50"}`}
          >
            <HardDrives className="w-3.5 h-3.5" />
            <span>{formattedModelSize}</span>
          </span>
        )}
        {onDelete && (status === "available" || status === "active") && (
          <Button
            variant="ghost"
            size="sm"
            onClick={handleDelete}
            title={t("modelSelector.deleteModel", { modelName: displayName })}
            className={`${isLight ? "text-stone-600 hover:bg-rose-400 hover:text-white" : "text-stone-100 hover:bg-rose-700 hover:text-white"}`}
          >
            {t("common.delete")}
          </Button>
        )}
      </div>

      {/* Download/extract progress */}
      {status === "downloading" && downloadProgress !== undefined && (
        <div className="w-full mt-3">
          <div
            className={`w-full h-1.5 rounded-full overflow-hidden ${isLight ? "bg-stone-200" : "bg-stone-800"}`}
          >
            <div
              className="h-full bg-blue-600 rounded-full transition-all duration-300"
              style={{ width: `${downloadProgress}%` }}
            />
          </div>
          <div className="flex items-center justify-between text-xs mt-1">
            <span className={isLight ? "text-stone-500" : "text-text/50"}>
              {t("modelSelector.downloading", {
                percentage: Math.round(downloadProgress),
              })}
            </span>
            <div className="flex items-center gap-2">
              {downloadSpeed !== undefined && downloadSpeed > 0 && (
                <span
                  className={`tabular-nums ${isLight ? "text-stone-500" : "text-text/50"}`}
                >
                  {t("modelSelector.downloadSpeed", {
                    speed: downloadSpeed.toFixed(1),
                  })}
                </span>
              )}
              {onCancel && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    onCancel(model.id);
                  }}
                  aria-label={t("modelSelector.cancelDownload")}
                >
                  {t("modelSelector.cancel")}
                </Button>
              )}
            </div>
          </div>
        </div>
      )}
      {status === "verifying" && (
        <div className="w-full mt-3">
          <div
            className={`w-full h-1.5 rounded-full overflow-hidden ${isLight ? "bg-stone-200" : "bg-stone-800"}`}
          >
            <div className="h-full bg-blue-600 rounded-full animate-pulse w-full" />
          </div>
          <p
            className={`text-xs mt-1 ${isLight ? "text-stone-500" : "text-text/50"}`}
          >
            {t("modelSelector.verifyingGeneric")}
          </p>
        </div>
      )}
      {status === "extracting" && (
        <div className="w-full mt-3">
          <div
            className={`w-full h-1.5 rounded-full overflow-hidden ${isLight ? "bg-stone-200" : "bg-stone-800"}`}
          >
            <div className="h-full bg-blue-600 rounded-full animate-pulse w-full" />
          </div>
          <p
            className={`text-xs mt-1 ${isLight ? "text-stone-500" : "text-text/50"}`}
          >
            {t("modelSelector.extractingGeneric")}
          </p>
        </div>
      )}
    </div>
  );
};

export default ModelCard;
