import React from "react";
import {
  WarningCircle,
  Warning,
  Info,
  CheckCircle,
} from "@phosphor-icons/react";

type AlertVariant = "error" | "warning" | "info" | "success";

interface AlertProps {
  variant?: AlertVariant;
  /** When true, removes rounded corners for use inside containers */
  contained?: boolean;
  children: React.ReactNode;
  className?: string;
}

const variantStyles: Record<
  AlertVariant,
  { container: string; icon: string; text: string }
> = {
  error: {
    container: "bg-[#f43f5e]/[0.11]",
    icon: "text-[#f43f5e]",
    text: "text-[#f43f5e]",
  },
  warning: {
    container: "bg-[#fb8442]/[0.11]",
    icon: "text-[#fb8442]",
    text: "text-[#fb8442]",
  },
  info: {
    container: "bg-blue-500/10",
    icon: "text-blue-500",
    text: "text-blue-400",
  },
  success: {
    container: "bg-[#22c55e]/[0.11]",
    icon: "text-[#22c55e]",
    text: "text-[#22c55e]",
  },
};

const variantIcons: Record<AlertVariant, React.ElementType> = {
  error: WarningCircle,
  warning: Warning,
  info: Info,
  success: CheckCircle,
};

export const Alert: React.FC<AlertProps> = ({
  variant = "error",
  contained = false,
  children,
  className = "",
}) => {
  const styles = variantStyles[variant];
  const Icon = variantIcons[variant];

  return (
    <div
      className={`flex items-center gap-2 border-0 px-3 py-2.5 shadow-none ${styles.container} ${contained ? "" : "rounded-[7px]"} ${className}`}
    >
      {/* @ts-ignore phosphor icon type mismatch with React 19 */}
      <Icon className={`size-4 shrink-0 ${styles.icon}`} />
      <p className={`text-sm font-normal leading-5 ${styles.text}`}>
        {children}
      </p>
    </div>
  );
};
