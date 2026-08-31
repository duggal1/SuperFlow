import type { HTMLAttributes } from "react";
import { useIsLight } from "../../lib/utils/theme";

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
  orange: "bg-[#fb8442]/[0.11] text-[#fb8442]",
  green: "bg-[#22c55e]/[0.11] text-[#22c55e]",
  blue: "bg-[#3b82f6]/[0.11] text-[#3b82f6]",
  violet: "bg-[#8b5cf6]/[0.11] text-[#8b5cf6]",
  purple: "bg-[#a855f7]/[0.11] text-[#a855f7]",
  fuchsia: "bg-[#d946ef]/[0.11] text-[#d946ef]",
  cyan: "bg-[#06b6d4]/[0.11] text-[#06b6d4]",
  pink: "bg-[#ec4899]/[0.11] text-[#ec4899]",
  red: "bg-[#ef4444]/[0.11] text-[#ef4444]",
  rose: "bg-[#f43f5e]/[0.11] text-[#f43f5e]",
  yellow: "bg-[#eab308]/[0.11] text-[#eab308]",
  indigo: "bg-[#6366f1]/[0.11] text-[#6366f1]",
  sky: "bg-[#0ea5e9]/[0.11] text-[#0ea5e9]",
  neutral: "bg-neutral-500/[0.11] text-neutral-600 dark:text-neutral-400",
};

/* White-mode badge palette: a 15% tint of the 500 step with 600-step text,
   no border. Shape (radius, padding, tracking) is unchanged. */
const badgeVariantsLight: Record<BadgeVariant, string> = {
  orange: "bg-orange-500/15 text-orange-600",
  green: "bg-green-500/15 text-green-600",
  blue: "bg-blue-500/15 text-blue-600",
  violet: "bg-violet-500/15 text-violet-600",
  purple: "bg-purple-500/15 text-purple-600",
  fuchsia: "bg-fuchsia-500/15 text-fuchsia-600",
  cyan: "bg-cyan-500/15 text-cyan-600",
  pink: "bg-pink-500/15 text-pink-600",
  red: "bg-red-500/15 text-red-600",
  rose: "bg-rose-500/15 text-rose-600",
  yellow: "bg-yellow-500/15 text-yellow-600",
  indigo: "bg-indigo-500/15 text-indigo-600",
  sky: "bg-sky-500/15 text-sky-600",
  neutral: "bg-neutral-500/15 text-neutral-700",
};

export function Badge({
  variant = "neutral",
  className = "",
  children,
  ...props
}: BadgeProps) {
  const isLight = useIsLight();
  const variants = isLight ? badgeVariantsLight : badgeVariants;
  return (
    <div
      className={[
        "inline-flex items-center justify-center gap-1",
        "rounded-[3.5px] px-2 py-0.5",
        "font-medium text-[14px] leading-none tracking-[-0.09px]",
        variants[variant],
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
