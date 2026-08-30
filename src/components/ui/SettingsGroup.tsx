import React from "react";

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
      <div className="overflow-visible rounded-lg border border-stone-700 bg-surface">
        <div className="divide-y divide-stone-700">{children}</div>
      </div>
    </div>
  );
};
