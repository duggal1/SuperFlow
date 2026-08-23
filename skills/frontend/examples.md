# ⛔ EXTRACT PRINCIPLES, NOT CODE — Ultra-Clean Landing Page Architecture (`SKILL.md`)

> **9/10 guard:** This file is a benchmark library, not a template. Do not copy-paste. Extract hierarchy, rails, and spacing principles, then rebuild with the brief's tokens. If you paste, you fail the taste test.

An extraction-based design system and component skill file for engineering ultra-clean, high-taste, minimalist landing pages. Extracted directly from production designs (Polar Signals, Vessa, Kira Learning, AccessGrid).

NOTE : These codebases serve strictly as reference benchmarks and must never be copy-pasted directly into production. High-end UI design ultimately depends on the user’s custom hierarchy, unique software theme, visual consistency, and deliberate UX judgment. Treat these examples not as static templates, but as structural blueprints. Extract the core architectural principles: restrained typography rules, hairline border containment, fluid grid rhythms, and strict anti-patterns. By understanding what to avoid and mastering how clean layouts truly function, you can leverage these foundational patterns to engineer bespoke, ultra-minimalist landing pages tailored precisely to any specific product or client requirement without compromise.

# Design Formula

For SaaS, clean design is a formula: deliberately stripping away bad patterns—heavy shadows, bloated fonts, harsh borders(colors), and generic inconsistency on UI while infusing bespoke themes with elite taste. UI is the structural foundation, but UX is paramount. True cleanliness demands obsessive craftsmanship, delivering deliberate emotional clarity and a frictionless user experience.

## A true exceptional, clean design come from real taste real judgment and ultimately following user vision and user instruction on design with real estate and judgment on it too to make the design extremely clean, but non-but non-lifeless AI can make the design extremely clean, but extremely lifeless and unforgettable this is what we need to stop. We need to make design extremely clean, but extremely memorable to with real taste judgment.

A truly exceptional, clean design comes from real taste, real judgment, and ultimately following the user's vision and instructions while applying your own taste and judgment to make the design extremely clean.

The problem is that AI can make a design clean "enough", but also extremely lifeless and forgettable. **That is exactly what we need to stop.**

We need to make designs **extremely clean, but also extremely memorable**, with real taste and real judgment behind every decision.

## 1. System Philosophy & Anti-Patterns

### Strict Design Directives

```
┌───────────────────────────────┬────────────────────────────────────────────────────────┐
│ DIRECTIVE                     │ ENFORCEMENT RULE                                       │
├───────────────────────────────┼────────────────────────────────────────────────────────┤
│ Max Font Weight               │ <= 500 (Medium). Strictly zero bold (700/800/900).      │
│ Elevation & Shadows           │ Hairline 1px borders, subtle rings. No blurry shadows. │
│ Color Gradients               │ Monochromatic depth only. No saturated rainbow fades.  │
│ Grid Structural Anchors       │ Repeating dashed hair lines (4px/3px or 9px/6px).      │
│ Interactive Micro-States      │ Letter-stagger hovers, smooth scale-98 active taps.    │
│ Spacing Rhythm                │ Fluid clamps (`clamp(1.5rem, 3vw, 2.5rem)`).           │
└───────────────────────────────┴────────────────────────────────────────────────────────┘
```

### Strict Anti-Patterns

- **DO NOT** use `font-bold`, `font-extrabold`, or `font-black`. Headlines achieve hierarchy through sizing (`text-3xl` to `text-6xl`), tight leading (`leading-[1.05]`), and negative tracking (`tracking-[-0.035em]`), never excessive stroke weight.
- **DO NOT** use `shadow-2xl` or blurry black drop shadows. Use dual-layer hairline containment: `box-shadow: 0 1px 1px 0 rgba(38,38,43,0.08), 0 0 0 1px rgba(38,38,43,0.04)`.
- **DO NOT** use rainbow glow gradients. Gradients are reserved for hairline progress fills (`linear-gradient(90deg, #566CF7 0%, #A9BBFF 100%)`) or dither noise fields.
- **DO NOT** use bloated borders. All separators are $1\text{px}$ solid or SVG-based dashed hairlines.

---

## 2. CSS Theme Engine & Variables

```css
/* theme-tokens.css */
:root {
  /* Surface Archetype A: Swiss Paper & Ink (Editorial Minimal) */
  --vessa-paper: #eef0f3;
  --vessa-paper-2: #e5e8ec;
  --vessa-surface: #f5f6f8;
  --vessa-surface-bright: #fcfdfe;
  --vessa-ink: #101216;
  --vessa-ink-soft: #3f434b;
  --vessa-ink-faint: #676d78;
  --vessa-hairline: #d3d7de;
  --vessa-accent: #1f2de6;
  --vessa-accent-ink: #ffffff;
  --m-border: #a2a3a5;

  /* Surface Archetype B: Technical Modern (Polar White / Carbon) */
  --polar-bg: #ffffff;
  --polar-card: #f9fafb;
  --polar-card-highlight: #ebf4ff;
  --polar-ink: #09090b;
  --polar-muted: #71717a;
  --polar-border: #e3e4e9;
  --polar-accent: #566cf7;

  /* Surface Archetype C: Warm Academy (Kira Warm Vanilla) */
  --warm-bg: #fdfdfd;
  --warm-surface: #fffdf0;
  --warm-card: #f5f5f4;
  --warm-ink: #1a1a1a;
  --warm-muted: #6b6761;
  --warm-border: #ceccca;
  --warm-accent: #56eaaf;
  --warm-accent-purple: #ac99ff;

  /* Surface Archetype D: Slate Industrial (AccessGrid Cloud) */
  --ag-gray-50: #f7f7f8;
  --ag-gray-100: #eeeef0;
  --ag-gray-200: #d9d9de;
  --ag-gray-300: #b8b9c1;
  --ag-gray-600: #5e5f6b;
  --ag-gray-700: #474853;
  --ag-gray-950: #272c30;
  --ag-brand-50: #eef5ff;
  --ag-brand-600: #005bd3;
  --ag-brand-700: #004bb0;

  /* Hairline Dashed Rails (CSS Repeating Linear Gradients) */
  --dash-on: 9px;
  --dash-off: 6px;
  --dash-h: repeating-linear-gradient(
    to right,
    var(--m-border) 0 var(--dash-on),
    transparent var(--dash-on) calc(var(--dash-on) + var(--dash-off))
  );
  --dash-v: repeating-linear-gradient(
    to bottom,
    var(--m-border) 0 var(--dash-on),
    transparent var(--dash-on) calc(var(--dash-on) + var(--dash-off))
  );

  /* Fluid Column Matrices */
  --col-wide: min(86rem, calc(100vw - 1.5rem));
  --col-main: min(78rem, calc(100vw - 3rem));
  --col-narrow: min(52rem, calc(100vw - 4rem));

  /* Motion Curves */
  --ease-drift: cubic-bezier(0.3, 0.9, 0.1, 1);
  --ease-punch: cubic-bezier(0.7, 0, 0.16, 1);
  --ease-spring: cubic-bezier(0.33, 1, 0.68, 1);
}

/* Base resets for extreme cleanliness */
html {
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
}

/* Universal dashed hairline rails */
.rail-border-t {
  position: relative;
}
.rail-border-t::before {
  content: "";
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 1px;
  background: var(--dash-h);
}

.rail-border-b {
  position: relative;
}
.rail-border-b::after {
  content: "";
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 1px;
  background: var(--dash-h);
}

.rail-border-l {
  position: relative;
}
.rail-border-l::before {
  content: "";
  position: absolute;
  top: 0;
  bottom: 0;
  left: 0;
  width: 1px;
  background: var(--dash-v);
}

/* Linear gradient marquee mask */
.mask-marquee-edges {
  mask-image: linear-gradient(
    to right,
    transparent 0%,
    black 15%,
    black 85%,
    transparent 100%
  );
  -webkit-mask-image: linear-gradient(
    to right,
    transparent 0%,
    black 15%,
    black 85%,
    transparent 100%
  );
}

/* Marquee keyframe animation */
@keyframes marquee-scroll {
  0% {
    transform: translateX(0%);
  }
  100% {
    transform: translateX(-50%);
  }
}

.animate-marquee-infinite {
  display: flex;
  width: max-content;
  animation: marquee-scroll 35s linear infinite;
}

.animate-marquee-infinite:hover {
  animation-play-state: paused;
}
```

