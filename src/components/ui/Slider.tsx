import React from "react";
import { AnimatePresence, motion } from "motion/react";
import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { SettingContainer } from "./SettingContainer";
import { ResetButton } from "./ResetButton";

interface SliderProps {
  value: number;
  onChange: (value: number) => void;
  onCommit?: (value: number) => void;
  min: number;
  max: number;
  step?: number;
  disabled?: boolean;
  label: string;
  description: string;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  showValue?: boolean;
  formatValue?: (value: number) => string;
  onReset?: () => void;
  isResetting?: boolean;
  appearance?: "default" | "square";
}

/* Ultra-clean slider: base-ui drag physics, hairline stone track, blue-600
   fill, spring-animated readout. Grab cursor while interacting. */
export const Slider: React.FC<SliderProps> = ({
  value,
  onChange,
  onCommit,
  min,
  max,
  step = 0.01,
  disabled = false,
  label,
  description,
  descriptionMode = "tooltip",
  grouped = false,
  showValue = true,
  formatValue = (v) => v.toFixed(2),
  onReset,
  isResetting = false,
  appearance = "default",
}) => {
  const readValue = (next: number | number[]) =>
    Array.isArray(next) ? next[0] : next;

  return (
    <SettingContainer
      title={label}
      description={description}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="horizontal"
      disabled={disabled}
    >
      <div className="flex w-full items-center gap-2.5">
        <SliderPrimitive.Root
          value={value}
          min={min}
          max={max}
          step={step}
          disabled={disabled}
          thumbAlignment="edge"
          onValueChange={(next) => {
            const nextValue = readValue(next);
            if (typeof nextValue === "number") onChange(nextValue);
          }}
          onValueCommitted={(next) => {
            const nextValue = readValue(next);
            if (typeof nextValue === "number") onCommit?.(nextValue);
          }}
          className="data-[orientation=horizontal]:w-full"
        >
          <SliderPrimitive.Control className="group/slider flex h-6 w-full cursor-grab touch-none select-none items-center active:cursor-grabbing data-disabled:pointer-events-none data-disabled:opacity-50">
            <SliderPrimitive.Track
              className={`relative w-full grow select-none overflow-hidden bg-stone-700 ${
                appearance === "square"
                  ? "h-1.5 rounded-[2px]"
                  : "h-1 rounded-full"
              }`}
            >
              <SliderPrimitive.Indicator
                className={`h-full bg-blue-600 ${appearance === "square" ? "rounded-[2px]" : "rounded-full"}`}
              />
            </SliderPrimitive.Track>
            <SliderPrimitive.Thumb
              className={`block size-4 shrink-0 select-none border border-stone-500 bg-stone-50 outline-none transition-[scale,box-shadow] duration-150 ease-out has-focus-visible:ring-[3px] has-focus-visible:ring-blue-600/40 data-dragging:scale-110 ${appearance === "square" ? "rounded-[3px]" : "rounded-full"}`}
            />
          </SliderPrimitive.Control>
        </SliderPrimitive.Root>

        {showValue && (
          <span className="w-12 shrink-0 text-end text-sm font-normal tracking-tight text-stone-400 tabular-nums">
            <AnimatePresence mode="popLayout" initial={false}>
              <motion.span
                key={formatValue(value)}
                initial={{ opacity: 0.35, y: 2 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -2 }}
                transition={{ duration: 0.12, ease: "easeOut" }}
                className="inline-block"
              >
                {formatValue(value)}
              </motion.span>
            </AnimatePresence>
          </span>
        )}
        {onReset && (
          <ResetButton onClick={onReset} disabled={disabled || isResetting} />
        )}
      </div>
    </SettingContainer>
  );
};
