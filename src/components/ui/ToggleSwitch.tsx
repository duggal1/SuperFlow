import React from "react";
import { motion } from "motion/react";
import { SettingContainer } from "./SettingContainer";
import { useIsLight } from "../../lib/utils/theme";

interface ToggleSwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  isUpdating?: boolean;
  label: string;
  description: string;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  tooltipPosition?: "top" | "bottom";
  /** Render only the switch control — for cards that own their own layout
   * (label/description chrome handled by the caller). */
  bare?: boolean;
}

/* Custom motion toggle — spring thumb, blue-600 when on, stone when off. */
export const ToggleSwitch: React.FC<ToggleSwitchProps> = ({
  checked,
  onChange,
  disabled = false,
  isUpdating = false,
  label,
  description,
  descriptionMode = "tooltip",
  grouped = false,
  tooltipPosition = "top",
  bare = false,
}) => {
  const isLight = useIsLight();
  const control = (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled || isUpdating}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full border transition-colors duration-200 ${
        checked
          ? "border-blue-600 bg-blue-600"
          : isLight
            ? "border-stone-200 bg-white"
            : "border-stone-700 bg-stone-700"
      } ${disabled || isUpdating ? "cursor-not-allowed opacity-50" : "cursor-pointer"} ${!checked && !disabled && !isLight ? "hover:border-stone-600" : ""} ${!checked && !disabled && isLight ? "hover:border-stone-300" : ""}`}
    >
      <motion.span
        initial={false}
        animate={{ x: checked ? 22 : 2 }}
        transition={{
          type: "spring",
          stiffness: 500,
          damping: 32,
          mass: 0.6,
        }}
        className={`pointer-events-none absolute left-0 top-1/2 -mt-2.5 block size-5 rounded-full shadow-sm ${checked ? "bg-white" : isLight ? "bg-white border border-stone-200" : "bg-stone-50"}`}
      />
    </button>
  );

  if (bare) {
    return control;
  }

  return (
    <SettingContainer
      title={label}
      description={description}
      descriptionMode={descriptionMode}
      grouped={grouped}
      disabled={disabled}
      tooltipPosition={tooltipPosition}
    >
      {control}
    </SettingContainer>
  );
};