---

## 3. Tailwind CSS Configuration Preset

```javascript
// tailwind.config.js
/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/**/*.{js,ts,jsx,tsx,mdx}"],
  theme: {
    extend: {
      colors: {
        paper: {
          DEFAULT: "var(--vessa-paper, #eef0f3)",
          secondary: "var(--vessa-paper-2, #e5e8ec)",
          surface: "var(--vessa-surface, #f5f6f8)",
          bright: "var(--vessa-surface-bright, #fcfdfe)",
        },
        ink: {
          DEFAULT: "var(--vessa-ink, #101216)",
          soft: "var(--vessa-ink-soft, #3f434b)",
          faint: "var(--vessa-ink-faint, #676d78)",
        },
        brand: {
          50: "var(--ag-brand-50, #EEF5FF)",
          100: "#D9E8FF",
          600: "var(--ag-brand-600, #005BD3)",
          700: "var(--ag-brand-700, #004BB0)",
          accent: "var(--vessa-accent, #1F2DE6)",
        },
        hairline: "var(--vessa-hairline, #D3D7DE)",
      },
      fontFamily: {
        sans: [
          "var(--font-sans)",
          "Switzer",
          "-apple-system",
          "BlinkMacSystemFont",
          "sans-serif",
        ],
        serif: ["var(--font-serif)", "Recife Text", "Georgia", "serif"],
        mono: ["var(--font-mono)", "Source Code Pro", "monospace"],
      },
      letterSpacing: {
        tightest: "-0.045em",
        tighter: "-0.035em",
        tight: "-0.02em",
        snug: "-0.012em",
      },
      lineHeight: {
        tighter: "1.02",
        tight: "1.15",
        normal: "1.55",
      },
      boxShadow: {
        hairline:
          "0 1px 1px 0 rgba(38,38,43,0.08), 0 0 0 1px rgba(38,38,43,0.04)",
        floating:
          "0 1px 1px 0 rgba(38,38,43,0.10), 0 0 0 1px rgba(38,38,43,0.04), 0 2px 12px -4px rgba(38,38,43,0.16)",
        popover:
          "0 24px 40px -20px rgba(38,38,43,0.30), 0 10px 24px 0 rgba(38,38,43,0.06), 0 1px 1px 0 rgba(38,38,43,0.16), 0 0 0 1px rgba(38,38,43,0.05)",
      },
    },
  },
  plugins: [],
};
```

---

## 4. Visual Primitives & Background Engines

### Dynamic Dither & Micro-Canvas Engine

```tsx
// components/primitives/DitherCanvas.tsx
"use client";

import React, { useRef, useEffect } from "react";

interface DitherCanvasProps {
  accent?: string;
  opacity?: number;
  bare?: boolean;
  className?: string;
}

export const DitherCanvas: React.FC<DitherCanvasProps> = ({
  accent = "#1f2de6",
  opacity = 0.16,
  bare = false,
  className = "",
}) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let animationFrameId: number;
    let width = (canvas.width = canvas.offsetWidth / 2);
    let height = (canvas.height = canvas.offsetHeight / 2);

    const handleResize = () => {
      if (!canvas) return;
      width = canvas.width = canvas.offsetWidth / 2;
      height = canvas.height = canvas.offsetHeight / 2;
    };

    window.addEventListener("resize", handleResize);

    const render = () => {
      const imgData = ctx.createImageData(width, height);
      const data = imgData.data;
      const len = data.length;

      for (let i = 0; i < len; i += 4) {
        if (Math.random() > 0.88) {
          const val = Math.floor(Math.random() * 255);
          data[i] = val;
          data[i + 1] = val;
          data[i + 2] = val;
          data[i + 3] = Math.floor(opacity * 255);
        } else {
          data[i + 3] = 0;
        }
      }

      ctx.putImageData(imgData, 0, 0);
    };

    render();

    return () => {
      window.removeEventListener("resize", handleResize);
      cancelAnimationFrame(animationFrameId);
    };
  }, [opacity]);

  return (
    <div
      className={`pointer-events-none absolute inset-0 overflow-hidden select-none ${
        bare ? "" : "bg-[var(--vessa-accent,#1f2de6)]"
      } ${className}`}
      style={{ backgroundColor: bare ? "transparent" : accent }}
    >
      <canvas
        ref={canvasRef}
        className="h-full w-full object-cover"
        style={{ opacity }}
      />
    </div>
  );
};
```

### Layout Grid Guides & Corner Node Markers

