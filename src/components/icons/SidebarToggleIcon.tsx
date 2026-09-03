import React from "react";
import { motion } from "motion/react";

/* Sidebar toggle glyph — outer frame from public/sidebar-fill.svg,
   inner panel animates between expanded and collapsed. */
const SidebarToggleIcon = ({
  expanded,
  size = 16,
  className,
}: {
  expanded: boolean;
  size?: number;
  className?: string;
}) => {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      focusable="false"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={1.85}
      aria-hidden="true"
      className={`overflow-visible ${className ?? ""}`}
    >
      <rect x="3" y="5" width="18" height="14" rx="0.5" />
      <motion.rect
        x="5.25"
        y="7.25"
        width="7.25"
        height="9.5"
        rx="0.5"
        fill="currentColor"
        stroke="none"
        initial={false}
        animate={{
          opacity: expanded ? 0.95 : 0.72,
          scaleX: expanded ? 1 : 0.34,
        }}
        transition={{ duration: 0.2, ease: "easeOut" }}
        style={{
          transformBox: "fill-box",
          transformOrigin: "left center",
        }}
      />
    </svg>
  );
};

export default SidebarToggleIcon;
