import React from "react";
import { useIsLight } from "../../lib/utils/theme";

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  variant?: "default" | "compact";
}

/* Ultra-clean input — stone surface, hairline stone border, blue focus. */
export const Input: React.FC<InputProps> = ({
  className = "",
  variant = "default",
  disabled,
  ...props
}) => {
  const isLight = useIsLight();
  const baseClasses = isLight
    ? "rounded-lg border border-stone-200/70 bg-stone-100 text-sm font-normal tracking-tight text-stone-900 placeholder:text-stone-400 outline-none transition-[border-color] duration-150"
    : "rounded-lg border border-stone-700 bg-stone-900 text-sm font-normal tracking-tight text-stone-50 placeholder:text-stone-500 outline-none transition-[border-color] duration-150";

  const interactiveClasses = disabled
    ? "cursor-not-allowed opacity-50 border-stone-200"
    : isLight
      ? "hover:border-stone-300 focus:border-blue-600 focus:bg-stone-100"
      : "hover:border-stone-600 focus:border-blue-600 focus:bg-stone-900";

  const variantClasses = {
    default: "px-3 py-2",
    compact: "px-2 py-1",
  } as const;

  return (
    <input
      className={`${baseClasses} ${variantClasses[variant]} ${interactiveClasses} ${className}`}
      disabled={disabled}
      {...props}
    />
  );
};