```tsx
// components/primitives/StageGuides.tsx
import React from "react";

interface StageGuidesProps {
  children: React.ReactNode;
  className?: string;
  showNodes?: boolean;
}

export const StageGuides: React.FC<StageGuidesProps> = ({
  children,
  className = "",
  showNodes = true,
}) => {
  return (
    <div className={`relative mx-auto w-[var(--col-wide)] ${className}`}>
      {/* Dashed Horizontal Boundary Rails */}
      <span
        aria-hidden="true"
        className="pointer-events-none absolute top-0 left-1/2 z-10 h-px w-screen -translate-x-1/2"
        style={{
          background: "var(--dash-h)",
          maskImage:
            "linear-gradient(90deg, transparent 0%, black 8%, black 92%, transparent 100%)",
          WebkitMaskImage:
            "linear-gradient(90deg, transparent 0%, black 8%, black 92%, transparent 100%)",
        }}
      />
      <span
        aria-hidden="true"
        className="pointer-events-none absolute bottom-0 left-1/2 z-10 h-px w-screen -translate-x-1/2"
        style={{
          background: "var(--dash-h)",
          maskImage:
            "linear-gradient(90deg, transparent 0%, black 8%, black 92%, transparent 100%)",
          WebkitMaskImage:
            "linear-gradient(90deg, transparent 0%, black 8%, black 92%, transparent 100%)",
        }}
      />

      {/* Structural Corner Nodes (8x8px Pure Accent Rectangles) */}
      {showNodes && (
        <>
          <span
            aria-hidden="true"
            className="pointer-events-none absolute -top-1 -left-1 z-20 size-2 bg-brand-accent"
          />
          <span
            aria-hidden="true"
            className="pointer-events-none absolute -top-1 -right-1 z-20 size-2 bg-brand-accent"
          />
          <span
            aria-hidden="true"
            className="pointer-events-none absolute -bottom-1 -left-1 z-20 size-2 bg-brand-accent"
          />
          <span
            aria-hidden="true"
            className="pointer-events-none absolute -bottom-1 -right-1 z-20 size-2 bg-brand-accent"
          />
        </>
      )}

      {children}
    </div>
  );
};
```

---

## 5. Navigation & Island Headers

### Island Floating Navigation (Dynamic Pill Blur + Dropdowns)

```tsx
// components/nav/FloatingIslandNav.tsx
"use client";

import React, { useState, useEffect, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";

interface NavItem {
  label: string;
  href: string;
  badge?: string;
  description?: string;
}

interface NavProps {
  brandName: string;
  logoSvg: React.ReactNode;
  navLinks: {
    label: string;
    href?: string;
    subItems?: NavItem[];
  }[];
  ctaText?: string;
  ctaHref?: string;
}

export const FloatingIslandNav: React.FC<NavProps> = ({
  brandName,
  logoSvg,
  navLinks,
  ctaText = "Start building free",
  ctaHref = "/signup",
}) => {
  const [isScrolled, setIsScrolled] = useState(false);
  const [activeDropdown, setActiveDropdown] = useState<string | null>(null);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const timeoutRef = useRef<NodeJS.Timeout | null>(null);

  useEffect(() => {
    const handleScroll = () => setIsScrolled(window.scrollY > 32);
    window.addEventListener("scroll", handleScroll);
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  const handleMouseEnter = (label: string) => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    setActiveDropdown(label);
  };

  const handleMouseLeave = () => {
    timeoutRef.current = setTimeout(() => {
      setActiveDropdown(null);
    }, 150);
  };

  return (
    <header className="fixed top-0 left-0 right-0 z-50 pt-3 md:pt-4">
      <div
        className={`mx-auto flex w-[var(--col-main)] max-w-[calc(100vw-1.5rem)] items-center justify-between rounded-xl px-4 py-2 transition-all duration-300 ${
          isScrolled
            ? "bg-paper/90 shadow-floating backdrop-blur-md border border-hairline/60"
            : "bg-paper/60 backdrop-blur-sm border border-transparent"
        }`}
      >
        {/* Brand Logo */}
        <a
          href="/"
          className="inline-flex items-center gap-2 text-ink no-underline"
          aria-label={brandName}
        >
          {logoSvg}
        </a>

        {/* Desktop Links Matrix */}
        <nav
          className="hidden md:flex items-center gap-6"
          aria-label="Main Navigation"
        >
          {navLinks.map((link) => (
            <div
              key={link.label}
              className="relative"
              onMouseEnter={() => link.subItems && handleMouseEnter(link.label)}
              onMouseLeave={handleMouseLeave}
            >
              {link.href ? (
                <a
                  href={link.href}
                  className="text-[13px] font-medium tracking-tight text-ink-soft transition-colors duration-150 hover:text-ink"
                >
                  {link.label}
                </a>
              ) : (
                <button
                  type="button"
                  onClick={() =>
                    setActiveDropdown(
                      activeDropdown === link.label ? null : link.label,
                    )
                  }
                  className="flex items-center gap-1 text-[13px] font-medium tracking-tight text-ink-soft transition-colors duration-150 hover:text-ink bg-transparent border-none cursor-pointer"
                >
                  <span>{link.label}</span>
                  <svg
                    width="12"
                    height="12"
                    viewBox="0 0 12 12"
                    fill="none"
                    className={`transition-transform duration-200 ${
                      activeDropdown === link.label ? "rotate-180" : ""
                    }`}
                  >
                    <path
                      d="M3 4.5L6 7.5L9 4.5"
                      stroke="currentColor"
                      strokeWidth="1.2"
                      strokeLinecap="round"
                    />
                  </svg>
                </button>
              )}

              {/* Popover Card */}
              <AnimatePresence>
                {activeDropdown === link.label && link.subItems && (
                  <motion.div
                    initial={{ opacity: 0, y: 6, scale: 0.98 }}
                    animate={{ opacity: 1, y: 0, scale: 1 }}
                    exit={{ opacity: 0, y: 4, scale: 0.98 }}
                    transition={{ duration: 0.16, ease: [0.33, 1, 0.68, 1] }}
                    className="absolute top-full left-0 mt-2 w-72 rounded-lg border border-hairline bg-paper-bright p-1.5 shadow-popover"
                  >
                    <ul className="flex flex-col gap-0.5 list-none m-0 p-0">
                      {link.subItems.map((sub) => (
                        <li key={sub.label}>
                          <a
                            href={sub.href}
                            className="group flex flex-col gap-0.5 rounded-md p-2 text-decoration-none transition-colors duration-150 hover:bg-paper-secondary"
                          >
                            <span className="text-[13px] font-medium text-ink">
                              {sub.label}
                            </span>
                            {sub.description && (
                              <span className="text-[11px] leading-snug text-ink-faint">
                                {sub.description}
                              </span>
                            )}
                          </a>
                        </li>
                      ))}
                    </ul>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          ))}
        </nav>

        {/* Right CTAs */}
        <div className="hidden md:flex items-center gap-3">
          <a
            href="/login"
            className="text-[13px] font-medium tracking-tight text-ink-soft no-underline transition-colors hover:text-ink px-2"
          >
            Sign in
          </a>
          <a
            href={ctaHref}
            className="inline-flex h-8 items-center justify-center rounded-md bg-ink px-3 text-[13px] font-medium text-white no-underline transition-all hover:bg-ink-soft active:scale-[0.98]"
          >
            {ctaText}
          </a>
        </div>

        {/* Mobile Hamburger Button */}
        <button
          type="button"
          onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
          className="flex md:hidden size-8 items-center justify-center border border-hairline rounded-md bg-transparent text-ink cursor-pointer"
          aria-label="Toggle menu"
        >
          <div className="relative size-4">
            <span
              className={`absolute left-0 block h-0.5 w-full bg-ink transition-transform duration-200 ${
                mobileMenuOpen ? "top-1.5 rotate-45" : "top-0.5"
              }`}
            />
            <span
              className={`absolute left-0 block h-0.5 w-full bg-ink transition-transform duration-200 ${
                mobileMenuOpen ? "top-1.5 -rotate-45" : "bottom-0.5"
              }`}
            />
          </div>
        </button>
      </div>

      {/* Mobile Drawer */}
      <AnimatePresence>
        {mobileMenuOpen && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            className="md:hidden mx-auto mt-2 w-[var(--col-main)] max-w-[calc(100vw-1.5rem)] overflow-hidden rounded-xl border border-hairline bg-paper-bright p-4 shadow-popover"
          >
            <nav className="flex flex-col gap-3">
              {navLinks.map((link) => (
                <div
                  key={link.label}
                  className="border-b border-hairline/50 pb-2"
                >
                  <a
                    href={link.href || "#"}
                    className="text-sm font-medium text-ink no-underline"
                  >
                    {link.label}
                  </a>
                  {link.subItems && (
                    <ul className="mt-1 flex flex-col gap-1 pl-3 list-none">
                      {link.subItems.map((sub) => (
                        <li key={sub.label}>
                          <a
                            href={sub.href}
                            className="text-xs text-ink-soft no-underline"
                          >
                            {sub.label}
                          </a>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              ))}
              <div className="flex flex-col gap-2 pt-2">
                <a
                  href="/login"
                  className="flex h-9 items-center justify-center rounded-md border border-hairline bg-transparent text-xs font-medium text-ink"
                >
                  Sign in
                </a>
                <a
                  href={ctaHref}
                  className="flex h-9 items-center justify-center rounded-md bg-ink text-xs font-medium text-white no-underline"
                >
                  {ctaText}
                </a>
              </div>
            </nav>
          </motion.div>
        )}
      </AnimatePresence>
    </header>
  );
};
```

