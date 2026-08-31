import React from "react";
import { useTranslation } from "react-i18next";
import { Button } from "./Button";
import { useIsLight } from "../../lib/utils/theme";

interface PathDisplayProps {
  path: string;
  onOpen: () => void;
  disabled?: boolean;
}

export const PathDisplay: React.FC<PathDisplayProps> = ({
  path,
  onOpen,
  disabled = false,
}) => {
  const { t } = useTranslation();
  const isLight = useIsLight();

  return (
    <div className="flex items-center gap-2">
      <div
        className={`flex-1 min-w-0 px-2 py-2 rounded-lg text-xs font-mono break-all select-text cursor-text ${
          isLight
            ? "bg-stone-100 border-0 text-stone-700"
            : "bg-stone-900 border border-stone-700 text-stone-100"
        }`}
      >
        {path}
      </div>
      <Button
        onClick={onOpen}
        variant="secondary"
        size="sm"
        disabled={disabled}
        className="px-3 py-2"
      >
        {t("common.open")}
      </Button>
    </div>
  );
};
