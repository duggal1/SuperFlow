import type { AnchorHTMLAttributes, ReactNode } from "react";
import { cn } from "../lib/utils";

/* Blue-only button system. Primary carries the brand accent; secondary and
   ghost are quiet neutral surfaces within the same geometry. */
type ButtonVariant = "primary" | "secondary" | "ghost";
type ButtonSize = "sm" | "md" | "lg" | "icon";

type ButtonProps = AnchorHTMLAttributes<HTMLAnchorElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
  disabled?: boolean;
  /** Optional leading icon; the button stretches horizontally to fit. */
  icon?: ReactNode;
};

const variants: Record<ButtonVariant, string> = {
  primary:
    "border-blue-600 bg-blue-600 hover:border-blue-700 hover:bg-blue-700/[0.85]",
  secondary:
    "border-stone-800 bg-surface hover:border-stone-700 hover:bg-surface-hover",
  ghost:
    "border-transparent bg-transparent text-stone-50/70 hover:bg-surface-hover hover:text-stone-50",
};

const shadows: Record<ButtonVariant, string> = {
  primary:
    "shadow-[0_0_0_1px_#2563eb26,inset_0_2px_#ffffff30,inset_0_-0.5px_2px_#00000065,0_2px_8px_#0000000d,0_3px_4px_#00000040] hover:shadow-[0_0_0_1px_#1d4ed833,inset_0_2px_#ffffff22,inset_0_-0.5px_2px_#00000080,0_2px_8px_#00000012,0_3px_4px_#0000004d]",
  secondary:
    "shadow-[0_0_0_1px_#29252426,inset_0_2px_#ffffff20,inset_0_-0.5px_2px_#00000065,0_2px_8px_#0000000d,0_3px_4px_#00000040] hover:shadow-[0_0_0_1px_#34302c33,inset_0_2px_#ffffff16,inset_0_-0.5px_2px_#00000080,0_2px_8px_#00000012,0_3px_4px_#0000004d]",
  ghost: "shadow-none",
};

const sizes: Record<ButtonSize, string> = {
  sm: "h-7 px-3 text-[13px]",
  md: "h-7.5 px-3.5 text-[15px]",
  lg: "h-10 px-5 text-[15px]",
  icon: "size-8 p-0 text-[15px]",
};

const overlayVariants: Record<ButtonVariant, string> = {
  primary: "bg-blue-950/15",
  secondary: "bg-stone-950/15",
  ghost: "bg-transparent",
};

const textVariants: Record<ButtonVariant, string> = {
  primary: "text-white",
  secondary: "text-stone-50",
  ghost: "text-current",
};

export function Button({
  variant = "primary",
  size = "md",
  className,
  children,
  icon,
  ...props
}: ButtonProps) {
  return (
    <a
      {...props}
      className={cn(
        "group relative inline-flex h-7.5 cursor-pointer items-center justify-center whitespace-nowrap rounded-[8px] px-3.5 py-1 no-underline",
        "transition-[background,border-color,box-shadow] duration-200 ease-out",
        variants[variant],
        shadows[variant],
        sizes[size],
        className,
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          "pointer-events-none absolute inset-0 rounded-[8px] opacity-0 transition-opacity duration-200 ease-out group-hover:opacity-100",
          overlayVariants[variant],
        )}
      />

      <span className="relative z-10 inline-flex min-w-0 items-center justify-center gap-1.5">
        {icon && (
          <span className="inline-flex shrink-0 items-center">{icon}</span>
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