---

## 6. Micro-Interaction CTA Buttons

### Staggered Letter Hover Animation

```tsx
// components/buttons/StaggeredTextButton.tsx
"use client";

import React from "react";
import { DitherCanvas } from "../primitives/DitherCanvas";

interface StaggeredTextButtonProps {
  label: string;
  href: string;
  variant?: "accent" | "inverted" | "outline";
  className?: string;
}

export const StaggeredTextButton: React.FC<StaggeredTextButtonProps> = ({
  label,
  href,
  variant = "accent",
  className = "",
}) => {
  const characters = label.split("");

  const getVariantStyles = () => {
    switch (variant) {
      case "accent":
        return "bg-brand-accent text-white hover:bg-brand-accent/90";
      case "inverted":
        return "bg-white text-brand-accent hover:bg-white/95";
      case "outline":
        return "border border-hairline bg-transparent text-ink hover:bg-paper-secondary";
    }
  };

  return (
    <a
      href={href}
      className={`group relative inline-flex h-10 flex-none items-center justify-center overflow-hidden rounded-lg px-4 text-sm font-medium tracking-tight no-underline transition-all duration-200 active:scale-[0.97] ${getVariantStyles()} ${className}`}
    >
      {/* Background Dither Noise Texture */}
      {variant === "accent" && (
        <DitherCanvas bare accent="#ffffff" opacity={0.14} />
      )}

      {/* Dual Staggered Line Container */}
      <span className="relative z-10 block h-[1.4em] overflow-hidden leading-[1.4]">
        {/* Line 1: Resting (translates UP on hover) */}
        <span className="flex whitespace-pre">
          {characters.map((char, index) => (
            <span
              key={`rest-${index}`}
              className="inline-block transition-transform duration-300 ease-[cubic-bezier(0.7,0,0.16,1)] group-hover:-translate-y-full"
              style={{ transitionDelay: `${index * 6}ms` }}
            >
              {char === " " ? "\u00A0" : char}
            </span>
          ))}
        </span>

        {/* Line 2: Hover Target (translates IN from bottom) */}
        <span className="absolute inset-0 flex whitespace-pre">
          {characters.map((char, index) => (
            <span
              key={`hover-${index}`}
              className="inline-block translate-y-full transition-transform duration-300 ease-[cubic-bezier(0.7,0,0.16,1)] group-hover:translate-y-0"
              style={{ transitionDelay: `${index * 6}ms` }}
            >
              {char === " " ? "\u00A0" : char}
            </span>
          ))}
        </span>
      </span>
    </a>
  );
};
```

---

## 7. Hero Blueprint & Infinite Marquee

