import React from "react";

export type StatusPingTone = "green" | "orange" | "rose";

const toneClasses: Record<StatusPingTone, string> = {
  green: "bg-green-500",
  orange: "bg-orange-500",
  rose: "bg-rose-500",
};

interface StatusPingProps {
  tone: StatusPingTone;
  className?: string;
}

/** Live-status dot: 1.5px-radius square with a soft ping halo. */
export const StatusPing: React.FC<StatusPingProps> = ({
  tone,
  className = "",
}) => (
  <span className={`relative flex size-1.5 shrink-0 ${className}`}>
    <span
      className={`absolute inline-flex h-full w-full animate-ping rounded-[1.5px] opacity-75 ${toneClasses[tone]}`}
    />
    <span
      className={`relative inline-flex size-1.5 rounded-[1.5px] ${toneClasses[tone]}`}
    />
  </span>
);
