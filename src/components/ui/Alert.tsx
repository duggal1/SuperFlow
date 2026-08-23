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
    container: "bg-[#f43f5e]",
    icon: "text-white",
    text: "text-white",
  },
  warning: {
    container: "bg-[#fb8442]",
    icon: "text-white",
    text: "text-white",
  },
  info: {
    container: "bg-blue-500/10",
    icon: "text-blue-500",
    text: "text-blue-400",
  },
  success: {
    container: "bg-[#22c55e]",
    icon: "text-white",
    text: "text-white",
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
      <Icon className={`size-4 shrink-0 ${styles.icon}`} />
      <p className={`text-sm font-normal leading-5 ${styles.text}`}>
        {children}
      </p>
    </div>
  );
};
