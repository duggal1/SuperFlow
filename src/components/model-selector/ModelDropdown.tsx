import React from "react";
import { useTranslation } from "react-i18next";
import type { ModelInfo } from "@/bindings";
import { getTranslatedModelName } from "../../lib/utils/modelTranslation";
import { Badge } from "../ui/Badge";

interface ModelDropdownProps {
  models: ModelInfo[];
  currentModelId: string;
  onModelSelect: (modelId: string) => void;
}

const ModelDropdown: React.FC<ModelDropdownProps> = ({
  models,
  currentModelId,
  onModelSelect,
}) => {
  const { t } = useTranslation();
  const downloadedModels = models.filter((m) => m.is_downloaded);

  const handleModelClick = (modelId: string) => {
    onModelSelect(modelId);
  };

  return (
    <div className="absolute bottom-full start-0 z-50 mb-2 max-h-[60vh] w-64 space-y-1 overflow-y-auto rounded-[8px] bg-stone-850 p-1.5">
      {downloadedModels.length > 0 ? (
        <div className="space-y-1">
          {downloadedModels.map((model) => (
            <div
              key={model.id}
              onClick={() => handleModelClick(model.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleModelClick(model.id);
                }
              }}
              tabIndex={0}
              role="button"
              className="w-full cursor-pointer rounded-[6px] bg-stone-800 px-3 py-2 text-start transition-colors hover:bg-stone-700 focus:outline-none"
            >
              <div className="flex items-center justify-between">
                <div className="min-w-0">
                  <div className="truncate text-sm text-text/80">
                    {getTranslatedModelName(model, t)}
                    {model.is_custom && (
                      <span className="ms-1.5 text-[10px] font-medium text-text/40 uppercase">
                        {t("modelSelector.custom")}
                      </span>
                    )}
                    {model.supports_streaming && (
                      <span className="ms-1.5 text-[10px] font-medium text-blue-500 uppercase">
                        {t("modelSelector.streaming")}
                      </span>
                    )}
                  </div>
                </div>
                {currentModelId === model.id && (
                  <Badge
                    variant="green"
                    className="ms-3 shrink-0 px-1.5 py-1 text-[10px]"
                  >
                    {t("modelSelector.active")}
                  </Badge>
                )}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="px-3 py-2 text-sm text-text/60">
          {t("modelSelector.noModelsAvailable")}
        </div>
      )}
    </div>
  );
};

export default ModelDropdown;
