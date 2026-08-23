import type { HTMLAttributes } from "react";

export type BadgeVariant =
  | "orange"
  | "green"
  | "blue"
  | "violet"
  | "purple"
  | "fuchsia"
  | "cyan"
  | "pink"
  | "red"
  | "rose"
  | "yellow"
  | "indigo"
  | "sky"
  | "neutral";

interface BadgeProps extends HTMLAttributes<HTMLDivElement> {
  variant?: BadgeVariant;
}

const badgeVariants: Record<BadgeVariant, string> = {
  orange: "bg-[#fb8442] text-white",
  green: "bg-[#22c55e] text-white",
  blue: "bg-[#3b82f6]/[0.11] text-[#3b82f6]",
  violet: "bg-[#8b5cf6]/[0.11] text-[#8b5cf6]",
  purple: "bg-[#a855f7]/[0.11] text-[#a855f7]",
  fuchsia: "bg-[#d946ef]/[0.11] text-[#d946ef]",
  cyan: "bg-[#06b6d4]/[0.11] text-[#06b6d4]",
  pink: "bg-[#ec4899]/[0.11] text-[#ec4899]",
  red: "bg-[#ef4444]/[0.11] text-[#ef4444]",
  rose: "bg-[#f43f5e] text-white",
  yellow: "bg-[#eab308]/[0.11] text-[#eab308]",
  indigo: "bg-[#6366f1]/[0.11] text-[#6366f1]",
  sky: "bg-[#0ea5e9]/[0.11] text-[#0ea5e9]",
  neutral: "bg-neutral-500/[0.11] text-neutral-600 dark:text-neutral-400",
};

export function Badge({
  variant = "neutral",
  className = "",
  children,
  ...props
}: BadgeProps) {
  return (
    <div
      className={[
        "inline-flex items-center justify-center gap-1",
        "rounded-[3.5px] px-2 py-0.5",
        "font-medium text-[14px] leading-none tracking-[-0.09px]",
        badgeVariants[variant],
        className,
      ]
        .filter(Boolean)
        .join(" ")}
      {...props}
    >
      {children}
    </div>
  );
}
