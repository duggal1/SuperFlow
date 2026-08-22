import React from "react";

interface SettingsGroupProps {
  title?: string;
  description?: string;
  children: React.ReactNode;
}

export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  title,
  description,
  children,
}) => {
  return (
    <div className="space-y-2">
      {title && (
        <div className="px-4">
          <h2 className="text-xs font-medium text-stone-500 uppercase tracking-wide">
            {title}
          </h2>
          {description && (
            <p className="text-xs text-stone-500 mt-1">{description}</p>
          )}
        </div>
      )}
      <div className="rounded-[10px] bg-surface overflow-visible">
        <div className="divide-y divide-stone-700">{children}</div>
      </div>
    </div>
  );
};