```tsx
// components/sections/EditorialHero.tsx
"use client";

import React, { useState } from "react";
import { StaggeredTextButton } from "../buttons/StaggeredTextButton";

interface EditorialHeroProps {
  announcementBadge?: {
    tag: string;
    text: string;
    href: string;
  };
  title: string;
  subtitle: string;
  primaryCta: { label: string; href: string };
  secondaryCta?: { label: string; href: string };
  commandSnippet?: string;
  proofBadgeCount?: string;
  proofBadgeLabel?: string;
  logos: { name: string; src: string; width: number }[];
}

export const EditorialHero: React.FC<EditorialHeroProps> = ({
  announcementBadge,
  title,
  subtitle,
  primaryCta,
  secondaryCta,
  commandSnippet,
  proofBadgeCount = "500+",
  proofBadgeLabel = "engineers onboarded",
  logos,
}) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    if (!commandSnippet) return;
    navigator.clipboard.writeText(commandSnippet);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <section className="relative overflow-hidden pt-28 pb-14 md:pt-36 md:pb-20">
      <div className="mx-auto flex max-w-[var(--col-main)] flex-col items-center px-4 text-center">
        {/* Announcement Badge */}
        {announcementBadge && (
          <a
            href={announcementBadge.href}
            className="group mb-6 inline-flex items-center gap-2 rounded-full border border-hairline/80 bg-paper-secondary py-1 pr-3 pl-1 text-xs font-medium tracking-tight text-ink no-underline transition-colors hover:border-ink-soft"
          >
            <span className="rounded-full bg-brand-accent px-2 py-0.5 text-[11px] font-medium text-white">
              {announcementBadge.tag}
            </span>
            <span>{announcementBadge.text}</span>
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              fill="none"
              className="transition-transform group-hover:translate-x-0.5"
            >
              <path
                d="M3.5 2L6.5 5L3.5 8"
                stroke="currentColor"
                strokeWidth="1.2"
                strokeLinecap="round"
              />
            </svg>
          </a>
        )}

        {/* Master Heading (Strict weight <= 500) */}
        <h1 className="max-w-4xl text-balance text-4xl font-normal leading-[1.04] tracking-tightest text-ink sm:text-5xl md:text-6xl">
          {title}
        </h1>

        {/* Subtitle */}
        <p className="mt-5 max-w-2xl text-base font-normal leading-normal tracking-tight text-ink-soft md:text-lg">
          {subtitle}
        </p>

        {/* Interactive Action Row */}
        <div className="mt-8 flex flex-wrap items-center justify-center gap-3">
          <StaggeredTextButton
            label={primaryCta.label}
            href={primaryCta.href}
            variant="accent"
          />
          {secondaryCta && (
            <a
              href={secondaryCta.href}
              className="inline-flex h-10 items-center justify-center rounded-lg border border-hairline bg-paper-bright px-4 text-sm font-medium tracking-tight text-ink no-underline transition-all hover:bg-paper-secondary active:scale-[0.98]"
            >
              {secondaryCta.label}
            </a>
          )}
        </div>

        {/* Proof Metric Indicator */}
        <div className="mt-4 flex items-center gap-2 text-xs text-ink-soft">
          <span className="rounded-full bg-brand-50 px-2 py-0.5 font-mono text-[11px] font-medium text-brand-700">
            {proofBadgeCount}
          </span>
          <span>{proofBadgeLabel}</span>
        </div>

        {/* Optional Terminal CLI Copy Box */}
        {commandSnippet && (
          <div className="mt-8 flex w-full max-w-lg items-center justify-between rounded-lg border border-hairline bg-[#121316] p-3 text-left shadow-hairline">
            <code className="truncate pr-4 font-mono text-xs text-[#f9fafb]">
              {commandSnippet}
            </code>
            <button
              type="button"
              onClick={handleCopy}
              className="flex size-7 shrink-0 items-center justify-center rounded border border-gray-700 bg-gray-800 text-gray-300 transition-colors hover:border-gray-500 hover:text-white cursor-pointer"
              aria-label="Copy snippet"
            >
              {copied ? (
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="#56eaaf"
                  strokeWidth="2"
                >
                  <polyline points="20 6 9 17 4 12" />
                </svg>
              ) : (
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.5"
                >
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
              )}
            </button>
          </div>
        )}

        {/* Infinite Logo Marquee Strip */}
        <div className="mt-14 w-full overflow-hidden mask-marquee-edges">
          <div className="animate-marquee-infinite items-center gap-12 py-2">
            {logos.concat(logos).map((logo, idx) => (
              <div
                key={idx}
                className="flex shrink-0 items-center justify-center opacity-70 transition-opacity hover:opacity-100"
              >
                <img
                  src={logo.src}
                  alt={logo.name}
                  width={logo.width}
                  height="24"
                  className="h-5 md:h-6 w-auto max-w-none grayscale contrast-125"
                />
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
};
```

---

## 8. Interactive Morphing Tabs & Showcase

