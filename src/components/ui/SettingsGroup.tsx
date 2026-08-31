import React from "react";
import { useIsLight } from "../../lib/utils/theme";

interface SettingsGroupProps {
  title?: string;
  titleClassName?: string;
  descriptionClassName?: string;
  headerClassName?: string;
  className?: string;
  description?: string;
  children: React.ReactNode;
}

export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  title,
  titleClassName,
  descriptionClassName,
  headerClassName = "",
  className = "",
  description,
  children,
}) => {
  const isLight = useIsLight();
  // Dark groups are borderless; white-mode groups get a softer stone border,
  // and dividers use a lighter stone so they read against the white cards.
  const cardBorder = isLight ? "border border-stone-200/60" : "";
  const divider = isLight ? "divide-stone-200/60" : "divide-stone-700";
  return (
    <div className={`space-y-2 ${className}`}>
      {title && (
        <div className={`px-4 ${headerClassName}`}>
          <h2
            className={
              titleClassName ??
              "text-xs font-medium text-stone-500 uppercase tracking-wide"
            }
          >
            {title}
          </h2>
          {description && (
            <p
              className={descriptionClassName ?? "mt-1 text-xs text-stone-500"}
            >
              {description}
            </p>
          )}
        </div>
      )}
      <div className={`rounded-[10px] bg-surface overflow-visible ${cardBorder}`}>
        <div className={`${divider}`}>{children}</div>
      </div>
    </div>
  );
};
