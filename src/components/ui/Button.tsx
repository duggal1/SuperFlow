import type { AnchorHTMLAttributes, ReactNode } from "react";

import { cn } from "../lib/utils";
import { useIsLight } from "../../lib/utils/theme";

type ButtonVariant = "primary" | "secondary" | "ghost";
type ButtonSize = "sm" | "md" | "lg" | "icon";

type ButtonProps = AnchorHTMLAttributes<HTMLAnchorElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
  disabled?: boolean;
  icon?: ReactNode;
};

const primaryShadow =
  "shadow-[0_0_0_1px_#2563eb26,inset_0_2px_#ffffff30,inset_0_-0.5px_2px_#00000065,0_2px_8px_#0000000d,0_3px_4px_#00000040] hover:shadow-[0_0_0_1px_#1d4ed833,inset_0_2px_#ffffff22,inset_0_-0.5px_2px_#00000080,0_2px_8px_#00000012,0_3px_4px_#0000004d]";

const primarySizes: Record<ButtonSize, string> = {
  sm: "h-7 px-3.5 py-0.5 text-[13px]",
  md: "h-7.5 px-4 py-0.5 text-[15px]",
  lg: "h-10 px-5.5 py-1 text-[15px]",
  icon: "size-8 p-0 text-[15px]",
};

const neutralSizes: Record<ButtonSize, string> = {
  sm: "h-7 px-4.5 py-1 text-[13px]",
  md: "h-7.5 px-5 py-1 text-[15px]",
  lg: "h-10 px-6.5 py-1 text-[15px]",
  icon: "size-8 p-0 text-[15px]",
};

export function Button({
  variant = "primary",
  size = "md",
  className,
  children,
  icon,
  disabled,
  ...props
}: ButtonProps) {
  const isLight = useIsLight();

  const variants: Record<ButtonVariant, string> = {
    primary:
      "border-blue-600 bg-blue-600 hover:border-blue-700 hover:bg-blue-700/[0.85]",

    secondary: isLight
      ? "border-stone-200/70 bg-white hover:border-stone-200/90 hover:bg-white"
      : "border-stone-700 bg-[#363230] hover:border-stone-700 hover:bg-[#363230]",

    ghost: isLight
      ? "border-transparent bg-transparent text-stone-900/70 hover:bg-transparent hover:text-stone-900"
      : "border-transparent bg-transparent text-stone-50/70 hover:bg-transparent hover:text-stone-50",
  };

  const textVariants: Record<ButtonVariant, string> = {
    primary: "text-white",
    secondary: isLight ? "text-stone-900" : "text-stone-50",
    ghost: "text-current",
  };

  const shadow =
    variant === "primary"
      ? primaryShadow
      : "!shadow-none hover:!shadow-none";

  const sizeClasses =
    variant === "primary"
      ? primarySizes[size]
      : neutralSizes[size];

  return (
    <a
      {...props}
      aria-disabled={disabled || undefined}
      className={cn(
        "group relative inline-flex cursor-pointer items-center justify-center whitespace-nowrap rounded-lg border no-underline",
        "transition-[background,border-color,color,opacity,box-shadow] duration-200 ease-out",
        variants[variant],
        shadow,
        sizeClasses,
        disabled && "pointer-events-none cursor-default opacity-50",
        className,
      )}
    >
      <span className="relative z-10 inline-flex min-w-0 items-center justify-center gap-1.5">
        {icon && (
          <span className="inline-flex shrink-0 items-center">
            {icon}
          </span>
        )}

        <span
          className={cn(
            "min-w-0 truncate text-[15px] font-[460] tracking-[0.15px]",
            textVariants[variant],
          )}
        >
          {children}
        </span>
      </span>
    </a>
  );
}