```tsx
// components/showcase/MorphingTabShowcase.tsx
"use client";

import React, { useState, useRef, useEffect } from "react";
import { StageGuides } from "../primitives/StageGuides";
import { DitherCanvas } from "../primitives/DitherCanvas";

interface TabSection {
  id: string;
  label: string;
  imgSrc: string;
  alt: string;
}

interface ShowcaseProps {
  tabs: TabSection[];
}

export const MorphingTabShowcase: React.FC<ShowcaseProps> = ({ tabs }) => {
  const [activeTab, setActiveTab] = useState(0);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const [pillStyle, setPillStyle] = useState<{
    width: number;
    height: number;
    left: number;
  }>({
    width: 96,
    height: 36,
    left: 4,
  });

  useEffect(() => {
    const el = tabRefs.current[activeTab];
    if (el) {
      setPillStyle({
        width: el.offsetWidth,
        height: el.offsetHeight,
        left: el.offsetLeft,
      });
    }
  }, [activeTab]);

  const handleNext = () => {
    setActiveTab((prev) => (prev + 1) % tabs.length);
  };

  return (
    <section className="py-16 md:py-24">
      <StageGuides>
        <div className="relative overflow-hidden rounded-xl border border-hairline bg-paper-surface p-4 sm:p-6 md:p-10">
          <DitherCanvas bare accent="#ffffff" opacity={0.12} />

          {/* Interactive Navigation Tabbar */}
          <div className="relative z-20 flex items-center justify-center gap-2">
            <div className="relative inline-flex max-w-full items-center gap-1 overflow-x-auto rounded-xl bg-paper-secondary p-1 no-scrollbar">
              {/* Dynamic Sliding Pill Indicator */}
              <span
                aria-hidden="true"
                className="absolute top-1 rounded-lg bg-paper-bright shadow-hairline transition-all duration-300 ease-[cubic-bezier(0.3,0.9,0.1,1)]"
                style={{
                  width: `${pillStyle.width}px`,
                  height: `${pillStyle.height}px`,
                  transform: `translateX(${pillStyle.left - 4}px)`,
                }}
              />

              {tabs.map((tab, idx) => (
                <button
                  key={tab.id}
                  ref={(el) => {
                    tabRefs.current[idx] = el;
                  }}
                  type="button"
                  onClick={() => setActiveTab(idx)}
                  className={`relative z-10 rounded-lg px-3.5 py-2 text-xs md:text-sm font-medium tracking-tight transition-colors duration-150 cursor-pointer border-none bg-transparent ${
                    activeTab === idx
                      ? "text-ink"
                      : "text-ink-faint hover:text-ink-soft"
                  }`}
                >
                  {tab.label}
                </button>
              ))}
            </div>

            {/* Circular Progress Advance Button */}
            <button
              type="button"
              onClick={handleNext}
              className="relative flex size-9 shrink-0 items-center justify-center rounded-full bg-paper-secondary text-ink transition-transform active:scale-95 cursor-pointer border-none"
              aria-label="Next tab"
            >
              <svg
                className="absolute inset-0 size-full -rotate-90"
                viewBox="0 0 36 36"
              >
                <circle
                  cx="18"
                  cy="18"
                  r="16"
                  fill="none"
                  stroke="var(--vessa-accent, #1f2de6)"
                  strokeWidth="2"
                  strokeDasharray="100.5"
                  strokeDashoffset={`${100.5 - (100.5 * (activeTab + 1)) / tabs.length}`}
                  strokeLinecap="round"
                  className="transition-all duration-300"
                />
              </svg>
              <svg
                width="12"
                height="12"
                viewBox="0 0 12 12"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
              >
                <path
                  d="M2.5 6H9.5M6.5 3L9.5 6L6.5 9"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </button>
          </div>

          {/* Crossfade Image Screen Layer */}
          <div className="relative z-10 mt-8 grid aspect-[1800/1129] w-full overflow-hidden rounded-lg border border-hairline bg-paper-bright">
            {tabs.map((tab, idx) => (
              <img
                key={tab.id}
                src={tab.imgSrc}
                alt={tab.alt}
                className={`col-start-1 row-start-1 size-full object-cover transition-all duration-300 ${
                  activeTab === idx
                    ? "opacity-100 scale-100 pointer-events-auto"
                    : "opacity-0 scale-[0.985] pointer-events-none"
                }`}
                loading={idx === 0 ? "eager" : "lazy"}
              />
            ))}
          </div>
        </div>
      </StageGuides>
    </section>
  );
};
```

---

## 9. Pricing Engine & Precision Range Slider

```tsx
// components/pricing/PrecisionUsageCalculator.tsx
"use client";

import React, { useState } from "react";

export const PrecisionUsageCalculator: React.FC = () => {
  // Slider state on log2 curve: 0 to 10 scale
  const [sliderVal, setSliderVal] = useState<number>(1.58496); // default log2(3) => 3 vCPUs

  // Mathematical logic
  const vCPUs = Math.max(1, Math.round(Math.pow(2, sliderVal)));
  const samples = vCPUs * 6_000_000;
  const cost = Math.max(50, Math.ceil(samples / 100_000_000) * 50);

  const ticks = [
    { label: "1", pos: "0%" },
    { label: "2", pos: "10%" },
    { label: "4", pos: "20%" },
    { label: "8", pos: "30%" },
    { label: "16", pos: "40%" },
    { label: "32", pos: "50%" },
    { label: "64", pos: "60%" },
    { label: "128", pos: "70%" },
    { label: "256", pos: "80%" },
    { label: "512", pos: "90%" },
    { label: "1024", pos: "100%" },
  ];

  return (
    <div className="mx-auto w-full max-w-4xl rounded-xl border border-hairline bg-paper-surface p-6 md:p-8 shadow-hairline">
      <div className="grid gap-8 lg:grid-cols-[1fr_1px_220px] lg:items-center">
        {/* Left Column: Logarithmic Estimator */}
        <div className="flex flex-col">
          <h3 className="text-lg font-medium tracking-tight text-ink md:text-xl">
            Usage Estimator
          </h3>
          <p className="mt-1 max-w-md text-sm leading-normal text-ink-soft">
            Estimate your continuous data throughput. On average 1 vCPU
            generates ~6,000,000 profiling samples per month.
          </p>

          {/* Metric Outputs */}
          <div className="mt-6 flex items-center justify-between text-xs font-mono text-ink-soft">
            <span className="font-medium text-ink">{vCPUs} vCPUs</span>
            <span>{samples.toLocaleString()} samples/mo</span>
          </div>

          {/* Custom Track Range Slider */}
          <div className="relative mt-4 h-12 w-full">
            {/* Background Base Rail */}
            <div className="pointer-events-none absolute top-[9px] left-0 right-0 h-px bg-hairline" />

            {/* Active Highlight Fill */}
            <div
              className="pointer-events-none absolute top-[9px] left-0 h-px bg-brand-accent transition-all duration-75"
              style={{ width: `${(sliderVal / 10) * 100}%` }}
            />

            {/* Tick Mark Separators */}
            <div className="pointer-events-none absolute inset-0">
              {ticks.map((tick, idx) => (
                <div
                  key={idx}
                  className="absolute top-0 flex -translate-x-1/2 flex-col items-center gap-3"
                  style={{ left: tick.pos }}
                >
                  <span className="h-2 w-px bg-hairline" />
                  <span className="text-[10px] font-mono text-ink-faint">
                    {tick.label}
                  </span>
                </div>
              ))}
            </div>

            {/* Invisible Range Input */}
            <input
              type="range"
              min="0"
              max="10"
              step="0.01"
              value={sliderVal}
              onChange={(e) => setSliderVal(parseFloat(e.target.value))}
              className="absolute -top-1 left-0 h-6 w-full cursor-pointer opacity-0"
              aria-label="vCPU range slider"
            />
          </div>
        </div>

        {/* Vertical Dashed Divider */}
        <div
          className="hidden h-36 w-px bg-hairline lg:block"
          style={{ background: "var(--dash-v)" }}
        />

        {/* Right Output Column */}
        <div className="flex flex-col items-center justify-center lg:items-start lg:pl-6">
          <span className="text-xs font-medium uppercase tracking-wider text-ink-faint">
            Estimated Cost
          </span>
          <div className="mt-1 flex items-baseline gap-1">
            <span className="text-4xl font-normal tracking-tight text-ink md:text-5xl">
              ${cost}
            </span>
            <span className="text-xs text-ink-soft">/ month</span>
          </div>
          <a
            href="/signup"
            className="mt-4 inline-flex h-9 w-full items-center justify-center rounded-md bg-ink text-xs font-medium text-white no-underline transition-colors hover:bg-ink-soft active:scale-[0.98]"
          >
            Deploy agent
          </a>
        </div>
      </div>
    </div>
  );
};
```

