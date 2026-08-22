import React from "react";
import { motion } from "motion/react";
import { SettingContainer } from "./SettingContainer";

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
}) => {
  return (
    <SettingContainer
      title={label}
      description={description}
      descriptionMode={descriptionMode}
      grouped={grouped}
      disabled={disabled}
      tooltipPosition={tooltipPosition}
    >
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled || isUpdating}
        onClick={() => onChange(!checked)}
        className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors duration-200 ${
          checked ? "bg-blue-600" : "bg-stone-700"
        } ${disabled || isUpdating ? "cursor-not-allowed opacity-50" : "cursor-pointer"}`}
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
          className="pointer-events-none absolute left-0 top-1/2 -mt-2.5 block size-5 rounded-full bg-stone-50"
        />
      </button>
    </SettingContainer>
  );
};
