import React from "react";

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
  const baseClasses =
    "rounded-lg border px-3 py-2 text-sm font-normal tracking-tight text-stone-50 placeholder:text-stone-500 outline-none resize-y transition-[background-color,border-color] duration-150";

  const interactiveClasses = disabled
    ? "cursor-not-allowed opacity-50 border-stone-800"
    : "hover:border-stone-600 focus:border-blue-600 focus:bg-stone-950/40";

  const variantClasses = {
    default: "min-h-[100px] border-stone-700 bg-stone-900",
    compact: "min-h-[80px] border-stone-700 bg-stone-900 px-2 py-1",
    inset: "min-h-[112px] border-stone-800 bg-stone-850",
  } as const;

  return (
    <textarea
      className={`${baseClasses} ${variantClasses[variant]} ${interactiveClasses} ${className}`}
      disabled={disabled}
      {...props}
    />
  );
};