---

## 10. Interactive Bento Grid System

```tsx
// components/bento/InteractiveBentoGrid.tsx
"use client";

import React, { useState } from "react";

export const InteractiveBentoGrid: React.FC = () => {
  const [copiedToken, setCopiedToken] = useState<string | null>(null);

  const copyHex = (hex: string) => {
    navigator.clipboard.writeText(hex);
    setCopiedToken(hex);
    setTimeout(() => setCopiedToken(null), 2000);
  };

  return (
    <section className="py-16 md:py-24">
      <div className="mx-auto max-w-[var(--col-wide)] px-4">
        <h2 className="mb-10 text-3xl font-normal tracking-tightest text-ink md:text-4xl">
          Engineered for line-level precision.
        </h2>

        {/* 2-Column Responsive Bento Grid with Dashed Containment */}
        <div className="grid grid-cols-1 md:grid-cols-2 border-t border-hairline">
          {/* Card 1: Live Clipboard Tokens */}
          <article className="flex flex-col justify-between border-b md:border-r border-hairline p-6 md:p-8">
            <div className="flex flex-col gap-2 rounded-lg border border-hairline bg-paper-surface p-3">
              {[
                { name: "Primary Ink", hex: "#101216" },
                { name: "Surface Paper", hex: "#EEF0F3" },
                { name: "Accent Blue", hex: "#1F2DE6" },
              ].map((token) => (
                <div
                  key={token.hex}
                  onClick={() => copyHex(token.hex)}
                  className="flex items-center justify-between rounded-md p-2 hover:bg-paper-secondary transition-colors cursor-pointer"
                >
                  <div className="flex items-center gap-2.5">
                    <span
                      className="size-4 rounded"
                      style={{ backgroundColor: token.hex }}
                    />
                    <span className="text-xs font-medium text-ink">
                      {token.name}
                    </span>
                  </div>
                  <span className="font-mono text-xs text-ink-faint">
                    {copiedToken === token.hex ? "Copied!" : token.hex}
                  </span>
                </div>
              ))}
            </div>

            <div className="mt-8">
              <h3 className="text-base font-medium text-ink">
                Zero eyedropper friction
              </h3>
              <p className="mt-1 text-sm text-ink-soft leading-normal">
                Every design token sits on the page as raw clipboard data. Click
                any row to copy production variables instantly.
              </p>
            </div>
          </article>

          {/* Card 2: Connected Ecosystem Fan */}
          <article className="flex flex-col justify-between border-b border-hairline p-6 md:p-8">
            <div className="flex flex-col items-center justify-center rounded-lg border border-hairline bg-paper-surface p-6">
              <span className="rounded-full bg-brand-accent px-3 py-1 text-xs font-medium text-white">
                MCP Protocol
              </span>
              <svg className="my-2 h-8 w-full" viewBox="0 0 200 30" fill="none">
                <path
                  d="M100 0 V30 M100 0 C100 20, 20 10, 20 30 M100 0 C100 20, 180 10, 180 30"
                  stroke="var(--m-border)"
                  strokeWidth="1"
                  strokeDasharray="3 3"
                />
              </svg>
              <div className="flex flex-wrap items-center justify-center gap-2">
                {["Claude", "Cursor", "Codex", "Gemini"].map((client) => (
                  <span
                    key={client}
                    className="rounded border border-hairline bg-paper-bright px-2.5 py-1 text-xs font-medium text-ink"
                  >
                    {client}
                  </span>
                ))}
              </div>
            </div>

            <div className="mt-8">
              <h3 className="text-base font-medium text-ink">
                AI Agent Compatibility
              </h3>
              <p className="mt-1 text-sm text-ink-soft leading-normal">
                Structured `brand.json` and continuous MCP servers allow tools
                like Claude Code and Cursor to build without manual asset
                extraction.
              </p>
            </div>
          </article>
        </div>
      </div>
    </section>
  );
};
```

---

## 11. Minimalist FAQ Accordion & Dark Anchor

```tsx
// components/sections/FAQAndMonochromeCTA.tsx
"use client";

import React, { useState } from "react";
import { StaggeredTextButton } from "../buttons/StaggeredTextButton";

interface FAQItem {
  q: string;
  a: string;
}

interface FAQProps {
  items: FAQItem[];
  supportEmail?: string;
}

export const FAQAndMonochromeCTA: React.FC<FAQProps> = ({
  items,
  supportEmail = "support@domain.com",
}) => {
  const [openIdx, setOpenIdx] = useState<number | null>(0);

  const toggle = (idx: number) => {
    setOpenIdx(openIdx === idx ? null : idx);
  };

  return (
    <>
      {/* FAQ Accordion Section */}
      <section className="border-t border-hairline py-16 md:py-24">
        <div className="mx-auto max-w-[var(--col-main)] px-4">
          <div className="grid gap-10 lg:grid-cols-[320px_1fr]">
            {/* Header Column */}
            <div>
              <h2 className="text-2xl font-normal tracking-tight text-ink md:text-3xl">
                Frequently asked questions.
              </h2>
              <p className="mt-2 text-sm text-ink-soft">
                Need more information? Email us directly at{" "}
                <a
                  href={`mailto:${supportEmail}`}
                  className="text-ink underline decoration-hairline hover:decoration-ink"
                >
                  {supportEmail}
                </a>
              </p>
            </div>

            {/* Accordion List Column */}
            <div className="flex flex-col border-t border-hairline">
              {items.map((item, idx) => {
                const isOpen = openIdx === idx;
                return (
                  <div key={idx} className="border-b border-hairline">
                    <button
                      type="button"
                      onClick={() => toggle(idx)}
                      className="flex w-full items-center justify-between py-4 text-left font-medium text-ink transition-colors hover:text-ink-soft cursor-pointer border-none bg-transparent"
                    >
                      <span className="text-base tracking-tight">{item.q}</span>
                      <span className="relative flex size-5 shrink-0 items-center justify-center">
                        <svg
                          width="14"
                          height="14"
                          viewBox="0 0 14 14"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="1.25"
                          className={`transition-transform duration-200 ${isOpen ? "rotate-45" : ""}`}
                        >
                          <path d="M7 1V13M1 7H13" strokeLinecap="round" />
                        </svg>
                      </span>
                    </button>
                    {isOpen && (
                      <div className="pb-5 text-sm leading-relaxed text-ink-soft">
                        {item.a}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </section>

      {/* Terminal Dark High-Contrast CTA Section */}
      <section className="relative overflow-hidden bg-[#101216] py-16 md:py-24 text-white">
        <div className="relative z-10 mx-auto flex max-w-[var(--col-narrow)] flex-col items-center px-4 text-center">
          <h2 className="text-3xl font-normal leading-tight tracking-tightest sm:text-4xl md:text-5xl">
            Start building with full line-level clarity today.
          </h2>
          <p className="mt-4 max-w-lg text-sm text-gray-400 md:text-base leading-normal">
            Zero configuration required. Free for up to 14 days, pay only when
            your workloads publish to production.
          </p>

          <div className="mt-8 flex flex-wrap items-center justify-center gap-3">
            <StaggeredTextButton
              label="Start free 14-day trial"
              href="/signup"
              variant="inverted"
            />
            <a
              href="/schedule"
              className="inline-flex h-10 items-center justify-center rounded-lg border border-gray-700 bg-transparent px-4 text-sm font-medium text-white no-underline transition-colors hover:bg-white/10 active:scale-[0.98]"
            >
              Book a live demo
            </a>
          </div>
        </div>
      </section>
    </>
  );
};
```

