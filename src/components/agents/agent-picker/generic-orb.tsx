"use client";

import * as React from "react";
import { motion, useReducedMotion } from "motion/react";
import { cn } from "@/lib/utils";

export type GenericAgentState = null | "thinking" | "listening" | "talking";

interface GenericOrbProps {
  colors?: [string, string];
  agentState?: GenericAgentState;
  className?: string;
}

/**
 * Generic custom Orb — pure CSS/SVG, no WebGL, no remote texture, no suspend.
 * Same props shape as orb.tsx Orb (colors, agentState, className) so it is a
 * drop-in replacement for blank-page isolation. orb.tsx is left untouched.
 */
export function GenericOrb({
  colors = ["#0B25E3", "#172EFF"],
  agentState = null,
  className,
}: GenericOrbProps) {
  const reduceMotion = useReducedMotion();
  const [c1, c2] = colors;

  const scale = agentState === "talking" ? 1.08 : agentState === "listening" ? 1.04 : 1;
  const duration = agentState === "talking" ? 1.2 : agentState === "listening" ? 1.6 : 2.8;

  return (
    <div className={cn("relative h-full w-full overflow-hidden", className)} aria-hidden="true">
      <motion.div
        className="absolute inset-0 rounded-full"
        style={{
          background: `radial-gradient(circle at 32% 28%, ${c2} 0%, ${c1} 42%, ${c1}CC 68%, transparent 78%), radial-gradient(circle at 68% 72%, ${c1} 0%, ${c2}66 55%, transparent 75%)`,
          filter: "blur(1px) saturate(1.15)",
          willChange: "transform, opacity",
        }}
        initial={false}
        animate={reduceMotion ? { scale: 1, opacity: 1 } : { scale, opacity: 1 }}
        transition={
          reduceMotion
            ? { duration: 0 }
            : { duration, repeat: Infinity, repeatType: "mirror", ease: [0.16, 1, 0.3, 1] }
        }
      />
      <motion.div
        className="absolute inset-[12%] rounded-full"
        style={{
          background: `conic-gradient(from 0deg, ${c1}, ${c2}, ${c1})`,
          opacity: 0.55,
          filter: "blur(6px)",
          willChange: "transform",
        }}
        initial={false}
        animate={reduceMotion ? { rotate: 0 } : { rotate: 360 }}
        transition={reduceMotion ? { duration: 0 } : { duration: 14, repeat: Infinity, ease: "linear" }}
      />
      <div
        className="absolute inset-[26%] rounded-full"
        style={{
          background: `radial-gradient(circle at 50% 38%, rgba(255,255,255,0.85) 0%, rgba(255,255,255,0.18) 42%, transparent 68%)`,
        }}
      />
    </div>
  );
}
