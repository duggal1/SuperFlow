import React from "react";
import { useIsLight } from "../../lib/utils/theme";

interface TextareaProps
  extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  variant?: "default" | "compact" | "inset";
}

/* Ultra-clean textarea — stone surface, hairline stone border, blue focus. */
export const Textarea: React.FC<TextareaProps> = ({
  className = "",
  variant = "default",
  disabled,
  ...props
}) => {
  const isLight = useIsLight();
  const baseClasses = isLight
    ? "rounded-lg border px-3 py-2 text-sm font-normal tracking-tight text-stone-900 placeholder:text-stone-400 outline-none resize-y transition-[border-color] duration-150"
    : "rounded-lg border px-3 py-2 text-sm font-normal tracking-tight text-stone-50 placeholder:text-stone-500 outline-none resize-y transition-[border-color] duration-150";

  const interactiveClasses = disabled
    ? "cursor-not-allowed opacity-50 border-stone-200"
    : isLight
      ? "hover:border-stone-300 focus:border-blue-600 focus:bg-stone-100"
      : "hover:border-stone-600 focus:border-blue-600 focus:bg-stone-900";

  const variantClasses = isLight
    ? {
        default: "min-h-[100px] border-stone-200/70 bg-stone-100",
        compact: "min-h-[80px] border-stone-200/70 bg-stone-100 px-2 py-1",
        inset: "min-h-[112px] border-stone-200 bg-stone-100",
      }
    : {
        default: "min-h-[100px] border-stone-700 bg-stone-900",
        compact: "min-h-[80px] border-stone-700 bg-stone-900 px-2 py-1",
        inset: "min-h-[112px] border-stone-800 bg-stone-850",
      };

  return (
    <textarea
      className={`${baseClasses} ${variantClasses[variant]} ${interactiveClasses} ${className}`}
      disabled={disabled}
      {...props}
    />
  );
};