---

## 12. Swiss Editorial Footer

```tsx
// components/footer/SwissEditorialFooter.tsx
import React from "react";

interface FooterLink {
  label: string;
  href: string;
}

interface FooterColumn {
  heading: string;
  links: FooterLink[];
}

interface FooterProps {
  brandLogo: React.ReactNode;
  tagline: string;
  columns: FooterColumn[];
  legalText?: string;
}

export const SwissEditorialFooter: React.FC<FooterProps> = ({
  brandLogo,
  tagline,
  columns,
  legalText = "© 2026 Polar System Inc. All rights reserved.",
}) => {
  return (
    <footer className="border-t border-hairline bg-paper-bright pt-12 pb-8 text-ink">
      <div className="mx-auto max-w-[var(--col-wide)] px-4">
        <div className="grid grid-cols-1 gap-8 md:grid-cols-[280px_1fr] lg:gap-16">
          {/* Brand Identity Column */}
          <div className="flex flex-col gap-3">
            <a
              href="/"
              className="inline-flex items-center text-ink no-underline"
            >
              {brandLogo}
            </a>
            <p className="text-xs text-ink-soft leading-normal">{tagline}</p>
          </div>

          {/* Dynamic Link Columns with Dashed Separators */}
          <div className="grid grid-cols-2 gap-8 sm:grid-cols-4">
            {columns.map((col, idx) => (
              <div key={idx} className="flex flex-col gap-3">
                <span className="font-mono text-xs uppercase tracking-wider text-ink-faint">
                  {col.heading}
                </span>
                <ul className="flex flex-col gap-2 list-none m-0 p-0">
                  {col.links.map((link, lIdx) => (
                    <li key={lIdx}>
                      <a
                        href={link.href}
                        className="text-xs text-ink-soft no-underline transition-colors hover:text-ink"
                      >
                        {link.label}
                      </a>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>

        {/* Hairline Bottom Legal Bar */}
        <div className="mt-12 flex flex-wrap items-center justify-between border-t border-hairline pt-6 text-xs text-ink-faint">
          <span>{legalText}</span>
          <div className="flex gap-4">
            <a
              href="/privacy"
              className="text-ink-faint no-underline hover:text-ink"
            >
              Privacy
            </a>
            <a
              href="/terms"
              className="text-ink-faint no-underline hover:text-ink"
            >
              Terms
            </a>
            <a
              href="/security"
              className="text-ink-faint no-underline hover:text-ink"
            >
              Security
            </a>
          </div>
        </div>
      </div>
    </footer>
  );
};
```

---

## 13. Dynamic Theme Archetype Switching

To switch between the 4 extracted visual archetypes, attach the corresponding `data-theme` attribute to the root layout container:

```tsx
// Archetype 1: Swiss Editorial Minimal (Default)
<div data-theme="swiss-paper" className="bg-[var(--vessa-paper)] text-[var(--vessa-ink)] font-sans antialiased" />

// Archetype 2: Polar Signals High-Tech Light
<div data-theme="polar-tech" className="bg-white text-[#09090B] font-sans antialiased" />

// Archetype 3: Kira Warm Academy (Serif Headings + Vanilla Surfaces)
<div data-theme="warm-academy" className="bg-[#fdfdfd] text-[#1A1A1A] font-sans antialiased" />

// Archetype 4: Terminal Obsidian (Dark Mode Platform)
<div data-theme="terminal-obsidian" className="bg-[#101216] text-[#F9FAFB] font-sans antialiased" />
```

## 14. Badge UI/UX

```tsx
type BadgeVariant = "sky" | "violet" | "yellow" | "rose" | "green" | "orange";

interface BadgeProps {
  children: React.ReactNode;
  variant?: BadgeVariant;
}

const variantStyles: Record<BadgeVariant, string> = {
  sky: "bg-sky-500/10 text-sky-700",
  violet: "bg-violet-500/10 text-violet-700",
  yellow: "bg-yellow-500/10 text-yellow-700",
  rose: "bg-rose-500/10 text-rose-700",
  green: "bg-green-500/10 text-green-700",
  orange: "bg-orange-500/10 text-orange-700",
};

export function Badge({ children, variant = "orange" }: BadgeProps) {
  return (
    <span
      className={[
        "inline-flex items-center justify-center",
        "rounded-[3px] px-2.5 py-1",
        "text-center text-[14px] leading-5 tracking-[-0.09px]",
        variantStyles[variant],
      ].join(" ")}
    >
      {children}
    </span>
  );
}
```

---

## 15. Verification Checklist

Before deploying any landing page built with this system, verify:

- [ ] No element exceeds `font-weight: 500`.
- [ ] No element contains standard Tailwind `shadow-lg`, `shadow-xl`, or `shadow-2xl` without a hairline border ring.
- [ ] All numeric metrics and stats utilize `tabular-nums` or `font-mono`.
- [ ] All horizontal separators use 1px hairlines or dashed SVG gradients.
- [ ] Responsive behavior utilizes fluid clamps (`clamp()`) to avoid layout jumps across viewport sizes.
